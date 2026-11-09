//! The Gauges view's frame: one call answers every widget's value, its min and max since the start of the view, and
//! where its current state began, so a frame is one round trip however many widgets a pane shows.

use std::sync::Arc;

use serde::Serialize;
use tlmbench_core::Timestamp;

use crate::Opened;

/// Samples read at a time while a channel's state runs are indexed.
const SLICE: usize = 1 << 20;
/// A state index with more runs than this isn't kept: the channel isn't a state, so runs are found by looking back.
const MAX_RUNS: usize = 4 << 20;
/// How far a look back for the start of a run goes, for a channel without an index.
const LOOK_BACK: usize = 4 << 20;

/// What one widget reads of a channel.
#[derive(Clone, Copy, Debug, Default)]
pub struct WidgetReq {
    pub channel: usize,
    /// The lowest and highest value from the start of the view to the time read.
    pub minmax: bool,
    /// The time the current run of equal values began (state lights, timelines, flags).
    pub run: bool,
}

/// One widget's answer; each part is None when it wasn't asked for, isn't known yet, or the sample is missing.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct WidgetOut {
    pub value: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Nanoseconds since the epoch, as a string like every time on the wire.
    pub run_start: Option<Timestamp>,
}

/// A frame: the sample read (the one nearest the time asked) and an answer per request, in order.
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub index: usize,
    pub t: Timestamp,
    pub widgets: Vec<WidgetOut>,
}

impl Opened {
    /// Every widget's answer at `t` with the view `[view.0, view.1)`. Min and max come from the exact statistics, so
    /// they wait for the pyramid (None until it is built); values and run starts don't.
    pub fn widgets_frame(&self, t: Timestamp, view: (Timestamp, Timestamp), reqs: &[WidgetReq]) -> Option<Frame> {
        let chans: Vec<usize> = reqs.iter().map(|r| r.channel).collect();
        let (index, ts, values) = self.values_at(&chans, t)?;
        let mut out: Vec<WidgetOut> =
            values.iter().map(|&v| WidgetOut { value: (!v.is_nan()).then_some(v), ..Default::default() }).collect();
        // min and max, from the start of the view up to and including the sample read
        let mm: Vec<usize> = (0..reqs.len()).filter(|&i| reqs[i].minmax).collect();
        if !mm.is_empty() {
            let end = ts.min(view.1.saturating_sub(1)).saturating_add(1);
            let start = view.0.min(ts);
            let ch: Vec<usize> = mm.iter().map(|&i| reqs[i].channel).collect();
            if let Some(stats) = self.stats(&ch, start, end.max(start + 1)) {
                for (&i, s) in mm.iter().zip(stats) {
                    out[i].min = s.min.map(|m| m.0);
                    out[i].max = s.max.map(|m| m.0);
                }
            }
        }
        for (i, r) in reqs.iter().enumerate().filter(|(_, r)| r.run) {
            out[i].run_start = self.run_start(r.channel, index);
        }
        Some(Frame { index, t: ts, widgets: out })
    }

    /// The time the run of equal values that sample `i` of `channel` is in began.
    fn run_start(&self, channel: usize, i: usize) -> Option<Timestamp> {
        if !self.live
            && let Some(runs) = self.run_index(channel)
        {
            let k = runs.partition_point(|&s| s <= i).saturating_sub(1);
            return Some(self.rec.time_at(runs.get(k).copied().unwrap_or(0)));
        }
        // no index (live, or a channel with too many runs): look back from the sample
        let lo = i.saturating_sub(LOOK_BACK);
        let (scale, offset) = self.rec.scale_offset(channel);
        let col = self.rec.read(channel, lo, i + 1).to_f64(scale, offset);
        let cur = *col.last()?;
        let same = |v: f64| v == cur || (v.is_nan() && cur.is_nan());
        let back = col.iter().rev().take_while(|&&v| same(v)).count();
        Some(self.rec.time_at(i + 1 - back))
    }

    /// The first sample of each run of equal values of `channel`, built once (None when there are too many runs).
    fn run_index(&self, channel: usize) -> Option<Arc<Vec<usize>>> {
        if let Some(r) = self.runs.lock().unwrap().get(&channel) {
            return r.clone();
        }
        let n = self.rec.info().samples;
        let (scale, offset) = self.rec.scale_offset(channel);
        let mut starts = Vec::new();
        let mut prev: Option<f64> = None;
        let mut a = 0;
        let mut ok = true;
        'scan: while a < n {
            let b = (a + SLICE).min(n);
            for (k, v) in self.rec.read(channel, a, b).to_f64(scale, offset).into_iter().enumerate() {
                if prev.is_none_or(|p| p != v && !(p.is_nan() && v.is_nan())) {
                    if starts.len() >= MAX_RUNS {
                        ok = false;
                        break 'scan;
                    }
                    starts.push(a + k);
                }
                prev = Some(v);
            }
            a = b;
        }
        let r = ok.then(|| Arc::new(starts));
        self.runs.lock().unwrap().insert(channel, r.clone());
        r
    }
}
