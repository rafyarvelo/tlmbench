//! Sample value types a channel can hold, and access to a channel's samples.

/// A sample value. Comparisons follow the type's own order; NaN (floats only) is a missing sample.
pub trait Value: Copy + PartialOrd + Send + Sync + 'static {
    fn is_nan(self) -> bool;
    fn to_f32(self) -> f32;
    /// Exact for every type except 64-bit integers past 2^53.
    fn to_f64(self) -> f64;
    /// Values of this type as a [`Column`].
    fn column(values: Vec<Self>) -> Column;
}

/// A run of one channel's samples in their stored type (what exports write).
#[derive(Clone, Debug, PartialEq)]
pub enum Column {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::I8(v) => v.len(),
            Column::I16(v) => v.len(),
            Column::I32(v) => v.len(),
            Column::I64(v) => v.len(),
            Column::U8(v) => v.len(),
            Column::U16(v) => v.len(),
            Column::U32(v) => v.len(),
            Column::U64(v) => v.len(),
            Column::F32(v) => v.len(),
            Column::F64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Value `i` as f64 (exact except for 64-bit integers past 2^53).
    pub fn get_f64(&self, i: usize) -> f64 {
        match self {
            Column::I8(v) => v[i].to_f64(),
            Column::I16(v) => v[i].to_f64(),
            Column::I32(v) => v[i].to_f64(),
            Column::I64(v) => v[i].to_f64(),
            Column::U8(v) => v[i].to_f64(),
            Column::U16(v) => v[i].to_f64(),
            Column::U32(v) => v[i].to_f64(),
            Column::U64(v) => v[i].to_f64(),
            Column::F32(v) => v[i].to_f64(),
            Column::F64(v) => v[i],
        }
    }

    /// Every value as f64, times `scale` plus `offset` (shown values).
    pub fn to_f64(&self, scale: f64, offset: f64) -> Vec<f64> {
        (0..self.len()).map(|i| self.get_f64(i) * scale + offset).collect()
    }
}

macro_rules! int_value {
    ($($t:ty => $c:ident),*) => {$(
        impl Value for $t {
            #[inline]
            fn is_nan(self) -> bool {
                false
            }
            #[inline]
            fn to_f32(self) -> f32 {
                self as f32
            }
            #[inline]
            fn to_f64(self) -> f64 {
                self as f64
            }
            fn column(values: Vec<Self>) -> Column {
                Column::$c(values)
            }
        }
    )*};
}

int_value!(i8 => I8, i16 => I16, i32 => I32, i64 => I64, u8 => U8, u16 => U16, u32 => U32, u64 => U64);

impl Value for f32 {
    #[inline]
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn column(values: Vec<Self>) -> Column {
        Column::F32(values)
    }
}

impl Value for f64 {
    #[inline]
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    fn column(values: Vec<Self>) -> Column {
        Column::F64(values)
    }
}

/// One channel's samples: a slice in memory, or a field read in place from records in a mapped file.
pub trait Samples: Sync {
    type Item: Value;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Sample `i` (`i < len()`).
    fn get(&self, i: usize) -> Self::Item;

    /// The samples as one slice, when they are stored that way (enables the vectorized paths).
    fn as_slice(&self) -> Option<&[Self::Item]> {
        None
    }

    /// Samples `start..end` as one slice, when they are stored that way.
    fn slice(&self, start: usize, end: usize) -> Option<&[Self::Item]> {
        self.as_slice().map(|s| &s[start..end])
    }

    /// Samples `start..end`, in order.
    fn range(
        &self,
        start: usize,
        end: usize,
    ) -> impl DoubleEndedIterator<Item = Self::Item> + ExactSizeIterator + Clone;
}

impl<V: Value> Samples for [V] {
    type Item = V;

    fn len(&self) -> usize {
        <[V]>::len(self)
    }

    #[inline]
    fn get(&self, i: usize) -> V {
        self[i]
    }

    fn as_slice(&self) -> Option<&[V]> {
        Some(self)
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        self[start..end].iter().copied()
    }
}

impl<V: Value> Samples for Vec<V> {
    type Item = V;

    fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[inline]
    fn get(&self, i: usize) -> V {
        self[i]
    }

    fn as_slice(&self) -> Option<&[V]> {
        Some(self)
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        self[start..end].iter().copied()
    }
}

/// Samples `offset..offset + data.len()` of a channel held in a buffer (a block read from a file); only those
/// indexes may be asked for.
#[derive(Clone, Copy, Debug)]
pub struct Shifted<'a, V> {
    pub offset: usize,
    pub data: &'a [V],
}

impl<V: Value> Samples for Shifted<'_, V> {
    type Item = V;

    fn len(&self) -> usize {
        self.offset + self.data.len()
    }

    #[inline]
    fn get(&self, i: usize) -> V {
        self.data[i - self.offset]
    }

    fn slice(&self, start: usize, end: usize) -> Option<&[V]> {
        Some(&self.data[start - self.offset..end - self.offset])
    }

    #[inline]
    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        self.data[start - self.offset..end - self.offset].iter().copied()
    }
}
