//! Math utilities.
//!
//! Provides low-level math functions and bit manipulation utilities.

/// Smallest value e such that 1+e^2 == 1, using floats (IEEE754).
pub const MIN_FLOAT_EPS_SQR: f32 = 0.000244141;

/// Pi constant.
pub const PI: f64 = std::f64::consts::PI;

/// Returns the IEEE-754 bit pattern of a single precision value.
///
/// # Examples
///
/// ```
/// use usd_arch::float_to_bit_pattern;
///
/// let bits = float_to_bit_pattern(1.0f32);
/// assert_eq!(bits, 0x3f800000);
/// ```
#[inline]
#[must_use]
pub fn float_to_bit_pattern(v: f32) -> u32 {
    v.to_bits()
}

/// Returns the single precision value from an IEEE-754 bit pattern.
///
/// # Examples
///
/// ```
/// use usd_arch::bit_pattern_to_float;
///
/// let v = bit_pattern_to_float(0x3f800000);
/// assert_eq!(v, 1.0f32);
/// ```
#[inline]
#[must_use]
pub fn bit_pattern_to_float(v: u32) -> f32 {
    f32::from_bits(v)
}

/// Returns the IEEE-754 bit pattern of a double precision value.
///
/// # Examples
///
/// ```
/// use usd_arch::double_to_bit_pattern;
///
/// let bits = double_to_bit_pattern(1.0f64);
/// assert_eq!(bits, 0x3ff0000000000000);
/// ```
#[inline]
#[must_use]
pub fn double_to_bit_pattern(v: f64) -> u64 {
    v.to_bits()
}

/// Returns the double precision value from an IEEE-754 bit pattern.
///
/// # Examples
///
/// ```
/// use usd_arch::bit_pattern_to_double;
///
/// let v = bit_pattern_to_double(0x3ff0000000000000);
/// assert_eq!(v, 1.0f64);
/// ```
#[inline]
#[must_use]
pub fn bit_pattern_to_double(v: u64) -> f64 {
    f64::from_bits(v)
}

/// Computes sine and cosine simultaneously (f32).
///
/// # Examples
///
/// ```
/// use usd_arch::sin_cos_f32;
///
/// let (s, c) = sin_cos_f32(0.0);
/// assert!((s - 0.0).abs() < 1e-6);
/// assert!((c - 1.0).abs() < 1e-6);
/// ```
#[inline]
#[must_use]
pub fn sin_cos_f32(v: f32) -> (f32, f32) {
    (v.sin(), v.cos())
}

/// Computes sine and cosine simultaneously (f64).
///
/// # Examples
///
/// ```
/// use usd_arch::sin_cos;
///
/// let (s, c) = sin_cos(0.0);
/// assert!((s - 0.0).abs() < 1e-10);
/// assert!((c - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn sin_cos(v: f64) -> (f64, f64) {
    (v.sin(), v.cos())
}

/// Returns the sign of a number: -1, 0, or 1.
///
/// # Examples
///
/// ```
/// use usd_arch::sign;
///
/// assert_eq!(sign(-5.0), -1);
/// assert_eq!(sign(0.0), 0);
/// assert_eq!(sign(5.0), 1);
/// ```
#[inline]
#[must_use]
pub fn sign<T: PartialOrd + Default + From<i8>>(val: T) -> i32 {
    let zero = T::default();
    if val > zero {
        1
    } else if val < zero {
        -1
    } else {
        0
    }
}

/// Returns the sign of a number as the same type.
///
/// # Examples
///
/// ```
/// use usd_arch::signum;
///
/// assert_eq!(signum(-5.0_f64), -1.0);
/// assert_eq!(signum(0.0_f64), 0.0);
/// assert_eq!(signum(5.0_f64), 1.0);
/// ```
#[inline]
#[must_use]
pub fn signum<T: num_traits::Signed>(val: T) -> T {
    val.signum()
}

/// Counts the number of trailing zero bits in an integer.
///
/// # Examples
///
/// ```
/// use usd_arch::count_trailing_zeros;
///
/// assert_eq!(count_trailing_zeros(0b1000u32), 3);
/// assert_eq!(count_trailing_zeros(0b1010u32), 1);
/// assert_eq!(count_trailing_zeros(1u32), 0);
/// ```
#[inline]
#[must_use]
pub fn count_trailing_zeros(val: u32) -> u32 {
    if val == 0 { 32 } else { val.trailing_zeros() }
}

/// Counts the number of trailing zero bits in a 64-bit integer.
#[inline]
#[must_use]
pub fn count_trailing_zeros_64(val: u64) -> u32 {
    if val == 0 { 64 } else { val.trailing_zeros() }
}

/// Counts the number of leading zero bits in an integer.
///
/// # Examples
///
/// ```
/// use usd_arch::count_leading_zeros;
///
/// assert_eq!(count_leading_zeros(1u32), 31);
/// assert_eq!(count_leading_zeros(0x80000000u32), 0);
/// ```
#[inline]
#[must_use]
pub fn count_leading_zeros(val: u32) -> u32 {
    val.leading_zeros()
}

/// Counts the number of leading zero bits in a 64-bit integer.
#[inline]
#[must_use]
pub fn count_leading_zeros_64(val: u64) -> u32 {
    val.leading_zeros()
}

/// Counts the number of set bits (population count) in an integer.
///
/// # Examples
///
/// ```
/// use usd_arch::popcount;
///
/// assert_eq!(popcount(0b1010_1010u32), 4);
/// assert_eq!(popcount(0xFFFFFFFFu32), 32);
/// ```
#[inline]
#[must_use]
pub fn popcount(val: u32) -> u32 {
    val.count_ones()
}

/// Counts the number of set bits in a 64-bit integer.
#[inline]
#[must_use]
pub fn popcount_64(val: u64) -> u32 {
    val.count_ones()
}

/// Rounds up to the next power of two.
///
/// Returns the same value if already a power of two.
/// Returns 0 for input 0.
///
/// # Examples
///
/// ```
/// use usd_arch::next_power_of_two;
///
/// assert_eq!(next_power_of_two(5u32), 8);
/// assert_eq!(next_power_of_two(8u32), 8);
/// assert_eq!(next_power_of_two(1u32), 1);
/// ```
#[inline]
#[must_use]
pub fn next_power_of_two(val: u32) -> u32 {
    if val == 0 { 0 } else { val.next_power_of_two() }
}

/// Rounds up to the next power of two (64-bit).
#[inline]
#[must_use]
pub fn next_power_of_two_64(val: u64) -> u64 {
    if val == 0 { 0 } else { val.next_power_of_two() }
}

/// Checks if a value is a power of two.
///
/// # Examples
///
/// ```
/// use usd_arch::is_power_of_two;
///
/// assert!(is_power_of_two(1u32));
/// assert!(is_power_of_two(2u32));
/// assert!(is_power_of_two(256u32));
/// assert!(!is_power_of_two(0u32));
/// assert!(!is_power_of_two(3u32));
/// ```
#[inline]
#[must_use]
pub fn is_power_of_two(val: u32) -> bool {
    val != 0 && (val & (val - 1)) == 0
}

/// Checks if a 64-bit value is a power of two.
#[inline]
#[must_use]
pub fn is_power_of_two_64(val: u64) -> bool {
    val != 0 && (val & (val - 1)) == 0
}

/// Returns the floor of log base 2 of a value.
///
/// Returns 0 for input 0 (mathematically undefined, but useful default).
///
/// # Examples
///
/// ```
/// use usd_arch::log2_floor;
///
/// assert_eq!(log2_floor(1u32), 0);
/// assert_eq!(log2_floor(2u32), 1);
/// assert_eq!(log2_floor(7u32), 2);
/// assert_eq!(log2_floor(8u32), 3);
/// ```
#[inline]
#[must_use]
pub fn log2_floor(val: u32) -> u32 {
    if val == 0 {
        0
    } else {
        31 - val.leading_zeros()
    }
}

/// Returns the floor of log base 2 of a 64-bit value.
#[inline]
#[must_use]
pub fn log2_floor_64(val: u64) -> u32 {
    if val == 0 {
        0
    } else {
        63 - val.leading_zeros()
    }
}

/// Returns the ceiling of log base 2 of a value.
///
/// # Examples
///
/// ```
/// use usd_arch::log2_ceil;
///
/// assert_eq!(log2_ceil(1u32), 0);
/// assert_eq!(log2_ceil(2u32), 1);
/// assert_eq!(log2_ceil(3u32), 2);
/// assert_eq!(log2_ceil(8u32), 3);
/// ```
#[inline]
#[must_use]
pub fn log2_ceil(val: u32) -> u32 {
    if val <= 1 {
        0
    } else {
        32 - (val - 1).leading_zeros()
    }
}

/// Returns the ceiling of log base 2 of a 64-bit value.
#[inline]
#[must_use]
pub fn log2_ceil_64(val: u64) -> u32 {
    if val <= 1 {
        0
    } else {
        64 - (val - 1).leading_zeros()
    }
}

/// Byte swap (reverse byte order) of a 16-bit value.
#[inline]
#[must_use]
pub fn byte_swap_16(val: u16) -> u16 {
    val.swap_bytes()
}

/// Byte swap (reverse byte order) of a 32-bit value.
#[inline]
#[must_use]
pub fn byte_swap_32(val: u32) -> u32 {
    val.swap_bytes()
}

/// Byte swap (reverse byte order) of a 64-bit value.
#[inline]
#[must_use]
pub fn byte_swap_64(val: u64) -> u64 {
    val.swap_bytes()
}

/// Rotates bits left by the specified amount.
#[inline]
#[must_use]
pub fn rotate_left_32(val: u32, bits: u32) -> u32 {
    val.rotate_left(bits)
}

/// Rotates bits left by the specified amount (64-bit).
#[inline]
#[must_use]
pub fn rotate_left_64(val: u64, bits: u32) -> u64 {
    val.rotate_left(bits)
}

/// Rotates bits right by the specified amount.
#[inline]
#[must_use]
pub fn rotate_right_32(val: u32, bits: u32) -> u32 {
    val.rotate_right(bits)
}

/// Rotates bits right by the specified amount (64-bit).
#[inline]
#[must_use]
pub fn rotate_right_64(val: u64, bits: u32) -> u64 {
    val.rotate_right(bits)
}

/// Clamps a value to a range.
///
/// # Examples
///
/// ```
/// use usd_arch::clamp;
///
/// assert_eq!(clamp(5, 0, 10), 5);
/// assert_eq!(clamp(-5, 0, 10), 0);
/// assert_eq!(clamp(15, 0, 10), 10);
/// ```
#[inline]
#[must_use]
pub fn clamp<T: Ord>(val: T, min: T, max: T) -> T {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

/// Clamps a floating-point value to a range.
#[inline]
#[must_use]
pub fn clamp_f32(val: f32, min: f32, max: f32) -> f32 {
    val.clamp(min, max)
}

/// Clamps a double-precision value to a range.
#[inline]
#[must_use]
pub fn clamp_f64(val: f64, min: f64, max: f64) -> f64 {
    val.clamp(min, max)
}

/// Linear interpolation between two values.
///
/// # Examples
///
/// ```
/// use usd_arch::lerp;
///
/// assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
/// assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
/// assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
/// ```
#[inline]
#[must_use]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Linear interpolation (f32 version).
#[inline]
#[must_use]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Provides traits for numeric operations.
mod num_traits {
    pub trait Signed: Copy {
        fn signum(self) -> Self;
    }

    impl Signed for f32 {
        fn signum(self) -> Self {
            if self > 0.0 {
                1.0
            } else if self < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
    }

    impl Signed for f64 {
        fn signum(self) -> Self {
            if self > 0.0 {
                1.0
            } else if self < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
    }

    impl Signed for i32 {
        fn signum(self) -> Self {
            if self > 0 {
                1
            } else if self < 0 {
                -1
            } else {
                0
            }
        }
    }

    impl Signed for i64 {
        fn signum(self) -> Self {
            if self > 0 {
                1
            } else if self < 0 {
                -1
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign() {
        assert_eq!(sign(-5.0_f64), -1);
        assert_eq!(sign(0.0_f64), 0);
        assert_eq!(sign(5.0_f64), 1);
        assert_eq!(sign(-5_i32), -1);
        assert_eq!(sign(0_i32), 0);
        assert_eq!(sign(5_i32), 1);
    }

    #[test]
    fn test_count_trailing_zeros() {
        assert_eq!(count_trailing_zeros(0), 32);
        assert_eq!(count_trailing_zeros(1), 0);
        assert_eq!(count_trailing_zeros(2), 1);
        assert_eq!(count_trailing_zeros(8), 3);
        assert_eq!(count_trailing_zeros(0b1010), 1);
    }

    #[test]
    fn test_count_leading_zeros() {
        assert_eq!(count_leading_zeros(0), 32);
        assert_eq!(count_leading_zeros(1), 31);
        assert_eq!(count_leading_zeros(0x80000000), 0);
    }

    #[test]
    fn test_popcount() {
        assert_eq!(popcount(0), 0);
        assert_eq!(popcount(1), 1);
        assert_eq!(popcount(0b1010_1010), 4);
        assert_eq!(popcount(0xFFFFFFFF), 32);
    }

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(0), 0);
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(8), 8);
    }

    #[test]
    fn test_is_power_of_two() {
        assert!(!is_power_of_two(0));
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(!is_power_of_two(3));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(256));
    }

    #[test]
    fn test_log2_floor() {
        assert_eq!(log2_floor(0), 0);
        assert_eq!(log2_floor(1), 0);
        assert_eq!(log2_floor(2), 1);
        assert_eq!(log2_floor(3), 1);
        assert_eq!(log2_floor(4), 2);
        assert_eq!(log2_floor(7), 2);
        assert_eq!(log2_floor(8), 3);
    }

    #[test]
    fn test_log2_ceil() {
        assert_eq!(log2_ceil(0), 0);
        assert_eq!(log2_ceil(1), 0);
        assert_eq!(log2_ceil(2), 1);
        assert_eq!(log2_ceil(3), 2);
        assert_eq!(log2_ceil(4), 2);
        assert_eq!(log2_ceil(5), 3);
        assert_eq!(log2_ceil(8), 3);
    }

    #[test]
    fn test_byte_swap() {
        assert_eq!(byte_swap_16(0x1234), 0x3412);
        assert_eq!(byte_swap_32(0x12345678), 0x78563412);
        assert_eq!(byte_swap_64(0x123456789ABCDEF0), 0xF0DEBC9A78563412);
    }

    #[test]
    fn test_rotate() {
        assert_eq!(rotate_left_32(0x12345678, 8), 0x34567812);
        assert_eq!(rotate_right_32(0x12345678, 8), 0x78123456);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-10);
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-10);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-10);
    }

    // Tests from OpenUSD testMath.cpp
    #[test]
    fn test_ieee754_float_compliance() {
        // Verify that the exponent and significand of float are IEEE-754 compliant
        assert_eq!(float_to_bit_pattern(5.6904566e-28f32), 0x12345678);
        assert_eq!(bit_pattern_to_float(0x12345678), 5.6904566e-28f32);
    }

    #[test]
    fn test_ieee754_double_compliance() {
        // Verify that the exponent and significand of double are IEEE-754 compliant
        assert_eq!(
            double_to_bit_pattern(5.6263470058989390e-221),
            0x1234567811223344u64
        );
        assert_eq!(
            bit_pattern_to_double(0x1234567811223344u64),
            5.6263470058989390e-221
        );
    }

    #[test]
    fn test_sign_from_openusd() {
        // From testMath.cpp
        assert_eq!(sign(-123i32), -1);
        assert_eq!(sign(123i32), 1);
        assert_eq!(sign(0i32), 0);
    }

    #[test]
    fn test_count_trailing_zeros_from_openusd() {
        // From testMath.cpp
        assert_eq!(count_trailing_zeros(1), 0);
        assert_eq!(count_trailing_zeros(2), 1);
        assert_eq!(count_trailing_zeros(3), 0);
        assert_eq!(count_trailing_zeros(4), 2);
        assert_eq!(count_trailing_zeros(5), 0);
        assert_eq!(count_trailing_zeros(6), 1);
        assert_eq!(count_trailing_zeros(7), 0);
        assert_eq!(count_trailing_zeros(8), 3);

        assert_eq!(count_trailing_zeros(65535), 0);
        assert_eq!(count_trailing_zeros(65536), 16);
    }

    #[test]
    fn test_count_trailing_zeros_64_from_openusd() {
        // From testMath.cpp
        assert_eq!(count_trailing_zeros_64(!((1u64 << 32) - 1)), 32);
        assert_eq!(count_trailing_zeros_64(1u64 << 63), 63);
    }

    #[test]
    fn test_sin_cos() {
        let (s, c) = sin_cos(0.0);
        assert!((s - 0.0).abs() < 1e-10);
        assert!((c - 1.0).abs() < 1e-10);

        let (s, c) = sin_cos(PI / 2.0);
        assert!((s - 1.0).abs() < 1e-10);
        assert!(c.abs() < 1e-10);

        let (s, c) = sin_cos(PI);
        assert!(s.abs() < 1e-10);
        assert!((c + 1.0).abs() < 1e-10);
    }
}
