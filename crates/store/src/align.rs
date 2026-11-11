//! Alignment methods: each takes a file against the primary and returns a time map (see
//! [`tlmbench_core::timemap`]) with how well it fits. The map takes the file's own clock to the shared timeline,
//! which is the primary's clock through the primary's own map.

use std::path::Path;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tlmbench_core::timemap::{self, Estimate, Grid, TimeMap};
use tlmbench_core::{Recording, Timestamp};

use crate::{Error, Opened, Result, events};

/// How a file is lined up with the primary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "lowercase")]
pub enum Method {
    /// Both start at the same moment.
    Starts,
    /// The recording clocks agree (files logged in one session): no shift.
    Clock,
    /// Starts aligned, then this many seconds later (negative: earlier).
    Manual { seconds: f64 },
    /// Matching moments in the two files, `(t_file, t_shared)` in nanoseconds; piecewise follows every point.
    Sync {
        pairs: Vec<(String, String)>,
        #[serde(default)]
        piecewise: bool,
    },
    /// The same named events in both files, paired in order.
    Events { name: Option<String> },
    /// A channel both files have, matched in windows to track drift.
    Xcorr {
        channel: Option<String>,
        #[serde(default)]
        windows: Option<usize>,
    },
    /// A time channel (a GPS time of week, in seconds) both files record.
    TimeSource { channel: Option<String> },
    /// An estimator plugin (see [`EstimatorPlugin`]) with its parameters, a JSON text the plugin understands.
    Plugin {
        name: String,
        #[serde(default)]
        params: String,
    },
}

/// A channel's samples as an estimator plugin sees them: times in nanoseconds on the clock the plugin is told, shown
/// values.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: String,
    pub t: Vec<Timestamp>,
    pub v: Vec<f64>,
}

/// What an estimator plugin returns.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginEstimate {
    /// `(t_file, t_shared)`, increasing in both.
    pub knots: Vec<(Timestamp, Timestamp)>,
    pub residuals: Vec<(Timestamp, f64)>,
    pub quality: f64,
    pub note: Option<String>,
}

/// Custom alignment code (a WASM component, see the `tlmbench-plugin` crate). The core decimates each channel to at
/// most [`PLUGIN_SAMPLES`] samples and hands over only the channels `needs` names; the reference's times are on the
/// shared timeline and the target's on its own clock. `hint` is the starts-aligned map.
pub trait EstimatorPlugin: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn needs(&self, params: &str) -> std::result::Result<Vec<String>, String>;
    fn estimate(
        &self,
        reference: &[Series],
        target: &[Series],
        hint: &TimeMap,
        params: &str,
    ) -> std::result::Result<PluginEstimate, String>;
}

/// The most samples of a channel an estimator plugin is given.
pub const PLUGIN_SAMPLES: usize = 1_000_000;

static ESTIMATORS: RwLock<Vec<Arc<dyn EstimatorPlugin>>> = RwLock::new(Vec::new());

/// Replaces the list of estimator plugins.
pub fn set_estimators(list: Vec<Arc<dyn EstimatorPlugin>>) {
    *ESTIMATORS.write().unwrap() = list;
}

pub fn estimators() -> Vec<Arc<dyn EstimatorPlugin>> {
    ESTIMATORS.read().unwrap().clone()
}

/// A map with how it was found.
#[derive(Clone, Debug, Serialize)]
pub struct Aligned {
    pub map: TimeMap,
    pub residuals: Vec<(Timestamp, f64)>,
    pub quality: f64,
    /// One line for the Alignment section (`Matched on accel_x: +1.2 s, r = 0.98`), when the method has something to say.
    pub note: Option<String>,
    /// The channel a method matched on.
    pub channel: Option<String>,
}

fn done(e: Estimate, note: Option<String>, channel: Option<String>) -> Aligned {
    Aligned { map: e.map, residuals: e.residuals, quality: e.quality, note, channel }
}

fn bad(why: impl Into<String>) -> Error {
    Error::Invalid(why.into())
}

fn ns(s: &str) -> Result<i64> {
    s.parse().map_err(|_| bad("A time is a whole number of nanoseconds"))
}

/// `reference` with its map `rmap` is the primary; `target` is lined up against it.
pub fn estimate(root: &Path, reference: &Opened, rmap: &TimeMap, target: &Opened, method: &Method) -> Result<Aligned> {
    let (ri, ti) = (reference.recording().info(), target.recording().info());
    let (Some(rf), Some(tf)) = (ri.t_first, ti.t_first) else { return Err(bad("A file has no samples")) };
    match method {
        Method::Starts => Ok(done(
            Estimate { map: TimeMap::offset(rmap.apply(rf) - tf), residuals: Vec::new(), quality: 1.0 },
            None,
            None,
        )),
        Method::Clock => Ok(done(Estimate { map: rmap.clone(), residuals: Vec::new(), quality: 1.0 }, None, None)),
        Method::Manual { seconds } => {
            let d = (seconds * 1e9).round() as i64;
            Ok(done(
                Estimate { map: TimeMap::offset(rmap.apply(rf) - tf + d), residuals: Vec::new(), quality: 1.0 },
                None,
                None,
            ))
        }
        Method::Sync { pairs, piecewise } => {
            let p: Result<Vec<(i64, i64)>> = pairs.iter().map(|(a, b)| Ok((ns(a)?, ns(b)?))).collect();
            let e = timemap::from_pairs(&p?, *piecewise).map_err(|e| bad(e.to_string()))?;
            Ok(done(e, None, None))
        }
        Method::Events { name } => by_events(root, reference, rmap, target, name.as_deref()),
        Method::Xcorr { channel, windows } => by_correlation(reference, rmap, target, channel.as_deref(), *windows),
        Method::TimeSource { channel } => by_time_channel(reference, rmap, target, channel.as_deref()),
        Method::Plugin { name, params } => by_plugin(reference, rmap, target, name, params),
    }
}

fn by_plugin(reference: &Opened, rmap: &TimeMap, target: &Opened, name: &str, params: &str) -> Result<Aligned> {
    let plugin = estimators()
        .into_iter()
        .find(|p| p.name() == name)
        .ok_or_else(|| bad(format!("No alignment plugin named {name} (Settings › Plugins lists them)")))?;
    let wanted = plugin.needs(params).map_err(|e| bad(format!("{name}: {e}")))?;
    let (rr, tr) = (reference.recording(), target.recording());
    let (Some(rf), Some(tf)) = (rr.info().t_first, tr.info().t_first) else {
        return Err(bad("A file has no samples"));
    };
    let series = |rec: &dyn Recording, map: Option<&TimeMap>| -> Result<Vec<Series>> {
        wanted
            .iter()
            .map(|n| {
                let ch = rec
                    .info()
                    .channels
                    .iter()
                    .position(|c| &c.name == n)
                    .ok_or_else(|| bad(format!("Both files need a channel named {n}")))?;
                let (mut t, v) = thin_many(rec, ch, PLUGIN_SAMPLES);
                if let Some(m) = map {
                    t.iter_mut().for_each(|x| *x = m.apply(*x));
                }
                Ok(Series { name: n.clone(), t, v })
            })
            .collect()
    };
    let (rs, ts) = (series(rr, Some(rmap))?, series(tr, None)?);
    let hint = TimeMap::offset(rmap.apply(rf) - tf);
    let e = plugin.estimate(&rs, &ts, &hint, params).map_err(|e| bad(format!("{name}: {e}")))?;
    let map = TimeMap::new(e.knots).map_err(|m| bad(format!("{name} returned a bad map: {m}")))?;
    let est = Estimate { map, residuals: e.residuals, quality: e.quality.clamp(0.0, 1.0) };
    Ok(done(est, e.note, None))
}

/// [`thin`], reading the whole channel at once when it has at most `m` samples.
fn thin_many(rec: &dyn Recording, ch: usize, m: usize) -> (Vec<Timestamp>, Vec<f64>) {
    let n = rec.info().samples;
    if n > m {
        return thin(rec, ch, m);
    }
    let (scale, offset) = rec.scale_offset(ch);
    (rec.read_times(0, n), rec.read(ch, 0, n).to_f64(scale, offset))
}

fn by_events(root: &Path, reference: &Opened, rmap: &TimeMap, target: &Opened, name: Option<&str>) -> Result<Aligned> {
    let list = |o: &Opened| -> Result<Vec<(String, i64)>> {
        Ok(events::list(root, o.identity())?
            .into_iter()
            .filter(|e| name.is_none_or(|n| e.name == n))
            .map(|e| (e.name, e.start.parse::<i64>().unwrap_or(0)))
            .collect())
    };
    let (r, t) = (list(reference)?, list(target)?);
    // the event name both files have the most of, when none is given
    let pick = match name {
        Some(n) => n.to_string(),
        None => {
            let count = |v: &[(String, i64)], n: &str| v.iter().filter(|e| e.0 == n).count();
            r.iter()
                .map(|e| e.0.clone())
                .filter(|n| count(&t, n) > 0)
                .max_by_key(|n| count(&r, n).min(count(&t, n)))
                .ok_or_else(|| bad("No event with the same name in both files"))?
        }
    };
    let of = |v: &[(String, i64)]| v.iter().filter(|e| e.0 == pick).map(|e| e.1).collect::<Vec<_>>();
    let (r, t) = (of(&r), of(&t));
    let k = r.len().min(t.len());
    if k == 0 {
        return Err(bad(format!("No “{pick}” events in both files")));
    }
    let pairs: Vec<(i64, i64)> = (0..k).map(|i| (t[i], rmap.apply(r[i]))).collect();
    let e = timemap::from_pairs(&pairs, false).map_err(|e| bad(e.to_string()))?;
    let note = format!("Paired {k} “{pick}” events");
    Ok(done(e, Some(note), None))
}

/// A channel's values at `m` evenly spread samples of the file: times and values.
fn thin(rec: &dyn Recording, ch: usize, m: usize) -> (Vec<Timestamp>, Vec<f64>) {
    let n = rec.info().samples;
    let (scale, offset) = rec.scale_offset(ch);
    let m = m.min(n);
    let (mut t, mut v) = (Vec::with_capacity(m), Vec::with_capacity(m));
    for k in 0..m {
        let i = if m == 1 { 0 } else { k * (n - 1) / (m - 1) };
        t.push(rec.time_at(i));
        v.push(rec.read(ch, i, i + 1).to_f64(scale, offset)[0]);
    }
    (t, v)
}

/// The channel to match on: the one asked for, else the first both files have (a channel with real variation).
fn shared_channel(reference: &Opened, target: &Opened, want: Option<&str>) -> Result<(String, usize, usize)> {
    let (ri, ti) = (reference.recording().info(), target.recording().info());
    let find = |i: &tlmbench_core::RecordingInfo, n: &str| i.channels.iter().position(|c| c.name == n);
    let names: Vec<&str> = match want {
        Some(w) => vec![w],
        None => ri.channels.iter().map(|c| c.name.as_str()).collect(),
    };
    for n in names {
        if let (Some(a), Some(b)) = (find(ri, n), find(ti, n)) {
            return Ok((n.to_string(), a, b));
        }
    }
    Err(bad(match want {
        Some(w) => format!("Both files need a channel named {w}"),
        None => "No channel with the same name in both files".to_string(),
    }))
}

fn median_interval(rec: &dyn Recording) -> i64 {
    let n = rec.info().samples;
    if n < 2 {
        return 1_000_000;
    }
    let k = n.min(1000);
    let t = rec.read_times(n / 2 - k.min(n / 2), n / 2 - k.min(n / 2) + k);
    let mut d: Vec<i64> = t.windows(2).map(|w| w[1] - w[0]).filter(|&d| d > 0).collect();
    d.sort_unstable();
    d.get(d.len() / 2).copied().unwrap_or(1_000_000)
}

/// `n` points of a channel from `t0` by `step`, in the file's own clock, by linear interpolation of its samples.
fn sample(rec: &dyn Recording, ch: usize, t0: Timestamp, step: i64, n: usize) -> Option<Grid> {
    let total = rec.info().samples;
    let a = rec.lower_bound(t0).saturating_sub(2);
    let b = (rec.lower_bound(t0 + step * n as i64) + 3).min(total);
    if b < a + 4 {
        return None;
    }
    let (scale, offset) = rec.scale_offset(ch);
    let (t, v) = (rec.read_times(a, b), rec.read(ch, a, b).to_f64(scale, offset));
    Some(Grid::at(&t, &v, t0, step, n))
}

fn by_correlation(
    reference: &Opened,
    rmap: &TimeMap,
    target: &Opened,
    channel: Option<&str>,
    windows: Option<usize>,
) -> Result<Aligned> {
    let (name, rc, tc) = shared_channel(reference, target, channel)?;
    let (rr, tr) = (reference.recording(), target.recording());
    let (ri, ti) = (rr.info(), tr.info());
    let (Some((rf, rl)), Some((tf, tl))) = (ri.t_first.zip(ri.t_last), ti.t_first.zip(ti.t_last)) else {
        return Err(bad("A file has no samples"));
    };
    // coarse: whole spans from their starts on a common grid, any shift up to half the shorter span
    let (rspan, tspan) = (rl - rf, tl - tf);
    let coarse = (rspan.max(tspan) / 6000).max(1);
    let ng = |span: i64| (span / coarse) as usize + 1;
    let (rt, rv) = thin(rr, rc, 6000);
    let (tt, tv) = thin(tr, tc, 6000);
    let rg = Grid::at(&rt, &rv, rt[0], coarse, ng(rspan));
    let tg = Grid::at(&tt, &tv, tt[0], coarse, ng(tspan));
    let lag = rg.values.len().min(tg.values.len()) / 2;
    let (shift0, r0) = timemap::xcorr(&Grid { start_ns: 0, ..rg }, &Grid { start_ns: 0, ..tg }, lag)
        .ok_or_else(|| bad("Not enough samples to match"))?;
    if r0 < 0.3 {
        return Err(bad(format!("No clear match on {name} (r = {r0:.2})")));
    }
    // target own t is the reference's own t + (rf - tf) + shift0
    let guess = |t: Timestamp| t + (rf - tf) + shift0;
    // fine: windows at the files' native rate around the coarse answer
    let step = median_interval(rr).max(median_interval(tr)).max(1);
    let w = windows.unwrap_or(if tspan > 120_000_000_000 { 3 } else { 1 }).clamp(1, 8);
    let len = (tspan / w as i64).min(20_000 * step).max(step * 64);
    let margin = (2 * coarse / step).clamp(8, 4000) as usize;
    let mut pairs: Vec<(Timestamp, Timestamp)> = Vec::new();
    let mut rs = Vec::new();
    for k in 0..w {
        let centre = tf + tspan * (2 * k as i64 + 1) / (2 * w as i64);
        let a = centre - len / 2;
        let n = (len / step) as usize;
        let Some(tw) = sample(tr, tc, a, step, n) else { continue };
        let Some(rw) = sample(rr, rc, guess(a) - margin as i64 * step, step, n + 2 * margin) else { continue };
        let Some((s, r)) = timemap::xcorr(&rw, &tw, 2 * margin) else { continue };
        // the shift is the reference's own time minus the target's, for the same moment
        let own = a + len / 2;
        pairs.push((own, rmap.apply(own + s)));
        rs.push(r);
    }
    if pairs.is_empty() {
        return Err(bad("The files don't overlap on that channel"));
    }
    let rbar = rs.iter().sum::<f64>() / rs.len() as f64;
    let mut e = timemap::from_pairs(&pairs, false).map_err(|e| bad(e.to_string()))?;
    e.quality = (e.quality * rbar).clamp(0.0, 1.0);
    let off = e.map.apply(tf) - tf;
    let note = format!("Matched on {name}: {}, r = {rbar:.2}", signed(off));
    Ok(done(e, Some(note), Some(name)))
}

fn signed(ns: i64) -> String {
    let s = ns as f64 / 1e9;
    format!("{}{}", if s < 0.0 { '−' } else { '+' }, fmt_dur(s.abs()))
}

fn fmt_dur(s: f64) -> String {
    if s >= 3600.0 {
        format!("{:.2} h", s / 3600.0)
    } else if s >= 60.0 {
        format!("{:.2} min", s / 60.0)
    } else if s >= 1.0 {
        format!("{s:.2} s")
    } else {
        format!("{:.1} ms", s * 1e3)
    }
}

/// Both files record a time channel (seconds, say a GPS time of week): the moment the target's sample says is the
/// moment the reference's channel reaches the same value.
fn by_time_channel(reference: &Opened, rmap: &TimeMap, target: &Opened, channel: Option<&str>) -> Result<Aligned> {
    let (name, rc, tc) = shared_channel(reference, target, channel)?;
    let (rr, tr) = (reference.recording(), target.recording());
    let (rt, rv) = thin(rr, rc, 4000);
    let (tt, tv) = thin(tr, tc, 4000);
    // the reference's own time at a time-source value, by interpolation (the channel only rises)
    let at = |v: f64| -> Option<f64> {
        let j = rv.partition_point(|&x| x < v);
        if j == 0 || j >= rv.len() {
            return None;
        }
        let (a, b) = (rv[j - 1], rv[j]);
        (b > a).then(|| rt[j - 1] as f64 + (rt[j] - rt[j - 1]) as f64 * (v - a) / (b - a))
    };
    let mut pairs = Vec::new();
    for (&t, &v) in tt.iter().zip(&tv) {
        if let Some(own) = at(v) {
            let p = (t, rmap.apply(own.round() as i64));
            if pairs.last().is_none_or(|l: &(i64, i64)| p.0 > l.0 && p.1 > l.1) {
                pairs.push(p);
            }
        }
    }
    if pairs.len() < 2 {
        return Err(bad(format!("The files' {name} values don't overlap")));
    }
    let e = timemap::from_pairs(&pairs, false).map_err(|e| bad(e.to_string()))?;
    let off = e.map.apply(pairs[0].0) - pairs[0].0;
    let note = format!("{name}: {}", signed(off));
    Ok(done(e, Some(note), Some(name)))
}
