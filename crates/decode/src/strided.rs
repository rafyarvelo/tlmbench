//! Fields read in place from fixed-width records: every `stride` bytes, at `offset` within the record.

use crate::backwards::Backwards;
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
    /// Where time went backwards (see [`Ordering`]): from each record on, no time reads earlier than its plateau.
    pub plateaus: &'a [(usize, i64)],
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
        clamp(self.plateaus, i, self.t0_ns.saturating_add(self.raw(i).saturating_mul(self.scale_ns)))
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

/// Time that goes backwards in a binary file, found once when it opens. The records stay in file order and a record that
/// reads earlier than the ones before it takes their latest time, as in a CSV file (`backwards.rs`), but the times are
/// read from the file's bytes as they are asked for, so only the places are kept: `plateaus` holds, for each record
/// that read earlier, the latest time reached before it. A time is then the larger of what the record says and the
/// plateau of the last such place at or before it (an exact running maximum, and it needs no more memory than the places).
#[derive(Debug, Default)]
pub(crate) struct Ordering {
    first: Option<i64>,
    prev: Option<i64>,
    pub found: Backwards,
    pub plateaus: Vec<(usize, i64)>,
}

impl Ordering {
    /// Takes the time `t` of record `i` (counting from 0); `row` is the number the person is told.
    pub fn push(&mut self, i: usize, t: i64) {
        self.first.get_or_insert(t);
        match self.prev {
            Some(p) if t < p => {
                self.found.record(i + 1, p, t);
                self.plateaus.push((i, p));
            }
            _ => self.prev = Some(t),
        }
    }

    /// The first time, and the latest (what the last record reads, held): none before the first push.
    pub fn first(&self) -> Option<i64> {
        self.first
    }

    pub fn last(&self) -> Option<i64> {
        self.prev
    }

    /// The note for the person, if time went backwards anywhere.
    pub fn note(&self) -> Option<String> {
        self.found.note_of("record", "record")
    }
}

/// `t`, the time record `i` says, held at the plateau of the last place at or before `i` where time went backwards.
#[inline]
pub(crate) fn clamp(plateaus: &[(usize, i64)], i: usize, t: i64) -> i64 {
    if plateaus.is_empty() {
        return t;
    }
    match plateaus.partition_point(|&(r, _)| r <= i) {
        0 => t,
        k => t.max(plateaus[k - 1].1),
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

/// The record numbers where a counter wrapped (the first one after each drop). A drop of less than half the range is
/// not a wrap: it is time going backwards, and is found afterwards from the times.
pub(crate) fn wrap_positions(n: usize, range: i64, raw: impl Fn(usize) -> i64) -> Vec<usize> {
    let mut out = Vec::new();
    let mut prev = if n > 0 { raw(0) } else { 0 };
    for i in 1..n {
        let now = raw(i);
        if wrapped(prev, now, range) == Ok(true) {
            out.push(i);
        }
        prev = now;
    }
    out
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
    pub plateaus: &'a [(usize, i64)],
}

impl Times for CounterTimes<'_> {
    fn len(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    fn at(&self, i: usize) -> Timestamp {
        let wraps = self.wraps.partition_point(|&p| p <= i) as i64;
        let ticks = self.raw.raw(i).saturating_add(wraps.saturating_mul(self.range));
        clamp(self.plateaus, i, self.t0_ns.saturating_add((ticks as f64 * 1e9 / self.clock_hz).round() as i64))
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
