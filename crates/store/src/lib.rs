//! TelemetryBench store: recordings opened with their cache entries, which live only under the cache root, never
//! beside the data. The CLI and the app share one root (`--cache`, else `$TLMBENCH_CACHE`, else the OS cache dir).
//!
//! Each recording gets a min/max pyramid ([`tlmbench_core::pyramid`]) and a gap index, built once in the
//! background and kept in `pyr/v2/<key>/`: `manifest.json`, one `c<k>.pyr` per channel (its stored levels back to
//! back, level 4 first, as 88-byte summaries), `gaps.bin` and, for a time column, `times.bin` (the time of every 64k-th
//! sample, so a search for a time reads one chunk of the column). The key hashes the source's fingerprint with how it is
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
//!
//! Channels not stored as contiguous arrays (fields of interleaved records, big-endian ones) are read through
//! decoded chunks kept in memory ([`ChunkCache`], 25% of RAM): raw reductions fill the chunks they read for every
//! channel of the request at once; views from the pyramid use the chunks already there and read the rest in place.

mod adjust;
pub mod align;
pub mod checks;
mod chunks;
pub mod config;
mod derived;
mod error;
pub mod events;
pub mod group;
mod joined;
mod live;
mod pyr;
mod recipes;
pub mod sys;
pub mod trigger;
pub mod widgets;
pub mod xy;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

pub use chunks::ChunkCache;
pub use error::{Error, Result};
pub use live::{Live, LiveDerived, LiveDerivedOpened};
use rayon::prelude::*;
use serde::Serialize;
use tlmbench_core::chunks::{CHUNK_SAMPLES, Held, chunk_of};
use tlmbench_core::gaps::{DEFAULT_FACTOR, MIN_FACTOR};
use tlmbench_core::mask::Mask;
use tlmbench_core::pyramid::{MIN_BUCKET_SAMPLES, Pyramid, Stats};
use tlmbench_core::table::{self, Col, Filter, Query, Rows, SortKey};
use tlmbench_core::{Recording, Timestamp, Trace, TraceKind, Viewport};
use tlmbench_decode::OpenOptions;
use tlmbench_decode::profile::ImportProfile;

pub use crate::adjust::Adjust;
pub use crate::derived::{Derived, ExprEnv, Foreign, RecEnv};
use crate::pyr::Loaded;
pub use crate::recipes::{Library, PREVIEW_SAMPLES, Preview, check, check_cross};

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

/// The cache root and what lives in it, and the chunks in memory (shared by every recording opened through it).
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    pub limits: Limits,
    chunks: Arc<ChunkCache>,
    /// Applied to every CSV file this store opens.
    profile: Option<Arc<ImportProfile>>,
    /// Scale, offset and missing values by channel name, applied to every recording this store opens.
    adjust: Arc<Vec<(String, Adjust)>>,
    /// Value labels and limits the expressions of derived channels read (set by the app from its settings).
    expr_env: Arc<std::sync::RwLock<ExprEnv>>,
}

impl Store {
    /// The store at `root`, else `$TLMBENCH_CACHE`, else the OS cache dir's `tlmbench`.
    pub fn new(root: Option<&Path>) -> Self {
        Self {
            root: tlmbench_decode::cache::cache_root(root),
            limits: Limits::default(),
            chunks: Arc::new(ChunkCache::new(ChunkCache::default_budget())),
            profile: None,
            adjust: Arc::default(),
            expr_env: Arc::default(),
        }
    }

    /// Sets the value labels and limits derived channels' expressions see (`state == 'flight'`, `past_warn(x)`).
    pub fn set_expr_env(&self, env: ExprEnv) {
        *self.expr_env.write().unwrap() = env;
    }

    /// The same store, applying `profile` to every CSV file it opens (each profile has its own column cache and
    /// pyramid; other formats ignore it).
    pub fn with_profile(mut self, profile: Option<ImportProfile>) -> Self {
        self.profile = profile.filter(|p| !p.is_default()).map(Arc::new);
        self
    }

    /// The same store, applying `adjust` (by channel name) to every recording it opens: a scale and offset on top of
    /// the format's, and which samples are missing. Each distinct set has its own pyramid.
    pub fn with_adjust(mut self, adjust: Vec<(String, Adjust)>) -> Self {
        self.adjust = Arc::new(adjust.into_iter().filter(|(_, a)| *a != Adjust::default()).collect());
        self
    }

    /// The same store with a chunk cache of `bytes` (0: channels are always read in place).
    pub fn with_chunk_budget(mut self, bytes: u64) -> Self {
        self.chunks = Arc::new(ChunkCache::new(bytes));
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn chunk_cache(&self) -> &ChunkCache {
        &self.chunks
    }

    /// Opens a recording (CSV and Parquet build their column cache first). Its pyramid is loaded if built, but not built:
    /// call [`Opened::build`] (on a background thread in the app). A packet file with several packet types needs
    /// [`Store::open_part`] or [`Store::open_all`].
    pub fn open(&self, path: &Path, descriptor: Option<&Path>) -> Result<Opened> {
        self.open_part(path, descriptor, None)
    }

    /// Opens the packet type `part` of a file with `[[packet_types]]` (any other file: `None`).
    pub fn open_part(&self, path: &Path, descriptor: Option<&Path>, part: Option<&str>) -> Result<Opened> {
        let opts = self.options(descriptor, part);
        let opts = self.adjusted(path, opts, |o| tlmbench_decode::open(path, o).map(|r| vec![r]))?;
        self.wrap(path, tlmbench_decode::open(path, &opts)?)
    }

    /// Opens every recording a file holds: one, or one per packet type, in the descriptor's order.
    pub fn open_all(&self, path: &Path, descriptor: Option<&Path>) -> Result<Vec<Opened>> {
        let opts = self.options(descriptor, None);
        let opts = self.adjusted(path, opts, |o| tlmbench_decode::open_all(path, o))?;
        tlmbench_decode::open_all(path, &opts)?.into_iter().map(|rec| self.wrap(path, rec)).collect()
    }

    /// `opts` with this store's adjustments: the conditions in them are evaluated against the file opened plain.
    fn adjusted(
        &self,
        _path: &Path,
        mut opts: OpenOptions,
        open: impl Fn(&OpenOptions) -> tlmbench_decode::Result<Vec<Box<dyn Recording>>>,
    ) -> Result<OpenOptions> {
        if self.adjust.is_empty() {
            return Ok(opts);
        }
        let plain = if self.adjust.iter().any(|(_, a)| a.when.is_some()) { open(&opts)? } else { Vec::new() };
        let refs: Vec<&dyn Recording> = plain.iter().map(|r| r.as_ref()).collect();
        opts.adjust = crate::adjust::resolve(&self.adjust, &refs)?;
        Ok(opts)
    }

    fn options(&self, descriptor: Option<&Path>, part: Option<&str>) -> OpenOptions {
        OpenOptions {
            descriptor: descriptor.map(Path::to_owned),
            part: part.map(str::to_owned),
            profile: self.profile.as_deref().cloned(),
            cache_dir: Some(self.root.clone()),
            adjust: Vec::new(),
        }
    }

    fn wrap(&self, path: &Path, rec: Box<dyn Recording>) -> Result<Opened> {
        let profile = self.profile.as_deref().filter(|_| rec.info().format == "CSV");
        let fingerprint = tlmbench_decode::cache::fingerprint_with(path, profile)?;
        self.wrap_keyed(rec, fingerprint)
    }

    /// `rec` with its cache entries named by `fingerprint` (a source file's, or a derived entry's key).
    fn wrap_keyed(&self, rec: Box<dyn Recording>, fingerprint: String) -> Result<Opened> {
        let key = pyr::key(&fingerprint, &rec.layout_id());
        let dir = self.root.join("pyr").join(pyr::VERSION);
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let in_use = hold(&self.root, &fingerprint)?;
        for kind in COLUMN_CACHES {
            touch_opened(&self.root.join(kind).join(&fingerprint));
        }
        let loaded = Loaded::open_built(&dir.join(&key), rec.as_ref())?;
        if loaded.is_some() {
            touch_opened(&dir.join(&key));
        }
        Ok(Opened {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            live: false,
            filling: Mutex::new(()),
            rec,
            joined: None,
            store: self.clone(),
            fingerprint,
            key,
            dir,
            loaded: RwLock::new(loaded),
            cancel: AtomicBool::new(false),
            _in_use: in_use,
            building: Mutex::new(()),
            masks: Mutex::default(),
            mask: RwLock::default(),
            runs: Mutex::default(),
        })
    }

    /// Every cache entry, most recently opened first.
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        // pyramids of every layout version: older ones are no longer read, and age out like any entry
        let versions = fs::read_dir(self.root.join("pyr")).into_iter().flatten().flatten().map(|e| e.path());
        let columns = COLUMN_CACHES.iter().map(|&k| (k, self.root.join(k)));
        let packets = std::iter::once(("packets", self.root.join("packets")));
        let dirs = columns.chain(packets).chain(versions.map(|d| ("pyramid", d)));
        for (kind, dir) in dirs {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                if !path.is_dir() || name.starts_with('.') {
                    continue;
                }
                let building = kind == "pyramid" && !path.join("manifest.json").exists();
                let fingerprint = match kind {
                    "pyramid" => pyr::fingerprint_of(&path),
                    // a packet index is read into memory on open: removing it never disturbs a reader
                    "packets" => None,
                    _ => Some(name.clone()),
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

/// Folders under the cache root holding column caches, named by the format they decode.
const COLUMN_CACHES: [&str; 3] = ["csv", "parquet", "derived"];

/// One cache entry.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// "csv" or "parquet" (the column cache of that format), "packets" (a packet index) or "pyramid".
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

/// Numbers opened recordings, for chunk cache keys.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A recording opened through the store.
#[derive(Debug)]
pub struct Opened {
    id: u64,
    /// A snapshot of a [`Live`] recording: it shares its chunks with the next snapshots.
    live: bool,
    /// One decode at a time, so two queries don't decode the same chunks.
    filling: Mutex<()>,
    rec: Box<dyn Recording>,
    /// For a recording joined with its derived channels: the two parts (see [`Store::join`]).
    joined: Option<(Arc<Opened>, Arc<Opened>)>,
    store: Store,
    fingerprint: String,
    key: String,
    /// `pyr/v2`
    dir: PathBuf,
    loaded: RwLock<Option<Loaded>>,
    cancel: AtomicBool,
    _in_use: File,
    building: Mutex<()>,
    /// What each owner (a table with Apply to charts) leaves out, and everything they leave out together.
    masks: Mutex<std::collections::HashMap<u32, Mask>>,
    mask: RwLock<Arc<Mask>>,
    /// Where each state channel's runs of equal values begin (see [`widgets`]).
    runs: Mutex<std::collections::HashMap<usize, Option<Arc<Vec<usize>>>>>,
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

/// One entry of a view batch: channels (each with its gap factor) reduced for a viewport.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub channels: Vec<usize>,
    /// A time jump over this many median intervals breaks the line, per channel (missing: [`DEFAULT_FACTOR`]; at
    /// least [`MIN_FACTOR`]).
    pub gap_factors: Vec<f64>,
    pub vp: Viewport,
    /// Connect the line across its gaps (time jumps, missing values, excluded rows) instead of breaking it; the
    /// connected stretches come back in [`View::dashed`].
    pub connect_gaps: bool,
}

/// How [`Opened::view_batch`] answers one request.
#[derive(Clone, Copy, Debug)]
struct Plan {
    /// Samples `lo..hi` are in view.
    lo: usize,
    hi: usize,
    /// Pixels of at least [`MIN_BUCKET_SAMPLES`]: summaries answer them (else they are reduced raw).
    coarse_pays: bool,
    /// Read level 4 through before an exact view of a cold file.
    warm: bool,
}

impl Plan {
    /// The chunks a raw reduction of the view reads, with the whole pixels at its edges.
    fn chunks(&self, n: usize) -> Vec<usize> {
        let (a, b) = (self.lo.saturating_sub(MIN_BUCKET_SAMPLES), (self.hi + MIN_BUCKET_SAMPLES).min(n));
        if a < b { (chunk_of(a)..=chunk_of(b - 1)).collect() } else { Vec::new() }
    }
}

/// A trace, and how it was made.
#[derive(Clone, Debug)]
pub struct View {
    pub channel: usize,
    pub trace: Trace,
    /// With `connect_gaps`: the index of each point that ends a connected stretch (the segment from the point
    /// before it is drawn dashed).
    pub dashed: Vec<u32>,
    /// Answered (at least partly) from the pyramid.
    pub from_pyramid: bool,
    /// Coarse: summaries counted whole at pixel edges (it moves at most a quarter pixel); show it labelled.
    pub coarse: bool,
    /// Samples up to this time are summarized (None: all of them, or no pyramid yet).
    pub built_until: Option<Timestamp>,
}

impl Opened {
    /// The store this was opened through (with its profile and adjustments).
    pub fn store(&self) -> &Store {
        &self.store
    }

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
        if let Some((base, derived)) = &self.joined {
            // each part builds its own pyramid (the recording's is usually there already); the joined one is the two
            let n = self.rec.info().samples as u64;
            base.build(&mut |d, t| progress(d.min(t) * n / t.max(1) / 2, n))?;
            derived.build(&mut |d, t| progress(n / 2 + d.min(t) * n / t.max(1) / 2, n))?;
            self.compose();
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
        if let Some((base, derived)) = &self.joined {
            base.cancel();
            derived.cancel();
        }
    }

    /// A joined recording's pyramid, once both parts have theirs.
    fn compose(&self) {
        let Some((base, derived)) = &self.joined else { return };
        let (b, d) = (base.loaded.read().unwrap(), derived.loaded.read().unwrap());
        if let (Some(b), Some(d)) = (b.as_ref(), d.as_ref())
            && let Ok(Some(l)) = Loaded::concat(b, d)
        {
            *self.loaded.write().unwrap() = Some(l);
        }
    }

    /// Whether this is a recording joined with derived channels.
    pub fn is_joined(&self) -> bool {
        self.joined.is_some()
    }

    /// Channel `channel` reduced for `vp`, with lines broken at gaps over `gap_factor` × the median interval.
    /// From the pyramid where it is built (exact: the same result as raw samples, faster).
    pub fn view(&self, channel: usize, vp: Viewport, gap_factor: f64, precision: Precision) -> View {
        self.views(&[channel], vp, &[gap_factor], precision).pop().expect("one channel")
    }

    /// [`Opened::view`] of several channels, in parallel (cold reads of different files overlap), each with its
    /// gap factor (`gap_factors[i]` for `channels[i]`; missing ones are [`DEFAULT_FACTOR`]).
    pub fn views(&self, channels: &[usize], vp: Viewport, gap_factors: &[f64], precision: Precision) -> Vec<View> {
        let req = Request { channels: channels.to_vec(), gap_factors: gap_factors.to_vec(), vp, connect_gaps: false };
        self.view_batch(std::slice::from_ref(&req), precision).pop().expect("one request")
    }

    /// Every request of a batch (the panes of a tab and the overview, each with its own viewport and width), with
    /// the chunks that raw reductions read decoded once for all of them.
    pub fn view_batch(&self, requests: &[Request], precision: Precision) -> Vec<Vec<View>> {
        let g = self.loaded.read().unwrap();
        let l = g.as_ref();
        let n = self.rec.info().samples;
        let plans: Vec<Plan> = requests.iter().map(|r| self.plan(r, l, precision)).collect();
        // raw reductions read every sample in view: their chunks are decoded for all their channels at once
        let mut chans: Vec<usize> = Vec::new();
        let mut ks: Vec<usize> = Vec::new();
        for (r, p) in requests.iter().zip(&plans).filter(|(_, p)| !p.coarse_pays) {
            chans.extend(&r.channels);
            ks.extend(p.chunks(n));
        }
        chans.sort_unstable();
        chans.dedup();
        ks.sort_unstable();
        ks.dedup();
        let decoded: Vec<Held> = self.hold(&chans, &ks, true);
        // views from the pyramid read a few samples at each pixel edge: through the chunks already decoded, else
        // in place
        requests
            .iter()
            .zip(&plans)
            .map(|(r, p)| {
                let held: Vec<Held> = if !p.coarse_pays {
                    r.channels.iter().map(|c| decoded[chans.binary_search(c).expect("listed")].clone()).collect()
                } else {
                    let ks = p.chunks(n);
                    if ks.len() <= LOOKUP_CHUNKS_MAX {
                        self.hold(&r.channels, &ks, false)
                    } else {
                        r.channels.iter().map(|_| Held::default()).collect()
                    }
                };
                self.views_planned(r, p, l, precision, &held)
            })
            .collect()
    }

    /// How a request is answered: its samples, whether summaries pay, whether to read level 4 through first.
    fn plan(&self, r: &Request, l: Option<&Loaded>, precision: Precision) -> Plan {
        let vp = r.vp;
        let (lo, hi) = (self.rec.lower_bound(vp.t0), self.rec.lower_bound(vp.t1));
        let in_view = hi.saturating_sub(lo);
        // a mask can't be answered from summaries: every sample in view is read
        let coarse_pays =
            self.mask.read().unwrap().is_empty() && in_view / vp.width_px.max(1) as usize >= MIN_BUCKET_SAMPLES;
        // An exact view reads level 4 at every pixel edge. A cold page fault costs about as much as reading 128 KB
        // in sequence, so when edges are closer than about 1024 summaries (88 KB) the range is read through first.
        // Only when cold: from memory, reading through costs more than it saves.
        let warm = precision == Precision::Exact
            && coarse_pays
            && in_view / vp.width_px.max(1) as usize <= 1024 * 256
            && l.zip(r.channels.first()).and_then(|(l, &c)| l.cold(c, lo, hi.min(l.done))) == Some(true);
        Plan { lo, hi, coarse_pays, warm }
    }

    fn views_planned(
        &self,
        r: &Request,
        p: &Plan,
        l: Option<&Loaded>,
        precision: Precision,
        held: &[Held],
    ) -> Vec<View> {
        let (vp, lo, hi) = (r.vp, p.lo, p.hi);
        let built_until =
            l.filter(|l| !l.complete).map(|l| if l.done == 0 { i64::MIN } else { self.rec.time_at(l.done - 1) });
        // one break list per distinct factor
        let factor = |i: usize| r.gap_factors.get(i).copied().unwrap_or(DEFAULT_FACTOR).max(MIN_FACTOR);
        let mut breaks: Vec<(f64, Vec<usize>)> = Vec::new();
        for i in 0..r.channels.len() {
            if !breaks.iter().any(|(f, _)| *f == factor(i)) {
                breaks.push((factor(i), l.map(|l| l.gaps.breaks(factor(i))).unwrap_or_default()));
            }
        }
        let breaks_of = |i: usize| &breaks.iter().find(|(f, _)| *f == factor(i)).expect("listed").1;
        if p.warm {
            // every pixel edge reads raw samples next to it: fetch them for all channels at once
            let bw = vp.bucket_ns();
            let (b0, b1) = (vp.t0.div_euclid(bw), (vp.t1 - 1).div_euclid(bw) + 1);
            let edges: Vec<_> = (b0..=b1)
                .map(|b| self.rec.lower_bound(b.saturating_mul(bw)))
                .map(|i| i.saturating_sub(256)..i + 256)
                .collect();
            self.rec.prefetch(&edges);
        }
        r.channels
            .par_iter()
            .enumerate()
            .zip(held)
            .map(|((i, &channel), held)| {
                if p.warm
                    && let Some(l) = l
                {
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
                let masked = self.mask.read().unwrap().clone();
                if !masked.is_empty() {
                    pyr = None;
                }
                let coarse = precision == Precision::Coarse && p.coarse_pays && pyr.is_some();
                let breaks = breaks_of(i);
                let trace = match &pyr {
                    Some(py) if coarse => self.rec.view_coarse(channel, vp, py, breaks, held),
                    None if skip_unread => {
                        Trace { kind: TraceKind::Raw, bucket_ns: vp.bucket_ns(), t: vec![], y: vec![] }
                    }
                    _ if !masked.is_empty() => self.rec.view_masked(channel, vp, &masked, breaks, held),
                    _ => self.rec.view(channel, vp, pyr.as_ref(), breaks, held),
                };
                let (trace, dashed) =
                    if r.connect_gaps { tlmbench_core::connect_gaps(&trace) } else { (trace, Vec::new()) };
                View { channel, trace, dashed, from_pyramid: pyr.is_some(), coarse, built_until }
            })
            .collect()
    }

    /// Exact statistics of each channel's samples in `[t0, t1)`; None until the pyramid is built (exact answers wait
    /// for it).
    pub fn stats(&self, channels: &[usize], t0: Timestamp, t1: Timestamp) -> Option<Vec<Stats>> {
        let g = self.loaded.read().unwrap();
        let l = g.as_ref().filter(|l| l.complete)?;
        let (a, b) = (self.rec.lower_bound(t0), self.rec.lower_bound(t1));
        if a >= b {
            return Some(
                channels
                    .iter()
                    .map(|_| Stats { count: 0, min: None, max: None, mean: f64::NAN, std: f64::NAN })
                    .collect(),
            );
        }
        // the ragged edges are read raw
        let mut ks = vec![chunk_of(a), chunk_of(b - 1)];
        ks.dedup();
        let held = self.hold(channels, &ks, true);
        let mask = self.mask.read().unwrap().clone();
        let stats = |c: usize, py: &Pyramid, held: &Held| {
            if mask.is_empty() {
                self.rec.stats(c, py, a, b, held)
            } else {
                self.rec.stats_masked(c, py, a, b, &mask, held)
            }
        };
        channels.iter().zip(&held).map(|(&c, held)| Some(stats(c, &l.pyramid(c)?, held))).collect()
    }

    /// Each channel's sample nearest `t` (the earlier one on ties), as (index, time, shown value). Channels share
    /// their times, so it is the same sample for all of them.
    pub fn values_at(&self, channels: &[usize], t: Timestamp) -> Option<(usize, Timestamp, Vec<f64>)> {
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
        let held = self.hold(channels, &[chunk_of(pick)], true);
        let hidden = self.mask.read().unwrap().hides(pick);
        let values = channels
            .iter()
            .zip(&held)
            .map(|(&c, held)| if hidden { f64::NAN } else { self.rec.value_at(c, pick, held) })
            .collect();
        Some((pick, self.rec.time_at(pick), values))
    }

    /// The rows with times in `[t0, t1)` that pass `filters` (every one) in `sort` order (see
    /// [`tlmbench_core::table::select`]): the index of the span's first sample, and the row ids as offsets from it.
    /// `progress(done)` goes 0..=1 and returns false to cancel (the result is then `None`).
    pub fn select_rows(
        &self,
        (t0, t1): (Timestamp, Timestamp),
        filters: Vec<(Col, Filter)>,
        sort: Vec<SortKey>,
        progress: &(dyn Fn(f64) -> bool + Sync),
    ) -> Option<(usize, Rows)> {
        let first = self.rec.lower_bound(t0);
        let count = self.rec.lower_bound(t1).max(first) - first;
        let read = |col: Col, r: std::ops::Range<usize>| match col {
            Col::Time => self.rec.read_times(r.start, r.end).into_iter().map(|t| t as f64).collect(),
            Col::Channel(c) => {
                let (scale, offset) = self.rec.scale_offset(c);
                self.rec.read(c, r.start, r.end).to_f64(scale, offset)
            }
        };
        let q = Query { first, count, filters, sort };
        table::select(&q, &read, progress).map(|rows| (first, rows))
    }

    /// Sets what `owner` (a table with Apply to charts) leaves out of every view, statistic and value; `None` takes
    /// it back. Samples any owner leaves out are missing.
    pub fn set_mask(&self, owner: u32, mask: Option<Mask>) {
        let mut masks = self.masks.lock().unwrap();
        match mask {
            Some(m) if !m.is_empty() => masks.insert(owner, m),
            _ => masks.remove(&owner),
        };
        let all = masks.values().fold(Mask::default(), |acc, m| acc.union(m));
        *self.mask.write().unwrap() = Arc::new(all);
    }

    /// Everything the owners leave out together.
    pub fn mask(&self) -> Arc<Mask> {
        self.mask.read().unwrap().clone()
    }

    /// The rows with times in `[t0, t1)` that pass `filters`, as a mask of the others in the file (for Apply to
    /// charts); `None` when cancelled. Rows outside the span stay.
    pub fn filter_mask(
        &self,
        (t0, t1): (Timestamp, Timestamp),
        filters: Vec<(Col, Filter)>,
        progress: &(dyn Fn(f64) -> bool + Sync),
    ) -> Option<Mask> {
        let (first, rows) = self.select_rows((t0, t1), filters, Vec::new(), progress)?;
        let count = self.rec.lower_bound(t1).max(first) - first;
        Some(Mask::from_kept(first, count, &rows.ids))
    }

    /// The sample nearest `t` (see [`Opened::values_at`]), as (index, time, shown value).
    pub fn nearest(&self, channel: usize, t: Timestamp) -> Option<(usize, Timestamp, f64)> {
        self.values_at(&[channel], t).map(|(i, t, v)| (i, t, v[0]))
    }

    /// Chunks `ks` (sorted) of each of `channels`, held for a query; empty for channels the decoder stores
    /// contiguously. Missing chunks are decoded when `fill` (each block of records read once for every channel
    /// missing it) unless they wouldn't fit the cache's budget; otherwise they are read in place.
    fn hold(&self, channels: &[usize], ks: &[usize], fill: bool) -> Vec<Held> {
        let mut out: Vec<Held> = channels.iter().map(|_| Held::default()).collect();
        let (Some(&first), Some(&last)) = (ks.first(), ks.last()) else { return out };
        let chunked: Vec<usize> = (0..channels.len()).filter(|&i| self.rec.chunked(channels[i])).collect();
        if chunked.is_empty() {
            return out;
        }
        let cache = &self.store.chunks;
        let bytes: u64 = chunked
            .iter()
            .map(|&i| self.rec.info().channels[channels[i]].dtype[1..].parse::<u64>().map_or(8, |b| b / 8))
            .sum::<u64>()
            * (ks.len() * CHUNK_SAMPLES) as u64;
        let fill = fill && bytes <= cache.budget() / 2;
        let _one = fill.then(|| self.filling.lock().unwrap());
        for &i in &chunked {
            out[i] = Held { first, chunks: vec![None; last - first + 1] };
            for &k in ks {
                out[i].chunks[k - first] = cache.get((self.id, channels[i], k));
            }
        }
        if !fill {
            return out;
        }
        // runs of chunks some channel is missing, and the channels missing any of each run
        let missing: Vec<usize> =
            ks.iter().copied().filter(|&k| chunked.iter().any(|&i| out[i].chunks[k - first].is_none())).collect();
        for run in missing.chunk_by(|a, b| b == &(a + 1)) {
            let (r0, r1) = (run[0], run[run.len() - 1] + 1);
            let want: Vec<usize> =
                chunked.iter().copied().filter(|&i| (r0..r1).any(|k| out[i].chunks[k - first].is_none())).collect();
            let chans: Vec<usize> = want.iter().map(|&i| channels[i]).collect();
            for (&i, decoded) in want.iter().zip(self.rec.decode_chunks(&chans, r0..r1)) {
                for (k, chunk) in (r0..r1).zip(decoded) {
                    let slot = &mut out[i].chunks[k - first];
                    if slot.is_none() {
                        *slot = Some(cache.insert((self.id, channels[i], k), chunk));
                    }
                }
            }
        }
        out
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
        if !self.live {
            self.store.chunks.forget(self.id);
        }
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

/// A view from the pyramid looks for decoded chunks only when it spans at most this many (beyond, few of its edge
/// samples would be in them).
const LOOKUP_CHUNKS_MAX: usize = 256;

/// The most unread samples a view of a file being built reads raw: past this the unread part stays empty.
const UNREAD_RAW_MAX: usize = 1 << 22;

/// How long a process waits between looks at another process's build.
const FOLLOW_EVERY: Duration = Duration::from_millis(200);
