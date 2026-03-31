//! Dual number extensions for Vec3 / Color3 / Vec2 types.
//!
//! Port of `dual_vec.h` from OSL. OSL represents vectors-with-derivs as
//! `Dual2<Vec3>`, NOT as `Vec3<Dual2<float>>`. This module provides the
//! necessary operations to work with that representation.
//!
//! NOTE: `Color3` is a type alias for `Vec3`, so all `Dual2<Vec3>` operations
//! also apply to `Dual2<Color3>`.

use crate::Float;
use crate::dual::Dual2;
use crate::math::{Vec2, Vec3};

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

// ---------------------------------------------------------------------------
// Dual2<Vec3> constructors
// ---------------------------------------------------------------------------

/// Construct a `Dual2<Vec3>` from three `Dual2<Float>` components.
/// Also works for `Dual2<Color3>` since `Color3 = Vec3`.
#[inline]
pub fn make_vec3(x: Dual2<Float>, y: Dual2<Float>, z: Dual2<Float>) -> Dual2<Vec3> {
    Dual2 {
        val: Vec3::new(x.val, y.val, z.val),
        dx: Vec3::new(x.dx, y.dx, z.dx),
        dy: Vec3::new(x.dy, y.dy, z.dy),
    }
}

/// Alias for `make_vec3` — constructs `Dual2<Color3>` (same as `Dual2<Vec3>`).
#[inline]
pub fn make_color3(x: Dual2<Float>, y: Dual2<Float>, z: Dual2<Float>) -> Dual2<Vec3> {
    make_vec3(x, y, z)
}

/// Construct a `Dual2<Vec2>` from two `Dual2<Float>` components.
#[inline]
pub fn make_vec2(x: Dual2<Float>, y: Dual2<Float>) -> Dual2<Vec2> {
    Dual2 {
        val: Vec2::new(x.val, y.val),
        dx: Vec2::new(x.dx, y.dx),
        dy: Vec2::new(x.dy, y.dy),
    }
}

// ---------------------------------------------------------------------------
// Component extraction: Dual2<Vec3> -> Dual2<Float>
// ---------------------------------------------------------------------------

/// Extract the x component of a `Dual2<Vec3>` as a `Dual2<Float>`.
#[inline]
pub fn comp_x(v: &Dual2<Vec3>) -> Dual2<Float> {
    Dual2 {
        val: v.val.x,
        dx: v.dx.x,
        dy: v.dy.x,
    }
}

/// Extract the y component of a `Dual2<Vec3>` as a `Dual2<Float>`.
#[inline]
pub fn comp_y(v: &Dual2<Vec3>) -> Dual2<Float> {
    Dual2 {
        val: v.val.y,
        dx: v.dx.y,
        dy: v.dy.y,
    }
}

/// Extract the z component of a `Dual2<Vec3>` as a `Dual2<Float>`.
#[inline]
pub fn comp_z(v: &Dual2<Vec3>) -> Dual2<Float> {
    Dual2 {
        val: v.val.z,
        dx: v.dx.z,
        dy: v.dy.z,
    }
}

/// Extract x from Dual2<Vec2>.
#[inline]
pub fn comp_x_vec2(v: &Dual2<Vec2>) -> Dual2<Float> {
    Dual2 {
        val: v.val.x,
        dx: v.dx.x,
        dy: v.dy.x,
    }
}

/// Extract y from Dual2<Vec2>.
#[inline]
pub fn comp_y_vec2(v: &Dual2<Vec2>) -> Dual2<Float> {
    Dual2 {
        val: v.val.y,
        dx: v.dx.y,
        dy: v.dy.y,
    }
}

// ---------------------------------------------------------------------------
// Dual2<Vec3> constants and helpers
// ---------------------------------------------------------------------------

impl Dual2<Vec3> {
    /// Zero vector with zero derivatives.
    pub const ZERO: Self = Self {
        val: Vec3::ZERO,
        dx: Vec3::ZERO,
        dy: Vec3::ZERO,
    };

    /// Construct from value only; derivatives are zero.
    #[inline]
    pub fn from_val(val: Vec3) -> Self {
        Self {
            val,
            dx: Vec3::ZERO,
            dy: Vec3::ZERO,
        }
    }

    /// Clear derivatives, keep value.
    #[inline]
    pub fn clear_derivs(&mut self) {
        self.dx = Vec3::ZERO;
        self.dy = Vec3::ZERO;
    }

    /// Strip derivatives, returning just the value.
    #[inline]
    pub fn remove_derivs(self) -> Vec3 {
        self.val
    }
}

impl Dual2<Vec2> {
    /// Zero vec2 with zero derivatives.
    pub const ZERO: Self = Self {
        val: Vec2::ZERO,
        dx: Vec2::ZERO,
        dy: Vec2::ZERO,
    };

    /// Construct from value only; derivatives are zero.
    #[inline]
    pub fn from_val(val: Vec2) -> Self {
        Self {
            val,
            dx: Vec2::ZERO,
            dy: Vec2::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators for Dual2<Vec3>
// ---------------------------------------------------------------------------

impl Add for Dual2<Vec3> {
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

impl Add<Vec3> for Dual2<Vec3> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Vec3) -> Self {
        Self {
            val: self.val + rhs,
            dx: self.dx,
            dy: self.dy,
        }
    }
}

impl Sub for Dual2<Vec3> {
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

impl Sub<Vec3> for Dual2<Vec3> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Vec3) -> Self {
        Self {
            val: self.val - rhs,
            dx: self.dx,
            dy: self.dy,
        }
    }
}

impl Neg for Dual2<Vec3> {
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

impl Mul<Float> for Dual2<Vec3> {
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

impl Mul<Dual2<Float>> for Dual2<Vec3> {
    type Output = Self;
    /// Multiply Dual2<Vec3> by Dual2<Float> using chain rule.
    #[inline]
    fn mul(self, rhs: Dual2<Float>) -> Self {
        Self {
            val: self.val * rhs.val,
            dx: self.val * rhs.dx + self.dx * rhs.val,
            dy: self.val * rhs.dy + self.dy * rhs.val,
        }
    }
}

impl Div<Float> for Dual2<Vec3> {
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

impl Div<Dual2<Float>> for Dual2<Vec3> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Dual2<Float>) -> Self {
        let inv_b = 1.0 / rhs.val;
        let a_over_b = self.val * inv_b;
        Self {
            val: a_over_b,
            dx: (self.dx - a_over_b * rhs.dx) * inv_b,
            dy: (self.dy - a_over_b * rhs.dy) * inv_b,
        }
    }
}

/// Component-wise multiply of two Dual2<Vec3> (product rule per component).
impl Mul for Dual2<Vec3> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            val: self.val.comp_mul(rhs.val),
            dx: self.val.comp_mul(rhs.dx) + self.dx.comp_mul(rhs.val),
            dy: self.val.comp_mul(rhs.dy) + self.dy.comp_mul(rhs.val),
        }
    }
}

// ---------------------------------------------------------------------------
// Assign operators for Dual2<Vec3>
// ---------------------------------------------------------------------------

impl AddAssign for Dual2<Vec3> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.val += rhs.val;
        self.dx += rhs.dx;
        self.dy += rhs.dy;
    }
}

impl AddAssign<Vec3> for Dual2<Vec3> {
    #[inline]
    fn add_assign(&mut self, rhs: Vec3) {
        self.val += rhs;
    }
}

impl SubAssign for Dual2<Vec3> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.val -= rhs.val;
        self.dx -= rhs.dx;
        self.dy -= rhs.dy;
    }
}

impl SubAssign<Vec3> for Dual2<Vec3> {
    #[inline]
    fn sub_assign(&mut self, rhs: Vec3) {
        self.val -= rhs;
    }
}

impl MulAssign<Float> for Dual2<Vec3> {
    #[inline]
    fn mul_assign(&mut self, rhs: Float) {
        self.val *= rhs;
        self.dx *= rhs;
        self.dy *= rhs;
    }
}

impl DivAssign<Float> for Dual2<Vec3> {
    #[inline]
    fn div_assign(&mut self, rhs: Float) {
        let inv = 1.0 / rhs;
        self.val *= inv;
        self.dx *= inv;
        self.dy *= inv;
    }
}

// ---------------------------------------------------------------------------
// Reverse operators: scalar/Vec3 op Dual2<Vec3>
// ---------------------------------------------------------------------------

/// f32 * Dual2<Vec3> (scalar scales all components + derivs).
impl Mul<Dual2<Vec3>> for Float {
    type Output = Dual2<Vec3>;
    #[inline]
    fn mul(self, rhs: Dual2<Vec3>) -> Dual2<Vec3> {
        Dual2 {
            val: rhs.val * self,
            dx: rhs.dx * self,
            dy: rhs.dy * self,
        }
    }
}

/// Vec3 + Dual2<Vec3> (constant + dual, derivs pass through).
impl Add<Dual2<Vec3>> for Vec3 {
    type Output = Dual2<Vec3>;
    #[inline]
    fn add(self, rhs: Dual2<Vec3>) -> Dual2<Vec3> {
        Dual2 {
            val: self + rhs.val,
            dx: rhs.dx,
            dy: rhs.dy,
        }
    }
}

/// Vec3 - Dual2<Vec3> (constant - dual, derivs are negated).
impl Sub<Dual2<Vec3>> for Vec3 {
    type Output = Dual2<Vec3>;
    #[inline]
    fn sub(self, rhs: Dual2<Vec3>) -> Dual2<Vec3> {
        Dual2 {
            val: self - rhs.val,
            dx: -rhs.dx,
            dy: -rhs.dy,
        }
    }
}

/// Vec3 * Dual2<Vec3> component-wise (constant * dual per component).
impl Mul<Dual2<Vec3>> for Vec3 {
    type Output = Dual2<Vec3>;
    #[inline]
    fn mul(self, rhs: Dual2<Vec3>) -> Dual2<Vec3> {
        Dual2 {
            val: self.comp_mul(rhs.val),
            dx: self.comp_mul(rhs.dx),
            dy: self.comp_mul(rhs.dy),
        }
    }
}

// ---------------------------------------------------------------------------
// Vector operations with derivatives
// ---------------------------------------------------------------------------

/// Dot product of two `Dual2<Vec3>`, returning `Dual2<Float>`.
#[inline]
pub fn dot(a: &Dual2<Vec3>, b: &Dual2<Vec3>) -> Dual2<Float> {
    let ax = comp_x(a);
    let ay = comp_y(a);
    let az = comp_z(a);
    let bx = comp_x(b);
    let by = comp_y(b);
    let bz = comp_z(b);
    ax * bx + ay * by + az * bz
}

/// Dot product of `Dual2<Vec3>` with plain `Vec3`.
#[inline]
pub fn dot_dv(a: &Dual2<Vec3>, b: &Vec3) -> Dual2<Float> {
    let ax = comp_x(a);
    let ay = comp_y(a);
    let az = comp_z(a);
    ax * b.x + ay * b.y + az * b.z
}

/// Dot product of plain `Vec3` with `Dual2<Vec3>` (reverse order).
#[inline]
pub fn dot_vd(a: &Vec3, b: &Dual2<Vec3>) -> Dual2<Float> {
    dot_dv(b, a)
}

/// Cross product of two `Dual2<Vec3>`.
#[inline]
pub fn cross(a: &Dual2<Vec3>, b: &Dual2<Vec3>) -> Dual2<Vec3> {
    let ax = comp_x(a);
    let ay = comp_y(a);
    let az = comp_z(a);
    let bx = comp_x(b);
    let by = comp_y(b);
    let bz = comp_z(b);
    let nx = ay * bz - az * by;
    let ny = az * bx - ax * bz;
    let nz = ax * by - ay * bx;
    make_vec3(nx, ny, nz)
}

/// Length of a `Dual2<Vec3>`, returning `Dual2<Float>`.
#[inline]
pub fn length(a: &Dual2<Vec3>) -> Dual2<Float> {
    let ax = comp_x(a);
    let ay = comp_y(a);
    let az = comp_z(a);
    (ax * ax + ay * ay + az * az).sqrt()
}

/// Normalize a `Dual2<Vec3>`.
#[inline]
pub fn normalize(a: &Dual2<Vec3>) -> Dual2<Vec3> {
    let ax = comp_x(a);
    let ay = comp_y(a);
    let az = comp_z(a);
    let len = (ax * ax + ay * ay + az * az).sqrt();
    if len.val > 0.0 {
        let invlen = Dual2::<Float>::from_val(1.0) / len;
        let nax = ax * invlen;
        let nay = ay * invlen;
        let naz = az * invlen;
        make_vec3(nax, nay, naz)
    } else {
        Dual2::<Vec3>::from_val(Vec3::ZERO)
    }
}

/// Distance between two `Dual2<Vec3>` points.
#[inline]
pub fn distance(a: &Dual2<Vec3>, b: &Dual2<Vec3>) -> Dual2<Float> {
    length(&(*a - *b))
}

/// Distance from `Dual2<Vec3>` to plain `Vec3`.
#[inline]
pub fn distance_dv(a: &Dual2<Vec3>, b: &Vec3) -> Dual2<Float> {
    length(&(*a - *b))
}

/// Distance from plain `Vec3` to `Dual2<Vec3>`.
#[inline]
pub fn distance_vd(a: &Vec3, b: &Dual2<Vec3>) -> Dual2<Float> {
    length(&(*a - *b))
}

/// Multiply a `Dual2<Vec3>` by a 4x4 matrix (point transform, with perspective divide).
pub fn robust_mult_vec_matrix(m: &crate::math::Matrix44, src: &Dual2<Vec3>) -> Dual2<Vec3> {
    let mx = &m.m;
    let sx = comp_x(src);
    let sy = comp_y(src);
    let sz = comp_z(src);

    let a = sx * mx[0][0] + sy * mx[1][0] + sz * mx[2][0] + Dual2::<Float>::from_val(mx[3][0]);
    let b = sx * mx[0][1] + sy * mx[1][1] + sz * mx[2][1] + Dual2::<Float>::from_val(mx[3][1]);
    let c = sx * mx[0][2] + sy * mx[1][2] + sz * mx[2][2] + Dual2::<Float>::from_val(mx[3][2]);
    let w = sx * mx[0][3] + sy * mx[1][3] + sz * mx[2][3] + Dual2::<Float>::from_val(mx[3][3]);

    if w.val.abs() > Float::EPSILON {
        let inv_w = Dual2::<Float>::from_val(1.0) / w;
        make_vec3(a * inv_w, b * inv_w, c * inv_w)
    } else {
        Dual2::<Vec3>::ZERO
    }
}

/// Multiply a direction (no translation) by a 4x4 matrix.
pub fn mult_dir_matrix(m: &crate::math::Matrix44, src: &Dual2<Vec3>) -> Dual2<Vec3> {
    // For directions, apply to val, dx, dy independently
    Dual2 {
        val: m.transform_vector(src.val),
        dx: m.transform_vector(src.dx),
        dy: m.transform_vector(src.dy),
    }
}

// ---------------------------------------------------------------------------
// Dual2<Vec2> dot product
// ---------------------------------------------------------------------------

/// Dot product of two `Dual2<Vec2>`.
#[inline]
pub fn dot_vec2(a: &Dual2<Vec2>, b: &Dual2<Vec2>) -> Dual2<Float> {
    let ax = comp_x_vec2(a);
    let ay = comp_y_vec2(a);
    let bx = comp_x_vec2(b);
    let by = comp_y_vec2(b);
    ax * bx + ay * by
}

/// Dot product of `Dual2<Vec2>` with plain `Vec2`.
#[inline]
pub fn dot_vec2_dv(a: &Dual2<Vec2>, b: &Vec2) -> Dual2<Float> {
    let ax = comp_x_vec2(a);
    let ay = comp_y_vec2(a);
    ax * b.x + ay * b.y
}

/// Dot product of plain `Vec2` with `Dual2<Vec2>` (reverse order).
#[inline]
pub fn dot_vec2_vd(a: &Vec2, b: &Dual2<Vec2>) -> Dual2<Float> {
    dot_vec2_dv(b, a)
}

/// Multiply a `Dual2<Vec3>` by a 3x3 matrix (row-vector * matrix).
pub fn vec3_mul_matrix33(src: &Dual2<Vec3>, m: &crate::math::Matrix33) -> Dual2<Vec3> {
    let s0 = comp_x(src);
    let s1 = comp_y(src);
    let s2 = comp_z(src);
    let a = s0 * m.m[0][0] + s1 * m.m[1][0] + s2 * m.m[2][0];
    let b = s0 * m.m[0][1] + s1 * m.m[1][1] + s2 * m.m[2][1];
    let c = s0 * m.m[0][2] + s1 * m.m[1][2] + s2 * m.m[2][2];
    make_vec3(a, b, c)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: Float = 1e-5;

    fn approx(a: Float, b: Float) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn test_make_vec3_comp() {
        let dx = Dual2::new(1.0_f32, 0.1, 0.2);
        let dy = Dual2::new(2.0_f32, 0.3, 0.4);
        let dz = Dual2::new(3.0_f32, 0.5, 0.6);
        let dv = make_vec3(dx, dy, dz);
        assert!(approx(dv.val.x, 1.0));
        assert!(approx(dv.val.y, 2.0));
        assert!(approx(dv.val.z, 3.0));
        assert!(approx(dv.dx.x, 0.1));
        assert!(approx(dv.dy.z, 0.6));
    }

    #[test]
    fn test_comp_roundtrip() {
        let dv = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        let cx = comp_x(&dv);
        let cy = comp_y(&dv);
        let cz = comp_z(&dv);
        let reconstructed = make_vec3(cx, cy, cz);
        assert!(approx(reconstructed.val.x, dv.val.x));
        assert!(approx(reconstructed.val.y, dv.val.y));
        assert!(approx(reconstructed.val.z, dv.val.z));
        assert!(approx(reconstructed.dx.z, dv.dx.z));
    }

    #[test]
    fn test_dot_dual_vec3() {
        let a = Dual2::<Vec3>::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::ZERO,
        );
        let b = Dual2::<Vec3>::new(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO);
        let d = dot(&a, &b);
        assert!(approx(d.val, 1.0));
        assert!(approx(d.dx, 0.1));
    }

    #[test]
    fn test_cross_dual_vec3() {
        let a = Dual2::<Vec3>::from_val(Vec3::new(1.0, 0.0, 0.0));
        let b = Dual2::<Vec3>::from_val(Vec3::new(0.0, 1.0, 0.0));
        let c = cross(&a, &b);
        assert!(approx(c.val.x, 0.0));
        assert!(approx(c.val.y, 0.0));
        assert!(approx(c.val.z, 1.0));
    }

    #[test]
    fn test_length_dual_vec3() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(3.0, 4.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        let l = length(&v);
        assert!(approx(l.val, 5.0));
        // dl/dx = (3*1 + 4*0 + 0*0)/5 = 0.6
        assert!(approx(l.dx, 0.6));
    }

    #[test]
    fn test_normalize_dual_vec3() {
        let v = Dual2::<Vec3>::from_val(Vec3::new(3.0, 0.0, 0.0));
        let n = normalize(&v);
        assert!(approx(n.val.x, 1.0));
        assert!(approx(n.val.y, 0.0));
        assert!(approx(n.val.z, 0.0));
    }

    #[test]
    fn test_distance_dual_vec3() {
        let a = Dual2::<Vec3>::from_val(Vec3::new(0.0, 0.0, 0.0));
        let b = Dual2::<Vec3>::from_val(Vec3::new(3.0, 4.0, 0.0));
        let d = distance(&a, &b);
        assert!(approx(d.val, 5.0));
    }

    #[test]
    fn test_add_sub_dual_vec3() {
        let a = Dual2::<Vec3>::from_val(Vec3::new(1.0, 2.0, 3.0));
        let b = Dual2::<Vec3>::from_val(Vec3::new(4.0, 5.0, 6.0));
        let sum = a + b;
        assert!(approx(sum.val.x, 5.0));
        assert!(approx(sum.val.y, 7.0));
        let diff = a - b;
        assert!(approx(diff.val.x, -3.0));
    }

    #[test]
    fn test_mul_dual_vec3_scalar() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::ZERO,
        );
        let scaled = v * 2.0;
        assert!(approx(scaled.val.x, 2.0));
        assert!(approx(scaled.dx.x, 0.2));
    }

    #[test]
    fn test_robust_mult_vec_matrix() {
        let m = crate::math::Matrix44::IDENTITY;
        let v = Dual2::<Vec3>::from_val(Vec3::new(1.0, 2.0, 3.0));
        let result = robust_mult_vec_matrix(&m, &v);
        assert!(approx(result.val.x, 1.0));
        assert!(approx(result.val.y, 2.0));
        assert!(approx(result.val.z, 3.0));
    }

    #[test]
    fn test_mult_dir_matrix() {
        let m = crate::math::Matrix44::IDENTITY;
        let v = Dual2::<Vec3>::from_val(Vec3::new(1.0, 0.0, 0.0));
        let result = mult_dir_matrix(&m, &v);
        assert!(approx(result.val.x, 1.0));
        assert!(approx(result.val.y, 0.0));
    }

    // --- Assign operators ---

    #[test]
    fn test_add_assign_dual_vec3() {
        let mut a = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        let b = Dual2::<Vec3>::new(
            Vec3::new(10.0, 20.0, 30.0),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
        );
        a += b;
        assert!(approx(a.val.x, 11.0));
        assert!(approx(a.dx.y, 2.2));
        assert!(approx(a.dy.z, 6.6));
    }

    #[test]
    fn test_add_assign_vec3() {
        let mut a = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::ZERO,
        );
        a += Vec3::new(10.0, 20.0, 30.0);
        assert!(approx(a.val.x, 11.0));
        // Derivs unchanged
        assert!(approx(a.dx.x, 0.1));
    }

    #[test]
    fn test_sub_assign_dual_vec3() {
        let mut a = Dual2::<Vec3>::new(
            Vec3::new(10.0, 20.0, 30.0),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::ZERO,
        );
        let b = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::ZERO,
        );
        a -= b;
        assert!(approx(a.val.x, 9.0));
        assert!(approx(a.dx.x, 0.5));
    }

    #[test]
    fn test_sub_assign_vec3() {
        let mut a = Dual2::<Vec3>::new(
            Vec3::new(10.0, 20.0, 30.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::ZERO,
        );
        a -= Vec3::new(1.0, 2.0, 3.0);
        assert!(approx(a.val.x, 9.0));
        assert!(approx(a.dx.x, 0.1));
    }

    #[test]
    fn test_mul_assign_f32() {
        let mut v = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        v *= 3.0;
        assert!(approx(v.val.x, 3.0));
        assert!(approx(v.dx.y, 0.6));
        assert!(approx(v.dy.z, 1.8));
    }

    #[test]
    fn test_div_assign_f32() {
        let mut v = Dual2::<Vec3>::new(
            Vec3::new(6.0, 9.0, 12.0),
            Vec3::new(0.3, 0.6, 0.9),
            Vec3::ZERO,
        );
        v /= 3.0;
        assert!(approx(v.val.x, 2.0));
        assert!(approx(v.val.y, 3.0));
        assert!(approx(v.dx.x, 0.1));
    }

    // --- Reverse operators ---

    #[test]
    fn test_f32_mul_dual_vec3() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        let r = 2.0_f32 * v;
        assert!(approx(r.val.x, 2.0));
        assert!(approx(r.val.y, 4.0));
        assert!(approx(r.dx.z, 0.6));
        assert!(approx(r.dy.x, 0.8));
    }

    #[test]
    fn test_vec3_add_dual_vec3() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        let c = Vec3::new(10.0, 20.0, 30.0);
        let r = c + v;
        assert!(approx(r.val.x, 11.0));
        assert!(approx(r.val.y, 22.0));
        // Derivs come from v only
        assert!(approx(r.dx.x, 0.1));
        assert!(approx(r.dy.z, 0.6));
    }

    #[test]
    fn test_vec3_sub_dual_vec3() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(0.4, 0.5, 0.6),
        );
        let c = Vec3::new(10.0, 20.0, 30.0);
        let r = c - v;
        assert!(approx(r.val.x, 9.0));
        assert!(approx(r.val.y, 18.0));
        // Derivs are negated
        assert!(approx(r.dx.x, -0.1));
        assert!(approx(r.dy.z, -0.6));
    }

    #[test]
    fn test_vec3_mul_dual_vec3_componentwise() {
        let v = Dual2::<Vec3>::new(
            Vec3::new(2.0, 3.0, 4.0),
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::ZERO,
        );
        let c = Vec3::new(10.0, 20.0, 30.0);
        let r = c * v;
        // val = (20, 60, 120)
        assert!(approx(r.val.x, 20.0));
        assert!(approx(r.val.y, 60.0));
        assert!(approx(r.val.z, 120.0));
        // dx = c * v.dx = (1, 4, 9)
        assert!(approx(r.dx.x, 1.0));
        assert!(approx(r.dx.y, 4.0));
        assert!(approx(r.dx.z, 9.0));
    }

    // --- Component-wise Dual * Dual ---

    #[test]
    fn test_dual_vec3_mul_dual_vec3() {
        let a = Dual2::<Vec3>::new(
            Vec3::new(2.0, 3.0, 4.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        let b = Dual2::<Vec3>::new(
            Vec3::new(5.0, 6.0, 7.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
        );
        let r = a * b;
        // val = (10, 18, 28)
        assert!(approx(r.val.x, 10.0));
        assert!(approx(r.val.y, 18.0));
        assert!(approx(r.val.z, 28.0));
        // dx.x = a.val.x * b.dx.x + a.dx.x * b.val.x = 2*0 + 1*5 = 5
        assert!(approx(r.dx.x, 5.0));
        // dx.y = a.val.y * b.dx.y + a.dx.y * b.val.y = 3*1 + 0*6 = 3
        assert!(approx(r.dx.y, 3.0));
        // dx.z = a.val.z * b.dx.z + a.dx.z * b.val.z = 4*0 + 0*7 = 0
        assert!(approx(r.dx.z, 0.0));
    }

    // --- Reverse dot/distance ---

    #[test]
    fn test_dot_vd() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Dual2::<Vec3>::new(
            Vec3::new(3.0, 4.0, 0.0),
            Vec3::new(0.1, 0.2, 0.0),
            Vec3::ZERO,
        );
        let d = dot_vd(&a, &b);
        // val = 1*3 + 0*4 + 0*0 = 3
        assert!(approx(d.val, 3.0));
        // dx = 1*0.1 + 0*0.2 = 0.1
        assert!(approx(d.dx, 0.1));
    }

    #[test]
    fn test_dot_vec2_dv() {
        let a = Dual2::<Vec2>::new(Vec2::new(1.0, 2.0), Vec2::new(0.1, 0.2), Vec2::ZERO);
        let b = Vec2::new(3.0, 4.0);
        let d = dot_vec2_dv(&a, &b);
        // val = 1*3 + 2*4 = 11
        assert!(approx(d.val, 11.0));
        // dx = 0.1*3 + 0.2*4 = 1.1
        assert!(approx(d.dx, 1.1));
    }

    #[test]
    fn test_dot_vec2_vd() {
        let a = Vec2::new(3.0, 4.0);
        let b = Dual2::<Vec2>::new(Vec2::new(1.0, 2.0), Vec2::new(0.1, 0.2), Vec2::ZERO);
        let d = dot_vec2_vd(&a, &b);
        assert!(approx(d.val, 11.0));
        assert!(approx(d.dx, 1.1));
    }

    #[test]
    fn test_distance_dv() {
        let a = Dual2::<Vec3>::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        let b = Vec3::new(3.0, 4.0, 0.0);
        let d = distance_dv(&a, &b);
        assert!(approx(d.val, 5.0));
        // d(dist)/dx = d(length(a-b))/dx, a.dx=(1,0,0), (a-b)=(-3,-4,0)
        // dl/dx = ((-3)*1 + (-4)*0)/5 = -0.6
        assert!(approx(d.dx, -0.6));
    }

    #[test]
    fn test_distance_vd() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Dual2::<Vec3>::new(
            Vec3::new(3.0, 4.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        let d = distance_vd(&a, &b);
        assert!(approx(d.val, 5.0));
        // (a-b) = (-3,-4,0), b.dx=(1,0,0) -> d(a-b)/dx = -b.dx = (-1,0,0)
        // dl/dx = ((-3)*(-1))/5 = 0.6
        assert!(approx(d.dx, 0.6));
    }
}
