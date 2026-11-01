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
    /// The data has a quote character somewhere, so it can't be split at arbitrary newlines.
    pub quoted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub unit: Option<String>,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Number,
    Text,
    Time,
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
}

/// Cheap check for a text file worth parsing as CSV: the extension, or printable text with a delimiter.
pub fn looks_like_csv(path: &Path, head: &[u8]) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if matches!(ext.as_deref(), Some("csv" | "tsv")) {
        return true;
    }
    let line = head.split(|&c| c == b'\n').next().unwrap_or_default();
    !head.contains(&0) && line.iter().any(|c| matches!(c, b',' | b';' | b'\t'))
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

    fn field(&self, i: usize) -> &[u8] {
        if i >= self.nfields {
            return &[];
        }
        let start = if i == 0 { 0 } else { self.ends[i - 1] };
        &self.out[start..self.ends[i]]
    }

    fn is_blank(&self) -> bool {
        (0..self.nfields).all(|i| self.field(i).iter().all(u8::is_ascii_whitespace))
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
fn time_unit(name: &str, unit: Option<&str>) -> Option<i64> {
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
fn split_unit(name: &str) -> (&str, Option<String>) {
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

/// Reads the header and the first rows to decide the schema.
pub fn scan(bytes: &[u8]) -> Result<Schema> {
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
            Column { name, unit, kind: Kind::Number }
        })
        .collect();
    let data_start = rec.pos;

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
    if let TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } = time {
        columns[index].kind = Kind::Time;
    }
    let quoted = memchr::memchr(b'"', &bytes[data_start..]).is_some();
    Ok(Schema { delimiter, data_start, columns, time, quoted })
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
    };
    let mut rec = Records::new(schema.delimiter, &bytes[range]);
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
        for (col, &i) in out.columns.iter_mut().zip(&numeric) {
            let f = rec.field(i);
            col.push(if is_missing(f) { f64::NAN } else { parse_number(f).unwrap_or(f64::NAN) });
        }
        out.rows += 1;
    }
    out
}

/// Parses a whole CSV in memory (tests and fuzzing; the cache streams chunks instead).
pub fn parse_all(bytes: &[u8]) -> Result<(Schema, Rows)> {
    let schema = scan(bytes)?;
    let mut all = Rows::default();
    for range in split(bytes, &schema, CHUNK_BYTES) {
        let r = parse(bytes, range, &schema);
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
