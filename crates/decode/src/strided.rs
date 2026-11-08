//! Fields read in place from fixed-width records: every `stride` bytes, at `offset` within the record.

use std::marker::PhantomData;

use tlmbench_core::{Samples, Times, Timestamp, Value};

use crate::descriptor::{Bits, DType};

/// A value type that can be decoded from its bytes.
pub(crate) trait Pod: Value + bytemuck::Pod {
    const SIZE: usize;
    fn from_le(b: &[u8]) -> Self;
    fn from_be(b: &[u8]) -> Self;
    /// The `width` bits from bit `shift` up (validated to fit): unsigned, or sign-extended for signed types.
    /// Floats come back as they are (validated: bitfields are integers).
    fn extract(self, shift: u32, width: u32) -> Self;
}

macro_rules! pod {
    ($wide:ty; $($t:ty),*) => {$(
        impl Pod for $t {
            #[inline]
            fn extract(self, shift: u32, width: u32) -> Self {
                ((((self as $wide) << (64 - shift - width)) >> (64 - width)) as $t)
            }

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

pod!(i64; i8, i16, i32, i64);
pod!(u64; u8, u16, u32, u64);

macro_rules! pod_float {
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
            #[inline]
            fn extract(self, _: u32, _: u32) -> Self {
                self
            }
        }
    )*};
}

pod_float!(f32, f64);

/// One field of every record. `records` holds whole records only.
#[derive(Debug)]
pub(crate) struct Strided<'a, V> {
    pub records: &'a [u8],
    pub offset: usize,
    pub stride: usize,
    pub big_endian: bool,
    pub bits: Option<Bits>,
    pub _v: PhantomData<V>,
}

/// One value of type `V` from the start of `b`, with the run of bits a bitfield takes.
#[inline]
pub(crate) fn decode<V: Pod>(big_endian: bool, bits: Option<Bits>, b: &[u8]) -> V {
    let v = if big_endian { V::from_be(b) } else { V::from_le(b) };
    match bits {
        Some(Bits { shift, width }) => v.extract(shift, width),
        None => v,
    }
}

impl<V: Pod> Samples for Strided<'_, V> {
    type Item = V;

    fn len(&self) -> usize {
        self.records.len() / self.stride
    }

    #[inline]
    fn get(&self, i: usize) -> V {
        decode(self.big_endian, self.bits, &self.records[i * self.stride + self.offset..])
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        let (offset, big, bits) = (self.offset, self.big_endian, self.bits);
        self.records[start * self.stride..end * self.stride]
            .chunks_exact(self.stride)
            .map(move |r| decode(big, bits, &r[offset..offset + V::SIZE]))
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
    pub bits: Option<Bits>,
    pub scale_ns: i64,
    pub t0_ns: i64,
}

impl FieldTimes<'_> {
    pub(crate) fn raw(&self, i: usize) -> i64 {
        let b = &self.records[i * self.stride + self.offset..];
        macro_rules! read {
            ($t:ty) => {{
                let v: $t = decode(self.big_endian, self.bits, b);
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

/// Whether a counter that read `prev` and then `now` wrapped (it dropped by more than half its `range`); `Err` when
/// it went back by less, which is time going backwards.
pub(crate) fn wrapped(prev: i64, now: i64, range: i64) -> std::result::Result<bool, ()> {
    if now >= prev {
        Ok(false)
    } else if prev - now > range / 2 {
        Ok(true)
    } else {
        Err(())
    }
}

/// The record numbers where a counter wrapped (the first one after each drop); the `Err` is the first record that
/// went back without wrapping.
pub(crate) fn wrap_positions(
    n: usize,
    range: i64,
    raw: impl Fn(usize) -> i64,
) -> std::result::Result<Vec<usize>, usize> {
    let mut out = Vec::new();
    let mut prev = if n > 0 { raw(0) } else { 0 };
    for i in 1..n {
        let now = raw(i);
        if wrapped(prev, now, range).map_err(|()| i)? {
            out.push(i);
        }
        prev = now;
    }
    Ok(out)
}

/// Sample times from a wrapping tick counter: the counter's ticks plus `range` for every wrap at or before the
/// sample (found by a binary search in the wrap positions), at `clock_hz` ticks a second.
#[derive(Debug)]
pub(crate) struct CounterTimes<'a> {
    pub raw: FieldTimes<'a>,
    pub wraps: &'a [usize],
    pub range: i64,
    pub clock_hz: f64,
    pub t0_ns: i64,
}

impl Times for CounterTimes<'_> {
    fn len(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    fn at(&self, i: usize) -> Timestamp {
        let wraps = self.wraps.partition_point(|&p| p <= i) as i64;
        let ticks = self.raw.raw(i).saturating_add(wraps.saturating_mul(self.range));
        self.t0_ns.saturating_add((ticks as f64 * 1e9 / self.clock_hz).round() as i64)
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
