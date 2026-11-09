//! What a person says about a channel that changes its values: a scale and offset (shown = stored × scale + offset,
//! after any the format has) and which samples are missing (stored values from a list, outside a valid range, or
//! while a condition held). [`Adjusted`] applies them inside the decoder's own reads, so pyramids, views, statistics,
//! derived channels and exports all see the same values; its layout id carries them, so a change is a new pyramid.

use std::ops::Range;

use crate::chunks::{Chunk, Held};
use crate::mask::{Mask, Masked};
use crate::recording::{ChannelOp, Channels, RecordingInfo, TimesOp};
use crate::{Samples, TimeIndex, Value};

/// One channel's adjustments (`scale` 1 and `offset` 0 change nothing).
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelAdjust {
    pub scale: f64,
    pub offset: f64,
    /// Stored values that mean "missing" (sentinels such as -9999 or 65535).
    pub values: Vec<f64>,
    /// Stored values below `lo` or above `hi` are missing.
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    /// Samples missing while a condition held (the expression's result, evaluated by the caller).
    pub when: Option<Mask>,
}

impl Default for ChannelAdjust {
    fn default() -> Self {
        Self { scale: 1.0, offset: 0.0, values: Vec::new(), lo: None, hi: None, when: None }
    }
}

impl ChannelAdjust {
    pub fn is_identity(&self) -> bool {
        self.scale == 1.0 && self.offset == 0.0 && !self.has_missing()
    }

    pub fn has_missing(&self) -> bool {
        !self.values.is_empty()
            || self.lo.is_some()
            || self.hi.is_some()
            || self.when.as_ref().is_some_and(|m| !m.is_empty())
    }

    fn id(&self, out: &mut Vec<u8>) {
        for v in [self.scale, self.offset] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.values {
            out.push(b'v');
            out.extend_from_slice(&v.to_le_bytes());
        }
        for (tag, v) in [(b'l', self.lo), (b'h', self.hi)] {
            if let Some(v) = v {
                out.push(tag);
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        if let Some(m) = &self.when {
            for r in m.runs() {
                out.push(b'w');
                out.extend_from_slice(&(r.start as u64).to_le_bytes());
                out.extend_from_slice(&(r.end as u64).to_le_bytes());
            }
        }
    }
}

/// The samples of one channel `adj` calls missing from its stored values (not the `when` ones), as runs.
struct Scan<'a>(&'a ChannelAdjust);

impl ChannelOp for Scan<'_> {
    type Out = Vec<Range<usize>>;

    fn run<T: crate::Times + ?Sized, S: Samples + ?Sized>(self, _: &T, values: &S) -> Self::Out {
        let a = self.0;
        let mut runs: Vec<Range<usize>> = Vec::new();
        for (i, v) in values.range(0, values.len()).enumerate() {
            let v = v.to_f64();
            if v.is_nan() {
                continue;
            }
            let bad = a.values.contains(&v) || a.lo.is_some_and(|lo| v < lo) || a.hi.is_some_and(|hi| v > hi);
            if !bad {
                continue;
            }
            match runs.last_mut() {
                Some(r) if r.end == i => r.end = i + 1,
                _ => runs.push(i..i + 1),
            }
        }
        runs
    }
}

/// A channel's operation, run over its values with the missing samples as NaN.
struct MissingOp<'m, O> {
    op: O,
    mask: &'m Mask,
}

impl<O: ChannelOp> ChannelOp for MissingOp<'_, O> {
    type Out = O::Out;

    fn run<T: crate::Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> O::Out {
        self.op.run(times, &Masked { values, mask: self.mask })
    }
}

/// A decoder's channels with a person's adjustments applied.
#[derive(Debug)]
pub struct Adjusted<C: Channels> {
    inner: C,
    /// By channel: the extra scale and offset, and the samples missing (empty: none).
    transform: Vec<(f64, f64)>,
    missing: Vec<Mask>,
    id: Vec<u8>,
}

impl<C: Channels> Adjusted<C> {
    /// `inner` with `adjust` (by channel name; names the recording lacks are ignored). Channels with missing rules
    /// are read once now to find their missing samples.
    pub fn new(inner: C, adjust: &[(String, ChannelAdjust)]) -> Self {
        let n = inner.describe().channels.len();
        let mut transform = vec![(1.0, 0.0); n];
        let mut missing = vec![Mask::default(); n];
        let mut id = inner.layout_id();
        id.extend_from_slice(b"|adjusted");
        for (name, a) in adjust {
            let Some(c) = inner.describe().channels.iter().position(|ch| &ch.name == name) else { continue };
            if a.is_identity() {
                continue;
            }
            transform[c] = (a.scale, a.offset);
            if a.has_missing() {
                let mut m = Mask::from_runs(inner.dispatch(c, Scan(a)));
                if let Some(w) = &a.when {
                    m = m.union(w);
                }
                missing[c] = m;
            }
            id.extend_from_slice(&(c as u64).to_le_bytes());
            a.id(&mut id);
        }
        Self { inner, transform, missing, id }
    }
}

impl<C: Channels> Channels for Adjusted<C> {
    fn describe(&self) -> &RecordingInfo {
        self.inner.describe()
    }

    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out {
        let m = &self.missing[channel];
        if m.is_empty() {
            self.inner.dispatch(channel, op)
        } else {
            self.inner.dispatch(channel, MissingOp { op, mask: m })
        }
    }

    fn dispatch_held<O: ChannelOp>(&self, channel: usize, held: &Held, op: O) -> O::Out {
        let m = &self.missing[channel];
        if m.is_empty() {
            self.inner.dispatch_held(channel, held, op)
        } else {
            self.inner.dispatch_held(channel, held, MissingOp { op, mask: m })
        }
    }

    fn chunked(&self, channel: usize) -> bool {
        self.inner.chunked(channel)
    }

    fn decode_chunks(&self, channels: &[usize], chunks: Range<usize>) -> Vec<Vec<Chunk>> {
        self.inner.decode_chunks(channels, chunks)
    }

    fn set_time_index(&self, index: TimeIndex) {
        self.inner.set_time_index(index)
    }

    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out {
        self.inner.dispatch_times(op)
    }

    fn transform(&self, channel: usize) -> (f64, f64) {
        let (s0, o0) = self.inner.transform(channel);
        let (s, o) = self.transform[channel];
        (s0 * s, o0 * s + o)
    }

    fn layout_id(&self) -> Vec<u8> {
        self.id.clone()
    }

    fn prefetch(&self, ranges: &[Range<usize>]) {
        Channels::prefetch(&self.inner, ranges)
    }

    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<crate::pyramid::Summary>> {
        if self.missing.iter().all(Mask::is_empty) {
            return self.inner.summarize_all(start, end);
        }
        // the inner one's block read would see the missing samples as numbers: summarize channel by channel
        struct Op(usize, usize);
        impl ChannelOp for Op {
            type Out = Vec<crate::pyramid::Summary>;
            fn run<T: crate::Times + ?Sized, S: Samples + ?Sized>(self, times: &T, values: &S) -> Self::Out {
                crate::pyramid::summarize(times, values, self.0, self.1)
            }
        }
        (0..self.inner.describe().channels.len()).map(|c| self.dispatch(c, Op(start, end))).collect()
    }

    fn missing_mask(&self, channel: usize) -> Mask {
        self.missing[channel].clone()
    }
}
