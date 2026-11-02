//! The min/max pyramid: per channel, summaries of fixed runs of samples at several levels, so a view of any span
//! reads a few thousand summaries instead of every sample.
//!
//! Level k holds one summary per 4^k samples, aligned to sample index: summary `s` covers samples
//! `s·4^k .. (s+1)·4^k` (the last one fewer). Levels from k = [`MIN_LEVEL`] (256 samples) are stored, up to the first
//! level with a single summary; finer views are reduced from raw samples. A summary is 88 bytes, so the stored
//! levels add about 11% to raw f32 data.
//!
//! Summaries are taken over the stored values (before a channel's scale and offset), so they never go stale when
//! a display setting changes; queries apply the scale and offset to what they return.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::m4::{Emit, break_lines, min_max, push_bucket, reduce_bucket, reduce_with, to_trace};
use crate::{Samples, Times, Trace, TraceKind, Value, Viewport};

/// The finest stored level: one summary per 4^4 = 256 samples.
pub const MIN_LEVEL: u32 = 4;

/// Samples per summary at level `k`.
pub const fn span(k: u32) -> usize {
    1 << (2 * k)
}

/// A run of samples summarized: enough for M4 and for exact min, max, mean and standard deviation. NaN samples
/// are missing: they never become the min or max and aren't counted. `first` and `last` are the values of the
/// run's first and last samples, NaN included, so their indexes need no storing.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Summary {
    /// Times of the run's first and last samples.
    pub t_first: i64,
    pub t_last: i64,
    /// The smallest and largest values (the earliest on ties) and their sample indexes; NaN when `count` is 0.
    pub min: f64,
    pub min_idx: u64,
    pub max: f64,
    pub max_idx: u64,
    pub first: f64,
    pub last: f64,
    pub sum: f64,
    pub sum_sq: f64,
    /// Samples that aren't NaN.
    pub count: u64,
}

const _: () = assert!(size_of::<Summary>() == 88);

impl Summary {
    /// Summarizes samples `start..end` (not empty).
    pub fn of<T: Times + ?Sized, S: Samples + ?Sized>(times: &T, values: &S, start: usize, end: usize) -> Self {
        debug_assert!(start < end);
        let mut s = Summary {
            t_first: times.at(start),
            t_last: times.at(end - 1),
            min: f64::NAN,
            min_idx: start as u64,
            max: f64::NAN,
            max_idx: start as u64,
            first: values.get(start).to_f64(),
            last: values.get(end - 1).to_f64(),
            sum: 0.0,
            sum_sq: 0.0,
            count: 0,
        };
        let Some(off) = values.range(start, end).position(|v| !v.is_nan()) else {
            return s;
        };
        let v0 = values.get(start + off);
        let (mn, mx) = match values.slice(start + off, end) {
            Some(sl) => min_max(sl, v0),
            None => values
                .range(start + off, end)
                .fold((v0, v0), |(mn, mx), v| (if v < mn { v } else { mn }, if v > mx { v } else { mx })),
        };
        let pos = |x| start + off + values.range(start + off, end).position(|v| v == x).unwrap_or(0);
        (s.min, s.min_idx, s.max, s.max_idx) = (mn.to_f64(), pos(mn) as u64, mx.to_f64(), pos(mx) as u64);
        // four running sums, so the adds pipeline (and vectorize for slices)
        let (mut sum, mut sq, mut n) = ([0f64; 4], [0f64; 4], 0u64);
        let mut acc = |k: usize, v: S::Item| {
            if !v.is_nan() {
                let x = v.to_f64();
                sum[k] += x;
                sq[k] += x * x;
                n += 1;
            }
        };
        match values.slice(start, end) {
            Some(sl) => {
                let (chunks, rest) = sl.as_chunks::<4>();
                for c in chunks {
                    (0..4).for_each(|k| acc(k, c[k]));
                }
                // sample i goes to sum i % 4 either way, so a chunked read adds in the same order, bit for bit
                rest.iter().enumerate().for_each(|(k, &v)| acc(k, v));
            }
            None => values.range(start, end).enumerate().for_each(|(i, v)| acc(i % 4, v)),
        }
        s.sum = sum.iter().sum();
        s.sum_sq = sq.iter().sum();
        s.count = n;
        s
    }

    /// `self` followed by `next` (adjacent runs, `self` first).
    pub fn merge(&self, next: &Summary) -> Summary {
        let take_next_min = next.count > 0 && (self.count == 0 || next.min < self.min);
        let take_next_max = next.count > 0 && (self.count == 0 || next.max > self.max);
        Summary {
            t_first: self.t_first,
            t_last: next.t_last,
            min: if take_next_min { next.min } else { self.min },
            min_idx: if take_next_min { next.min_idx } else { self.min_idx },
            max: if take_next_max { next.max } else { self.max },
            max_idx: if take_next_max { next.max_idx } else { self.max_idx },
            first: self.first,
            last: next.last,
            sum: self.sum + next.sum,
            sum_sq: self.sum_sq + next.sum_sq,
            count: self.count + next.count,
        }
    }
}

/// Level-[`MIN_LEVEL`] summaries of samples `start..end`, where `start` is a multiple of 256 (and `end` is too,
/// unless it is the channel's end). Runs on several threads.
pub fn summarize<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    start: usize,
    end: usize,
) -> Vec<Summary> {
    let w = span(MIN_LEVEL);
    assert!(start.is_multiple_of(w), "summaries start on a 256-sample boundary");
    let count = (end - start).div_ceil(w);
    (0..count)
        .into_par_iter()
        .with_min_len(256)
        .map(|s| Summary::of(times, values, start + s * w, (start + (s + 1) * w).min(end)))
        .collect()
}

/// The next level up: each summary merges four of `level`.
pub fn coarser(level: &[Summary]) -> Vec<Summary> {
    level.chunks(4).map(|c| c[1..].iter().fold(c[0], |a, b| a.merge(b))).collect()
}

/// Every stored level from level-[`MIN_LEVEL`] summaries: the first entry is `level4`, and each next one is
/// [`coarser`] until a level has a single summary.
pub fn build_levels(level4: Vec<Summary>) -> Vec<Vec<Summary>> {
    let mut levels = vec![level4];
    while levels.last().is_some_and(|l| l.len() > 1) {
        let next = coarser(levels.last().unwrap());
        levels.push(next);
    }
    levels
}

/// The number of summaries at each stored level for a channel of `len` samples (level [`MIN_LEVEL`] first).
pub fn level_lengths(len: usize) -> Vec<usize> {
    let mut out = vec![len.div_ceil(span(MIN_LEVEL))];
    while *out.last().unwrap() > 1 {
        out.push(out.last().unwrap().div_ceil(4));
    }
    out
}

/// One channel's stored levels, borrowed (from memory-mapped cache files or from vectors).
#[derive(Clone, Debug)]
pub struct Pyramid<'a> {
    /// Samples in the channel.
    pub len: usize,
    /// `levels[j]` is level `MIN_LEVEL + j`.
    pub levels: Vec<&'a [Summary]>,
    /// Whether buckets past `len` (not summarized yet) are reduced from raw samples (true), or left empty, for
    /// views of a file still being built whose unread part is too large to read raw.
    pub raw_past: bool,
}

impl<'a> Pyramid<'a> {
    /// `levels` are the first levels of a channel of `len` samples: all of them, or fewer while a build is still
    /// making the coarse ones. `len` may be less than the channel's samples while a build is under way: views
    /// read raw samples past it.
    pub fn new(len: usize, levels: Vec<&'a [Summary]>) -> Self {
        assert!(!levels.is_empty(), "a pyramid has at least level 4");
        let want = level_lengths(len);
        assert!(levels.len() <= want.len(), "more levels than the samples make");
        assert!(levels.iter().zip(&want).all(|(l, &n)| l.len() >= n), "a level is too short for {len} samples");
        Self { len, levels, raw_past: true }
    }

    /// The summary at level `k` (stored) holding sample `i`.
    pub fn at(&self, k: u32, i: usize) -> &Summary {
        &self.levels[(k - MIN_LEVEL) as usize][i >> (2 * k)]
    }
    pub(crate) fn top(&self) -> u32 {
        MIN_LEVEL + self.levels.len() as u32 - 1
    }

    /// The coarsest level, at most `cap`, whose summary starting at sample `i` ends by `end`.
    fn fits_from(&self, i: usize, end: usize, cap: u32) -> Option<u32> {
        (MIN_LEVEL..=cap.min(self.top()))
            .rev()
            .find(|&k| i.is_multiple_of(span(k)) && (i + span(k)).min(self.len) <= end)
    }

    /// The coarsest level, at most `cap`, whose summary ending at sample `j` (exclusive) starts at or after
    /// `start`; with that summary's first sample.
    fn fits_to(&self, j: usize, start: usize, cap: u32) -> Option<(u32, usize)> {
        (MIN_LEVEL..=cap.min(self.top())).rev().find_map(|k| {
            let s = if j == self.len {
                ((j - 1) >> (2 * k)) << (2 * k)
            } else if j.is_multiple_of(span(k)) {
                j - span(k)
            } else {
                return None;
            };
            (s >= start).then_some((k, s))
        })
    }
}

/// Views whose pixel buckets hold fewer samples than this reduce them from raw samples: reading the summaries
/// and raw edges of such a bucket costs about as much.
pub const MIN_BUCKET_SAMPLES: usize = 1024;

/// One channel reduced for a viewport from its pyramid: point for point the same as [`crate::m4_breaking`] over
/// the raw samples, reading raw samples only at bucket edges where the summaries can't decide. The line breaks
/// before each sample in `breaks` (sorted, from the gap index).
pub fn m4<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    pyr: &Pyramid,
    vp: Viewport,
    breaks: &[usize],
) -> Trace {
    let (kind, emits) = reduce(times, values, pyr, vp, MIN_BUCKET_SAMPLES);
    to_trace(times, values, kind, &break_lines(emits, breaks, times), vp)
}

pub(crate) fn reduce<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    pyr: &Pyramid,
    vp: Viewport,
    min_bucket: usize,
) -> (TraceKind, Vec<Emit>) {
    assert_eq!(times.len(), values.len(), "times and values differ in length");
    assert!(pyr.len <= values.len(), "the pyramid is of another channel");
    reduce_with(times, vp, |start, end, bucket_t, out| {
        if end > pyr.len && !pyr.raw_past {
            push_bucket(out, None, bucket_t)
        } else if end - start < min_bucket || end > pyr.len {
            reduce_bucket(values, start, end, bucket_t, out)
        } else {
            let picks = first_real(pyr, values, start, end).map(|first| {
                let last = last_real(pyr, values, first, end).expect("a real sample exists");
                let (imin, imax) = extremes(pyr, values, first, last + 1);
                [first, imin, imax, last]
            });
            push_bucket(out, picks, bucket_t)
        }
    })
}

/// Exact statistics of samples `a..b` (not empty) as one [`Summary`]: whole summaries for the middle, raw
/// samples for the ragged edges (at most 255 on each side), and for samples past the pyramid when it allows them
/// ([`Pyramid::raw_past`]).
pub fn summary_of_range<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    pyr: &Pyramid,
    a: usize,
    b: usize,
) -> Summary {
    assert!(a < b && (b <= pyr.len || pyr.raw_past), "an empty or out-of-range span");
    // past the summarized part (a recording still growing), samples are read raw
    let end = b.min(pyr.len);
    let la = if a.is_multiple_of(span(MIN_LEVEL)) { a } else { next_boundary(a).min(b) };
    let mut acc = (a < la).then(|| Summary::of(times, values, a, la));
    let mut i = la;
    while i < end {
        let Some(k) = pyr.fits_from(i, end, u32::MAX) else { break };
        let s = pyr.at(k, i);
        acc = Some(acc.map_or(*s, |x| x.merge(s)));
        i = (i + span(k)).min(pyr.len);
    }
    if i < b {
        let edge = Summary::of(times, values, i, b);
        acc = Some(acc.map_or(edge, |x| x.merge(&edge)));
    }
    acc.expect("the span is not empty")
}

/// What Measure shows for a span: from [`summary_of_range`], with mean and standard deviation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// Samples that aren't NaN.
    pub count: u64,
    /// The smallest and largest values with their sample indexes (earliest on ties); None when `count` is 0.
    pub min: Option<(f64, usize)>,
    pub max: Option<(f64, usize)>,
    pub mean: f64,
    /// Population standard deviation, from the sums: its relative error grows with (mean / std)^2, about 1e-8 for
    /// a signal whose mean is 10,000 times its spread.
    pub std: f64,
}

impl Stats {
    /// The stats of `value × scale + offset`.
    pub fn transformed(self, scale: f64, offset: f64) -> Stats {
        if scale == 1.0 && offset == 0.0 {
            return self;
        }
        let f = |(v, i): (f64, usize)| (v * scale + offset, i);
        let (min, max) =
            if scale < 0.0 { (self.max.map(f), self.min.map(f)) } else { (self.min.map(f), self.max.map(f)) };
        Stats { count: self.count, min, max, mean: self.mean * scale + offset, std: self.std * scale.abs() }
    }
}

impl From<Summary> for Stats {
    fn from(s: Summary) -> Self {
        let n = s.count as f64;
        let mean = if s.count > 0 { s.sum / n } else { f64::NAN };
        let real = s.count > 0;
        Stats {
            count: s.count,
            min: real.then_some((s.min, s.min_idx as usize)),
            max: real.then_some((s.max, s.max_idx as usize)),
            mean,
            std: if real { (s.sum_sq / n - mean * mean).max(0.0).sqrt() } else { f64::NAN },
        }
    }
}

/// A coarse view: each pixel bucket from whole summaries, with values from the summaries, so raw samples are read
/// only within 256 samples of the view's two edges and of buckets small enough to reduce raw (and the one beyond
/// each edge). Every extreme is drawn at its true time. A boundary between two such buckets rounds up to one grid
/// for the view, summaries of the coarsest level at most a quarter of the smallest such bucket: it moves a sample at
/// most a quarter of a pixel into the next one, leaves none out, and reads no finer summary than the grid's
/// (finer ones only next to the exact edges).
/// Shown first, labelled "coarse", until [`m4`] answers. Buckets past a partly built pyramid are left empty.
pub fn m4_coarse<T: Times + ?Sized, S: Samples + ?Sized>(
    times: &T,
    values: &S,
    pyr: &Pyramid,
    vp: Viewport,
    breaks: &[usize],
) -> Trace {
    assert_eq!(times.len(), values.len(), "times and values differ in length");
    // A boundary between two buckets of whole summaries rounds up to the finest summaries' grid (the same answer for
    // both, so they share no sample and leave none out). The view's outer edges and the edges of buckets small
    // enough to reduce from raw samples stay exact.
    let bw = vp.bucket_ns();
    let (b_first, b_last) = (vp.t0.div_euclid(bw), (vp.t1 - 1).div_euclid(bw));
    // where each pixel starts (as reduce_with places them), once
    let starts: Vec<usize> = (b_first..=b_last + 1).map(|b| times.lower_bound(b.saturating_mul(bw))).collect();
    let size = |b: i64| starts[(b - b_first + 1) as usize] - starts[(b - b_first) as usize];
    let big = |b: i64| size(b) >= MIN_BUCKET_SAMPLES;
    // the grid: summaries of the coarsest level at most a quarter of the smallest pixel answered from summaries, so
    // a boundary moves at most a quarter pixel and every pixel is whole summaries of that level or coarser
    let smallest = (b_first..=b_last)
        .filter(|&b| big(b) && starts[(b - b_first + 1) as usize] <= pyr.len)
        .map(size)
        .min()
        .unwrap_or(MIN_BUCKET_SAMPLES);
    let grid = span((MIN_LEVEL..=pyr.top()).rev().find(|&k| 4 * span(k) <= smallest).unwrap_or(MIN_LEVEL));
    let fine = span(MIN_LEVEL);
    let snap = |i: usize, left: i64| {
        let inner = left >= b_first && left < b_last && big(left) && big(left + 1);
        if inner && i < pyr.len { i.next_multiple_of(grid).min(pyr.len) } else { i }
    };
    let (kind, emits) = reduce_with(times, vp, |start, end, bucket_t, out| {
        if end - start < MIN_BUCKET_SAMPLES {
            return reduce_bucket(values, start, end, bucket_t, out);
        }
        if end > pyr.len {
            return push_bucket(out, None, bucket_t);
        }
        let b = bucket_t.div_euclid(bw);
        let (a, b) = (snap(start, b - 1), snap(end, b));
        // at an exact edge: raw samples up to the finest summaries, then finer summaries up to the grid; between
        // grid points, whole summaries of the grid's level or coarser
        let a1 = a.next_multiple_of(fine).min(b);
        let b1 = if b == pyr.len { b } else { b / fine * fine }.max(a1);
        let mut m: Option<Summary> = None;
        let mut add = |s: Summary| m = Some(m.map_or(s, |p| p.merge(&s)));
        if a < a1 {
            add(Summary::of(times, values, a, a1));
        }
        let mut i = a1;
        while i < b1 {
            let k = pyr.fits_from(i, b1, pyr.top()).expect("summaries cover the grid");
            add(*pyr.at(k, i));
            i = (i + span(k)).min(pyr.len);
        }
        if b1 < b {
            add(Summary::of(times, values, b1, b));
        }
        let Some(m) = m.filter(|m| m.count > 0) else {
            return push_bucket(out, None, bucket_t);
        };
        let mut pts = [
            (!m.first.is_nan()).then_some((a, m.first)),
            Some((m.min_idx as usize, m.min)),
            Some((m.max_idx as usize, m.max)),
            (!m.last.is_nan()).then_some((b - 1, m.last)),
        ];
        pts.sort_by_key(|p| p.map(|(i, _)| i));
        let mut prev = usize::MAX;
        for (i, v) in pts.into_iter().flatten() {
            if i != prev {
                out.push(Emit::Known(i, v as f32));
                prev = i;
            }
        }
    });
    to_trace(times, values, kind, &break_lines(emits, breaks, times), vp)
}

/// The next multiple of 256 after `i`.
fn next_boundary(i: usize) -> usize {
    (i / span(MIN_LEVEL) + 1) * span(MIN_LEVEL)
}

/// Index of the first non-NaN sample in `a..b`.
fn first_real<S: Samples + ?Sized>(pyr: &Pyramid, values: &S, a: usize, b: usize) -> Option<usize> {
    let (mut i, mut cap) = (a, u32::MAX);
    while i < b {
        let Some(k) = pyr.fits_from(i, b, cap) else {
            let end = next_boundary(i).min(b);
            if let Some(p) = values.range(i, end).position(|v| !v.is_nan()) {
                return Some(i + p);
            }
            (i, cap) = (end, u32::MAX);
            continue;
        };
        let s = pyr.at(k, i);
        if s.count == 0 {
            (i, cap) = ((i + span(k)).min(pyr.len), u32::MAX);
        } else if !s.first.is_nan() {
            return Some(i);
        } else if k == MIN_LEVEL {
            let end = (i + span(k)).min(pyr.len);
            return values.range(i, end).position(|v| !v.is_nan()).map(|p| i + p);
        } else {
            cap = k - 1; // it is inside this summary: look at its parts
        }
    }
    None
}

/// Index of the last non-NaN sample in `a..b`.
fn last_real<S: Samples + ?Sized>(pyr: &Pyramid, values: &S, a: usize, b: usize) -> Option<usize> {
    let (mut j, mut cap) = (b, u32::MAX);
    while j > a {
        let Some((k, start)) = pyr.fits_to(j, a, cap) else {
            let start = ((j - 1) / span(MIN_LEVEL) * span(MIN_LEVEL)).max(a);
            if let Some(p) = values.range(start, j).rposition(|v| !v.is_nan()) {
                return Some(start + p);
            }
            (j, cap) = (start, u32::MAX);
            continue;
        };
        let s = pyr.at(k, start);
        if s.count == 0 {
            (j, cap) = (start, u32::MAX);
        } else if !s.last.is_nan() {
            return Some(j - 1);
        } else if k == MIN_LEVEL {
            return values.range(start, j).rposition(|v| !v.is_nan()).map(|p| start + p);
        } else {
            cap = k - 1;
        }
    }
    None
}

/// A best value so far and its sample index.
type Best = Option<(f64, usize)>;

/// Indexes of the earliest min and max of `a..b`, which holds at least one non-NaN sample.
///
/// Whole summaries cover the middle. At each ragged edge the 256-sample summary straddling it decides when its
/// own extreme lies inside the range (then it is the edge's extreme), or when it can't beat the middle; only
/// otherwise are the edge's raw samples read.
fn extremes<S: Samples + ?Sized>(pyr: &Pyramid, values: &S, a: usize, b: usize) -> (usize, usize) {
    let w = span(MIN_LEVEL);
    let la = if a.is_multiple_of(w) { a } else { next_boundary(a).min(b) };
    let (mut lo, mut hi): (Best, Best) = (None, None);
    let mut i = la;
    while i < b {
        let Some(k) = pyr.fits_from(i, b, u32::MAX) else { break };
        let s = pyr.at(k, i);
        if s.count > 0 {
            if lo.is_none_or(|(v, _)| s.min < v) {
                lo = Some((s.min, s.min_idx as usize));
            }
            if hi.is_none_or(|(v, _)| s.max > v) {
                hi = Some((s.max, s.max_idx as usize));
            }
        }
        i = (i + span(k)).min(pyr.len);
    }
    // the right edge comes after everything so far (ties keep the earlier), the left edge before it (ties move)
    if i < b {
        edge(pyr, values, i, b, false, &mut lo, &mut hi);
    }
    if a < la {
        edge(pyr, values, a, la, true, &mut lo, &mut hi);
    }
    (lo.expect("a real sample exists").1, hi.expect("a real sample exists").1)
}

/// Folds the ragged edge `p0..p1` (inside one 256-sample summary) into `lo` and `hi`. `earlier`: the edge comes
/// before every sample folded so far, so it wins ties.
fn edge<S: Samples + ?Sized>(
    pyr: &Pyramid,
    values: &S,
    p0: usize,
    p1: usize,
    earlier: bool,
    lo: &mut Best,
    hi: &mut Best,
) {
    let s = pyr.at(MIN_LEVEL, p0);
    if s.count == 0 {
        return;
    }
    let inside = |idx: u64| (p0..p1).contains(&(idx as usize));
    let beats_lo = |v: f64, cur: &Best| cur.is_none_or(|(c, _)| if earlier { v <= c } else { v < c });
    let beats_hi = |v: f64, cur: &Best| cur.is_none_or(|(c, _)| if earlier { v >= c } else { v > c });
    // the edge's own extreme is at least as extreme as the summary's, so the summary bounds what it can offer
    let (mut lo_cand, mut hi_cand): (Best, Best) = (None, None);
    let mut need_raw = false;
    if inside(s.min_idx) {
        lo_cand = Some((s.min, s.min_idx as usize));
    } else if beats_lo(s.min, lo) {
        need_raw = true;
    }
    if inside(s.max_idx) {
        hi_cand = Some((s.max, s.max_idx as usize));
    } else if beats_hi(s.max, hi) {
        need_raw = true;
    }
    if need_raw {
        let (mut rl, mut rh): (Best, Best) = (None, None);
        for (o, v) in values.range(p0, p1).enumerate() {
            if v.is_nan() {
                continue;
            }
            let x = v.to_f64();
            if rl.is_none_or(|(c, _)| x < c) {
                rl = Some((x, p0 + o));
            }
            if rh.is_none_or(|(c, _)| x > c) {
                rh = Some((x, p0 + o));
            }
        }
        lo_cand = lo_cand.or(rl);
        hi_cand = hi_cand.or(rh);
    }
    if let Some((v, i)) = lo_cand
        && beats_lo(v, lo)
    {
        *lo = Some((v, i));
    }
    if let Some((v, i)) = hi_cand
        && beats_hi(v, hi)
    {
        *hi = Some((v, i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RateTimes;
    use proptest::prelude::*;

    fn brute(times: &impl Times, v: &[f32], start: usize, end: usize) -> Summary {
        let real: Vec<usize> = (start..end).filter(|&i| !v[i].is_nan()).collect();
        let pick = |better: &dyn Fn(f32, f32) -> bool| {
            real.iter().copied().fold(None, |best: Option<usize>, i| match best {
                Some(b) if !better(v[i], v[b]) => Some(b),
                _ => Some(i),
            })
        };
        let imin = pick(&|a, b| a < b);
        let imax = pick(&|a, b| a > b);
        Summary {
            t_first: times.at(start),
            t_last: times.at(end - 1),
            min: imin.map_or(f64::NAN, |i| v[i] as f64),
            min_idx: imin.unwrap_or(start) as u64,
            max: imax.map_or(f64::NAN, |i| v[i] as f64),
            max_idx: imax.unwrap_or(start) as u64,
            first: v[start] as f64,
            last: v[end - 1] as f64,
            sum: real.iter().map(|&i| v[i] as f64).sum(),
            sum_sq: real.iter().map(|&i| (v[i] as f64).powi(2)).sum(),
            count: real.len() as u64,
        }
    }

    /// Equal, with NaN equal to NaN and sums within rounding.
    fn same(a: &Summary, b: &Summary) -> bool {
        let eq = |x: f64, y: f64| x == y || (x.is_nan() && y.is_nan());
        let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * (1.0 + x.abs().max(y.abs()));
        (a.t_first, a.t_last, a.min_idx, a.max_idx, a.count) == (b.t_first, b.t_last, b.min_idx, b.max_idx, b.count)
            && eq(a.min, b.min)
            && eq(a.max, b.max)
            && eq(a.first, b.first)
            && eq(a.last, b.last)
            && close(a.sum, b.sum)
            && close(a.sum_sq, b.sum_sq)
    }

    fn values() -> impl Strategy<Value = Vec<f32>> {
        proptest::collection::vec(
            prop_oneof![6 => (-8i32..8).prop_map(|v| v as f32), 1 => Just(f32::NAN), 1 => -1e6f32..1e6],
            1..5000,
        )
        .prop_flat_map(|v| {
            // sometimes long NaN runs, as in recordings with dropouts
            let n = v.len();
            (Just(v), 0..n, 0..n.min(1500)).prop_map(move |(mut v, at, len)| {
                v[at..(at + len).min(n)].fill(f32::NAN);
                v
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// Every summary of every level equals a brute-force summary of the samples it covers.
        #[test]
        fn levels_match_brute_force(v in values(), rate in 1u32..100_000) {
            let times = RateTimes::from_rate(-3_000_000, rate as f64, v.len());
            let levels = build_levels(summarize(&times, &v, 0, v.len()));
            prop_assert_eq!(levels.iter().map(Vec::len).collect::<Vec<_>>(), level_lengths(v.len()));
            for (j, level) in levels.iter().enumerate() {
                let w = span(MIN_LEVEL + j as u32);
                for (s, got) in level.iter().enumerate() {
                    let want = brute(&times, &v, s * w, ((s + 1) * w).min(v.len()));
                    prop_assert!(same(got, &want), "level {} summary {}: {:?} vs {:?}", MIN_LEVEL + j as u32, s, got, want);
                }
            }
        }

        /// The strided (generic) path summarizes the same as the slice path.
        #[test]
        fn generic_path_matches_slices(v in values()) {
            struct NoSlice<'a>(&'a [f32]);
            impl Samples for NoSlice<'_> {
                type Item = f32;
                fn len(&self) -> usize { self.0.len() }
                fn get(&self, i: usize) -> f32 { self.0[i] }
                fn range(&self, a: usize, b: usize) -> impl DoubleEndedIterator<Item = f32> + ExactSizeIterator + Clone {
                    self.0[a..b].iter().copied()
                }
            }
            let times = RateTimes::from_rate(0, 1000.0, v.len());
            let a = summarize(&times, &v, 0, v.len());
            let b = summarize(&times, &NoSlice(&v), 0, v.len());
            prop_assert!(a.iter().zip(&b).all(|(x, y)| same(x, y)));
        }
    }

    fn owned(times: &impl Times, v: &[f32]) -> Vec<Vec<Summary>> {
        build_levels(summarize(times, v, 0, v.len()))
    }

    fn view(levels: &[Vec<Summary>], len: usize) -> Pyramid<'_> {
        Pyramid::new(len, levels.iter().map(Vec::as_slice).collect())
    }

    /// Sorted times with repeats and jumps (gaps).
    fn column_times(n: usize) -> impl Strategy<Value = Vec<i64>> {
        proptest::collection::vec(prop_oneof![20 => 1i64..4, 2 => Just(0), 1 => 100i64..20_000], n).prop_map(|steps| {
            steps
                .iter()
                .scan(-5_000i64, |t, s| {
                    *t += s;
                    Some(*t)
                })
                .collect()
        })
    }

    fn vp_within(span: i64) -> impl Strategy<Value = Viewport> {
        (0.0f64..1.0, 0.0f64..1.0, 1u32..300).prop_map(move |(a, b, w)| {
            let (a, b) = (a.min(b), a.max(b));
            let t0 = -6_000 + (a * (span + 12_000) as f64) as i64;
            let t1 = (-6_000 + (b * (span + 12_000) as f64) as i64).max(t0 + 1);
            Viewport { t0, t1, width_px: w }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(600))]

        /// Answering from the pyramid gives exactly what M4 over the raw samples gives: every pixel bucket of
        /// every size goes through the pyramid here (min_bucket 1), which tests the straddling edges hardest.
        #[test]
        fn pyramid_m4_equals_raw_m4(
            (v, times) in values().prop_flat_map(|v| { let n = v.len(); (Just(v), column_times(n)) }),
            vp in vp_within(40_000),
        ) {
            let levels = owned(&times, &v);
            let pyr = view(&levels, v.len());
            prop_assert_eq!(reduce(&times, &v, &pyr, vp, 1), crate::m4::reduce(&times, &v, vp));
            prop_assert_eq!(reduce(&times, &v, &pyr, vp, MIN_BUCKET_SAMPLES), crate::m4::reduce(&times, &v, vp));
        }

        /// While a build is under way only a prefix (whole blocks, fine levels only) exists: buckets past it are
        /// reduced from raw samples, and the result is still exact.
        #[test]
        fn partial_pyramid_m4_equals_raw_m4(v in values(), vp in vp_within(5_000_000), cut in 0.0f64..1.0) {
            let times = crate::RateTimes::from_rate(0, 1000.0, v.len());
            let levels = owned(&times, &v);
            let block = span(MIN_LEVEL + 1);
            let len = ((v.len() as f64 * cut) as usize / block * block).max(block).min(v.len() / block * block);
            prop_assume!(len >= block);
            let pyr = Pyramid::new(len, levels.iter().take(2).map(Vec::as_slice).collect());
            let vp = Viewport { t0: vp.t0 * 1000, t1: vp.t1 * 1000, width_px: vp.width_px };
            prop_assert_eq!(reduce(&times, &v, &pyr, vp, 1), crate::m4::reduce(&times, &v, vp));
        }

        #[test]
        fn pyramid_m4_equals_raw_m4_at_a_rate(v in values(), rate in 1u32..20_000, vp in vp_within(1 << 40), scale in 1i64..2000) {
            let times = crate::RateTimes::from_rate(-1_000, rate as f64, v.len());
            let vp = Viewport { t0: vp.t0 * scale, t1: vp.t1 * scale, width_px: vp.width_px };
            let levels = owned(&times, &v);
            let pyr = view(&levels, v.len());
            prop_assert_eq!(reduce(&times, &v, &pyr, vp, 1), crate::m4::reduce(&times, &v, vp));
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(80))]

        /// Coarse views show only real samples at their true times, and hide no spike: every sample lies within
        /// the range drawn in its own pixel or a neighbour.
        #[test]
        fn coarse_views_are_real_samples_and_hide_nothing(
            v in proptest::collection::vec(prop_oneof![20 => -100f32..100.0, 1 => Just(f32::NAN), 1 => Just(500f32)], 4096..30_000),
            w in 1u32..40,
            a in 0.0f64..0.5,
            b in 0.5f64..1.0,
        ) {
            let n = v.len();
            let times = crate::RateTimes::from_rate(0, 1000.0, n);
            let levels = owned(&times, &v);
            let pyr = view(&levels, n);
            let (t0, t1) = ((a * n as f64) as i64 * 1_000_000, (b * n as f64) as i64 * 1_000_000 + 1);
            let vp = Viewport { t0, t1, width_px: w };
            let tr = m4_coarse(&times, &v, &pyr, vp, &[]);
            prop_assert!(tr.t.is_sorted(), "times go back");
            for (&t, &y) in tr.t.iter().zip(&tr.y) {
                if y.is_nan() { continue; }
                let i = (t / 1_000_000) as usize;
                prop_assert_eq!(times.at(i), t);
                prop_assert_eq!(v[i], y, "point at sample {} isn't that sample", i);
            }
            let bw = vp.bucket_ns();
            let bucket = |t: i64| t.div_euclid(bw);
            for (i, &x) in v.iter().enumerate().take(times.lower_bound(t1)).skip(times.lower_bound(t0)) {
                if x.is_nan() { continue; }
                let b = bucket(times.at(i));
                let near = tr.t.iter().zip(&tr.y).filter(|(t, y)| (bucket(**t) - b).abs() <= 1 && !y.is_nan());
                let (lo, hi) = near.fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), (_, &y)| (l.min(y), h.max(y)));
                prop_assert!(lo <= x && x <= hi, "sample {} ({}) outside [{}, {}] near its pixel", i, x, lo, hi);
            }
        }
    }

    /// Whole-range extremes over long flat runs with ties far apart: the earliest wins, as in raw M4.
    #[test]
    fn ties_go_to_the_earliest_sample() {
        let n = 300_000;
        let mut v = vec![0.0f32; n];
        for i in [1_000, 70_000, 250_001] {
            v[i] = 5.0;
        }
        v[123_457] = -5.0;
        v[299_999] = -5.0;
        let times = crate::RateTimes::from_rate(0, 1000.0, n);
        let levels = owned(&times, &v);
        let pyr = view(&levels, n);
        for (a, b) in [(0, n), (999, n), (1_001, n), (5, 260_000), (70_000, 123_458)] {
            let (imin, imax) = extremes(&pyr, &v, a, b);
            let (mn, mx) = v[a..b].iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
            let want_min = (a..b).find(|&i| v[i] == mn).unwrap();
            let want_max = (a..b).find(|&i| v[i] == mx).unwrap();
            assert_eq!((imin, imax), (want_min, want_max), "range {a}..{b}");
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// Stats of any span equal brute force over its samples.
        #[test]
        fn range_summaries_match_brute_force(v in values(), a in 0.0f64..1.0, b in 0.0f64..1.0) {
            let n = v.len();
            let (a, b) = ((a.min(b) * n as f64) as usize, ((a.max(b) * n as f64) as usize + 1).min(n));
            let times = crate::RateTimes::from_rate(0, 1000.0, n);
            let levels = owned(&times, &v);
            let got = summary_of_range(&times, &v, &view(&levels, n), a, b);
            let want = brute(&times, &v, a, b);
            prop_assert!(same(&got, &want), "{}..{}: {:?} vs {:?}", a, b, got, want);
        }
    }

    #[test]
    fn stats_from_sums() {
        let v = [1.0f32, f32::NAN, 3.0, 5.0];
        let times = crate::RateTimes::from_rate(0, 1.0, 4);
        let st = Stats::from(Summary::of(&times, &v[..], 0, 4));
        assert_eq!((st.count, st.min, st.max, st.mean), (3, Some((1.0, 0)), Some((5.0, 3)), 3.0));
        assert!((st.std - (8.0f64 / 3.0).sqrt()).abs() < 1e-12);
    }

    /// A view of a file being built leaves the unread part empty when it is too large to read raw.
    #[test]
    fn unread_buckets_stay_empty_when_raw_past_is_off() {
        let n = 1 << 16;
        let v: Vec<f32> = (0..n).map(|i| (i % 97) as f32).collect();
        let times = crate::RateTimes::from_rate(0, 1000.0, n);
        let levels = owned(&times, &v);
        let mut pyr = Pyramid::new(n / 4, levels.iter().take(2).map(Vec::as_slice).collect());
        pyr.raw_past = false;
        let vp = Viewport { t0: 0, t1: times.at(n - 1) + 1, width_px: 16 };
        let (_, emits) = reduce(&times, &v, &pyr, vp, 1);
        let read: Vec<usize> =
            emits.iter().filter_map(|e| if let Emit::Sample(i) = e { Some(*i) } else { None }).collect();
        assert!(read.iter().all(|&i| i < n / 4), "a raw sample past the built part was read");
        assert!(emits.iter().any(|e| matches!(e, Emit::Gap(_))), "the unread part is a gap");
    }

    #[test]
    fn pieces_join_like_the_whole() {
        let n = 10_000;
        let v: Vec<f32> = (0..n).map(|i| ((i * 7919) % 1000) as f32).collect();
        let times = RateTimes::from_rate(0, 100.0, n);
        let whole = summarize(&times, &v, 0, n);
        let mut parts = summarize(&times, &v, 0, 4096);
        parts.extend(summarize(&times, &v, 4096, n));
        assert_eq!(whole, parts);
    }
}
