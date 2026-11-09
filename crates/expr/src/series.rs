//! A channel's values on a time grid, and the rule for combining grids.

use std::sync::Arc;

/// Sample times in seconds.
#[derive(Clone, Debug, PartialEq)]
pub enum Grid {
    /// Sample `i` is at `i / hz`.
    Rate(f64),
    Stamps(Arc<Vec<f64>>),
}

impl Grid {
    pub fn time(&self, i: usize) -> f64 {
        match self {
            Grid::Rate(hz) => i as f64 / hz,
            Grid::Stamps(t) => t[i],
        }
    }
    /// Seconds from sample `i` to the next (the previous one for the last sample, `fallback` for a single sample).
    pub fn step(&self, i: usize, len: usize) -> f64 {
        match self {
            Grid::Rate(hz) => 1.0 / hz,
            Grid::Stamps(t) if len > 1 => {
                if i + 1 < len {
                    t[i + 1] - t[i]
                } else {
                    t[i] - t[i - 1]
                }
            }
            Grid::Stamps(_) => 0.0,
        }
    }
}

/// Values (NaN is a gap) on a grid. `ch` names the channel it came straight from, for `past_warn` and labels.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub v: Vec<f64>,
    pub grid: Grid,
    pub ch: Option<String>,
    /// Resampled linearly (not held) when combined with a faster series: `interp(x)`.
    pub linear: bool,
}

impl Series {
    pub fn new(v: Vec<f64>, grid: Grid) -> Self {
        Self { v, grid, ch: None, linear: false }
    }
    pub fn rate(v: Vec<f64>, hz: f64) -> Self {
        Self::new(v, Grid::Rate(hz))
    }
    pub fn len(&self) -> usize {
        self.v.len()
    }
    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
    pub fn time(&self, i: usize) -> f64 {
        self.grid.time(i)
    }

    /// This series on `grid`: each target sample takes the last sample at or before its time (the first sample before
    /// the series starts), or interpolates when `linear`.
    pub fn onto(&self, grid: &Grid, len: usize) -> Vec<f64> {
        if &self.grid == grid && self.len() == len {
            return self.v.clone();
        }
        let n = self.len();
        let mut out = Vec::with_capacity(len);
        let mut k = 0usize;
        for i in 0..len {
            let t = grid.time(i);
            while k + 1 < n && self.time(k + 1) <= t {
                k += 1;
            }
            if n == 0 {
                out.push(f64::NAN);
            } else if self.linear && k + 1 < n && t > self.time(k) {
                let (t0, t1) = (self.time(k), self.time(k + 1));
                out.push(self.v[k] + (t - t0) / (t1 - t0) * (self.v[k + 1] - self.v[k]));
            } else if self.linear && k + 1 == n && t > self.time(k) {
                out.push(f64::NAN);
            } else {
                out.push(self.v[k]);
            }
        }
        out
    }
}
