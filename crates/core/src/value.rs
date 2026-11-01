//! Sample value types a channel can hold.

/// A sample value. Comparisons follow the type's own order; NaN (floats only) is a missing sample.
pub trait Value: Copy + PartialOrd + Send + Sync + 'static {
    fn is_nan(self) -> bool;
    fn to_f32(self) -> f32;
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
}
