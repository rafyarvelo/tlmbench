//! Export to HTML, the data half: a folder of static files the viewer reads (`tlmbench export html`, the app's Export).
//!
//! ```text
//! export.json            what the export holds: files, channels, sizes, the layout, events, the checks summary, checksums
//! data/f<k>/t.<j>.bin    file k's sample times (i64 LE ns), chunks of CHUNK samples; none when the times come from a rate
//! data/f<k>/c<n>.<j>.bin channel n's shown values (f64 LE, NaN for missing): one file per entry of `raw_chunks` in
//!                        the manifest, `[first sample, samples]`. Full and raw channels hold every sample in chunks of
//!                        CHUNK; in Light a channel holds only its windows (around events and violations)
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
    /// Stretches of time (start inclusive, end exclusive) every channel keeps its raw samples for in Light: around
    /// events and check violations (see [`Window`]).
    pub windows: Vec<(Timestamp, Timestamp)>,
}

/// How much raw data Light keeps around an event or a violation: a window centred on it, `factor` times its duration
/// wide or `min_ns` if that is more, at most `cap_ns` (the event itself is always inside).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub min_ns: i64,
    pub factor: f64,
    pub cap_ns: i64,
}

impl Default for Window {
    /// 30 s, or 3 times the event's duration when that is longer, at most 10 minutes (the design's raw windows).
    fn default() -> Self {
        Self { min_ns: 30_000_000_000, factor: 3.0, cap_ns: 600_000_000_000 }
    }
}

impl Window {
    /// The window (start inclusive, end exclusive) around an event that starts at `start` and lasts `duration_ns` (0:
    /// an instant).
    pub fn around(&self, start: Timestamp, duration_ns: i64) -> (Timestamp, Timestamp) {
        let dur = duration_ns.max(0);
        let width = (((dur as f64) * self.factor) as i64).max(self.min_ns).min(self.cap_ns.max(self.min_ns));
        let centre = start.saturating_add(dur / 2);
        let (a, b) = (centre.saturating_sub(width / 2), centre.saturating_add(width - width / 2));
        (a.min(start), b.max(start.saturating_add(dur)).max(start.saturating_add(1)))
    }

    /// `--detail`: a duration is the minimum (`30s`), or `3x,30s,10min` give the factor, the minimum and the cap in any
    /// order; `0` is no windows (`None`).
    pub fn parse(text: &str) -> std::result::Result<Option<Window>, String> {
        let t = text.trim();
        if t == "0" || t.eq_ignore_ascii_case("off") || t.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        let mut w = Window::default();
        let (mut seen_min, mut seen_cap) = (false, false);
        for part in t.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some(f) = part.strip_suffix(['x', 'X', '×']) {
                w.factor = f
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|f| *f >= 0.0)
                    .ok_or_else(|| format!("{part:?} is not a factor like 3x"))?;
            } else {
                let ns = duration_ns(part)?;
                // the first duration is the minimum, the second the cap
                if !seen_min {
                    (w.min_ns, seen_min) = (ns, true);
                } else if !seen_cap {
                    (w.cap_ns, seen_cap) = (ns, true);
                } else {
                    return Err(format!("{text:?} has more than a factor, a minimum and a cap"));
                }
            }
        }
        if w.cap_ns < w.min_ns {
            w.cap_ns = w.min_ns;
        }
        Ok(Some(w))
    }
}

/// `30s`, `500ms`, `10min`, `2h` or plain seconds, in nanoseconds.
fn duration_ns(text: &str) -> std::result::Result<i64, String> {
    let t = text.trim().to_ascii_lowercase();
    let (num, unit) = [("ms", 1e6), ("min", 6e10), ("m", 6e10), ("h", 3.6e12), ("s", 1e9)]
        .iter()
        .find_map(|(u, m)| t.strip_suffix(u).map(|n| (n, *m)))
        .unwrap_or((&t, 1e9));
    num.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| *n >= 0.0)
        .map(|n| (n * unit) as i64)
        .ok_or_else(|| format!("{text:?} is not a duration (30s, 500ms, 10min)"))
}

/// `spans` sorted, with those that overlap or touch joined.
fn merged(mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    spans.retain(|(a, b)| a < b);
    spans.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (a, b) in spans {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
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
    /// Whether Light keeps every raw sample of this channel (it is shown in a table, values or XY).
    raw: bool,
    /// The sample ranges Light keeps (the windows), when it doesn't keep them all.
    windows: Vec<(usize, usize)>,
    /// Levels dropped from the finest end.
    dropped: usize,
}

impl Chan {
    /// The samples held raw: every one (`full`, or a raw channel), else the windows.
    fn spans(&self, full: bool) -> Vec<(usize, usize)> {
        if self.values.is_empty() {
            Vec::new()
        } else if full || self.raw {
            vec![(0, self.values.len())]
        } else {
            self.windows.clone()
        }
    }
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
    // the windows as sample ranges of the exported span
    let windows = merged(
        f.windows
            .iter()
            .map(|&(t0, t1)| (times.partition_point(|&t| t < t0), times.partition_point(|&t| t < t1)))
            .collect(),
    );
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
            Chan { index, values, levels, raw: f.raw.contains(&index), windows: windows.clone(), dropped: 0 }
        })
        .collect();
    Plan { range: (a, b), times, rate, chans }
}

/// What `export.json` records about how it was made (`provenance`): the command line, a hash of everything that
/// configured the export (the layout, the check sets, the options: `config`, as the caller has it), the core's version,
/// and each input file's size and BLAKE3 (the whole file, read once). A report's footer shows the same.
pub fn provenance(command: &[String], config: &serde_json::Value, inputs: &[&Path]) -> Result<serde_json::Value> {
    use std::io::Read;
    let mut files = Vec::new();
    for p in inputs {
        let mut f = fs::File::open(p).map_err(io_err(p))?;
        let (mut h, mut n, mut buf) = (blake3::Hasher::new(), 0u64, vec![0u8; 1 << 20]);
        loop {
            let k = f.read(&mut buf).map_err(io_err(p))?;
            if k == 0 {
                break;
            }
            h.update(&buf[..k]);
            n += k as u64;
        }
        files.push(serde_json::json!({ "path": p, "bytes": n, "blake3": h.finalize().to_hex().to_string() }));
    }
    // serde_json keeps object keys sorted, so the same configuration always hashes the same
    let config_hash =
        blake3::hash(&serde_json::to_vec(config).map_err(|e| Error::Write(e.to_string()))?).to_hex().to_string();
    // when it was made, for the viewer's header (Unix milliseconds; not part of the configuration's hash)
    let created =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    Ok(serde_json::json!({
        "command": command,
        "config_hash": config_hash,
        "core_version": env!("CARGO_PKG_VERSION"),
        "created": created,
        "inputs": files,
    }))
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
            t + p
                .chans
                .iter()
                .map(|c| pyr_bytes(c) + c.spans(full).iter().map(|(a, b)| raw_bytes(b - a)).sum::<u64>())
                .sum::<u64>()
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
    export_with(out, files, extra, o, &mut |_, _| {})
}

/// [`export`], telling `progress(done, total)` as it goes: one step per file read, then one per channel written.
pub fn export_with(
    out: &Path,
    files: &[File],
    extra: serde_json::Value,
    o: &Options,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Report> {
    if files.is_empty() {
        return Err(Error::Write("nothing to export".into()));
    }
    let steps = files.len() + files.iter().map(|f| f.channels.len()).sum::<usize>();
    let mut done = 0;
    progress(done, steps);
    let mut plans: Vec<Plan> = files
        .iter()
        .map(|f| {
            let p = plan(f, o);
            done += 1;
            progress(done, steps);
            p
        })
        .collect();
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
            let spans = c.spans(full);
            let keep_raw = !spans.is_empty();
            let mut rb = 0;
            // one file per chunk of a span: `[first sample, samples]` in the manifest, in file order
            let mut raw_chunks = Vec::new();
            for &(a, b) in &spans {
                for s in (a..b).step_by(CHUNK) {
                    let e = (s + CHUNK).min(b);
                    let bytes = f64_bytes(&c.values[s..e]);
                    rb += bytes.len() as u64;
                    write(&dir.join(format!("c{}.{}.bin", c.index, raw_chunks.len())), &bytes, &mut sums, out)?;
                    raw_chunks.push([s, e - s]);
                }
            }
            let kept: Vec<&Vec<Summary>> = c.levels.iter().skip(c.dropped).collect();
            let pb: Vec<u8> = kept.iter().flat_map(|l| bytemuck_bytes(l)).collect();
            write(&dir.join(format!("c{}.pyr.bin", c.index)), &pb, &mut sums, out)?;
            file_bytes += rb + pb.len() as u64;
            let first_level = pyramid::MIN_LEVEL + c.dropped as u32;
            chans.push(serde_json::json!({
                "index": c.index, "name": ch.name, "unit": ch.unit, "dtype": ch.dtype, "category": ch.category,
                "raw": keep_raw, "raw_chunks": raw_chunks,
                "pyramid": { "first_level": first_level, "levels": kept.iter().map(|l| l.len()).collect::<Vec<_>>() },
            }));
            creports.push(ChannelReport {
                name: ch.name.clone(),
                raw: keep_raw,
                raw_bytes: rb,
                pyramid_bytes: pb.len() as u64,
                first_level,
            });
            done += 1;
            progress(done, steps);
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
    let report = Report {
        mode: if full { Mode::Full } else { Mode::Light },
        budget: o.budget,
        bytes: bytes_all,
        files: reports,
        trimmed,
    };
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
    fs::write(&path, serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Write(e.to_string()))?)
        .map_err(io_err(&path))?;
    Ok(report)
}

fn bytemuck_bytes(l: &[Summary]) -> Vec<u8> {
    // Summary is plain old data (repr(C), 88 bytes): write its bytes as they are, little-endian on every target we ship
    l.iter().flat_map(summary_le).collect()
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

/// The viewer built into this program: `(path, bytes)` pairs, empty when `just viewer` hasn't run before the build.
/// A pack is, per file, a u32 LE path length, the path, a u32 LE data length and the data.
pub fn embedded_viewer() -> Vec<(String, Vec<u8>)> {
    unpack(include_bytes!(concat!(env!("OUT_DIR"), "/viewer.pack")))
}

/// The one-file viewer built into this program (`index.html` with a `<!--TLM_FILES-->` marker, and the WASM core).
pub fn embedded_single_viewer() -> Vec<(String, Vec<u8>)> {
    unpack(include_bytes!(concat!(env!("OUT_DIR"), "/single.pack")))
}

/// A one-file export's size limit, the design's 25 MB.
pub const SINGLE_FILE_LIMIT: u64 = 25 * 1024 * 1024;

/// The bytes the one-file viewer takes before any data (its page and the core).
pub fn single_overhead(viewer: &[(String, Vec<u8>)]) -> u64 {
    // the core is base64 in the page
    viewer.iter().map(|(p, d)| if p == "index.html" { d.len() as u64 } else { (d.len() as u64).div_ceil(3) * 4 }).sum()
}

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A one-file export: the viewer's page with the export folder `dir` (`export.json` and `data/**`) and the core inside
/// it, as one HTML file that opens from `file://` (a page there can't fetch the core or the data).
pub fn single_file(dir: &Path, viewer: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let page = viewer
        .iter()
        .find(|(p, _)| p == "index.html")
        .ok_or_else(|| Error::Write("the one-file viewer has no page".into()))?;
    let html = String::from_utf8_lossy(&page.1).into_owned();
    let mut entries: Vec<(String, Vec<u8>)> = viewer.iter().filter(|(p, _)| p != "index.html").cloned().collect();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
        let mut names: Vec<_> = fs::read_dir(dir).map_err(io_err(dir))?.flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            if p.is_dir() {
                walk(root, &p, out)?;
            } else {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((rel, fs::read(&p).map_err(io_err(&p))?));
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut entries)?;
    let mut table = String::from("{");
    for (i, (path, data)) in entries.iter().enumerate() {
        if i > 0 {
            table.push(',');
        }
        table.push_str(&serde_json::to_string(path).map_err(|e| Error::Write(e.to_string()))?);
        table.push(':');
        table.push('"');
        table.push_str(&base64(data));
        table.push('"');
    }
    table.push('}');
    let tag = format!("<script>window.__tlmFiles={table}</script>");
    if !html.contains("<!--TLM_FILES-->") {
        return Err(Error::Write("the one-file viewer has no place for the data".into()));
    }
    Ok(html.replacen("<!--TLM_FILES-->", &tag, 1).into_bytes())
}

pub fn unpack(mut b: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let take = |b: &mut &[u8], n: usize| -> Option<Vec<u8>> {
        let (x, rest) = b.split_at_checked(n)?;
        *b = rest;
        Some(x.to_vec())
    };
    let num = |b: &mut &[u8]| -> Option<usize> { Some(u32::from_le_bytes(take(b, 4)?.try_into().ok()?) as usize) };
    while !b.is_empty() {
        let (Some(n),) = (num(&mut b),) else { break };
        let Some(path) = take(&mut b, n) else { break };
        let Some(m) = num(&mut b) else { break };
        let Some(data) = take(&mut b, m) else { break };
        out.push((String::from_utf8_lossy(&path).into_owned(), data));
    }
    out
}

/// Writes `files` (a viewer pack's contents) into `out`.
pub fn write_viewer(out: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    for (rel, data) in files {
        // a pack never names a path outside the folder
        if rel.split('/').any(|p| p == ".." || p.is_empty()) {
            return Err(Error::Write(format!("the viewer names {rel:?}")));
        }
        let path = out.join(rel);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_err(dir))?;
        }
        fs::write(&path, data).map_err(io_err(&path))?;
    }
    Ok(())
}
