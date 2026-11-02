//! TelemetryBench store: recordings opened with their cache entries, which live only under the cache root, never
//! beside the data. The CLI and the app share one root (`--cache`, else `$TLMBENCH_CACHE`, else the OS cache dir).
//!
//! Each recording gets a min/max pyramid ([`tlmbench_core::pyramid`]) and a gap index, built once in the
//! background and kept in `pyr/v1/<key>/`: `manifest.json`, one `c<k>.pyr` per channel (its stored levels back to
//! back, level 4 first, as 88-byte summaries) and `gaps.bin`. The key hashes the source's fingerprint with how it is
//! decoded, so a changed file or descriptor never reads stale summaries.
//!
//! A build reads the file once, in order, in blocks of 4^10 samples, writing each block's levels 4–10 in place in
//! preallocated files and recording its progress after each block (`progress.json`), so an interrupted build
//! resumes. Views meanwhile come from the finished blocks (exact; the rest is read raw). The coarser levels and the
//! gap index are made at the end, and `manifest.json`, written last, marks the entry complete. A lock file keeps two
//! processes from building the same key; the second follows the first.
//!
//! The cache is capped: when it passes 20 GB, or free disk drops below 10 GB, the least recently opened entries go
//! first. Entries in use by any process are kept.

mod error;
mod pyr;
pub mod sys;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, SystemTime};

pub use error::{Error, Result};
use rayon::prelude::*;
use serde::Serialize;
use tlmbench_core::pyramid::{MIN_BUCKET_SAMPLES, Pyramid, Stats};
use tlmbench_core::{Recording, Timestamp, Trace, TraceKind, Viewport};
use tlmbench_decode::OpenOptions;

use crate::pyr::Loaded;

/// The cache's size limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Total size of the cache, in bytes.
    pub cap: u64,
    /// Free space to keep on the cache's disk, in bytes.
    pub min_free: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { cap: 20 << 30, min_free: 10 << 30 }
    }
}

/// The cache root and what lives in it.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    pub limits: Limits,
}

impl Store {
    /// The store at `root`, else `$TLMBENCH_CACHE`, else the OS cache dir's `tlmbench`.
    pub fn new(root: Option<&Path>) -> Self {
        Self { root: tlmbench_decode::cache::cache_root(root), limits: Limits::default() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Opens a recording (CSV builds its column cache first). Its pyramid is loaded if built, but not built:
    /// call [`Opened::build`] (on a background thread in the app).
    pub fn open(&self, path: &Path, descriptor: Option<&Path>) -> Result<Opened> {
        let opts = OpenOptions { descriptor: descriptor.map(Path::to_owned), cache_dir: Some(self.root.clone()) };
        let rec = tlmbench_decode::open(path, &opts)?;
        let fingerprint = tlmbench_decode::cache::fingerprint(path)?;
        let key = pyr::key(&fingerprint, &rec.layout_id());
        let dir = self.root.join("pyr").join(pyr::VERSION);
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let in_use = hold(&self.root, &fingerprint)?;
        touch_opened(&self.root.join("csv").join(&fingerprint));
        let loaded = Loaded::open_built(&dir.join(&key), rec.as_ref())?;
        if loaded.is_some() {
            touch_opened(&dir.join(&key));
        }
        Ok(Opened {
            rec,
            store: self.clone(),
            fingerprint,
            key,
            dir,
            loaded: RwLock::new(loaded),
            cancel: AtomicBool::new(false),
            _in_use: in_use,
            building: Mutex::new(()),
        })
    }

    /// Every cache entry, most recently opened first.
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for (kind, dir) in [("csv", self.root.join("csv")), ("pyramid", self.root.join("pyr").join(pyr::VERSION))] {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                if !path.is_dir() || name.starts_with('.') {
                    continue;
                }
                let building = kind == "pyramid" && !path.join("manifest.json").exists();
                let fingerprint = match kind {
                    "csv" => Some(name.clone()),
                    _ => pyr::fingerprint_of(&path),
                };
                out.push(Entry {
                    kind,
                    path: path.clone(),
                    bytes: dir_size(&path),
                    opened: opened_at(&path),
                    building,
                    fingerprint,
                });
            }
        }
        out.sort_by_key(|e| std::cmp::Reverse(e.opened));
        Ok(out)
    }

    /// Removes the least recently opened entries until the cache is within its limits; entries in use by any
    /// process stay. Returns what was removed.
    pub fn evict(&self) -> Result<Vec<PathBuf>> {
        let mut entries = self.entries()?;
        let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
        let mut removed = Vec::new();
        let over = |total: u64| {
            total > self.limits.cap || sys::free_disk(&self.root).is_some_and(|f| f < self.limits.min_free)
        };
        while over(total) {
            let Some(pos) =
                entries.iter().rposition(|e| e.fingerprint.as_deref().is_none_or(|f| !in_use(&self.root, f)))
            else {
                break;
            };
            let e = entries.remove(pos);
            if fs::remove_dir_all(&e.path).is_ok() {
                total -= e.bytes;
                removed.push(e.path);
            }
        }
        Ok(removed)
    }

    /// Removes every entry not in use; returns what was removed.
    pub fn clear(&self) -> Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        for e in self.entries()? {
            if e.fingerprint.as_deref().is_none_or(|f| !in_use(&self.root, f)) && fs::remove_dir_all(&e.path).is_ok() {
                removed.push(e.path);
            }
        }
        Ok(removed)
    }
}

/// One cache entry.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// "csv" (the column cache) or "pyramid".
    pub kind: &'static str,
    pub path: PathBuf,
    pub bytes: u64,
    /// When a recording last opened it.
    #[serde(serialize_with = "unix_seconds")]
    pub opened: SystemTime,
    /// A pyramid build that hasn't finished (it resumes on the next open).
    pub building: bool,
    /// The source's fingerprint, when known.
    pub fingerprint: Option<String>,
}

fn unix_seconds<S: serde::Serializer>(t: &SystemTime, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_u64(t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

/// Where a recording's pyramid stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    /// Not built, and no build under way here.
    Missing,
    /// Samples `0..done` are summarized.
    Building {
        done: u64,
        total: u64,
    },
    Built,
}

/// A recording opened through the store.
#[derive(Debug)]
pub struct Opened {
    rec: Box<dyn Recording>,
    store: Store,
    fingerprint: String,
    key: String,
    /// `pyr/v1`
    dir: PathBuf,
    loaded: RwLock<Option<Loaded>>,
    cancel: AtomicBool,
    _in_use: File,
    building: Mutex<()>,
}

/// How a view is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// From the pyramid alone, reading no raw samples: fast on a cold file (see `pyramid::m4_coarse`).
    Coarse,
    /// The same pixels as every raw sample would draw.
    Exact,
}

/// A trace, and how it was made.
#[derive(Clone, Debug)]
pub struct View {
    pub channel: usize,
    pub trace: Trace,
    /// Answered (at least partly) from the pyramid.
    pub from_pyramid: bool,
    /// Coarse: summaries counted whole at pixel edges (it moves at most a quarter pixel); show it labelled.
    pub coarse: bool,
    /// Samples up to this time are summarized (None: all of them, or no pyramid yet).
    pub built_until: Option<Timestamp>,
}

impl Opened {
    pub fn recording(&self) -> &dyn Recording {
        self.rec.as_ref()
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn status(&self) -> Status {
        let g = self.loaded.read().unwrap();
        match &*g {
            Some(l) if l.complete => Status::Built,
            Some(l) => Status::Building { done: l.done as u64, total: self.rec.info().samples as u64 },
            None => Status::Missing,
        }
    }

    /// Builds the pyramid and gap index if they aren't built, calling `progress(done, total)` after each block.
    /// Resumes an interrupted build; waits for (and follows) one under way in another process. Returns early,
    /// leaving progress to resume from, when [`Opened::cancel`] is called.
    pub fn build(&self, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
        let _one = self.building.lock().unwrap();
        if self.status() == Status::Built {
            return Ok(());
        }
        let done = pyr::build(self, progress)?;
        if done {
            touch_opened(&self.dir.join(&self.key));
            let _ = self.store.evict();
        }
        Ok(())
    }

    /// Stops a build at the next block; it resumes on the next [`Opened::build`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Channel `channel` reduced for `vp`, with lines broken at gaps over `gap_factor` × the median interval.
    /// From the pyramid where it is built (exact: the same result as raw samples, faster).
    pub fn view(&self, channel: usize, vp: Viewport, gap_factor: f64, precision: Precision) -> View {
        self.views(&[channel], vp, gap_factor, precision).pop().expect("one channel")
    }

    /// [`Opened::view`] of several channels, in parallel (cold reads of different files overlap).
    pub fn views(&self, channels: &[usize], vp: Viewport, gap_factor: f64, precision: Precision) -> Vec<View> {
        let g = self.loaded.read().unwrap();
        let l = g.as_ref();
        let breaks = l.map(|l| l.gaps.breaks(gap_factor)).unwrap_or_default();
        let built_until =
            l.filter(|l| !l.complete).map(|l| if l.done == 0 { i64::MIN } else { self.rec.time_at(l.done - 1) });
        let (lo, hi) = (self.rec.lower_bound(vp.t0), self.rec.lower_bound(vp.t1));
        let in_view = hi.saturating_sub(lo);
        let coarse_pays = in_view / vp.width_px.max(1) as usize >= MIN_BUCKET_SAMPLES;
        // An exact view reads level 4 at every pixel edge. A cold page fault costs about as much as reading 128 KB
        // in sequence, so when edges are closer than about 1024 summaries (88 KB) the range is read through first.
        // Only when cold: from memory, reading through costs more than it saves.
        let warm = precision == Precision::Exact
            && coarse_pays
            && in_view / vp.width_px.max(1) as usize <= 1024 * 256
            && l.zip(channels.first()).and_then(|(l, &c)| l.cold(c, lo, hi.min(l.done))) == Some(true);
        if warm {
            // every pixel edge reads raw samples next to it: fetch them for all channels at once
            let bw = vp.bucket_ns();
            let (b0, b1) = (vp.t0.div_euclid(bw), (vp.t1 - 1).div_euclid(bw) + 1);
            let edges: Vec<_> = (b0..=b1)
                .map(|b| self.rec.lower_bound(b.saturating_mul(bw)))
                .map(|i| i.saturating_sub(256)..i + 256)
                .collect();
            self.rec.prefetch(&edges);
        }
        channels
            .par_iter()
            .map(|&channel| {
                if warm && let Some(l) = l {
                    l.warm(channel, lo, hi.min(l.done));
                }
                let mut pyr = l.and_then(|l| l.pyramid(channel));
                // while building, a large unread part of the view stays empty (hatched) rather than read raw
                // alongside the build; a view zoomed into it is read raw
                let unread = l.filter(|l| !l.complete).map_or(0, |l| hi.saturating_sub(lo.max(l.done)));
                let skip_unread = unread > UNREAD_RAW_MAX;
                if let Some(p) = &mut pyr {
                    p.raw_past = !skip_unread;
                }
                let coarse = precision == Precision::Coarse && coarse_pays && pyr.is_some();
                let trace = match &pyr {
                    Some(p) if coarse => self.rec.view_coarse(channel, vp, p, &breaks),
                    None if skip_unread => {
                        Trace { kind: TraceKind::Raw, bucket_ns: vp.bucket_ns(), t: vec![], y: vec![] }
                    }
                    _ => self.rec.view(channel, vp, pyr.as_ref(), &breaks),
                };
                View { channel, trace, from_pyramid: pyr.is_some(), coarse, built_until }
            })
            .collect()
    }

    /// Exact statistics of the samples in `[t0, t1)`; None until the pyramid is built (exact answers wait for it).
    pub fn stats(&self, channel: usize, t0: Timestamp, t1: Timestamp) -> Option<Stats> {
        let g = self.loaded.read().unwrap();
        let l = g.as_ref().filter(|l| l.complete)?;
        let (a, b) = (self.rec.lower_bound(t0), self.rec.lower_bound(t1));
        if a >= b {
            return Some(Stats { count: 0, min: None, max: None, mean: f64::NAN, std: f64::NAN });
        }
        Some(self.rec.stats(channel, &l.pyramid(channel)?, a, b))
    }

    /// The sample nearest `t` (the earlier one on ties), as (index, time, shown value).
    pub fn nearest(&self, channel: usize, t: Timestamp) -> Option<(usize, Timestamp, f64)> {
        let n = self.rec.info().samples;
        if n == 0 {
            return None;
        }
        let i = self.rec.lower_bound(t);
        let pick = match (i.checked_sub(1), (i < n).then_some(i)) {
            (Some(a), Some(b)) => {
                if t - self.rec.time_at(a) <= self.rec.time_at(b) - t {
                    a
                } else {
                    b
                }
            }
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => return None,
        };
        Some((pick, self.rec.time_at(pick), self.rec.value_at(channel, pick)))
    }

    /// Indexes of the samples that start after a gap, for `gap_factor` (empty until the gap index is built).
    pub fn breaks(&self, gap_factor: f64) -> Vec<usize> {
        self.loaded.read().unwrap().as_ref().filter(|l| l.complete).map_or_else(Vec::new, |l| l.gaps.breaks(gap_factor))
    }

    /// The median sample interval from the gap index, once built.
    pub fn median_interval_ns(&self) -> Option<i64> {
        self.loaded.read().unwrap().as_ref().filter(|l| l.complete).map(|l| l.gaps.median_ns)
    }

    /// Runs `f` with channel `channel`'s pyramid, if any of it is built.
    pub fn with_pyramid<R>(&self, channel: usize, f: impl FnOnce(Option<&Pyramid>) -> R) -> R {
        let g = self.loaded.read().unwrap();
        f(g.as_ref().and_then(|l| l.pyramid(channel)).as_ref())
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Marks the source with fingerprint `fp` as in use for as long as the returned file is open (a shared lock).
fn hold(root: &Path, fp: &str) -> Result<File> {
    let dir = root.join("use");
    fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    let path = dir.join(fp);
    let f = File::options().create(true).truncate(false).write(true).open(&path).map_err(Error::io(&path))?;
    f.lock_shared().map_err(Error::io(&path))?;
    Ok(f)
}

/// Whether any process holds the source with fingerprint `fp` open.
fn in_use(root: &Path, fp: &str) -> bool {
    let path = root.join("use").join(fp);
    match File::options().write(true).open(&path) {
        Ok(f) => f.try_lock().is_err(),
        Err(_) => false,
    }
}

fn touch_opened(dir: &Path) {
    if dir.is_dir() {
        let _ = fs::write(dir.join("opened"), b"");
    }
}

fn opened_at(dir: &Path) -> SystemTime {
    let m = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    m(&dir.join("opened")).or_else(|| m(dir)).unwrap_or(SystemTime::UNIX_EPOCH)
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

/// The most unread samples a view of a file being built reads raw: past this the unread part stays empty.
const UNREAD_RAW_MAX: usize = 1 << 22;

/// How long a process waits between looks at another process's build.
const FOLLOW_EVERY: Duration = Duration::from_millis(200);
