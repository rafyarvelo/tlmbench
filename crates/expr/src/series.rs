//! A channel's values on a time grid, and the rule for combining grids.

use std::ops::Range;
use std::sync::Arc;

/// Sample times in seconds.
#[derive(Clone, Debug, PartialEq)]
pub enum Grid {
    /// Sample `i` is at `t0 + (start + i) / hz`: `start` is the index of the first sample in the whole channel, so a
    /// chunk's times are bit for bit those of the whole.
    Rate { hz: f64, t0: f64, start: usize },
    /// One time per sample.
    Stamps(Arc<Vec<f64>>),
}

impl Grid {
    pub fn time(&self, i: usize) -> f64 {
        match self {
            Grid::Rate { hz, t0, start } => t0 + (start + i) as f64 / hz,
            Grid::Stamps(t) => t[i],
        }
    }

    /// The grid of samples `r` of this one (indices from 0 again).
    pub fn slice(&self, r: Range<usize>) -> Grid {
        match self {
            Grid::Rate { hz, t0, start } => Grid::Rate { hz: *hz, t0: *t0, start: start + r.start },
            Grid::Stamps(t) => Grid::Stamps(Arc::new(t[r].to_vec())),
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
    /// Values at `hz` from time 0.
    pub fn rate(v: Vec<f64>, hz: f64) -> Self {
        Self::new(v, Grid::Rate { hz, t0: 0.0, start: 0 })
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

    /// Samples `r` of this series (clamped to its length).
    pub fn slice(&self, r: Range<usize>) -> Series {
        let r = r.start.min(self.len())..r.end.min(self.len());
        Series { v: self.v[r.clone()].to_vec(), grid: self.grid.slice(r), ch: self.ch.clone(), linear: self.linear }
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
