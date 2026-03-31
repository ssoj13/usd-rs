//! Half-precision (16-bit) floating-point type.
//!
//! This module provides `Half`, a 16-bit IEEE 754 half-precision floating-point type.
//! Half-precision floats are useful for storing large amounts of floating-point data
//! where full precision is not needed (e.g., vertex data, texture coordinates).
//!
//! # Format
//!
//! ```text
//! 15 (msb)
//! |
//! | 14  10
//! | |   |
//! | |   | 9        0 (lsb)
//! | |   | |        |
//! X XXXXX XXXXXXXXXX
//!
//! s e     m
//! ```
//!
//! - `s`: Sign bit (1 bit)
//! - `e`: Exponent (5 bits, bias 15)
//! - `m`: Mantissa/significand (10 bits)
//!
//! # Range and Precision
//!
//! - **Smallest positive (denormalized):** ~5.96e-8
//! - **Smallest positive normalized:** ~6.10e-5
//! - **Largest positive:** 65504.0
//! - **Epsilon (1.0 + e != 1.0):** ~0.00097656
//! - **Exact integer range:** -2048 to +2048
//!
//! # Examples
//!
//! ```
//! use usd_gf::half::Half;
//!
//! // Create from f32
//! let h = Half::from_f32(3.14);
//!
//! // Convert back to f32 (lossless)
//! let f: f32 = h.to_f32();
//!
//! // Arithmetic operations
//! let a = Half::from_f32(2.0);
//! let b = Half::from_f32(3.0);
//! let c = a + b;
//! assert!((c.to_f32() - 5.0).abs() < 0.01);
//!
//! // Classification
//! assert!(Half::POS_INF.is_infinite());
//! assert!(Half::NAN.is_nan());
//! ```

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 16-bit floating-point number (IEEE 754 half-precision).
///
/// This type wraps the `half` crate's `f16` type and provides the same API
/// as OpenUSD's `GfHalf` type.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Half(half::f16);

/// Alias for Half (matches OpenUSD naming).
pub type GfHalf = Half;

// Constants matching OpenUSD's PXR_HALF_* macros
impl Half {
    /// Smallest positive half value (denormalized).
    pub const MIN_POSITIVE: f32 = 5.960_464_5e-8;

    /// Smallest positive normalized half value.
    pub const MIN_POSITIVE_NORMAL: f32 = 6.103_515_6e-5;

    /// Largest positive half value.
    pub const MAX: f32 = 65504.0;

    /// Smallest positive epsilon (1.0 + epsilon != 1.0).
    pub const EPSILON: f32 = 0.000_976_56;

    /// Number of mantissa digits (including hidden 1).
    pub const MANT_DIG: u32 = 11;

    /// Number of base-10 digits that can be represented without change.
    pub const DIG: u32 = 3;

    /// Number of base-10 digits needed to represent all distinct values.
    pub const DECIMAL_DIG: u32 = 5;

    /// Base of the exponent.
    pub const RADIX: u32 = 2;

    /// Minimum exponent.
    pub const MIN_EXP: i32 = -13;

    /// Maximum exponent.
    pub const MAX_EXP: i32 = 16;

    /// Minimum base-10 exponent.
    pub const MIN_10_EXP: i32 = -4;

    /// Maximum base-10 exponent.
    pub const MAX_10_EXP: i32 = 4;

    /// Zero.
    pub const ZERO: Half = Half(half::f16::ZERO);

    /// One.
    pub const ONE: Half = Half(half::f16::ONE);

    /// Negative zero.
    pub const NEG_ZERO: Half = Half(half::f16::NEG_ZERO);

    /// Positive infinity.
    pub const POS_INF: Half = Half(half::f16::INFINITY);

    /// Negative infinity.
    pub const NEG_INF: Half = Half(half::f16::NEG_INFINITY);

    /// Quiet NaN (not a number).
    pub const NAN: Half = Half(half::f16::NAN);
}

impl Half {
    /// Creates a new Half from its raw bit representation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// let h = Half::from_bits(0x3C00); // 1.0
    /// assert_eq!(h.to_f32(), 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(half::f16::from_bits(bits))
    }

    /// Returns the raw bit representation of the half-precision number.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// let h = Half::from_f32(1.0);
    /// assert_eq!(h.bits(), 0x3C00);
    /// ```
    #[inline]
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0.to_bits()
    }

    /// Sets the raw bit representation.
    #[inline]
    pub fn set_bits(&mut self, bits: u16) {
        self.0 = half::f16::from_bits(bits);
    }

    /// Creates a Half from an f32 value.
    ///
    /// Values outside the representable range are clamped to infinity.
    /// Values too small are converted to zero or denormalized numbers.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// let h = Half::from_f32(3.14);
    /// assert!((h.to_f32() - 3.14).abs() < 0.01);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_f32(value: f32) -> Self {
        Self(half::f16::from_f32(value))
    }

    /// Creates a Half from an f64 value.
    #[inline]
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        Self(half::f16::from_f64(value))
    }

    /// Converts to f32.
    ///
    /// This conversion is lossless - all half values can be exactly
    /// represented as f32.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// let h = Half::from_f32(2.5);
    /// assert_eq!(h.to_f32(), 2.5);
    /// ```
    #[inline]
    #[must_use]
    pub fn to_f32(self) -> f32 {
        self.0.to_f32()
    }

    /// Converts to f64.
    #[inline]
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.0.to_f64()
    }

    /// Returns true if this is a finite number (not infinity or NaN).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(Half::from_f32(1.0).is_finite());
    /// assert!(!Half::POS_INF.is_finite());
    /// assert!(!Half::NAN.is_finite());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// Returns true if this is a normalized number.
    ///
    /// A normalized number has a non-zero exponent (1-30).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(Half::from_f32(1.0).is_normalized());
    /// assert!(!Half::from_bits(0x0001).is_normalized()); // Denormalized
    /// ```
    #[inline]
    #[must_use]
    pub fn is_normalized(self) -> bool {
        self.0.is_normal()
    }

    /// Returns true if this is a denormalized number.
    ///
    /// A denormalized number has exponent 0 and non-zero mantissa.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(!Half::from_f32(1.0).is_denormalized());
    /// assert!(Half::from_bits(0x0001).is_denormalized());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_denormalized(self) -> bool {
        // Exponent is 0 and mantissa is non-zero
        let e = (self.bits() >> 10) & 0x1F;
        let m = self.bits() & 0x3FF;
        e == 0 && m != 0
    }

    /// Returns true if this is zero (positive or negative).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(Half::ZERO.is_zero());
    /// assert!(Half::NEG_ZERO.is_zero());
    /// assert!(!Half::ONE.is_zero());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.bits() & 0x7FFF == 0
    }

    /// Returns true if this is NaN (not a number).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(Half::NAN.is_nan());
    /// assert!(!Half::from_f32(1.0).is_nan());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_nan(self) -> bool {
        self.0.is_nan()
    }

    /// Returns true if this is positive or negative infinity.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(Half::POS_INF.is_infinite());
    /// assert!(Half::NEG_INF.is_infinite());
    /// assert!(!Half::from_f32(1.0).is_infinite());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_infinite(self) -> bool {
        self.0.is_infinite()
    }

    /// Returns true if the sign bit is set (negative).
    ///
    /// Note: Negative zero returns true.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// assert!(!Half::from_f32(1.0).is_negative());
    /// assert!(Half::from_f32(-1.0).is_negative());
    /// assert!(Half::NEG_ZERO.is_negative());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_negative(self) -> bool {
        self.0.is_sign_negative()
    }

    /// Returns true if the sign bit is not set (positive or zero).
    #[inline]
    #[must_use]
    pub fn is_positive(self) -> bool {
        self.0.is_sign_positive()
    }

    /// Returns positive infinity.
    #[inline]
    #[must_use]
    pub const fn pos_inf() -> Self {
        Self::POS_INF
    }

    /// Returns negative infinity.
    #[inline]
    #[must_use]
    pub const fn neg_inf() -> Self {
        Self::NEG_INF
    }

    /// Returns a quiet NaN (0x7FFF).
    #[inline]
    #[must_use]
    pub const fn q_nan() -> Self {
        Self::from_bits(0x7FFF)
    }

    /// Returns a signaling NaN (0x7DFF).
    #[inline]
    #[must_use]
    pub const fn s_nan() -> Self {
        Self::from_bits(0x7DFF)
    }

    /// Rounds to n-bit precision (n should be between 0 and 10).
    ///
    /// After rounding, the significand's (10-n) least significant bits
    /// will be zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::half::Half;
    ///
    /// let h = Half::from_f32(3.14159);
    /// let rounded = h.round(5);
    /// // Lower precision bits are zeroed
    /// ```
    #[inline]
    #[must_use]
    pub fn round(self, n: u32) -> Self {
        if n >= 10 {
            return self;
        }

        let s = self.bits() & 0x8000;
        let mut e = self.bits() & 0x7FFF;

        // Round to nearest value with ones only in the (10-n) most significant bits
        let shift = 9 - n;
        e >>= shift;
        e += e & 1;
        e <<= shift;

        // Check for exponent overflow
        if e >= 0x7C00 {
            // Overflow - truncate instead of rounding
            e = self.bits() & 0x7FFF;
            let trunc_shift = 10 - n;
            e >>= trunc_shift;
            e <<= trunc_shift;
        }

        Self::from_bits(s | e)
    }

    /// Returns the absolute value.
    #[inline]
    #[must_use]
    pub fn abs(self) -> Self {
        Self::from_bits(self.bits() & 0x7FFF)
    }

    /// Returns the sign of the value.
    ///
    /// Returns -1.0 for negative, 1.0 for positive, 0.0 for zero.
    #[inline]
    #[must_use]
    pub fn signum(self) -> Self {
        if self.is_nan() {
            self
        } else if self.is_zero() {
            Self::ZERO
        } else if self.is_negative() {
            Self::from_f32(-1.0)
        } else {
            Self::ONE
        }
    }

    /// Returns the minimum of two half values.
    #[inline]
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    /// Returns the maximum of two half values.
    #[inline]
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    /// Clamps the value to the given range.
    #[inline]
    #[must_use]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }
}

// Arithmetic operations

impl Neg for Half {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::from_bits(self.bits() ^ 0x8000)
    }
}

impl Add for Half {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::from_f32(self.to_f32() + rhs.to_f32())
    }
}

impl AddAssign for Half {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Half {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_f32(self.to_f32() - rhs.to_f32())
    }
}

impl SubAssign for Half {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for Half {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::from_f32(self.to_f32() * rhs.to_f32())
    }
}

impl MulAssign for Half {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Div for Half {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::from_f32(self.to_f32() / rhs.to_f32())
    }
}

impl DivAssign for Half {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

// Operations with f32

impl Add<f32> for Half {
    type Output = Self;

    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        Self::from_f32(self.to_f32() + rhs)
    }
}

impl AddAssign<f32> for Half {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        *self = Self::from_f32(self.to_f32() + rhs);
    }
}

impl Sub<f32> for Half {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Self::from_f32(self.to_f32() - rhs)
    }
}

impl SubAssign<f32> for Half {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        *self = Self::from_f32(self.to_f32() - rhs);
    }
}

impl Mul<f32> for Half {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::from_f32(self.to_f32() * rhs)
    }
}

impl MulAssign<f32> for Half {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        *self = Self::from_f32(self.to_f32() * rhs);
    }
}

impl Div<f32> for Half {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::from_f32(self.to_f32() / rhs)
    }
}

impl DivAssign<f32> for Half {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        *self = Self::from_f32(self.to_f32() / rhs);
    }
}

// Conversions

impl From<f32> for Half {
    #[inline]
    fn from(value: f32) -> Self {
        Self::from_f32(value)
    }
}

impl From<f64> for Half {
    #[inline]
    fn from(value: f64) -> Self {
        Self::from_f64(value)
    }
}

impl From<Half> for f32 {
    #[inline]
    fn from(value: Half) -> Self {
        value.to_f32()
    }
}

impl From<Half> for f64 {
    #[inline]
    fn from(value: Half) -> Self {
        value.to_f64()
    }
}

impl From<i32> for Half {
    #[inline]
    fn from(value: i32) -> Self {
        Self::from_f32(value as f32)
    }
}

// Display and Debug

impl fmt::Debug for Half {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Half({})", self.to_f32())
    }
}

impl fmt::Display for Half {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// Comparison (uses bit comparison for consistent ordering)

impl PartialEq for Half {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // NaN is not equal to itself
        if self.is_nan() || other.is_nan() {
            return false;
        }
        // +0 and -0 are equal
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.bits() == other.bits()
    }
}

impl PartialOrd for Half {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_f32().partial_cmp(&other.to_f32())
    }
}

// Hash (raw bits for OpenUSD parity: +0 and -0 hash differently)

impl Hash for Half {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits().hash(state);
    }
}

/// Computes a hash value for a half-precision number (raw bits).
///
/// Matches OpenUSD `hash_value(half)`.
#[inline]
#[must_use]
pub fn hash_value(h: Half) -> usize {
    h.bits() as usize
}

/// Returns the bit representation as a string, e.g. `"0 01111 0000000000"`.
/// Matches C++ `printBits(ostream, half)` format (sign, exponent, mantissa with spaces).
#[inline]
#[must_use]
pub fn print_bits(h: Half) -> String {
    let b = h.bits();
    let mut s = String::with_capacity(18);
    for i in (0..16).rev() {
        s.push(if (b >> i) & 1 != 0 { '1' } else { '0' });
        if i == 15 || i == 10 {
            s.push(' ');
        }
    }
    s
}

// num_traits implementations for compatibility with generic vector types

impl num_traits::Zero for Half {
    #[inline]
    fn zero() -> Self {
        Self::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        Half::is_zero(*self)
    }
}

impl num_traits::One for Half {
    #[inline]
    fn one() -> Self {
        Self::ONE
    }
}

impl std::ops::Rem for Half {
    type Output = Self;

    #[inline]
    fn rem(self, rhs: Self) -> Self::Output {
        Self::from_f32(self.to_f32() % rhs.to_f32())
    }
}

impl num_traits::Num for Half {
    type FromStrRadixErr = num_traits::ParseFloatError;

    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        f32::from_str_radix(str, radix).map(Self::from_f32)
    }
}

impl num_traits::ToPrimitive for Half {
    fn to_i64(&self) -> Option<i64> {
        Some(Half::to_f32(*self) as i64)
    }

    fn to_u64(&self) -> Option<u64> {
        let f = Half::to_f32(*self);
        if f < 0.0 { None } else { Some(f as u64) }
    }

    fn to_f32(&self) -> Option<f32> {
        Some(Half::to_f32(*self))
    }

    fn to_f64(&self) -> Option<f64> {
        Some(Half::to_f64(*self))
    }
}

impl num_traits::NumCast for Half {
    fn from<T: num_traits::ToPrimitive>(n: T) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
}

impl num_traits::Float for Half {
    fn nan() -> Self {
        Self::NAN
    }

    fn infinity() -> Self {
        Self::POS_INF
    }

    fn neg_infinity() -> Self {
        Self::NEG_INF
    }

    fn neg_zero() -> Self {
        Self::NEG_ZERO
    }

    fn min_value() -> Self {
        Self::from_bits(0xFBFF) // Largest negative half
    }

    fn min_positive_value() -> Self {
        Self::from_bits(0x0001) // Smallest positive denormalized
    }

    fn max_value() -> Self {
        Self::from_bits(0x7BFF) // Largest positive half
    }

    fn is_nan(self) -> bool {
        Half::is_nan(self)
    }

    fn is_infinite(self) -> bool {
        Half::is_infinite(self)
    }

    fn is_finite(self) -> bool {
        Half::is_finite(self)
    }

    fn is_normal(self) -> bool {
        Half::is_normalized(self)
    }

    fn classify(self) -> std::num::FpCategory {
        if self.is_nan() {
            std::num::FpCategory::Nan
        } else if self.is_infinite() {
            std::num::FpCategory::Infinite
        } else if self.is_zero() {
            std::num::FpCategory::Zero
        } else if self.is_denormalized() {
            std::num::FpCategory::Subnormal
        } else {
            std::num::FpCategory::Normal
        }
    }

    fn floor(self) -> Self {
        Self::from_f32(self.to_f32().floor())
    }

    fn ceil(self) -> Self {
        Self::from_f32(self.to_f32().ceil())
    }

    fn round(self) -> Self {
        Self::from_f32(self.to_f32().round())
    }

    fn trunc(self) -> Self {
        Self::from_f32(self.to_f32().trunc())
    }

    fn fract(self) -> Self {
        Self::from_f32(self.to_f32().fract())
    }

    fn abs(self) -> Self {
        Half::abs(self)
    }

    fn signum(self) -> Self {
        Half::signum(self)
    }

    fn is_sign_positive(self) -> bool {
        Half::is_positive(self)
    }

    fn is_sign_negative(self) -> bool {
        Half::is_negative(self)
    }

    fn mul_add(self, a: Self, b: Self) -> Self {
        Self::from_f32(self.to_f32().mul_add(a.to_f32(), b.to_f32()))
    }

    fn recip(self) -> Self {
        Self::from_f32(self.to_f32().recip())
    }

    fn powi(self, n: i32) -> Self {
        Self::from_f32(self.to_f32().powi(n))
    }

    fn powf(self, n: Self) -> Self {
        Self::from_f32(self.to_f32().powf(n.to_f32()))
    }

    fn sqrt(self) -> Self {
        Self::from_f32(self.to_f32().sqrt())
    }

    fn exp(self) -> Self {
        Self::from_f32(self.to_f32().exp())
    }

    fn exp2(self) -> Self {
        Self::from_f32(self.to_f32().exp2())
    }

    fn ln(self) -> Self {
        Self::from_f32(self.to_f32().ln())
    }

    fn log(self, base: Self) -> Self {
        Self::from_f32(self.to_f32().log(base.to_f32()))
    }

    fn log2(self) -> Self {
        Self::from_f32(self.to_f32().log2())
    }

    fn log10(self) -> Self {
        Self::from_f32(self.to_f32().log10())
    }

    fn max(self, other: Self) -> Self {
        Half::max(self, other)
    }

    fn min(self, other: Self) -> Self {
        Half::min(self, other)
    }

    fn abs_sub(self, other: Self) -> Self {
        Self::from_f32((self.to_f32() - other.to_f32()).abs())
    }

    fn cbrt(self) -> Self {
        Self::from_f32(self.to_f32().cbrt())
    }

    fn hypot(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().hypot(other.to_f32()))
    }

    fn sin(self) -> Self {
        Self::from_f32(self.to_f32().sin())
    }

    fn cos(self) -> Self {
        Self::from_f32(self.to_f32().cos())
    }

    fn tan(self) -> Self {
        Self::from_f32(self.to_f32().tan())
    }

    fn asin(self) -> Self {
        Self::from_f32(self.to_f32().asin())
    }

    fn acos(self) -> Self {
        Self::from_f32(self.to_f32().acos())
    }

    fn atan(self) -> Self {
        Self::from_f32(self.to_f32().atan())
    }

    fn atan2(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().atan2(other.to_f32()))
    }

    fn sin_cos(self) -> (Self, Self) {
        let (s, c) = self.to_f32().sin_cos();
        (Self::from_f32(s), Self::from_f32(c))
    }

    fn exp_m1(self) -> Self {
        Self::from_f32(self.to_f32().exp_m1())
    }

    fn ln_1p(self) -> Self {
        Self::from_f32(self.to_f32().ln_1p())
    }

    fn sinh(self) -> Self {
        Self::from_f32(self.to_f32().sinh())
    }

    fn cosh(self) -> Self {
        Self::from_f32(self.to_f32().cosh())
    }

    fn tanh(self) -> Self {
        Self::from_f32(self.to_f32().tanh())
    }

    fn asinh(self) -> Self {
        Self::from_f32(self.to_f32().asinh())
    }

    fn acosh(self) -> Self {
        Self::from_f32(self.to_f32().acosh())
    }

    fn atanh(self) -> Self {
        Self::from_f32(self.to_f32().atanh())
    }

    fn integer_decode(self) -> (u64, i16, i8) {
        // Decode half-precision IEEE 754 format
        let bits = self.bits();
        let sign = if bits >> 15 != 0 { -1i8 } else { 1i8 };
        let exponent = ((bits >> 10) & 0x1F) as i16;
        let mantissa = (bits & 0x3FF) as u64;

        if exponent == 0 {
            // Denormalized or zero
            (mantissa, -24, sign)
        } else if exponent == 31 {
            // Infinity or NaN
            (mantissa, i16::MAX, sign)
        } else {
            // Normalized: add implicit leading 1
            (mantissa | 0x400, exponent - 25, sign)
        }
    }

    fn epsilon() -> Self {
        Self::from_f32(Self::EPSILON)
    }

    fn to_degrees(self) -> Self {
        Self::from_f32(self.to_f32().to_degrees())
    }

    fn to_radians(self) -> Self {
        Self::from_f32(self.to_f32().to_radians())
    }
}

impl std::ops::RemAssign for Half {
    #[inline]
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_f32_roundtrip() {
        let values = [0.0f32, 1.0, -1.0, 0.5, 2.0, 3.0, 100.0, 0.001];
        for &v in &values {
            let h = Half::from_f32(v);
            let back = h.to_f32();
            // Half has limited precision, so we check approximate equality
            assert!(
                (back - v).abs() < 0.01 || (back - v).abs() / v.abs() < 0.01,
                "roundtrip failed for {v}: got {back}"
            );
        }
    }

    #[test]
    fn test_special_values() {
        assert!(Half::POS_INF.is_infinite());
        assert!(!Half::POS_INF.is_negative());
        assert!(Half::NEG_INF.is_infinite());
        assert!(Half::NEG_INF.is_negative());
        assert!(Half::NAN.is_nan());
        assert!(Half::ZERO.is_zero());
        assert!(Half::NEG_ZERO.is_zero());
        assert!(Half::NEG_ZERO.is_negative());
    }

    #[test]
    fn test_classification() {
        assert!(Half::from_f32(1.0).is_normalized());
        assert!(Half::from_f32(1.0).is_finite());
        assert!(!Half::from_f32(1.0).is_nan());
        assert!(!Half::from_f32(1.0).is_infinite());

        // Denormalized number
        let denorm = Half::from_bits(0x0001);
        assert!(denorm.is_denormalized());
        assert!(denorm.is_finite());
        assert!(!denorm.is_normalized());
    }

    #[test]
    fn test_arithmetic() {
        let a = Half::from_f32(2.0);
        let b = Half::from_f32(3.0);

        assert!((a + b).to_f32() - 5.0 < 0.001);
        assert!((b - a).to_f32() - 1.0 < 0.001);
        assert!((a * b).to_f32() - 6.0 < 0.001);
        assert!((b / a).to_f32() - 1.5 < 0.001);
    }

    #[test]
    fn test_negation() {
        let a = Half::from_f32(3.14);
        let neg_a = -a;
        assert!(neg_a.is_negative());
        assert!((neg_a.to_f32() + 3.14).abs() < 0.01);
    }

    #[test]
    fn test_bits() {
        // 1.0 in half-precision: sign=0, exp=15 (0b01111), mantissa=0
        // bits = 0 01111 0000000000 = 0x3C00
        let h = Half::from_f32(1.0);
        assert_eq!(h.bits(), 0x3C00);

        let h2 = Half::from_bits(0x3C00);
        assert_eq!(h2.to_f32(), 1.0);
    }

    #[test]
    fn test_round() {
        let h = Half::from_f32(3.14159);
        let r5 = h.round(5);
        let r0 = h.round(0);
        // Lower precision should have fewer significant bits
        assert_ne!(h.bits(), r5.bits());
        assert_ne!(h.bits(), r0.bits());
        // Full precision should be unchanged
        let r10 = h.round(10);
        assert_eq!(h.bits(), r10.bits());
    }

    #[test]
    fn test_abs() {
        assert_eq!(Half::from_f32(-3.0).abs().to_f32(), 3.0);
        assert_eq!(Half::from_f32(3.0).abs().to_f32(), 3.0);
    }

    #[test]
    fn test_equality() {
        assert_eq!(Half::from_f32(1.0), Half::from_f32(1.0));
        assert_ne!(Half::from_f32(1.0), Half::from_f32(2.0));
        // +0 and -0 should be equal
        assert_eq!(Half::ZERO, Half::NEG_ZERO);
        // NaN should not equal itself
        assert_ne!(Half::NAN, Half::NAN);
    }

    #[test]
    fn test_hash_value() {
        let h = Half::from_f32(1.0);
        assert_eq!(hash_value(h), 0x3C00);
    }

    #[test]
    fn test_assign_ops() {
        let mut h = Half::from_f32(2.0);
        h += Half::from_f32(1.0);
        assert!((h.to_f32() - 3.0).abs() < 0.001);

        h -= Half::from_f32(1.0);
        assert!((h.to_f32() - 2.0).abs() < 0.001);

        h *= Half::from_f32(2.0);
        assert!((h.to_f32() - 4.0).abs() < 0.001);

        h /= Half::from_f32(2.0);
        assert!((h.to_f32() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_f32_ops() {
        let h = Half::from_f32(2.0);

        assert!((h + 1.0f32).to_f32() - 3.0 < 0.001);
        assert!((h - 1.0f32).to_f32() - 1.0 < 0.001);
        assert!((h * 2.0f32).to_f32() - 4.0 < 0.001);
        assert!((h / 2.0f32).to_f32() - 1.0 < 0.001);
    }

    #[test]
    fn test_min_max_clamp() {
        let a = Half::from_f32(1.0);
        let b = Half::from_f32(3.0);

        assert_eq!(a.min(b).to_f32(), 1.0);
        assert_eq!(a.max(b).to_f32(), 3.0);

        let c = Half::from_f32(5.0);
        assert_eq!(c.clamp(a, b).to_f32(), 3.0);

        let d = Half::from_f32(0.5);
        assert_eq!(d.clamp(a, b).to_f32(), 1.0);
    }
}
