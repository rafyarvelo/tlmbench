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
                    bits: None,
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
                bits: None,
            });
        }
        Descriptor {
            sequence: None,
            files: Vec::new(),
            packet_types: Vec::new(),
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
}
