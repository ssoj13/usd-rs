//! 3D vector types.
//!
//! This module provides Vec3 types for 3D vector math operations.
//! Vec3 is generic over the scalar type, with type aliases for common types.
//!
//! # Examples
//!
//! ```
//! use usd_gf::vec3::{Vec3d, Vec3f, Vec3i};
//!
//! // Create vectors
//! let v1 = Vec3d::new(1.0, 2.0, 3.0);
//! let v2 = Vec3d::new(4.0, 5.0, 6.0);
//!
//! // Arithmetic
//! let sum = v1 + v2;
//! let scaled = v1 * 2.0;
//! let dot = v1.dot(&v2);
//!
//! // Cross product (3D specific)
//! let cross = v1.cross(&v2);
//!
//! // Normalize
//! let normalized = v1.normalized();
//! ```

use crate::limits::{MIN_ORTHO_TOLERANCE, MIN_VECTOR_LENGTH};
use crate::traits::{GfVec, Scalar};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{
    Add, AddAssign, BitXor, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// A 3D vector with scalar type `T`.
///
/// Vec3 supports standard arithmetic operations, dot product, cross product,
/// length, normalization, and projection operations.
///
/// # Examples
///
/// ```
/// use usd_gf::vec3::Vec3d;
///
/// let v = Vec3d::new(1.0, 2.0, 2.0);
/// assert!((v.length() - 3.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vec3<T> {
    /// X component.
    pub x: T,
    /// Y component.
    pub y: T,
    /// Z component.
    pub z: T,
}

// Mark Vec3 as a gf vector type
impl<T> GfVec for Vec3<T> {}

/// Type alias for 3D double-precision vector.
pub type Vec3d = Vec3<f64>;

/// Type alias for 3D single-precision vector.
pub type Vec3f = Vec3<f32>;

/// Type alias for 3D half-precision vector.
pub type Vec3h = Vec3<crate::half::Half>;

/// Type alias for 3D integer vector.
pub type Vec3i = Vec3<i32>;

impl<T: Scalar> Vec3<T> {
    /// Dimension of the vector.
    pub const DIMENSION: usize = 3;

    /// Creates a new vector with the given components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::new(1.0, 2.0, 3.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(x: T, y: T, z: T) -> Self {
        Self { x, y, z }
    }

    /// Creates a vector with all components set to the same value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::splat(5.0);
    /// assert_eq!(v.x, 5.0);
    /// assert_eq!(v.y, 5.0);
    /// assert_eq!(v.z, 5.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn splat(value: T) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }

    /// Creates a zero vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self::splat(T::ZERO)
    }

    /// Creates a unit vector along the X-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::x_axis();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn x_axis() -> Self {
        Self {
            x: T::ONE,
            y: T::ZERO,
            z: T::ZERO,
        }
    }

    /// Creates a unit vector along the Y-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::y_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 1.0);
    /// assert_eq!(v.z, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn y_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ONE,
            z: T::ZERO,
        }
    }

    /// Creates a unit vector along the Z-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::z_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn z_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ONE,
        }
    }

    /// Creates a unit vector along the i-th axis (0=X, 1=Y, 2=Z).
    ///
    /// Returns the zero vector if `i >= 3`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// assert_eq!(Vec3d::axis(0), Vec3d::x_axis());
    /// assert_eq!(Vec3d::axis(1), Vec3d::y_axis());
    /// assert_eq!(Vec3d::axis(2), Vec3d::z_axis());
    /// assert_eq!(Vec3d::axis(3), Vec3d::zero());
    /// ```
    #[inline]
    #[must_use]
    pub fn axis(i: usize) -> Self {
        match i {
            0 => Self::x_axis(),
            1 => Self::y_axis(),
            2 => Self::z_axis(),
            _ => Self::zero(),
        }
    }

    /// Sets all components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let mut v = Vec3d::zero();
    /// v.set(1.0, 2.0, 3.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// ```
    #[inline]
    pub fn set(&mut self, x: T, y: T, z: T) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Returns a pointer to the underlying data.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        &self.x as *const T
    }

    /// Returns a mutable pointer to the underlying data.
    #[inline]
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        &mut self.x as *mut T
    }

    /// Returns the data as a slice.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: Vec3 is repr(C) with three contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 3) }
    }

    /// Returns the data as a mutable slice.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: Vec3 is repr(C) with three contiguous T values
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), 3) }
    }

    /// Converts to an array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [T; 3] {
        [self.x, self.y, self.z]
    }

    /// Creates from an array.
    #[inline]
    #[must_use]
    pub fn from_array(arr: [T; 3]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
        }
    }

    /// Creates from a pointer to values.
    ///
    /// Matches C++ `GfVec3d(Scl const *p)` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 3 valid `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const T) -> Self {
        unsafe {
            Self {
                x: *ptr,
                y: *ptr.add(1),
                z: *ptr.add(2),
            }
        }
    }

    /// Sets all elements from a pointer to values.
    ///
    /// Matches C++ `Set(double const *a)` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 3 valid `T` values.
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const T) {
        unsafe {
            self.x = *ptr;
            self.y = *ptr.add(1);
            self.z = *ptr.add(2);
        }
    }

    /// Returns the dot product of two vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v1 = Vec3d::new(1.0, 2.0, 3.0);
    /// let v2 = Vec3d::new(4.0, 5.0, 6.0);
    /// assert!((v1.dot(&v2) - 32.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> T {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Returns the cross product of two vectors (3D specific).
    ///
    /// The cross product returns a vector perpendicular to both input vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let x = Vec3d::x_axis();
    /// let y = Vec3d::y_axis();
    /// let z = x.cross(&y);
    /// assert!((z.x).abs() < 1e-10);
    /// assert!((z.y).abs() < 1e-10);
    /// assert!((z.z - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn cross(&self, other: &Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Returns the squared length of the vector.
    ///
    /// This is more efficient than `length()` when only comparing magnitudes.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::new(1.0, 2.0, 2.0);
    /// assert!((v.length_squared() - 9.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn length_squared(&self) -> T {
        self.dot(self)
    }

    /// Returns the length (magnitude) of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::new(1.0, 2.0, 2.0);
    /// assert!((v.length() - 3.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn length(&self) -> T {
        self.length_squared().sqrt()
    }

    /// Returns a normalized (unit length) copy of the vector.
    ///
    /// If the vector length is smaller than `eps`, returns a vector
    /// divided by `eps` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::new(1.0, 2.0, 2.0);
    /// let n = v.normalized();
    /// assert!((n.length() - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn normalized(&self) -> Self {
        self.normalized_with_eps(T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON))
    }

    /// Returns a normalized copy with custom epsilon.
    #[inline]
    #[must_use]
    pub fn normalized_with_eps(&self, eps: T) -> Self {
        let len = self.length();
        let divisor = if len > eps { len } else { eps };
        *self / divisor
    }

    /// Normalizes the vector in place, returning the original length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let mut v = Vec3d::new(1.0, 2.0, 2.0);
    /// let original_length = v.normalize();
    /// assert!((original_length - 3.0).abs() < 1e-10);
    /// assert!((v.length() - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    pub fn normalize(&mut self) -> T {
        self.normalize_with_eps(T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON))
    }

    /// Normalizes in place with custom epsilon, returning the original length.
    #[inline]
    pub fn normalize_with_eps(&mut self, eps: T) -> T {
        let len = self.length();
        let divisor = if len > eps { len } else { eps };
        *self /= divisor;
        len
    }

    /// Returns the projection of this vector onto `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v = Vec3d::new(3.0, 4.0, 5.0);
    /// let axis = Vec3d::x_axis();
    /// let proj = v.projection(&axis);
    /// assert!((proj.x - 3.0).abs() < 1e-10);
    /// assert!(proj.y.abs() < 1e-10);
    /// assert!(proj.z.abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn projection(&self, other: &Self) -> Self {
        *other * self.dot(other)
    }

    /// Returns the component of this vector orthogonal to `other`.
    ///
    /// Equivalent to `self - self.projection(other)`.
    #[inline]
    #[must_use]
    pub fn complement(&self, other: &Self) -> Self {
        *self - self.projection(other)
    }

    /// Component-wise multiplication.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v1 = Vec3d::new(2.0, 3.0, 4.0);
    /// let v2 = Vec3d::new(5.0, 6.0, 7.0);
    /// let result = v1.comp_mult(&v2);
    /// assert!((result.x - 10.0).abs() < 1e-10);
    /// assert!((result.y - 18.0).abs() < 1e-10);
    /// assert!((result.z - 28.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn comp_mult(&self, other: &Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }

    /// Component-wise division.
    #[inline]
    #[must_use]
    pub fn comp_div(&self, other: &Self) -> Self {
        Self::new(self.x / other.x, self.y / other.y, self.z / other.z)
    }

    /// Returns whether this vector is approximately equal to another.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v1 = Vec3d::new(1.0, 2.0, 3.0);
    /// let v2 = Vec3d::new(1.0 + 1e-10, 2.0, 3.0);
    /// assert!(v1.is_close(&v2, 1e-9));
    /// assert!(!v1.is_close(&v2, 1e-11));
    /// ```
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        (self.x - other.x).abs() <= eps
            && (self.y - other.y).abs() <= eps
            && (self.z - other.z).abs() <= eps
    }

    /// Returns the component-wise minimum.
    #[inline]
    #[must_use]
    pub fn min(&self, other: &Self) -> Self {
        Self::new(
            if self.x < other.x { self.x } else { other.x },
            if self.y < other.y { self.y } else { other.y },
            if self.z < other.z { self.z } else { other.z },
        )
    }

    /// Returns the component-wise maximum.
    #[inline]
    #[must_use]
    pub fn max(&self, other: &Self) -> Self {
        Self::new(
            if self.x > other.x { self.x } else { other.x },
            if self.y > other.y { self.y } else { other.y },
            if self.z > other.z { self.z } else { other.z },
        )
    }

    /// Returns the component-wise absolute value.
    #[inline]
    #[must_use]
    pub fn abs(&self) -> Self {
        Self::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    /// Returns the component-wise floor.
    #[inline]
    #[must_use]
    pub fn floor(&self) -> Self {
        Self::new(self.x.floor(), self.y.floor(), self.z.floor())
    }

    /// Returns the component-wise ceiling.
    #[inline]
    #[must_use]
    pub fn ceil(&self) -> Self {
        Self::new(self.x.ceil(), self.y.ceil(), self.z.ceil())
    }

    /// Returns the component-wise round.
    #[inline]
    #[must_use]
    pub fn round(&self) -> Self {
        Self::new(self.x.round(), self.y.round(), self.z.round())
    }

    /// Spherical linear interpolation between two vectors.
    ///
    /// Returns a vector that is interpolated between `self` and `other`
    /// along the great circle arc on the unit sphere.
    ///
    /// # Arguments
    ///
    /// * `other` - The target vector
    /// * `alpha` - Interpolation factor (0.0 = self, 1.0 = other)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v0 = Vec3d::x_axis();
    /// let v1 = Vec3d::y_axis();
    /// let mid = v0.slerp(&v1, 0.5);
    /// // mid is approximately (0.707, 0.707, 0)
    /// assert!((mid.length() - 1.0).abs() < 1e-6);
    /// ```
    #[inline]
    #[must_use]
    pub fn slerp(&self, other: &Self, alpha: T) -> Self {
        // Handle edge cases
        if alpha <= T::ZERO {
            return *self;
        }
        if alpha >= T::ONE {
            return *other;
        }

        // Compute angle between vectors
        let cos_theta = self.dot(other);

        // If vectors are nearly parallel, use linear interpolation
        let eps = T::from(1e-6).unwrap_or(T::EPSILON);
        if cos_theta > T::ONE - eps {
            return *self * (T::ONE - alpha) + *other * alpha;
        }

        // Clamp cos_theta to valid range
        let cos_theta = if cos_theta < -T::ONE {
            -T::ONE
        } else if cos_theta > T::ONE {
            T::ONE
        } else {
            cos_theta
        };

        let theta = cos_theta.acos();
        let sin_theta = theta.sin();

        if sin_theta.abs() < eps {
            // Vectors are nearly opposite, use linear interpolation
            return *self * (T::ONE - alpha) + *other * alpha;
        }

        let scale_self = ((T::ONE - alpha) * theta).sin() / sin_theta;
        let scale_other = (alpha * theta).sin() / sin_theta;

        *self * scale_self + *other * scale_other
    }

    /// Orthogonalize a set of basis vectors using iterative Gram-Schmidt.
    ///
    /// This uses an iterative method that is very stable even when the vectors
    /// are far from orthogonal (close to colinear). Returns whether the solution
    /// converged. Colinear vectors will be unaltered, and the method returns false.
    ///
    /// # Arguments
    ///
    /// * `v0` - First basis vector (modified in place)
    /// * `v1` - Second basis vector (modified in place)
    /// * `v2` - Third basis vector (modified in place)
    /// * `normalize` - Whether to normalize the result to unit length
    /// * `eps` - Tolerance for orthogonality checks
    ///
    /// # Returns
    ///
    /// `true` if orthogonalization converged, `false` otherwise.
    pub fn orthogonalize_basis(
        v0: &mut Self,
        v1: &mut Self,
        v2: &mut Self,
        normalize: bool,
        eps: T,
    ) -> bool {
        let max_iterations = 10;
        let mut converged = false;

        for _ in 0..max_iterations {
            // Project v1 onto v0 and subtract
            let proj_10 = v0.dot(v1);
            *v1 -= *v0 * proj_10;

            // Project v2 onto v0 and v1 and subtract
            let proj_20 = v0.dot(v2);
            let proj_21 = v1.dot(v2);
            *v2 = *v2 - *v0 * proj_20 - *v1 * proj_21;

            // Check for convergence
            let dot01 = v0.dot(v1).abs();
            let dot02 = v0.dot(v2).abs();
            let dot12 = v1.dot(v2).abs();

            if dot01 < eps && dot02 < eps && dot12 < eps {
                converged = true;
                break;
            }
        }

        if normalize {
            let len0 = v0.length();
            let len1 = v1.length();
            let len2 = v2.length();

            let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);

            if len0 > min_len {
                *v0 /= len0;
            }
            if len1 > min_len {
                *v1 /= len1;
            }
            if len2 > min_len {
                *v2 /= len2;
            }
        }

        converged
    }

    /// Build an orthonormal frame from this vector.
    ///
    /// Sets `v1` and `v2` to unit vectors such that v1, v2 and `*self` are
    /// mutually orthogonal. If the length L of `*self` is smaller than `eps`,
    /// then v1 and v2 will have magnitude L/eps.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let v0 = Vec3d::new(0.0, 0.0, 1.0);
    /// let (v1, v2) = v0.build_orthonormal_frame();
    /// // v0, v1, v2 are now mutually orthogonal
    /// assert!(v0.dot(&v1).abs() < 1e-10);
    /// assert!(v0.dot(&v2).abs() < 1e-10);
    /// assert!(v1.dot(&v2).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn build_orthonormal_frame(&self) -> (Self, Self) {
        self.build_orthonormal_frame_with_eps(T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON))
    }

    /// Build an orthonormal frame with custom epsilon.
    #[must_use]
    pub fn build_orthonormal_frame_with_eps(&self, eps: T) -> (Self, Self) {
        let length = self.length();

        // If the vector is too small, return default orthogonal vectors
        if length < eps {
            let scale = length / eps;
            return (Self::x_axis() * scale, Self::y_axis() * scale);
        }

        // Normalize the input vector
        let n = *self / length;

        // Choose a vector not parallel to n to compute the cross product
        // Use x-axis unless n is nearly parallel to it
        let v1 = if n.x.abs() < T::from(0.9).unwrap_or(T::ONE) {
            n.cross(&Self::x_axis()).normalized()
        } else {
            n.cross(&Self::y_axis()).normalized()
        };

        // v2 is perpendicular to both n and v1
        let v2 = n.cross(&v1);

        (v1, v2)
    }
}

// Default - zero vector
impl<T: Scalar> Default for Vec3<T> {
    fn default() -> Self {
        Self::zero()
    }
}

// From array
impl<T: Scalar> From<[T; 3]> for Vec3<T> {
    fn from(arr: [T; 3]) -> Self {
        Self::from_array(arr)
    }
}

// Into array
impl<T: Scalar> From<Vec3<T>> for [T; 3] {
    fn from(v: Vec3<T>) -> Self {
        v.to_array()
    }
}

// From tuple
impl<T: Scalar> From<(T, T, T)> for Vec3<T> {
    fn from((x, y, z): (T, T, T)) -> Self {
        Self::new(x, y, z)
    }
}

// Indexing
impl<T> Index<usize> for Vec3<T> {
    type Output = T;

    #[inline]
    fn index(&self, i: usize) -> &T {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 index out of bounds: {}", i),
        }
    }
}

impl<T> IndexMut<usize> for Vec3<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut T {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("Vec3 index out of bounds: {}", i),
        }
    }
}

// Equality
impl<T: PartialEq> PartialEq for Vec3<T> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl<T: Eq> Eq for Vec3<T> {}

// Hash
impl<T: Hash> Hash for Vec3<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

// Negation
impl<T: Scalar> Neg for Vec3<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

// Addition
impl<T: Scalar> Add for Vec3<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl<T: Scalar> AddAssign for Vec3<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
        self.z = self.z + rhs.z;
    }
}

// Subtraction
impl<T: Scalar> Sub for Vec3<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<T: Scalar> SubAssign for Vec3<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
        self.z = self.z - rhs.z;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Vec3<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl<T: Scalar> MulAssign<T> for Vec3<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.x = self.x * rhs;
        self.y = self.y * rhs;
        self.z = self.z * rhs;
    }
}

// Scalar multiplication (scalar on left) - requires specific implementations
impl Mul<Vec3<f64>> for f64 {
    type Output = Vec3<f64>;

    #[inline]
    fn mul(self, rhs: Vec3<f64>) -> Vec3<f64> {
        rhs * self
    }
}

impl Mul<Vec3<f32>> for f32 {
    type Output = Vec3<f32>;

    #[inline]
    fn mul(self, rhs: Vec3<f32>) -> Vec3<f32> {
        rhs * self
    }
}

// Scalar division
impl<T: Scalar> Div<T> for Vec3<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: T) -> Self {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl<T: Scalar> DivAssign<T> for Vec3<T> {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        self.x = self.x / rhs;
        self.y = self.y / rhs;
        self.z = self.z / rhs;
    }
}

// Cross product operator (^) - matches C++ OpenUSD
impl<T: Scalar> BitXor for Vec3<T> {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        self.cross(&rhs)
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Vec3<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

/// Creates a Vec3d from components.
///
/// # Examples
///
/// ```
/// use usd_gf::vec3::vec3d;
///
/// let v = vec3d(1.0, 2.0, 3.0);
/// assert_eq!(v.x, 1.0);
/// assert_eq!(v.y, 2.0);
/// assert_eq!(v.z, 3.0);
/// ```
#[inline]
#[must_use]
pub fn vec3d(x: f64, y: f64, z: f64) -> Vec3d {
    Vec3d::new(x, y, z)
}

/// Creates a Vec3f from components.
#[inline]
#[must_use]
pub fn vec3f(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f::new(x, y, z)
}

/// Creates a Vec3i from components.
#[inline]
#[must_use]
pub fn vec3i(x: i32, y: i32, z: i32) -> Vec3i {
    Vec3i { x, y, z }
}

// Vec3i needs special handling since i32 doesn't impl Scalar
impl Vec3<i32> {
    /// Creates a new integer vector.
    #[inline]
    #[must_use]
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Creates a zero vector.
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self { x: 0, y: 0, z: 0 }
    }

    /// Creates a vector with all components set to the same value.
    #[inline]
    #[must_use]
    pub fn splat(value: i32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> i32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product for integer vectors.
    #[inline]
    #[must_use]
    pub fn cross(&self, other: &Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Squared length.
    #[inline]
    #[must_use]
    pub fn length_squared(&self) -> i32 {
        self.dot(self)
    }
}

impl Default for Vec3<i32> {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for Vec3<i32> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3<i32> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<i32> for Vec3<i32> {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl Neg for Vec3<i32> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// Free function: returns the cross product of two vectors.
///
/// # Examples
///
/// ```
/// use usd_gf::vec3::{Vec3d, cross};
///
/// let v1 = Vec3d::x_axis();
/// let v2 = Vec3d::y_axis();
/// let v3 = cross(&v1, &v2);
/// assert!((v3.z - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn cross<T: Scalar>(v1: &Vec3<T>, v2: &Vec3<T>) -> Vec3<T> {
    v1.cross(v2)
}

/// Free function: returns the dot product of two vectors.
#[inline]
#[must_use]
pub fn dot<T: Scalar>(v1: &Vec3<T>, v2: &Vec3<T>) -> T {
    v1.dot(v2)
}

/// Free function: returns the length of a vector.
#[inline]
#[must_use]
pub fn length<T: Scalar>(v: &Vec3<T>) -> T {
    v.length()
}

/// Free function: returns a normalized vector.
#[inline]
#[must_use]
pub fn normalized<T: Scalar>(v: &Vec3<T>) -> Vec3<T> {
    v.normalized()
}

/// Free function: spherical linear interpolation.
#[inline]
#[must_use]
pub fn slerp<T: Scalar>(v0: &Vec3<T>, v1: &Vec3<T>, alpha: T) -> Vec3<T> {
    v0.slerp(v1, alpha)
}

/// Free function: orthogonalize basis vectors.
pub fn orthogonalize_basis<T: Scalar>(
    v0: &mut Vec3<T>,
    v1: &mut Vec3<T>,
    v2: &mut Vec3<T>,
    normalize: bool,
) -> bool {
    Vec3::orthogonalize_basis(
        v0,
        v1,
        v2,
        normalize,
        T::from(MIN_ORTHO_TOLERANCE).unwrap_or(T::EPSILON),
    )
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Vec3f> for Vec3d {
    fn from(other: Vec3f) -> Self {
        Self::new(other.x as f64, other.y as f64, other.z as f64)
    }
}

impl From<Vec3h> for Vec3d {
    fn from(other: Vec3h) -> Self {
        Self::new(other.x.to_f64(), other.y.to_f64(), other.z.to_f64())
    }
}

impl From<Vec3i> for Vec3d {
    fn from(other: Vec3i) -> Self {
        Self::new(other.x as f64, other.y as f64, other.z as f64)
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads).
// C++ uses implicit promotion and exact ==; we match that behavior.

// -- Vec3d vs others --
impl PartialEq<Vec3f> for Vec3d {
    fn eq(&self, other: &Vec3f) -> bool {
        self.x == other.x as f64 && self.y == other.y as f64 && self.z == other.z as f64
    }
}
impl PartialEq<Vec3h> for Vec3d {
    fn eq(&self, other: &Vec3h) -> bool {
        self.x == other.x.to_f64() && self.y == other.y.to_f64() && self.z == other.z.to_f64()
    }
}
impl PartialEq<Vec3i> for Vec3d {
    fn eq(&self, other: &Vec3i) -> bool {
        self.x == other.x as f64 && self.y == other.y as f64 && self.z == other.z as f64
    }
}

// -- Vec3f vs others --
impl PartialEq<Vec3d> for Vec3f {
    fn eq(&self, other: &Vec3d) -> bool {
        self.x as f64 == other.x && self.y as f64 == other.y && self.z as f64 == other.z
    }
}
impl PartialEq<Vec3h> for Vec3f {
    fn eq(&self, other: &Vec3h) -> bool {
        self.x == other.x.to_f32() && self.y == other.y.to_f32() && self.z == other.z.to_f32()
    }
}
impl PartialEq<Vec3i> for Vec3f {
    fn eq(&self, other: &Vec3i) -> bool {
        self.x == other.x as f32 && self.y == other.y as f32 && self.z == other.z as f32
    }
}

// -- Vec3h vs others --
impl PartialEq<Vec3d> for Vec3h {
    fn eq(&self, other: &Vec3d) -> bool {
        self.x.to_f64() == other.x && self.y.to_f64() == other.y && self.z.to_f64() == other.z
    }
}
impl PartialEq<Vec3f> for Vec3h {
    fn eq(&self, other: &Vec3f) -> bool {
        self.x.to_f32() == other.x && self.y.to_f32() == other.y && self.z.to_f32() == other.z
    }
}
impl PartialEq<Vec3i> for Vec3h {
    fn eq(&self, other: &Vec3i) -> bool {
        self.x.to_f64() == other.x as f64
            && self.y.to_f64() == other.y as f64
            && self.z.to_f64() == other.z as f64
    }
}

// -- Vec3i vs others --
impl PartialEq<Vec3d> for Vec3i {
    fn eq(&self, other: &Vec3d) -> bool {
        self.x as f64 == other.x && self.y as f64 == other.y && self.z as f64 == other.z
    }
}
impl PartialEq<Vec3f> for Vec3i {
    fn eq(&self, other: &Vec3f) -> bool {
        self.x as f32 == other.x && self.y as f32 == other.y && self.z as f32 == other.z
    }
}
impl PartialEq<Vec3h> for Vec3i {
    fn eq(&self, other: &Vec3h) -> bool {
        self.x as f64 == other.x.to_f64()
            && self.y as f64 == other.y.to_f64()
            && self.z as f64 == other.z.to_f64()
    }
}

// Additional global functions (matching C++ Gf* functions)
/// Returns component-wise multiplication of vectors.
///
/// Matches C++ `GfCompMult(GfVec3d const &v1, GfVec3d const &v2)`.
#[inline]
#[must_use]
pub fn comp_mult<T: Scalar>(v1: &Vec3<T>, v2: &Vec3<T>) -> Vec3<T> {
    v1.comp_mult(v2)
}

/// Returns component-wise quotient of vectors.
///
/// Matches C++ `GfCompDiv(GfVec3d const &v1, GfVec3d const &v2)`.
#[inline]
#[must_use]
pub fn comp_div<T: Scalar>(v1: &Vec3<T>, v2: &Vec3<T>) -> Vec3<T> {
    v1.comp_div(v2)
}

/// Normalizes a vector in place to unit length, returning the length before normalization.
///
/// Matches C++ `GfNormalize(GfVec3d *v, double eps)`.
#[inline]
pub fn normalize<T: Scalar>(v: &mut Vec3<T>) -> T {
    v.normalize()
}

/// Returns the projection of `a` onto `b`.
///
/// Matches C++ `GfGetProjection(GfVec3d const &a, GfVec3d const &b)`.
#[inline]
#[must_use]
pub fn projection<T: Scalar>(a: &Vec3<T>, b: &Vec3<T>) -> Vec3<T> {
    a.projection(b)
}

/// Returns the orthogonal complement of `a.projection(b)`.
///
/// Matches C++ `GfGetComplement(GfVec3d const &a, GfVec3d const &b)`.
#[inline]
#[must_use]
pub fn complement<T: Scalar>(a: &Vec3<T>, b: &Vec3<T>) -> Vec3<T> {
    a.complement(b)
}

/// Tests for equality within a given tolerance (component-wise).
///
/// Note: Uses component-wise (Chebyshev/L-inf) comparison, unlike C++
/// `GfIsClose` which uses Euclidean distance (L2 norm).
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(v1: &Vec3<T>, v2: &Vec3<T>, tolerance: T) -> bool {
    v1.is_close(v2, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec3d::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn test_splat() {
        let v = Vec3d::splat(5.0);
        assert_eq!(v.x, 5.0);
        assert_eq!(v.y, 5.0);
        assert_eq!(v.z, 5.0);
    }

    #[test]
    fn test_zero() {
        let v = Vec3d::zero();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn test_axis() {
        assert_eq!(Vec3d::x_axis(), Vec3d::new(1.0, 0.0, 0.0));
        assert_eq!(Vec3d::y_axis(), Vec3d::new(0.0, 1.0, 0.0));
        assert_eq!(Vec3d::z_axis(), Vec3d::new(0.0, 0.0, 1.0));
        assert_eq!(Vec3d::axis(0), Vec3d::x_axis());
        assert_eq!(Vec3d::axis(1), Vec3d::y_axis());
        assert_eq!(Vec3d::axis(2), Vec3d::z_axis());
        assert_eq!(Vec3d::axis(3), Vec3d::zero());
    }

    #[test]
    fn test_indexing() {
        let mut v = Vec3d::new(1.0, 2.0, 3.0);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 2.0);
        assert_eq!(v[2], 3.0);
        v[0] = 4.0;
        assert_eq!(v[0], 4.0);
    }

    #[test]
    fn test_dot() {
        let v1 = Vec3d::new(1.0, 2.0, 3.0);
        let v2 = Vec3d::new(4.0, 5.0, 6.0);
        assert!((v1.dot(&v2) - 32.0).abs() < 1e-10);
    }

    #[test]
    fn test_cross() {
        let x = Vec3d::x_axis();
        let y = Vec3d::y_axis();
        let z = x.cross(&y);
        assert!(z.is_close(&Vec3d::z_axis(), 1e-10));

        // Test cross product properties
        let a = Vec3d::new(1.0, 2.0, 3.0);
        let b = Vec3d::new(4.0, 5.0, 6.0);
        let c = a.cross(&b);

        // Cross product is perpendicular to both inputs
        assert!(c.dot(&a).abs() < 1e-10);
        assert!(c.dot(&b).abs() < 1e-10);

        // Anti-commutativity: a × b = -(b × a)
        let d = b.cross(&a);
        assert!(c.is_close(&(-d), 1e-10));
    }

    #[test]
    fn test_cross_operator() {
        let x = Vec3d::x_axis();
        let y = Vec3d::y_axis();
        let z = x ^ y;
        assert!(z.is_close(&Vec3d::z_axis(), 1e-10));
    }

    #[test]
    fn test_length() {
        let v = Vec3d::new(1.0, 2.0, 2.0);
        assert!((v.length_squared() - 9.0).abs() < 1e-10);
        assert!((v.length() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize() {
        let v = Vec3d::new(1.0, 2.0, 2.0);
        let n = v.normalized();
        assert!((n.length() - 1.0).abs() < 1e-10);
        assert!((n.x - 1.0 / 3.0).abs() < 1e-10);
        assert!((n.y - 2.0 / 3.0).abs() < 1e-10);
        assert!((n.z - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_arithmetic() {
        let v1 = Vec3d::new(1.0, 2.0, 3.0);
        let v2 = Vec3d::new(4.0, 5.0, 6.0);

        let sum = v1 + v2;
        assert_eq!(sum, Vec3d::new(5.0, 7.0, 9.0));

        let diff = v2 - v1;
        assert_eq!(diff, Vec3d::new(3.0, 3.0, 3.0));

        let scaled = v1 * 2.0;
        assert_eq!(scaled, Vec3d::new(2.0, 4.0, 6.0));

        let divided = v2 / 2.0;
        assert_eq!(divided, Vec3d::new(2.0, 2.5, 3.0));

        let neg = -v1;
        assert_eq!(neg, Vec3d::new(-1.0, -2.0, -3.0));
    }

    #[test]
    fn test_scalar_left_mul() {
        let v = Vec3d::new(1.0, 2.0, 3.0);
        let scaled = 3.0 * v;
        assert_eq!(scaled, Vec3d::new(3.0, 6.0, 9.0));
    }

    #[test]
    fn test_is_close() {
        let v1 = Vec3d::new(1.0, 2.0, 3.0);
        let v2 = Vec3d::new(1.0 + 1e-10, 2.0, 3.0);
        assert!(v1.is_close(&v2, 1e-9));
        assert!(!v1.is_close(&v2, 1e-11));
    }

    #[test]
    fn test_projection() {
        let v = Vec3d::new(3.0, 4.0, 5.0);
        let axis = Vec3d::x_axis();
        let proj = v.projection(&axis);
        assert!((proj.x - 3.0).abs() < 1e-10);
        assert!(proj.y.abs() < 1e-10);
        assert!(proj.z.abs() < 1e-10);
    }

    #[test]
    fn test_comp_mult_div() {
        let v1 = Vec3d::new(2.0, 3.0, 4.0);
        let v2 = Vec3d::new(5.0, 6.0, 7.0);

        let mult = v1.comp_mult(&v2);
        assert_eq!(mult, Vec3d::new(10.0, 18.0, 28.0));

        let div = mult.comp_div(&v2);
        assert!(div.is_close(&v1, 1e-10));
    }

    #[test]
    fn test_min_max() {
        let v1 = Vec3d::new(1.0, 4.0, 2.0);
        let v2 = Vec3d::new(3.0, 2.0, 5.0);

        assert_eq!(v1.min(&v2), Vec3d::new(1.0, 2.0, 2.0));
        assert_eq!(v1.max(&v2), Vec3d::new(3.0, 4.0, 5.0));
    }

    #[test]
    fn test_abs_floor_ceil_round() {
        let v = Vec3d::new(-1.5, 2.7, -0.3);
        assert_eq!(v.abs(), Vec3d::new(1.5, 2.7, 0.3));
        assert_eq!(v.floor(), Vec3d::new(-2.0, 2.0, -1.0));
        assert_eq!(v.ceil(), Vec3d::new(-1.0, 3.0, 0.0));
        assert_eq!(v.round(), Vec3d::new(-2.0, 3.0, 0.0));
    }

    #[test]
    fn test_from_array() {
        let v: Vec3d = [1.0, 2.0, 3.0].into();
        assert_eq!(v, Vec3d::new(1.0, 2.0, 3.0));

        let arr: [f64; 3] = v.into();
        assert_eq!(arr, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_from_tuple() {
        let v: Vec3d = (1.0, 2.0, 3.0).into();
        assert_eq!(v, Vec3d::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_display() {
        let v = Vec3d::new(1.0, 2.0, 3.0);
        assert_eq!(format!("{}", v), "(1, 2, 3)");
    }

    #[test]
    fn test_vec3i() {
        let v1 = Vec3i::new(1, 2, 3);
        let v2 = Vec3i::new(4, 5, 6);

        assert_eq!(v1 + v2, Vec3i::new(5, 7, 9));
        assert_eq!(v2 - v1, Vec3i::new(3, 3, 3));
        assert_eq!(v1 * 2, Vec3i::new(2, 4, 6));
        assert_eq!(-v1, Vec3i::new(-1, -2, -3));
        assert_eq!(v1.dot(&v2), 32);

        let cross = v1.cross(&v2);
        assert_eq!(cross, Vec3i::new(-3, 6, -3));
    }

    #[test]
    fn test_helper_functions() {
        assert_eq!(vec3d(1.0, 2.0, 3.0), Vec3d::new(1.0, 2.0, 3.0));
        assert_eq!(vec3f(1.0, 2.0, 3.0), Vec3f::new(1.0, 2.0, 3.0));
        assert_eq!(vec3i(1, 2, 3), Vec3i::new(1, 2, 3));
    }

    #[test]
    fn test_slerp() {
        let v0 = Vec3d::x_axis();
        let v1 = Vec3d::y_axis();

        // At alpha=0, should return v0
        let s0 = v0.slerp(&v1, 0.0);
        assert!(s0.is_close(&v0, 1e-10));

        // At alpha=1, should return v1
        let s1 = v0.slerp(&v1, 1.0);
        assert!(s1.is_close(&v1, 1e-10));

        // At alpha=0.5, should be halfway between on the great circle
        let mid = v0.slerp(&v1, 0.5);
        assert!((mid.length() - 1.0).abs() < 1e-10);
        let expected = Vec3d::new(0.5_f64.sqrt(), 0.5_f64.sqrt(), 0.0);
        assert!(mid.is_close(&expected, 1e-10));
    }

    #[test]
    fn test_build_orthonormal_frame() {
        let v0 = Vec3d::new(0.0, 0.0, 1.0);
        let (v1, v2) = v0.build_orthonormal_frame();

        // All three should be mutually orthogonal
        assert!(v0.dot(&v1).abs() < 1e-10);
        assert!(v0.dot(&v2).abs() < 1e-10);
        assert!(v1.dot(&v2).abs() < 1e-10);

        // v1 and v2 should be unit length
        assert!((v1.length() - 1.0).abs() < 1e-10);
        assert!((v2.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_orthogonalize_basis() {
        // Start with non-orthogonal vectors
        let mut v0 = Vec3d::new(1.0, 0.1, 0.1);
        let mut v1 = Vec3d::new(0.1, 1.0, 0.1);
        let mut v2 = Vec3d::new(0.1, 0.1, 1.0);

        let converged = Vec3d::orthogonalize_basis(&mut v0, &mut v1, &mut v2, true, 1e-6);

        assert!(converged);

        // Check orthogonality
        assert!(v0.dot(&v1).abs() < 1e-5);
        assert!(v0.dot(&v2).abs() < 1e-5);
        assert!(v1.dot(&v2).abs() < 1e-5);

        // Check unit length
        assert!((v0.length() - 1.0).abs() < 1e-5);
        assert!((v1.length() - 1.0).abs() < 1e-5);
        assert!((v2.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_free_functions() {
        let v1 = Vec3d::x_axis();
        let v2 = Vec3d::y_axis();

        assert!(cross(&v1, &v2).is_close(&Vec3d::z_axis(), 1e-10));
        assert!((dot(&v1, &v2)).abs() < 1e-10);
        assert!((length(&v1) - 1.0).abs() < 1e-10);

        let v3 = Vec3d::new(1.0, 2.0, 2.0);
        let n = normalized(&v3);
        assert!((n.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_close_boundary_le() {
        // H-vt-1: is_close uses <= so diff == eps should be true
        let v1 = Vec3d::new(1.0, 2.0, 3.0);
        let v2 = Vec3d::new(1.5, 2.0, 3.0);
        assert!(v1.is_close(&v2, 0.5)); // diff == eps => true with <=
        assert!(!v1.is_close(&v2, 0.49)); // diff > eps => false
    }

    // =========================================================================
    // M-gf: Cross-type exact equality (matching C++ operator== overloads)
    // =========================================================================

    #[test]
    fn test_cross_type_eq_d_f() {
        let d = Vec3d::new(1.0, 2.0, 3.0);
        let f = Vec3f::new(1.0, 2.0, 3.0);
        assert_eq!(d, f);
        assert_eq!(f, d); // reverse
    }

    #[test]
    fn test_cross_type_eq_d_i() {
        let d = Vec3d::new(1.0, 2.0, 3.0);
        let i = Vec3i::new(1, 2, 3);
        assert_eq!(d, i);
        assert_eq!(i, d); // reverse
    }

    #[test]
    fn test_cross_type_eq_f_i() {
        let f = Vec3f::new(1.0, 2.0, 3.0);
        let i = Vec3i::new(1, 2, 3);
        assert_eq!(f, i);
        assert_eq!(i, f); // reverse
    }

    #[test]
    fn test_cross_type_neq() {
        let d = Vec3d::new(1.0, 2.0, 3.5);
        let i = Vec3i::new(1, 2, 3);
        assert_ne!(d, i);
        assert_ne!(i, d);
    }

    #[test]
    fn test_cross_type_exact_eq() {
        // C++ uses exact ==, not epsilon. Verify non-exactly-representable values differ.
        let d = Vec3d::new(0.1 + 0.2, 0.0, 0.0);
        let f = Vec3f::new(0.3, 0.0, 0.0);
        // 0.1+0.2 in f64 != 0.3f32 promoted to f64 (different bit patterns)
        assert_ne!(d, f);
    }
}
