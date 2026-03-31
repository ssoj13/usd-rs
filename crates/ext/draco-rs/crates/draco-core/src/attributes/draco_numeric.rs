//! DracoNumeric trait for type conversion in geometry attributes.
//! Reduces ~360 lines of TypeId dispatch to trait-based static dispatch. Plan5 [M-4].

/// Type metadata for conversion and range checks.
#[derive(Clone, Copy)]
pub struct DracoTypeInfo {
    pub is_integral: bool,
    pub is_float: bool,
    pub is_signed: bool,
    pub min_i128: i128,
    pub max_i128: i128,
    pub max_f64: f64,
    pub min_f64: f64,
    pub size: usize,
}

/// Trait for Draco attribute numeric types. Replaces TypeId-based dispatch.
pub trait DracoNumeric: Copy + 'static {
    fn draco_to_f64(self) -> f64;
    fn draco_to_i128(self) -> Option<i128>;
    fn draco_from_f64(value: f64) -> Self;
    fn draco_type_info() -> DracoTypeInfo;
    fn draco_is_nan_or_inf(self) -> bool;
    fn draco_is_bool() -> bool {
        false
    }
}

macro_rules! impl_draco_numeric_signed_int {
    ($t:ty, $size:expr) => {
        impl DracoNumeric for $t {
            fn draco_to_f64(self) -> f64 {
                self as f64
            }
            fn draco_to_i128(self) -> Option<i128> {
                Some(self as i128)
            }
            fn draco_from_f64(value: f64) -> Self {
                value as $t
            }
            fn draco_type_info() -> DracoTypeInfo {
                DracoTypeInfo {
                    is_integral: true,
                    is_float: false,
                    is_signed: true,
                    min_i128: <$t>::MIN as i128,
                    max_i128: <$t>::MAX as i128,
                    max_f64: <$t>::MAX as f64,
                    min_f64: <$t>::MIN as f64,
                    size: $size,
                }
            }
            fn draco_is_nan_or_inf(self) -> bool {
                false
            }
        }
    };
}

macro_rules! impl_draco_numeric_unsigned_int {
    ($t:ty, $size:expr) => {
        impl DracoNumeric for $t {
            fn draco_to_f64(self) -> f64 {
                self as f64
            }
            fn draco_to_i128(self) -> Option<i128> {
                Some(self as i128)
            }
            fn draco_from_f64(value: f64) -> Self {
                value as $t
            }
            fn draco_type_info() -> DracoTypeInfo {
                DracoTypeInfo {
                    is_integral: true,
                    is_float: false,
                    is_signed: false,
                    min_i128: 0,
                    max_i128: <$t>::MAX as i128,
                    max_f64: <$t>::MAX as f64,
                    min_f64: 0.0,
                    size: $size,
                }
            }
            fn draco_is_nan_or_inf(self) -> bool {
                false
            }
        }
    };
}

impl_draco_numeric_signed_int!(i8, 1);
impl_draco_numeric_unsigned_int!(u8, 1);
impl_draco_numeric_signed_int!(i16, 2);
impl_draco_numeric_unsigned_int!(u16, 2);
impl_draco_numeric_signed_int!(i32, 4);
impl_draco_numeric_unsigned_int!(u32, 4);
impl_draco_numeric_signed_int!(i64, 8);
impl_draco_numeric_unsigned_int!(u64, 8);

impl DracoNumeric for f32 {
    fn draco_to_f64(self) -> f64 {
        self as f64
    }
    fn draco_to_i128(self) -> Option<i128> {
        None
    }
    fn draco_from_f64(value: f64) -> Self {
        value as f32
    }
    fn draco_type_info() -> DracoTypeInfo {
        DracoTypeInfo {
            is_integral: false,
            is_float: true,
            is_signed: true,
            min_i128: 0,
            max_i128: 0,
            max_f64: f32::MAX as f64,
            min_f64: f32::MIN as f64,
            size: 4,
        }
    }
    fn draco_is_nan_or_inf(self) -> bool {
        self.is_nan() || self.is_infinite()
    }
}

impl DracoNumeric for f64 {
    fn draco_to_f64(self) -> f64 {
        self
    }
    fn draco_to_i128(self) -> Option<i128> {
        None
    }
    fn draco_from_f64(value: f64) -> Self {
        value
    }
    fn draco_type_info() -> DracoTypeInfo {
        DracoTypeInfo {
            is_integral: false,
            is_float: true,
            is_signed: true,
            min_i128: 0,
            max_i128: 0,
            max_f64: f64::MAX,
            min_f64: f64::MIN,
            size: 8,
        }
    }
    fn draco_is_nan_or_inf(self) -> bool {
        self.is_nan() || self.is_infinite()
    }
}

impl DracoNumeric for bool {
    fn draco_to_f64(self) -> f64 {
        if self {
            1.0
        } else {
            0.0
        }
    }
    fn draco_to_i128(self) -> Option<i128> {
        Some(if self { 1 } else { 0 })
    }
    fn draco_from_f64(value: f64) -> Self {
        value != 0.0
    }
    fn draco_type_info() -> DracoTypeInfo {
        DracoTypeInfo {
            is_integral: true,
            is_float: false,
            is_signed: false,
            min_i128: 0,
            max_i128: 1,
            max_f64: 1.0,
            min_f64: 0.0,
            size: 1,
        }
    }
    fn draco_is_nan_or_inf(self) -> bool {
        false
    }
    fn draco_is_bool() -> bool {
        true
    }
}
