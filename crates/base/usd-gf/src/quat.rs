//! Quaternion types for rotations.
//!
//! This module provides Quat types for representing rotations as quaternions.
//! A quaternion consists of a real (scalar) part and an imaginary (vector) part.
//!
//! # Quaternion Representation
//!
//! A quaternion q is represented as:
//! - q = w + xi + yj + zk
//! - where w is the real part and (x, y, z) is the imaginary part
//!
//! # Examples
//!
//! ```
//! use usd_gf::quat::{Quatd, Quatf};
//! use usd_gf::vec3::Vec3d;
//! use std::f64::consts::PI;
//!
//! // Create identity quaternion
//! let q = Quatd::identity();
//!
//! // Create rotation around Z axis
//! let rot = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
//!
//! // Transform a point
//! let p = Vec3d::new(1.0, 0.0, 0.0);
//! let rotated = rot.transform(&p);
//! ```

use crate::limits::MIN_VECTOR_LENGTH;
use crate::traits::Scalar;
use crate::vec3::Vec3;
use num_traits::Float;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A quaternion with scalar type `T`.
///
/// Stored as imaginary (Vec3) followed by real (scalar) for memory alignment.
///
/// # Examples
///
/// ```
/// use usd_gf::quat::Quatd;
///
/// let q = Quatd::identity();
/// assert_eq!(q.real(), 1.0);
/// assert_eq!(q.imaginary().x, 0.0);
/// ```
// P1-13: Memory layout verification.
// With #[repr(C)], Quat<T> lays out as: [imaginary.x, imaginary.y, imaginary.z, real]
// This is identical to Vec4<T> layout (4 × sizeof(T) bytes, no padding).
// HdType mapping: Quatf -> FloatVec4, Quatd -> DoubleVec4, Quath -> HalfFloatVec4.
// GPU buffers can thus reinterpret Quat<T> as Vec4<T> without any casting.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Quat<T> {
    /// Imaginary part (i, j, k coefficients) — occupies first 3 × sizeof(T) bytes.
    imaginary: Vec3<T>,
    /// Real part (scalar coefficient) — occupies last sizeof(T) bytes.
    real: T,
}

/// Type alias for double-precision quaternion.
pub type Quatd = Quat<f64>;

/// Type alias for single-precision quaternion.
pub type Quatf = Quat<f32>;

/// Type alias for half-precision quaternion.
pub type Quath = Quat<crate::half::Half>;

impl<T: Scalar> Quat<T> {
    /// Creates a new quaternion from real and imaginary parts.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let q = Quatd::new(1.0, Vec3d::new(0.0, 0.0, 0.0));
    /// assert_eq!(q.real(), 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(real: T, imaginary: Vec3<T>) -> Self {
        Self { imaginary, real }
    }

    /// Creates a new quaternion from real and three imaginary coefficients.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    ///
    /// let q = Quatd::from_components(1.0, 0.0, 0.0, 0.0);
    /// assert_eq!(q.real(), 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_components(real: T, i: T, j: T, k: T) -> Self {
        Self {
            imaginary: Vec3 { x: i, y: j, z: k },
            real,
        }
    }

    /// Creates a quaternion from just the real part (imaginary = 0).
    #[inline]
    #[must_use]
    pub fn from_real(real: T) -> Self {
        Self::new(
            real,
            Vec3 {
                x: T::ZERO,
                y: T::ZERO,
                z: T::ZERO,
            },
        )
    }

    /// Returns the zero quaternion (0 + 0i + 0j + 0k).
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self::from_real(T::ZERO)
    }

    /// Returns the identity quaternion (1 + 0i + 0j + 0k).
    ///
    /// The identity quaternion represents no rotation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    ///
    /// let q = Quatd::identity();
    /// assert_eq!(q.real(), 1.0);
    /// assert_eq!(q.imaginary().x, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self::from_real(T::ONE)
    }

    /// Returns the real part of the quaternion.
    #[inline]
    #[must_use]
    pub fn real(&self) -> T {
        self.real
    }

    /// Returns a reference to the imaginary part of the quaternion.
    #[inline]
    #[must_use]
    pub fn imaginary(&self) -> &Vec3<T> {
        &self.imaginary
    }

    /// Sets the real part of the quaternion.
    #[inline]
    pub fn set_real(&mut self, real: T) {
        self.real = real;
    }

    /// Sets the imaginary part of the quaternion.
    #[inline]
    pub fn set_imaginary(&mut self, imaginary: Vec3<T>) {
        self.imaginary = imaginary;
    }

    /// Sets the imaginary part from three coefficients.
    #[inline]
    pub fn set_imaginary_components(&mut self, i: T, j: T, k: T) {
        self.imaginary = Vec3 { x: i, y: j, z: k };
    }

    /// Returns the squared length of the quaternion.
    #[inline]
    #[must_use]
    pub fn length_squared(&self) -> T {
        self.real * self.real
            + self.imaginary.x * self.imaginary.x
            + self.imaginary.y * self.imaginary.y
            + self.imaginary.z * self.imaginary.z
    }

    /// Returns the conjugate of the quaternion (same real, negated imaginary).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let q = Quatd::new(1.0, Vec3d::new(2.0, 3.0, 4.0));
    /// let conj = q.conjugate();
    /// assert_eq!(conj.real(), 1.0);
    /// assert_eq!(conj.imaginary().x, -2.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn conjugate(&self) -> Self {
        Self::new(
            self.real,
            Vec3 {
                x: -self.imaginary.x,
                y: -self.imaginary.y,
                z: -self.imaginary.z,
            },
        )
    }
}

impl<T: Scalar + Float> Quat<T> {
    /// Creates a quaternion representing rotation around an axis by an angle.
    ///
    /// # Arguments
    ///
    /// * `axis` - The rotation axis (will be normalized)
    /// * `angle_radians` - The rotation angle in radians
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// // 90 degree rotation around Z axis
    /// let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// ```
    #[must_use]
    pub fn from_axis_angle(axis: Vec3<T>, angle_radians: T) -> Self {
        let len_sq = axis.x * axis.x + axis.y * axis.y + axis.z * axis.z;
        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);

        if len_sq < min_len * min_len {
            return Self::identity();
        }

        let len = len_sq.sqrt();
        let half_angle = angle_radians / (T::ONE + T::ONE);
        let s = half_angle.sin() / len;
        let c = half_angle.cos();

        Self::new(
            c,
            Vec3 {
                x: axis.x * s,
                y: axis.y * s,
                z: axis.z * s,
            },
        )
    }

    /// Returns the geometric length of the quaternion.
    #[inline]
    #[must_use]
    pub fn length(&self) -> T {
        self.length_squared().sqrt()
    }

    /// Returns a normalized copy of the quaternion.
    ///
    /// If the length is too small, returns identity.
    #[must_use]
    pub fn normalized(&self) -> Self {
        self.normalized_with_eps(T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON))
    }

    /// Returns a normalized copy with custom epsilon.
    #[must_use]
    pub fn normalized_with_eps(&self, eps: T) -> Self {
        let len = self.length();
        if len < eps {
            return Self::identity();
        }
        *self / len
    }

    /// Normalizes the quaternion in place, returning the original length.
    ///
    /// If the length is too small, sets to identity and returns 0.
    pub fn normalize(&mut self) -> T {
        self.normalize_with_eps(T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON))
    }

    /// Normalizes with custom epsilon, returning the original length.
    pub fn normalize_with_eps(&mut self, eps: T) -> T {
        let len = self.length();
        if len < eps {
            *self = Self::identity();
            return T::ZERO;
        }
        *self /= len;
        len
    }

    /// Returns the inverse (reciprocal) of the quaternion.
    ///
    /// For a unit quaternion, the inverse equals the conjugate.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let inv = q.inverse();
    /// let product = q * inv;
    /// assert!((product.real() - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn inverse(&self) -> Self {
        self.conjugate() / self.length_squared()
    }

    /// Transforms a 3D point by this quaternion.
    ///
    /// For a unit quaternion, this performs a rotation.
    /// Equivalent to: (q * Quat(0, point) * q.inverse()).imaginary()
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// // 90 degree rotation around Z axis
    /// let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let p = Vec3d::new(1.0, 0.0, 0.0);
    /// let rotated = q.transform(&p);
    /// assert!(rotated.x.abs() < 1e-10);
    /// assert!((rotated.y - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn transform(&self, point: &Vec3<T>) -> Vec3<T> {
        // Optimized quaternion-vector rotation formula:
        // v' = v + 2*real*(imag × v) + 2*(imag × (imag × v))
        //    = v + 2*(real*(imag × v) + imag × (imag × v))

        let two = T::ONE + T::ONE;

        // imag × v
        let uv = Vec3 {
            x: self.imaginary.y * point.z - self.imaginary.z * point.y,
            y: self.imaginary.z * point.x - self.imaginary.x * point.z,
            z: self.imaginary.x * point.y - self.imaginary.y * point.x,
        };

        // imag × (imag × v)
        let uuv = Vec3 {
            x: self.imaginary.y * uv.z - self.imaginary.z * uv.y,
            y: self.imaginary.z * uv.x - self.imaginary.x * uv.z,
            z: self.imaginary.x * uv.y - self.imaginary.y * uv.x,
        };

        Vec3 {
            x: point.x + two * (self.real * uv.x + uuv.x),
            y: point.y + two * (self.real * uv.y + uuv.y),
            z: point.z + two * (self.real * uv.z + uuv.z),
        }
    }

    /// Returns the axis and angle of the rotation represented by this quaternion.
    ///
    /// Returns (axis, angle_radians). If the quaternion is identity, returns
    /// ((0,0,1), 0).
    #[must_use]
    pub fn to_axis_angle(&self) -> (Vec3<T>, T) {
        let len = (self.imaginary.x * self.imaginary.x
            + self.imaginary.y * self.imaginary.y
            + self.imaginary.z * self.imaginary.z)
            .sqrt();

        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);
        if len < min_len {
            // No rotation - return arbitrary axis
            return (
                Vec3 {
                    x: T::ZERO,
                    y: T::ZERO,
                    z: T::ONE,
                },
                T::ZERO,
            );
        }

        let axis = Vec3 {
            x: self.imaginary.x / len,
            y: self.imaginary.y / len,
            z: self.imaginary.z / len,
        };
        let two = T::ONE + T::ONE;
        let angle = two * len.atan2(self.real);

        (axis, angle)
    }

    /// Spherical linear interpolation between two quaternions.
    ///
    /// # Arguments
    ///
    /// * `other` - The target quaternion
    /// * `t` - Interpolation parameter (0 = self, 1 = other)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// let q0 = Quatd::identity();
    /// let q1 = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let q_mid = q0.slerp(&q1, 0.5);
    /// ```
    #[must_use]
    pub fn slerp(&self, other: &Self, t: T) -> Self {
        // Compute dot product
        let mut dot = self.real * other.real
            + self.imaginary.x * other.imaginary.x
            + self.imaginary.y * other.imaginary.y
            + self.imaginary.z * other.imaginary.z;

        // If dot is negative, negate one quaternion to take shorter path
        let mut other = *other;
        if dot < T::ZERO {
            other = -other;
            dot = -dot;
        }

        // If very close, use linear interpolation to avoid numerical issues
        let threshold = T::from(0.9995).unwrap_or(T::ONE - T::EPSILON);
        if dot > threshold {
            // Linear interpolation
            let result = Self::new(
                self.real + t * (other.real - self.real),
                Vec3 {
                    x: self.imaginary.x + t * (other.imaginary.x - self.imaginary.x),
                    y: self.imaginary.y + t * (other.imaginary.y - self.imaginary.y),
                    z: self.imaginary.z + t * (other.imaginary.z - self.imaginary.z),
                },
            );
            return result.normalized();
        }

        // Spherical interpolation
        let theta_0 = dot.acos();
        let sin_theta_0 = theta_0.sin();

        // s0 = sin((1-t)*theta_0) / sin(theta_0)
        // s1 = sin(t*theta_0) / sin(theta_0)
        let s0 = ((T::ONE - t) * theta_0).sin() / sin_theta_0;
        let s1 = (t * theta_0).sin() / sin_theta_0;

        Self::new(
            s0 * self.real + s1 * other.real,
            Vec3 {
                x: s0 * self.imaginary.x + s1 * other.imaginary.x,
                y: s0 * self.imaginary.y + s1 * other.imaginary.y,
                z: s0 * self.imaginary.z + s1 * other.imaginary.z,
            },
        )
    }
}

// Default - identity quaternion
impl<T: Scalar> Default for Quat<T> {
    fn default() -> Self {
        Self::identity()
    }
}

// Equality
impl<T: PartialEq> PartialEq for Quat<T> {
    fn eq(&self, other: &Self) -> bool {
        self.real == other.real && self.imaginary == other.imaginary
    }
}

impl<T: Eq> Eq for Quat<T> {}

// Hash
impl<T: Hash> Hash for Quat<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.real.hash(state);
        self.imaginary.hash(state);
    }
}

// Negation
impl<T: Scalar> Neg for Quat<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(
            -self.real,
            Vec3 {
                x: -self.imaginary.x,
                y: -self.imaginary.y,
                z: -self.imaginary.z,
            },
        )
    }
}

// Addition
impl<T: Scalar> Add for Quat<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.real + rhs.real,
            Vec3 {
                x: self.imaginary.x + rhs.imaginary.x,
                y: self.imaginary.y + rhs.imaginary.y,
                z: self.imaginary.z + rhs.imaginary.z,
            },
        )
    }
}

impl<T: Scalar> AddAssign for Quat<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.real = self.real + rhs.real;
        self.imaginary.x = self.imaginary.x + rhs.imaginary.x;
        self.imaginary.y = self.imaginary.y + rhs.imaginary.y;
        self.imaginary.z = self.imaginary.z + rhs.imaginary.z;
    }
}

// Subtraction
impl<T: Scalar> Sub for Quat<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.real - rhs.real,
            Vec3 {
                x: self.imaginary.x - rhs.imaginary.x,
                y: self.imaginary.y - rhs.imaginary.y,
                z: self.imaginary.z - rhs.imaginary.z,
            },
        )
    }
}

impl<T: Scalar> SubAssign for Quat<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.real = self.real - rhs.real;
        self.imaginary.x = self.imaginary.x - rhs.imaginary.x;
        self.imaginary.y = self.imaginary.y - rhs.imaginary.y;
        self.imaginary.z = self.imaginary.z - rhs.imaginary.z;
    }
}

// Quaternion multiplication (Hamilton product)
impl<T: Scalar> Mul for Quat<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        // Hamilton product:
        // (a + bi + cj + dk) * (e + fi + gj + hk)
        // = (ae - bf - cg - dh) + (af + be + ch - dg)i + (ag - bh + ce + df)j + (ah + bg - cf + de)k
        let a = self.real;
        let b = self.imaginary.x;
        let c = self.imaginary.y;
        let d = self.imaginary.z;
        let e = rhs.real;
        let f = rhs.imaginary.x;
        let g = rhs.imaginary.y;
        let h = rhs.imaginary.z;

        Self::new(
            a * e - b * f - c * g - d * h,
            Vec3 {
                x: a * f + b * e + c * h - d * g,
                y: a * g - b * h + c * e + d * f,
                z: a * h + b * g - c * f + d * e,
            },
        )
    }
}

impl<T: Scalar> MulAssign for Quat<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Quat<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(
            self.real * rhs,
            Vec3 {
                x: self.imaginary.x * rhs,
                y: self.imaginary.y * rhs,
                z: self.imaginary.z * rhs,
            },
        )
    }
}

impl<T: Scalar> MulAssign<T> for Quat<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.real = self.real * rhs;
        self.imaginary.x = self.imaginary.x * rhs;
        self.imaginary.y = self.imaginary.y * rhs;
        self.imaginary.z = self.imaginary.z * rhs;
    }
}

// Scalar on left
impl Mul<Quat<f64>> for f64 {
    type Output = Quat<f64>;

    #[inline]
    fn mul(self, rhs: Quat<f64>) -> Quat<f64> {
        rhs * self
    }
}

impl Mul<Quat<f32>> for f32 {
    type Output = Quat<f32>;

    #[inline]
    fn mul(self, rhs: Quat<f32>) -> Quat<f32> {
        rhs * self
    }
}

// Scalar division
impl<T: Scalar> Div<T> for Quat<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: T) -> Self {
        Self::new(
            self.real / rhs,
            Vec3 {
                x: self.imaginary.x / rhs,
                y: self.imaginary.y / rhs,
                z: self.imaginary.z / rhs,
            },
        )
    }
}

impl<T: Scalar> DivAssign<T> for Quat<T> {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        self.real = self.real / rhs;
        self.imaginary.x = self.imaginary.x / rhs;
        self.imaginary.y = self.imaginary.y / rhs;
        self.imaginary.z = self.imaginary.z / rhs;
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Quat<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {}, {}, {})",
            self.real, self.imaginary.x, self.imaginary.y, self.imaginary.z
        )
    }
}

/// Returns the dot product of two quaternions.
///
/// # Examples
///
/// ```
/// use usd_gf::quat::{Quatd, dot};
///
/// let q1 = Quatd::identity();
/// let q2 = Quatd::identity();
/// assert!((dot(&q1, &q2) - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn dot<T: Scalar>(q1: &Quat<T>, q2: &Quat<T>) -> T {
    q1.real * q2.real
        + q1.imaginary.x * q2.imaginary.x
        + q1.imaginary.y * q2.imaginary.y
        + q1.imaginary.z * q2.imaginary.z
}

/// Spherical linear interpolation between two quaternions.
///
/// # Arguments
///
/// * `alpha` - Interpolation parameter (0 = q0, 1 = q1)
/// * `q0` - Start quaternion
/// * `q1` - End quaternion
#[inline]
#[must_use]
pub fn slerp<T: Scalar + Float>(alpha: T, q0: &Quat<T>, q1: &Quat<T>) -> Quat<T> {
    q0.slerp(q1, alpha)
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Quatf> for Quatd {
    fn from(other: Quatf) -> Self {
        use crate::vec3::Vec3d;
        Self::new(
            other.real() as f64,
            Vec3d::new(
                other.imaginary().x as f64,
                other.imaginary().y as f64,
                other.imaginary().z as f64,
            ),
        )
    }
}

impl From<Quath> for Quatd {
    fn from(other: Quath) -> Self {
        use crate::vec3::Vec3d;
        Self::new(
            other.real().to_f64(),
            Vec3d::new(
                other.imaginary().x.to_f64(),
                other.imaginary().y.to_f64(),
                other.imaginary().z.to_f64(),
            ),
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Quatf> for Quatd {
    fn eq(&self, other: &Quatf) -> bool {
        (self.real() - other.real() as f64).abs() < f64::EPSILON
            && (self.imaginary().x - other.imaginary().x as f64).abs() < f64::EPSILON
            && (self.imaginary().y - other.imaginary().y as f64).abs() < f64::EPSILON
            && (self.imaginary().z - other.imaginary().z as f64).abs() < f64::EPSILON
    }
}

impl PartialEq<Quath> for Quatd {
    fn eq(&self, other: &Quath) -> bool {
        (self.real() - other.real().to_f64()).abs() < f64::EPSILON
            && (self.imaginary().x - other.imaginary().x.to_f64()).abs() < f64::EPSILON
            && (self.imaginary().y - other.imaginary().y.to_f64()).abs() < f64::EPSILON
            && (self.imaginary().z - other.imaginary().z.to_f64()).abs() < f64::EPSILON
    }
}

/// Creates a Quatd from components.
#[inline]
#[must_use]
pub fn quatd(real: f64, i: f64, j: f64, k: f64) -> Quatd {
    Quatd::from_components(real, i, j, k)
}

/// Creates a Quatf from components.
#[inline]
#[must_use]
pub fn quatf(real: f32, i: f32, j: f32, k: f32) -> Quatf {
    Quatf::from_components(real, i, j, k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3d;
    use std::f64::consts::PI;

    #[test]
    fn test_identity() {
        let q = Quatd::identity();
        assert_eq!(q.real(), 1.0);
        assert_eq!(q.imaginary().x, 0.0);
        assert_eq!(q.imaginary().y, 0.0);
        assert_eq!(q.imaginary().z, 0.0);
    }

    #[test]
    fn test_zero() {
        let q = Quatd::zero();
        assert_eq!(q.real(), 0.0);
        assert_eq!(q.imaginary().x, 0.0);
    }

    #[test]
    fn test_new() {
        let q = Quatd::new(1.0, Vec3d::new(2.0, 3.0, 4.0));
        assert_eq!(q.real(), 1.0);
        assert_eq!(q.imaginary().x, 2.0);
        assert_eq!(q.imaginary().y, 3.0);
        assert_eq!(q.imaginary().z, 4.0);
    }

    #[test]
    fn test_from_components() {
        let q = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        assert_eq!(q.real(), 1.0);
        assert_eq!(q.imaginary().x, 2.0);
    }

    #[test]
    fn test_conjugate() {
        let q = Quatd::new(1.0, Vec3d::new(2.0, 3.0, 4.0));
        let conj = q.conjugate();
        assert_eq!(conj.real(), 1.0);
        assert_eq!(conj.imaginary().x, -2.0);
        assert_eq!(conj.imaginary().y, -3.0);
        assert_eq!(conj.imaginary().z, -4.0);
    }

    #[test]
    fn test_length() {
        let q = Quatd::identity();
        assert!((q.length() - 1.0).abs() < 1e-10);

        let q2 = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let expected = (1.0 + 4.0 + 9.0 + 16.0_f64).sqrt();
        assert!((q2.length() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_normalize() {
        let q = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let normalized = q.normalized();
        assert!((normalized.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let inv = q.inverse();
        let product = q * inv;
        assert!((product.real() - 1.0).abs() < 1e-10);
        assert!(product.imaginary().x.abs() < 1e-10);
        assert!(product.imaginary().y.abs() < 1e-10);
        assert!(product.imaginary().z.abs() < 1e-10);
    }

    #[test]
    fn test_from_axis_angle() {
        // 180 degree rotation around Z axis
        let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI);
        // real = cos(PI/2) = 0
        // imaginary.z = sin(PI/2) = 1
        assert!(q.real().abs() < 1e-10);
        assert!(q.imaginary().x.abs() < 1e-10);
        assert!(q.imaginary().y.abs() < 1e-10);
        assert!((q.imaginary().z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_to_axis_angle() {
        let axis = Vec3d::new(0.0, 0.0, 1.0);
        let angle = PI / 3.0;
        let q = Quatd::from_axis_angle(axis, angle);
        let (recovered_axis, recovered_angle) = q.to_axis_angle();

        assert!((recovered_axis.x - axis.x).abs() < 1e-10);
        assert!((recovered_axis.y - axis.y).abs() < 1e-10);
        assert!((recovered_axis.z - axis.z).abs() < 1e-10);
        assert!((recovered_angle - angle).abs() < 1e-10);
    }

    #[test]
    fn test_transform() {
        // 90 degree rotation around Z axis
        let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
        let p = Vec3d::new(1.0, 0.0, 0.0);
        let rotated = q.transform(&p);

        // X axis should rotate to Y axis
        assert!(rotated.x.abs() < 1e-10);
        assert!((rotated.y - 1.0).abs() < 1e-10);
        assert!(rotated.z.abs() < 1e-10);
    }

    #[test]
    fn test_transform_identity() {
        let q = Quatd::identity();
        let p = Vec3d::new(1.0, 2.0, 3.0);
        let result = q.transform(&p);

        assert!((result.x - p.x).abs() < 1e-10);
        assert!((result.y - p.y).abs() < 1e-10);
        assert!((result.z - p.z).abs() < 1e-10);
    }

    #[test]
    fn test_multiplication() {
        let q1 = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let q2 = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let combined = q1 * q2;

        // Combined should be 90 degree rotation
        let p = Vec3d::new(1.0, 0.0, 0.0);
        let result = combined.transform(&p);

        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_addition() {
        let q1 = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let q2 = Quatd::from_components(5.0, 6.0, 7.0, 8.0);
        let sum = q1 + q2;

        assert_eq!(sum.real(), 6.0);
        assert_eq!(sum.imaginary().x, 8.0);
        assert_eq!(sum.imaginary().y, 10.0);
        assert_eq!(sum.imaginary().z, 12.0);
    }

    #[test]
    fn test_subtraction() {
        let q1 = Quatd::from_components(5.0, 6.0, 7.0, 8.0);
        let q2 = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let diff = q1 - q2;

        assert_eq!(diff.real(), 4.0);
        assert_eq!(diff.imaginary().x, 4.0);
    }

    #[test]
    fn test_scalar_multiplication() {
        let q = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let scaled = q * 2.0;

        assert_eq!(scaled.real(), 2.0);
        assert_eq!(scaled.imaginary().x, 4.0);
        assert_eq!(scaled.imaginary().y, 6.0);
        assert_eq!(scaled.imaginary().z, 8.0);
    }

    #[test]
    fn test_scalar_division() {
        let q = Quatd::from_components(2.0, 4.0, 6.0, 8.0);
        let divided = q / 2.0;

        assert_eq!(divided.real(), 1.0);
        assert_eq!(divided.imaginary().x, 2.0);
    }

    #[test]
    fn test_negation() {
        let q = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let neg = -q;

        assert_eq!(neg.real(), -1.0);
        assert_eq!(neg.imaginary().x, -2.0);
    }

    #[test]
    fn test_slerp() {
        let q0 = Quatd::identity();
        let q1 = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);

        // At t=0, should be q0
        let t0 = q0.slerp(&q1, 0.0);
        assert!((t0.real() - q0.real()).abs() < 1e-10);

        // At t=1, should be q1
        let t1 = q0.slerp(&q1, 1.0);
        assert!((t1.real() - q1.real()).abs() < 1e-10);

        // At t=0.5, should be 45 degree rotation
        let t_mid = q0.slerp(&q1, 0.5);
        let p = Vec3d::new(1.0, 0.0, 0.0);
        let result = t_mid.transform(&p);

        // Should be approximately (cos(45), sin(45), 0) = (0.707, 0.707, 0)
        assert!((result.x - 0.7071067811865476).abs() < 1e-10);
        assert!((result.y - 0.7071067811865476).abs() < 1e-10);
    }

    #[test]
    fn test_dot() {
        let q1 = Quatd::identity();
        let q2 = Quatd::identity();
        assert!((dot(&q1, &q2) - 1.0).abs() < 1e-10);

        let q3 = Quatd::from_components(1.0, 0.0, 0.0, 0.0);
        let q4 = Quatd::from_components(0.0, 1.0, 0.0, 0.0);
        assert!(dot(&q3, &q4).abs() < 1e-10);
    }

    #[test]
    fn test_helper_functions() {
        let q = quatd(1.0, 2.0, 3.0, 4.0);
        assert_eq!(q.real(), 1.0);

        let qf = quatf(1.0, 2.0, 3.0, 4.0);
        assert_eq!(qf.real(), 1.0);
    }

    #[test]
    fn test_display() {
        let q = Quatd::from_components(1.0, 2.0, 3.0, 4.0);
        let s = format!("{}", q);
        assert!(s.contains("1"));
        assert!(s.contains("4"));
    }
}
