//! Time maps: a monotonic, piecewise-linear map from a file's own clock to the tab's shared timeline,
//! `t_shared = map(t_file)`, stored as knots. One knot is an offset, two add drift, more follow jumps and wander.
//! Every alignment method produces one. The core applies it at query time: a shared span goes back through
//! [`TimeMap::inverse`] to the file's clock, and the times that come back go forward through [`TimeMap::apply`].
//! Samples are never rewritten, so changing a map needs no re-indexing.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// `(t_file_ns, t_shared_ns)`, strictly increasing in both. One knot is an offset, two are offset and drift.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeMap {
    pub knots: Vec<(Timestamp, Timestamp)>,
}

/// Why a list of knots is not a time map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapError {
    /// Times must increase in both clocks (a map that runs backwards has no inverse).
    NotMonotonic,
    /// Fewer points than the method needs.
    TooFew,
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::NotMonotonic => f.write_str("the times must increase in both files"),
            MapError::TooFew => f.write_str("not enough points"),
        }
    }
}

impl std::error::Error for MapError {}

impl TimeMap {
    /// The identity (no knots).
    pub fn identity() -> Self {
        Self::default()
    }

    /// A plain offset: `t_shared = t_file + offset`.
    pub fn offset(offset_ns: i64) -> Self {
        Self { knots: vec![(0, offset_ns)] }
    }

    /// A map through `knots`, which must increase in both coordinates.
    pub fn new(knots: Vec<(Timestamp, Timestamp)>) -> Result<Self, MapError> {
        if knots.windows(2).any(|w| w[1].0 <= w[0].0 || w[1].1 <= w[0].1) {
            return Err(MapError::NotMonotonic);
        }
        Ok(Self { knots })
    }

    pub fn is_identity(&self) -> bool {
        self.knots.iter().all(|&(f, s)| f == s)
    }

    /// The offset at the file's time zero-point: for one knot the offset itself, else the first segment's.
    pub fn offset_at(&self, t_file: Timestamp) -> i64 {
        self.apply(t_file) - t_file
    }

    /// Seconds of shared time per second of the file's clock (1 for none or one knot; the end segments' slope
    /// beyond the ends).
    pub fn rate_at(&self, t_file: Timestamp) -> f64 {
        let k = &self.knots;
        if k.len() < 2 {
            return 1.0;
        }
        let i = k.partition_point(|&(f, _)| f <= t_file).clamp(1, k.len() - 1);
        let (a, b) = (k[i - 1], k[i]);
        (b.1 - a.1) as f64 / (b.0 - a.0) as f64
    }

    /// The shared time of a file time. Beyond the knots the end segments continue (slope 1 for a single knot).
    pub fn apply(&self, t: Timestamp) -> Timestamp {
        self.through(t, false)
    }

    /// The file time of a shared time: the inverse of [`TimeMap::apply`].
    pub fn inverse(&self, s: Timestamp) -> Timestamp {
        self.through(s, true)
    }

    fn through(&self, x: Timestamp, back: bool) -> Timestamp {
        let k = &self.knots;
        let (from, to): (fn(&(i64, i64)) -> i64, fn(&(i64, i64)) -> i64) =
            if back { (|p| p.1, |p| p.0) } else { (|p| p.0, |p| p.1) };
        match k.len() {
            0 => x,
            1 => x - from(&k[0]) + to(&k[0]),
            n => {
                let i = k.partition_point(|p| from(p) <= x).clamp(1, n - 1);
                let (a, b) = (&k[i - 1], &k[i]);
                let (dx, dy) = ((from(b) - from(a)) as i128, (to(b) - to(a)) as i128);
                let y = to(a) as i128 + (x - from(a)) as i128 * dy / dx;
                y as i64
            }
        }
    }

    /// The shared times of a span of the file's clock (a map is increasing, so the ends map to the ends).
    pub fn span(&self, t0: Timestamp, t1: Timestamp) -> (Timestamp, Timestamp) {
        (self.apply(t0), self.apply(t1))
    }

    /// Adds `delta_ns` to every shared time (a manual nudge on top of any method).
    pub fn nudged(&self, delta_ns: i64) -> Self {
        if self.knots.is_empty() {
            return Self::offset(delta_ns);
        }
        Self { knots: self.knots.iter().map(|&(f, s)| (f, s + delta_ns)).collect() }
    }
}

/// What an estimator returns: the map, how far each matched point is from it, and a score in `0..=1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub map: TimeMap,
    /// `(t_file, residual_ns)` per matched point (empty for methods with nothing to match).
    pub residuals: Vec<(Timestamp, f64)>,
    pub quality: f64,
}

/// A least-squares line through matched moments `(t_file, t_shared)`. Two points give offset and drift exactly; more
/// give the best straight fit with its residuals. `piecewise` instead passes through every point (jumps and wander;
/// the points must increase in both clocks).
pub fn from_pairs(pairs: &[(Timestamp, Timestamp)], piecewise: bool) -> Result<Estimate, MapError> {
    if pairs.is_empty() {
        return Err(MapError::TooFew);
    }
    let mut p = pairs.to_vec();
    p.sort_unstable();
    if pairs.len() == 1 {
        return Ok(Estimate { map: TimeMap::offset(p[0].1 - p[0].0), residuals: vec![(p[0].0, 0.0)], quality: 1.0 });
    }
    if piecewise {
        let map = TimeMap::new(p.clone())?;
        return Ok(Estimate { map, residuals: p.iter().map(|&(f, _)| (f, 0.0)).collect(), quality: 1.0 });
    }
    // the line through the centroid with the least-squares slope, kept as two knots at the extreme points
    let n = p.len() as f64;
    let (f0, s0) = (p[0].0, p[0].1);
    let (mf, ms) =
        (p.iter().map(|q| (q.0 - f0) as f64).sum::<f64>() / n, p.iter().map(|q| (q.1 - s0) as f64).sum::<f64>() / n);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for q in &p {
        let (x, y) = ((q.0 - f0) as f64 - mf, (q.1 - s0) as f64 - ms);
        sxx += x * x;
        sxy += x * y;
    }
    if sxx == 0.0 {
        return Err(MapError::NotMonotonic);
    }
    let slope = sxy / sxx;
    if slope <= 0.0 {
        return Err(MapError::NotMonotonic);
    }
    let at = |t: Timestamp| s0 as f64 + ms + slope * ((t - f0) as f64 - mf);
    let (a, b) = (p[0].0, p[p.len() - 1].0);
    let map = TimeMap::new(vec![(a, at(a).round() as i64), (b, at(b).round() as i64)])?;
    let residuals: Vec<(Timestamp, f64)> = p.iter().map(|q| (q.0, q.1 as f64 - at(q.0))).collect();
    let rms = (residuals.iter().map(|r| r.1 * r.1).sum::<f64>() / n).sqrt();
    // a point set is as good as its misfit is small against the span it covers
    let quality = (1.0 - rms / ((b - a) as f64 * 1e-3).max(1.0)).clamp(0.0, 1.0);
    Ok(Estimate { map, residuals, quality })
}

/// A series of `(time_ns, value)` on an even grid of `step_ns` from `start_ns`, for correlation.
#[derive(Clone, Debug)]
pub struct Grid {
    pub start_ns: Timestamp,
    pub step_ns: i64,
    pub values: Vec<f64>,
}

impl Grid {
    /// Resamples `(t, v)` points (increasing times) onto a grid of `n` points over their span by linear
    /// interpolation, so two series logged at different rates can be compared.
    pub fn resample(times: &[Timestamp], values: &[f64], n: usize) -> Option<Grid> {
        if times.len() < 2 || n < 2 || times.len() != values.len() {
            return None;
        }
        let (t0, t1) = (times[0], times[times.len() - 1]);
        let step = ((t1 - t0) as f64 / (n - 1) as f64).max(1.0);
        let mut j = 0;
        let out = (0..n)
            .map(|k| {
                let t = t0 as f64 + step * k as f64;
                while j + 2 < times.len() && (times[j + 1] as f64) < t {
                    j += 1;
                }
                let (a, b) = (times[j] as f64, times[j + 1] as f64);
                let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
                values[j] + (values[j + 1] - values[j]) * u
            })
            .collect();
        Some(Grid { start_ns: t0, step_ns: step.round() as i64, values: out })
    }
}

/// The shift of `other` against `reference` (both on grids of the same step) that maximises their Pearson
/// correlation, within `±max_lag` steps: the shared time of an `other` time is `t + shift_ns`. Returns the shift and
/// r. Direct correlation over the overlap, refined to a sample by a parabola through the peak.
pub fn xcorr(reference: &Grid, other: &Grid, max_lag: usize) -> Option<(i64, f64)> {
    let (a, b) = (&reference.values, &other.values);
    if a.len() < 8 || b.len() < 8 || reference.step_ns != other.step_ns {
        return None;
    }
    let max_lag = max_lag.min(a.len().min(b.len()) / 2) as isize;
    let r_at = |lag: isize| -> f64 {
        // other[j] pairs with reference[j + lag]
        let (lo, hi) = (0.max(-lag) as usize, (b.len() as isize).min(a.len() as isize - lag).max(0) as usize);
        if hi <= lo + 4 {
            return f64::NEG_INFINITY;
        }
        let n = (hi - lo) as f64;
        let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for j in lo..hi {
            let (x, y) = (a[(j as isize + lag) as usize], b[j]);
            sa += x;
            sb += y;
            saa += x * x;
            sbb += y * y;
            sab += x * y;
        }
        let (va, vb) = (saa - sa * sa / n, sbb - sb * sb / n);
        if va <= 0.0 || vb <= 0.0 { 0.0 } else { (sab - sa * sb / n) / (va * vb).sqrt() }
    };
    let (mut best, mut best_r) = (0isize, f64::NEG_INFINITY);
    for lag in -max_lag..=max_lag {
        let r = r_at(lag);
        if r > best_r {
            (best, best_r) = (lag, r);
        }
    }
    if !best_r.is_finite() {
        return None;
    }
    let (rm, rp) = (r_at(best - 1), r_at(best + 1));
    let frac = if rm.is_finite() && rp.is_finite() {
        let d = rm - 2.0 * best_r + rp;
        if d < 0.0 { (0.5 * (rm - rp) / d).clamp(-0.5, 0.5) } else { 0.0 }
    } else {
        0.0
    };
    let lag_steps = best as f64 + frac;
    // reference[j + lag] is the same moment as other[j]: shared = other_time + (ref_start - other_start) + lag × step
    let shift = (reference.start_ns - other.start_ns) as f64 + lag_steps * reference.step_ns as f64;
    Some((shift.round() as i64, best_r))
}

/// A time-source channel (a GPS time of week, a PPS count) becomes the map directly: `source_ns[i]` is the shared
/// time of the sample at `file_ns[i]`. Only points that increase in both are kept, then thinned to `max_knots`.
pub fn from_source(file_ns: &[Timestamp], source_ns: &[Timestamp], max_knots: usize) -> Result<Estimate, MapError> {
    let mut pairs: Vec<(Timestamp, Timestamp)> = Vec::new();
    for (&f, &s) in file_ns.iter().zip(source_ns) {
        if pairs.last().is_none_or(|l| f > l.0 && s > l.1) {
            pairs.push((f, s));
        }
    }
    if pairs.len() < 2 {
        return Err(MapError::TooFew);
    }
    // a clock that only has an offset and steady drift is two knots; a jump needs the rest
    let line = from_pairs(&pairs, false)?;
    let worst = line.residuals.iter().map(|r| r.1.abs()).fold(0.0, f64::max);
    if worst < 1e6 {
        return Ok(line);
    }
    let step = pairs.len().div_ceil(max_knots.max(2));
    let mut thin: Vec<_> = pairs.iter().copied().step_by(step.max(1)).collect();
    if thin.last() != pairs.last() {
        thin.push(*pairs.last().expect("non-empty"));
    }
    from_pairs(&thin, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_and_inverse() {
        let m = TimeMap::offset(5_000);
        assert_eq!(m.apply(1_000), 6_000);
        assert_eq!(m.inverse(6_000), 1_000);
        assert!(TimeMap::identity().is_identity());
    }

    #[test]
    fn drift_round_trips_within_a_nanosecond() {
        // the shared clock runs 100 ppm faster, starting 2 s late
        let m = TimeMap::new(vec![(0, 2_000_000_000), (1_000_000_000_000, 1_000_100_000_000 + 2_000_000_000)]).unwrap();
        for t in [-5_000_000_000i64, 0, 123_456_789_012, 2_000_000_000_000] {
            assert!((m.inverse(m.apply(t)) - t).abs() <= 1, "{t}");
        }
        assert!((m.rate_at(5) - 1.0001).abs() < 1e-9);
    }

    #[test]
    fn rejects_a_map_that_runs_backwards() {
        assert_eq!(TimeMap::new(vec![(0, 10), (5, 5)]), Err(MapError::NotMonotonic));
    }

    #[test]
    fn two_sync_points_give_offset_and_drift() {
        let e = from_pairs(&[(0, 1_000), (1_000_000, 1_002_000)], false).unwrap();
        assert_eq!(e.map.apply(500_000), 501_500);
        assert!(e.quality > 0.99);
    }

    #[test]
    fn more_points_fit_a_line_and_report_residuals() {
        let pairs: Vec<_> = (0..10)
            .map(|i| (i * 1_000_000, i * 1_000_000 * 1001 / 1000 + 7 + if i % 2 == 0 { 3 } else { -3 }))
            .collect();
        let e = from_pairs(&pairs, false).unwrap();
        assert_eq!(e.residuals.len(), 10);
        assert!(e.residuals.iter().all(|r| r.1.abs() < 5.0));
        assert!((e.map.rate_at(0) - 1.001).abs() < 1e-5);
    }

    #[test]
    fn piecewise_follows_a_jump() {
        let e = from_pairs(&[(0, 0), (100, 100), (200, 5_200)], true).unwrap();
        assert_eq!(e.map.apply(150), 2_650);
        assert_eq!(e.map.inverse(2_650), 150);
    }

    #[test]
    fn nudge_shifts_every_knot() {
        let m = TimeMap::new(vec![(0, 0), (10, 20)]).unwrap().nudged(7);
        assert_eq!(m.apply(10), 27);
    }

    fn wave(n: usize, shift: f64) -> Vec<f64> {
        (0..n)
            .map(|i| {
                let t = i as f64 - shift;
                (t * 0.05).sin() + 0.5 * (t * 0.013).sin() + if (150.0..170.0).contains(&t) { 2.0 } else { 0.0 }
            })
            .collect()
    }

    #[test]
    fn correlation_finds_a_known_shift_within_a_sample() {
        // the same signal 37.4 grid steps later in `other`
        let step = 1_000_000i64;
        let a = Grid { start_ns: 0, step_ns: step, values: wave(1000, 0.0) };
        let b = Grid { start_ns: 0, step_ns: step, values: wave(1000, 37.4) };
        let (shift, r) = xcorr(&a, &b, 100).unwrap();
        // other's moment t is the reference's t - 37.4 steps
        assert!((shift + (37.4 * step as f64) as i64).abs() < step, "{shift}");
        assert!(r > 0.99);
    }

    #[test]
    fn source_channel_becomes_the_map() {
        let f: Vec<i64> = (0..100).map(|i| i * 10_000_000).collect();
        let s: Vec<i64> = f.iter().map(|t| 3_000_000_000 + t + t / 10_000).collect();
        let e = from_source(&f, &s, 64).unwrap();
        assert_eq!(e.map.knots.len(), 2);
        assert!((e.map.apply(500_000_000) - (3_000_000_000 + 500_000_000 + 50_000)).abs() <= 2);
    }
}
