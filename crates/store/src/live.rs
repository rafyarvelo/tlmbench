//! Recordings that grow: a live session's files, read while their sources append to them.
//!
//! [`Live`] follows one such file. Each [`Live::refresh`] maps the file again at its new length and folds the new
//! samples into a pyramid that grows: every whole 256 samples become a level-4 summary, every four summaries of a
//! level one of the next, each level appended to a file of its own under the cache's `live/` folder (dropped with
//! the `Live`: the recording, reopened later, gets an ordinary cache entry). Queries go to a snapshot,
//! [`Live::opened`], an [`Opened`] like any other: the same view batches, values and stats. A snapshot uses levels
//! up to 9 (4^9 samples a summary), summarized up to its last whole summary; the newest samples past that, at most
//! 4^9 of them, are read raw. Snapshots share one chunk cache id: chunks before the old end don't change.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use memmap2::Mmap;
use tlmbench_core::chunks::chunk_of;
use tlmbench_core::gaps::GapIndex;
use tlmbench_core::pyramid::{MIN_LEVEL, Summary, span};

use crate::pyr::Loaded;
use crate::trigger::{Spec, Status, Trigger};
use crate::{Error, NEXT_ID, Opened, Result, Store, hold};

/// The coarsest level a snapshot uses: past its last whole summary, samples are read raw.
const TOP_LEVEL: u32 = 9;
const LEVELS: usize = (TOP_LEVEL - MIN_LEVEL) as usize + 1;

/// A recording that grows, followed by [`Live::refresh`].
#[derive(Debug)]
pub struct Live {
    path: PathBuf,
    store: Store,
    /// The chunk cache id every snapshot shares.
    id: u64,
    /// `live/<key>` in the cache: the pyramid's level files.
    dir: PathBuf,
    key: String,
    current: RwLock<Arc<Opened>>,
    grow: Mutex<Grow>,
    trigger: Mutex<Option<Trigger>>,
}

/// What a refresh continues from.
#[derive(Debug)]
struct Grow {
    /// Samples folded into level 4 (a multiple of 256), and the samples of the last snapshot.
    folded: usize,
    samples: usize,
    /// `files[channel][level]`, and how many summaries each holds.
    files: Vec<Vec<File>>,
    counts: Vec<Vec<usize>>,
    /// Per channel and level, the summaries not yet merged into the next level (fewer than four).
    pending: Vec<Vec<Vec<Summary>>>,
    gaps: GapIndex,
}

impl Store {
    /// Opens a recording that is still being written, to follow with [`Live::refresh`]. Nothing is written beside
    /// it: its growing pyramid lives in the cache until the `Live` is dropped.
    pub fn open_live(&self, path: &Path) -> Result<Live> {
        let key = format!("live-{}", &blake3::hash(path.to_string_lossy().as_bytes()).to_hex()[..16]);
        let dir = self.root.join("live").join(format!("{key}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(Error::io(&dir))?;
        }
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let first = self.live_snapshot(path, &key, id, None)?;
        let channels = first.recording().info().channels.len();
        let mut files = Vec::with_capacity(channels);
        for c in 0..channels {
            let mut levels = Vec::with_capacity(LEVELS);
            for k in 0..LEVELS {
                let p = dir.join(format!("c{c}-l{}.pyr", k as u32 + MIN_LEVEL));
                levels.push(OpenOptions::new().create(true).read(true).append(true).open(&p).map_err(Error::io(&p))?);
            }
            files.push(levels);
        }
        let grow = Grow {
            folded: 0,
            samples: 0,
            files,
            counts: vec![vec![0; LEVELS]; channels],
            pending: vec![vec![Vec::new(); LEVELS]; channels],
            gaps: GapIndex::default(),
        };
        let live = Live {
            path: path.to_owned(),
            store: self.clone(),
            id,
            dir,
            key,
            current: RwLock::new(Arc::new(first)),
            grow: Mutex::new(grow),
            trigger: Mutex::new(None),
        };
        live.refresh()?;
        Ok(live)
    }

    /// An [`Opened`] over `path` at its current length, with the growing pyramid `loaded`.
    fn live_snapshot(&self, path: &Path, key: &str, id: u64, loaded: Option<Loaded>) -> Result<Opened> {
        let opts = tlmbench_decode::OpenOptions {
            descriptor: None,
            part: None,
            profile: None,
            cache_dir: Some(self.root.clone()),
            adjust: Vec::new(),
        };
        let rec = tlmbench_decode::open(path, &opts)?;
        Ok(Opened {
            id,
            live: true,
            filling: Mutex::new(()),
            rec,
            joined: None,
            store: self.clone(),
            fingerprint: key.to_owned(),
            key: key.to_owned(),
            dir: self.root.join("live"),
            loaded: RwLock::new(loaded),
            cancel: AtomicBool::new(false),
            _in_use: hold(&self.root, key)?,
            building: Mutex::new(()),
            masks: Mutex::default(),
            mask: RwLock::default(),
            runs: Mutex::default(),
        })
    }
}

impl Live {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The recording as of the last refresh, for queries.
    pub fn opened(&self) -> Arc<Opened> {
        self.current.read().unwrap().clone()
    }

    /// Reads the file at its current length and folds the new samples into the pyramid; returns whether it grew.
    pub fn refresh(&self) -> Result<bool> {
        let mut g = self.grow.lock().unwrap();
        let snap = self.store.live_snapshot(&self.path, &self.key, self.id, None)?;
        let rec = snap.recording();
        let n = rec.info().samples;
        if n == g.samples && g.samples > 0 {
            return Ok(false);
        }
        // whole 256-sample blocks become level-4 summaries, and each four of a level one of the next
        let w = span(MIN_LEVEL);
        let upto = n / w * w;
        if upto > g.folded {
            let level4 = rec.summarize_all(g.folded, upto);
            for (c, sums) in level4.into_iter().enumerate() {
                for s in sums {
                    g.append(c, 0, s).map_err(Error::io(&self.dir))?;
                }
            }
            g.folded = upto;
        }
        let from = g.samples;
        rec.extend_gap_index(&mut g.gaps, from);
        // the newest chunk was decoded while it was still filling
        if from > 0 {
            self.store.chunks.forget_from(self.id, chunk_of(from - 1));
        }
        g.samples = n;
        let loaded = g.snapshot().map_err(Error::io(&self.dir))?;
        *snap.loaded.write().unwrap() = Some(loaded);
        let snap = Arc::new(snap);
        *self.current.write().unwrap() = snap.clone();
        drop(g);
        // the trigger looks at what just arrived; each crossing is an event of the recording
        let events = self.trigger.lock().unwrap().as_mut().map(|t| t.poll(&self.store, &snap)).unwrap_or_default();
        for e in events {
            crate::events::save(self.store.root(), &self.key, e)?;
        }
        Ok(true)
    }

    /// Arms a trigger on samples that arrive from now on, replacing any other. `recipes` are the library, for a
    /// derived channel named by its recipe.
    pub fn arm_trigger(&self, spec: Spec, recipes: &[tlmbench_expr::Recipe]) -> std::result::Result<(), String> {
        let t = Trigger::arm(spec, &self.opened(), recipes)?;
        *self.trigger.lock().unwrap() = Some(t);
        Ok(())
    }

    /// Stops the trigger (its count and last crossing stay in [`Live::trigger_status`] until it is armed again).
    pub fn disarm_trigger(&self) {
        if let Some(t) = self.trigger.lock().unwrap().as_mut() {
            t.disarm();
        }
    }

    pub fn trigger_status(&self) -> Status {
        self.trigger.lock().unwrap().as_ref().map(|t| t.status(&self.opened())).unwrap_or_default()
    }
}

impl Grow {
    /// Appends `s` at level `k` of channel `c`, and merges up as groups of four complete.
    fn append(&mut self, c: usize, k: usize, s: Summary) -> std::io::Result<()> {
        self.files[c][k].write_all(bytemuck::bytes_of(&s))?;
        self.counts[c][k] += 1;
        if k + 1 < LEVELS {
            let p = &mut self.pending[c][k];
            p.push(s);
            if p.len() == 4 {
                let merged = p[1..].iter().fold(p[0], |a, b| a.merge(b));
                p.clear();
                self.append(c, k + 1, merged)?;
            }
        }
        Ok(())
    }

    /// The pyramid as it stands: levels up to the coarsest with a summary (at most [`TOP_LEVEL`]), each used up to
    /// that level's last whole summary.
    fn snapshot(&self) -> std::io::Result<Loaded> {
        let counts = &self.counts[0];
        let Some(top) = counts.iter().rposition(|&n| n > 0) else {
            return Ok(Loaded::growing(Vec::new(), Vec::new(), 0, self.gaps.clone()));
        };
        let done = counts[top] * span(MIN_LEVEL + top as u32);
        let lens: Vec<usize> = (0..=top).map(|k| done / span(MIN_LEVEL + k as u32)).collect();
        let maps = self
            .files
            .iter()
            // SAFETY: the level files are only appended to, by this `Live`, and readers use the first `lens`
            // summaries of each, written before the map was made.
            .map(|levels| levels[..=top].iter().map(|f| unsafe { Mmap::map(f) }).collect::<std::io::Result<Vec<_>>>())
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(Loaded::growing(maps, lens, done, self.gaps.clone()))
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.store.chunks.forget(self.id);
        let _ = fs::remove_dir_all(&self.dir);
    }
}
