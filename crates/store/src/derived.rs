//! Derived channels: recipes evaluated over an opened recording, chunk by chunk, into a cache entry of their own
//! (`derived/<key>`, keyed by the recording and the recipes), which opens like any recording and gets its own pyramid.
//! Nothing is written beside the source.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::path::Path;

use rayon::prelude::*;
use tlmbench_core::{Recording, TimeSource};
use tlmbench_decode::cache::{DerivedChannel, DerivedSpec, DerivedTime};
use tlmbench_expr::recipe::{self, Recipe, Resolved};
use tlmbench_expr::{CHUNK, Env, ExprError, Grid, Series};

use crate::{Opened, Result, Store};

/// What opening derived channels produced.
#[derive(Debug)]
pub struct Derived {
    /// The computed channels, as a recording of their own (`None` when no recipe applied). Its channels are in the
    /// order of `names`, and it has the same sample times as the recording they came from.
    pub opened: Option<Opened>,
    /// The recipes that were computed, in channel order.
    pub names: Vec<String>,
    /// The recipes that don't apply to this recording, and why (a channel they need is missing, a mistake in them).
    pub skipped: Vec<(String, ExprError)>,
}

/// A recording's channels as an expression environment.
#[derive(Debug)]
pub struct RecEnv<'a> {
    rec: &'a dyn Recording,
    idx: HashMap<&'a str, usize>,
    t_first: i64,
    labels: HashMap<(String, String), f64>,
    limits: HashMap<(String, bool), (Option<f64>, Option<f64>)>,
}

impl<'a> RecEnv<'a> {
    pub fn new(rec: &'a dyn Recording) -> Self {
        let info = rec.info();
        let idx = info.channels.iter().enumerate().map(|(i, c)| (c.name.as_str(), i)).collect();
        Self { rec, idx, t_first: info.t_first.unwrap_or(0), labels: HashMap::new(), limits: HashMap::new() }
    }

    /// Value labels (`channel`, `text`) → stored number, for `state == 'flight'`.
    pub fn with_labels(mut self, labels: HashMap<(String, String), f64>) -> Self {
        self.labels = labels;
        self
    }

    /// Warn and alarm limits by (channel, alarm?), for `past_warn` and `past_alarm`.
    pub fn with_limits(mut self, limits: HashMap<(String, bool), (Option<f64>, Option<f64>)>) -> Self {
        self.limits = limits;
        self
    }
}

impl Env for RecEnv<'_> {
    fn series(&self, name: &str) -> Option<Series> {
        self.read(name, 0..self.rec.info().samples)
    }

    fn read(&self, name: &str, r: Range<usize>) -> Option<Series> {
        let c = *self.idx.get(name)?;
        let info = self.rec.info();
        let (a, b) = (r.start.min(info.samples), r.end.min(info.samples));
        let (scale, offset) = self.rec.scale_offset(c);
        let v = self.rec.read(c, a, b).to_f64(scale, offset);
        let grid = match &info.time {
            TimeSource::Rate { hz } => Grid::Rate { hz: *hz, t0: 0.0, start: a },
            TimeSource::Index => Grid::Rate { hz: 1.0, t0: 0.0, start: a },
            TimeSource::Column { .. } => {
                let t = self.rec.read_times(a, b);
                Grid::Stamps(t.iter().map(|&t| (t - self.t_first) as f64 / 1e9).collect::<Vec<_>>().into())
            }
        };
        Some(Series::new(v, grid))
    }

    fn samples(&self) -> Option<usize> {
        Some(self.rec.info().samples)
    }

    fn hz(&self) -> Option<f64> {
        let info = self.rec.info();
        Some(match (&info.time, info.t_first, info.t_last) {
            (TimeSource::Rate { hz }, ..) => *hz,
            (TimeSource::Column { .. }, Some(a), Some(b)) if b > a && info.samples > 1 => {
                (info.samples - 1) as f64 / ((b - a) as f64 / 1e9)
            }
            _ => 1.0,
        })
    }

    fn label(&self, channel: &str, text: &str) -> Option<f64> {
        self.labels.get(&(channel.to_string(), text.to_string())).copied()
    }

    fn limits(&self, channel: &str, alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        self.limits.get(&(channel.to_string(), alarm)).copied()
    }
}

fn write_f64(w: &mut impl Write, v: &[f64]) -> std::io::Result<()> {
    w.write_all(bytemuck::cast_slice(v))
}

impl Store {
    /// Computes `recipes` over `base` (those whose channels it has) into a recording of their own. The result is cached
    /// by the recording and the recipes, so reopening costs nothing; change a recipe and only its entry is rebuilt.
    pub fn open_derived(&self, base: &Opened, recipes: &[Recipe]) -> Result<Derived> {
        self.open_derived_with(base, recipes, |e| e)
    }

    /// [`Store::open_derived`] with the environment adjusted (value labels and limits for the recipes that use them).
    pub fn open_derived_with<'a>(
        &self,
        base: &'a Opened,
        recipes: &[Recipe],
        env: impl FnOnce(RecEnv<'a>) -> RecEnv<'a>,
    ) -> Result<Derived> {
        let rec = base.recording();
        let info = rec.info();
        let has: std::collections::HashSet<&str> = info.channels.iter().map(|c| c.name.as_str()).collect();
        let (ok, skipped) = recipe::resolve(recipes, &|n| has.contains(n));
        if ok.is_empty() {
            return Ok(Derived { opened: None, names: Vec::new(), skipped });
        }
        let env = env(RecEnv::new(rec));
        let mut h = blake3::Hasher::new();
        h.update(base.fingerprint().as_bytes());
        h.update(&rec.layout_id());
        for r in &ok {
            h.update(r.key().as_bytes());
        }
        let key = h.finalize().to_hex()[..32].to_owned();
        let spec = DerivedSpec {
            rows: info.samples,
            time: match &info.time {
                TimeSource::Column { name } => DerivedTime::Column(name.clone()),
                TimeSource::Rate { hz } => DerivedTime::Rate { t0: info.t_first.unwrap_or(0), hz: *hz },
                TimeSource::Index => DerivedTime::Index,
            },
            t_first: info.t_first,
            t_last: info.t_last,
            channels: ok
                .iter()
                .map(|r| DerivedChannel {
                    name: r.recipe.name.clone(),
                    unit: r.recipe.unit.clone(),
                    category: r.recipe.category.clone(),
                })
                .collect(),
        };
        let rows = info.samples;
        let time_column = matches!(spec.time, DerivedTime::Column(_));
        let cached = tlmbench_decode::cache::open_derived(&info.path, self.root(), &key, &spec, |tmp| {
            if time_column {
                write_times(rec, tmp).map_err(|e| e.to_string())?;
            }
            ok.par_iter()
                .enumerate()
                .try_for_each(|(k, r)| write_channel(&env, r, rows, &tmp.join(format!("c{k}.f64"))))
        })?;
        let opened = self.wrap_keyed(Box::new(cached), key)?;
        Ok(Derived { opened: Some(opened), names: ok.iter().map(|r| r.recipe.name.clone()).collect(), skipped })
    }
}

fn write_times(rec: &dyn Recording, dir: &Path) -> std::io::Result<()> {
    let mut w = BufWriter::new(File::create(dir.join("t.i64"))?);
    let n = rec.info().samples;
    for a in (0..n).step_by(CHUNK) {
        w.write_all(bytemuck::cast_slice(&rec.read_times(a, (a + CHUNK).min(n))))?;
    }
    w.flush()
}

fn write_channel(env: &RecEnv, r: &Resolved, rows: usize, path: &Path) -> std::result::Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", r.recipe.name);
    let mut w = BufWriter::new(File::create(path).map_err(io)?);
    let mut wrote = 0usize;
    let mut failed = None;
    let number = tlmbench_expr::run(&r.expr, env, CHUNK, &mut |_, s| {
        if failed.is_none() {
            failed = write_f64(&mut w, &s.v).err();
            wrote += s.len();
        }
    })
    .map_err(|e| format!("{}: {}", r.recipe.name, e.msg))?;
    if let Some(e) = failed {
        return Err(io(e));
    }
    // a one-number recipe is a flat line
    if let Some(x) = number {
        let block = vec![x; CHUNK.min(rows.max(1))];
        let mut left = rows;
        while left > 0 {
            let n = left.min(block.len());
            write_f64(&mut w, &block[..n]).map_err(io)?;
            left -= n;
        }
    } else if wrote != rows {
        return Err(format!("{}: computed {wrote} samples for {rows}", r.recipe.name));
    }
    w.flush().map_err(io)
}
