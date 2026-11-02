//! The gap index: time jumps between consecutive samples, so lines break across dropouts instead of drawing
//! straight through them.
//!
//! A jump is a gap when it exceeds N × the median sample interval, where N is a per-channel setting (default 4,
//! at least [`MIN_FACTOR`]). The index keeps every jump over [`MIN_FACTOR`] × the median, so changing N needs no
//! rebuild: [`GapIndex::breaks`] filters it. Channels of one recording share their times, so one index serves
//! them all. Fixed-rate times have no jumps.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::Times;

/// The smallest gap factor N a channel can use.
pub const MIN_FACTOR: f64 = 2.0;
/// The default gap factor N.
pub const DEFAULT_FACTOR: f64 = 4.0;
/// The median is taken over at most this many intervals, spread evenly through the file.
const MEDIAN_SAMPLE: usize = 1 << 20;

/// A jump in time: sample `index` comes `dt_ns` after sample `index - 1`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct Jump {
    pub index: u64,
    pub dt_ns: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GapIndex {
    /// The median interval between samples (of the positive ones, when most are repeats).
    pub median_ns: i64,
    /// Every jump over [`MIN_FACTOR`] × the median, by index.
    pub jumps: Vec<Jump>,
}

impl GapIndex {
    /// Scans `times` (on several threads).
    pub fn build<T: Times + ?Sized>(times: &T) -> Self {
        let n = times.len();
        if n < 3 {
            return Self::default();
        }
        let dt = |i: usize| times.at(i) - times.at(i - 1);
        let picks = (n - 1).min(MEDIAN_SAMPLE);
        let mut sample: Vec<i64> = (0..picks).map(|k| dt(1 + k * (n - 1) / picks)).collect();
        let mid = sample.len() / 2;
        let mut median = *sample.select_nth_unstable(mid).1;
        if median == 0 {
            sample.retain(|&d| d > 0);
            if sample.is_empty() {
                return Self::default();
            }
            let mid = sample.len() / 2;
            median = *sample.select_nth_unstable(mid).1;
        }
        let threshold = (median as f64 * MIN_FACTOR) as i64;
        const CHUNK: usize = 1 << 16;
        let jumps = (1..n)
            .step_by(CHUNK)
            .collect::<Vec<_>>()
            .into_par_iter()
            .flat_map_iter(|a| {
                (a..(a + CHUNK).min(n)).filter_map(move |i| {
                    let d = dt(i);
                    (d > threshold).then_some(Jump { index: i as u64, dt_ns: d })
                })
            })
            .collect();
        Self { median_ns: median, jumps }
    }

    /// Indexes of the samples that start after a gap, for gap factor `factor` (at least [`MIN_FACTOR`]), in order.
    pub fn breaks(&self, factor: f64) -> Vec<usize> {
        let limit = self.median_ns as f64 * factor.max(MIN_FACTOR);
        self.jumps.iter().filter(|j| j.dt_ns as f64 > limit).map(|j| j.index as usize).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn finds_every_jump_over_the_threshold(
            steps in proptest::collection::vec(prop_oneof![30 => Just(10i64), 3 => 8i64..13, 1 => Just(0), 1 => 15i64..400], 2..3000),
            factor in 2.0f64..10.0,
        ) {
            let times: Vec<i64> = steps.iter().scan(0i64, |t, s| { *t += s; Some(*t) }).collect();
            let g = GapIndex::build(&times);
            let mut d: Vec<i64> = times.windows(2).map(|w| w[1] - w[0]).collect();
            if d.len() >= 2 {
                d.sort();
                let mut median = d[d.len() / 2];
                if median == 0 {
                    let pos: Vec<i64> = d.iter().copied().filter(|&x| x > 0).collect();
                    median = pos.get(pos.len() / 2).copied().unwrap_or(0);
                }
                prop_assert_eq!(g.median_ns, median);
                let want: Vec<usize> =
                    (1..times.len()).filter(|&i| (times[i] - times[i - 1]) as f64 > median as f64 * factor).collect();
                prop_assert_eq!(g.breaks(factor), want);
            }
        }
    }

    #[test]
    fn regular_times_have_no_gaps() {
        let times = crate::RateTimes::from_rate(0, 1000.0, 100_000);
        let g = GapIndex::build(&times);
        assert_eq!((g.median_ns, g.jumps.len()), (1_000_000, 0));
    }
}
