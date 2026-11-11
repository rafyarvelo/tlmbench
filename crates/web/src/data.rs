//! What the viewer holds: each file's times and, per channel, its raw samples (when the export has them) and its
//! pyramid. A file is a [`Channels`], so the core's `Recording` queries run over it unchanged.

use std::path::PathBuf;

use tlmbench_core::gaps::GapIndex;
use tlmbench_core::pyramid::{Pyramid, Summary, build_levels, summarize};
use tlmbench_core::recording::{ChannelInfo, ChannelOp, Channels, RecordingInfo, TimeSource, TimesOp};
use tlmbench_core::{RateTimes, Samples, Times, Timestamp};

#[derive(Default)]
pub struct State {
    pub files: Vec<Option<Ds>>,
}

pub enum Tm {
    Rate(RateTimes),
    Col(Vec<Timestamp>),
}

#[derive(Default)]
pub struct Col {
    /// Shown values; None where the export keeps only the pyramid (Light, a chart-only channel).
    pub raw: Option<Vec<f64>>,
    /// Levels from `first_level`, finest first.
    pub levels: Vec<Vec<Summary>>,
    pub first_level: u32,
}

#[derive(Debug)]
pub struct Ds {
    pub info: RecordingInfo,
    pub tm: Tm,
    pub cols: Vec<Col>,
    pub gaps: GapIndex,
    /// Samples the file has (times may still be arriving).
    pub len: usize,
}

impl std::fmt::Debug for Tm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tm")
    }
}
impl std::fmt::Debug for Col {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Col")
    }
}

/// A channel without raw samples: every value reads as missing (the pyramid answers instead).
pub struct Absent(pub usize);

impl Samples for Absent {
    type Item = f64;
    fn len(&self) -> usize {
        self.0
    }
    fn get(&self, _: usize) -> f64 {
        f64::NAN
    }
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = f64> + ExactSizeIterator + Clone {
        (start..end).map(|_| f64::NAN)
    }
}

pub fn dtype(s: &str) -> &'static str {
    match s {
        "i8" => "i8",
        "i16" => "i16",
        "i32" => "i32",
        "i64" => "i64",
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" => "u64",
        "f32" => "f32",
        _ => "f64",
    }
}

impl Ds {
    pub fn new(
        path: &str,
        format: &str,
        part: Option<String>,
        len: usize,
        time: TimeSource,
        tm: Tm,
        channels: Vec<ChannelInfo>,
    ) -> Self {
        let n = channels.len();
        let mut ds = Ds {
            info: RecordingInfo {
                path: PathBuf::from(path),
                format: format.to_owned(),
                part,
                samples: len,
                time,
                t_first: None,
                t_last: None,
                channels,
                skipped: Vec::new(),
                notes: Vec::new(),
            },
            tm,
            cols: (0..n).map(|_| Col::default()).collect(),
            gaps: GapIndex::default(),
            len,
        };
        ds.refresh();
        ds
    }

    /// Times known: the first and last sample, and the gap index.
    pub fn refresh(&mut self) {
        let (first, last) = match &self.tm {
            Tm::Rate(r) if r.len > 0 => (Some(r.at(0)), Some(r.at(r.len - 1))),
            Tm::Col(v) if !v.is_empty() => (v.first().copied(), v.last().copied()),
            _ => (None, None),
        };
        self.info.t_first = first;
        self.info.t_last = last;
        self.gaps = match &self.tm {
            Tm::Rate(_) => GapIndex::default(),
            Tm::Col(v) if v.len() == self.len => GapIndex::build(v.as_slice()),
            Tm::Col(_) => GapIndex::default(),
        };
    }

    pub fn times_ready(&self) -> bool {
        match &self.tm {
            Tm::Rate(_) => true,
            Tm::Col(v) => v.len() == self.len,
        }
    }

    /// The channel's pyramid, when it has one covering the whole file.
    pub fn pyramid(&self, c: usize) -> Option<Pyramid<'_>> {
        let col = &self.cols[c];
        if col.levels.is_empty() || col.first_level != tlmbench_core::pyramid::MIN_LEVEL || self.len == 0 {
            return None;
        }
        Some(Pyramid::new(self.len, col.levels.iter().map(|l| l.as_slice()).collect()))
    }

    /// Builds the pyramid of a channel that has raw samples and none yet.
    pub fn build_pyramid(&mut self, c: usize) {
        let Some(raw) = self.cols[c].raw.as_ref().filter(|r| !r.is_empty() && r.len() == self.len) else { return };
        let level4 = match &self.tm {
            Tm::Rate(t) => summarize(t, raw, 0, raw.len()),
            Tm::Col(t) => summarize(t, raw, 0, raw.len()),
        };
        self.cols[c].levels = build_levels(level4);
        self.cols[c].first_level = tlmbench_core::pyramid::MIN_LEVEL;
    }
}

impl Channels for Ds {
    fn describe(&self) -> &RecordingInfo {
        &self.info
    }

    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out {
        let col = &self.cols[channel];
        let n = self.len;
        match (&self.tm, &col.raw) {
            (Tm::Rate(t), Some(v)) => op.run(t, v),
            (Tm::Rate(t), None) => op.run(t, &Absent(n)),
            (Tm::Col(t), Some(v)) => op.run(t, v),
            (Tm::Col(t), None) => op.run(t, &Absent(n)),
        }
    }

    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out {
        match &self.tm {
            Tm::Rate(t) => op.run(t),
            Tm::Col(t) => op.run(t),
        }
    }

    fn layout_id(&self) -> Vec<u8> {
        b"export".to_vec()
    }
}
