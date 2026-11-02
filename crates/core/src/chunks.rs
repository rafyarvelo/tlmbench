//! Decoded chunks: 64k samples of one channel as a contiguous native array, for channels that aren't stored that
//! way (fields of interleaved records, big-endian or misaligned ones). Reading a chunk is as fast as reading a slice,
//! where reading a field in place decodes it record by record.
//!
//! The store keeps chunks in an LRU cache with a byte budget. A query holds the chunks it reads ([`Held`]), and a
//! held chunk is reference-counted, so the cache never frees one in use. Samples outside the held chunks are read in
//! place, so a query is correct whatever is held.

use std::sync::Arc;

use bytemuck::Pod;

use crate::{Samples, Value};

/// Samples per chunk.
pub const CHUNK_SAMPLES: usize = 1 << CHUNK_SHIFT;
const CHUNK_SHIFT: u32 = 16;

/// The chunk sample `i` falls in.
#[inline]
pub const fn chunk_of(i: usize) -> usize {
    i >> CHUNK_SHIFT
}

/// One channel's samples `k·CHUNK_SAMPLES..` (up to [`CHUNK_SAMPLES`] of them), in native order, held as 8-byte
/// words so every value type is aligned.
#[derive(Debug)]
pub struct Chunk {
    words: Vec<u64>,
    len: usize,
}

impl Chunk {
    /// `len` values of the channel's type packed into `words` (at least `len × size` bytes).
    pub fn new(words: Vec<u64>, len: usize) -> Self {
        Self { words, len }
    }

    pub fn from_values<V: Pod>(values: &[V]) -> Self {
        let mut words = vec![0u64; size_of_val(values).div_ceil(8)];
        bytemuck::cast_slice_mut::<u64, u8>(&mut words)[..size_of_val(values)]
            .copy_from_slice(bytemuck::cast_slice(values));
        Self::new(words, values.len())
    }

    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.words.capacity() * 8
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The values, as the channel's type.
    #[inline]
    pub fn values<V: Pod>(&self) -> &[V] {
        &bytemuck::cast_slice(&self.words)[..self.len]
    }
}

/// Chunks of one channel held for a query: chunk `first + j` is `chunks[j]`, when decoded.
#[derive(Clone, Debug, Default)]
pub struct Held {
    pub first: usize,
    pub chunks: Vec<Option<Arc<Chunk>>>,
}

impl Held {
    /// Chunk `k`, if held.
    #[inline]
    pub fn get(&self, k: usize) -> Option<&Chunk> {
        self.chunks.get(k.wrapping_sub(self.first))?.as_deref()
    }

    /// Whether no chunk is held.
    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(Option::is_none)
    }
}

/// A channel's samples read through held chunks, and from `base` (in place) outside them.
#[derive(Debug)]
pub struct Chunked<'a, S: ?Sized> {
    pub base: &'a S,
    pub held: &'a Held,
}

impl<S: ?Sized> Clone for Chunked<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized> Copy for Chunked<'_, S> {}

impl<S: Samples + ?Sized> Samples for Chunked<'_, S>
where
    S::Item: Pod,
{
    type Item = S::Item;

    fn len(&self) -> usize {
        self.base.len()
    }

    #[inline]
    fn get(&self, i: usize) -> S::Item {
        match self.held.get(chunk_of(i)) {
            Some(c) => c.values()[i & (CHUNK_SAMPLES - 1)],
            None => self.base.get(i),
        }
    }

    /// A slice within one held chunk (where the vectorized paths run), else whatever `base` gives.
    fn slice(&self, start: usize, end: usize) -> Option<&[S::Item]> {
        let k = chunk_of(start);
        match self.held.get(k) {
            Some(c) if start < end && chunk_of(end - 1) == k => {
                let at = k * CHUNK_SAMPLES;
                Some(&c.values()[start - at..end - at])
            }
            _ => self.base.slice(start, end),
        }
    }

    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = S::Item> + ExactSizeIterator + Clone {
        Range { s: *self, start, end }
    }
}

/// Samples `start..end` of a [`Chunked`], one by one.
#[derive(Debug)]
struct Range<'a, S: ?Sized> {
    s: Chunked<'a, S>,
    start: usize,
    end: usize,
}

impl<S: ?Sized> Clone for Range<'_, S> {
    fn clone(&self) -> Self {
        Self { s: self.s, start: self.start, end: self.end }
    }
}

impl<S: Samples + ?Sized> Iterator for Range<'_, S>
where
    S::Item: Pod,
{
    type Item = S::Item;

    #[inline]
    fn next(&mut self) -> Option<S::Item> {
        (self.start < self.end).then(|| {
            self.start += 1;
            self.s.get(self.start - 1)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.start;
        (n, Some(n))
    }
}

impl<S: Samples + ?Sized> DoubleEndedIterator for Range<'_, S>
where
    S::Item: Pod,
{
    #[inline]
    fn next_back(&mut self) -> Option<S::Item> {
        (self.start < self.end).then(|| {
            self.end -= 1;
            self.s.get(self.end)
        })
    }
}

impl<S: Samples + ?Sized> ExactSizeIterator for Range<'_, S> where S::Item: Pod {}

/// Chunks `first..first + n` of `values` (tests and benchmarks).
pub fn hold_all<V: Value + Pod>(values: &[V], first: usize, n: usize) -> Held {
    let chunks = (first..first + n)
        .map(|k| {
            let a = (k * CHUNK_SAMPLES).min(values.len());
            let b = ((k + 1) * CHUNK_SAMPLES).min(values.len());
            (a < b).then(|| Arc::new(Chunk::from_values(&values[a..b])))
        })
        .collect();
    Held { first, chunks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyramid::{self, Pyramid};
    use crate::{RateTimes, Times, Viewport, m4};
    use proptest::prelude::*;

    /// Samples with no slice path: what a field read in place looks like to a query.
    struct InPlace<'a>(&'a [f32]);

    impl Samples for InPlace<'_> {
        type Item = f32;
        fn len(&self) -> usize {
            self.0.len()
        }
        fn get(&self, i: usize) -> f32 {
            self.0[i]
        }
        fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = f32> + ExactSizeIterator + Clone {
            self.0[start..end].iter().copied()
        }
    }

    /// Equal point for point, NaN gaps included.
    fn same(a: &crate::Trace, b: &crate::Trace) -> bool {
        let bits = |t: &crate::Trace| t.y.iter().map(|y| y.to_bits()).collect::<Vec<_>>();
        (a.kind, a.bucket_ns, &a.t, bits(a)) == (b.kind, b.bucket_ns, &b.t, bits(b))
    }

    fn signal(n: usize, nan_every: usize) -> Vec<f32> {
        (0..n)
            .map(|i| match i {
                _ if i % nan_every < 3 => f32::NAN,
                _ if i % 9_973 == 0 => 40.0,
                _ => (i as f32 * 0.003).sin() * 5.0 + ((i * 7919) % 13) as f32 * 0.1,
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Whichever chunks are held, a view or a stat reads the same samples: raw, from the pyramid, coarse.
        #[test]
        fn chunked_samples_reduce_exactly_like_raw(
            n in 1usize..(5 * CHUNK_SAMPLES),
            nan_every in 500usize..200_000,
            held_mask in any::<u8>(),
            a in 0.0f64..1.0, span in 0.0f64..1.0, width_px in 1u32..3000,
        ) {
            let values = signal(n, nan_every);
            let times = RateTimes::from_rate(1_000, 10_000.0, n);
            let mut held = hold_all(&values, 0, n.div_ceil(CHUNK_SAMPLES));
            for (k, c) in held.chunks.iter_mut().enumerate() {
                if held_mask >> (k % 8) & 1 == 0 {
                    *c = None;
                }
            }
            let chunked = Chunked { base: &InPlace(&values), held: &held };
            let (first, last) = (times.at(0), times.at(n - 1) + 1);
            let t0 = first + ((last - first) as f64 * a) as i64;
            let t1 = t0 + 1 + ((last - t0) as f64 * span) as i64;
            let vp = Viewport { t0, t1, width_px };
            prop_assert!(same(&m4(&times, &chunked, vp), &m4(&times, &values, vp)));

            let levels = pyramid::build_levels(pyramid::summarize(&times, &values, 0, n));
            let pyr = Pyramid::new(n, levels.iter().map(Vec::as_slice).collect());
            prop_assert!(same(&pyramid::m4(&times, &chunked, &pyr, vp, &[]), &pyramid::m4(&times, &values, &pyr, vp, &[])));
            prop_assert!(same(
                &pyramid::m4_coarse(&times, &chunked, &pyr, vp, &[]),
                &pyramid::m4_coarse(&times, &values, &pyr, vp, &[])
            ));
            let (i, j) = (times.lower_bound(t0), times.lower_bound(t1));
            if i < j {
                let same = |x: pyramid::Summary, y: pyramid::Summary| bytemuck::bytes_of(&x) == bytemuck::bytes_of(&y);
                prop_assert!(same(
                    pyramid::summary_of_range(&times, &chunked, &pyr, i, j),
                    pyramid::summary_of_range(&times, &values, &pyr, i, j)
                ));
            }
        }
    }

    #[test]
    fn slices_stay_within_a_held_chunk() {
        let values: Vec<f32> = (0..3 * CHUNK_SAMPLES).map(|i| i as f32).collect();
        let mut held = hold_all(&values, 0, 3);
        held.chunks[2] = None;
        let c = Chunked { base: &InPlace(&values), held: &held };
        assert_eq!(c.slice(10, 20), Some(&values[10..20]));
        assert_eq!(c.slice(CHUNK_SAMPLES, 2 * CHUNK_SAMPLES).map(<[f32]>::len), Some(CHUNK_SAMPLES));
        assert_eq!(c.slice(CHUNK_SAMPLES - 1, CHUNK_SAMPLES + 1), None); // across chunks: no slice
        assert_eq!(c.slice(2 * CHUNK_SAMPLES + 5, 2 * CHUNK_SAMPLES + 9), None); // not held
        assert_eq!(c.get(2 * CHUNK_SAMPLES + 7), values[2 * CHUNK_SAMPLES + 7]);
        let r: Vec<f32> = c.range(CHUNK_SAMPLES - 2, 2 * CHUNK_SAMPLES + 2).rev().collect();
        let want: Vec<f32> = values[CHUNK_SAMPLES - 2..2 * CHUNK_SAMPLES + 2].iter().rev().copied().collect();
        assert_eq!(r, want);
    }
}
