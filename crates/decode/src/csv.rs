//! CSV parsing: the schema from the header and the first rows, then rows parsed in chunks (in parallel when no
//! field is quoted, since a quoted field may hold a newline).
//!
//! - Delimiter: comma, semicolon or tab, whichever the header row has most of.
//! - Header names are trimmed; empty ones become `column N`. A unit in brackets or parentheses after a space
//!   (`Speed [m/s]`, `Temp (°C)`) becomes the channel's unit; `accel[0]` is a name. Repeated names get ` (2)`, ` (3)`.
//! - Time: the first column named like a time (`time`, `t`, `timestamp`, `date`, `time_s`, `t_ms`, …) whose values
//!   parse, else the first column of ISO dates; numbers are seconds unless the name has a unit (`_ms`, `_us`,
//!   `_ns`, `[ms]`). With no time column the x axis is the row number.
//! - A column is numeric when every non-missing value in the first rows parses; others (text) are skipped.
//!   Missing: empty, `NA`, `N/A`, `NaN`, `null`, `-`. Missing or unparsable numbers become NaN; rows whose time
//!   doesn't parse are skipped and counted.

use std::ops::Range;
use std::path::Path;

use csv_core::{ReadRecordResult, Reader, ReaderBuilder};
use tlmbench_core::Timestamp;
use tlmbench_core::datetime::{parse_decimal, parse_iso};

use crate::error::{Error, Result};
use crate::profile::{ColumnKind, ImportProfile, RateChoice, TimeChoice};

/// Rows used to infer column kinds.
const SAMPLE_ROWS: usize = 10_000;
/// Target chunk size for parallel parsing.
pub const CHUNK_BYTES: usize = 8 << 20;

#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    pub delimiter: u8,
    /// Byte offset of the first data row.
    pub data_start: usize,
    pub columns: Vec<Column>,
    pub time: TimeColumn,
    /// Read with the general CSV reader, in one piece: quotes were seen (a quoted field may hold a newline, so the
    /// data can't be split at arbitrary line ends). Set when the sampled rows have a quote, or when a chunk read on
    /// the fast path meets one (`Rows::quotes`) and the file is read again.
    pub quoted: bool,
    /// Rows are at this rate from zero (a profile's assumed rate; with `TimeColumn::Index`).
    pub index_rate: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub unit: Option<String>,
    pub kind: Kind,
    /// The name as the file spells it (what an import profile finds the column by).
    pub source: String,
    pub category: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Number,
    Text,
    Time,
    /// Left out by an import profile.
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeColumn {
    /// Numbers in this unit (nanoseconds per unit).
    Numeric {
        index: usize,
        unit_ns: i64,
    },
    Iso {
        index: usize,
    },
    /// No time column: row numbers.
    Index,
}

impl Schema {
    pub fn numeric_columns(&self) -> impl Iterator<Item = (usize, &Column)> {
        self.columns.iter().enumerate().filter(|(_, c)| c.kind == Kind::Number)
    }
}

/// Rows parsed from one chunk: the time of each row (None with index time) and each numeric column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows {
    pub t: Vec<Timestamp>,
    pub columns: Vec<Vec<f64>>,
    pub rows: usize,
    /// Rows left out because their time didn't parse.
    pub skipped: usize,
    /// The fast path met a quote character and stopped: read the file again with `Schema::quoted` set.
    pub quotes: bool,
}

/// Cheap check for a text file worth parsing as CSV: the extension, or printable text with a delimiter.
pub fn looks_like_csv(path: &Path, head: &[u8]) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if matches!(ext.as_deref(), Some("csv" | "tsv")) {
        return true;
    }
    let line = head[comment_lines(head)..].split(|&c| c == b'\n').next().unwrap_or_default();
    !head.contains(&0) && line.iter().any(|c| matches!(c, b',' | b';' | b'\t'))
}

/// A reader of rows: advance, then read fields of the current row.
trait RowReader {
    fn next(&mut self) -> bool;
    fn field(&self, i: usize) -> &[u8];
    fn is_blank(&self) -> bool;
}

/// Rows of data with no quote characters: lines split at the delimiter with memchr (the fast path; about 10x
/// faster than the general CSV state machine).
struct Lines<'a> {
    input: &'a [u8],
    pos: usize,
    line: &'a [u8],
    /// Delimiter positions in `line`, then its length.
    ends: Vec<usize>,
    delimiter: u8,
}

impl<'a> Lines<'a> {
    fn new(delimiter: u8, input: &'a [u8]) -> Self {
        Self { input, pos: 0, line: &[], ends: Vec::with_capacity(64), delimiter }
    }
}

impl RowReader for Lines<'_> {
    fn next(&mut self) -> bool {
        if self.pos >= self.input.len() {
            return false;
        }
        let rest = &self.input[self.pos..];
        let end = memchr::memchr(b'\n', rest).unwrap_or(rest.len());
        self.pos += end + 1;
        let line = &rest[..end];
        self.line = line.strip_suffix(b"\r").unwrap_or(line);
        self.ends.clear();
        self.ends.extend(memchr::memchr_iter(self.delimiter, self.line));
        self.ends.push(self.line.len());
        true
    }

    #[inline]
    fn field(&self, i: usize) -> &[u8] {
        match self.ends.get(i) {
            Some(&end) => &self.line[if i == 0 { 0 } else { self.ends[i - 1] + 1 }..end],
            None => &[],
        }
    }

    fn is_blank(&self) -> bool {
        self.line.iter().all(u8::is_ascii_whitespace)
    }
}

/// One record's fields, reusing buffers between records.
struct Records<'a> {
    rdr: Reader,
    input: &'a [u8],
    pos: usize,
    out: Vec<u8>,
    ends: Vec<usize>,
    nfields: usize,
    done: bool,
}

impl<'a> Records<'a> {
    fn new(delimiter: u8, input: &'a [u8]) -> Self {
        Self {
            rdr: ReaderBuilder::new().delimiter(delimiter).build(),
            input,
            pos: 0,
            out: vec![0; 4096],
            ends: vec![0; 64],
            nfields: 0,
            done: false,
        }
    }

    /// Reads the next record; false at the end. Afterwards `pos` is where the next record starts.
    fn next(&mut self) -> bool {
        if self.done {
            return false;
        }
        let (mut nout, mut nend) = (0, 0);
        loop {
            let (res, nin, o, e) =
                self.rdr.read_record(&self.input[self.pos..], &mut self.out[nout..], &mut self.ends[nend..]);
            self.pos += nin;
            nout += o;
            nend += e;
            match res {
                ReadRecordResult::InputEmpty => {
                    if self.pos >= self.input.len() {
                        // flush the last record (no trailing newline)
                        let (res, _, _, e) = self.rdr.read_record(&[], &mut self.out[nout..], &mut self.ends[nend..]);
                        nend += e;
                        if res == ReadRecordResult::Record {
                            self.nfields = nend;
                            return true;
                        }
                        self.done = true;
                        return false;
                    }
                }
                ReadRecordResult::OutputFull => {
                    let n = self.out.len();
                    self.out.resize(n * 2, 0);
                }
                ReadRecordResult::OutputEndsFull => {
                    let n = self.ends.len();
                    self.ends.resize(n * 2, 0);
                }
                ReadRecordResult::Record => {
                    self.nfields = nend;
                    return true;
                }
                ReadRecordResult::End => {
                    self.done = true;
                    return false;
                }
            }
        }
    }

    fn field_(&self, i: usize) -> &[u8] {
        if i >= self.nfields {
            return &[];
        }
        let start = if i == 0 { 0 } else { self.ends[i - 1] };
        &self.out[start..self.ends[i]]
    }

    fn is_blank_(&self) -> bool {
        (0..self.nfields).all(|i| self.field_(i).iter().all(u8::is_ascii_whitespace))
    }
}

impl RowReader for Records<'_> {
    fn next(&mut self) -> bool {
        Records::next(self)
    }

    fn field(&self, i: usize) -> &[u8] {
        self.field_(i)
    }

    fn is_blank(&self) -> bool {
        self.is_blank_()
    }
}

fn trim(b: &[u8]) -> &[u8] {
    b.trim_ascii()
}

fn is_missing(b: &[u8]) -> bool {
    let b = trim(b);
    b.is_empty()
        || b == b"-"
        || b.eq_ignore_ascii_case(b"na")
        || b.eq_ignore_ascii_case(b"n/a")
        || b.eq_ignore_ascii_case(b"nan")
        || b.eq_ignore_ascii_case(b"null")
}

fn parse_number(b: &[u8]) -> Option<f64> {
    let b = trim(b);
    lexical_core::parse::<f64>(b).ok().or_else(|| if b.starts_with(b"+") { parse_number(&b[1..]) } else { None })
}

/// Time-like names, and the unit (nanoseconds per unit) from the name's suffix or its bracketed unit.
pub(crate) fn time_unit(name: &str, unit: Option<&str>) -> Option<i64> {
    let base = name.trim().to_ascii_lowercase();
    let base = base.as_str();
    let unit_ns = |u: &str| match u {
        "s" | "sec" | "secs" | "seconds" => Some(1_000_000_000),
        "ms" => Some(1_000_000),
        "us" | "µs" => Some(1_000),
        "ns" => Some(1),
        _ => None,
    };
    let (stem, suffix_unit) = match base.rsplit_once('_') {
        Some((stem, u)) if unit_ns(u).is_some() => (stem, unit_ns(u)),
        _ => (base, None),
    };
    let named =
        matches!(stem, "time" | "t" | "timestamp" | "ts" | "date" | "datetime" | "elapsed" | "time_since_start");
    named.then(|| unit.and_then(|u| unit_ns(&u.to_ascii_lowercase())).or(suffix_unit).unwrap_or(1_000_000_000))
}

/// `Speed [m/s]` → (`Speed`, `m/s`); `accel[0]` stays whole (no space before the bracket).
pub(crate) fn split_unit(name: &str) -> (&str, Option<String>) {
    let n = name.trim_end();
    for (open, close) in [('[', ']'), ('(', ')')] {
        if let Some(stripped) = n.strip_suffix(close)
            && let Some(i) = stripped.rfind(open)
            && stripped[..i].ends_with(' ')
            && i + 1 < stripped.len()
        {
            return (stripped[..i].trim_end(), Some(stripped[i + 1..].trim().to_owned()));
        }
    }
    (n, None)
}

/// Bytes taken by `#` comment lines before the header (exports note a resampled grid there).
fn comment_lines(bytes: &[u8]) -> usize {
    let mut at = 0;
    while bytes[at..].starts_with(b"#") {
        match memchr::memchr(b'\n', &bytes[at..]) {
            Some(i) => at += i + 1,
            None => return bytes.len(),
        }
    }
    at
}

/// Reads the header and the first rows to decide the schema. `#` comment lines before the header are skipped.
pub fn scan(all: &[u8]) -> Result<Schema> {
    scan_with(all, None)
}

/// The column names of a header as [`scan`] sees them (what profiles are keyed by), without reading any rows.
pub fn header_names(all: &[u8]) -> Result<Vec<String>> {
    let bytes = &all[comment_lines(all)..];
    let end = memchr::memchr(b'\n', bytes).map_or(bytes.len(), |i| i + 1);
    Ok(scan_with(&bytes[..end], None)?.columns.into_iter().map(|c| c.source).collect())
}

/// [`scan`] with an import profile: its names, units and types replace what was detected, and its time column
/// and rate decide the x axis.
pub fn scan_with(all: &[u8], profile: Option<&ImportProfile>) -> Result<Schema> {
    let skip = comment_lines(all);
    let bytes = &all[skip..];
    let first_line = bytes.split(|&c| c == b'\n').next().unwrap_or_default();
    let count = |d: u8| first_line.iter().filter(|&&c| c == d).count();
    let delimiter = b",;\t".iter().copied().max_by_key(|&d| (count(d), d == b',')).unwrap();

    let mut rec = Records::new(delimiter, bytes);
    if !rec.next() {
        return Err(Error::Descriptor("CSV: no header row".into()));
    }
    let mut seen = std::collections::HashMap::<String, usize>::new();
    let mut columns: Vec<Column> = (0..rec.nfields)
        .map(|i| {
            let raw = String::from_utf8_lossy(rec.field(i)).trim().to_owned();
            let raw = if raw.is_empty() { format!("column {}", i + 1) } else { raw };
            let (name, unit) = split_unit(&raw);
            let n = seen.entry(name.to_owned()).or_insert(0);
            *n += 1;
            let name = if *n > 1 { format!("{name} ({n})") } else { name.to_owned() };
            Column { source: name.clone(), name, unit, kind: Kind::Number, category: None }
        })
        .collect();
    let data_start = skip + rec.pos;

    // sample rows: per column, how many values are present, numeric, ISO times
    let mut stats = vec![(0usize, 0usize, 0usize); columns.len()];
    let mut rows = 0;
    while rows < SAMPLE_ROWS && rec.next() {
        if rec.is_blank() {
            continue;
        }
        rows += 1;
        for (i, s) in stats.iter_mut().enumerate() {
            let f = rec.field(i);
            if is_missing(f) {
                continue;
            }
            s.0 += 1;
            s.1 += usize::from(parse_number(f).is_some());
            s.2 += usize::from(std::str::from_utf8(f).ok().and_then(parse_iso).is_some());
        }
    }
    for (c, &(any, num, _)) in columns.iter_mut().zip(&stats) {
        c.kind = if num < any { Kind::Text } else { Kind::Number };
    }

    // a time column may have a few bad rows (they are skipped): at least 90% of its values must parse
    let mostly = |n: usize, any: usize| any > 0 && n * 10 >= any * 9;
    let time = columns
        .iter()
        .enumerate()
        .find_map(|(i, c)| {
            let unit_ns = time_unit(&c.name, c.unit.as_deref())?;
            let (any, num, iso) = stats[i];
            match () {
                _ if mostly(num, any) => Some(TimeColumn::Numeric { index: i, unit_ns }),
                _ if mostly(iso, any) => Some(TimeColumn::Iso { index: i }),
                _ => None,
            }
        })
        .or_else(|| stats.iter().position(|&(any, _, iso)| mostly(iso, any)).map(|index| TimeColumn::Iso { index }))
        .unwrap_or(TimeColumn::Index);
    let mut index_rate = None;
    let time = match profile {
        Some(p) => apply_profile(p, &mut columns, time, &mut index_rate)?,
        None => time,
    };
    if let TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } = time {
        columns[index].kind = Kind::Time;
    }
    // quotes in the sampled rows only: the rest of the file is checked chunk by chunk as it is parsed
    let quoted = memchr::memchr(b'"', &all[data_start..(skip + rec.pos).max(data_start)]).is_some();
    Ok(Schema { delimiter, data_start, columns, time, quoted, index_rate })
}

/// Applies a profile's column choices and decides the time: returns the time column to use.
fn apply_profile(
    p: &ImportProfile,
    columns: &mut [Column],
    detected: TimeColumn,
    index_rate: &mut Option<f64>,
) -> Result<TimeColumn> {
    p.validate().map_err(|e| Error::Descriptor(format!("import profile: {e}")))?;
    for c in columns.iter_mut() {
        let Some(ch) = p.column(&c.source) else { continue };
        if let Some(n) = ch.name.as_ref().filter(|n| !n.trim().is_empty()) {
            c.name = n.trim().to_owned();
        }
        if ch.unit.is_some() {
            c.unit = ch.unit.clone().filter(|u| !u.trim().is_empty());
        }
        c.category = ch.category.clone().filter(|u| !u.trim().is_empty());
        match ch.kind {
            Some(ColumnKind::Number) => c.kind = Kind::Number,
            Some(ColumnKind::Text) => c.kind = Kind::Text,
            Some(ColumnKind::Skip) => c.kind = Kind::Skip,
            None => {}
        }
    }
    // time: a choice of a column, or none, or the rate settings that drop the column
    let time = match &p.time {
        TimeChoice::Auto => detected,
        TimeChoice::None => TimeColumn::Index,
        TimeChoice::Column { name, unit } => {
            let index = columns
                .iter()
                .position(|c| c.source == *name)
                .ok_or_else(|| Error::Descriptor(format!("import profile: no time column named {name:?}")))?;
            let unit = unit.as_deref().map(str::to_ascii_lowercase);
            match unit.as_deref() {
                Some("iso") => TimeColumn::Iso { index },
                Some(u) => match time_unit_of(u) {
                    Some(unit_ns) => TimeColumn::Numeric { index, unit_ns },
                    None => return Err(Error::Descriptor(format!("import profile: unknown time unit {u:?}"))),
                },
                None => {
                    let unit_ns =
                        time_unit(&columns[index].name, columns[index].unit.as_deref()).unwrap_or(1_000_000_000);
                    TimeColumn::Numeric { index, unit_ns }
                }
            }
        }
    };
    let time = match p.rate {
        RateChoice::Assume { hz } => {
            *index_rate = Some(hz);
            TimeColumn::Index
        }
        RateChoice::Samples => TimeColumn::Index,
        _ => time,
    };
    // a detected time column that isn't the time any more is an ordinary column again
    for (i, c) in columns.iter_mut().enumerate() {
        let is_time = matches!(time, TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } if index == i);
        if is_time && c.kind != Kind::Skip {
            c.kind = Kind::Time;
        } else if c.kind == Kind::Time {
            c.kind = Kind::Number;
        }
    }
    Ok(time)
}

pub(crate) fn time_unit_of(u: &str) -> Option<i64> {
    match u {
        "s" | "sec" | "secs" | "seconds" => Some(1_000_000_000),
        "ms" => Some(1_000_000),
        "us" | "µs" => Some(1_000),
        "ns" => Some(1),
        _ => None,
    }
}

/// Splits the data rows into chunks of about `target` bytes at line ends (one chunk if fields are quoted).
pub fn split(bytes: &[u8], schema: &Schema, target: usize) -> Vec<Range<usize>> {
    let (start, end) = (schema.data_start, bytes.len());
    if schema.quoted || end - start <= target {
        return std::iter::once(start..end).collect();
    }
    let mut out = Vec::new();
    let mut a = start;
    while a < end {
        let probe = (a + target).min(end);
        let b = match memchr::memchr(b'\n', &bytes[probe..]) {
            Some(i) => probe + i + 1,
            None => end,
        };
        out.push(a..b);
        a = b;
    }
    out
}

/// Parses the rows in `bytes[range]`.
pub fn parse(bytes: &[u8], range: Range<usize>, schema: &Schema) -> Rows {
    let numeric: Vec<usize> = schema.numeric_columns().map(|(i, _)| i).collect();
    let est = (range.len() / 32).max(16);
    let mut out = Rows {
        t: Vec::with_capacity(if schema.time == TimeColumn::Index { 0 } else { est }),
        columns: numeric.iter().map(|_| Vec::with_capacity(est)).collect(),
        rows: 0,
        skipped: 0,
        quotes: false,
    };
    if schema.quoted {
        parse_rows(Records::new(schema.delimiter, &bytes[range]), schema, &numeric, &mut out);
    } else if memchr::memchr(b'"', &bytes[range.clone()]).is_some() {
        out.quotes = true;
    } else {
        parse_rows(Lines::new(schema.delimiter, &bytes[range]), schema, &numeric, &mut out);
    }
    out
}

fn parse_rows(mut rec: impl RowReader, schema: &Schema, numeric: &[usize], out: &mut Rows) {
    while rec.next() {
        if rec.is_blank() {
            continue;
        }
        let t = match schema.time {
            TimeColumn::Numeric { index, unit_ns } => {
                std::str::from_utf8(trim(rec.field(index))).ok().and_then(|s| parse_decimal(s, unit_ns))
            }
            TimeColumn::Iso { index } => std::str::from_utf8(rec.field(index)).ok().and_then(parse_iso),
            TimeColumn::Index => Some(0),
        };
        let Some(t) = t else {
            out.skipped += 1;
            continue;
        };
        if schema.time != TimeColumn::Index {
            out.t.push(t);
        }
        for (col, &i) in out.columns.iter_mut().zip(numeric) {
            let f = rec.field(i);
            col.push(parse_number(f).unwrap_or(f64::NAN));
        }
        out.rows += 1;
    }
}

/// Parses a whole CSV in memory (tests and fuzzing; the cache streams chunks instead).
pub fn parse_all(bytes: &[u8]) -> Result<(Schema, Rows)> {
    let mut schema = scan(bytes)?;
    let parts: Vec<Rows> = split(bytes, &schema, CHUNK_BYTES).into_iter().map(|r| parse(bytes, r, &schema)).collect();
    let parts = if parts.iter().any(|r| r.quotes) {
        schema.quoted = true;
        split(bytes, &schema, CHUNK_BYTES).into_iter().map(|r| parse(bytes, r, &schema)).collect()
    } else {
        parts
    };
    let mut all = Rows::default();
    for r in parts {
        if all.columns.is_empty() {
            all.columns = vec![Vec::new(); r.columns.len()];
        }
        all.t.extend(r.t);
        for (a, b) in all.columns.iter_mut().zip(r.columns) {
            a.extend(b);
        }
        all.rows += r.rows;
        all.skipped += r.skipped;
    }
    Ok((schema, all))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_lines_before_the_header_are_skipped() {
        let (schema, rows) = parse_all(b"# resampled to 2 Hz\n# second note\nt,v\n0,1\n0.5,2\n").unwrap();
        assert_eq!(schema.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["t", "v"]);
        assert_eq!(rows.rows, 2);
    }
    use proptest::prelude::Just;

    #[test]
    fn nasty_headers() {
        let csv = "time_s,Speed [m/s],Temp (°C),Motor.RPM,\"Note, quoted\",accel[0],Flight Mode,Speed [m/s],MixedCase  ,\n\
                   0.00,10.0,25.0,3000,N/A,0.0,IDLE,11.0,0,\n\
                   0.01,10.5,,3001,ok,0.05,IDLE,11.5,1,\n";
        let (s, rows) = parse_all(csv.as_bytes()).unwrap();
        let names: Vec<_> = s.columns.iter().map(|c| (c.name.as_str(), c.unit.as_deref(), c.kind)).collect();
        assert_eq!(
            names,
            [
                ("time_s", None, Kind::Time),
                ("Speed", Some("m/s"), Kind::Number),
                ("Temp", Some("°C"), Kind::Number),
                ("Motor.RPM", None, Kind::Number),
                ("Note, quoted", None, Kind::Text),
                ("accel[0]", None, Kind::Number),
                ("Flight Mode", None, Kind::Text),
                ("Speed (2)", Some("m/s"), Kind::Number),
                ("MixedCase", None, Kind::Number),
                ("column 10", None, Kind::Number),
            ]
        );
        assert_eq!(s.time, TimeColumn::Numeric { index: 0, unit_ns: 1_000_000_000 });
        assert!(!s.quoted); // the only quotes are in the header
        assert_eq!(rows.t, [0, 10_000_000]);
        assert_eq!(rows.columns[0], [10.0, 10.5]);
        assert!(rows.columns[1][1].is_nan());
    }

    #[test]
    fn units_and_delimiters() {
        let (s, rows) = parse_all(b"t_ms;value\n0;1,5\n20;2\n").unwrap();
        assert_eq!(s.delimiter, b';');
        assert_eq!(s.time, TimeColumn::Numeric { index: 0, unit_ns: 1_000_000 });
        assert_eq!(rows.t, [0, 20_000_000]);
        // decimal commas aren't read as numbers yet, so the column is text
        assert_eq!(s.columns[1].kind, Kind::Text);
        let (s, _) = parse_all(b"time [ms]\tv\n1\t2\n").unwrap();
        assert_eq!(s.delimiter, b'\t');
        assert_eq!(s.time, TimeColumn::Numeric { index: 0, unit_ns: 1_000_000 });
    }

    #[test]
    fn iso_dates_and_no_time() {
        let (s, rows) = parse_all(b"Date,Close\n2023-01-03,150.69\n2023-01-04,148.18\n").unwrap();
        assert_eq!(s.time, TimeColumn::Iso { index: 0 });
        assert_eq!(rows.t[1] - rows.t[0], 86_400_000_000_000);
        let (s, rows) = parse_all(b"alpha,beta\n0.0,1.0\n0.1,0.9\n\n").unwrap();
        assert_eq!(s.time, TimeColumn::Index);
        assert_eq!((rows.rows, rows.t.len()), (2, 0));
    }

    #[test]
    fn bad_times_are_skipped() {
        let mut csv = String::from("time,v\n");
        for i in 0..20 {
            csv.push_str(&format!("{i},{i}\n"));
        }
        csv.push_str("bad,2\n");
        let (_, rows) = parse_all(csv.as_bytes()).unwrap();
        assert_eq!((rows.t.len(), rows.skipped), (20, 1));
    }

    proptest::proptest! {
        /// The memchr fast path reads unquoted data exactly as the general CSV reader does.
        #[test]
        fn fast_path_matches_csv_core(
            rows in proptest::collection::vec(
                proptest::collection::vec(
                    proptest::prop_oneof![
                        "-?[0-9]{1,6}(\\.[0-9]{0,4})?", Just(String::new()), Just("N/A".to_string()),
                        Just(" 7 ".to_string()), Just("1e3".to_string()), Just("x".to_string()),
                    ],
                    0..5,
                ),
                0..40,
            ),
            crlf in proptest::bool::ANY,
        ) {
            let nl = if crlf { "\r\n" } else { "\n" };
            let mut csv = format!("time,a,b,c{nl}");
            for (i, r) in rows.iter().enumerate() {
                csv.push_str(&format!("{i}"));
                for f in r {
                    csv.push(',');
                    csv.push_str(f);
                }
                csv.push_str(nl);
                if i % 7 == 3 {
                    csv.push_str(nl); // blank line
                }
            }
            let b = csv.as_bytes();
            let mut schema = scan(b).unwrap();
            proptest::prop_assume!(!schema.quoted);
            let fast = parse(b, schema.data_start..b.len(), &schema);
            schema.quoted = true;
            let general = parse(b, schema.data_start..b.len(), &schema);
            proptest::prop_assert_eq!(fast.t, general.t);
            proptest::prop_assert_eq!(fast.rows, general.rows);
            for (x, y) in fast.columns.iter().zip(&general.columns) {
                proptest::prop_assert_eq!(x.len(), y.len());
                for (p, q) in x.iter().zip(y) {
                    proptest::prop_assert!(p == q || (p.is_nan() && q.is_nan()), "{} vs {}", p, q);
                }
            }
        }
    }

    #[test]
    fn quotes_past_the_sample_switch_to_the_general_reader() {
        let mut csv = String::from("time,v,note\n");
        for i in 0..20_000 {
            csv.push_str(&format!("{i},{i},x\n"));
        }
        csv.push_str("20000,1,\"two\nlines\"\n20001,2,y\n");
        let b = csv.as_bytes();
        assert!(!scan(b).unwrap().quoted);
        let (schema, rows) = parse_all(b).unwrap();
        assert!(schema.quoted);
        assert_eq!(rows.rows, 20_002);
        assert_eq!(*rows.t.last().unwrap(), 20_001_000_000_000);
    }

    #[test]
    fn chunked_equals_whole() {
        let mut csv = String::from("time,a,b\n");
        for i in 0..5000 {
            csv.push_str(&format!("{}.{:03},{},{}\n", i / 1000, i % 1000, i, -i));
        }
        let b = csv.as_bytes();
        let s = scan(b).unwrap();
        let mut t = Vec::new();
        for r in split(b, &s, 1000) {
            assert_eq!(b[r.end - 1], b'\n');
            t.extend(parse(b, r, &s).t);
        }
        assert_eq!(t, parse_all(b).unwrap().1.t);
        assert_eq!(t.len(), 5000);
    }
}

/// What the import dialog shows about a CSV file: its columns as detected, the first rows as text, and the time
/// and rate it would use.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Inspection {
    /// The key import profiles are found by (see [`crate::profile::header_key`]).
    pub header_key: String,
    pub delimiter: char,
    pub columns: Vec<InspectedColumn>,
    /// Index into `columns` of the detected time column.
    pub time: Option<usize>,
    /// `s`, `ms`, `us`, `ns` or `iso` for the detected time column.
    pub time_unit: Option<&'static str>,
    /// The first rows, as the file spells them (up to 8).
    pub preview: Vec<Vec<String>>,
    /// Rows in the file: counted when the whole file was read, else estimated from the size.
    pub rows: usize,
    pub rows_exact: bool,
    /// The recorded rate from the median step of the first times, when there is a time column.
    pub rate_hz: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct InspectedColumn {
    pub source: String,
    pub name: String,
    pub unit: Option<String>,
    /// `number`, `text` or `time`.
    pub kind: &'static str,
}

/// Inspects the start of a CSV file (`head`: its first bytes, cut at a line end; `file_len`: the whole file's size).
pub fn inspect(head: &[u8], file_len: u64) -> Result<Inspection> {
    let schema = scan(head)?;
    let mut rec = Records::new(schema.delimiter, &head[schema.data_start..]);
    let mut preview = Vec::new();
    let mut rows = 0usize;
    let mut times: Vec<i64> = Vec::new();
    while rec.next() {
        if rec.is_blank() {
            continue;
        }
        rows += 1;
        if preview.len() < 8 {
            preview.push(
                (0..schema.columns.len()).map(|i| String::from_utf8_lossy(rec.field(i)).trim().to_owned()).collect(),
            );
        }
        if times.len() < 1000 {
            let t = match schema.time {
                TimeColumn::Numeric { index, unit_ns } => {
                    std::str::from_utf8(trim(rec.field(index))).ok().and_then(|s| parse_decimal(s, unit_ns))
                }
                TimeColumn::Iso { index } => std::str::from_utf8(rec.field(index)).ok().and_then(parse_iso),
                TimeColumn::Index => None,
            };
            times.extend(t);
        }
    }
    let exact = head.len() as u64 >= file_len;
    if !exact && !head.is_empty() {
        rows = (rows as f64 * file_len as f64 / head.len() as f64) as usize;
    }
    let mut steps: Vec<i64> = times.windows(2).map(|w| w[1] - w[0]).filter(|&d| d > 0).collect();
    steps.sort_unstable();
    let rate_hz = steps.get(steps.len() / 2).map(|&d| 1e9 / d as f64);
    let (time, time_unit) = match schema.time {
        TimeColumn::Numeric { index, unit_ns } => (
            Some(index),
            Some(match unit_ns {
                1_000_000_000 => "s",
                1_000_000 => "ms",
                1_000 => "us",
                _ => "ns",
            }),
        ),
        TimeColumn::Iso { index } => (Some(index), Some("iso")),
        TimeColumn::Index => (None, None),
    };
    let names: Vec<String> = schema.columns.iter().map(|c| c.source.clone()).collect();
    Ok(Inspection {
        header_key: crate::profile::header_key(&names),
        delimiter: schema.delimiter as char,
        columns: schema
            .columns
            .iter()
            .map(|c| InspectedColumn {
                source: c.source.clone(),
                name: c.name.clone(),
                unit: c.unit.clone(),
                kind: match c.kind {
                    Kind::Number => "number",
                    Kind::Time => "time",
                    Kind::Text | Kind::Skip => "text",
                },
            })
            .collect(),
        time,
        time_unit,
        preview,
        rows,
        rows_exact: exact,
        rate_hz: if time.is_some() { rate_hz } else { None },
    })
}

/// Inspects a CSV file for the import dialog: reads its first 4 MB.
pub fn inspect_file(path: &Path) -> Result<Inspection> {
    use std::io::Read;
    const HEAD: usize = 4 << 20;
    let file = std::fs::File::open(path).map_err(crate::error::io_err(path))?;
    let len = file.metadata().map_err(crate::error::io_err(path))?.len();
    let mut head = Vec::new();
    file.take(HEAD as u64).read_to_end(&mut head).map_err(crate::error::io_err(path))?;
    if (head.len() as u64) < len
        && let Some(end) = memchr::memrchr(b'\n', &head)
    {
        head.truncate(end + 1);
    }
    inspect(&head, len)
}
