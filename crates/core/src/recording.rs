//! An opened recording: what it holds, and its channels reduced for a viewport.

use std::fmt::Debug;
use std::path::PathBuf;

use serde::Serialize;

use crate::chunks::{Chunk, Held};
use crate::gaps::GapIndex;
use crate::pyramid::{self, Pyramid, Stats, Summary};
use crate::{Samples, TimeIndex, Times, Timestamp, Trace, Value, Viewport};

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

    /// [`Channels::dispatch`], reading values through the chunks in `held` where a decoder stores the channel other
    /// than as a contiguous array (see [`Channels::chunked`]).
    fn dispatch_held<O: ChannelOp>(&self, channel: usize, _held: &Held, op: O) -> O::Out {
        self.dispatch(channel, op)
    }

    /// Whether channel `channel` is stored other than as a contiguous native array (interleaved, big-endian or
    /// misaligned), so queries read it faster from decoded chunks.
    fn chunked(&self, _channel: usize) -> bool {
        false
    }

    /// Chunks `chunks` of each of `channels` (chunked ones), decoded: `out[i][j]` is chunk `chunks.start + j` of
    /// `channels[i]`. A decoder reads each block of records once for every channel.
    fn decode_chunks(&self, channels: &[usize], chunks: std::ops::Range<usize>) -> Vec<Vec<Chunk>> {
        assert!(channels.is_empty() || chunks.is_empty(), "this decoder has no chunked channels");
        channels.iter().map(|_| Vec::new()).collect()
    }

    /// Gives a time column its chunk index (see [`TimeIndex`]): searches for a time then read one chunk of it.
    /// Decoders without a time column ignore it.
    fn set_time_index(&self, _index: TimeIndex) {}

    /// Runs `op` over the sample times.
    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out;

    /// The scale and offset applied to channel `channel`'s stored values: shown = stored × scale + offset.
    fn transform(&self, _channel: usize) -> (f64, f64) {
        (1.0, 0.0)
    }

    /// Bytes that identify how the source is decoded (format and descriptor), for cache keys.
    fn layout_id(&self) -> Vec<u8>;

    /// Hints that samples in `ranges` will be read soon: a decoder reading a cold file can fetch them all at once
    /// (in parallel) rather than fault them in one by one.
    fn prefetch(&self, _ranges: &[std::ops::Range<usize>]) {}

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
        self.view(channel, vp, None, &[], &Held::default())
    }

    /// Channel `channel` reduced for `vp`, from its pyramid when given (the same result, faster), with the line
    /// broken before each sample in `breaks`. Raw samples are read through `held` where it holds them.
    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize], held: &Held) -> Trace;

    /// A coarse view from the pyramid alone (see [`pyramid::m4_coarse`]): drawn first, then replaced by
    /// [`Recording::view`].
    fn view_coarse(&self, channel: usize, vp: Viewport, pyr: &Pyramid, breaks: &[usize], held: &Held) -> Trace;

    /// Level-4 summaries of the stored values of samples `start..end` (see [`pyramid::summarize`]).
    fn summarize(&self, channel: usize, start: usize, end: usize) -> Vec<Summary>;

    /// See [`Channels::prefetch`].
    fn prefetch(&self, ranges: &[std::ops::Range<usize>]);

    /// [`Recording::summarize`] for every channel at once (see [`Channels::summarize_all`]).
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>>;

    /// Exact statistics of samples `a..b` (shown values), with raw edges read through `held`.
    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, held: &Held) -> Stats;

    /// Shown value of sample `i`, read through `held`.
    fn value_at(&self, channel: usize, i: usize, held: &Held) -> f64;

    /// See [`Channels::chunked`].
    fn chunked(&self, channel: usize) -> bool;

    /// See [`Channels::set_time_index`].
    fn set_time_index(&self, index: TimeIndex);

    /// The chunk index of the sample times (see [`TimeIndex`]); empty for fixed-rate times.
    fn time_index(&self) -> TimeIndex;

    /// See [`Channels::decode_chunks`].
    fn decode_chunks(&self, channels: &[usize], chunks: std::ops::Range<usize>) -> Vec<Vec<Chunk>>;

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

    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize], held: &Held) -> Trace {
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
        let trace = self.dispatch_held(channel, held, Op { vp, pyr, breaks });
        transformed(trace, self.transform(channel))
    }

    fn view_coarse(&self, channel: usize, vp: Viewport, pyr: &Pyramid, breaks: &[usize], held: &Held) -> Trace {
        struct Op<'a> {
            vp: Viewport,
            pyr: &'a Pyramid<'a>,
            breaks: &'a [usize],
        }
        impl ChannelOp for Op<'_> {
            type Out = Trace;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Trace {
                pyramid::m4_coarse(times, values, self.pyr, self.vp, self.breaks)
            }
        }
        let trace = self.dispatch_held(channel, held, Op { vp, pyr, breaks });
        transformed(trace, self.transform(channel))
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

    fn prefetch(&self, ranges: &[std::ops::Range<usize>]) {
        Channels::prefetch(self, ranges)
    }

    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, held: &Held) -> Stats {
        struct Op<'a>(&'a Pyramid<'a>, usize, usize);
        impl ChannelOp for Op<'_> {
            type Out = Summary;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Summary {
                pyramid::summary_of_range(times, values, self.0, self.1, self.2)
            }
        }
        let (scale, offset) = self.transform(channel);
        Stats::from(self.dispatch_held(channel, held, Op(pyr, a, b))).transformed(scale, offset)
    }

    fn value_at(&self, channel: usize, i: usize, held: &Held) -> f64 {
        struct Op(usize);
        impl ChannelOp for Op {
            type Out = f64;
            fn run<T: Times + ?Sized, S: Samples + ?Sized>(self, _: &T, values: &S) -> f64 {
                values.get(self.0).to_f64()
            }
        }
        let (scale, offset) = self.transform(channel);
        self.dispatch_held(channel, held, Op(i)) * scale + offset
    }

    fn chunked(&self, channel: usize) -> bool {
        Channels::chunked(self, channel)
    }

    fn decode_chunks(&self, channels: &[usize], chunks: std::ops::Range<usize>) -> Vec<Vec<Chunk>> {
        Channels::decode_chunks(self, channels, chunks)
    }

    fn set_time_index(&self, index: TimeIndex) {
        Channels::set_time_index(self, index)
    }

    fn time_index(&self) -> TimeIndex {
        struct Op;
        impl TimesOp for Op {
            type Out = TimeIndex;
            fn run<T: Times + ?Sized>(self, times: &T) -> TimeIndex {
                TimeIndex::build(times)
            }
        }
        match Channels::describe(self).time {
            TimeSource::Rate { .. } | TimeSource::Index => TimeIndex::default(),
            TimeSource::Column { .. } => self.dispatch_times(Op),
        }
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

/// Applies a channel's scale and offset to a trace's values.
fn transformed(mut trace: Trace, (scale, offset): (f64, f64)) -> Trace {
    if scale != 1.0 || offset != 0.0 {
        for y in &mut trace.y {
            *y = (*y as f64 * scale + offset) as f32;
        }
    }
    trace
}
