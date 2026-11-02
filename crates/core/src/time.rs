//! Time base. Every sample's time is an `i64` count of nanoseconds (since the epoch, or since the start for
//! relative logs). Channels either store a timestamp per sample (a column) or have a fixed rate, where
//! `t = t0 + i / rate` is computed on demand and nothing is stored per sample.

/// Nanoseconds since the epoch (or since the start of a relative recording).
pub type Timestamp = i64;

/// Sample times of one channel, in non-decreasing order.
pub trait Times: Sync {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Time of sample `i` (`i < len()`).
    fn at(&self, i: usize) -> Timestamp;

    /// Index of the first sample with time `>= t` (`len()` if none).
    fn lower_bound(&self, t: Timestamp) -> usize;
}

impl Times for [Timestamp] {
    fn len(&self) -> usize {
        <[Timestamp]>::len(self)
    }

    fn at(&self, i: usize) -> Timestamp {
        self[i]
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        self.partition_point(|&x| x < t)
    }
}

impl Times for Vec<Timestamp> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn at(&self, i: usize) -> Timestamp {
        self[i]
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        self.as_slice().lower_bound(t)
    }
}

/// Times of a fixed-rate channel: `t(i) = t0 + floor(i * period_num / period_den)` nanoseconds.
///
/// The period is a ratio so common rates are exact (10 kHz is 1e9/10000 ns) and times stay exact at any index,
/// which an `f64` period would not be past about 2^53 ns of offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateTimes {
    pub t0: Timestamp,
    pub period_num: u64,
    pub period_den: u64,
    pub len: usize,
}

impl RateTimes {
    /// A channel sampled at `rate_hz` from `t0`. Integer rates are exact; others are rounded to a period
    /// with microhertz resolution.
    pub fn from_rate(t0: Timestamp, rate_hz: f64, len: usize) -> Self {
        assert!(rate_hz.is_finite() && rate_hz > 0.0, "sample rate must be positive");
        let (period_num, period_den) = if rate_hz.fract() == 0.0 && rate_hz <= u64::MAX as f64 {
            (1_000_000_000, rate_hz as u64)
        } else {
            (1_000_000_000_000_000, (rate_hz * 1e6).round().max(1.0) as u64)
        };
        Self { t0, period_num, period_den, len }
    }
}

impl Times for RateTimes {
    fn len(&self) -> usize {
        self.len
    }

    fn at(&self, i: usize) -> Timestamp {
        let offset = i as i128 * self.period_num as i128 / self.period_den as i128;
        (self.t0 as i128 + offset) as Timestamp
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        // Smallest i with floor(i * num / den) >= t - t0, i.e. i = ceil((t - t0) * den / num).
        let d = t as i128 - self.t0 as i128;
        if d <= 0 {
            return 0;
        }
        let (num, den) = (self.period_num as i128, self.period_den as i128);
        let i = (d * den + num - 1) / num;
        i.min(self.len as i128) as usize
    }
}

/// The time of every [`CHUNK_SAMPLES`](crate::chunks::CHUNK_SAMPLES)-th sample of a time column: narrows a search for
/// a time to one chunk, so a search of a cold file touches a few pages near the answer instead of pages spread
/// across the whole file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimeIndex(pub Vec<Timestamp>);

impl TimeIndex {
    pub fn build<T: Times + ?Sized>(times: &T) -> Self {
        Self((0..times.len()).step_by(crate::chunks::CHUNK_SAMPLES).map(|i| times.at(i)).collect())
    }
}

/// Times searched through a [`TimeIndex`] of them.
#[derive(Debug)]
pub struct Indexed<'a, T: ?Sized> {
    pub times: &'a T,
    pub index: &'a TimeIndex,
}

impl<T: Times + ?Sized> Times for Indexed<'_, T> {
    fn len(&self) -> usize {
        self.times.len()
    }

    #[inline]
    fn at(&self, i: usize) -> Timestamp {
        self.times.at(i)
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        // chunk k starts at or after `t`: the answer is in chunk k - 1, or is chunk k's first sample
        let k = self.index.0.partition_point(|&x| x < t);
        let Some(prev) = k.checked_sub(1) else { return 0 };
        let c = crate::chunks::CHUNK_SAMPLES;
        let (mut lo, mut hi) = (prev * c, (k * c).min(self.times.len()));
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.times.at(mid) < t {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_times_are_exact_for_integer_rates() {
        let r = RateTimes::from_rate(1_700_000_000_000_000_000, 10_000.0, 1_000_000_000);
        assert_eq!(r.at(0), 1_700_000_000_000_000_000);
        assert_eq!(r.at(999_999_999), 1_700_000_000_000_000_000 + 99_999_999_900_000);
    }

    #[test]
    fn rate_lower_bound_matches_linear_search() {
        let r = RateTimes::from_rate(-5, 3.0, 50); // period 333_333_333.33 ns
        for t in [-10, -5, 0, 1, 333_333_333, 333_333_334, 16_000_000_000, 17_000_000_000] {
            let expect = (0..r.len).find(|&i| r.at(i) >= t).unwrap_or(r.len);
            assert_eq!(r.lower_bound(t), expect, "t = {t}");
        }
    }

    #[test]
    fn an_indexed_search_finds_what_a_full_one_does() {
        let c = crate::chunks::CHUNK_SAMPLES as i64;
        // repeats across chunk boundaries, and a jump
        let times: Vec<Timestamp> =
            (0..3 * c + 77).map(|i| i / 3 * 10 + if i > 2 * c { 1_000_000 } else { 0 }).collect();
        let index = TimeIndex::build(&times);
        assert_eq!(index.0.len(), 4);
        let ix = Indexed { times: &times, index: &index };
        let probes = times.iter().flat_map(|&t| [t - 1, t, t + 1]).step_by(97).chain([i64::MIN, -5, 0, i64::MAX]);
        for t in probes {
            assert_eq!(ix.lower_bound(t), times.lower_bound(t), "t = {t}");
        }
        let empty: Vec<Timestamp> = vec![];
        assert_eq!(Indexed { times: &empty, index: &TimeIndex::build(&empty) }.lower_bound(5), 0);
    }

    #[test]
    fn fractional_rates_round_to_microhertz() {
        let r = RateTimes::from_rate(0, 0.5, 4);
        assert_eq!(r.at(3), 6_000_000_000);
    }
}
