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
    fn fractional_rates_round_to_microhertz() {
        let r = RateTimes::from_rate(0, 0.5, 4);
        assert_eq!(r.at(3), 6_000_000_000);
    }
}
