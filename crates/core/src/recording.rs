//! An opened recording: what it holds, and its channels reduced for a viewport.

use std::fmt::Debug;
use std::path::PathBuf;

use serde::Serialize;

use crate::{Timestamp, Trace, Viewport};

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

/// A recording opened for reading. Every channel shares the recording's sample times.
pub trait Recording: Send + Sync + Debug {
    fn info(&self) -> &RecordingInfo;

    /// Channel `channel` reduced for `vp`, with the channel's scale and offset applied.
    fn trace(&self, channel: usize, vp: Viewport) -> Trace;
}
