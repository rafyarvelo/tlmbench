//! Following a growing file: a CSV or a .tlm, recorded from its start and then as it grows.
//!
//! The file is only read. New bytes are read every 20 ms; only whole lines (CSV) or whole records (.tlm) are taken,
//! so a row the writer is half-way through waits for its end. A CSV's schema comes from its header and the rows there
//! when it is first read, by the same rules as opening it ([`csv::scan`]), and each row becomes a record: the time
//! in nanoseconds (an i64, unless rows are numbered) then an f64 per numeric column. Within one file, a row earlier
//! than the row before is left out and counted, as are rows whose time doesn't parse.
//!
//! When the file is truncated or replaced (log rotation), the rest of the old file is read, and the new one starts
//! the recording's next part, read from its top, its schema found again. A row cut short by the rotation is left out
//! and counted; so is anything written and truncated away between two reads (at most about 20 ms of it).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tlmbench_core::Timestamp;
use tlmbench_decode::csv::{self, Schema, TimeColumn};
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec, TimeUnit};
use tlmbench_decode::reference;

use crate::{Options, Ring};

const TICK: Duration = Duration::from_millis(20);
/// Read at most this much at a time, so records reach the ring while a large file is still being read.
const READ: usize = 8 << 20;

/// What identifies a file on disk: a replaced file has a new identity. Windows has no inode in `Metadata`, and a
/// file's creation time carries over to one renamed into its place (file tunneling), so there it is the volume serial
/// and file index of an open handle.
#[cfg(unix)]
fn identity(file: &File) -> io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}

#[cfg(windows)]
fn identity(file: &File) -> io::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
    // SAFETY: a plain-data struct the call fills in, for a handle that stays open for the call
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(info.dwVolumeSerialNumber), (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)))
}

/// The identity and length of the file now at `path` (None while there is none).
fn at_path(path: &Path) -> Option<((u64, u64), u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path).ok()?;
        Some(((m.dev(), m.ino()), m.len()))
    }
    #[cfg(windows)]
    {
        let f = File::open(path).ok()?;
        Some((identity(&f).ok()?, f.metadata().ok()?.len()))
    }
}

/// One file being followed.
struct Open {
    file: File,
    id: (u64, u64),
    /// Bytes read so far.
    offset: u64,
    parser: Parser,
}

pub(crate) fn follow(path: &Path, ring: &mut Ring, stop: &AtomicBool, opts: &Options) -> io::Result<()> {
    let mut cur: Option<Open> = None;
    let mut buf = vec![0u8; READ];
    let mut idle_since = Instant::now();
    while !stop.load(Ordering::Acquire) {
        let Some(open) = &mut cur else {
            match File::open(path) {
                Ok(file) => {
                    let id = identity(&file)?;
                    cur = Some(Open { file, id, offset: 0, parser: Parser::new(path) });
                    idle_since = Instant::now();
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if opts.idle_end.is_some_and(|d| idle_since.elapsed() >= d) {
                        return Err(io::Error::new(e.kind(), format!("{} doesn't exist", path.display())));
                    }
                    thread::sleep(TICK);
                }
                Err(e) => return Err(e),
            }
            continue;
        };
        let n = open.file.read(&mut buf)?;
        if n > 0 {
            open.offset += n as u64;
            ring.counters.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
            open.parser.feed(&buf[..n], ring);
            idle_since = Instant::now();
            continue;
        }
        // at the end of what was written: rotated (shorter, or another file now at the path), or waiting
        let now = at_path(path);
        let rotated = match now {
            None => false, // gone for a moment while being replaced: keep the old file until a new one appears
            Some((id, len)) => id != open.id || len < open.offset,
        };
        if rotated {
            open.parser.finish(ring, false);
            if now.is_some_and(|(id, _)| id == open.id) {
                open.file.seek(SeekFrom::Start(0))?; // truncated in place: read it again from the top
                (open.offset, open.parser) = (0, Parser::new(path));
            } else {
                cur = None;
            }
            continue;
        }
        open.parser.idle(ring);
        if opts.idle_end.is_some_and(|d| idle_since.elapsed() >= d) {
            // the file has been still for a while: a last row without its line end is whole
            open.parser.finish(ring, true);
            cur = None;
            break;
        }
        thread::sleep(TICK);
    }
    if let Some(open) = &mut cur {
        open.parser.finish(ring, false);
    }
    Ok(())
}

/// Turns a file's bytes into records, part by part.
enum Parser {
    Tlm(TlmParser),
    Csv(CsvParser),
    /// Not enough bytes yet to tell which.
    Unknown(Vec<u8>),
}

impl Parser {
    fn new(path: &Path) -> Parser {
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some(reference::EXTENSION) => Parser::Tlm(TlmParser::default()),
            Some("csv" | "tsv" | "txt") => Parser::Csv(CsvParser::default()),
            _ => Parser::Unknown(Vec::new()),
        }
    }

    fn feed(&mut self, bytes: &[u8], ring: &mut Ring) {
        if let Parser::Unknown(head) = self {
            head.extend_from_slice(bytes);
            if head.len() < reference::MAGIC.len() {
                return;
            }
            let head = std::mem::take(head);
            *self = if head.starts_with(reference::MAGIC) {
                Parser::Tlm(TlmParser::default())
            } else {
                Parser::Csv(CsvParser::default())
            };
            return self.feed(&head, ring);
        }
        match self {
            Parser::Tlm(p) => p.feed(bytes, ring),
            Parser::Csv(p) => p.feed(bytes, ring, false),
            Parser::Unknown(_) => unreachable!(),
        }
    }

    /// Nothing new for a tick: a CSV that has only its header and a few rows decides its schema from them.
    fn idle(&mut self, ring: &mut Ring) {
        if let Parser::Csv(p) = self {
            p.feed(&[], ring, true);
        }
    }

    /// The file is done: rotated or stopped (`still` false), or still for the idle time. A last row without its line
    /// end counts only when the file is still: otherwise it may be cut short, and is left out and counted.
    fn finish(&mut self, ring: &mut Ring, still: bool) {
        match self {
            Parser::Csv(p) => {
                if still && !p.pending.is_empty() && !p.pending.ends_with(b"\n") {
                    p.pending.push(b'\n');
                }
                p.feed(&[], ring, true);
                ring.counters.malformed.fetch_add(p.pending.len() as u64, Ordering::Relaxed);
            }
            Parser::Tlm(p) => {
                ring.counters.malformed.fetch_add(p.pending.len() as u64, Ordering::Relaxed);
            }
            Parser::Unknown(head) => {
                ring.counters.malformed.fetch_add(head.len() as u64, Ordering::Relaxed);
            }
        }
    }
}

/// A growing .tlm: its header, then whole records as they are appended.
#[derive(Default)]
struct TlmParser {
    pending: Vec<u8>,
    /// Record size, once the header is read.
    rs: Option<usize>,
}

impl TlmParser {
    fn feed(&mut self, bytes: &[u8], ring: &mut Ring) {
        self.pending.extend_from_slice(bytes);
        if self.rs.is_none() {
            if self.pending.len() < 16 {
                return;
            }
            let len = u32::from_le_bytes(self.pending[12..16].try_into().unwrap()) as usize;
            if self.pending.len() < len {
                return;
            }
            match reference::read_header(&self.pending) {
                Ok((start, d)) => {
                    self.rs = Some(d.record_size as usize);
                    ring.part(d);
                    self.pending.drain(..start);
                }
                Err(e) => {
                    ring.counters.fail(e);
                    return;
                }
            }
        }
        let rs = self.rs.expect("set above");
        let whole = self.pending.len() / rs * rs;
        ring.push(&self.pending[..whole]);
        self.pending.drain(..whole);
    }
}

/// A growing CSV: its schema once the header and some rows are there, then each whole line as a record.
#[derive(Default)]
struct CsvParser {
    pending: Vec<u8>,
    schema: Option<Schema>,
    /// Whether rows have a time (else they are numbered) and how many numeric columns follow it.
    timed: bool,
    numeric: usize,
    /// The last recorded row's time: rows before it are left out.
    last: Option<Timestamp>,
    records: Vec<u8>,
}

impl CsvParser {
    /// Takes `bytes`; with `settle`, decides the schema from whatever whole rows there are (at least the header).
    fn feed(&mut self, bytes: &[u8], ring: &mut Ring, settle: bool) {
        self.pending.extend_from_slice(bytes);
        let end = self.complete();
        if self.schema.is_none() {
            let lines = memchr::memchr_iter(b'\n', &self.pending[..end]).count();
            // the header and enough rows to tell numbers from text, or whatever is there once the file pauses
            if lines < 2 || (lines < 1000 && !settle) {
                return;
            }
            let schema = match csv::scan(&self.pending[..end]) {
                Ok(s) => s,
                Err(e) => return ring.counters.fail(e),
            };
            let d = self.descriptor(&schema);
            self.timed = schema.time != TimeColumn::Index;
            self.numeric = schema.numeric_columns().count();
            self.pending.drain(..schema.data_start);
            self.schema = Some(schema);
            ring.part(d);
            return self.feed(&[], ring, settle);
        }
        if end == 0 {
            return;
        }
        let schema = self.schema.as_mut().expect("set above");
        let mut rows = csv::parse(&self.pending[..end], 0..end, schema);
        if rows.quotes {
            schema.quoted = true;
            let end = self.complete();
            rows = csv::parse(&self.pending[..end], 0..end, self.schema.as_ref().unwrap());
        }
        let end = self.complete();
        self.records.clear();
        let mut back = 0;
        for r in 0..rows.rows {
            if self.timed {
                let t = rows.t[r];
                if self.last.is_some_and(|l| t < l) {
                    back += 1;
                    continue;
                }
                self.last = Some(t);
                self.records.extend_from_slice(&t.to_le_bytes());
            }
            for c in &rows.columns {
                self.records.extend_from_slice(&c[r].to_le_bytes());
            }
        }
        let c = ring.counters;
        c.rows_skipped.fetch_add(rows.skipped as u64, Ordering::Relaxed);
        c.rows_back.fetch_add(back, Ordering::Relaxed);
        if !self.records.is_empty() {
            ring.push(&self.records);
        }
        self.pending.drain(..end);
    }

    /// Bytes up to the end of the last whole line (outside quotes, when fields are quoted).
    fn complete(&self) -> usize {
        let p = &self.pending;
        if !self.schema.as_ref().is_some_and(|s| s.quoted) {
            return memchr::memrchr(b'\n', p).map_or(0, |i| i + 1);
        }
        let (mut quotes, mut end) = (0usize, 0);
        for i in memchr::memchr2_iter(b'"', b'\n', p) {
            match p[i] {
                b'"' => quotes += 1,
                _ if quotes.is_multiple_of(2) => end = i + 1,
                _ => {}
            }
        }
        end
    }

    /// The recording's layout: the time in nanoseconds (unless rows are numbered), then each numeric column.
    fn descriptor(&self, s: &Schema) -> Descriptor {
        let mut fields = Vec::new();
        let time = match s.time {
            TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } => {
                let name = s.columns[index].name.clone();
                fields.push(Field {
                    name: name.clone(),
                    dtype: DType::I64,
                    offset: 0,
                    unit: None,
                    scale: 1.0,
                    value_offset: 0.0,
                    category: None,
                    bits: None,
                    plugin: None,
                    size: 0,
                    params: None,
                });
                TimeSpec::Field { field: name, unit: TimeUnit::Ns, t0_ns: 0 }
            }
            TimeColumn::Index => TimeSpec::Index,
        };
        for (_, c) in s.numeric_columns() {
            let offset = 8 * fields.len() as u64;
            fields.push(Field {
                name: c.name.clone(),
                dtype: DType::F64,
                offset,
                unit: c.unit.clone(),
                scale: 1.0,
                value_offset: 0.0,
                category: None,
                bits: None,
                plugin: None,
                size: 0,
                params: None,
            });
        }
        Descriptor {
            sequence: None,
            files: Vec::new(),
            packet_types: Vec::new(),
            events: Vec::new(),
            packets: None,
            schema: SCHEMA,
            name: Some("tailed CSV".into()),
            endian: Endian::Little,
            header_size: 0,
            record_size: 8 * fields.len().max(1) as u64,
            time,
            fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::{Endpoint, Health, Source};

    fn opts() -> Options {
        Options { idle_end: Some(Duration::from_millis(400)), ..Options::default() }
    }

    /// Each recorded part's records, read back: (descriptor, rows of f64 with the time first when there is one).
    fn parts(h: &Health) -> Vec<(Descriptor, Vec<Vec<f64>>)> {
        h.files
            .iter()
            .map(|f| {
                let bytes = std::fs::read(f).unwrap();
                let (start, d) = reference::read_header(&bytes).unwrap();
                let rows = bytes[start..]
                    .chunks_exact(d.record_size as usize)
                    .map(|r| {
                        d.fields
                            .iter()
                            .map(|f| {
                                let b: [u8; 8] = r[f.offset as usize..][..8].try_into().unwrap();
                                match f.dtype {
                                    DType::I64 => i64::from_le_bytes(b) as f64,
                                    _ => f64::from_le_bytes(b),
                                }
                            })
                            .collect()
                    })
                    .collect();
                (d, rows)
            })
            .collect()
    }

    fn csv_rows(from: usize, to: usize, crlf: bool) -> String {
        let end = if crlf { "\r\n" } else { "\n" };
        (from..to).map(|i| format!("{}.{:03},{},{:.2},ok{end}", i / 1000, i % 1000, i * 2, i as f64 * 0.25)).collect()
    }

    fn write(path: &Path, text: &[u8]) {
        std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap().write_all(text).unwrap();
    }

    #[test]
    fn a_growing_csv_is_recorded_row_for_row() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("run.csv");
        // some rows before the tail starts (recorded too), the rest appended in chunks that cut lines and CRLFs
        let head = format!("time_s,count,volts [V],note\r\n{}", csv_rows(0, 1500, true));
        write(&csv, head.as_bytes());
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("source-1"), &opts()).unwrap();
        let rest = csv_rows(1500, 5000, true);
        for chunk in rest.as_bytes().chunks(997) {
            write(&csv, chunk);
            thread::sleep(Duration::from_millis(3));
        }
        let h = src.wait();
        assert_eq!(
            (h.records_written, h.rows_skipped, h.rows_back_in_time, h.malformed_bytes, &h.error),
            (5000, 0, 0, 0, &None),
            "{h:?}"
        );
        let got = parts(&h);
        let (d, rows) = &got[0];
        assert_eq!(
            d.fields.iter().map(|f| (f.name.as_str(), f.unit.as_deref())).collect::<Vec<_>>(),
            [("time_s", None), ("count", None), ("volts", Some("V"))]
        );
        assert_eq!(d.time, TimeSpec::Field { field: "time_s".into(), unit: TimeUnit::Ns, t0_ns: 0 });
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r, &[i as f64 * 1e6, (i * 2) as f64, (i as f64 * 0.25 * 100.0).round() / 100.0], "row {i}");
        }
    }

    #[test]
    fn waits_for_the_file_and_its_header() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("later.csv");
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        thread::sleep(Duration::from_millis(100));
        write(&csv, b""); // exists, empty
        thread::sleep(Duration::from_millis(100));
        write(&csv, b"time_s,count,volts [V],no");
        thread::sleep(Duration::from_millis(100));
        write(&csv, format!("te\n{}", csv_rows(0, 10, false)).as_bytes());
        let h = src.wait();
        assert_eq!(h.records_written, 10, "{h:?}");
        assert_eq!(parts(&h)[0].1[9][1], 18.0);
    }

    #[test]
    fn rotation_starts_the_next_part() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("log.csv");
        write(&csv, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 100, false)).as_bytes());
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("source-1"), &opts()).unwrap();
        thread::sleep(Duration::from_millis(150));
        // truncated in place, times starting over
        std::fs::write(&csv, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 30, false))).unwrap();
        thread::sleep(Duration::from_millis(150));
        // then replaced by another file (rename), with an extra column
        let next = dir.path().join("log.csv.new");
        std::fs::write(&next, "time_s,count,volts [V],note,amps\n1.000,1,1.5,x,0.5\n2.000,2,2.5,y,0.25\n").unwrap();
        std::fs::rename(&next, &csv).unwrap();
        let h = src.wait();
        let got = parts(&h);
        let names: Vec<_> = h.files.iter().map(|f| f.file_name().unwrap().to_str().unwrap().to_owned()).collect();
        assert_eq!(names, ["source-1.tlm", "source-1-2.tlm", "source-1-3.tlm"], "{h:?}");
        assert_eq!(got.iter().map(|p| p.1.len()).collect::<Vec<_>>(), [100, 30, 2]);
        assert_eq!(got[1].1[0][0], 0.0, "times as written: the second part starts again at 0");
        assert_eq!(got[2].0.fields.len(), 4);
        assert_eq!(got[2].1[1], [2e9, 2.0, 2.5, 0.25]);
    }

    #[test]
    fn rows_going_back_in_time_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("jumpy.csv");
        // 20 rows at 1..=20 s, with one going back (after row 10) and one whose time doesn't parse
        let mut text = String::from("t,v\n");
        for i in 1..=20 {
            text += &format!("{i},{}\n", i * 10);
            if i == 10 {
                text += "9.5,95\nbad,0\n";
            }
        }
        write(&csv, text.as_bytes());
        let h = Source::start(Endpoint::File(csv), None, &dir.path().join("s"), &opts()).unwrap().wait();
        assert_eq!(
            (h.records_written, h.rows_back_in_time, h.rows_skipped, h.bytes_in),
            (20, 1, 1, text.len() as u64),
            "{h:?}"
        );
        assert_eq!(
            parts(&h)[0].1.iter().map(|r| r[1]).collect::<Vec<_>>(),
            (1..=20).map(|i| i as f64 * 10.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_growing_tlm_is_recorded_record_for_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grow.tlm");
        let d = crate::tests_support::desc();
        let mut head = Vec::new();
        reference::write_header(&mut head, &d).unwrap();
        let body: Vec<u8> = (0u32..5000).flat_map(|s| [s.to_le_bytes(), (s as f32).to_le_bytes()].concat()).collect();
        write(&path, &head[..10]); // even the header arrives in pieces
        let src = Source::start(Endpoint::File(path.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        thread::sleep(Duration::from_millis(60));
        write(&path, &head[10..]);
        for chunk in body.chunks(333) {
            write(&path, chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let h = src.wait();
        assert_eq!((h.records_written, h.sequence_missing, h.malformed_bytes), (5000, 0, 0), "{h:?}");
        let bytes = std::fs::read(&h.files[0]).unwrap();
        let (start, got) = reference::read_header(&bytes).unwrap();
        assert_eq!((got, &bytes[start..]), (d, &body[..]));
    }

    /// A growing file whose name doesn't say its format is read as `.tlm` by its magic, even when the first read is
    /// shorter than the magic, and as CSV otherwise (coverage, Nov 24: no test reached the sniffing).
    #[test]
    fn a_growing_file_without_a_known_extension_is_sniffed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grow.dat");
        let d = crate::tests_support::desc();
        let mut head = Vec::new();
        reference::write_header(&mut head, &d).unwrap();
        let body: Vec<u8> = (0u32..2000).flat_map(|s| [s.to_le_bytes(), (s as f32).to_le_bytes()].concat()).collect();
        write(&path, &head[..3]); // fewer bytes than the magic: undecided until more come
        let src = Source::start(Endpoint::File(path.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        thread::sleep(Duration::from_millis(60));
        write(&path, &head[3..]);
        for chunk in body.chunks(500) {
            write(&path, chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let h = src.wait();
        assert_eq!((h.records_written, h.malformed_bytes, &h.error), (2000, 0, &None), "{h:?}");
        let bytes = std::fs::read(&h.files[0]).unwrap();
        let (start, got) = reference::read_header(&bytes).unwrap();
        assert_eq!((got, &bytes[start..]), (d, &body[..]));

        let log = dir.path().join("grow.log");
        write(&log, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 300, false)).as_bytes());
        let src = Source::start(Endpoint::File(log.clone()), None, &dir.path().join("t"), &opts()).unwrap();
        write(&log, csv_rows(300, 600, false).as_bytes());
        let h = src.wait();
        assert_eq!((h.records_written, h.malformed_bytes, &h.error), (600, 0, &None), "read as CSV: {h:?}");
    }

    /// A quoted CSV field may hold a line end: a row is whole only when its quotes are, however the reads cut it
    /// (coverage, Nov 24).
    #[test]
    fn a_quoted_field_with_a_line_end_waits_for_its_closing_quote() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("notes.csv");
        let rows: String =
            (0..400).map(|i| format!("{}.{:03},{},\"note {i},\nsecond line\"\n", i / 1000, i % 1000, i * 3)).collect();
        write(&csv, b"time_s,count,note\n");
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        // chunks of 7 bytes cut rows inside their quotes, often right after the quoted line end
        for chunk in rows.as_bytes().chunks(7) {
            write(&csv, chunk);
        }
        let h = src.wait();
        assert_eq!((h.records_written, h.rows_skipped, h.malformed_bytes, &h.error), (400, 0, 0, &None), "{h:?}");
        let got = parts(&h);
        let (d, recs) = &got[0];
        assert_eq!(d.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["time_s", "count"]);
        for (i, r) in recs.iter().enumerate() {
            assert_eq!(r, &[i as f64 * 1e6, (i * 3) as f64], "row {i}");
        }
    }

    /// A file that never appears ends the source, once `idle_end` has passed, with the reason.
    #[test]
    fn a_file_that_never_appears_ends_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let h = Source::start(Endpoint::File(dir.path().join("never.csv")), None, &dir.path().join("s"), &opts())
            .unwrap()
            .wait();
        assert!(h.ended && h.records_written == 0, "{h:?}");
        assert!(h.error.as_deref().is_some_and(|e| e.contains("never.csv doesn't exist")), "{h:?}");
    }

    /// A path that can't be read ends the source with the system's reason: a folder named like a CSV is opened, then
    /// fails to read; a file nobody may read fails to open.
    #[cfg(unix)]
    #[test]
    fn a_path_that_cannot_be_read_ends_the_source_with_the_reason() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder.csv");
        std::fs::create_dir(&folder).unwrap();
        let h = Source::start(Endpoint::File(folder), None, &dir.path().join("a"), &opts()).unwrap().wait();
        assert!(h.ended && h.error.is_some() && h.records_written == 0, "{h:?}");

        let locked = dir.path().join("locked.csv");
        std::fs::write(&locked, "t,v\n1,2\n").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        if File::open(&locked).is_ok() {
            return; // running as root: nothing is locked
        }
        let h = Source::start(Endpoint::File(locked), None, &dir.path().join("b"), &opts()).unwrap().wait();
        assert!(h.ended && h.error.as_deref().is_some_and(|e| e.contains("ermission")), "{h:?}");
    }

    /// Stopping a source that follows a file records what it read, with no error.
    #[test]
    fn a_followed_file_can_be_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("run.csv");
        write(&csv, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 50, false)).as_bytes());
        let src = Source::start(Endpoint::File(csv), None, &dir.path().join("s"), &Options::default()).unwrap();
        let t = Instant::now();
        while src.records_written() < 50 {
            assert!(t.elapsed() < Duration::from_secs(10), "{:?}", src.health());
            thread::sleep(Duration::from_millis(5));
        }
        let h = src.stop();
        assert!(h.ended && h.error.is_none(), "{h:?}");
        assert_eq!((h.records_written, h.malformed_bytes), (50, 0));
    }

    /// The last row has no line end: once the file has been still for `idle_end` it is whole.
    #[test]
    fn a_last_row_without_its_line_end_counts_once_the_file_is_still() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("cut.csv");
        write(&csv, b"t,v\n1,10\n2,20\n3,30");
        let h = Source::start(Endpoint::File(csv), None, &dir.path().join("s"), &opts()).unwrap().wait();
        assert_eq!((h.records_written, h.malformed_bytes, &h.error), (3, 0, &None), "{h:?}");
        assert_eq!(parts(&h)[0].1[2], [3e9, 30.0]);
    }

    /// A file that is neither named like a CSV or a .tlm, nor long enough to tell which, leaves its bytes counted as
    /// malformed.
    #[test]
    fn a_short_file_of_unknown_kind_is_counted_as_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.dat");
        write(&path, b"abc");
        let h = Source::start(Endpoint::File(path), None, &dir.path().join("s"), &opts()).unwrap().wait();
        assert_eq!((h.records_written, h.malformed_bytes, h.bytes_in), (0, 3, 3), "{h:?}");
        assert!(h.error.is_none() && h.files.is_empty(), "{h:?}");
    }

    /// A .tlm's header may arrive after the 16 bytes that say how long it is.
    #[test]
    fn a_tlm_header_that_arrives_in_two_pieces_is_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("late.tlm");
        let d = crate::tests_support::desc();
        let mut head = Vec::new();
        reference::write_header(&mut head, &d).unwrap();
        assert!(head.len() > 20);
        write(&path, &head[..20]);
        let src = Source::start(Endpoint::File(path.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(src.records_written(), 0);
        write(&path, &head[20..]);
        let body: Vec<u8> = (0u32..100).flat_map(|s| [s.to_le_bytes(), (s as f32).to_le_bytes()].concat()).collect();
        write(&path, &body);
        let h = src.wait();
        assert_eq!((h.records_written, h.malformed_bytes, &h.error), (100, 0, &None), "{h:?}");
    }

    /// A .tlm whose header can't be read is the source's error, and nothing is recorded.
    #[test]
    fn a_tlm_with_a_bad_header_is_the_sources_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.tlm");
        let mut bytes = vec![b'x'; 12];
        bytes.extend_from_slice(&16u32.to_le_bytes());
        write(&path, &bytes);
        let h = Source::start(Endpoint::File(path), None, &dir.path().join("s"), &opts()).unwrap().wait();
        assert!(h.ended && h.error.is_some() && h.records_written == 0, "{h:?}");
    }

    /// A CSV with nothing in it to take a header from is the source's error.
    #[test]
    fn a_csv_without_a_header_is_the_sources_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.csv");
        write(&path, b"\n\n\n");
        let h = Source::start(Endpoint::File(path), None, &dir.path().join("s"), &opts()).unwrap().wait();
        assert!(h.ended && h.error.as_deref().is_some_and(|e| e.contains("no header")), "{h:?}");
        assert_eq!(h.records_written, 0);
    }

    /// Quotes that first appear after the schema was settled: the rows from there on are read as quoted, and a line
    /// end inside the quotes doesn't cut a row.
    #[test]
    fn quotes_that_appear_after_the_schema_is_settled_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("late-quotes.csv");
        write(&csv, b"time_s,count,note\n0.000,0,a\n0.001,1,b\n0.002,2,c\n");
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        let t = Instant::now();
        while src.records_written() < 3 {
            assert!(t.elapsed() < Duration::from_secs(10), "{:?}", src.health());
            thread::sleep(Duration::from_millis(5));
        }
        write(&csv, b"0.003,3,\"x, y\"\n0.004,4,\"two\nlines\"\n0.005,5,plain\n");
        let h = src.wait();
        assert_eq!((h.records_written, h.rows_skipped, h.malformed_bytes, &h.error), (6, 0, 0, &None), "{h:?}");
        let got = parts(&h);
        assert_eq!(got[0].1.iter().map(|r| r[1]).collect::<Vec<_>>(), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(got[0].1[5][0], 5e6);
    }

    /// An ISO time column is recorded as nanoseconds; rows with no time column are numbered.
    #[test]
    fn iso_times_are_nanoseconds_and_a_file_without_time_is_numbered() {
        let dir = tempfile::tempdir().unwrap();
        let iso = dir.path().join("iso.csv");
        write(&iso, b"when,v\n2024-01-01T00:00:00.000Z,1\n2024-01-01T00:00:00.500Z,2\n2024-01-01T00:00:01.000Z,3\n");
        let h = Source::start(Endpoint::File(iso), None, &dir.path().join("a"), &opts()).unwrap().wait();
        assert_eq!((h.records_written, &h.error), (3, &None), "{h:?}");
        let got = parts(&h);
        assert_eq!(got[0].0.time, TimeSpec::Field { field: "when".into(), unit: TimeUnit::Ns, t0_ns: 0 });
        assert_eq!(got[0].1, [[1_704_067_200e9, 1.0], [1_704_067_200.5e9, 2.0], [1_704_067_201e9, 3.0]]);

        let plain = dir.path().join("plain.csv");
        write(&plain, b"a,b\n5,2\n3,9\n8,1\n4,7\n");
        let h = Source::start(Endpoint::File(plain), None, &dir.path().join("b"), &opts()).unwrap().wait();
        assert_eq!((h.records_written, &h.error), (4, &None), "{h:?}");
        let got = parts(&h);
        assert_eq!(got[0].0.time, TimeSpec::Index);
        assert_eq!(got[0].1, [[5.0, 2.0], [3.0, 9.0], [8.0, 1.0], [4.0, 7.0]]);
    }

    /// A file that is moved away leaves the source waiting, not ending: when another takes its place, that is the
    /// next part.
    #[test]
    fn a_file_that_is_gone_for_a_moment_is_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("log.csv");
        write(&csv, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 50, false)).as_bytes());
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        let t = Instant::now();
        while src.records_written() < 50 {
            assert!(t.elapsed() < Duration::from_secs(10), "{:?}", src.health());
            thread::sleep(Duration::from_millis(5));
        }
        // moved, not deleted: the old file keeps its identity, so the new one can't share it
        std::fs::rename(&csv, dir.path().join("log.csv.1")).unwrap();
        thread::sleep(Duration::from_millis(120));
        assert!(!src.health().ended, "a file that is gone doesn't end the source");
        write(&csv, format!("time_s,count,volts [V],note\n{}", csv_rows(0, 20, false)).as_bytes());
        let h = src.wait();
        assert_eq!(h.files.len(), 2, "{h:?}");
        assert_eq!(parts(&h).iter().map(|p| p.1.len()).collect::<Vec<_>>(), [50, 20]);
    }

    /// Rows that are all left out (a time that doesn't read, a time that goes back) add no records.
    #[test]
    fn rows_that_are_all_left_out_add_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("skipped.csv");
        write(&csv, b"t,v\n1,10\n2,20\n3,30\n");
        let src = Source::start(Endpoint::File(csv.clone()), None, &dir.path().join("s"), &opts()).unwrap();
        let t = Instant::now();
        while src.records_written() < 3 {
            assert!(t.elapsed() < Duration::from_secs(10), "{:?}", src.health());
            thread::sleep(Duration::from_millis(5));
        }
        write(&csv, b"oops,9\n");
        thread::sleep(Duration::from_millis(100));
        write(&csv, b"1.5,9\n");
        let h = src.wait();
        assert_eq!((h.records_written, h.rows_skipped, h.rows_back_in_time, &h.error), (3, 1, 1, &None), "{h:?}");
    }
}
