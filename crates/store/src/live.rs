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
            .map(|levels| {
                levels[..=top]
                    .iter()
                    .map(|f| unsafe { Mmap::map(f) })
                    .collect::<std::io::Result<Vec<_>>>()
                    .map(Arc::new)
            })
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

/// A live recording with derived channels: the recipes are computed over each batch of new samples (after the samples
/// their windows need) and appended to a recording of their own, itself followed as it grows; the two are joined, so
/// the derived channels come after the recording's, as for a file. Recipes whose state can't be rebuilt from a bounded
/// look-back (running totals, filters that start at the beginning) can't run live and are reported as skipped.
#[derive(Debug)]
pub struct LiveDerived {
    store: Store,
    base: Arc<Live>,
    derived: Live,
    file: Mutex<File>,
    /// Samples of the base already computed.
    done: Mutex<usize>,
    recipes: Vec<tlmbench_expr::Resolved>,
    joined: RwLock<Arc<Opened>>,
    dir: PathBuf,
}

/// What opening derived channels on a live recording produced.
#[derive(Debug)]
pub struct LiveDerivedOpened {
    pub live: LiveDerived,
    /// The recipes computed, in channel order.
    pub names: Vec<String>,
    /// The recipes that don't apply (a channel they need is missing, a mistake in them, can't run live), and why.
    pub skipped: Vec<(String, String)>,
}

impl Store {
    /// Computes `recipes` over `base` as it grows. See [`LiveDerived`].
    pub fn open_live_derived(&self, base: Arc<Live>, recipes: &[tlmbench_expr::Recipe]) -> Result<LiveDerivedOpened> {
        use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, TimeUnit};
        let snap = base.opened();
        let has: std::collections::HashSet<&str> =
            snap.recording().info().channels.iter().map(|c| c.name.as_str()).collect();
        let (resolved, failed) = tlmbench_expr::recipe::resolve(recipes, &|n| has.contains(n));
        let mut skipped: Vec<(String, String)> = failed.into_iter().map(|(n, e)| (n, e.msg)).collect();
        let hz = crate::derived::RecEnv::new(snap.recording());
        let hz = tlmbench_expr::Env::hz(&hz).unwrap_or(1.0);
        let mut ok = Vec::new();
        for r in resolved {
            if tlmbench_expr::is_stateless(&r.expr) || tlmbench_expr::warm_rows_at(&r.expr, hz).is_some() {
                ok.push(r);
            } else {
                skipped.push((
                    r.recipe.name.clone(),
                    "It needs the whole recording to compute, so it can't follow a live source".into(),
                ));
            }
        }
        let dir = self.root.join("live").join(format!(
            "derived-{}-{}",
            &base.key[5..],
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let field = |name: &str, dtype, offset, unit: Option<String>| Field {
            name: name.to_owned(),
            dtype,
            offset,
            unit,
            scale: 1.0,
            value_offset: 0.0,
            bits: None,
        };
        let mut fields = vec![field("time", DType::I64, 0, None)];
        for (k, r) in ok.iter().enumerate() {
            fields.push(field(&r.recipe.name, DType::F64, 8 * (k as u64 + 1), r.recipe.unit.clone()));
        }
        let d = Descriptor {
            sequence: None,
            files: Vec::new(),
            packet_types: Vec::new(),
            packets: None,
            schema: tlmbench_decode::descriptor::SCHEMA,
            name: Some("derived".into()),
            endian: Endian::Little,
            header_size: 0,
            record_size: 8 * fields.len() as u64,
            time: tlmbench_decode::descriptor::TimeSpec::Field { field: "time".into(), unit: TimeUnit::Ns, t0_ns: 0 },
            fields,
        };
        let path = dir.join("derived.tlm");
        let mut file = OpenOptions::new().create_new(true).append(true).open(&path).map_err(Error::io(&path))?;
        tlmbench_decode::reference::write_header(&mut file, &d).map_err(Error::io(&path))?;
        let names: Vec<String> = ok.iter().map(|r| r.recipe.name.clone()).collect();
        let this_live = LiveDerived {
            store: self.clone(),
            base,
            // the placeholder is replaced below, once the first rows are written
            derived: self.open_live(&path)?,
            file: Mutex::new(file),
            done: Mutex::new(0),
            recipes: ok,
            joined: RwLock::new(snap),
            dir,
        };
        this_live.refresh()?;
        Ok(LiveDerivedOpened { live: this_live, names, skipped })
    }
}

impl LiveDerived {
    /// Follows the base, computes the recipes over its new samples, and joins what is there. Returns whether it grew.
    pub fn refresh(&self) -> Result<bool> {
        let grew = self.base.refresh()?;
        let snap = self.base.opened();
        let rec = snap.recording();
        let n = rec.info().samples;
        let mut done = self.done.lock().unwrap();
        if n > *done && !self.recipes.is_empty() {
            let shared = self.store.expr_env.read().unwrap().clone();
            let env = crate::derived::RecEnv::new(rec).with_labels(shared.labels).with_limits(shared.limits);
            let hz = tlmbench_expr::Env::hz(&env).unwrap_or(1.0);
            let (a, rows) = (*done, n - *done);
            let mut cols: Vec<Vec<f64>> = Vec::with_capacity(self.recipes.len());
            for r in &self.recipes {
                let v = if tlmbench_expr::is_stateless(&r.expr) {
                    tlmbench_expr::eval_range(&r.expr, &env, a..n).map(Some)
                } else {
                    let warm = tlmbench_expr::warm_rows_at(&r.expr, hz).unwrap_or(0);
                    tlmbench_expr::eval_warm(&r.expr, &env, a..n, warm)
                };
                let col = match v {
                    Ok(Some(tlmbench_expr::Val::Ser(s))) if s.len() == rows => s.v,
                    Ok(Some(v)) if v.number().is_some() => vec![v.number().unwrap_or(f64::NAN); rows],
                    // a window that fell short, or a mistake: these samples are missing
                    _ => vec![f64::NAN; rows],
                };
                cols.push(col);
            }
            let times = rec.read_times(a, n);
            let mut bytes = Vec::with_capacity(rows * 8 * (cols.len() + 1));
            for (i, t) in times.iter().enumerate() {
                bytes.extend_from_slice(&t.to_le_bytes());
                for c in &cols {
                    bytes.extend_from_slice(&c[i].to_le_bytes());
                }
            }
            let mut f = self.file.lock().unwrap();
            f.write_all(&bytes).map_err(Error::io(&self.dir))?;
            f.flush().map_err(Error::io(&self.dir))?;
            *done = n;
        }
        if *done == n {
            self.derived.refresh()?;
            let d = self.derived.opened();
            if d.recording().info().samples == snap.recording().info().samples {
                *self.joined.write().unwrap() = Arc::new(self.store.join(snap, d)?);
            }
        }
        Ok(grew)
    }

    /// The recording with its derived channels, as of the last refresh.
    pub fn opened(&self) -> Arc<Opened> {
        self.joined.read().unwrap().clone()
    }

    /// The base recording being followed.
    pub fn base(&self) -> &Arc<Live> {
        &self.base
    }
}

impl Drop for LiveDerived {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
