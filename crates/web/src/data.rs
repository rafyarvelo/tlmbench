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

/// A channel's raw samples as the export has them: spans of consecutive samples, sorted and apart. A Full export (or a
/// file opened in the page) holds one span of every sample; a Light one holds windows around events and violations;
/// a chart-only channel holds none. A sample outside every span is *absent*: it is never read as a number.
#[derive(Default)]
pub struct Raw {
    spans: Vec<(usize, Vec<f64>)>,
}

impl Raw {
    pub fn whole(values: Vec<f64>) -> Self {
        Self { spans: if values.is_empty() { Vec::new() } else { vec![(0, values)] } }
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Samples held.
    #[cfg(test)]
    pub fn held(&self) -> usize {
        self.spans.iter().map(|(_, v)| v.len()).sum()
    }

    /// Adds `values` as samples `start..`. A span that touches the one before extends it; overlapping what is held is
    /// an error (a chunk is fetched once).
    pub fn put(&mut self, start: usize, values: Vec<f64>) -> Result<(), String> {
        if values.is_empty() {
            return Ok(());
        }
        let end = start + values.len();
        let at = self.spans.partition_point(|(s, _)| *s < start);
        if at > 0 {
            let (ps, pv) = &self.spans[at - 1];
            if ps + pv.len() > start {
                return Err(format!("samples {start}..{end} overlap samples already held"));
            }
        }
        if let Some((ns, _)) = self.spans.get(at)
            && *ns < end
        {
            return Err(format!("samples {start}..{end} overlap samples already held"));
        }
        self.spans.insert(at, (start, values));
        // join neighbours that now touch
        if at + 1 < self.spans.len() && self.spans[at].0 + self.spans[at].1.len() == self.spans[at + 1].0 {
            let (_, next) = self.spans.remove(at + 1);
            self.spans[at].1.extend(next);
        }
        if at > 0 && self.spans[at - 1].0 + self.spans[at - 1].1.len() == self.spans[at].0 {
            let (_, cur) = self.spans.remove(at);
            self.spans[at - 1].1.extend(cur);
        }
        Ok(())
    }

    /// One past the last sample held in the span that ends the data (where a sequential feed continues).
    pub fn next(&self) -> usize {
        self.spans.last().map_or(0, |(s, v)| s + v.len())
    }

    /// Every sample of `a..b` is held (an empty range always is).
    pub fn covers(&self, a: usize, b: usize) -> bool {
        if a >= b {
            return true;
        }
        let at = self.spans.partition_point(|(s, _)| *s <= a);
        at > 0 && {
            let (s, v) = &self.spans[at - 1];
            b <= s + v.len()
        }
    }

    pub fn get(&self, i: usize) -> Option<f64> {
        let at = self.spans.partition_point(|(s, _)| *s <= i);
        let (s, v) = self.spans.get(at.checked_sub(1)?)?;
        v.as_slice().get(i - s).copied()
    }

    /// All `n` samples as one slice, when that is what is held.
    pub fn slice(&self, n: usize) -> Option<&[f64]> {
        match self.spans.as_slice() {
            [(0, v)] if v.len() == n => Some(v),
            _ => None,
        }
    }
}

/// Samples read through a [`Raw`]: absent ones read as missing. Only reached through views and stats that checked
/// coverage first, so no shown value comes from an absent sample.
pub struct Sparse<'a>(pub &'a Raw, pub usize);

impl Samples for Sparse<'_> {
    type Item = f64;
    fn len(&self) -> usize {
        self.1
    }
    fn get(&self, i: usize) -> f64 {
        self.0.get(i).unwrap_or(f64::NAN)
    }
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = f64> + ExactSizeIterator + Clone {
        (start..end).map(|i| self.0.get(i).unwrap_or(f64::NAN))
    }
}

#[derive(Default)]
pub struct Col {
    /// Shown values the export keeps; none where it keeps only the pyramid (Light, a chart-only channel).
    pub raw: Raw,
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
        let Some(raw) = self.cols[c].raw.slice(self.len).filter(|r| !r.is_empty()) else { return };
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
        match (&self.tm, col.raw.slice(n)) {
            (Tm::Rate(t), Some(v)) => op.run(t, v),
            (Tm::Rate(t), None) if col.raw.is_empty() => op.run(t, &Absent(n)),
            (Tm::Rate(t), None) => op.run(t, &Sparse(&col.raw, n)),
            (Tm::Col(t), Some(v)) => op.run(t, v),
            (Tm::Col(t), None) if col.raw.is_empty() => op.run(t, &Absent(n)),
            (Tm::Col(t), None) => op.run(t, &Sparse(&col.raw, n)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_merge_and_cover() {
        let mut r = Raw::default();
        r.put(10, vec![1.0; 5]).unwrap();
        r.put(20, vec![2.0; 5]).unwrap();
        assert!(r.covers(10, 15) && r.covers(12, 14) && r.covers(20, 25));
        assert!(!r.covers(9, 11) && !r.covers(14, 21) && !r.covers(15, 20));
        assert_eq!(
            (r.get(9), r.get(10), r.get(14), r.get(15), r.get(22)),
            (None, Some(1.0), Some(1.0), None, Some(2.0))
        );
        // a span that fills the gap joins both
        r.put(15, vec![3.0; 5]).unwrap();
        assert!(r.covers(10, 25) && r.held() == 15);
        assert_eq!(r.next(), 25);
    }

    #[test]
    fn overlap_is_refused_and_leaves_what_is_held() {
        let mut r = Raw::whole(vec![0.0; 10]);
        assert!(r.put(5, vec![1.0; 10]).is_err());
        assert!(r.put(0, vec![1.0; 2]).is_err());
        assert_eq!(r.held(), 10);
        r.put(10, vec![7.0; 3]).unwrap();
        assert_eq!(r.slice(13).map(<[f64]>::len), Some(13));
        assert_eq!(r.slice(10), None);
    }

    #[test]
    fn an_absent_sample_is_none_not_a_number() {
        let mut r = Raw::default();
        r.put(0, vec![f64::NAN, 1.0]).unwrap();
        // a held NaN is a missing sample the recording has; an absent one is not held at all
        assert!(r.get(0).is_some_and(f64::is_nan));
        assert_eq!(r.get(2), None);
        assert!(!r.covers(0, 3) && r.covers(0, 2) && r.covers(5, 5));
    }
}
