//! M4 downsampling straight from raw samples.
//!
//! For a viewport of W pixel columns, each column's bucket keeps its first, min, max and last samples (in time
//! order). A polyline through those points draws the same pixels as one through every raw sample, so the reply
//! is at most about 4W points whatever the file size.
//!
//! Buckets sit on a global grid (`floor(t / bucket_ns)`), not on the viewport's left edge, so panning at a fixed
//! zoom picks the same samples and the line doesn't shimmer. When the view holds at most 4W samples the raw
//! samples are cheaper than M4 and are returned as they are.
//!
//! Missing samples: NaN values never become a bucket's min or max. A bucket with no samples, or only NaNs, is a
//! gap: the trace gets one NaN point there, which breaks the line.

use rayon::prelude::*;

use crate::{Samples, Times, Timestamp, Value};

/// The time range `[t0, t1)` drawn across `width_px` pixel columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub t0: Timestamp,
    pub t1: Timestamp,
    pub width_px: u32,
    /// Draw every sample: a view of at most this many samples comes back raw, not reduced (0: only when there are
    /// at most four per pixel column, which looks the same as reduced).
    pub raw_max: u32,
}

impl Viewport {
    /// The most samples in view that come back raw: four per pixel column, or `raw_max` when it is more.
    pub fn raw_limit(&self) -> usize {
        (4 * self.width_px as usize).max(self.raw_max as usize)
    }

    /// Bucket width in nanoseconds: the time one pixel column covers, at least 1 ns.
    pub fn bucket_ns(&self) -> i64 {
        let span = self.t1 as i128 - self.t0 as i128;
        (span / self.width_px.max(1) as i128).clamp(1, i64::MAX as i128) as i64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    /// Every sample in the view (at most 4W of them).
    Raw,
    /// First, min, max and last of each pixel column's bucket.
    M4,
}

/// One channel reduced for a viewport. Includes the nearest sample on each side of the view, so the line runs
/// to the edges. A NaN in `y` is a gap.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub kind: TraceKind,
    /// Bucket width used (M4), or the bucket width the view would use (raw).
    pub bucket_ns: i64,
    pub t: Vec<Timestamp>,
    pub y: Vec<f32>,
}

impl Trace {
    pub fn len(&self) -> usize {
        self.t.len()
    }

    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }
}

/// What the reduction emits, before times and values are looked up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Emit {
    Sample(usize),
    /// A sample whose value is already known (from a summary), so it needn't be read.
    Known(usize, f32),
    /// A gap starting at this time.
    Gap(Timestamp),
}

/// Views with more samples than this are reduced on several threads.
const PARALLEL_MIN_SAMPLES: usize = 1 << 18;

/// Reduces one channel for a viewport. `values[i]` is the sample at `times.at(i)`; times must not decrease.
pub fn m4<T: Times + ?Sized, S: Samples + ?Sized>(times: &T, values: &S, vp: Viewport) -> Trace {
    m4_breaking(times, values, vp, &[])
}

/// [`m4`], with the line broken before each sample in `breaks` (sorted sample indexes, from the gap index).
pub fn m4_breaking<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    vp: Viewport,
    breaks: &[usize],
) -> Trace {
    let (kind, emits) = reduce(times, values, vp);
    to_trace(times, values, kind, &break_lines(emits, breaks, times), vp)
}

/// Inserts a gap between two emitted samples when a break falls after the first and at or before the second.
pub(crate) fn break_lines<T: Times + ?Sized>(emits: Vec<Emit>, breaks: &[usize], times: &T) -> Vec<Emit> {
    if breaks.is_empty() {
        return emits;
    }
    let mut out = Vec::with_capacity(emits.len() + 8);
    let mut prev: Option<usize> = None;
    for e in emits {
        if let (Emit::Sample(q) | Emit::Known(q, _), Some(p)) = (e, prev) {
            let k = breaks.partition_point(|&b| b <= p);
            if breaks.get(k).is_some_and(|&b| b <= q) && !matches!(out.last(), Some(Emit::Gap(_))) {
                out.push(Emit::Gap(times.at(q - 1)));
            }
        }
        prev = match e {
            Emit::Sample(i) | Emit::Known(i, _) => Some(i),
            Emit::Gap(_) => prev,
        };
        out.push(e);
    }
    out
}

/// Looks up the times and values of what a reduction emitted.
pub(crate) fn to_trace<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    kind: TraceKind,
    emits: &[Emit],
    vp: Viewport,
) -> Trace {
    let mut t = Vec::with_capacity(emits.len());
    let mut y = Vec::with_capacity(emits.len());
    for &e in emits {
        match e {
            Emit::Sample(i) => {
                t.push(times.at(i));
                y.push(values.get(i).to_f32());
            }
            Emit::Known(i, v) => {
                t.push(times.at(i));
                y.push(v);
            }
            Emit::Gap(at) => {
                t.push(at);
                y.push(f32::NAN);
            }
        }
    }
    Trace { kind, bucket_ns: vp.bucket_ns(), t, y }
}

pub(crate) fn reduce<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    vp: Viewport,
) -> (TraceKind, Vec<Emit>) {
    assert_eq!(times.len(), values.len(), "times and values differ in length");
    reduce_with(times, vp, |start, end, bucket_t, out| reduce_bucket(values, start, end, bucket_t, out))
}

/// The shape of every reduction: raw samples for sparse views, otherwise `bucket(start, end, bucket_t, out)` for
/// each bucket on the global grid (on several threads for large views), plus the sample beyond each edge.
pub(crate) fn reduce_with<T: Times + ?Sized>(
    times: &T,
    vp: Viewport,
    bucket: impl Fn(usize, usize, Timestamp, &mut Vec<Emit>) + Sync,
) -> (TraceKind, Vec<Emit>) {
    let n = times.len();
    if n == 0 || vp.t1 <= vp.t0 || vp.width_px == 0 {
        return (TraceKind::Raw, Vec::new());
    }
    let bw = vp.bucket_ns();

    let (lo, hi) = (times.lower_bound(vp.t0), times.lower_bound(vp.t1));
    if hi - lo <= vp.raw_limit() {
        let mut out = Vec::with_capacity(hi - lo + 2);
        out.extend(lo.checked_sub(1).map(Emit::Sample));
        out.extend((lo..hi).map(Emit::Sample));
        if hi < n {
            out.push(Emit::Sample(hi));
        }
        return (TraceKind::Raw, out);
    }

    // Whole buckets on the global grid, including the parts outside [t0, t1) of the first and last ones.
    let (b_first, b_last) = (vp.t0.div_euclid(bw), (vp.t1 - 1).div_euclid(bw));
    let bucket_start = |b: i64| times.lower_bound(b.saturating_mul(bw));
    let (lo, hi) = (bucket_start(b_first), bucket_start(b_last + 1));

    let reduce_buckets = |from: i64, to: i64| {
        let mut out = Vec::with_capacity(4 * (to - from) as usize);
        let mut start = bucket_start(from);
        for b in from..to {
            let end = bucket_start(b + 1);
            bucket(start, end, b * bw, &mut out);
            start = end;
        }
        out
    };

    let buckets = b_last - b_first + 1;
    let parts: Vec<Vec<Emit>> = if hi - lo >= PARALLEL_MIN_SAMPLES && buckets > 1 {
        let chunks = (rayon::current_num_threads() as i64 * 4).min(buckets);
        (0..chunks)
            .into_par_iter()
            .map(|c| reduce_buckets(b_first + buckets * c / chunks, b_first + buckets * (c + 1) / chunks))
            .collect()
    } else {
        vec![reduce_buckets(b_first, b_last + 1)]
    };

    let mut out = Vec::with_capacity(parts.iter().map(Vec::len).sum::<usize>() + 2);
    out.extend(lo.checked_sub(1).map(Emit::Sample));
    for e in parts.into_iter().flatten() {
        // runs of empty buckets are one gap, also across chunk boundaries
        if !(matches!(e, Emit::Gap(_)) && matches!(out.last(), Some(Emit::Gap(_)))) {
            out.push(e);
        }
    }
    if hi < n {
        out.push(Emit::Sample(hi));
    }
    (TraceKind::M4, out)
}

/// Appends the first, min, max and last samples of `values[start..end]` in index order (each once), or a gap
/// if the bucket has no non-NaN samples. Ties go to the earliest sample.
#[inline]
pub(crate) fn reduce_bucket<S: Samples + ?Sized>(
    values: &S,
    start: usize,
    end: usize,
    bucket_t: Timestamp,
    out: &mut Vec<Emit>,
) {
    let mut bucket = values.range(start, end);
    let Some(first) = bucket.clone().position(|v| !v.is_nan()) else {
        return push_bucket(out, None, bucket_t);
    };
    let last = bucket.rposition(|v| !v.is_nan()).unwrap_or(first);
    let mut body = values.range(start + first, start + last + 1);

    // Comparisons with NaN are false, so starting from a real value skips NaNs.
    let v0 = values.get(start + first);
    let (imin, imax) = match values.slice(start + first, start + last + 1) {
        Some(body) => {
            let (mn, mx) = min_max(body, v0);
            (
                first + body.iter().position(|&v| v == mn).unwrap_or(0),
                first + body.iter().position(|&v| v == mx).unwrap_or(0),
            )
        }
        None => {
            let (mn, mx) =
                body.clone().fold((v0, v0), |(mn, mx), v| (if v < mn { v } else { mn }, if v > mx { v } else { mx }));
            (first + body.clone().position(|v| v == mn).unwrap_or(0), first + body.position(|v| v == mx).unwrap_or(0))
        }
    };

    let idx = [first, imin, imax, last].map(|i| i + start);
    push_bucket(out, Some(idx), bucket_t);
}

/// Appends a bucket's picks `[first, imin, imax, last]` (sample indexes) in index order, each once, or a gap when
/// the bucket has no real samples (one gap for a run of them).
#[inline]
pub(crate) fn push_bucket(out: &mut Vec<Emit>, picks: Option<[usize; 4]>, bucket_t: Timestamp) {
    let Some([first, imin, imax, last]) = picks else {
        if !matches!(out.last(), Some(Emit::Gap(_))) {
            out.push(Emit::Gap(bucket_t));
        }
        return;
    };
    let mut prev = usize::MAX;
    for i in [first, imin.min(imax), imin.max(imax), last] {
        if i != prev {
            out.push(Emit::Sample(i));
            prev = i;
        }
    }
}

/// Min and max of `s`, skipping NaN, starting from the real value `v0`. Sixteen independent lanes of compare and
/// select, which the compiler turns into SIMD; a single running min/max can't vectorize.
#[inline]
pub(crate) fn min_max<V: Value>(s: &[V], v0: V) -> (V, V) {
    const LANES: usize = 16;
    let (mut mn, mut mx) = ([v0; LANES], [v0; LANES]);
    let (chunks, rest) = s.as_chunks::<LANES>();
    for c in chunks {
        for j in 0..LANES {
            let v = c[j];
            mn[j] = if v < mn[j] { v } else { mn[j] };
            mx[j] = if v > mx[j] { v } else { mx[j] };
        }
    }
    let (mut lo, mut hi) = (v0, v0);
    for v in mn.into_iter().chain(rest.iter().copied()) {
        lo = if v < lo { v } else { lo };
    }
    for v in mx.into_iter().chain(rest.iter().copied()) {
        hi = if v > hi { v } else { hi };
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RateTimes;
    use proptest::prelude::*;

    /// Brute force: group every sample by its bucket, one bucket at a time, with no binary searches.
    fn reference<T: Times + ?Sized>(times: &T, values: &[f32], vp: Viewport) -> (TraceKind, Vec<Emit>) {
        let n = times.len();
        if n == 0 || vp.t1 <= vp.t0 || vp.width_px == 0 {
            return (TraceKind::Raw, vec![]);
        }
        let bw = vp.bucket_ns();
        let in_view: Vec<usize> = (0..n).filter(|&i| (vp.t0..vp.t1).contains(&times.at(i))).collect();
        let context = |range: &[usize], out: &mut Vec<Emit>, before: bool| {
            let edge = if before { range.first() } else { range.last() };
            let idx = match (edge, before) {
                (Some(&i), true) => i.checked_sub(1),
                (Some(&i), false) => Some(i + 1).filter(|&j| j < n),
                (None, _) => None,
            };
            out.extend(idx.map(Emit::Sample));
        };
        if in_view.len() <= vp.raw_limit() {
            let mut out = vec![];
            if in_view.is_empty() {
                let lo = (0..n).find(|&i| times.at(i) >= vp.t0).unwrap_or(n);
                out.extend(lo.checked_sub(1).map(Emit::Sample));
                if lo < n {
                    out.push(Emit::Sample(lo));
                }
                return (TraceKind::Raw, out);
            }
            context(&in_view, &mut out, true);
            out.extend(in_view.iter().map(|&i| Emit::Sample(i)));
            context(&in_view, &mut out, false);
            return (TraceKind::Raw, out);
        }
        let (bf, bl) = (vp.t0.div_euclid(bw), (vp.t1 - 1).div_euclid(bw));
        let all: Vec<usize> = (0..n).filter(|&i| (bf..=bl).contains(&times.at(i).div_euclid(bw))).collect();
        let mut out = vec![];
        context(&all, &mut out, true);
        for b in bf..=bl {
            let idx: Vec<usize> =
                all.iter().copied().filter(|&i| times.at(i).div_euclid(bw) == b && !values[i].is_nan()).collect();
            if idx.is_empty() {
                if !matches!(out.last(), Some(Emit::Gap(_))) {
                    out.push(Emit::Gap(b * bw));
                }
                continue;
            }
            let mn = idx.iter().map(|&i| values[i]).fold(f32::INFINITY, f32::min);
            let mx = idx.iter().map(|&i| values[i]).fold(f32::NEG_INFINITY, f32::max);
            let imin = *idx.iter().find(|&&i| values[i] == mn).unwrap();
            let imax = *idx.iter().find(|&&i| values[i] == mx).unwrap();
            let mut pick = vec![idx[0], imin, imax, *idx.last().unwrap()];
            pick.sort();
            pick.dedup();
            out.extend(pick.into_iter().map(Emit::Sample));
        }
        context(&all, &mut out, false);
        (TraceKind::M4, out)
    }

    /// Sorted timestamps with repeats and jumps (gaps), and values with NaN runs and repeated extremes.
    fn series() -> impl Strategy<Value = (Vec<Timestamp>, Vec<f32>)> {
        (1usize..600).prop_flat_map(|n| {
            (
                proptest::collection::vec(prop_oneof![8 => 0i64..5, 1 => Just(0), 1 => 50i64..400], n),
                proptest::collection::vec(prop_oneof![8 => (-4i32..4).prop_map(|v| v as f32), 1 => Just(f32::NAN)], n),
                -1000i64..1000,
            )
                .prop_map(|(steps, values, start)| {
                    let times = steps
                        .iter()
                        .scan(start, |t, s| {
                            *t += s;
                            Some(*t)
                        })
                        .collect();
                    (times, values)
                })
        })
    }

    fn viewport() -> impl Strategy<Value = Viewport> {
        (-1500i64..40_000, 1i64..40_000, 1u32..80).prop_map(|(t0, span, w)| Viewport {
            t0,
            t1: t0 + span,
            width_px: w,
            raw_max: 0,
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2000))]

        #[test]
        fn matches_brute_force((times, values) in series(), vp in viewport()) {
            prop_assert_eq!(reduce(&times, &values, vp), reference(&times, &values, vp));
        }

        #[test]
        fn rate_times_match_brute_force(
            values in proptest::collection::vec(prop_oneof![8 => -100f32..100.0, 1 => Just(f32::NAN)], 1..800),
            rate in 1u32..5000, t0 in -10_000_000i64..10_000_000, vp in viewport(), scale in 1i64..200_000,
        ) {
            let times = RateTimes::from_rate(t0, rate as f64, values.len());
            let vp = Viewport { t0: vp.t0 * scale, t1: vp.t1 * scale, width_px: vp.width_px, raw_max: 0 };
            prop_assert_eq!(reduce(&times, &values, vp), reference(&times, &values, vp));
        }

        #[test]
        fn min_max_matches_a_scalar_fold(
            v in proptest::collection::vec(prop_oneof![6 => -50f32..50.0, 1 => Just(f32::NAN)], 1..200),
            w in proptest::collection::vec(any::<i16>(), 1..200),
        ) {
            if let Some(&v0) = v.iter().find(|x| !x.is_nan()) {
                let lo = v.iter().copied().filter(|x| !x.is_nan()).fold(f32::INFINITY, f32::min);
                let hi = v.iter().copied().filter(|x| !x.is_nan()).fold(f32::NEG_INFINITY, f32::max);
                prop_assert_eq!(min_max(&v, v0), (lo, hi));
            }
            let (lo, hi) = (*w.iter().min().unwrap(), *w.iter().max().unwrap());
            prop_assert_eq!(min_max(&w, w[0]), (lo, hi));
        }

        /// The pixel-exactness argument: every bucket's min and max survive the reduction.
        #[test]
        fn keeps_every_bucket_extreme((times, values) in series(), vp in viewport()) {
            let trace = m4(&times, &values, vp);
            let bw = vp.bucket_ns();
            for (i, (&t, &v)) in times.iter().zip(&values).enumerate() {
                if v.is_nan() || !(vp.t0..vp.t1).contains(&t) { continue; }
                let b = t.div_euclid(bw);
                let kept = trace.t.iter().zip(&trace.y).filter(|(tt, _)| tt.div_euclid(bw) == b).map(|(_, y)| *y);
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for y in kept.filter(|y| !y.is_nan()) { lo = lo.min(y); hi = hi.max(y); }
                prop_assert!(lo <= v && v <= hi, "sample {} ({}, {}) outside its bucket's kept range", i, t, v);
            }
        }
    }

    #[test]
    fn parallel_path_matches_sequential() {
        let n = PARALLEL_MIN_SAMPLES * 3;
        let times = RateTimes::from_rate(0, 1000.0, n);
        let values: Vec<f32> =
            (0..n).map(|i| if i % 100_003 < 50 { f32::NAN } else { ((i as f32) * 0.001).sin() }).collect();
        let vp = Viewport { t0: 0, t1: times.at(n - 1) + 1, width_px: 1500, raw_max: 0 };
        let (kind, par) = reduce(&times, &values, vp);
        assert_eq!(kind, TraceKind::M4);
        // the same viewport split by hand into sequential pieces gives the same emits
        let bw = vp.bucket_ns();
        let mut seq = vec![];
        let mut start = 0;
        for b in vp.t0.div_euclid(bw)..=(vp.t1 - 1).div_euclid(bw) {
            let end = times.lower_bound((b + 1) * bw);
            reduce_bucket(&values, start, end, b * bw, &mut seq);
            start = end;
        }
        assert_eq!(par, seq);
    }

    #[test]
    fn raw_max_draws_every_sample_up_to_it() {
        let times: Vec<Timestamp> = (0..10_000).collect();
        let values: Vec<f32> = (0..10_000).map(|i| (i % 7) as f32).collect();
        let vp = Viewport { t0: 0, t1: 10_000, width_px: 100, raw_max: 0 };
        assert_eq!(m4(&times, &values, vp).kind, TraceKind::M4, "400 raw at most by default");
        let every = m4(&times, &values, Viewport { raw_max: 10_000, ..vp });
        assert_eq!(every.kind, TraceKind::Raw);
        assert_eq!(every.t.len(), 10_000);
        assert_eq!(
            m4(&times, &values, Viewport { raw_max: 9_999, ..vp }).kind,
            TraceKind::M4,
            "past the budget it reduces"
        );
    }

    #[test]
    fn sparse_view_returns_raw_samples_with_neighbours() {
        let times: Vec<Timestamp> = (0..100).map(|i| i * 10).collect();
        let values: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let tr = m4(&times, &values, Viewport { t0: 205, t1: 255, width_px: 1000, raw_max: 0 });
        assert_eq!(tr.kind, TraceKind::Raw);
        assert_eq!(tr.t, vec![200, 210, 220, 230, 240, 250, 260]);
    }

    #[test]
    fn indexed_jumps_break_the_line_in_raw_and_m4_views() {
        // 10 ns steps with a 35 ns jump before sample 50: not an empty bucket in either view, but a gap at N = 2
        let times: Vec<Timestamp> = (0..100).map(|i| i * 10 + if i >= 50 { 25 } else { 0 }).collect();
        let values: Vec<f32> = (0..100).map(|i| (i % 7) as f32).collect();
        let breaks = crate::gaps::GapIndex::build(&times).breaks(2.0);
        assert_eq!(breaks, vec![50]);
        for width_px in [1000, 4] {
            let tr = m4_breaking(&times, &values, Viewport { t0: 0, t1: 1100, width_px, raw_max: 0 }, &breaks);
            let gap = tr.y.iter().position(|y| y.is_nan()).expect("a gap");
            assert!(tr.t[gap - 1] <= times[49] && tr.t[gap + 1] >= times[50], "width {width_px}: {:?}", tr.t);
            assert_eq!(tr.y.iter().filter(|y| y.is_nan()).count(), 1);
        }
    }

    #[test]
    fn empty_buckets_become_one_gap() {
        let mut times: Vec<Timestamp> = (0..1000).collect();
        times.extend(5000..6000);
        let values = vec![1.0f32; times.len()];
        let tr = m4(&times, &values, Viewport { t0: 0, t1: 6000, width_px: 60, raw_max: 0 });
        assert_eq!(tr.kind, TraceKind::M4);
        assert_eq!(tr.y.iter().filter(|y| y.is_nan()).count(), 1);
    }
}
