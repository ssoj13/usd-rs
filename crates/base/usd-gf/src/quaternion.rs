//! Legacy quaternion type (GfQuaternion).
//!
//! This is the older double-precision quaternion type, distinct from [`Quatd`].
//! It represents a quaternion as a scalar real part and a vector imaginary part.
//!
//! This type is used by [`Rotation`] and is part of the VT value types.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Quaternion, vec3d};
//!
//! let q = Quaternion::new(1.0, vec3d(0.0, 0.0, 0.0));
//! assert!(q.is_identity());
//!
//! let identity = Quaternion::identity();
//! assert_eq!(identity.real(), 1.0);
//! ```

use crate::limits::MIN_VECTOR_LENGTH;
use crate::vec3::{Vec3d, cross as vec3_cross};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Legacy quaternion type with scalar real and vector imaginary parts.
///
/// Represents a quaternion as (real, imaginary) where:
/// - `real` is a scalar (double)
/// - `imaginary` is a 3D vector (x, y, z)
///
/// The full quaternion is: q = real + imaginary.x*i + imaginary.y*j + imaginary.z*k
///
/// # Relationship to Quatd
///
/// This is the older quaternion API. [`Quatd`] stores (x, y, z, w) where w is the
/// scalar part. This type stores (real, imaginary) where real=w and imaginary=(x,y,z).
///
/// # Examples
///
/// ```
/// use usd_gf::{Quaternion, vec3d};
///
/// // Create identity quaternion
/// let identity = Quaternion::identity();
/// assert_eq!(identity.real(), 1.0);
///
/// // Multiply quaternions
/// let q1 = Quaternion::new(0.707, vec3d(0.0, 0.0, 0.707));
/// let q2 = Quaternion::new(0.707, vec3d(0.0, 0.707, 0.0));
/// let product = q1 * q2;
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Quaternion {
    /// Scalar (real) part.
    real: f64,
    /// Vector (imaginary) part.
    imaginary: Vec3d,
}

impl Default for Quaternion {
    /// Default constructor creates identity quaternion.
    fn default() -> Self {
        Self::identity()
    }
}

impl Quaternion {
    /// Creates a quaternion from real and imaginary parts.
    #[inline]
    #[must_use]
    pub fn new(real: f64, imaginary: Vec3d) -> Self {
        Self { real, imaginary }
    }

    /// Creates a quaternion from just the real part (imaginary = 0).
    ///
    /// Since quaternions need to be normalized, only -1, 0, or 1 are
    /// meaningful values for the real part alone.
    #[inline]
    #[must_use]
    pub fn from_real(real: f64) -> Self {
        Self {
            real,
            imaginary: Vec3d::new(0.0, 0.0, 0.0),
        }
    }

    /// Returns the zero quaternion (0 + 0i + 0j + 0k).
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self {
            real: 0.0,
            imaginary: Vec3d::new(0.0, 0.0, 0.0),
        }
    }

    /// Returns the identity quaternion (1 + 0i + 0j + 0k).
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self {
            real: 1.0,
            imaginary: Vec3d::new(0.0, 0.0, 0.0),
        }
    }

    /// Sets the real (scalar) part.
    #[inline]
    pub fn set_real(&mut self, real: f64) {
        self.real = real;
    }

    /// Sets the imaginary (vector) part.
    #[inline]
    pub fn set_imaginary(&mut self, imaginary: Vec3d) {
        self.imaginary = imaginary;
    }

    /// Returns the real (scalar) part.
    #[inline]
    #[must_use]
    pub fn real(&self) -> f64 {
        self.real
    }

    /// Returns the imaginary (vector) part.
    #[inline]
    #[must_use]
    pub fn imaginary(&self) -> &Vec3d {
        &self.imaginary
    }

    /// Returns true if this is the identity quaternion.
    #[inline]
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.real == 1.0
            && self.imaginary.x == 0.0
            && self.imaginary.y == 0.0
            && self.imaginary.z == 0.0
    }

    /// Returns the squared length of the quaternion.
    #[inline]
    #[must_use]
    fn length_squared(&self) -> f64 {
        self.real * self.real + self.imaginary.dot(&self.imaginary)
    }

    /// Returns the geometric length of the quaternion.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Returns a normalized (unit-length) version of this quaternion.
    ///
    /// If the length is smaller than `eps`, returns identity quaternion.
    #[must_use]
    pub fn normalized(&self, eps: f64) -> Self {
        let len = self.length();
        if len < eps {
            return Self::identity();
        }
        *self / len
    }

    /// Normalizes this quaternion in place.
    ///
    /// Returns the length before normalization. If length < eps, sets to identity.
    pub fn normalize(&mut self, eps: f64) -> f64 {
        let len = self.length();
        if len < eps {
            *self = Self::identity();
        } else {
            *self /= len;
        }
        len
    }

    /// Normalizes with default epsilon.
    #[must_use]
    pub fn get_normalized(&self) -> Self {
        self.normalized(MIN_VECTOR_LENGTH)
    }

    /// Returns the inverse (conjugate/|q|^2) of this quaternion.
    #[must_use]
    pub fn inverse(&self) -> Self {
        let len_sq = self.length_squared();
        if len_sq == 0.0 {
            return Self::zero();
        }
        Self {
            real: self.real / len_sq,
            imaginary: -self.imaginary / len_sq,
        }
    }

    /// Alias for inverse().
    #[inline]
    #[must_use]
    pub fn get_inverse(&self) -> Self {
        self.inverse()
    }
}

impl PartialEq for Quaternion {
    fn eq(&self, other: &Self) -> bool {
        self.real == other.real && self.imaginary == other.imaginary
    }
}

impl Eq for Quaternion {}

impl Hash for Quaternion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.real.to_bits().hash(state);
        self.imaginary.x.to_bits().hash(state);
        self.imaginary.y.to_bits().hash(state);
        self.imaginary.z.to_bits().hash(state);
    }
}

// Quaternion multiplication: q1 * q2
impl Mul for Quaternion {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        // (r1, v1) * (r2, v2) = (r1*r2 - v1.v2, r1*v2 + r2*v1 + v1 x v2)
        let real = self.real * rhs.real - self.imaginary.dot(&rhs.imaginary);
        let imaginary = rhs.imaginary * self.real
            + self.imaginary * rhs.real
            + vec3_cross(&self.imaginary, &rhs.imaginary);
        Self { real, imaginary }
    }
}

impl MulAssign for Quaternion {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

// Scalar multiplication
impl Mul<f64> for Quaternion {
    type Output = Self;

    fn mul(self, s: f64) -> Self::Output {
        Self {
            real: self.real * s,
            imaginary: self.imaginary * s,
        }
    }
}

impl Mul<Quaternion> for f64 {
    type Output = Quaternion;

    fn mul(self, q: Quaternion) -> Self::Output {
        q * self
    }
}

impl MulAssign<f64> for Quaternion {
    fn mul_assign(&mut self, s: f64) {
        self.real *= s;
        self.imaginary = self.imaginary * s;
    }
}

// Scalar division
impl Div<f64> for Quaternion {
    type Output = Self;

    fn div(self, s: f64) -> Self::Output {
        Self {
            real: self.real / s,
            imaginary: self.imaginary / s,
        }
    }
}

impl DivAssign<f64> for Quaternion {
    fn div_assign(&mut self, s: f64) {
        self.real /= s;
        self.imaginary = self.imaginary / s;
    }
}

// Addition
impl Add for Quaternion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real + rhs.real,
            imaginary: self.imaginary + rhs.imaginary,
        }
    }
}

impl AddAssign for Quaternion {
    fn add_assign(&mut self, rhs: Self) {
        self.real += rhs.real;
        self.imaginary = self.imaginary + rhs.imaginary;
    }
}

// Subtraction
impl Sub for Quaternion {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real - rhs.real,
            imaginary: self.imaginary - rhs.imaginary,
        }
    }
}

impl SubAssign for Quaternion {
    fn sub_assign(&mut self, rhs: Self) {
        self.real -= rhs.real;
        self.imaginary = self.imaginary - rhs.imaginary;
    }
}

impl fmt::Display for Quaternion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({} + ({}, {}, {}))",
            self.real, self.imaginary.x, self.imaginary.y, self.imaginary.z
        )
    }
}

/// Returns the dot product of two quaternions.
#[inline]
#[must_use]
pub fn dot(q1: &Quaternion, q2: &Quaternion) -> f64 {
    q1.real * q2.real + q1.imaginary.dot(&q2.imaginary)
}

/// Spherically interpolates between two quaternions.
///
/// - alpha = 0 returns q0
/// - alpha = 1 returns q1
#[must_use]
pub fn slerp(alpha: f64, q0: &Quaternion, q1: &Quaternion) -> Quaternion {
    let cos_theta = dot(q0, q1);

    // If the quaternions are nearly parallel, use linear interpolation
    if cos_theta.abs() > 0.9995 {
        let result = *q0 * (1.0 - alpha) + *q1 * alpha;
        return result.get_normalized();
    }

    // Clamp to avoid domain errors
    let cos_theta = cos_theta.clamp(-1.0, 1.0);
    let theta = cos_theta.acos();
    let sin_theta = theta.sin();

    let s0 = ((1.0 - alpha) * theta).sin() / sin_theta;
    let s1 = (alpha * theta).sin() / sin_theta;

    *q0 * s0 + *q1 * s1
}

/// Legacy slerp signature for compatibility.
#[inline]
#[must_use]
pub fn slerp_legacy(q0: &Quaternion, q1: &Quaternion, alpha: f64) -> Quaternion {
    slerp(alpha, q0, q1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    #[test]
    fn test_identity() {
        let q = Quaternion::identity();
        assert_eq!(q.real(), 1.0);
        assert_eq!(q.imaginary(), &Vec3d::new(0.0, 0.0, 0.0));
        assert!(q.is_identity());
    }

    #[test]
    fn test_zero() {
        let q = Quaternion::zero();
        assert_eq!(q.real(), 0.0);
        assert_eq!(q.imaginary(), &Vec3d::new(0.0, 0.0, 0.0));
        assert!(!q.is_identity());
    }

    #[test]
    fn test_new() {
        let q = Quaternion::new(0.5, vec3d(0.5, 0.5, 0.5));
        assert_eq!(q.real(), 0.5);
        assert_eq!(q.imaginary().x, 0.5);
    }

    #[test]
    fn test_length() {
        let q = Quaternion::identity();
        assert!((q.length() - 1.0).abs() < 1e-10);

        let q2 = Quaternion::new(1.0, vec3d(1.0, 1.0, 1.0));
        assert!((q2.length() - 2.0).abs() < 1e-10); // sqrt(1 + 1 + 1 + 1) = 2
    }

    #[test]
    fn test_normalize() {
        let q = Quaternion::new(1.0, vec3d(1.0, 1.0, 1.0));
        let normalized = q.get_normalized();
        assert!((normalized.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multiplication() {
        let q1 = Quaternion::identity();
        let q2 = Quaternion::new(0.5, vec3d(0.5, 0.5, 0.5));

        // identity * q = q
        let result = q1 * q2;
        assert!((result.real() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let q = Quaternion::new(0.5, vec3d(0.5, 0.5, 0.5)).get_normalized();
        let inv = q.get_inverse();
        let product = q * inv;

        // q * q^-1 should be identity
        assert!((product.real() - 1.0).abs() < 1e-10);
        assert!(product.imaginary().length() < 1e-10);
    }

    #[test]
    fn test_scalar_ops() {
        let q = Quaternion::new(1.0, vec3d(2.0, 3.0, 4.0));

        let doubled = q * 2.0;
        assert_eq!(doubled.real(), 2.0);
        assert_eq!(doubled.imaginary().x, 4.0);

        let halved = q / 2.0;
        assert_eq!(halved.real(), 0.5);
        assert_eq!(halved.imaginary().x, 1.0);
    }

    #[test]
    fn test_addition_subtraction() {
        let q1 = Quaternion::new(1.0, vec3d(1.0, 1.0, 1.0));
        let q2 = Quaternion::new(2.0, vec3d(2.0, 2.0, 2.0));

        let sum = q1 + q2;
        assert_eq!(sum.real(), 3.0);
        assert_eq!(sum.imaginary().x, 3.0);

        let diff = q2 - q1;
        assert_eq!(diff.real(), 1.0);
        assert_eq!(diff.imaginary().x, 1.0);
    }

    #[test]
    fn test_dot() {
        let q1 = Quaternion::new(1.0, vec3d(0.0, 0.0, 0.0));
        let q2 = Quaternion::new(1.0, vec3d(0.0, 0.0, 0.0));
        assert!((dot(&q1, &q2) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_slerp() {
        let q0 = Quaternion::identity();
        let q1 = Quaternion::new(0.0, vec3d(1.0, 0.0, 0.0)).get_normalized();

        // alpha=0 should give q0
        let result0 = slerp(0.0, &q0, &q1);
        assert!((result0.real() - 1.0).abs() < 1e-10);

        // alpha=1 should give q1 (approximately, due to normalization)
        let result1 = slerp(1.0, &q0, &q1);
        assert!((result1.imaginary().x.abs() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_display() {
        let q = Quaternion::new(1.0, vec3d(2.0, 3.0, 4.0));
        let s = format!("{}", q);
        assert!(s.contains("1"));
        assert!(s.contains("2"));
    }

    #[test]
    fn test_equality() {
        let q1 = Quaternion::new(1.0, vec3d(2.0, 3.0, 4.0));
        let q2 = Quaternion::new(1.0, vec3d(2.0, 3.0, 4.0));
        let q3 = Quaternion::new(1.0, vec3d(2.0, 3.0, 5.0));

        assert_eq!(q1, q2);
        assert_ne!(q1, q3);
    }
}
