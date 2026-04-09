//! Dual numbers for automatic differentiation.
//!
//! `Dual2<T>` represents a value with two partial derivatives (dx, dy).
//! This is the standard type used throughout OSL for propagating
//! derivatives through shading computations.
//!
//! All mathematical functions implement the chain rule to propagate
//! derivatives automatically, matching the C++ OSL `dual.h` behavior.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::Float;

/// Dual number with 2 partial derivatives (dx, dy).
/// This is the standard type used throughout OSL.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Dual2<T> {
    pub val: T,
    pub dx: T,
    pub dy: T,
}

/// Convenience alias: `Dual` = `Dual2` (the most common variant in OSL).
pub type Dual<T> = Dual2<T>;

impl<T: Copy> Dual2<T> {
    /// Create from value and derivatives.
    #[inline]
    pub const fn new(val: T, dx: T, dy: T) -> Self {
        Self { val, dx, dy }
    }

    /// Get the value.
    #[inline]
    pub const fn val(&self) -> T {
        self.val
    }

    /// Get the x-derivative.
    #[inline]
    pub const fn dx(&self) -> T {
        self.dx
    }

    /// Get the y-derivative.
    #[inline]
    pub const fn dy(&self) -> T {
        self.dy
    }
}

impl Dual2<Float> {
    /// Zero constant.
    pub const ZERO: Self = Self {
        val: 0.0,
        dx: 0.0,
        dy: 0.0,
    };

    /// Construct from value only, derivatives are zero.
    #[inline]
    pub const fn from_val(val: Float) -> Self {
        Self {
            val,
            dx: 0.0,
            dy: 0.0,
        }
    }

    /// Clear derivatives, keep value.
    #[inline]
    pub fn clear_derivs(&mut self) {
        self.dx = 0.0;
        self.dy = 0.0;
    }

    /// Strip derivatives, returning just the value.
    #[inline]
    pub fn remove_derivs(self) -> Float {
        self.val
    }

    // -- Helpers for building dual results of functions ---------------------

    /// Build dual result for f(u) given f(u.val) and f'(u.val).
    #[inline]
    fn dualfunc1(u: Self, f_val: Float, df_val: Float) -> Self {
        Self {
            val: f_val,
            dx: df_val * u.dx,
            dy: df_val * u.dy,
        }
    }

    /// Build dual result for f(u, v) given f(u.val, v.val),
    /// df/du(u.val, v.val), and df/dv(u.val, v.val).
    #[inline]
    fn dualfunc2(u: Self, v: Self, f_val: Float, dfdu: Float, dfdv: Float) -> Self {
        Self {
            val: f_val,
            dx: dfdu * u.dx + dfdv * v.dx,
            dy: dfdu * u.dy + dfdv * v.dy,
        }
    }

    // -- Trigonometric functions --------------------------------------------

    pub fn sin(self) -> Self {
        let (sina, cosa) = (self.val.sin(), self.val.cos());
        Self::dualfunc1(self, sina, cosa)
    }

    pub fn cos(self) -> Self {
        let (sina, cosa) = (self.val.sin(), self.val.cos());
        Self::dualfunc1(self, cosa, -sina)
    }

    pub fn sincos(self) -> (Self, Self) {
        let (sina, cosa) = (self.val.sin(), self.val.cos());
        (
            Self::dualfunc1(self, sina, cosa),
            Self::dualfunc1(self, cosa, -sina),
        )
    }

    pub fn tan(self) -> Self {
        let t = self.val.tan();
        let c = self.val.cos();
        let sec2 = 1.0 / (c * c);
        Self::dualfunc1(self, t, sec2)
    }

    pub fn asin(self) -> Self {
        if self.val >= 1.0 {
            return Self::from_val(std::f32::consts::FRAC_PI_2);
        }
        if self.val <= -1.0 {
            return Self::from_val(-std::f32::consts::FRAC_PI_2);
        }
        let f = self.val.asin();
        let df = 1.0 / (1.0 - self.val * self.val).sqrt();
        Self::dualfunc1(self, f, df)
    }

    pub fn acos(self) -> Self {
        if self.val >= 1.0 {
            return Self::from_val(0.0);
        }
        if self.val <= -1.0 {
            return Self::from_val(std::f32::consts::PI);
        }
        let f = self.val.acos();
        let df = -1.0 / (1.0 - self.val * self.val).sqrt();
        Self::dualfunc1(self, f, df)
    }

    pub fn atan(self) -> Self {
        let f = self.val.atan();
        let df = 1.0 / (1.0 + self.val * self.val);
        Self::dualfunc1(self, f, df)
    }

    pub fn atan2(y: Self, x: Self) -> Self {
        let f = y.val.atan2(x.val);
        let denom = if x.val == 0.0 && y.val == 0.0 {
            0.0
        } else {
            1.0 / (x.val * x.val + y.val * y.val)
        };
        // Match C++ dual.h:1022 sign convention for parity
        Self::dualfunc2(y, x, f, -x.val * denom, y.val * denom)
    }

    // -- Hyperbolic functions -----------------------------------------------

    pub fn sinh(self) -> Self {
        let f = self.val.sinh();
        let df = self.val.cosh();
        Self::dualfunc1(self, f, df)
    }

    pub fn cosh(self) -> Self {
        let f = self.val.cosh();
        let df = self.val.sinh();
        Self::dualfunc1(self, f, df)
    }

    pub fn tanh(self) -> Self {
        let t = self.val.tanh();
        let c = self.val.cosh();
        let sech2 = 1.0 / (c * c);
        Self::dualfunc1(self, t, sech2)
    }

    // -- Exponential / logarithmic -----------------------------------------

    pub fn exp(self) -> Self {
        let f = self.val.exp();
        Self::dualfunc1(self, f, f)
    }

    pub fn exp2(self) -> Self {
        let f = self.val.exp2();
        Self::dualfunc1(self, f, f * std::f32::consts::LN_2)
    }

    pub fn expm1(self) -> Self {
        let f = self.val.exp() - 1.0;
        let df = self.val.exp();
        Self::dualfunc1(self, f, df)
    }

    pub fn ln(self) -> Self {
        let f = if self.val > 0.0 {
            self.val.ln()
        } else {
            -f32::MAX
        };
        let df = if self.val < f32::MIN_POSITIVE {
            0.0
        } else {
            1.0 / self.val
        };
        Self::dualfunc1(self, f, df)
    }

    pub fn log2(self) -> Self {
        let f = if self.val > 0.0 {
            self.val.log2()
        } else {
            -f32::MAX
        };
        let df = if self.val < f32::MIN_POSITIVE {
            0.0
        } else {
            1.0 / (self.val * std::f32::consts::LN_2)
        };
        Self::dualfunc1(self, f, df)
    }

    pub fn log10(self) -> Self {
        let f = if self.val > 0.0 {
            self.val.log10()
        } else {
            -f32::MAX
        };
        let df = if self.val < f32::MIN_POSITIVE {
            0.0
        } else {
            1.0 / (self.val * std::f32::consts::LN_10)
        };
        Self::dualfunc1(self, f, df)
    }

    pub fn powf(self, exp: Self) -> Self {
        let powuvm1 = safe_powf(self.val, exp.val - 1.0);
        let powuv = powuvm1 * self.val;
        let logu = if self.val > 0.0 { self.val.ln() } else { 0.0 };
        Self::dualfunc2(self, exp, powuv, exp.val * powuvm1, logu * powuv)
    }

    pub fn sqrt(self) -> Self {
        if self.val > 0.0 {
            let f = self.val.sqrt();
            let df = 0.5 / f;
            Self::dualfunc1(self, f, df)
        } else {
            Self::from_val(0.0)
        }
    }

    pub fn inversesqrt(self) -> Self {
        if self.val > 0.0 {
            let f = 1.0 / self.val.sqrt();
            let df = -0.5 * f / self.val;
            Self::dualfunc1(self, f, df)
        } else {
            Self::from_val(0.0)
        }
    }

    pub fn cbrt(self) -> Self {
        if self.val != 0.0 {
            let f = self.val.cbrt();
            let df = 1.0 / (3.0 * f * f);
            Self::dualfunc1(self, f, df)
        } else {
            Self::from_val(0.0)
        }
    }

    // -- Special functions --------------------------------------------------

    pub fn erf(self) -> Self {
        let f = libm::erff(self.val);
        // 2/sqrt(pi) from f64 constant for full precision
        let two_over_sqrt_pi = (2.0_f64 / std::f64::consts::PI.sqrt()) as Float;
        let df = (-self.val * self.val).exp() * two_over_sqrt_pi;
        Self::dualfunc1(self, f, df)
    }

    pub fn erfc(self) -> Self {
        let f = libm::erfcf(self.val);
        // derivative of erfc is -d(erf)/dx
        let two_over_sqrt_pi = -(2.0_f64 / std::f64::consts::PI.sqrt()) as Float;
        let df = (-self.val * self.val).exp() * two_over_sqrt_pi;
        Self::dualfunc1(self, f, df)
    }

    // -- Interpolation / clamping ------------------------------------------

    pub fn abs(self) -> Self {
        if self.val >= 0.0 { self } else { -self }
    }

    pub fn mix(x: Self, y: Self, a: Self) -> Self {
        let one_minus_a = 1.0 - a.val;
        let mixval = x.val * one_minus_a + y.val * a.val;
        Self {
            val: mixval,
            dx: x.dx * one_minus_a + y.dx * a.val + (y.val - x.val) * a.dx,
            dy: x.dy * one_minus_a + y.dy * a.val + (y.val - x.val) * a.dy,
        }
    }

    pub fn smoothstep(e0: Self, e1: Self, x: Self) -> Self {
        if x.val < e0.val {
            return Self::from_val(0.0);
        }
        if x.val >= e1.val {
            return Self::from_val(1.0);
        }
        let t = (x - e0) / (e1 - e0);
        (Self::from_val(3.0) - Self::from_val(2.0) * t) * t * t
    }

    /// Clamp to [lo, hi].
    pub fn clamp(self, lo: Float, hi: Float) -> Self {
        if self.val < lo {
            Self::from_val(lo)
        } else if self.val > hi {
            Self::from_val(hi)
        } else {
            self
        }
    }

    /// Min of two duals (by value, derivatives follow).
    pub fn min(self, other: Self) -> Self {
        if self.val <= other.val { self } else { other }
    }

    /// Max of two duals (by value, derivatives follow).
    pub fn max(self, other: Self) -> Self {
        if self.val >= other.val { self } else { other }
    }

    /// fmod that is safe against division by zero.
    pub fn safe_fmod(self, b: Self) -> Self {
        if b.val != 0.0 {
            let n = (self.val / b.val) as i32;
            Self {
                val: self.val - (n as Float) * b.val,
                dx: self.dx,
                dy: self.dy,
            }
        } else {
            Self::from_val(0.0)
        }
    }

    /// Floor — returns scalar, drops derivatives.
    pub fn floor(self) -> Float {
        self.val.floor()
    }

    /// Ceil — returns scalar, drops derivatives.
    pub fn ceil(self) -> Float {
        self.val.ceil()
    }

    /// Round — returns scalar, drops derivatives.
    pub fn round(self) -> Float {
        self.val.round()
    }

    /// Truncate — returns scalar, drops derivatives.
    pub fn trunc(self) -> Float {
        self.val.trunc()
    }
}

// Safe powf: handle negative base (matches C++ OSL behavior).
// For integer exponents: odd → negate, even → positive. Matches C++ `int(b) & 1` check.
fn safe_powf(base: Float, exp: Float) -> Float {
    if base < 0.0 {
        // Non-integer exponent with negative base is undefined; OSL returns 0.
        if exp != exp.floor() {
            return 0.0;
        }
        // Check odd/even: odd exponent keeps negative sign, even gives positive.
        let result = (-base).powf(exp);
        if (exp as i64) & 1 != 0 {
            -result
        } else {
            result
        }
    } else {
        base.powf(exp)
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators for Dual2<Float>
// ---------------------------------------------------------------------------

impl Add for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            val: self.val + rhs.val,
            dx: self.dx + rhs.dx,
            dy: self.dy + rhs.dy,
        }
    }
}

impl Add<Float> for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Float) -> Self {
        Self {
            val: self.val + rhs,
            dx: self.dx,
            dy: self.dy,
        }
    }
}

impl Add<Dual2<Float>> for Float {
    type Output = Dual2<Float>;
    #[inline]
    fn add(self, rhs: Dual2<Float>) -> Dual2<Float> {
        Dual2 {
            val: self + rhs.val,
            dx: rhs.dx,
            dy: rhs.dy,
        }
    }
}

impl Sub for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            val: self.val - rhs.val,
            dx: self.dx - rhs.dx,
            dy: self.dy - rhs.dy,
        }
    }
}

impl Sub<Float> for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Float) -> Self {
        Self {
            val: self.val - rhs,
            dx: self.dx,
            dy: self.dy,
        }
    }
}

impl Sub<Dual2<Float>> for Float {
    type Output = Dual2<Float>;
    #[inline]
    fn sub(self, rhs: Dual2<Float>) -> Dual2<Float> {
        Dual2 {
            val: self - rhs.val,
            dx: -rhs.dx,
            dy: -rhs.dy,
        }
    }
}

impl Neg for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            val: -self.val,
            dx: -self.dx,
            dy: -self.dy,
        }
    }
}

impl Mul for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            val: self.val * rhs.val,
            dx: self.val * rhs.dx + self.dx * rhs.val,
            dy: self.val * rhs.dy + self.dy * rhs.val,
        }
    }
}

impl Mul<Float> for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Float) -> Self {
        Self {
            val: self.val * rhs,
            dx: self.dx * rhs,
            dy: self.dy * rhs,
        }
    }
}

impl Mul<Dual2<Float>> for Float {
    type Output = Dual2<Float>;
    #[inline]
    fn mul(self, rhs: Dual2<Float>) -> Dual2<Float> {
        rhs * self
    }
}

impl Div for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        let inv_b = 1.0 / rhs.val;
        let a_over_b = self.val * inv_b;
        Self {
            val: a_over_b,
            dx: inv_b * (self.dx - a_over_b * rhs.dx),
            dy: inv_b * (self.dy - a_over_b * rhs.dy),
        }
    }
}

impl Div<Float> for Dual2<Float> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Float) -> Self {
        let inv = 1.0 / rhs;
        Self {
            val: self.val * inv,
            dx: self.dx * inv,
            dy: self.dy * inv,
        }
    }
}

impl AddAssign for Dual2<Float> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.val += rhs.val;
        self.dx += rhs.dx;
        self.dy += rhs.dy;
    }
}

impl SubAssign for Dual2<Float> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.val -= rhs.val;
        self.dx -= rhs.dx;
        self.dy -= rhs.dy;
    }
}

impl MulAssign<Float> for Dual2<Float> {
    #[inline]
    fn mul_assign(&mut self, rhs: Float) {
        self.val *= rhs;
        self.dx *= rhs;
        self.dy *= rhs;
    }
}

impl DivAssign<Float> for Dual2<Float> {
    #[inline]
    fn div_assign(&mut self, rhs: Float) {
        let inv = 1.0 / rhs;
        self.val *= inv;
        self.dx *= inv;
        self.dy *= inv;
    }
}

// -- Comparison (by value only, matching C++ behavior) ----------------------

impl PartialOrd for Dual2<Float> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

// -- Display ----------------------------------------------------------------

impl fmt::Debug for Dual2<Float> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dual2({}, [{}, {}])", self.val, self.dx, self.dy)
    }
}

impl fmt::Display for Dual2<Float> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{},{}]", self.val, self.dx, self.dy)
    }
}

impl From<Float> for Dual2<Float> {
    #[inline]
    fn from(val: Float) -> Self {
        Self::from_val(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: Float = 1e-5;

    fn approx(a: Float, b: Float) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn test_basic_arithmetic() {
        let a = Dual2::new(2.0, 1.0, 0.0);
        let b = Dual2::new(3.0, 0.0, 1.0);

        let sum = a + b;
        assert!(approx(sum.val, 5.0));
        assert!(approx(sum.dx, 1.0));
        assert!(approx(sum.dy, 1.0));

        let prod = a * b;
        assert!(approx(prod.val, 6.0));
        assert!(approx(prod.dx, 3.0)); // d/dx(x*y) = y
        assert!(approx(prod.dy, 2.0)); // d/dy(x*y) = x
    }

    #[test]
    fn test_division() {
        let a = Dual2::new(6.0, 1.0, 0.0);
        let b = Dual2::new(3.0, 0.0, 1.0);
        let d = a / b;
        assert!(approx(d.val, 2.0));
        assert!(approx(d.dx, 1.0 / 3.0));
        assert!(approx(d.dy, -6.0 / 9.0));
    }

    #[test]
    fn test_sin_cos() {
        let x = Dual2::new(0.0, 1.0, 0.0);
        let s = x.sin();
        assert!(approx(s.val, 0.0));
        assert!(approx(s.dx, 1.0)); // cos(0) = 1

        let c = x.cos();
        assert!(approx(c.val, 1.0));
        assert!(approx(c.dx, 0.0)); // -sin(0) = 0
    }

    #[test]
    fn test_exp_log() {
        let x = Dual2::new(1.0, 1.0, 0.0);
        let e = x.exp();
        assert!(approx(e.val, std::f32::consts::E));
        assert!(approx(e.dx, std::f32::consts::E));

        let l = x.ln();
        assert!(approx(l.val, 0.0));
        assert!(approx(l.dx, 1.0));
    }

    #[test]
    fn test_sqrt() {
        let x = Dual2::new(4.0, 1.0, 0.0);
        let s = x.sqrt();
        assert!(approx(s.val, 2.0));
        assert!(approx(s.dx, 0.25)); // 1/(2*sqrt(4)) = 0.25
    }

    #[test]
    fn test_smoothstep() {
        let e0 = Dual2::<Float>::from_val(0.0);
        let e1 = Dual2::<Float>::from_val(1.0);

        let x_mid = Dual2::new(0.5, 1.0, 0.0);
        let ss = Dual2::smoothstep(e0, e1, x_mid);
        assert!(approx(ss.val, 0.5));
        assert!(approx(ss.dx, 1.5));
    }

    #[test]
    fn test_mix() {
        let a = Dual2::<Float>::from_val(0.0);
        let b = Dual2::<Float>::from_val(10.0);
        let t = Dual2::new(0.3, 1.0, 0.0);
        let m = Dual2::mix(a, b, t);
        assert!(approx(m.val, 3.0));
        assert!(approx(m.dx, 10.0));
    }

    #[test]
    fn test_comparison_by_value() {
        let a = Dual2::new(1.0, 100.0, 200.0);
        let b = Dual2::new(2.0, 0.0, 0.0);
        assert!(a < b);
        assert_ne!(b.partial_cmp(&a), Some(std::cmp::Ordering::Less));
    }
}
