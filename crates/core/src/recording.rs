//! An opened recording: what it holds, and its channels reduced for a viewport.

use std::fmt::Debug;
use std::path::PathBuf;

use serde::Serialize;

use crate::gaps::GapIndex;
use crate::pyramid::{self, Pyramid, Stats, Summary};
use crate::{Samples, Times, Timestamp, Trace, Value, Viewport};

/// Where a recording's sample times come from.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TimeSource {
    /// A timestamp per sample, from this column or field.
    Column { name: String },
    /// A fixed sample rate: nothing is stored per sample.
    Rate { hz: f64 },
    /// No time at all: the x axis is the sample number (one per second of "time" until a rate is given).
    Index,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChannelInfo {
    pub name: String,
    pub unit: Option<String>,
    /// Stored type: f32, f64, i16, …
    pub dtype: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RecordingInfo {
    pub path: PathBuf,
    /// Format name, e.g. "tlmbench reference" or "CSV".
    pub format: String,
    pub samples: usize,
    pub time: TimeSource,
    /// First and last sample times (None when there are no samples).
    pub t_first: Option<Timestamp>,
    pub t_last: Option<Timestamp>,
    pub channels: Vec<ChannelInfo>,
    /// Columns or fields that hold no plottable numbers (text, for now), left out of `channels`.
    pub skipped: Vec<String>,
}

/// A computation over one channel, run with its concrete time and sample types (see [`Channels::dispatch`]).
pub trait ChannelOp {
    type Out;
    fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Self::Out;
}

/// A computation over a recording's sample times.
pub trait TimesOp {
    type Out;
    fn run<T: Times + ?Sized>(self, times: &T) -> Self::Out;
}

/// What a decoder implements: its channels as concrete types. Every `Channels` is a [`Recording`].
pub trait Channels: Send + Sync + Debug {
    /// What the recording holds (the [`Recording::info`] of every `Channels`).
    fn describe(&self) -> &RecordingInfo;

    /// Runs `op` over channel `channel`'s stored values (before scale and offset).
    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out;

    /// Runs `op` over the sample times.
    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out;

    /// The scale and offset applied to channel `channel`'s stored values: shown = stored × scale + offset.
    fn transform(&self, _channel: usize) -> (f64, f64) {
        (1.0, 0.0)
    }

    /// Bytes that identify how the source is decoded (format and descriptor), for cache keys.
    fn layout_id(&self) -> Vec<u8>;

    /// Level-4 summaries of samples `start..end` of every channel (`start` a multiple of 256). A decoder whose
    /// channels are interleaved overrides this to read the block once for all of them.
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        struct Op(usize, usize);
        impl ChannelOp for Op {
            type Out = Vec<Summary>;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Vec<Summary> {
                pyramid::summarize(times, values, self.0, self.1)
            }
        }
        (0..self.describe().channels.len()).map(|c| self.dispatch(c, Op(start, end))).collect()
    }
}

/// A recording opened for reading. Every channel shares the recording's sample times. Values returned are
/// shown values (scale and offset applied) unless a method says otherwise.
pub trait Recording: Send + Sync + Debug {
    fn info(&self) -> &RecordingInfo;

    /// Channel `channel` reduced for `vp` from its raw samples.
    fn trace(&self, channel: usize, vp: Viewport) -> Trace {
        self.view(channel, vp, None, &[])
    }

    /// Channel `channel` reduced for `vp`, from its pyramid when given (the same result, faster), with the line
    /// broken before each sample in `breaks`.
    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize]) -> Trace;

    /// Level-4 summaries of the stored values of samples `start..end` (see [`pyramid::summarize`]).
    fn summarize(&self, channel: usize, start: usize, end: usize) -> Vec<Summary>;

    /// [`Recording::summarize`] for every channel at once (see [`Channels::summarize_all`]).
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>>;

    /// Exact statistics of samples `a..b` (shown values).
    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize) -> Stats;

    /// Shown value of sample `i`.
    fn value_at(&self, channel: usize, i: usize) -> f64;

    fn time_at(&self, i: usize) -> Timestamp;

    /// Index of the first sample at or after `t`.
    fn lower_bound(&self, t: Timestamp) -> usize;

    /// The gap index of the sample times (empty for fixed-rate times).
    fn gap_index(&self) -> GapIndex;

    /// See [`Channels::layout_id`].
    fn layout_id(&self) -> Vec<u8>;
}

impl<C: Channels> Recording for C {
    fn info(&self) -> &RecordingInfo {
        Channels::describe(self)
    }

    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize]) -> Trace {
        struct Op<'a> {
            vp: Viewport,
            pyr: Option<&'a Pyramid<'a>>,
            breaks: &'a [usize],
        }
        impl ChannelOp for Op<'_> {
            type Out = Trace;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Trace {
                match self.pyr {
                    Some(p) => pyramid::m4(times, values, p, self.vp, self.breaks),
                    None => crate::m4_breaking(times, values, self.vp, self.breaks),
                }
            }
        }
        let mut trace = self.dispatch(channel, Op { vp, pyr, breaks });
        let (scale, offset) = self.transform(channel);
        if scale != 1.0 || offset != 0.0 {
            for y in &mut trace.y {
                *y = (*y as f64 * scale + offset) as f32;
            }
        }
        trace
    }

    fn summarize(&self, channel: usize, start: usize, end: usize) -> Vec<Summary> {
        struct Op(usize, usize);
        impl ChannelOp for Op {
            type Out = Vec<Summary>;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Vec<Summary> {
                pyramid::summarize(times, values, self.0, self.1)
            }
        }
        self.dispatch(channel, Op(start, end))
    }

    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        Channels::summarize_all(self, start, end)
    }

    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize) -> Stats {
        struct Op<'a>(&'a Pyramid<'a>, usize, usize);
        impl ChannelOp for Op<'_> {
            type Out = Summary;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Summary {
                pyramid::summary_of_range(times, values, self.0, self.1, self.2)
            }
        }
        let (scale, offset) = self.transform(channel);
        Stats::from(self.dispatch(channel, Op(pyr, a, b))).transformed(scale, offset)
    }

    fn value_at(&self, channel: usize, i: usize) -> f64 {
        struct Op(usize);
        impl ChannelOp for Op {
            type Out = f64;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, _: &T, values: &S) -> f64 {
                values.get(self.0).to_f64()
            }
        }
        let (scale, offset) = self.transform(channel);
        self.dispatch(channel, Op(i)) * scale + offset
    }

    fn time_at(&self, i: usize) -> Timestamp {
        struct Op(usize);
        impl TimesOp for Op {
            type Out = Timestamp;
            fn run<T: Times + ?Sized>(self, times: &T) -> Timestamp {
                times.at(self.0)
            }
        }
        self.dispatch_times(Op(i))
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        struct Op(Timestamp);
        impl TimesOp for Op {
            type Out = usize;
            fn run<T: Times + ?Sized>(self, times: &T) -> usize {
                times.lower_bound(self.0)
            }
        }
        self.dispatch_times(Op(t))
    }

    fn gap_index(&self) -> GapIndex {
        struct Op;
        impl TimesOp for Op {
            type Out = GapIndex;
            fn run<T: Times + ?Sized>(self, times: &T) -> GapIndex {
                GapIndex::build(times)
            }
        }
        match Channels::describe(self).time {
            TimeSource::Rate { .. } | TimeSource::Index => GapIndex::default(),
            TimeSource::Column { .. } => self.dispatch_times(Op),
        }
    }

    fn layout_id(&self) -> Vec<u8> {
        Channels::layout_id(self)
    }
}
