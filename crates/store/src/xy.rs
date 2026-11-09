//! The XY plot's query: samples of one channel plotted against another over a time span, reduced to a pixel grid.
//!
//! Channels of a file share their times, so pairing is by sample index (no resampling, gaps stay gaps). Consecutive
//! samples that fall in the same grid cell are one point, so a slow track over millions of samples is a few
//! thousand points and a still stretch is one.

use serde::Serialize;
use tlmbench_core::Timestamp;

use crate::Opened;

/// Samples read at a time.
const SLICE: usize = 1 << 18;

/// What an XY pane asks for.
#[derive(Clone, Debug)]
pub struct XyReq {
    pub x: usize,
    pub ys: Vec<usize>,
    pub t0: Timestamp,
    pub t1: Timestamp,
    /// The visible area `[x0, x1, y0, y1]` when the pane is zoomed: only points in it, and the neighbours that
    /// connect them to the rest of the line, are returned. None: everything in the span.
    pub rect: Option<[f64; 4]>,
    /// The grid points are reduced on, in cells: about twice the pane's size in pixels.
    pub grid: (u32, u32),
    /// Most points a series returns; past it every n-th point is kept and the reply says it is sampled.
    pub max_points: usize,
}

/// One Y channel's points: a NaN pair breaks the line (a gap in either channel).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct XySeries {
    pub channel: usize,
    pub t: Vec<Timestamp>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Xy {
    /// `[x0, x1, y0, y1]` of every sample pair in the span (None without any), for the auto range.
    pub range: Option<[f64; 4]>,
    pub series: Vec<XySeries>,
    /// Some series had more points than `max_points`.
    pub sampled: bool,
}

impl Opened {
    pub fn xy_points(&self, q: &XyReq) -> Xy {
        let (lo, hi) = (self.rec.lower_bound(q.t0), self.rec.lower_bound(q.t1));
        let mut out = Xy {
            series: q.ys.iter().map(|&c| XySeries { channel: c, ..Default::default() }).collect(),
            ..Default::default()
        };
        if hi <= lo {
            return out;
        }
        let mask = self.mask.read().unwrap().clone();
        let read = |c: usize, a: usize, b: usize| {
            let (scale, offset) = self.rec.scale_offset(c);
            let mut v = self.rec.read(c, a, b).to_f64(scale, offset);
            if !mask.is_empty() {
                for (k, x) in v.iter_mut().enumerate() {
                    if mask.hides(a + k) {
                        *x = f64::NAN;
                    }
                }
            }
            v
        };
        // pass 1: the range of the pairs
        let mut r = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
        let mut a = lo;
        while a < hi {
            let b = (a + SLICE).min(hi);
            let xs = read(q.x, a, b);
            for &yc in &q.ys {
                let ys = read(yc, a, b);
                for (&x, &y) in xs.iter().zip(&ys) {
                    if x.is_finite() && y.is_finite() {
                        r = [r[0].min(x), r[1].max(x), r[2].min(y), r[3].max(y)];
                    }
                }
            }
            a = b;
        }
        if !r[0].is_finite() {
            return out;
        }
        out.range = Some(r);
        // pass 2: reduce on the grid of the visible area (the data's own range when not zoomed)
        let rect = q.rect.unwrap_or(r);
        let (gw, gh) = (q.grid.0.max(1) as f64, q.grid.1.max(1) as f64);
        let cell = |x: f64, y: f64| {
            let cx = ((x - rect[0]) / (rect[1] - rect[0]).max(f64::MIN_POSITIVE) * gw).floor();
            let cy = ((y - rect[2]) / (rect[3] - rect[2]).max(f64::MIN_POSITIVE) * gh).floor();
            (cx as i64, cy as i64)
        };
        let inside =
            |x: f64, y: f64| q.rect.is_none() || (x >= rect[0] && x <= rect[1] && y >= rect[2] && y <= rect[3]);
        for (s, &yc) in out.series.iter_mut().zip(&q.ys) {
            let mut last: Option<(i64, i64)> = None;
            let mut was_in = false;
            let mut held: Option<(Timestamp, f64, f64)> = None;
            let mut a = lo;
            while a < hi {
                let b = (a + SLICE).min(hi);
                let (xs, ys, ts) = (read(q.x, a, b), read(yc, a, b), self.rec.read_times(a, b));
                for k in 0..xs.len() {
                    let (x, y) = (xs[k], ys[k]);
                    if !(x.is_finite() && y.is_finite()) {
                        if s.x.last().is_some_and(|v| !v.is_nan()) {
                            s.t.push(ts[k]);
                            s.x.push(f64::NAN);
                            s.y.push(f64::NAN);
                        }
                        last = None;
                        was_in = false;
                        held = None;
                        continue;
                    }
                    let c = cell(x, y);
                    if inside(x, y) {
                        if let Some((t, hx, hy)) = held.take() {
                            s.t.push(t);
                            s.x.push(hx);
                            s.y.push(hy);
                        }
                        if last != Some(c) {
                            s.t.push(ts[k]);
                            s.x.push(x);
                            s.y.push(y);
                        }
                        was_in = true;
                    } else {
                        if was_in {
                            s.t.push(ts[k]);
                            s.x.push(x);
                            s.y.push(y);
                        } else {
                            held = Some((ts[k], x, y));
                        }
                        was_in = false;
                    }
                    last = Some(c);
                }
                a = b;
            }
            if s.x.len() > q.max_points && q.max_points > 0 {
                let step = s.x.len().div_ceil(q.max_points);
                let keep = |i: usize| i.is_multiple_of(step) || s.x[i].is_nan() || i + 1 == s.x.len();
                let idx: Vec<usize> = (0..s.x.len()).filter(|&i| keep(i)).collect();
                s.t = idx.iter().map(|&i| s.t[i]).collect();
                s.y = idx.iter().map(|&i| s.y[i]).collect();
                s.x = idx.iter().map(|&i| s.x[i]).collect();
                out.sampled = true;
            }
        }
        out
    }
}
