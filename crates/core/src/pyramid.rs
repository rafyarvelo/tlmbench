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

use crate::m4::min_max;
use crate::{Samples, Times, Value};

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
        let (mn, mx) = match values.as_slice() {
            Some(sl) => min_max(&sl[start + off..end], v0),
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
        match values.as_slice() {
            Some(sl) => {
                let (chunks, rest) = sl[start..end].as_chunks::<4>();
                for c in chunks {
                    (0..4).for_each(|k| acc(k, c[k]));
                }
                rest.iter().for_each(|&v| acc(0, v));
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
}

impl<'a> Pyramid<'a> {
    pub fn new(len: usize, levels: Vec<&'a [Summary]>) -> Self {
        debug_assert_eq!(levels.iter().map(|l| l.len()).collect::<Vec<_>>(), level_lengths(len));
        Self { len, levels }
    }

    /// The summary at level `k` (stored) holding sample `i`.
    pub fn at(&self, k: u32, i: usize) -> &Summary {
        &self.levels[(k - MIN_LEVEL) as usize][i >> (2 * k)]
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
