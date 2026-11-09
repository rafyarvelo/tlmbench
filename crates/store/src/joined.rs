//! A recording and its derived channels as one: the channels of the recording, then the derived ones. Each part keeps
//! its own cache entries and pyramid; the joined pyramid is the two put side by side, so joining rebuilds nothing.

use std::ops::Range;
use std::sync::Arc;

use tlmbench_core::chunks::{Chunk, Held};
use tlmbench_core::gaps::GapIndex;
use tlmbench_core::mask::Mask;
use tlmbench_core::pyramid::{Pyramid, Stats, Summary};
use tlmbench_core::{Column, Recording, RecordingInfo, TimeIndex, Timestamp, Trace, Viewport};

use crate::Opened;

#[derive(Debug)]
pub(crate) struct Joined {
    base: Arc<Opened>,
    derived: Arc<Opened>,
    info: RecordingInfo,
    /// The recording's channel count: channels from here on are derived.
    n: usize,
}

impl Joined {
    pub fn new(base: Arc<Opened>, derived: Arc<Opened>) -> Self {
        let b = base.recording().info();
        let mut info = b.clone();
        info.channels.extend(derived.recording().info().channels.iter().cloned());
        let n = b.channels.len();
        Self { base, derived, info, n }
    }

    /// The part holding channel `c`, and the channel's number there.
    fn at(&self, c: usize) -> (&dyn Recording, usize) {
        if c < self.n { (self.base.recording(), c) } else { (self.derived.recording(), c - self.n) }
    }
}

impl Recording for Joined {
    fn info(&self) -> &RecordingInfo {
        &self.info
    }
    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view(c, vp, pyr, breaks, held)
    }
    fn view_masked(&self, channel: usize, vp: Viewport, mask: &Mask, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view_masked(c, vp, mask, breaks, held)
    }
    fn stats_masked(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, mask: &Mask, held: &Held) -> Stats {
        let (r, c) = self.at(channel);
        r.stats_masked(c, pyr, a, b, mask, held)
    }
    fn view_coarse(&self, channel: usize, vp: Viewport, pyr: &Pyramid, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view_coarse(c, vp, pyr, breaks, held)
    }
    fn summarize(&self, channel: usize, start: usize, end: usize) -> Vec<Summary> {
        let (r, c) = self.at(channel);
        r.summarize(c, start, end)
    }
    fn prefetch(&self, ranges: &[Range<usize>]) {
        self.base.recording().prefetch(ranges);
    }
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        let mut all = self.base.recording().summarize_all(start, end);
        all.extend(self.derived.recording().summarize_all(start, end));
        all
    }
    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, held: &Held) -> Stats {
        let (r, c) = self.at(channel);
        r.stats(c, pyr, a, b, held)
    }
    fn value_at(&self, channel: usize, i: usize, held: &Held) -> f64 {
        let (r, c) = self.at(channel);
        r.value_at(c, i, held)
    }
    fn read(&self, channel: usize, start: usize, end: usize) -> Column {
        let (r, c) = self.at(channel);
        r.read(c, start, end)
    }
    fn read_times(&self, start: usize, end: usize) -> Vec<Timestamp> {
        self.base.recording().read_times(start, end)
    }
    fn scale_offset(&self, channel: usize) -> (f64, f64) {
        let (r, c) = self.at(channel);
        r.scale_offset(c)
    }
    fn missing_mask(&self, channel: usize) -> Mask {
        let (r, c) = self.at(channel);
        r.missing_mask(c)
    }
    fn chunked(&self, channel: usize) -> bool {
        let (r, c) = self.at(channel);
        r.chunked(c)
    }
    fn set_time_index(&self, index: TimeIndex) {
        self.base.recording().set_time_index(index);
    }
    fn time_index(&self) -> TimeIndex {
        self.base.recording().time_index()
    }
    fn decode_chunks(&self, channels: &[usize], chunks: Range<usize>) -> Vec<Vec<Chunk>> {
        // derived channels are plain arrays: only the recording's own are ever chunked
        let own: Vec<usize> = channels.iter().copied().filter(|&c| c < self.n).collect();
        assert!(own.len() == channels.len(), "derived channels aren't chunked");
        self.base.recording().decode_chunks(&own, chunks)
    }
    fn time_at(&self, i: usize) -> Timestamp {
        self.base.recording().time_at(i)
    }
    fn lower_bound(&self, t: Timestamp) -> usize {
        self.base.recording().lower_bound(t)
    }
    fn gap_index(&self) -> GapIndex {
        self.base.recording().gap_index()
    }
    fn extend_gap_index(&self, gaps: &mut GapIndex, from: usize) {
        self.base.recording().extend_gap_index(gaps, from);
    }
    fn layout_id(&self) -> Vec<u8> {
        let mut id = self.base.recording().layout_id();
        id.push(b'+');
        id.extend(self.derived.recording().layout_id());
        id
    }
}
