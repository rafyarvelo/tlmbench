//! The columnar cache for formats that can't be read in place (CSV now; compressed formats later).
//!
//! The first open parses the source in parallel into one file per column under the cache root: `t.i64` (sample
//! times, absent with index time) and `c<k>.f64`, plain little-endian arrays, plus `manifest.json`. Later opens
//! memory-map those files, so reopening costs the fingerprint only. The cache lives in the app cache dir, never
//! beside the source, and can be deleted at any time: it is rebuilt on the next open.
//!
//! The key is a fingerprint of the source: its size and modification time, and a BLAKE3 hash of its first and
//! last MiB and 64 blocks spread through it. Hashing whole multi-GB files would cost about a second on each open.
//!
//! The source is read with positioned reads, each worker reading its own 8 MB chunk. Parallel page faults on one
//! memory map serialize on macOS (about 330 MB/s on an M1 for a 2.4 GB CSV); separate reads run at disk speed.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tlmbench_core::{ChannelInfo, RateTimes, Recording, RecordingInfo, TimeSource, Trace, Viewport, m4};

use crate::bytes::{self, Bytes};
use crate::csv::{self, TimeColumn};
use crate::error::{Result, format_err, io_err};

const MANIFEST_VERSION: u32 = 1;

/// The cache root: `override`, else `$TLMBENCH_CACHE`, else the OS cache dir's `tlmbench` folder (the desktop app
/// and the CLI share it).
pub fn cache_root(over: Option<&Path>) -> PathBuf {
    if let Some(p) = over {
        return p.to_owned();
    }
    if let Some(p) = std::env::var_os("TLMBENCH_CACHE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("tlmbench")
}

/// Bytes read for the schema: the header and the sample rows.
const HEAD_BYTES: usize = 16 << 20;

/// The source file, read with positioned reads.
#[derive(Debug)]
struct Source {
    path: PathBuf,
    file: File,
    len: u64,
    mtime_ns: u128,
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(io_err(path))?;
        let meta = file.metadata().map_err(io_err(path))?;
        let mtime_ns = meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        Ok(Self { path: path.to_owned(), file, len: meta.len(), mtime_ns })
    }

    /// Up to `len` bytes from `at` (fewer at the end of the file).
    fn read(&self, at: u64, len: usize) -> Result<Vec<u8>> {
        let len = len.min(self.len.saturating_sub(at) as usize);
        let mut buf = vec![0; len];
        read_exact_at(&self.file, &mut buf, at).map_err(io_err(&self.path))?;
        Ok(buf)
    }

    fn fingerprint(&self) -> Result<String> {
        const EDGE: usize = 1 << 20;
        const BLOCK: usize = 4096;
        let mut h = blake3::Hasher::new();
        h.update(&self.len.to_le_bytes());
        h.update(&self.mtime_ns.to_le_bytes());
        h.update(&self.read(0, EDGE)?);
        h.update(&self.read(self.len.saturating_sub(EDGE as u64), EDGE)?);
        for k in 1..64 {
            h.update(&self.read(self.len / 64 * k, BLOCK)?);
        }
        Ok(h.finalize().to_hex()[..32].to_owned())
    }

    /// Ranges of about `target` bytes from `start` to the end, each ending at a line end.
    fn split(&self, start: u64, target: u64) -> Result<Vec<Range<u64>>> {
        const WINDOW: usize = 64 << 10;
        let mut out = Vec::new();
        let mut a = start;
        while a < self.len {
            let mut probe = a + target;
            let b = loop {
                if probe >= self.len {
                    break self.len;
                }
                let w = self.read(probe, WINDOW)?;
                match memchr::memchr(b'\n', &w) {
                    Some(i) => break probe + i as u64 + 1,
                    None => probe += w.len() as u64,
                }
            };
            out.push(a..b);
            a = b;
        }
        Ok(out)
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, at)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, at) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                at += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    source_size: u64,
    rows: usize,
    skipped_rows: usize,
    /// None: index time (row numbers).
    time: Option<String>,
    channels: Vec<ManifestChannel>,
    skipped_columns: Vec<String>,
    t_first: Option<i64>,
    t_last: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestChannel {
    name: String,
    unit: Option<String>,
    file: String,
}

/// A CSV file read through its cache.
#[derive(Debug)]
pub struct CsvRecording {
    info: RecordingInfo,
    rows: usize,
    t: Option<Bytes>,
    columns: Vec<Bytes>,
    /// Rows left out because their time didn't parse.
    pub skipped_rows: usize,
}

/// Opens a CSV through the cache under `root`, building the cache entry first if needed.
pub fn open_csv(path: &Path, root: &Path) -> Result<CsvRecording> {
    let source = Source::open(path)?;
    let dir = root.join("csv").join(source.fingerprint()?);
    let manifest = match read_manifest(&dir) {
        Some(m) => m,
        None => build(&source, &dir)?,
    };
    load(path, &dir, manifest)
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let m: Manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json")).ok()?).ok()?;
    (m.version == MANIFEST_VERSION).then_some(m)
}

fn build(source: &Source, dir: &Path) -> Result<Manifest> {
    let path = source.path.as_path();
    let mut head = source.read(0, HEAD_BYTES)?;
    if (head.len() as u64) < source.len
        && let Some(end) = memchr::memrchr(b'\n', &head)
    {
        head.truncate(end + 1); // whole rows only
    }
    let schema = csv::scan(&head).map_err(|e| format_err(path, e.to_string()))?;
    drop(head);
    let parent = dir.parent().expect("cache dirs have a parent");
    fs::create_dir_all(parent).map_err(io_err(parent))?;
    let tmp = parent.join(format!(".{}.tmp-{}", dir.file_name().unwrap().to_string_lossy(), std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(io_err(&tmp))?;
    let mut schema = schema;
    let mut result =
        if schema.quoted { write_quoted(source, &schema, &tmp) } else { write_fast(source, &schema, &tmp) };
    if let Ok(None) = result {
        // a chunk met a quote: read the whole file with the general CSV reader instead
        schema.quoted = true;
        result = write_quoted(source, &schema, &tmp);
    }
    let result = result.map(|m| m.expect("the general reader never asks to start over"));
    let manifest = match result {
        Ok(m) => m,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let json = serde_json::to_vec_pretty(&manifest).expect("manifests serialize");
    fs::write(tmp.join("manifest.json"), json).map_err(io_err(&tmp))?;
    if fs::rename(&tmp, dir).is_err() {
        // another process built it first: use theirs
        let _ = fs::remove_dir_all(&tmp);
        return read_manifest(dir).ok_or_else(|| format_err(dir, "cache entry could not be written"));
    }
    Ok(manifest)
}

/// Unquoted data: 8 MB chunks, each read and parsed by its own worker.
fn write_fast(source: &Source, schema: &csv::Schema, dir: &Path) -> Result<Option<Manifest>> {
    let chunks = source.split(schema.data_start as u64, csv::CHUNK_BYTES as u64)?;
    write_columns(source, &chunks, schema, dir, &|r: &Range<u64>| {
        let b = source.read(r.start, (r.end - r.start) as usize)?;
        Ok(csv::parse(&b, 0..b.len(), schema))
    })
}

/// Quoted data: the whole file in one piece through the general CSV reader, from a memory map.
fn write_quoted(source: &Source, schema: &csv::Schema, dir: &Path) -> Result<Option<Manifest>> {
    let map = bytes::map(&source.path)?;
    map.advise_sequential();
    let whole = schema.data_start as u64..map.len() as u64;
    let chunks = std::slice::from_ref(&whole);
    write_columns(source, chunks, schema, dir, &|r: &Range<u64>| {
        Ok(csv::parse(&map, r.start as usize..r.end as usize, schema))
    })
}

type ParseChunk<'a> = dyn Fn(&Range<u64>) -> Result<csv::Rows> + Sync + 'a;

/// Parses the chunks and writes every column; `Ok(None)` when the fast path met a quote and the file must be read
/// again.
fn write_columns(
    source: &Source,
    chunks: &[Range<u64>],
    schema: &csv::Schema,
    dir: &Path,
    parse_chunk: &ParseChunk,
) -> Result<Option<Manifest>> {
    let path = source.path.as_path();
    let create = |name: &str| {
        let p = dir.join(name);
        File::create(&p).map(|f| BufWriter::with_capacity(1 << 20, f)).map_err(io_err(p))
    };
    let has_time = schema.time != TimeColumn::Index;
    let mut t_out = if has_time { Some(create("t.i64")?) } else { None };
    let numeric: Vec<_> = schema.numeric_columns().map(|(_, c)| c.clone()).collect();
    let mut outs = (0..numeric.len()).map(|k| create(&format!("c{k}.f64"))).collect::<Result<Vec<_>>>()?;

    // Batches parse in parallel while a writer thread writes the previous batch in order.
    let batch = rayon::current_num_threads() * 2;
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<csv::Rows>>(1);
    let written = std::thread::scope(|s| {
        let writer = s.spawn(move || -> Result<(usize, usize, Option<i64>, Option<i64>)> {
            let (mut rows, mut skipped) = (0usize, 0usize);
            let (mut t_first, mut t_last) = (None, None::<i64>);
            for parsed in rx {
                for p in parsed {
                    if let Some(w) = &mut t_out {
                        if let Some(i) = p.t.windows(2).position(|w| w[1] < w[0]) {
                            return Err(format_err(path, format!("time goes backwards at data row {}", rows + i + 2)));
                        }
                        if let (Some(prev), Some(&first)) = (t_last, p.t.first())
                            && first < prev
                        {
                            return Err(format_err(path, format!("time goes backwards at data row {}", rows + 1)));
                        }
                        t_first = t_first.or(p.t.first().copied());
                        t_last = p.t.last().copied().or(t_last);
                        w.write_all(bytemuck::cast_slice(&p.t)).map_err(io_err(dir))?;
                    }
                    for (w, col) in outs.iter_mut().zip(&p.columns) {
                        w.write_all(bytemuck::cast_slice(col)).map_err(io_err(dir))?;
                    }
                    rows += p.rows;
                    skipped += p.skipped;
                }
            }
            for mut w in t_out.into_iter().chain(outs) {
                w.flush().map_err(io_err(dir))?;
            }
            Ok((rows, skipped, t_first, t_last))
        });
        let (mut quotes, mut read_error) = (false, None);
        for group in chunks.chunks(batch) {
            let parsed = match group.par_iter().map(parse_chunk).collect::<Result<Vec<csv::Rows>>>() {
                Ok(p) => p,
                Err(e) => {
                    read_error = Some(e);
                    break;
                }
            };
            if parsed.iter().any(|r| r.quotes) {
                quotes = true;
                break;
            }
            if tx.send(parsed).is_err() {
                break; // the writer stopped with an error
            }
        }
        drop(tx);
        let written = writer.join().expect("the cache writer doesn't panic");
        match read_error {
            Some(e) => Err(e),
            None => written.map(|w| (w, quotes)),
        }
    });
    let ((rows, skipped, t_first, t_last), quotes) = written?;
    if quotes {
        return Ok(None);
    }
    let time = match schema.time {
        TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } => Some(schema.columns[index].name.clone()),
        TimeColumn::Index => None,
    };
    Ok(Some(Manifest {
        version: MANIFEST_VERSION,
        source_size: source.len,
        rows,
        skipped_rows: skipped,
        time,
        channels: numeric
            .into_iter()
            .enumerate()
            .map(|(k, c)| ManifestChannel { name: c.name, unit: c.unit, file: format!("c{k}.f64") })
            .collect(),
        skipped_columns: schema.columns.iter().filter(|c| c.kind == csv::Kind::Text).map(|c| c.name.clone()).collect(),
        t_first,
        t_last,
    }))
}

fn load(path: &Path, dir: &Path, m: Manifest) -> Result<CsvRecording> {
    let map_checked = |name: &str, elem: usize| -> Result<Bytes> {
        let b = bytes::map(&dir.join(name))?;
        if b.len() != m.rows * elem {
            return Err(format_err(dir.join(name), "cache file has the wrong size (delete the cache to rebuild)"));
        }
        Ok(b)
    };
    let t = m.time.as_ref().map(|_| map_checked("t.i64", 8)).transpose()?;
    let columns = m.channels.iter().map(|c| map_checked(&c.file, 8)).collect::<Result<Vec<_>>>()?;
    let (t_first, t_last) = match m.time {
        Some(_) => (m.t_first, m.t_last),
        None => (if m.rows > 0 { Some(0) } else { None }, m.rows.checked_sub(1).map(|r| r as i64 * 1_000_000_000)),
    };
    let info = RecordingInfo {
        path: path.to_owned(),
        format: "CSV".into(),
        samples: m.rows,
        time: match &m.time {
            Some(name) => TimeSource::Column { name: name.clone() },
            None => TimeSource::Index,
        },
        t_first,
        t_last,
        channels: m
            .channels
            .iter()
            .map(|c| ChannelInfo { name: c.name.clone(), unit: c.unit.clone(), dtype: "f64" })
            .collect(),
        skipped: m.skipped_columns,
    };
    Ok(CsvRecording { info, rows: m.rows, t, columns, skipped_rows: m.skipped_rows })
}

impl Recording for CsvRecording {
    fn info(&self) -> &RecordingInfo {
        &self.info
    }

    fn trace(&self, channel: usize, vp: Viewport) -> Trace {
        let values: &[f64] = bytemuck::cast_slice(&self.columns[channel]);
        match &self.t {
            Some(t) => m4(bytemuck::cast_slice::<u8, i64>(t), values, vp),
            None => m4(&RateTimes::from_rate(0, 1.0, self.rows), values, vp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_once_then_reuses() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("data.csv");
        let mut text = String::from("time,v,label\n");
        for i in 0..20_000 {
            text.push_str(&format!("{}.{:04},{},x\n", i / 10_000, i % 10_000, i % 7));
        }
        fs::write(&src, &text).unwrap();
        let root = tmp.path().join("cache");

        let r = open_csv(&src, &root).unwrap();
        assert_eq!(r.info().samples, 20_000);
        assert_eq!(r.info().skipped, ["label"]);
        assert_eq!(r.info().t_last, Some(1_999_900_000));
        let tr = r.trace(0, Viewport { t0: 0, t1: 2_000_000_000, width_px: 100 });
        assert_eq!(tr.kind, tlmbench_core::TraceKind::M4);
        assert!(tr.y.iter().all(|&y| (0.0..=6.0).contains(&y)));

        let entries: Vec<_> = fs::read_dir(root.join("csv")).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let manifest = entries[0].as_ref().unwrap().path().join("manifest.json");
        let before = fs::metadata(&manifest).unwrap().modified().unwrap();
        let again = open_csv(&src, &root).unwrap();
        assert_eq!(again.info(), r.info());
        assert_eq!(fs::metadata(&manifest).unwrap().modified().unwrap(), before);
        // nothing beside the source
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 2);
    }

    #[test]
    fn quotes_found_late_restart_with_the_general_reader() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("late_quotes.csv");
        let mut text = String::from("time,v,note\n");
        for i in 0..400_000 {
            text.push_str(&format!("{i},{i},x\n"));
        }
        text.push_str("400000,1,\"two\nlines\"\n400001,2,y\n");
        fs::write(&src, &text).unwrap();
        let r = open_csv(&src, &tmp.path().join("cache")).unwrap();
        assert_eq!(r.info().samples, 400_002);
        assert_eq!(r.info().t_last, Some(400_001_000_000_000));
    }

    #[test]
    fn backwards_time_fails_and_leaves_no_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("bad.csv");
        fs::write(&src, "t,v\n0,1\n2,2\n1,3\n").unwrap();
        let root = tmp.path().join("cache");
        let err = open_csv(&src, &root).unwrap_err().to_string();
        assert!(err.contains("backwards at data row 3"), "{err}");
        assert_eq!(fs::read_dir(root.join("csv")).unwrap().count(), 0);
    }
}
