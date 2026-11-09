//! The columnar cache for formats that can't be read in place: CSV, and Parquet (see [`crate::parquet`]).
//!
//! The first open parses the source in parallel into one file per column under the cache root: `t.i64` (sample
//! times, absent with index time) and `c<k>.<type>` (`f64` for CSV, the column's own type for Parquet), plain
//! little-endian arrays, plus `manifest.json`. Later opens
//! memory-map those files, so reopening costs the fingerprint only. The cache lives in the app cache dir, never
//! beside the source, and can be deleted at any time: it is rebuilt on the next open.
//!
//! The key is a fingerprint of the source: its size and modification time, and a BLAKE3 hash of its first and
//! last MiB and 64 blocks spread through it. Hashing whole multi-GB files would cost about a second on each open.
//!
//! The source is read with positioned reads, each worker reading its own 8 MB chunk. Parallel page faults on one
//! memory map serialize on macOS (about 330 MB/s on an M1 for a 2.4 GB CSV); separate reads run at disk speed.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tlmbench_core::chunks::CHUNK_SAMPLES;
use tlmbench_core::{
    ChannelInfo, ChannelOp, Channels, Indexed, RateTimes, RecordingInfo, TimeIndex, TimeSource, Times, TimesOp,
};

use crate::bytes::{self, Bytes, read_exact_at};
use crate::csv::{self, TimeColumn};
use crate::descriptor::DType;
use crate::error::{Result, format_err, io_err};
use crate::profile::{ImportProfile, RateChoice};

pub(crate) const MANIFEST_VERSION: u32 = 1;

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

/// The cache key of a source file: its size and modification time, and a BLAKE3 hash of its first and last MiB
/// and 63 blocks spread through it (32 hex digits).
pub fn fingerprint(path: &Path) -> Result<String> {
    Source::open(path)?.fingerprint()
}

/// [`fingerprint`] with an import profile: the file's own key, joined with the profile's when it changes anything,
/// so a different profile has its own cache entry and pyramid.
pub fn fingerprint_with(path: &Path, profile: Option<&ImportProfile>) -> Result<String> {
    Ok(profiled_key(fingerprint(path)?, profile))
}

fn profiled_key(fp: String, profile: Option<&ImportProfile>) -> String {
    match profile.filter(|p| !p.is_default()) {
        Some(p) => blake3::hash(format!("{fp}\n{}", p.key()).as_bytes()).to_hex()[..32].to_owned(),
        None => fp,
    }
}

/// Bytes read for the schema: the header and the sample rows.
const HEAD_BYTES: usize = 16 << 20;

/// The source file, read with positioned reads.
#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) path: PathBuf,
    file: File,
    pub(crate) len: u64,
    mtime_ns: u128,
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<Self> {
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

    pub(crate) fn fingerprint(&self) -> Result<String> {
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) version: u32,
    pub(crate) source_size: u64,
    pub(crate) rows: usize,
    pub(crate) skipped_rows: usize,
    /// None: index time (row numbers).
    pub(crate) time: Option<String>,
    pub(crate) channels: Vec<ManifestChannel>,
    pub(crate) skipped_columns: Vec<String>,
    pub(crate) t_first: Option<i64>,
    pub(crate) t_last: Option<i64>,
    /// An even rate from `t0` instead of a time column or the row number (a profile's assumed or resampled rate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rate: Option<ManifestRate>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct ManifestRate {
    pub(crate) t0: i64,
    pub(crate) hz: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ManifestChannel {
    pub(crate) name: String,
    pub(crate) unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) category: Option<String>,
    pub(crate) file: String,
    #[serde(default = "f64_type")]
    pub(crate) dtype: DType,
}

fn f64_type() -> DType {
    DType::F64
}

/// A file read through its cache entry (CSV or Parquet).
#[derive(Debug)]
pub struct CachedRecording {
    info: RecordingInfo,
    rows: usize,
    t: Option<Bytes>,
    columns: Vec<(Bytes, DType)>,
    /// Rows left out because their time didn't parse.
    pub skipped_rows: usize,
    /// The chunk index of the time column, once the store has it.
    time_index: std::sync::OnceLock<TimeIndex>,
    /// An even rate from the first time, for a recording with no time column.
    rate: Option<ManifestRate>,
}

/// Opens a CSV through the cache under `root`, building the cache entry first if needed.
pub fn open_csv(path: &Path, root: &Path) -> Result<CachedRecording> {
    open_csv_with(path, root, None)
}

/// [`open_csv`] with an import profile (names, units, types, the time column and the rate).
pub fn open_csv_with(path: &Path, root: &Path, profile: Option<&ImportProfile>) -> Result<CachedRecording> {
    let source = Source::open(path)?;
    let dir = root.join("csv").join(profiled_key(source.fingerprint()?, profile));
    let manifest = match read_manifest(&dir) {
        Some(m) => m,
        None => build_entry(&dir, |tmp| build(&source, tmp, profile))?,
    };
    load(path, &dir, manifest, "CSV")
}

pub(crate) fn read_manifest(dir: &Path) -> Option<Manifest> {
    let m: Manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json")).ok()?).ok()?;
    (m.version == MANIFEST_VERSION).then_some(m)
}

/// A suffix for a temporary folder that no other build, in this process or another, uses at the same moment (files with
/// the same contents share a cache entry and may be opened at once).
pub(crate) fn unique_suffix() -> String {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    format!("{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// Builds a cache entry: `write` fills a temporary folder beside `dir` and returns the manifest, which is written
/// last; the folder is then renamed into place, so a failed or interrupted build leaves nothing.
pub(crate) fn build_entry(dir: &Path, write: impl FnOnce(&Path) -> Result<Manifest>) -> Result<Manifest> {
    let parent = dir.parent().expect("cache dirs have a parent");
    fs::create_dir_all(parent).map_err(io_err(parent))?;
    let tmp =
        parent.join(format!(".{}.tmp-{}", dir.file_name().unwrap().to_string_lossy(), crate::cache::unique_suffix()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(io_err(&tmp))?;
    let manifest = match write(&tmp) {
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

fn build(source: &Source, tmp: &Path, profile: Option<&ImportProfile>) -> Result<Manifest> {
    let path = source.path.as_path();
    let mut head = source.read(0, HEAD_BYTES)?;
    if (head.len() as u64) < source.len
        && let Some(end) = memchr::memrchr(b'\n', &head)
    {
        head.truncate(end + 1); // whole rows only
    }
    let schema = csv::scan_with(&head, profile).map_err(|e| format_err(path, e.to_string()))?;
    drop(head);
    let mut schema = schema;
    let mut result = if schema.quoted { write_quoted(source, &schema, tmp) } else { write_fast(source, &schema, tmp) };
    if let Ok(None) = result {
        // a chunk met a quote: read the whole file with the general CSV reader instead
        schema.quoted = true;
        let _ = fs::remove_dir_all(tmp);
        fs::create_dir_all(tmp).map_err(io_err(tmp))?;
        result = write_quoted(source, &schema, tmp);
    }
    let manifest = result.map(|m| m.expect("the general reader never asks to start over"))?;
    match profile.map(|p| p.rate) {
        Some(RateChoice::Resample { hz }) => resample(manifest, hz, tmp, path),
        _ => Ok(manifest),
    }
}

/// Rewrites the columns on an even grid at `hz` from the first time, by linear interpolation (a gap, a missing
/// value on either side of a point, or a point outside the data is missing), and drops the time column.
fn resample(mut m: Manifest, hz: f64, dir: &Path, path: &Path) -> Result<Manifest> {
    let (Some(t0), Some(t1)) = (m.t_first, m.t_last) else {
        return Err(format_err(path, "can't resample: the file has no time column"));
    };
    if m.time.is_none() {
        return Err(format_err(path, "can't resample: the file has no time column"));
    }
    let n = ((t1 - t0) as f64 * hz / 1e9).floor() as usize + 1;
    if n > (1usize << 34) {
        return Err(format_err(path, format!("resampling at {hz} Hz would make {n} rows")));
    }
    let grid = RateTimes::from_rate(t0, hz, n);
    let tmap = bytes::map(&dir.join("t.i64"))?;
    let t: &[i64] = bytemuck::cast_slice(&tmap);
    for ch in &mut m.channels {
        let src = bytes::map(&dir.join(&ch.file))?;
        let y: &[f64] = bytemuck::cast_slice(&src);
        let out = dir.join(format!("{}.rs", ch.file));
        let mut w = BufWriter::with_capacity(1 << 20, File::create(&out).map_err(io_err(&out))?);
        let mut j = 0usize; // t[j] <= grid time < t[j + 1]
        let mut buf = Vec::with_capacity(1 << 16);
        for k in 0..n {
            let tk = grid.at(k);
            while j + 1 < t.len() && t[j + 1] <= tk {
                j += 1;
            }
            let v = if t[j] == tk {
                y[j]
            } else if j + 1 < t.len() && t[j] < tk {
                let f = (tk - t[j]) as f64 / (t[j + 1] - t[j]) as f64;
                y[j] + (y[j + 1] - y[j]) * f // NaN on either side stays NaN
            } else {
                f64::NAN
            };
            buf.push(v);
            if buf.len() == buf.capacity() {
                w.write_all(bytemuck::cast_slice(&buf)).map_err(io_err(&out))?;
                buf.clear();
            }
        }
        w.write_all(bytemuck::cast_slice(&buf)).map_err(io_err(&out))?;
        w.flush().map_err(io_err(&out))?;
        drop(src);
        fs::rename(&out, dir.join(&ch.file)).map_err(io_err(&out))?;
    }
    drop(tmap);
    fs::remove_file(dir.join("t.i64")).map_err(io_err(dir))?;
    m.rows = n;
    m.time = None;
    m.rate = Some(ManifestRate { t0, hz });
    m.t_first = Some(t0);
    m.t_last = Some(grid.at(n - 1));
    Ok(m)
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
            .map(|(k, c)| ManifestChannel {
                name: c.name,
                unit: c.unit,
                category: c.category,
                file: format!("c{k}.f64"),
                dtype: DType::F64,
            })
            .collect(),
        skipped_columns: schema
            .columns
            .iter()
            .filter(|c| matches!(c.kind, csv::Kind::Text | csv::Kind::Skip))
            .map(|c| c.name.clone())
            .collect(),
        t_first,
        t_last,
        rate: schema.index_rate.map(|hz| ManifestRate { t0: 0, hz }),
    }))
}

pub(crate) fn load(path: &Path, dir: &Path, m: Manifest, format: &str) -> Result<CachedRecording> {
    let map_checked = |name: &str, elem: usize| -> Result<Bytes> {
        let b = bytes::map(&dir.join(name))?;
        if b.len() != m.rows * elem {
            return Err(format_err(dir.join(name), "cache file has the wrong size (delete the cache to rebuild)"));
        }
        Ok(b)
    };
    let t = m.time.as_ref().map(|_| map_checked("t.i64", 8)).transpose()?;
    let columns = m
        .channels
        .iter()
        .map(|c| Ok((map_checked(&c.file, c.dtype.size() as usize)?, c.dtype)))
        .collect::<Result<Vec<_>>>()?;
    let (t_first, t_last) = match (&m.time, m.rate) {
        (Some(_), _) => (m.t_first, m.t_last),
        (None, Some(r)) if m.rows > 0 => (Some(r.t0), Some(RateTimes::from_rate(r.t0, r.hz, m.rows).at(m.rows - 1))),
        (None, _) => (if m.rows > 0 { Some(0) } else { None }, m.rows.checked_sub(1).map(|r| r as i64 * 1_000_000_000)),
    };
    let info = RecordingInfo {
        path: path.to_owned(),
        format: format.into(),
        part: None,
        samples: m.rows,
        time: match (&m.time, m.rate) {
            (Some(name), _) => TimeSource::Column { name: name.clone() },
            (None, Some(r)) => TimeSource::Rate { hz: r.hz },
            (None, None) => TimeSource::Index,
        },
        t_first,
        t_last,
        channels: m
            .channels
            .iter()
            .map(|c| ChannelInfo {
                name: c.name.clone(),
                unit: c.unit.clone(),
                dtype: c.dtype.name(),
                category: c.category.clone(),
            })
            .collect(),
        skipped: m.skipped_columns,
    };
    Ok(CachedRecording {
        info,
        rows: m.rows,
        t,
        columns,
        skipped_rows: m.skipped_rows,
        time_index: Default::default(),
        rate: m.rate,
    })
}

impl Channels for CachedRecording {
    fn describe(&self) -> &RecordingInfo {
        &self.info
    }

    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out {
        struct WithTimes<'a, O>(O, &'a [u8], DType);
        impl<O: ChannelOp> TimesOp for WithTimes<'_, O> {
            type Out = O::Out;
            fn run<T: tlmbench_core::Times + ?Sized>(self, times: &T) -> O::Out {
                let WithTimes(op, b, dtype) = self;
                match dtype {
                    DType::I8 => op.run(times, bytemuck::cast_slice::<u8, i8>(b)),
                    DType::I16 => op.run(times, bytemuck::cast_slice::<u8, i16>(b)),
                    DType::I32 => op.run(times, bytemuck::cast_slice::<u8, i32>(b)),
                    DType::I64 => op.run(times, bytemuck::cast_slice::<u8, i64>(b)),
                    DType::U8 => op.run(times, bytemuck::cast_slice::<u8, u8>(b)),
                    DType::U16 => op.run(times, bytemuck::cast_slice::<u8, u16>(b)),
                    DType::U32 => op.run(times, bytemuck::cast_slice::<u8, u32>(b)),
                    DType::U64 => op.run(times, bytemuck::cast_slice::<u8, u64>(b)),
                    DType::F32 => op.run(times, bytemuck::cast_slice::<u8, f32>(b)),
                    DType::F64 => op.run(times, bytemuck::cast_slice::<u8, f64>(b)),
                }
            }
        }
        let (bytes, dtype) = &self.columns[channel];
        self.dispatch_times(WithTimes(op, bytes, *dtype))
    }

    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out {
        match (&self.t, self.time_index.get()) {
            (Some(t), Some(index)) => op.run(&Indexed { times: bytemuck::cast_slice::<u8, i64>(t), index }),
            (Some(t), None) => op.run(bytemuck::cast_slice::<u8, i64>(t)),
            (None, _) => match self.rate {
                Some(r) => op.run(&RateTimes::from_rate(r.t0, r.hz, self.rows)),
                None => op.run(&RateTimes::from_rate(0, 1.0, self.rows)),
            },
        }
    }

    fn set_time_index(&self, index: TimeIndex) {
        if self.t.is_some() && index.0.len() == self.rows.div_ceil(CHUNK_SAMPLES) {
            let _ = self.time_index.set(index);
        }
    }

    fn layout_id(&self) -> Vec<u8> {
        format!("{}\n{MANIFEST_VERSION}\n", self.info.format.to_lowercase()).into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlmbench_core::{Recording, Viewport};

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
        let tr = r.trace(0, Viewport { t0: 0, t1: 2_000_000_000, width_px: 100, raw_max: 0 });
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

    fn sample_csv(dir: &Path) -> PathBuf {
        let src = dir.join("p.csv");
        fs::write(&src, "time_ms,Speed [m/s],gps,note\n0,1,10,a\n100,2,20,b\n200,3,30,c\n400,5,50,d\n").unwrap();
        src
    }

    #[test]
    fn profile_renames_retypes_and_skips() {
        use crate::profile::*;
        let tmp = tempfile::tempdir().unwrap();
        let src = sample_csv(tmp.path());
        let root = tmp.path().join("cache");
        let plain = open_csv(&src, &root).unwrap();
        assert_eq!(plain.info().channels.len(), 2); // time_ms is the time column
        let p = ImportProfile {
            columns: vec![
                ColumnChoice {
                    source: "Speed".into(),
                    name: Some("v".into()),
                    unit: Some("km/h".into()),
                    category: Some("nav".into()),
                    ..Default::default()
                },
                ColumnChoice { source: "gps".into(), kind: Some(ColumnKind::Skip), ..Default::default() },
            ],
            ..Default::default()
        };
        let r = open_csv_with(&src, &root, Some(&p)).unwrap();
        let ch = &r.info().channels;
        assert_eq!(ch.len(), 1);
        assert_eq!(
            (ch[0].name.as_str(), ch[0].unit.as_deref(), ch[0].category.as_deref()),
            ("v", Some("km/h"), Some("nav"))
        );
        assert_eq!(r.info().skipped, ["gps", "note"]);
        // its own entry; the plain one is untouched
        assert_eq!(fs::read_dir(root.join("csv")).unwrap().count(), 2);
        assert_eq!(fingerprint_with(&src, Some(&p)).unwrap(), fingerprint_with(&src, Some(&p.clone())).unwrap());
        assert_eq!(fingerprint_with(&src, Some(&ImportProfile::default())).unwrap(), fingerprint(&src).unwrap());
    }

    #[test]
    fn profile_picks_the_time_column_and_unit() {
        use crate::profile::*;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("t.csv");
        fs::write(&src, "a,b,c\n1,0,5\n2,1000,6\n3,2000,7\n").unwrap();
        let p = ImportProfile {
            time: TimeChoice::Column { name: "b".into(), unit: Some("ms".into()) },
            ..Default::default()
        };
        let r = open_csv_with(&src, &tmp.path().join("c"), Some(&p)).unwrap();
        assert_eq!(r.info().t_last, Some(2_000_000_000));
        assert_eq!(r.info().channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        let bad = ImportProfile { time: TimeChoice::Column { name: "zz".into(), unit: None }, ..Default::default() };
        assert!(open_csv_with(&src, &tmp.path().join("c2"), Some(&bad)).is_err());
    }

    #[test]
    fn profile_rates() {
        use crate::profile::*;
        let tmp = tempfile::tempdir().unwrap();
        let src = sample_csv(tmp.path());
        let root = tmp.path().join("cache");
        let with = |rate| open_csv_with(&src, &root, Some(&ImportProfile { rate, ..Default::default() })).unwrap();

        // assumed rate: the time column is ignored and becomes an ordinary channel
        let a = with(RateChoice::Assume { hz: 10.0 });
        assert_eq!(a.info().time, TimeSource::Rate { hz: 10.0 });
        assert_eq!(a.info().t_last, Some(300_000_000));
        assert_eq!(a.info().channels.len(), 3); // time_ms, Speed, gps (note is text)
        // sample numbers
        let s = with(RateChoice::Samples);
        assert_eq!(s.info().time, TimeSource::Index);
        assert_eq!(s.info().samples, 4);
        // resample: 10 Hz from 0 to 0.4 s: five rows, linear between the recorded samples
        let r = with(RateChoice::Resample { hz: 10.0 });
        assert_eq!(r.info().samples, 5);
        assert_eq!(r.info().time, TimeSource::Rate { hz: 10.0 });
        assert_eq!(r.info().t_last, Some(400_000_000));
        let tr = r.trace(0, Viewport { t0: 0, t1: 500_000_000, width_px: 100, raw_max: 100 });
        assert_eq!(tr.y, [1.0, 2.0, 3.0, 4.0, 5.0]); // 0.3 s was never recorded: 4 is interpolated
        // a file with no time column can't be resampled
        let no_time = tmp.path().join("n.csv");
        fs::write(&no_time, "x,y\n1,2\n3,4\n").unwrap();
        let p = ImportProfile { rate: RateChoice::Resample { hz: 5.0 }, ..Default::default() };
        assert!(open_csv_with(&no_time, &root, Some(&p)).is_err());
        let bad = ImportProfile { rate: RateChoice::Assume { hz: 0.0 }, ..Default::default() };
        assert!(open_csv_with(&src, &root, Some(&bad)).is_err());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Derived channels: columns computed from another recording, stored in the same layout so they reopen the same way.

/// How a derived entry's sample times are stored.
#[derive(Clone, Debug)]
pub enum DerivedTime {
    /// `t.i64` holds a time per sample (the `fill` function writes it).
    Column(String),
    /// An even rate from `t0` (nanoseconds).
    Rate { t0: i64, hz: f64 },
    /// The sample number.
    Index,
}

/// A computed channel of a derived entry: `c<k>.f64` holds its values.
#[derive(Clone, Debug)]
pub struct DerivedChannel {
    pub name: String,
    pub unit: Option<String>,
    pub category: Option<String>,
}

/// What a derived entry holds besides its columns.
#[derive(Clone, Debug)]
pub struct DerivedSpec {
    pub rows: usize,
    pub time: DerivedTime,
    pub t_first: Option<i64>,
    pub t_last: Option<i64>,
    pub channels: Vec<DerivedChannel>,
}

/// Opens the derived entry `key` under `root` (`derived/<key>`), building it first if needed: `fill` writes `t.i64`
/// (for a time column) and `c<k>.f64` for each channel into the folder it is given, or says why it can't. `path` is
/// the source file, for the recording's info.
pub fn open_derived(
    path: &Path,
    root: &Path,
    key: &str,
    spec: &DerivedSpec,
    fill: impl FnOnce(&Path) -> std::result::Result<(), String>,
) -> Result<CachedRecording> {
    let dir = root.join("derived").join(key);
    let manifest = match read_manifest(&dir) {
        Some(m) => m,
        None => build_entry(&dir, |tmp| {
            fill(tmp).map_err(|msg| format_err(path, msg))?;
            Ok(Manifest {
                version: MANIFEST_VERSION,
                source_size: 0,
                rows: spec.rows,
                skipped_rows: 0,
                time: match &spec.time {
                    DerivedTime::Column(n) => Some(n.clone()),
                    _ => None,
                },
                channels: spec
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(k, c)| ManifestChannel {
                        name: c.name.clone(),
                        unit: c.unit.clone(),
                        category: c.category.clone(),
                        file: format!("c{k}.f64"),
                        dtype: DType::F64,
                    })
                    .collect(),
                skipped_columns: Vec::new(),
                t_first: spec.t_first,
                t_last: spec.t_last,
                rate: match spec.time {
                    DerivedTime::Rate { t0, hz } => Some(ManifestRate { t0, hz }),
                    _ => None,
                },
            })
        })?,
    };
    load(path, &dir, manifest, "Derived")
}
