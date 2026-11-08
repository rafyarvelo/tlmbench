//! Samples a table filter leaves out ("Apply to charts"): a sorted list of excluded runs of sample indexes. Excluded
//! samples are missing everywhere the mask is applied: lines break at them, they never become a min or max, and
//! statistics skip them.

use std::ops::Range;

use crate::value::Samples;

/// Excluded runs of sample indexes: sorted, non-empty, disjoint and not touching.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    runs: Vec<Range<usize>>,
}

impl Mask {
    /// The samples of `first..first + count` that are not in `kept` (sorted offsets from `first`) are excluded.
    pub fn from_kept(first: usize, count: usize, kept: &[u32]) -> Mask {
        let mut runs: Vec<Range<usize>> = Vec::new();
        let mut at = 0usize;
        for &k in kept {
            let k = k as usize;
            if k > at {
                runs.push(first + at..first + k);
            }
            at = k + 1;
        }
        if at < count {
            runs.push(first + at..first + count);
        }
        Mask { runs }
    }

    /// A mask from runs in any order: empty ones dropped, overlapping and touching ones joined.
    pub fn from_runs(mut runs: Vec<Range<usize>>) -> Mask {
        runs.retain(|r| r.start < r.end);
        runs.sort_by_key(|r| r.start);
        let mut out: Vec<Range<usize>> = Vec::with_capacity(runs.len());
        for r in runs {
            match out.last_mut() {
                Some(l) if r.start <= l.end => l.end = l.end.max(r.end),
                _ => out.push(r),
            }
        }
        Mask { runs: out }
    }

    /// Everything either mask excludes.
    pub fn union(&self, other: &Mask) -> Mask {
        Mask::from_runs(self.runs.iter().chain(&other.runs).cloned().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    pub fn runs(&self) -> &[Range<usize>] {
        &self.runs
    }

    /// Samples excluded in total.
    pub fn excluded(&self) -> usize {
        self.runs.iter().map(|r| r.len()).sum()
    }

    /// Whether sample `i` is excluded.
    #[inline]
    pub fn hides(&self, i: usize) -> bool {
        let k = self.runs.partition_point(|r| r.end <= i);
        self.runs.get(k).is_some_and(|r| r.start <= i)
    }

    /// The runs of `a..b` that are kept, in order.
    pub fn kept(&self, a: usize, b: usize) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut at = a;
        for r in &self.runs[self.runs.partition_point(|r| r.end <= a)..] {
            if r.start >= b {
                break;
            }
            if r.start > at {
                out.push(at..r.start);
            }
            at = at.max(r.end);
        }
        if at < b {
            out.push(at..b);
        }
        out
    }

    /// The first kept sample after each excluded run, within `lo..hi`: where a line must break.
    pub fn breaks(&self, lo: usize, hi: usize) -> Vec<usize> {
        self.runs.iter().map(|r| r.end).filter(|&e| e > lo && e < hi).collect()
    }
}

/// A channel's stored values as f64 with the mask's samples missing (NaN).
#[derive(Debug)]
pub struct Masked<'a, S: ?Sized> {
    pub values: &'a S,
    pub mask: &'a Mask,
}

impl<S: Samples + ?Sized> Samples for Masked<'_, S> {
    type Item = f64;

    fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn get(&self, i: usize) -> f64 {
        if self.mask.hides(i) { f64::NAN } else { crate::Value::to_f64(self.values.get(i)) }
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = f64> + ExactSizeIterator + Clone {
        (start..end).map(move |i| self.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_kept_excludes_the_rest() {
        let m = Mask::from_kept(100, 10, &[1, 2, 5, 9]);
        assert_eq!(m.runs(), [100..101, 103..105, 106..109]);
        assert_eq!(m.excluded(), 6);
        assert!(Mask::from_kept(0, 3, &[0, 1, 2]).is_empty());
        assert_eq!(Mask::from_kept(5, 3, &[]).runs().to_vec(), vec![5..8]);
    }

    #[test]
    fn hides_and_kept() {
        let m = Mask::from_runs(vec![10..20, 5..8, 18..25, 30..30]);
        assert_eq!(m.runs(), [5..8, 10..25]);
        assert!(m.hides(5) && m.hides(7) && !m.hides(8) && !m.hides(9) && m.hides(10) && m.hides(24) && !m.hides(25));
        assert_eq!(m.kept(0, 30), [0..5, 8..10, 25..30]);
        assert_eq!(m.kept(6, 12).len(), 1);
        assert_eq!(m.kept(6, 12)[0], 8..10);
        assert_eq!(m.kept(12, 20), Vec::<Range<usize>>::new());
        assert_eq!(m.breaks(0, 30), [8, 25]);
        assert_eq!(m.breaks(9, 25), Vec::<usize>::new(), "an end at the edge isn't inside");
    }

    #[test]
    fn union_joins() {
        let a = Mask::from_runs(vec![0..5, 10..12]);
        let b = Mask::from_runs(vec![4..8, 12..14]);
        assert_eq!(a.union(&b).runs(), [0..8, 10..14]);
    }

    #[test]
    fn masked_samples_are_missing() {
        let data: Vec<i32> = (0..10).collect();
        let mask = Mask::from_runs(vec![2..4, 3..4]);
        let m = Masked { values: &data, mask: &mask };
        let got: Vec<f64> = m.range(0, 6).collect();
        assert_eq!(got[..2], [0.0, 1.0]);
        assert!(got[2].is_nan() && got[3].is_nan());
        assert_eq!(got[4..], [4.0, 5.0]);
    }
}
