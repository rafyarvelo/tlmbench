//! Sample value types a channel can hold, and access to a channel's samples.

/// A sample value. Comparisons follow the type's own order; NaN (floats only) is a missing sample.
pub trait Value: Copy + PartialOrd + Send + Sync + 'static {
    fn is_nan(self) -> bool;
    fn to_f32(self) -> f32;
    /// Exact for every type except 64-bit integers past 2^53.
    fn to_f64(self) -> f64;
}

macro_rules! int_value {
    ($($t:ty),*) => {$(
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
        }
    )*};
}

int_value!(i8, i16, i32, i64, u8, u16, u32, u64);

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
