//! Mathematical utility functions.
//!
//! This module provides basic math functions used throughout USD.
//! Most are thin wrappers around Rust's standard library math functions,
//! provided for API compatibility with C++ OpenUSD.
//!
//! # Examples
//!
//! ```
//! use usd_gf::math::*;
//!
//! // Angle conversions
//! let rad = degrees_to_radians(180.0);
//! assert!((rad - std::f64::consts::PI).abs() < 1e-10);
//!
//! // Interpolation
//! let mid = lerp(0.5, 0.0, 10.0);
//! assert!((mid - 5.0).abs() < 1e-10);
//!
//! // Clamping
//! let clamped = clamp(15.0, 0.0, 10.0);
//! assert_eq!(clamped, 10.0);
//! ```

use std::f64::consts::PI;

/// Checks if two values are within epsilon of each other.
///
/// # Examples
///
/// ```
/// use usd_gf::math::is_close;
///
/// assert!(is_close(1.0, 1.0001, 0.001));
/// assert!(!is_close(1.0, 1.1, 0.001));
/// ```
#[inline]
#[must_use]
pub fn is_close(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() < epsilon
}

/// Converts an angle from radians to degrees.
///
/// # Examples
///
/// ```
/// use usd_gf::math::radians_to_degrees;
/// use std::f64::consts::PI;
///
/// assert!((radians_to_degrees(PI) - 180.0).abs() < 1e-10);
/// assert!((radians_to_degrees(PI / 2.0) - 90.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn radians_to_degrees(radians: f64) -> f64 {
    radians * (180.0 / PI)
}

/// Converts an angle from degrees to radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::degrees_to_radians;
/// use std::f64::consts::PI;
///
/// assert!((degrees_to_radians(180.0) - PI).abs() < 1e-10);
/// assert!((degrees_to_radians(90.0) - PI / 2.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * (PI / 180.0)
}

/// Smooth step function using cubic Hermite interpolation.
///
/// Returns 0 if `val <= min`, and 1 if `val >= max`.
/// Between min and max, smoothly interpolates using a cubic Hermite curve.
///
/// # Arguments
///
/// * `min` - Lower bound of the range
/// * `max` - Upper bound of the range
/// * `val` - Value to evaluate
/// * `slope0` - Slope at min (default 0)
/// * `slope1` - Slope at max (default 0)
///
/// # Examples
///
/// ```
/// use usd_gf::math::smooth_step;
///
/// assert_eq!(smooth_step(0.0, 1.0, -0.5, 0.0, 0.0), 0.0);
/// assert_eq!(smooth_step(0.0, 1.0, 1.5, 0.0, 0.0), 1.0);
/// assert!((smooth_step(0.0, 1.0, 0.5, 0.0, 0.0) - 0.5).abs() < 0.01);
/// ```
#[must_use]
pub fn smooth_step(min: f64, max: f64, val: f64, slope0: f64, slope1: f64) -> f64 {
    if val <= min {
        return 0.0;
    }
    if val >= max {
        return 1.0;
    }

    // C++ parity: GfSmoothStep uses standard Hermite formulation
    //   p(h) = (2h^3 - 3h^2 + 1)*p0 + (h^3 - 2h^2 + h)*m0
    //        + (-2h^3 + 3h^2)*p1    + (h^3 - h^2)*m1
    // where p0=0, p1=1, and slopes are normalized by dividing by range.
    let dv = max - min;
    let h = (val - min) / dv;

    let h2 = h * h;
    let h3 = h2 * h;

    // p1 term
    let mut v = -2.0 * h3 + 3.0 * h2;

    // p0 is always zero, so h00 term is omitted

    if slope0 != 0.0 {
        // Normalize slope by dividing by range (NOT multiplying -- was the bug)
        let s0 = slope0 / dv;
        v += (h3 - 2.0 * h2 + h) * s0;
    }

    if slope1 != 0.0 {
        let s1 = slope1 / dv;
        v += (h3 - h2) * s1;
    }

    v
}

/// Smooth ramp with independently controllable shoulders.
///
/// Similar to smooth_step but uses a linear ramp with smooth parabolic
/// shoulders at each end.
///
/// # Arguments
///
/// * `tmin` - Start of the ramp
/// * `tmax` - End of the ramp (must be > tmin)
/// * `t` - Value to evaluate
/// * `w0` - Size of first shoulder as fraction of range (0-1)
/// * `w1` - Size of second shoulder as fraction of range (0-1)
///
/// Note: w0 + w1 must be <= 1
///
/// # Examples
///
/// ```
/// use usd_gf::math::smooth_ramp;
///
/// // Pure linear ramp
/// let v = smooth_ramp(0.0, 1.0, 0.5, 0.0, 0.0);
/// assert!((v - 0.5).abs() < 1e-10);
///
/// // Smooth ramp with 20% shoulders
/// let v = smooth_ramp(0.0, 1.0, 0.0, 0.2, 0.2);
/// assert_eq!(v, 0.0);
/// let v = smooth_ramp(0.0, 1.0, 1.0, 0.2, 0.2);
/// assert_eq!(v, 1.0);
/// ```
#[must_use]
pub fn smooth_ramp(tmin: f64, tmax: f64, t: f64, w0: f64, w1: f64) -> f64 {
    // Normalize t to [0, 1] range
    let range = tmax - tmin;
    if range <= 0.0 {
        return if t < tmin { 0.0 } else { 1.0 };
    }

    let tn = (t - tmin) / range;

    // Clamp to [0, 1]
    if tn <= 0.0 {
        return 0.0;
    }
    if tn >= 1.0 {
        return 1.0;
    }

    // Calculate slope
    let denom = 2.0 - w0 - w1;
    if denom <= 0.0 {
        return tn; // Fallback to linear
    }
    let s = 2.0 / denom;

    // Parabolic segment helper: g(t, w, s) = s * t² / (2 * w)
    let parabola = |x: f64, w: f64| -> f64 { if w <= 0.0 { 0.0 } else { s * x * x / (2.0 * w) } };

    // y value at shoulder edge
    let y0 = parabola(w0, w0);

    if tn < w0 {
        // First parabolic shoulder
        parabola(tn, w0)
    } else if tn > 1.0 - w1 {
        // Second parabolic shoulder (flipped)
        1.0 - parabola(1.0 - tn, w1)
    } else {
        // Linear middle section
        s * tn - y0
    }
}

/// Returns the square of a value (x * x).
///
/// # Examples
///
/// ```
/// use usd_gf::math::sqr;
///
/// assert_eq!(sqr(3.0), 9.0);
/// assert_eq!(sqr(-4.0), 16.0);
/// assert_eq!(sqr(0.0), 0.0);
/// ```
#[inline]
#[must_use]
pub fn sqr<T: std::ops::Mul<Output = T> + Copy>(x: T) -> T {
    x * x
}

/// Returns the sign of a value (-1, 0, or 1).
///
/// # Examples
///
/// ```
/// use usd_gf::math::sgn;
///
/// assert_eq!(sgn(5i32), 1);
/// assert_eq!(sgn(-3i32), -1);
/// assert_eq!(sgn(0i32), 0);
/// ```
#[inline]
#[must_use]
pub fn sgn<T: PartialOrd + Default + From<i8>>(v: T) -> T {
    let zero = T::default();
    if v < zero {
        T::from(-1i8)
    } else if v > zero {
        T::from(1i8)
    } else {
        zero
    }
}

/// Returns the square root of a value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::sqrt;
///
/// assert!((sqrt(4.0f64) - 2.0).abs() < 1e-10);
/// assert!((sqrt(9.0f32) - 3.0).abs() < 1e-6);
/// ```
#[inline]
#[must_use]
pub fn sqrt<T: num_traits::Float>(f: T) -> T {
    f.sqrt()
}

/// Returns e raised to the power of the value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::exp;
///
/// assert!((exp(0.0f64) - 1.0).abs() < 1e-10);
/// assert!((exp(1.0f64) - std::f64::consts::E).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn exp<T: num_traits::Float>(f: T) -> T {
    f.exp()
}

/// Returns the natural logarithm of a value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::log;
///
/// assert!((log(1.0f64)).abs() < 1e-10);
/// assert!((log(std::f64::consts::E) - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn log<T: num_traits::Float>(f: T) -> T {
    f.ln()
}

/// Returns the floor of a value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::floor;
///
/// assert_eq!(floor(3.7f64), 3.0);
/// assert_eq!(floor(-2.3f64), -3.0);
/// ```
#[inline]
#[must_use]
pub fn floor<T: num_traits::Float>(f: T) -> T {
    f.floor()
}

/// Returns the ceiling of a value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::ceil;
///
/// assert_eq!(ceil(3.2f64), 4.0);
/// assert_eq!(ceil(-2.7f64), -2.0);
/// ```
#[inline]
#[must_use]
pub fn ceil<T: num_traits::Float>(f: T) -> T {
    f.ceil()
}

/// Returns the absolute value.
///
/// # Examples
///
/// ```
/// use usd_gf::math::abs;
///
/// assert_eq!(abs(-3.5f64), 3.5);
/// assert_eq!(abs(2.5f64), 2.5);
/// ```
#[inline]
#[must_use]
pub fn abs<T: num_traits::Float>(f: T) -> T {
    f.abs()
}

/// Rounds a value to the nearest integer.
///
/// # Examples
///
/// ```
/// use usd_gf::math::round;
///
/// assert_eq!(round(3.4f64), 3.0);
/// assert_eq!(round(3.6f64), 4.0);
/// assert_eq!(round(-2.5f64), -3.0); // Rounds away from zero
/// ```
#[inline]
#[must_use]
pub fn round<T: num_traits::Float>(f: T) -> T {
    f.round()
}

/// Returns a value raised to a power.
///
/// # Examples
///
/// ```
/// use usd_gf::math::pow;
///
/// assert!((pow(2.0f64, 3.0) - 8.0).abs() < 1e-10);
/// assert!((pow(4.0f64, 0.5) - 2.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn pow<T: num_traits::Float>(f: T, p: T) -> T {
    f.powf(p)
}

/// Returns the sine of an angle in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::sin;
/// use std::f64::consts::PI;
///
/// assert!((sin(0.0f64)).abs() < 1e-10);
/// assert!((sin(PI / 2.0) - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn sin<T: num_traits::Float>(v: T) -> T {
    v.sin()
}

/// Returns the cosine of an angle in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::cos;
/// use std::f64::consts::PI;
///
/// assert!((cos(0.0f64) - 1.0).abs() < 1e-10);
/// assert!((cos(PI) + 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn cos<T: num_traits::Float>(v: T) -> T {
    v.cos()
}

/// Returns both sine and cosine of an angle in radians.
///
/// This can be more efficient than computing them separately.
///
/// # Examples
///
/// ```
/// use usd_gf::math::sin_cos;
/// use std::f64::consts::PI;
///
/// let (s, c) = sin_cos(PI / 4.0);
/// assert!((s - c).abs() < 1e-10); // sin(45°) == cos(45°)
/// ```
#[inline]
#[must_use]
pub fn sin_cos<T: num_traits::Float>(v: T) -> (T, T) {
    v.sin_cos()
}

/// Returns the tangent of an angle in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::tan;
/// use std::f64::consts::PI;
///
/// assert!((tan(0.0f64)).abs() < 1e-10);
/// assert!((tan(PI / 4.0) - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn tan<T: num_traits::Float>(v: T) -> T {
    v.tan()
}

/// Returns the arc sine (inverse sine) in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::asin;
/// use std::f64::consts::PI;
///
/// assert!((asin(0.0f64)).abs() < 1e-10);
/// assert!((asin(1.0f64) - PI / 2.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn asin<T: num_traits::Float>(v: T) -> T {
    v.asin()
}

/// Returns the arc cosine (inverse cosine) in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::acos;
/// use std::f64::consts::PI;
///
/// assert!((acos(1.0f64)).abs() < 1e-10);
/// assert!((acos(0.0f64) - PI / 2.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn acos<T: num_traits::Float>(v: T) -> T {
    v.acos()
}

/// Returns the arc tangent (inverse tangent) in radians.
///
/// # Examples
///
/// ```
/// use usd_gf::math::atan;
/// use std::f64::consts::PI;
///
/// assert!((atan(0.0f64)).abs() < 1e-10);
/// assert!((atan(1.0f64) - PI / 4.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn atan<T: num_traits::Float>(v: T) -> T {
    v.atan()
}

/// Returns the arc tangent of y/x in radians, using signs to determine quadrant.
///
/// # Examples
///
/// ```
/// use usd_gf::math::atan2;
/// use std::f64::consts::PI;
///
/// assert!((atan2(1.0f64, 1.0) - PI / 4.0).abs() < 1e-10);
/// assert!((atan2(1.0f64, -1.0) - 3.0 * PI / 4.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn atan2<T: num_traits::Float>(y: T, x: T) -> T {
    y.atan2(x)
}

/// Clamps a value to a range.
///
/// Returns `min` if `value < min`, `max` if `value > max`, otherwise `value`.
///
/// # Examples
///
/// ```
/// use usd_gf::math::clamp;
///
/// assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
/// assert_eq!(clamp(-5.0, 0.0, 10.0), 0.0);
/// assert_eq!(clamp(15.0, 0.0, 10.0), 10.0);
/// ```
#[inline]
#[must_use]
pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Modulo function with "correct" behavior for negative numbers.
///
/// If `a = n * b` for some integer `n`, returns zero.
/// Otherwise, for positive `a` returns `a % b`,
/// and for negative `a` returns `(a % b) + b`.
///
/// # Examples
///
/// ```
/// use usd_gf::math::modulo;
///
/// assert!((modulo(5.5, 2.0) - 1.5).abs() < 1e-10);
/// assert!((modulo(-1.5, 2.0) - 0.5).abs() < 1e-10);
/// assert!(modulo(4.0, 2.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn modulo(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 0.0;
    }

    let r = a % b;
    if r == 0.0 {
        0.0
    } else if a < 0.0 {
        r + b.abs()
    } else {
        r
    }
}

/// Modulo function with "correct" behavior for negative numbers (f32 version).
#[inline]
#[must_use]
pub fn modulo_f32(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return 0.0;
    }

    let r = a % b;
    if r == 0.0 {
        0.0
    } else if a < 0.0 {
        r + b.abs()
    } else {
        r
    }
}

/// Linear interpolation between two values.
///
/// Returns `(1 - alpha) * a + alpha * b`.
///
/// When `alpha = 0`, returns `a`.
/// When `alpha = 1`, returns `b`.
///
/// # Examples
///
/// ```
/// use usd_gf::math::lerp;
///
/// assert_eq!(lerp(0.0, 0.0, 10.0), 0.0);
/// assert_eq!(lerp(1.0, 0.0, 10.0), 10.0);
/// assert_eq!(lerp(0.5, 0.0, 10.0), 5.0);
/// assert_eq!(lerp(0.25, 0.0, 100.0), 25.0);
/// ```
#[inline]
#[must_use]
pub fn lerp<T>(alpha: f64, a: T, b: T) -> T
where
    T: std::ops::Mul<f64, Output = T> + std::ops::Add<Output = T>,
{
    a * (1.0 - alpha) + b * alpha
}

/// Returns the minimum of two values.
///
/// # Examples
///
/// ```
/// use usd_gf::math::min;
///
/// assert_eq!(min(3, 5), 3);
/// assert_eq!(min(5.0, 3.0), 3.0);
/// ```
#[inline]
#[must_use]
pub fn min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

/// Returns the minimum of three values.
#[inline]
#[must_use]
pub fn min3<T: PartialOrd + Copy>(a: T, b: T, c: T) -> T {
    min(min(a, b), c)
}

/// Returns the minimum of four values.
#[inline]
#[must_use]
pub fn min4<T: PartialOrd + Copy>(a: T, b: T, c: T, d: T) -> T {
    min(min3(a, b, c), d)
}

/// Returns the minimum of five values.
#[inline]
#[must_use]
pub fn min5<T: PartialOrd + Copy>(a: T, b: T, c: T, d: T, e: T) -> T {
    min(min4(a, b, c, d), e)
}

/// Returns the maximum of two values.
///
/// # Examples
///
/// ```
/// use usd_gf::math::max;
///
/// assert_eq!(max(3, 5), 5);
/// assert_eq!(max(5.0, 3.0), 5.0);
/// ```
#[inline]
#[must_use]
pub fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { b } else { a }
}

/// Returns the maximum of three values.
#[inline]
#[must_use]
pub fn max3<T: PartialOrd + Copy>(a: T, b: T, c: T) -> T {
    max(max(a, b), c)
}

/// Returns the maximum of four values.
#[inline]
#[must_use]
pub fn max4<T: PartialOrd + Copy>(a: T, b: T, c: T, d: T) -> T {
    max(max3(a, b, c), d)
}

/// Returns the maximum of five values.
#[inline]
#[must_use]
pub fn max5<T: PartialOrd + Copy>(a: T, b: T, c: T, d: T, e: T) -> T {
    max(max4(a, b, c, d), e)
}

/// Dot product for scalar types (just multiplication).
///
/// For vectors, see the vector types' dot methods.
///
/// # Examples
///
/// ```
/// use usd_gf::math::dot;
///
/// assert_eq!(dot(3.0, 4.0), 12.0);
/// ```
#[inline]
#[must_use]
pub fn dot<T: std::ops::Mul<Output = T>>(a: T, b: T) -> T {
    a * b
}

/// Component-wise multiplication for scalar types (just multiplication).
///
/// For vectors, see the vector types' comp_mult methods.
#[inline]
#[must_use]
pub fn comp_mult<T: std::ops::Mul<Output = T>>(a: T, b: T) -> T {
    a * b
}

/// Component-wise division for scalar types (just division).
///
/// For vectors, see the vector types' comp_div methods.
#[inline]
#[must_use]
pub fn comp_div<T: std::ops::Div<Output = T>>(a: T, b: T) -> T {
    a / b
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_is_close() {
        assert!(is_close(1.0, 1.0, 0.001));
        assert!(is_close(1.0, 1.0001, 0.001));
        assert!(!is_close(1.0, 1.1, 0.001));
        assert!(is_close(-5.0, -5.0, 0.001));
    }

    #[test]
    fn test_angle_conversions() {
        assert!((radians_to_degrees(PI) - 180.0).abs() < 1e-10);
        assert!((radians_to_degrees(PI / 2.0) - 90.0).abs() < 1e-10);
        assert!((degrees_to_radians(180.0) - PI).abs() < 1e-10);
        assert!((degrees_to_radians(90.0) - PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_smooth_step() {
        assert_eq!(smooth_step(0.0, 1.0, -0.5, 0.0, 0.0), 0.0);
        assert_eq!(smooth_step(0.0, 1.0, 1.5, 0.0, 0.0), 1.0);
        assert!((smooth_step(0.0, 1.0, 0.5, 0.0, 0.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_smooth_ramp() {
        assert_eq!(smooth_ramp(0.0, 1.0, -0.5, 0.0, 0.0), 0.0);
        assert_eq!(smooth_ramp(0.0, 1.0, 1.5, 0.0, 0.0), 1.0);
        assert!((smooth_ramp(0.0, 1.0, 0.5, 0.0, 0.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_sqr() {
        assert_eq!(sqr(3.0), 9.0);
        assert_eq!(sqr(-4.0), 16.0);
        assert_eq!(sqr(0.0), 0.0);
        assert_eq!(sqr(5i32), 25);
    }

    #[test]
    fn test_sgn() {
        assert_eq!(sgn(5i32), 1);
        assert_eq!(sgn(-3i32), -1);
        assert_eq!(sgn(0i32), 0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp(15.0, 0.0, 10.0), 10.0);
        assert_eq!(clamp(5, 0, 10), 5);
    }

    #[test]
    fn test_modulo() {
        assert!((modulo(5.5, 2.0) - 1.5).abs() < 1e-10);
        assert!((modulo(-1.5, 2.0) - 0.5).abs() < 1e-10);
        assert!(modulo(4.0, 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 0.0, 10.0), 0.0);
        assert_eq!(lerp(1.0, 0.0, 10.0), 10.0);
        assert!((lerp(0.5, 0.0, 10.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_min_max() {
        assert_eq!(min(3, 5), 3);
        assert_eq!(max(3, 5), 5);
        assert_eq!(min3(3, 1, 5), 1);
        assert_eq!(max3(3, 1, 5), 5);
        assert_eq!(min4(3, 1, 5, 2), 1);
        assert_eq!(max4(3, 1, 5, 2), 5);
        assert_eq!(min5(3, 1, 5, 2, 0), 0);
        assert_eq!(max5(3, 1, 5, 2, 10), 10);
    }

    #[test]
    fn test_trig() {
        assert!(sin(0.0f64).abs() < 1e-10);
        assert!((cos(0.0f64) - 1.0).abs() < 1e-10);
        assert!(tan(0.0f64).abs() < 1e-10);

        let (s, c) = sin_cos(PI / 4.0);
        assert!((s - c).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_trig() {
        assert!(asin(0.0f64).abs() < 1e-10);
        assert!((acos(0.0f64) - PI / 2.0).abs() < 1e-10);
        assert!(atan(0.0f64).abs() < 1e-10);
        assert!((atan2(1.0f64, 1.0) - PI / 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_basic_math() {
        assert!((sqrt(4.0f64) - 2.0).abs() < 1e-10);
        assert!((exp(0.0f64) - 1.0).abs() < 1e-10);
        assert!(log(1.0f64).abs() < 1e-10);
        assert_eq!(floor(3.7f64), 3.0);
        assert_eq!(ceil(3.2f64), 4.0);
        assert_eq!(abs(-3.5f64), 3.5);
        assert_eq!(round(3.4f64), 3.0);
        assert!((pow(2.0f64, 3.0) - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_dot_comp() {
        assert_eq!(dot(3.0, 4.0), 12.0);
        assert_eq!(comp_mult(3.0, 4.0), 12.0);
        assert_eq!(comp_div(12.0, 4.0), 3.0);
    }
}
