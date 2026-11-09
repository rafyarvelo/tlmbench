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

fn write_f64(w: &mut (impl Write + ?Sized), v: &[f64]) -> std::io::Result<()> {
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

/// Writes `bytes` at `offset`.
fn write_at(f: &File, bytes: &[u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::write_all_at(f, bytes, offset)
    }
    #[cfg(windows)]
    {
        let (mut done, mut at) = (0, offset);
        while done < bytes.len() {
            let n = std::os::windows::fs::FileExt::seek_write(f, &bytes[done..], at)?;
            done += n;
            at += n as u64;
        }
        Ok(())
    }
}

fn write_channel(env: &RecEnv, r: &Resolved, rows: usize, path: &Path) -> std::result::Result<(), String> {
    let name = &r.recipe.name;
    let io = |e: std::io::Error| format!("{name}: {e}");
    let fail = |e: tlmbench_expr::ExprError| format!("{name}: {}", e.msg);
    let flat = |w: &mut dyn Write, x: f64| -> std::result::Result<(), String> {
        let block = vec![x; CHUNK.min(rows.max(1))];
        let mut left = rows;
        while left > 0 {
            let n = left.min(block.len());
            write_f64(w, &block[..n]).map_err(io)?;
            left -= n;
        }
        w.flush().map_err(io)
    };
    if tlmbench_expr::is_stateless(&r.expr) && rows > 0 {
        // every chunk on its own, in parallel, each written where it belongs
        let first = tlmbench_expr::eval_range(&r.expr, env, 0..CHUNK.min(rows)).map_err(fail)?;
        if let Some(x) = first.number() {
            return flat(&mut BufWriter::new(File::create(path).map_err(io)?), x);
        }
        let file = File::create(path).map_err(io)?;
        file.set_len(rows as u64 * 8).map_err(io)?;
        let starts: Vec<usize> = (0..rows).step_by(CHUNK).collect();
        return starts.par_iter().try_for_each(|&a| {
            let v = if a == 0 {
                first.clone()
            } else {
                tlmbench_expr::eval_range(&r.expr, env, a..(a + CHUNK).min(rows)).map_err(fail)?
            };
            let s = v.series().ok_or_else(|| format!("{name}: computed a number where samples were expected"))?;
            if s.len() != (a + CHUNK).min(rows) - a {
                return Err(format!("{name}: computed {} samples for {}", s.len(), (a + CHUNK).min(rows) - a));
            }
            write_at(&file, bytemuck::cast_slice(&s.v), a as u64 * 8).map_err(io)
        });
    }
    let mut w = BufWriter::new(File::create(path).map_err(io)?);
    let mut wrote = 0usize;
    let mut failed = None;
    let number = tlmbench_expr::run(&r.expr, env, CHUNK, &mut |_, s| {
        if failed.is_none() {
            failed = write_f64(&mut w, &s.v).err();
            wrote += s.len();
        }
    })
    .map_err(fail)?;
    if let Some(e) = failed {
        return Err(io(e));
    }
    // a one-number recipe is a flat line
    if let Some(x) = number {
        flat(&mut w, x)
    } else if wrote != rows {
        Err(format!("{name}: computed {wrote} samples for {rows}"))
    } else {
        w.flush().map_err(io)
    }
}

impl Opened {
    /// The frequency content of `channel` over `t0..t1` (the Spectrum view): a window of up to `max_points` samples
    /// (a power of two) centred in the span, taken at the recording's sample rate, holding the last sample where
    /// times are irregular. `None` when the span holds fewer than 16 samples or the window is mostly missing.
    pub fn spectrum(
        &self,
        channel: usize,
        t0: tlmbench_core::Timestamp,
        t1: tlmbench_core::Timestamp,
        max_points: usize,
    ) -> Option<tlmbench_expr::Spectrum> {
        let rec = self.recording();
        let info = rec.info();
        let (first, last) = (info.t_first?, info.t_last?);
        let rate = RecEnv::new(rec).hz()?;
        let available = ((t1 - t0) as f64 / 1e9 * rate).floor();
        let n = tlmbench_expr::fft::points(
            if available.is_finite() && available > 0.0 { available as usize } else { 0 },
            max_points,
        )?;
        let start = t0 as f64 + ((t1 - t0) as f64 - n as f64 / rate * 1e9) / 2.0;
        let idx: Vec<Option<usize>> = (0..n)
            .map(|i| {
                let t = (start + i as f64 / rate * 1e9).round() as tlmbench_core::Timestamp;
                (first..=last).contains(&t).then(|| rec.lower_bound(t.saturating_add(1)).clamp(1, info.samples) - 1)
            })
            .collect();
        let (lo, hi) = idx.iter().flatten().fold((usize::MAX, 0), |(l, h), &i| (l.min(i), h.max(i)));
        if lo > hi {
            return None;
        }
        let (scale, offset) = rec.scale_offset(channel);
        let v = rec.read(channel, lo, hi + 1).to_f64(scale, offset);
        let x: Vec<f64> = idx.iter().map(|i| i.map_or(f64::NAN, |i| v[i - lo])).collect();
        tlmbench_expr::spectrum(&x, rate)
    }
}
