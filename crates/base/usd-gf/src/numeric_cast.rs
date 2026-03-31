//! Safe numeric casting utilities.
//!
//! Provides functions for converting between numeric types with range checking.
//! Matches C++ GfNumericCast - supports int, float, bool, and GfHalf.
//!
//! # Examples
//!
//! ```
//! use usd_gf::numeric_cast::{numeric_cast, integer_compare_less};
//!
//! // Safe integer conversion
//! let result: Option<u8> = numeric_cast(255i32);
//! assert_eq!(result, Some(255u8));
//!
//! // Overflow detection
//! let result: Option<u8> = numeric_cast(256i32);
//! assert_eq!(result, None);
//!
//! // Safe comparison of signed/unsigned
//! assert!(integer_compare_less(-1i32, 0u32));
//!
//! // Half support
//! use usd_gf::Half;
//! let h = Half::from_f32(100.0);
//! let x: Option<i32> = numeric_cast(h);
//! assert_eq!(x, Some(100));
//! ```

use crate::half::Half;

/// Reason for numeric cast failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericCastFailure {
    /// Value is too high to convert.
    PosOverflow,
    /// Value is too low to convert.
    NegOverflow,
    /// Value is a floating-point NaN.
    NaN,
}

/// Compare integers safely across signed/unsigned boundaries.
///
/// Returns true if `t` is logically less than `u` in a mathematical sense.
/// Negative signed integers always compare less than unsigned integers.
///
/// This mimics C++20's `std::cmp_less` function.
///
/// # Examples
///
/// ```
/// use usd_gf::numeric_cast::integer_compare_less;
///
/// assert!(integer_compare_less(-1i32, 0u32));
/// assert!(integer_compare_less(5i32, 10u32));
/// assert!(!integer_compare_less(10u32, 5i32));
/// ```
#[inline]
pub fn integer_compare_less<T: Integer, U: Integer>(t: T, u: U) -> bool {
    // Convert both to i128 for comparison (works for all integer types)
    let t_wide = t.to_i128();
    let u_wide = u.to_i128();
    t_wide < u_wide
}

/// Trait for integer types.
pub trait Integer: Copy {
    /// Converts to i128 for comparison.
    fn to_i128(self) -> i128;
    /// Minimum value for this type.
    fn type_min() -> i128;
    /// Maximum value for this type.
    fn type_max() -> i128;
}

macro_rules! impl_integer {
    ($($t:ty),*) => {
        $(
            impl Integer for $t {
                #[inline]
                fn to_i128(self) -> i128 { self as i128 }
                #[inline]
                fn type_min() -> i128 { <$t>::MIN as i128 }
                #[inline]
                fn type_max() -> i128 { <$t>::MAX as i128 }
            }
        )*
    };
}

impl_integer!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

// Sealed trait pattern for type-safe dispatch
mod sealed {
    use super::*;

    /// Sealed trait for numeric source types - internal dispatch.
    pub trait NumericSource: Copy {
        fn to_i128_opt(&self) -> Option<i128>;
        fn to_half_opt(&self) -> Option<Half>;
        fn to_f32_opt(&self) -> Option<f32>;
        fn to_f64_opt(&self) -> Option<f64>;
        fn check_i128_range(&self, min: i128, max: i128) -> Option<NumericCastFailure>;
    }

    /// Sealed trait for numeric target types - internal dispatch.
    pub trait NumericTarget: Sized {
        fn from_i128(val: i128) -> Option<Self>;
        fn from_half(val: Half) -> Option<Self>;
        fn from_f32(val: f32) -> Option<Self>;
        fn from_f64(val: f64) -> Option<Self>;
        fn bounds() -> (i128, i128);
    }
}

use sealed::{NumericSource, NumericTarget};

// Integer source implementations
macro_rules! impl_int_source {
    ($($t:ty),*) => {
        $(
            impl NumericSource for $t {
                #[inline]
                fn to_i128_opt(&self) -> Option<i128> {
                    Some(*self as i128)
                }
                #[inline]
                fn to_half_opt(&self) -> Option<Half> {
                    Some(Half::from_f64(*self as f64))
                }
                #[inline]
                fn to_f32_opt(&self) -> Option<f32> {
                    Some(*self as f32)
                }
                #[inline]
                fn to_f64_opt(&self) -> Option<f64> {
                    Some(*self as f64)
                }
                #[inline]
                fn check_i128_range(&self, min: i128, max: i128) -> Option<NumericCastFailure> {
                    let val = *self as i128;
                    if val < min {
                        Some(NumericCastFailure::NegOverflow)
                    } else if val > max {
                        Some(NumericCastFailure::PosOverflow)
                    } else {
                        None
                    }
                }
            }
        )*
    };
}

impl_int_source!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

// Float source implementations
impl NumericSource for f32 {
    #[inline]
    fn to_i128_opt(&self) -> Option<i128> {
        if self.is_nan() || self.is_infinite() {
            None
        } else {
            Some(*self as i128)
        }
    }
    #[inline]
    fn to_half_opt(&self) -> Option<Half> {
        Some(Half::from_f32(*self))
    }
    #[inline]
    fn to_f32_opt(&self) -> Option<f32> {
        Some(*self)
    }
    #[inline]
    fn to_f64_opt(&self) -> Option<f64> {
        Some(*self as f64)
    }
    #[inline]
    fn check_i128_range(&self, min: i128, max: i128) -> Option<NumericCastFailure> {
        if self.is_nan() {
            return Some(NumericCastFailure::NaN);
        }
        if self.is_infinite() {
            return Some(if self.is_sign_negative() {
                NumericCastFailure::NegOverflow
            } else {
                NumericCastFailure::PosOverflow
            });
        }
        let val = *self as f64;
        if val < (min as f64 - 1.0) {
            Some(NumericCastFailure::NegOverflow)
        } else if val > (max as f64 + 1.0) {
            Some(NumericCastFailure::PosOverflow)
        } else {
            None
        }
    }
}

impl NumericSource for f64 {
    #[inline]
    fn to_i128_opt(&self) -> Option<i128> {
        if self.is_nan() || self.is_infinite() {
            None
        } else {
            Some(*self as i128)
        }
    }
    #[inline]
    fn to_half_opt(&self) -> Option<Half> {
        Some(Half::from_f64(*self))
    }
    #[inline]
    fn to_f32_opt(&self) -> Option<f32> {
        Some(*self as f32)
    }
    #[inline]
    fn to_f64_opt(&self) -> Option<f64> {
        Some(*self)
    }
    #[inline]
    fn check_i128_range(&self, min: i128, max: i128) -> Option<NumericCastFailure> {
        if self.is_nan() {
            return Some(NumericCastFailure::NaN);
        }
        if self.is_infinite() {
            return Some(if self.is_sign_negative() {
                NumericCastFailure::NegOverflow
            } else {
                NumericCastFailure::PosOverflow
            });
        }
        if *self < (min as f64 - 1.0) {
            Some(NumericCastFailure::NegOverflow)
        } else if *self > (max as f64 + 1.0) {
            Some(NumericCastFailure::PosOverflow)
        } else {
            None
        }
    }
}

// Half source implementation
impl NumericSource for Half {
    #[inline]
    fn to_i128_opt(&self) -> Option<i128> {
        if self.is_nan() || self.is_infinite() {
            None
        } else {
            Some(self.to_f64() as i128)
        }
    }
    #[inline]
    fn to_half_opt(&self) -> Option<Half> {
        Some(*self)
    }
    #[inline]
    fn to_f32_opt(&self) -> Option<f32> {
        Some(self.to_f32())
    }
    #[inline]
    fn to_f64_opt(&self) -> Option<f64> {
        Some(self.to_f64())
    }
    #[inline]
    fn check_i128_range(&self, min: i128, max: i128) -> Option<NumericCastFailure> {
        if self.is_nan() {
            return Some(NumericCastFailure::NaN);
        }
        if self.is_infinite() {
            return Some(if self.is_negative() {
                NumericCastFailure::NegOverflow
            } else {
                NumericCastFailure::PosOverflow
            });
        }
        let val = self.to_f64();
        if val < (min as f64 - 1.0) {
            Some(NumericCastFailure::NegOverflow)
        } else if val > (max as f64 + 1.0) {
            Some(NumericCastFailure::PosOverflow)
        } else {
            None
        }
    }
}

// Integer target implementations
macro_rules! impl_int_target {
    ($($t:ty),*) => {
        $(
            impl NumericTarget for $t {
                #[inline]
                fn from_i128(val: i128) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn from_half(val: Half) -> Option<Self> {
                    Some(val.to_f64() as $t)
                }
                #[inline]
                fn from_f32(val: f32) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn from_f64(val: f64) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn bounds() -> (i128, i128) {
                    (<$t>::MIN as i128, <$t>::MAX as i128)
                }
            }
        )*
    };
}

impl_int_target!(i8, i16, i32, i64, i128, isize);

// Unsigned integer target implementations (separate because u128::MAX > i128::MAX)
macro_rules! impl_uint_target {
    ($($t:ty),*) => {
        $(
            impl NumericTarget for $t {
                #[inline]
                fn from_i128(val: i128) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn from_half(val: Half) -> Option<Self> {
                    Some(val.to_f64() as $t)
                }
                #[inline]
                fn from_f32(val: f32) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn from_f64(val: f64) -> Option<Self> {
                    Some(val as $t)
                }
                #[inline]
                fn bounds() -> (i128, i128) {
                    (0, <$t>::MAX as i128)
                }
            }
        )*
    };
}

impl_uint_target!(u8, u16, u32, u64, usize);

// u128 special case
impl NumericTarget for u128 {
    #[inline]
    fn from_i128(val: i128) -> Option<Self> {
        Some(val as u128)
    }
    #[inline]
    fn from_half(val: Half) -> Option<Self> {
        Some(val.to_f64() as u128)
    }
    #[inline]
    fn from_f32(val: f32) -> Option<Self> {
        Some(val as u128)
    }
    #[inline]
    fn from_f64(val: f64) -> Option<Self> {
        Some(val as u128)
    }
    #[inline]
    fn bounds() -> (i128, i128) {
        (0, i128::MAX) // Can't represent u128::MAX in i128
    }
}

// Float target implementations (no range checking)
impl NumericTarget for f32 {
    #[inline]
    fn from_i128(val: i128) -> Option<Self> {
        Some(val as f32)
    }
    #[inline]
    fn from_half(val: Half) -> Option<Self> {
        Some(val.to_f32())
    }
    #[inline]
    fn from_f32(val: f32) -> Option<Self> {
        Some(val)
    }
    #[inline]
    fn from_f64(val: f64) -> Option<Self> {
        Some(val as f32)
    }
    #[inline]
    fn bounds() -> (i128, i128) {
        (i128::MIN, i128::MAX) // No bounds checking for floats
    }
}

impl NumericTarget for f64 {
    #[inline]
    fn from_i128(val: i128) -> Option<Self> {
        Some(val as f64)
    }
    #[inline]
    fn from_half(val: Half) -> Option<Self> {
        Some(val.to_f64())
    }
    #[inline]
    fn from_f32(val: f32) -> Option<Self> {
        Some(val as f64)
    }
    #[inline]
    fn from_f64(val: f64) -> Option<Self> {
        Some(val)
    }
    #[inline]
    fn bounds() -> (i128, i128) {
        (i128::MIN, i128::MAX) // No bounds checking for floats
    }
}

// Half target implementation
impl NumericTarget for Half {
    #[inline]
    fn from_i128(val: i128) -> Option<Self> {
        Some(Half::from_f64(val as f64))
    }
    #[inline]
    fn from_half(val: Half) -> Option<Self> {
        Some(val)
    }
    #[inline]
    fn from_f32(val: f32) -> Option<Self> {
        Some(Half::from_f32(val))
    }
    #[inline]
    fn from_f64(val: f64) -> Option<Self> {
        Some(Half::from_f64(val))
    }
    #[inline]
    fn bounds() -> (i128, i128) {
        (i128::MIN, i128::MAX) // No bounds checking for floats
    }
}

// Public API traits (for backwards compatibility)
/// Trait for types that support numeric casting.
pub trait NumericCastTarget: NumericTarget {}
impl<T: NumericTarget> NumericCastTarget for T {}

/// Trait for types that can be cast to other numeric types.
pub trait NumericCastSource: NumericSource {}
impl<T: NumericSource> NumericCastSource for T {}

/// Attempts to safely convert a value to a target type.
///
/// Returns `Some(value)` if conversion succeeds, `None` if it would
/// overflow/underflow or if the source is NaN.
///
/// # Examples
///
/// ```
/// use usd_gf::numeric_cast::numeric_cast;
///
/// // Successful conversion
/// let x: Option<u8> = numeric_cast(100i32);
/// assert_eq!(x, Some(100u8));
///
/// // Overflow
/// let x: Option<u8> = numeric_cast(300i32);
/// assert_eq!(x, None);
///
/// // Negative to unsigned
/// let x: Option<u32> = numeric_cast(-5i32);
/// assert_eq!(x, None);
/// ```
#[inline]
pub fn numeric_cast<S: NumericSource, T: NumericTarget>(source: S) -> Option<T> {
    let (min, max) = T::bounds();

    // Check range first (only for integer targets)
    if min != i128::MIN || max != i128::MAX {
        if let Some(_failure) = source.check_i128_range(min, max) {
            return None;
        }
    }

    // Try direct conversions in order of preference
    // For integer targets, prefer i128 path (preserves exact values)
    // For float/Half targets, prefer float paths
    if min != i128::MIN || max != i128::MAX {
        // Integer target
        if let Some(i) = source.to_i128_opt() {
            T::from_i128(i)
        } else if let Some(d) = source.to_f64_opt() {
            T::from_f64(d)
        } else if let Some(f) = source.to_f32_opt() {
            T::from_f32(f)
        } else if let Some(h) = source.to_half_opt() {
            T::from_half(h)
        } else {
            None
        }
    } else {
        // Float/Half target
        if let Some(d) = source.to_f64_opt() {
            T::from_f64(d)
        } else if let Some(f) = source.to_f32_opt() {
            T::from_f32(f)
        } else if let Some(h) = source.to_half_opt() {
            T::from_half(h)
        } else if let Some(i) = source.to_i128_opt() {
            T::from_i128(i)
        } else {
            None
        }
    }
}

/// Attempts to safely convert a value, also returning the failure reason.
///
/// Returns `Ok(value)` if conversion succeeds, `Err(reason)` if it fails.
#[inline]
pub fn numeric_cast_with_reason<S: NumericSource, T: NumericTarget>(
    source: S,
) -> Result<T, NumericCastFailure> {
    let (min, max) = T::bounds();

    // Check range first
    if let Some(failure) = source.check_i128_range(min, max) {
        return Err(failure);
    }

    // Try conversion
    numeric_cast(source).ok_or(NumericCastFailure::PosOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::half::Half;

    #[test]
    fn test_integer_compare_less_same_sign() {
        assert!(integer_compare_less(5i32, 10i32));
        assert!(!integer_compare_less(10i32, 5i32));
        assert!(integer_compare_less(5u32, 10u32));
    }

    #[test]
    fn test_integer_compare_less_mixed_sign() {
        // Negative signed always less than unsigned
        assert!(integer_compare_less(-1i32, 0u32));
        assert!(integer_compare_less(-100i32, 1u32));

        // Positive comparisons work correctly
        assert!(integer_compare_less(5i32, 10u32));
        assert!(!integer_compare_less(10u32, 5i32));
    }

    #[test]
    fn test_numeric_cast_int_to_int() {
        // In-range
        let x: Option<u8> = numeric_cast(100i32);
        assert_eq!(x, Some(100u8));

        // At boundary
        let x: Option<u8> = numeric_cast(255i32);
        assert_eq!(x, Some(255u8));

        // Overflow
        let x: Option<u8> = numeric_cast(256i32);
        assert_eq!(x, None);

        // Underflow (negative to unsigned)
        let x: Option<u32> = numeric_cast(-1i32);
        assert_eq!(x, None);
    }

    #[test]
    fn test_numeric_cast_float_to_int() {
        // In-range
        let x: Option<i32> = numeric_cast(100.5f64);
        assert_eq!(x, Some(100i32));

        // NaN
        let x: Option<i32> = numeric_cast(f64::NAN);
        assert_eq!(x, None);

        // Infinity
        let x: Option<i32> = numeric_cast(f64::INFINITY);
        assert_eq!(x, None);
    }

    #[test]
    fn test_numeric_cast_int_to_float() {
        // Always succeeds (no range checking)
        let x: Option<f32> = numeric_cast(100i32);
        assert_eq!(x, Some(100.0f32));

        let x: Option<f64> = numeric_cast(i64::MAX);
        assert!(x.is_some());
    }

    #[test]
    fn test_numeric_cast_with_reason() {
        let x: Result<u8, _> = numeric_cast_with_reason(300i32);
        assert_eq!(x, Err(NumericCastFailure::PosOverflow));

        let x: Result<u32, _> = numeric_cast_with_reason(-5i32);
        assert_eq!(x, Err(NumericCastFailure::NegOverflow));

        let x: Result<i32, _> = numeric_cast_with_reason(f64::NAN);
        assert_eq!(x, Err(NumericCastFailure::NaN));
    }

    #[test]
    fn test_widening_conversions() {
        // u8 -> u16 always works
        let x: Option<u16> = numeric_cast(255u8);
        assert_eq!(x, Some(255u16));

        // i8 -> i32 always works
        let x: Option<i32> = numeric_cast(-128i8);
        assert_eq!(x, Some(-128i32));
    }

    #[test]
    fn test_numeric_cast_half() {
        // Half as source -> int
        let h = Half::from_f32(100.0);
        let x: Option<i32> = numeric_cast(h);
        assert_eq!(x, Some(100i32));

        // Half as source -> float
        let h = Half::from_f32(3.14);
        let x: Option<f32> = numeric_cast(h);
        assert!(x.map(|f| (f - 3.14).abs() < 0.01).unwrap_or(false));

        // int/float as source -> Half
        let x: Option<Half> = numeric_cast(42i32);
        assert_eq!(x.map(Half::to_f32), Some(42.0f32));

        let x: Option<Half> = numeric_cast(1.5f32);
        assert!(x.map(|h| (h.to_f32() - 1.5).abs() < 0.01).unwrap_or(false));

        // Half NaN/Infinity -> int fails
        let x: Option<i32> = numeric_cast(Half::NAN);
        assert_eq!(x, None);

        let x: Option<i32> = numeric_cast(Half::POS_INF);
        assert_eq!(x, None);
    }
}
