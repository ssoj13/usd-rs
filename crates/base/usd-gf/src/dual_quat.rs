//! Dual Quaternion types for rigid body transformations.
//!
//! Dual quaternions represent rotation and translation in a unified form,
//! useful for skeletal animation blending (DLB - Dual quaternion Linear Blending).
//!
//! A dual quaternion consists of two quaternions:
//! - Real part: encodes rotation
//! - Dual part: encodes translation (combined with real part)
//!
//! # References
//!
//! - [Kavan et al. 2006](https://www.cs.utah.edu/~ladislav/kavan06dual/kavan06dual.pdf)
//! - [Dual Quaternion Tutorial](https://faculty.sites.iastate.edu/jia/files/inline-files/dual-quaternion.pdf)
//!
//! # Examples
//!
//! ```
//! use usd_gf::{DualQuatd, Quatd, Vec3d};
//! use std::f64::consts::PI;
//!
//! // Create from rotation and translation
//! let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 1.0, 0.0), PI / 2.0);
//! let translation = Vec3d::new(1.0, 2.0, 3.0);
//! let dq = DualQuatd::from_rotation_translation(&rotation, &translation);
//!
//! // Transform a point
//! let p = Vec3d::new(1.0, 0.0, 0.0);
//! let result = dq.transform(&p);
//! ```

use crate::half::Half;
use crate::limits::MIN_VECTOR_LENGTH;
use crate::ostream_helpers::{ostream_helper_p_double, ostream_helper_p_float};
use crate::quat::{Quat, dot};
use crate::traits::Scalar;
use crate::vec3::Vec3;
use num_traits::Float;

/// Trait for default normalize epsilon (matches OpenUSD GF_MIN_VECTOR_LENGTH).
pub trait GfNormalizeEps {
    /// Returns the minimum vector length below which normalization treats the vector as zero.
    fn default_normalize_eps() -> Self;
}
impl GfNormalizeEps for f64 {
    fn default_normalize_eps() -> Self {
        MIN_VECTOR_LENGTH
    }
}
impl GfNormalizeEps for f32 {
    fn default_normalize_eps() -> Self {
        MIN_VECTOR_LENGTH as f32
    }
}
impl GfNormalizeEps for Half {
    fn default_normalize_eps() -> Self {
        Half::from_f32(0.001)
    }
}
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Generic dual quaternion type.
///
/// A dual quaternion `dq = r + εd` where:
/// - `r` is the real quaternion (rotation)
/// - `d` is the dual quaternion (translation encoded)
/// - `ε` is the dual number unit (ε² = 0)
///
/// For rigid transforms:
/// - The real part encodes the rotation
/// - The translation is recovered via `2 * (d * r.conjugate()).imaginary`
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct DualQuat<T> {
    /// Real part quaternion (encodes rotation).
    real: Quat<T>,
    /// Dual part quaternion (encodes translation combined with real).
    dual: Quat<T>,
}

/// Double-precision dual quaternion.
pub type DualQuatd = DualQuat<f64>;

/// Single-precision dual quaternion.
pub type DualQuatf = DualQuat<f32>;

/// Half-precision dual quaternion.
pub type DualQuath = DualQuat<crate::half::Half>;

impl<T: Scalar> DualQuat<T> {
    /// Creates a new dual quaternion from real and dual parts.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{DualQuatd, Quatd, Vec3d};
    ///
    /// let real = Quatd::identity();
    /// let dual = Quatd::zero();
    /// let dq = DualQuatd::new(real, dual);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(real: Quat<T>, dual: Quat<T>) -> Self {
        Self { real, dual }
    }

    /// Creates a dual quaternion with only the real part set.
    ///
    /// The dual part is set to zero.
    #[inline]
    #[must_use]
    pub fn from_real(real: Quat<T>) -> Self {
        Self {
            real,
            dual: Quat::zero(),
        }
    }

    /// Creates a dual quaternion from a scalar value.
    ///
    /// Sets the real part to a quaternion with that scalar as the real component
    /// and zero imaginary, and the dual part to zero.
    #[inline]
    #[must_use]
    pub fn from_scalar(value: T) -> Self {
        Self {
            real: Quat::from_real(value),
            dual: Quat::zero(),
        }
    }

    /// Returns the zero dual quaternion.
    ///
    /// Both real and dual parts are zero quaternions.
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self {
            real: Quat::zero(),
            dual: Quat::zero(),
        }
    }

    /// Returns the identity dual quaternion.
    ///
    /// Real part is identity quaternion, dual part is zero.
    /// Represents no rotation and no translation.
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self {
            real: Quat::identity(),
            dual: Quat::zero(),
        }
    }

    /// Returns the real (rotation) part.
    #[inline]
    #[must_use]
    pub fn real(&self) -> &Quat<T> {
        &self.real
    }

    /// Returns the dual part.
    #[inline]
    #[must_use]
    pub fn dual(&self) -> &Quat<T> {
        &self.dual
    }

    /// Sets the real (rotation) part.
    #[inline]
    pub fn set_real(&mut self, real: Quat<T>) {
        self.real = real;
    }

    /// Sets the dual part.
    #[inline]
    pub fn set_dual(&mut self, dual: Quat<T>) {
        self.dual = dual;
    }
}

impl<T: Scalar + Float + GfNormalizeEps> DualQuat<T> {
    /// Creates a dual quaternion from rotation and translation.
    ///
    /// This is the primary constructor for rigid body transforms.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{DualQuatd, Quatd, Vec3d};
    /// use std::f64::consts::PI;
    ///
    /// let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let translation = Vec3d::new(10.0, 0.0, 0.0);
    /// let dq = DualQuatd::from_rotation_translation(&rotation, &translation);
    ///
    /// // The translation can be recovered
    /// let t = dq.translation();
    /// assert!((t.x - 10.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_rotation_translation(rotation: &Quat<T>, translation: &Vec3<T>) -> Self {
        let mut dq = Self {
            real: *rotation,
            dual: Quat::zero(),
        };
        dq.set_translation(translation);
        dq
    }

    /// Sets the translation component.
    ///
    /// The dual part is computed as: `dual = Quat(0, 0.5 * translation) * real`
    pub fn set_translation(&mut self, translation: &Vec3<T>) {
        let half = T::ONE / (T::ONE + T::ONE);
        let half_t = Vec3 {
            x: half * translation.x,
            y: half * translation.y,
            z: half * translation.z,
        };
        // dual = Quat(0, half_t) * real
        let t_quat = Quat::new(T::ZERO, half_t);
        self.dual = t_quat * self.real;
    }

    /// Gets the translation component.
    ///
    /// Returns `2 * (dual * real.conjugate()).imaginary`.
    /// Assumes the dual quaternion is normalized (real part unit length).
    /// Per C++ GetTranslation: TF_DEV_AXIOM asserts real part is unit length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{DualQuatd, Quatd, Vec3d};
    ///
    /// let rotation = Quatd::identity();
    /// let translation = Vec3d::new(1.0, 2.0, 3.0);
    /// let dq = DualQuatd::from_rotation_translation(&rotation, &translation);
    ///
    /// let t = dq.translation();
    /// assert!((t.x - 1.0).abs() < 1e-10);
    /// assert!((t.y - 2.0).abs() < 1e-10);
    /// assert!((t.z - 3.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn translation(&self) -> Vec3<T> {
        #[cfg(debug_assertions)]
        {
            let real_len = self.real.length();
            debug_assert!(
                (real_len - T::ONE).abs() < T::default_normalize_eps(),
                "DualQuat::translation assumes normalized dual quaternion (real part unit length)"
            );
        }
        // translation = 2 * (dual * real.conjugate()).imaginary
        // But we inline for efficiency
        let r1 = self.dual.real();
        let r2 = self.real.real();
        let i1 = self.dual.imaginary();
        let i2 = self.real.imaginary();

        let two = T::ONE + T::ONE;
        // -2.0 * (r1*i2 - r2*i1 + i1 × i2)
        Vec3 {
            x: -two * (r1 * i2.x - r2 * i1.x + (i1.y * i2.z - i1.z * i2.y)),
            y: -two * (r1 * i2.y - r2 * i1.y + (i1.z * i2.x - i1.x * i2.z)),
            z: -two * (r1 * i2.z - r2 * i1.z + (i1.x * i2.y - i1.y * i2.x)),
        }
    }

    /// Returns the geometric length as a pair (real_length, dual_component).
    ///
    /// For a unit dual quaternion, real_length is 1 and dual_component is 0.
    #[must_use]
    pub fn length(&self) -> (T, T) {
        let real_len = self.real.length();
        if real_len == T::ZERO {
            return (T::ZERO, T::ZERO);
        }
        let dual_comp = dot(&self.real, &self.dual) / real_len;
        (real_len, dual_comp)
    }

    /// Returns a normalized version of this dual quaternion.
    ///
    /// If the real part length is less than eps, returns identity.
    /// Uses GF_MIN_VECTOR_LENGTH (1e-10) for f32/f64, 0.001 for Half.
    #[must_use]
    pub fn normalized(&self) -> Self {
        self.normalized_with_eps(T::default_normalize_eps())
    }

    /// Returns a normalized version with custom epsilon.
    #[must_use]
    pub fn normalized_with_eps(&self, eps: T) -> Self {
        let mut result = *self;
        result.normalize_with_eps(eps);
        result
    }

    /// Normalizes this dual quaternion in place.
    ///
    /// Returns the length before normalization.
    pub fn normalize(&mut self) -> (T, T) {
        self.normalize_with_eps(T::default_normalize_eps())
    }

    /// Normalizes with custom epsilon, returning original length.
    ///
    /// After normalization:
    /// - Real part has unit length
    /// - Dual part is orthogonal to real part (dot(real, dual) = 0)
    pub fn normalize_with_eps(&mut self, eps: T) -> (T, T) {
        let len = self.length();
        let real_len = len.0;

        if real_len < eps {
            *self = Self::identity();
        } else {
            let inv_real_len = T::ONE / real_len;
            self.real *= inv_real_len;
            self.dual *= inv_real_len;

            // Make dual orthogonal to real: dual -= dot(real, dual) * real
            let d = dot(&self.real, &self.dual);
            self.dual -= self.real * d;
        }

        len
    }

    /// Returns the conjugate of this dual quaternion.
    ///
    /// Conjugate is `(real.conjugate(), dual.conjugate())`.
    #[must_use]
    pub fn conjugate(&self) -> Self {
        Self {
            real: self.real.conjugate(),
            dual: self.dual.conjugate(),
        }
    }

    /// Returns the inverse of this dual quaternion.
    ///
    /// For a unit dual quaternion, inverse equals conjugate.
    #[must_use]
    pub fn inverse(&self) -> Self {
        let real_len_sq = dot(&self.real, &self.real);

        if real_len_sq <= T::ZERO {
            return Self::identity();
        }

        let inv_real_len_sq = T::ONE / real_len_sq;
        let conj_inv = self.conjugate() * inv_real_len_sq;

        let two = T::ONE + T::ONE;
        let d = dot(&self.real, &self.dual);
        let real_part = *conj_inv.real();
        let dual_part = *conj_inv.dual() - real_part * (two * inv_real_len_sq * d);

        Self {
            real: real_part,
            dual: dual_part,
        }
    }

    /// Transforms a point by this dual quaternion.
    ///
    /// Applies both rotation and translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{DualQuatd, Quatd, Vec3d};
    /// use std::f64::consts::PI;
    ///
    /// // 90 degree rotation around Z + translation (1, 0, 0)
    /// let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let translation = Vec3d::new(1.0, 0.0, 0.0);
    /// let dq = DualQuatd::from_rotation_translation(&rotation, &translation);
    ///
    /// // Transform point (1, 0, 0)
    /// let p = Vec3d::new(1.0, 0.0, 0.0);
    /// let result = dq.transform(&p);
    ///
    /// // Rotation: (1,0,0) -> (0,1,0), then +translation -> (1,1,0)
    /// assert!((result.x - 1.0).abs() < 1e-10);
    /// assert!((result.y - 1.0).abs() < 1e-10);
    /// assert!(result.z.abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn transform(&self, point: &Vec3<T>) -> Vec3<T> {
        // Apply rotation and add translation
        let rotated = self.real.transform(point);
        let t = self.translation();
        Vec3 {
            x: rotated.x + t.x,
            y: rotated.y + t.y,
            z: rotated.z + t.z,
        }
    }
}

// Default - identity dual quaternion
impl<T: Scalar> Default for DualQuat<T> {
    fn default() -> Self {
        Self::identity()
    }
}

// PartialEq - component-wise comparison
impl<T: Scalar + PartialEq> PartialEq for DualQuat<T> {
    fn eq(&self, other: &Self) -> bool {
        self.real == other.real && self.dual == other.dual
    }
}

// Hash
impl<T: Scalar> Hash for DualQuat<T>
where
    T: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.real.hash(state);
        self.dual.hash(state);
    }
}

// Negation
impl<T: Scalar + Neg<Output = T>> Neg for DualQuat<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            real: -self.real,
            dual: -self.dual,
        }
    }
}

// Addition
impl<T: Scalar> Add for DualQuat<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real + rhs.real,
            dual: self.dual + rhs.dual,
        }
    }
}

impl<T: Scalar> AddAssign for DualQuat<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.real = self.real + rhs.real;
        self.dual = self.dual + rhs.dual;
    }
}

// Subtraction
impl<T: Scalar> Sub for DualQuat<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real - rhs.real,
            dual: self.dual - rhs.dual,
        }
    }
}

impl<T: Scalar> SubAssign for DualQuat<T> {
    fn sub_assign(&mut self, rhs: Self) {
        self.real = self.real - rhs.real;
        self.dual = self.dual - rhs.dual;
    }
}

// Dual quaternion multiplication
// (r1 + εd1) * (r2 + εd2) = r1*r2 + ε(r1*d2 + d1*r2)
impl<T: Scalar + Float> Mul for DualQuat<T> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            real: self.real * rhs.real,
            dual: self.real * rhs.dual + self.dual * rhs.real,
        }
    }
}

impl<T: Scalar + Float> MulAssign for DualQuat<T> {
    fn mul_assign(&mut self, rhs: Self) {
        let new_real = self.real * rhs.real;
        let new_dual = self.real * rhs.dual + self.dual * rhs.real;
        self.real = new_real;
        self.dual = new_dual;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for DualQuat<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self {
            real: self.real * rhs,
            dual: self.dual * rhs,
        }
    }
}

impl<T: Scalar> MulAssign<T> for DualQuat<T> {
    fn mul_assign(&mut self, rhs: T) {
        self.real = self.real * rhs;
        self.dual = self.dual * rhs;
    }
}

// Scalar division
impl<T: Scalar + Float> Div<T> for DualQuat<T> {
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        let inv = T::ONE / rhs;
        Self {
            real: self.real * inv,
            dual: self.dual * inv,
        }
    }
}

impl<T: Scalar + Float> DivAssign<T> for DualQuat<T> {
    fn div_assign(&mut self, rhs: T) {
        let inv = T::ONE / rhs;
        self.real *= inv;
        self.dual *= inv;
    }
}

// Display - per OpenUSD dualQuat.template.cpp: (Gf_OstreamHelperP(real), Gf_OstreamHelperP(dual))

impl fmt::Display for DualQuatd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.real();
        let d = self.dual();
        write!(
            f,
            "(({}, {}, {}, {}), ({}, {}, {}, {}))",
            ostream_helper_p_double(r.real()),
            ostream_helper_p_double(r.imaginary().x),
            ostream_helper_p_double(r.imaginary().y),
            ostream_helper_p_double(r.imaginary().z),
            ostream_helper_p_double(d.real()),
            ostream_helper_p_double(d.imaginary().x),
            ostream_helper_p_double(d.imaginary().y),
            ostream_helper_p_double(d.imaginary().z),
        )
    }
}

impl fmt::Display for DualQuatf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.real();
        let d = self.dual();
        write!(
            f,
            "(({}, {}, {}, {}), ({}, {}, {}, {}))",
            ostream_helper_p_float(r.real()),
            ostream_helper_p_float(r.imaginary().x),
            ostream_helper_p_float(r.imaginary().y),
            ostream_helper_p_float(r.imaginary().z),
            ostream_helper_p_float(d.real()),
            ostream_helper_p_float(d.imaginary().x),
            ostream_helper_p_float(d.imaginary().y),
            ostream_helper_p_float(d.imaginary().z),
        )
    }
}

impl fmt::Display for DualQuath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.real, self.dual)
    }
}

/// Returns the dot product of two dual quaternions.
///
/// This is the sum of dots of real and dual parts.
/// Matches C++ `GfDot` overload for dual quaternions.
#[doc(alias = "GfDot")]
#[inline]
#[must_use]
pub fn dual_quat_dot<T: Scalar + Float>(a: &DualQuat<T>, b: &DualQuat<T>) -> T {
    dot(&a.real, &b.real) + dot(&a.dual, &b.dual)
}

// Cross-precision conversions (matching C++ implicit/explicit conversions)

impl From<DualQuatf> for DualQuatd {
    /// Implicitly convert from DualQuatf to DualQuatd (widening).
    fn from(other: DualQuatf) -> Self {
        Self {
            real: Quat::new(
                other.real.real() as f64,
                Vec3 {
                    x: other.real.imaginary().x as f64,
                    y: other.real.imaginary().y as f64,
                    z: other.real.imaginary().z as f64,
                },
            ),
            dual: Quat::new(
                other.dual.real() as f64,
                Vec3 {
                    x: other.dual.imaginary().x as f64,
                    y: other.dual.imaginary().y as f64,
                    z: other.dual.imaginary().z as f64,
                },
            ),
        }
    }
}

impl From<DualQuath> for DualQuatd {
    fn from(other: DualQuath) -> Self {
        Self {
            real: Quat::new(
                other.real.real().to_f32() as f64,
                Vec3 {
                    x: other.real.imaginary().x.to_f32() as f64,
                    y: other.real.imaginary().y.to_f32() as f64,
                    z: other.real.imaginary().z.to_f32() as f64,
                },
            ),
            dual: Quat::new(
                other.dual.real().to_f32() as f64,
                Vec3 {
                    x: other.dual.imaginary().x.to_f32() as f64,
                    y: other.dual.imaginary().y.to_f32() as f64,
                    z: other.dual.imaginary().z.to_f32() as f64,
                },
            ),
        }
    }
}

impl From<DualQuatd> for DualQuath {
    fn from(other: DualQuatd) -> Self {
        Self {
            real: Quat::new(
                Half::from_f32(other.real.real() as f32),
                Vec3 {
                    x: Half::from_f32(other.real.imaginary().x as f32),
                    y: Half::from_f32(other.real.imaginary().y as f32),
                    z: Half::from_f32(other.real.imaginary().z as f32),
                },
            ),
            dual: Quat::new(
                Half::from_f32(other.dual.real() as f32),
                Vec3 {
                    x: Half::from_f32(other.dual.imaginary().x as f32),
                    y: Half::from_f32(other.dual.imaginary().y as f32),
                    z: Half::from_f32(other.dual.imaginary().z as f32),
                },
            ),
        }
    }
}

impl From<DualQuatd> for DualQuatf {
    /// Explicitly convert from DualQuatd to DualQuatf (narrowing, may lose precision).
    fn from(other: DualQuatd) -> Self {
        Self {
            real: Quat::new(
                other.real.real() as f32,
                Vec3 {
                    x: other.real.imaginary().x as f32,
                    y: other.real.imaginary().y as f32,
                    z: other.real.imaginary().z as f32,
                },
            ),
            dual: Quat::new(
                other.dual.real() as f32,
                Vec3 {
                    x: other.dual.imaginary().x as f32,
                    y: other.dual.imaginary().y as f32,
                    z: other.dual.imaginary().z as f32,
                },
            ),
        }
    }
}

/// Creates a DualQuatd from rotation and translation.
#[inline]
#[must_use]
pub fn dual_quatd(rotation: &Quat<f64>, translation: &Vec3<f64>) -> DualQuatd {
    DualQuatd::from_rotation_translation(rotation, translation)
}

/// Creates a DualQuatf from rotation and translation.
#[inline]
#[must_use]
pub fn dual_quatf(rotation: &Quat<f32>, translation: &Vec3<f32>) -> DualQuatf {
    DualQuatf::from_rotation_translation(rotation, translation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quat::Quatd;
    use crate::vec3::Vec3d;
    use std::f64::consts::PI;

    #[test]
    fn test_identity() {
        let dq = DualQuatd::identity();
        assert_eq!(dq.real().real(), 1.0);
        assert_eq!(dq.dual().real(), 0.0);
    }

    #[test]
    fn test_zero() {
        let dq = DualQuatd::zero();
        assert_eq!(dq.real().real(), 0.0);
        assert_eq!(dq.dual().real(), 0.0);
    }

    #[test]
    fn test_from_rotation_translation() {
        let rotation = Quatd::identity();
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let t = dq.translation();
        assert!((t.x - 1.0).abs() < 1e-10);
        assert!((t.y - 2.0).abs() < 1e-10);
        assert!((t.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_translation_only() {
        let rotation = Quatd::identity();
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let p = Vec3d::new(0.0, 0.0, 0.0);
        let result = dq.transform(&p);

        assert!((result.x - 1.0).abs() < 1e-10);
        assert!((result.y - 2.0).abs() < 1e-10);
        assert!((result.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_rotation_only() {
        // 90 degree rotation around Z axis
        let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
        let translation = Vec3d::new(0.0, 0.0, 0.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let p = Vec3d::new(1.0, 0.0, 0.0);
        let result = dq.transform(&p);

        // (1,0,0) -> (0,1,0) after 90 deg rotation around Z
        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
        assert!(result.z.abs() < 1e-10);
    }

    #[test]
    fn test_transform_rotation_and_translation() {
        // 90 degree rotation around Z + translation (1, 0, 0)
        let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
        let translation = Vec3d::new(1.0, 0.0, 0.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let p = Vec3d::new(1.0, 0.0, 0.0);
        let result = dq.transform(&p);

        // Rotation: (1,0,0) -> (0,1,0), then +translation -> (1,1,0)
        assert!((result.x - 1.0).abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
        assert!(result.z.abs() < 1e-10);
    }

    #[test]
    fn test_conjugate() {
        let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let conj = dq.conjugate();
        assert_eq!(conj.real().real(), dq.real().real());
        assert_eq!(conj.real().imaginary().x, -dq.real().imaginary().x);
    }

    #[test]
    fn test_normalize() {
        let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        // Scale by 2
        let scaled = dq * 2.0;
        let normalized = scaled.normalized();

        // Real part should have unit length
        assert!((normalized.real().length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_length() {
        let dq = DualQuatd::identity();
        let (real_len, _dual_comp) = dq.length();
        assert!((real_len - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_addition() {
        let dq1 = DualQuatd::identity();
        let dq2 = DualQuatd::identity();
        let sum = dq1 + dq2;

        assert!((sum.real().real() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_subtraction() {
        let dq1 = DualQuatd::identity();
        let dq2 = DualQuatd::identity();
        let diff = dq1 - dq2;

        assert!(diff.real().real().abs() < 1e-10);
    }

    #[test]
    fn test_scalar_multiplication() {
        let dq = DualQuatd::identity();
        let scaled = dq * 2.0;

        assert!((scaled.real().real() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_scalar_division() {
        let dq = DualQuatd::identity() * 2.0;
        let divided = dq / 2.0;

        assert!((divided.real().real() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_dual_quat_multiplication() {
        // Two translations should compose
        let t1 = Vec3d::new(1.0, 0.0, 0.0);
        let t2 = Vec3d::new(0.0, 1.0, 0.0);
        let dq1 = DualQuatd::from_rotation_translation(&Quatd::identity(), &t1);
        let dq2 = DualQuatd::from_rotation_translation(&Quatd::identity(), &t2);

        let composed = dq1 * dq2;
        let total_t = composed.translation();

        assert!((total_t.x - 1.0).abs() < 1e-10);
        assert!((total_t.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_identity() {
        let dq = DualQuatd::identity();
        let inv = dq.inverse();

        assert!((inv.real().real() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let rotation = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = DualQuatd::from_rotation_translation(&rotation, &translation);

        let inv = dq.inverse();
        let product = dq * inv;

        // Should be close to identity
        assert!((product.real().real() - 1.0).abs() < 1e-10);
        assert!(product.real().imaginary().x.abs() < 1e-10);
    }

    #[test]
    fn test_display() {
        let dq = DualQuatd::identity();
        let s = format!("{}", dq);
        assert!(s.contains("1"));
    }

    #[test]
    fn test_dual_quat_dot() {
        let dq1 = DualQuatd::identity();
        let dq2 = DualQuatd::identity();
        let d = dual_quat_dot(&dq1, &dq2);
        assert!((d - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_helper_functions() {
        let rotation = Quatd::identity();
        let translation = Vec3d::new(1.0, 2.0, 3.0);
        let dq = dual_quatd(&rotation, &translation);

        let t = dq.translation();
        assert!((t.x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_default() {
        let dq: DualQuatd = Default::default();
        assert!((dq.real().real() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_negation() {
        let dq = DualQuatd::identity();
        let neg = -dq;
        assert!((neg.real().real() - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_from_real() {
        let q = Quatd::from_axis_angle(Vec3d::new(0.0, 1.0, 0.0), PI / 4.0);
        let dq = DualQuatd::from_real(q);

        assert_eq!(dq.real().real(), q.real());
        assert_eq!(dq.dual().real(), 0.0);
    }

    #[test]
    fn test_from_scalar() {
        let dq = DualQuatd::from_scalar(1.0);
        assert!((dq.real().real() - 1.0).abs() < 1e-10);
        assert!(dq.real().imaginary().x.abs() < 1e-10);
    }
}
