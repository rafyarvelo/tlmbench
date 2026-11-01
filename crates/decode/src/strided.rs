//! Fields read in place from fixed-width records: every `stride` bytes, at `offset` within the record.

use std::marker::PhantomData;

use tlmbench_core::{Samples, Times, Timestamp, Value};

use crate::descriptor::DType;

/// A value type that can be decoded from its bytes.
pub(crate) trait Pod: Value + bytemuck::Pod {
    const SIZE: usize;
    fn from_le(b: &[u8]) -> Self;
    fn from_be(b: &[u8]) -> Self;
}

macro_rules! pod {
    ($($t:ty),*) => {$(
        impl Pod for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            #[inline]
            fn from_le(b: &[u8]) -> Self {
                <$t>::from_le_bytes(b[..Self::SIZE].try_into().unwrap())
            }
            #[inline]
            fn from_be(b: &[u8]) -> Self {
                <$t>::from_be_bytes(b[..Self::SIZE].try_into().unwrap())
            }
        }
    )*};
}

pod!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

/// One field of every record. `records` holds whole records only.
#[derive(Debug)]
pub(crate) struct Strided<'a, V> {
    pub records: &'a [u8],
    pub offset: usize,
    pub stride: usize,
    pub big_endian: bool,
    pub _v: PhantomData<V>,
}

impl<V: Pod> Strided<'_, V> {
    #[inline]
    fn decode(big_endian: bool, b: &[u8]) -> V {
        if big_endian { V::from_be(b) } else { V::from_le(b) }
    }
}

impl<V: Pod> Samples for Strided<'_, V> {
    type Item = V;

    fn len(&self) -> usize {
        self.records.len() / self.stride
    }

    #[inline]
    fn get(&self, i: usize) -> V {
        Self::decode(self.big_endian, &self.records[i * self.stride + self.offset..])
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        let (offset, big) = (self.offset, self.big_endian);
        self.records[start * self.stride..end * self.stride]
            .chunks_exact(self.stride)
            .map(move |r| Self::decode(big, &r[offset..offset + V::SIZE]))
    }
}

/// Sample times from an integer field of every record, scaled to nanoseconds.
#[derive(Debug)]
pub(crate) struct FieldTimes<'a> {
    pub records: &'a [u8],
    pub offset: usize,
    pub stride: usize,
    pub big_endian: bool,
    pub dtype: DType,
    pub scale_ns: i64,
    pub t0_ns: i64,
}

impl FieldTimes<'_> {
    fn raw(&self, i: usize) -> i64 {
        let b = &self.records[i * self.stride + self.offset..];
        macro_rules! read {
            ($t:ty) => {{
                let v: $t = if self.big_endian { <$t as Pod>::from_be(b) } else { <$t as Pod>::from_le(b) };
                v as i64
            }};
        }
        match self.dtype {
            DType::I8 => read!(i8),
            DType::I16 => read!(i16),
            DType::I32 => read!(i32),
            DType::I64 => read!(i64),
            DType::U8 => read!(u8),
            DType::U16 => read!(u16),
            DType::U32 => read!(u32),
            DType::U64 => read!(u64),
            DType::F32 | DType::F64 => unreachable!("validated: time fields are integers"),
        }
    }
}

impl Times for FieldTimes<'_> {
    fn len(&self) -> usize {
        self.records.len() / self.stride
    }

    #[inline]
    fn at(&self, i: usize) -> Timestamp {
        self.t0_ns.saturating_add(self.raw(i).saturating_mul(self.scale_ns))
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.at(mid) < t {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }
}
