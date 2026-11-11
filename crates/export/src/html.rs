//! Export to HTML, the data half: a folder of static files the viewer reads (`tlmbench export html`, the app's Export).
//!
//! ```text
//! export.json            what the export holds: files, channels, sizes, the layout, events, the checks summary, checksums
//! data/f<k>/t.<j>.bin    file k's sample times (i64 LE ns), chunks of CHUNK samples; none when the times come from a rate
//! data/f<k>/c<n>.<j>.bin channel n's shown values (f64 LE, NaN for missing), chunks of CHUNK samples (Full, or raw channels)
//! data/f<k>/c<n>.pyr.bin channel n's pyramid levels (88-byte summaries, finest stored level first, levels end to end)
//! ```
//!
//! Values are shown values (scale and offset applied), so the viewer's pyramid and views are over exactly what the app
//! shows; times are exact. The viewer folder (`index.html`, the script, the WASM core) is copied beside by the caller.
//!
//! Size budget (250 MB by default): **Full** ships raw samples for every channel; **Light** ships raw samples only
//! for the channels the caller marks (tables, values, XY) and the pyramid for the rest; if Light still doesn't fit,
//! the finest pyramid levels are left out channel by channel until it does.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tlmbench_core::pyramid::{self, Summary, build_levels, level_lengths};
use tlmbench_core::{Recording, TimeSource, Timestamp};

use crate::{Error, Result, io_err};

/// Samples per data chunk file.
pub const CHUNK: usize = 1 << 20;
/// The default size budget of an export folder.
pub const DEFAULT_BUDGET: u64 = 250 * 1024 * 1024;
/// Bytes of a summary.
const SUMMARY: u64 = size_of::<Summary>() as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Full when it fits the budget, else Light.
    #[default]
    Auto,
    Full,
    Light,
}

/// One file of the export.
#[derive(Debug)]
pub struct File<'a> {
    pub name: String,
    pub rec: &'a dyn Recording,
    /// The channels to export (indexes into the recording), in order.
    pub channels: Vec<usize>,
    /// Channels that keep raw samples in Light (shown in a table, values or XY).
    pub raw: Vec<usize>,
}

#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    pub budget: u64,
    /// The span to export: samples from this time (inclusive) to this time (exclusive); None: the whole recording.
    pub from: Option<Timestamp>,
    pub until: Option<Timestamp>,
}

impl Default for Options {
    fn default() -> Self {
        Self { mode: Mode::Auto, budget: DEFAULT_BUDGET, from: None, until: None }
    }
}

/// What an export holds, printed by the CLI and written to `export.json`.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub mode: Mode,
    pub budget: u64,
    pub bytes: u64,
    pub files: Vec<FileReport>,
    /// The mode Auto picked was Light, or the finest levels were left out: the viewer says so past that zoom.
    pub trimmed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileReport {
    pub name: String,
    pub samples: usize,
    pub bytes: u64,
    pub channels: Vec<ChannelReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelReport {
    pub name: String,
    pub raw: bool,
    pub raw_bytes: u64,
    pub pyramid_bytes: u64,
    /// The finest pyramid level shipped (4 is the finest there is).
    pub first_level: u32,
}

struct Plan {
    /// The samples exported.
    range: (usize, usize),
    times: Vec<Timestamp>,
    /// The times are a fixed rate: `(t0, hz)`; no time files.
    rate: Option<(Timestamp, f64)>,
    chans: Vec<Chan>,
}

struct Chan {
    index: usize,
    values: Vec<f64>,
    levels: Vec<Vec<Summary>>,
    raw: bool,
    /// Levels dropped from the finest end.
    dropped: usize,
}

fn shown(rec: &dyn Recording, ch: usize, a: usize, b: usize) -> Vec<f64> {
    let (scale, offset) = rec.scale_offset(ch);
    let col = rec.read(ch, a, b);
    (0..col.len()).map(|i| col.get_f64(i) * scale + offset).collect()
}

fn plan(f: &File, o: &Options) -> Plan {
    let info = f.rec.info();
    let n = info.samples;
    let a = o.from.map_or(0, |t| f.rec.lower_bound(t)).min(n);
    let b = o.until.map_or(n, |t| f.rec.lower_bound(t)).clamp(a, n);
    let times = f.rec.read_times(a, b);
    let rate = match &info.time {
        TimeSource::Rate { hz } if !times.is_empty() => Some((times[0], *hz)),
        TimeSource::Index if !times.is_empty() => Some((times[0], 1.0)),
        _ => None,
    };
    let chans = f
        .channels
        .iter()
        .map(|&index| {
            let values = shown(f.rec, index, a, b);
            let levels = if values.is_empty() {
                Vec::new()
            } else {
                // level 4 over the shown values with the exported times, then each coarser level
                build_levels(pyramid::summarize(times.as_slice(), values.as_slice(), 0, values.len()))
            };
            Chan { index, values, levels, raw: f.raw.contains(&index), dropped: 0 }
        })
        .collect();
    Plan { range: (a, b), times, rate, chans }
}

fn raw_bytes(len: usize) -> u64 {
    len as u64 * 8
}

fn pyr_bytes(c: &Chan) -> u64 {
    c.levels.iter().skip(c.dropped).map(|l| l.len() as u64).sum::<u64>() * SUMMARY
}

fn total(plans: &[Plan], full: bool) -> u64 {
    plans
        .iter()
        .map(|p| {
            let t = if p.rate.is_some() { 0 } else { raw_bytes(p.times.len()) };
            t + p.chans.iter().map(|c| pyr_bytes(c) + if full || c.raw { raw_bytes(c.values.len()) } else { 0 }).sum::<u64>()
        })
        .sum()
}

fn write(path: &Path, bytes: &[u8], sums: &mut BTreeMap<String, String>, root: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    fs::write(path, bytes).map_err(io_err(path))?;
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
    sums.insert(rel, blake3::hash(bytes).to_hex().to_string());
    Ok(())
}

fn f64_bytes(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Writes the data files and `export.json` into `out` (created; an existing `export.json` is replaced). `extra` is
/// merged into the manifest: the layout, events and the checks summary, as the caller has them.
pub fn export(out: &Path, files: &[File], extra: serde_json::Value, o: &Options) -> Result<Report> {
    if files.is_empty() {
        return Err(Error::Write("nothing to export".into()));
    }
    let mut plans: Vec<Plan> = files.iter().map(|f| plan(f, o)).collect();
    let full_size = total(&plans, true);
    let full = match o.mode {
        Mode::Full => true,
        Mode::Light => false,
        Mode::Auto => full_size <= o.budget,
    };
    let mut trimmed = !full && o.mode == Mode::Auto;
    if !full {
        // leave out the finest levels, the biggest channels first, until the folder fits
        while total(&plans, false) > o.budget {
            let Some(c) = plans
                .iter_mut()
                .flat_map(|p| p.chans.iter_mut())
                .filter(|c| c.levels.len().saturating_sub(c.dropped) > 1)
                .max_by_key(|c| pyr_bytes(c))
            else {
                break;
            };
            c.dropped += 1;
            trimmed = true;
        }
    }
    fs::create_dir_all(out).map_err(io_err(out))?;
    let mut sums = BTreeMap::new();
    let mut reports = Vec::new();
    let mut manifest_files = Vec::new();
    let mut bytes_all = 0u64;
    for (k, (f, p)) in files.iter().zip(&plans).enumerate() {
        let dir = out.join("data").join(format!("f{k}"));
        let info = f.rec.info();
        let n = p.times.len();
        let mut file_bytes = 0u64;
        let time = match p.rate {
            Some((t0, hz)) => serde_json::json!({ "kind": "rate", "t0": t0.to_string(), "hz": hz }),
            None => {
                for (j, c) in p.times.chunks(CHUNK).enumerate() {
                    let b: Vec<u8> = c.iter().flat_map(|t| t.to_le_bytes()).collect();
                    file_bytes += b.len() as u64;
                    write(&dir.join(format!("t.{j}.bin")), &b, &mut sums, out)?;
                }
                serde_json::json!({ "kind": "column", "chunks": n.div_ceil(CHUNK) })
            }
        };
        let mut chans = Vec::new();
        let mut creports = Vec::new();
        for (c, ci) in p.chans.iter().zip(&f.channels) {
            let ch = &info.channels[*ci];
            let keep_raw = full || c.raw;
            let mut rb = 0;
            if keep_raw {
                for (j, v) in c.values.chunks(CHUNK).enumerate() {
                    let b = f64_bytes(v);
                    rb += b.len() as u64;
                    write(&dir.join(format!("c{}.{j}.bin", c.index)), &b, &mut sums, out)?;
                }
            }
            let kept: Vec<&Vec<Summary>> = c.levels.iter().skip(c.dropped).collect();
            let pb: Vec<u8> = kept.iter().flat_map(|l| bytemuck_bytes(l)).collect();
            write(&dir.join(format!("c{}.pyr.bin", c.index)), &pb, &mut sums, out)?;
            file_bytes += rb + pb.len() as u64;
            let first_level = pyramid::MIN_LEVEL + c.dropped as u32;
            chans.push(serde_json::json!({
                "index": c.index, "name": ch.name, "unit": ch.unit, "dtype": ch.dtype, "category": ch.category,
                "raw": keep_raw, "raw_chunks": if keep_raw { n.div_ceil(CHUNK) } else { 0 },
                "pyramid": { "first_level": first_level, "levels": kept.iter().map(|l| l.len()).collect::<Vec<_>>() },
            }));
            creports.push(ChannelReport { name: ch.name.clone(), raw: keep_raw, raw_bytes: rb, pyramid_bytes: pb.len() as u64, first_level });
        }
        // the levels a channel of n samples has, so the viewer can check what it reads
        debug_assert!(p.chans.iter().all(|c| c.values.is_empty() || c.levels.len() == level_lengths(n).len()));
        bytes_all += file_bytes;
        manifest_files.push(serde_json::json!({
            "k": k, "name": f.name, "path": info.path, "format": info.format, "part": info.part,
            "samples": n, "range": [p.range.0, p.range.1], "chunk": CHUNK, "time": time,
            "t_first": p.times.first().map(|t| t.to_string()), "t_last": p.times.last().map(|t| t.to_string()),
            "channels": chans,
        }));
        reports.push(FileReport { name: f.name.clone(), samples: n, bytes: file_bytes, channels: creports });
    }
    let report = Report { mode: if full { Mode::Full } else { Mode::Light }, budget: o.budget, bytes: bytes_all, files: reports, trimmed };
    let mut manifest = serde_json::json!({
        "format": "tlmbench-export", "version": 1,
        "report": report, "files": manifest_files, "checksums": sums,
    });
    if let (Some(m), serde_json::Value::Object(x)) = (manifest.as_object_mut(), extra) {
        for (key, v) in x {
            m.insert(key, v);
        }
    }
    let path: PathBuf = out.join("export.json");
    fs::write(&path, serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Write(e.to_string()))?).map_err(io_err(&path))?;
    Ok(report)
}

fn bytemuck_bytes(l: &[Summary]) -> Vec<u8> {
    // Summary is plain old data (repr(C), 88 bytes): write its bytes as they are, little-endian on every target we ship
    l.iter().flat_map(|s| summary_le(s)).collect()
}

/// A summary's 88 bytes, little-endian, in field order.
pub fn summary_le(s: &Summary) -> [u8; 88] {
    let mut b = [0u8; 88];
    let mut at = 0;
    let mut put = |x: [u8; 8]| {
        b[at..at + 8].copy_from_slice(&x);
        at += 8;
    };
    put(s.t_first.to_le_bytes());
    put(s.t_last.to_le_bytes());
    put(s.min.to_le_bytes());
    put(s.min_idx.to_le_bytes());
    put(s.max.to_le_bytes());
    put(s.max_idx.to_le_bytes());
    put(s.first.to_le_bytes());
    put(s.last.to_le_bytes());
    put(s.sum.to_le_bytes());
    put(s.sum_sq.to_le_bytes());
    put(s.count.to_le_bytes());
    b
}
