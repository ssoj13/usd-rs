//! 4D vector types.
//!
//! This module provides Vec4 types for 4D vector math operations.
//! Vec4 is generic over the scalar type, with type aliases for common types.
//!
//! # Examples
//!
//! ```
//! use usd_gf::vec4::{Vec4d, Vec4f, Vec4i};
//!
//! // Create vectors
//! let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
//! let v2 = Vec4d::new(5.0, 6.0, 7.0, 8.0);
//!
//! // Arithmetic
//! let sum = v1 + v2;
//! let scaled = v1 * 2.0;
//! let dot = v1.dot(&v2);
//!
//! // Normalize
//! let normalized = v1.normalized();
//! ```

use crate::limits::MIN_VECTOR_LENGTH;
use crate::traits::{GfVec, Scalar};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// A 4D vector with scalar type `T`.
///
/// Vec4 supports standard arithmetic operations, dot product,
/// length, normalization, and projection operations.
///
/// # Examples
///
/// ```
/// use usd_gf::vec4::Vec4d;
///
/// let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
/// assert!((v.length() - 5.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vec4<T> {
    /// X component.
    pub x: T,
    /// Y component.
    pub y: T,
    /// Z component.
    pub z: T,
    /// W component.
    pub w: T,
}

// Mark Vec4 as a gf vector type
impl<T> GfVec for Vec4<T> {}

/// Type alias for 4D double-precision vector.
pub type Vec4d = Vec4<f64>;

/// Type alias for 4D single-precision vector.
pub type Vec4f = Vec4<f32>;

/// Type alias for 4D half-precision vector.
pub type Vec4h = Vec4<crate::half::Half>;

/// Type alias for 4D integer vector.
pub type Vec4i = Vec4<i32>;

impl<T: Scalar> Vec4<T> {
    /// Dimension of the vector.
    pub const DIMENSION: usize = 4;

    /// Creates a new vector with the given components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::new(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// assert_eq!(v.w, 4.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(x: T, y: T, z: T, w: T) -> Self {
        Self { x, y, z, w }
    }

    /// Creates a vector with all components set to the same value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::splat(5.0);
    /// assert_eq!(v.x, 5.0);
    /// assert_eq!(v.y, 5.0);
    /// assert_eq!(v.z, 5.0);
    /// assert_eq!(v.w, 5.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn splat(value: T) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
            w: value,
        }
    }

    /// Creates a zero vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// assert_eq!(v.w, 0.0);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::x_axis();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// assert_eq!(v.w, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn x_axis() -> Self {
        Self {
            x: T::ONE,
            y: T::ZERO,
            z: T::ZERO,
            w: T::ZERO,
        }
    }

    /// Creates a unit vector along the Y-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::y_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 1.0);
    /// assert_eq!(v.z, 0.0);
    /// assert_eq!(v.w, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn y_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ONE,
            z: T::ZERO,
            w: T::ZERO,
        }
    }

    /// Creates a unit vector along the Z-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::z_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 1.0);
    /// assert_eq!(v.w, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn z_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ONE,
            w: T::ZERO,
        }
    }

    /// Creates a unit vector along the W-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::w_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// assert_eq!(v.w, 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn w_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ZERO,
            w: T::ONE,
        }
    }

    /// Creates a unit vector along the i-th axis (0=X, 1=Y, 2=Z, 3=W).
    ///
    /// Returns the zero vector if `i >= 4`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// assert_eq!(Vec4d::axis(0), Vec4d::x_axis());
    /// assert_eq!(Vec4d::axis(1), Vec4d::y_axis());
    /// assert_eq!(Vec4d::axis(2), Vec4d::z_axis());
    /// assert_eq!(Vec4d::axis(3), Vec4d::w_axis());
    /// assert_eq!(Vec4d::axis(4), Vec4d::zero());
    /// ```
    #[inline]
    #[must_use]
    pub fn axis(i: usize) -> Self {
        match i {
            0 => Self::x_axis(),
            1 => Self::y_axis(),
            2 => Self::z_axis(),
            3 => Self::w_axis(),
            _ => Self::zero(),
        }
    }

    /// Sets all components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let mut v = Vec4d::zero();
    /// v.set(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// assert_eq!(v.w, 4.0);
    /// ```
    #[inline]
    pub fn set(&mut self, x: T, y: T, z: T, w: T) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
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
        // SAFETY: Vec4 is repr(C) with four contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 4) }
    }

    /// Returns the data as a mutable slice.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: Vec4 is repr(C) with four contiguous T values
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), 4) }
    }

    /// Converts to an array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [T; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// Creates from an array.
    #[inline]
    #[must_use]
    pub fn from_array(arr: [T; 4]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
            w: arr[3],
        }
    }

    /// Creates from a pointer to values.
    ///
    /// Matches C++ `GfVec4d(Scl const *p)` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 4 valid `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const T) -> Self {
        unsafe {
            Self {
                x: *ptr,
                y: *ptr.add(1),
                z: *ptr.add(2),
                w: *ptr.add(3),
            }
        }
    }

    /// Sets all elements from a pointer to values.
    ///
    /// Matches C++ `Set(double const *a)` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 4 valid `T` values.
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const T) {
        unsafe {
            self.x = *ptr;
            self.y = *ptr.add(1);
            self.z = *ptr.add(2);
            self.w = *ptr.add(3);
        }
    }

    /// Returns the dot product of two vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
    /// let v2 = Vec4d::new(5.0, 6.0, 7.0, 8.0);
    /// assert!((v1.dot(&v2) - 70.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> T {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    /// Returns the squared length of the vector.
    ///
    /// This is more efficient than `length()` when only comparing magnitudes.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
    /// assert!((v.length_squared() - 25.0).abs() < 1e-10);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
    /// assert!((v.length() - 5.0).abs() < 1e-10);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let mut v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
    /// let original_length = v.normalize();
    /// assert!((original_length - 5.0).abs() < 1e-10);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v = Vec4d::new(3.0, 4.0, 5.0, 6.0);
    /// let axis = Vec4d::x_axis();
    /// let proj = v.projection(&axis);
    /// assert!((proj.x - 3.0).abs() < 1e-10);
    /// assert!(proj.y.abs() < 1e-10);
    /// assert!(proj.z.abs() < 1e-10);
    /// assert!(proj.w.abs() < 1e-10);
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
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v1 = Vec4d::new(2.0, 3.0, 4.0, 5.0);
    /// let v2 = Vec4d::new(6.0, 7.0, 8.0, 9.0);
    /// let result = v1.comp_mult(&v2);
    /// assert!((result.x - 12.0).abs() < 1e-10);
    /// assert!((result.y - 21.0).abs() < 1e-10);
    /// assert!((result.z - 32.0).abs() < 1e-10);
    /// assert!((result.w - 45.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn comp_mult(&self, other: &Self) -> Self {
        Self::new(
            self.x * other.x,
            self.y * other.y,
            self.z * other.z,
            self.w * other.w,
        )
    }

    /// Component-wise division.
    #[inline]
    #[must_use]
    pub fn comp_div(&self, other: &Self) -> Self {
        Self::new(
            self.x / other.x,
            self.y / other.y,
            self.z / other.z,
            self.w / other.w,
        )
    }

    /// Returns whether this vector is approximately equal to another.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec4::Vec4d;
    ///
    /// let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
    /// let v2 = Vec4d::new(1.0 + 1e-10, 2.0, 3.0, 4.0);
    /// assert!(v1.is_close(&v2, 1e-9));
    /// assert!(!v1.is_close(&v2, 1e-11));
    /// ```
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        (self.x - other.x).abs() <= eps
            && (self.y - other.y).abs() <= eps
            && (self.z - other.z).abs() <= eps
            && (self.w - other.w).abs() <= eps
    }

    /// Returns the component-wise minimum.
    #[inline]
    #[must_use]
    pub fn min(&self, other: &Self) -> Self {
        Self::new(
            if self.x < other.x { self.x } else { other.x },
            if self.y < other.y { self.y } else { other.y },
            if self.z < other.z { self.z } else { other.z },
            if self.w < other.w { self.w } else { other.w },
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
            if self.w > other.w { self.w } else { other.w },
        )
    }

    /// Returns the component-wise absolute value.
    #[inline]
    #[must_use]
    pub fn abs(&self) -> Self {
        Self::new(self.x.abs(), self.y.abs(), self.z.abs(), self.w.abs())
    }

    /// Returns the component-wise floor.
    #[inline]
    #[must_use]
    pub fn floor(&self) -> Self {
        Self::new(
            self.x.floor(),
            self.y.floor(),
            self.z.floor(),
            self.w.floor(),
        )
    }

    /// Returns the component-wise ceiling.
    #[inline]
    #[must_use]
    pub fn ceil(&self) -> Self {
        Self::new(self.x.ceil(), self.y.ceil(), self.z.ceil(), self.w.ceil())
    }

    /// Returns the component-wise round.
    #[inline]
    #[must_use]
    pub fn round(&self) -> Self {
        Self::new(
            self.x.round(),
            self.y.round(),
            self.z.round(),
            self.w.round(),
        )
    }
}

// Default - zero vector
impl<T: Scalar> Default for Vec4<T> {
    fn default() -> Self {
        Self::zero()
    }
}

// From array
impl<T: Scalar> From<[T; 4]> for Vec4<T> {
    fn from(arr: [T; 4]) -> Self {
        Self::from_array(arr)
    }
}

// Into array
impl<T: Scalar> From<Vec4<T>> for [T; 4] {
    fn from(v: Vec4<T>) -> Self {
        v.to_array()
    }
}

// From tuple
impl<T: Scalar> From<(T, T, T, T)> for Vec4<T> {
    fn from((x, y, z, w): (T, T, T, T)) -> Self {
        Self::new(x, y, z, w)
    }
}

// Indexing
impl<T> Index<usize> for Vec4<T> {
    type Output = T;

    #[inline]
    fn index(&self, i: usize) -> &T {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("Vec4 index out of bounds: {}", i),
        }
    }
}

impl<T> IndexMut<usize> for Vec4<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut T {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("Vec4 index out of bounds: {}", i),
        }
    }
}

// Equality
impl<T: PartialEq> PartialEq for Vec4<T> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z && self.w == other.w
    }
}

impl<T: Eq> Eq for Vec4<T> {}

// Hash
impl<T: Hash> Hash for Vec4<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
        self.w.hash(state);
    }
}

// Negation
impl<T: Scalar> Neg for Vec4<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

// Addition
impl<T: Scalar> Add for Vec4<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.x + rhs.x,
            self.y + rhs.y,
            self.z + rhs.z,
            self.w + rhs.w,
        )
    }
}

impl<T: Scalar> AddAssign for Vec4<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
        self.z = self.z + rhs.z;
        self.w = self.w + rhs.w;
    }
}

// Subtraction
impl<T: Scalar> Sub for Vec4<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.x - rhs.x,
            self.y - rhs.y,
            self.z - rhs.z,
            self.w - rhs.w,
        )
    }
}

impl<T: Scalar> SubAssign for Vec4<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
        self.z = self.z - rhs.z;
        self.w = self.w - rhs.w;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Vec4<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
    }
}

impl<T: Scalar> MulAssign<T> for Vec4<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.x = self.x * rhs;
        self.y = self.y * rhs;
        self.z = self.z * rhs;
        self.w = self.w * rhs;
    }
}

// Scalar multiplication (scalar on left) - requires specific implementations
impl Mul<Vec4<f64>> for f64 {
    type Output = Vec4<f64>;

    #[inline]
    fn mul(self, rhs: Vec4<f64>) -> Vec4<f64> {
        rhs * self
    }
}

impl Mul<Vec4<f32>> for f32 {
    type Output = Vec4<f32>;

    #[inline]
    fn mul(self, rhs: Vec4<f32>) -> Vec4<f32> {
        rhs * self
    }
}

// Scalar division
impl<T: Scalar> Div<T> for Vec4<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: T) -> Self {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs, self.w / rhs)
    }
}

impl<T: Scalar> DivAssign<T> for Vec4<T> {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        self.x = self.x / rhs;
        self.y = self.y / rhs;
        self.z = self.z / rhs;
        self.w = self.w / rhs;
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Vec4<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {}, {})", self.x, self.y, self.z, self.w)
    }
}

/// Creates a Vec4d from components.
///
/// # Examples
///
/// ```
/// use usd_gf::vec4::vec4d;
///
/// let v = vec4d(1.0, 2.0, 3.0, 4.0);
/// assert_eq!(v.x, 1.0);
/// assert_eq!(v.y, 2.0);
/// assert_eq!(v.z, 3.0);
/// assert_eq!(v.w, 4.0);
/// ```
#[inline]
#[must_use]
pub fn vec4d(x: f64, y: f64, z: f64, w: f64) -> Vec4d {
    Vec4d::new(x, y, z, w)
}

/// Creates a Vec4f from components.
#[inline]
#[must_use]
pub fn vec4f(x: f32, y: f32, z: f32, w: f32) -> Vec4f {
    Vec4f::new(x, y, z, w)
}

/// Creates a Vec4i from components.
#[inline]
#[must_use]
pub fn vec4i(x: i32, y: i32, z: i32, w: i32) -> Vec4i {
    Vec4i { x, y, z, w }
}

// Vec4i needs special handling since i32 doesn't impl Scalar
impl Vec4<i32> {
    /// Creates a new integer vector.
    #[inline]
    #[must_use]
    pub fn new(x: i32, y: i32, z: i32, w: i32) -> Self {
        Self { x, y, z, w }
    }

    /// Creates a zero vector.
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self {
            x: 0,
            y: 0,
            z: 0,
            w: 0,
        }
    }

    /// Creates a vector with all components set to the same value.
    #[inline]
    #[must_use]
    pub fn splat(value: i32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
            w: value,
        }
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> i32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    /// Squared length.
    #[inline]
    #[must_use]
    pub fn length_squared(&self) -> i32 {
        self.dot(self)
    }
}

impl Default for Vec4<i32> {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for Vec4<i32> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.x + rhs.x,
            self.y + rhs.y,
            self.z + rhs.z,
            self.w + rhs.w,
        )
    }
}

impl Sub for Vec4<i32> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.x - rhs.x,
            self.y - rhs.y,
            self.z - rhs.z,
            self.w - rhs.w,
        )
    }
}

impl Mul<i32> for Vec4<i32> {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
    }
}

impl Neg for Vec4<i32> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

/// Free function: returns the dot product of two vectors.
#[inline]
#[must_use]
pub fn dot<T: Scalar>(v1: &Vec4<T>, v2: &Vec4<T>) -> T {
    v1.dot(v2)
}

/// Free function: returns the length of a vector.
#[inline]
#[must_use]
pub fn length<T: Scalar>(v: &Vec4<T>) -> T {
    v.length()
}

/// Free function: returns a normalized vector.
#[inline]
#[must_use]
pub fn normalized<T: Scalar>(v: &Vec4<T>) -> Vec4<T> {
    v.normalized()
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Vec4f> for Vec4d {
    fn from(other: Vec4f) -> Self {
        Self::new(
            other.x as f64,
            other.y as f64,
            other.z as f64,
            other.w as f64,
        )
    }
}

impl From<Vec4h> for Vec4d {
    fn from(other: Vec4h) -> Self {
        Self::new(
            other.x.to_f64(),
            other.y.to_f64(),
            other.z.to_f64(),
            other.w.to_f64(),
        )
    }
}

impl From<Vec4i> for Vec4d {
    fn from(other: Vec4i) -> Self {
        Self::new(
            other.x as f64,
            other.y as f64,
            other.z as f64,
            other.w as f64,
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads).
// C++ uses implicit promotion and exact ==; we match that behavior.

// -- Vec4d vs others --
impl PartialEq<Vec4f> for Vec4d {
    fn eq(&self, other: &Vec4f) -> bool {
        self.x == other.x as f64
            && self.y == other.y as f64
            && self.z == other.z as f64
            && self.w == other.w as f64
    }
}
impl PartialEq<Vec4h> for Vec4d {
    fn eq(&self, other: &Vec4h) -> bool {
        self.x == other.x.to_f64()
            && self.y == other.y.to_f64()
            && self.z == other.z.to_f64()
            && self.w == other.w.to_f64()
    }
}
impl PartialEq<Vec4i> for Vec4d {
    fn eq(&self, other: &Vec4i) -> bool {
        self.x == other.x as f64
            && self.y == other.y as f64
            && self.z == other.z as f64
            && self.w == other.w as f64
    }
}

// -- Vec4f vs others --
impl PartialEq<Vec4d> for Vec4f {
    fn eq(&self, other: &Vec4d) -> bool {
        self.x as f64 == other.x
            && self.y as f64 == other.y
            && self.z as f64 == other.z
            && self.w as f64 == other.w
    }
}
impl PartialEq<Vec4h> for Vec4f {
    fn eq(&self, other: &Vec4h) -> bool {
        self.x == other.x.to_f32()
            && self.y == other.y.to_f32()
            && self.z == other.z.to_f32()
            && self.w == other.w.to_f32()
    }
}
impl PartialEq<Vec4i> for Vec4f {
    fn eq(&self, other: &Vec4i) -> bool {
        self.x == other.x as f32
            && self.y == other.y as f32
            && self.z == other.z as f32
            && self.w == other.w as f32
    }
}

// -- Vec4h vs others --
impl PartialEq<Vec4d> for Vec4h {
    fn eq(&self, other: &Vec4d) -> bool {
        self.x.to_f64() == other.x
            && self.y.to_f64() == other.y
            && self.z.to_f64() == other.z
            && self.w.to_f64() == other.w
    }
}
impl PartialEq<Vec4f> for Vec4h {
    fn eq(&self, other: &Vec4f) -> bool {
        self.x.to_f32() == other.x
            && self.y.to_f32() == other.y
            && self.z.to_f32() == other.z
            && self.w.to_f32() == other.w
    }
}
impl PartialEq<Vec4i> for Vec4h {
    fn eq(&self, other: &Vec4i) -> bool {
        self.x.to_f64() == other.x as f64
            && self.y.to_f64() == other.y as f64
            && self.z.to_f64() == other.z as f64
            && self.w.to_f64() == other.w as f64
    }
}

// -- Vec4i vs others --
impl PartialEq<Vec4d> for Vec4i {
    fn eq(&self, other: &Vec4d) -> bool {
        self.x as f64 == other.x
            && self.y as f64 == other.y
            && self.z as f64 == other.z
            && self.w as f64 == other.w
    }
}
impl PartialEq<Vec4f> for Vec4i {
    fn eq(&self, other: &Vec4f) -> bool {
        self.x as f32 == other.x
            && self.y as f32 == other.y
            && self.z as f32 == other.z
            && self.w as f32 == other.w
    }
}
impl PartialEq<Vec4h> for Vec4i {
    fn eq(&self, other: &Vec4h) -> bool {
        self.x as f64 == other.x.to_f64()
            && self.y as f64 == other.y.to_f64()
            && self.z as f64 == other.z.to_f64()
            && self.w as f64 == other.w.to_f64()
    }
}

// Additional global functions (matching C++ Gf* functions)
/// Returns component-wise multiplication of vectors.
///
/// Matches C++ `GfCompMult(GfVec4d const &v1, GfVec4d const &v2)`.
#[inline]
#[must_use]
pub fn comp_mult<T: Scalar>(v1: &Vec4<T>, v2: &Vec4<T>) -> Vec4<T> {
    v1.comp_mult(v2)
}

/// Returns component-wise quotient of vectors.
///
/// Matches C++ `GfCompDiv(GfVec4d const &v1, GfVec4d const &v2)`.
#[inline]
#[must_use]
pub fn comp_div<T: Scalar>(v1: &Vec4<T>, v2: &Vec4<T>) -> Vec4<T> {
    v1.comp_div(v2)
}

/// Normalizes a vector in place to unit length, returning the length before normalization.
///
/// Matches C++ `GfNormalize(GfVec4d *v, double eps)`.
#[inline]
pub fn normalize<T: Scalar>(v: &mut Vec4<T>) -> T {
    v.normalize()
}

/// Returns the projection of `a` onto `b`.
///
/// Matches C++ `GfGetProjection(GfVec4d const &a, GfVec4d const &b)`.
#[inline]
#[must_use]
pub fn projection<T: Scalar>(a: &Vec4<T>, b: &Vec4<T>) -> Vec4<T> {
    a.projection(b)
}

/// Returns the orthogonal complement of `a.projection(b)`.
///
/// Matches C++ `GfGetComplement(GfVec4d const &a, GfVec4d const &b)`.
#[inline]
#[must_use]
pub fn complement<T: Scalar>(a: &Vec4<T>, b: &Vec4<T>) -> Vec4<T> {
    a.complement(b)
}

/// Tests for equality within a given tolerance (component-wise).
///
/// Note: Uses component-wise (Chebyshev/L-inf) comparison, unlike C++
/// `GfIsClose` which uses Euclidean distance (L2 norm).
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(v1: &Vec4<T>, v2: &Vec4<T>, tolerance: T) -> bool {
    v1.is_close(v2, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
        assert_eq!(v.w, 4.0);
    }

    #[test]
    fn test_splat() {
        let v = Vec4d::splat(5.0);
        assert_eq!(v.x, 5.0);
        assert_eq!(v.y, 5.0);
        assert_eq!(v.z, 5.0);
        assert_eq!(v.w, 5.0);
    }

    #[test]
    fn test_zero() {
        let v = Vec4d::zero();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
        assert_eq!(v.w, 0.0);
    }

    #[test]
    fn test_axis() {
        assert_eq!(Vec4d::x_axis(), Vec4d::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(Vec4d::y_axis(), Vec4d::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(Vec4d::z_axis(), Vec4d::new(0.0, 0.0, 1.0, 0.0));
        assert_eq!(Vec4d::w_axis(), Vec4d::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(Vec4d::axis(0), Vec4d::x_axis());
        assert_eq!(Vec4d::axis(1), Vec4d::y_axis());
        assert_eq!(Vec4d::axis(2), Vec4d::z_axis());
        assert_eq!(Vec4d::axis(3), Vec4d::w_axis());
        assert_eq!(Vec4d::axis(4), Vec4d::zero());
    }

    #[test]
    fn test_indexing() {
        let mut v = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 2.0);
        assert_eq!(v[2], 3.0);
        assert_eq!(v[3], 4.0);
        v[0] = 5.0;
        assert_eq!(v[0], 5.0);
    }

    #[test]
    fn test_dot() {
        let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4d::new(5.0, 6.0, 7.0, 8.0);
        assert!((v1.dot(&v2) - 70.0).abs() < 1e-10);
    }

    #[test]
    fn test_length() {
        let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
        assert!((v.length_squared() - 25.0).abs() < 1e-10);
        assert!((v.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize() {
        let v = Vec4d::new(1.0, 2.0, 2.0, 4.0);
        let n = v.normalized();
        assert!((n.length() - 1.0).abs() < 1e-10);
        assert!((n.x - 0.2).abs() < 1e-10);
        assert!((n.y - 0.4).abs() < 1e-10);
        assert!((n.z - 0.4).abs() < 1e-10);
        assert!((n.w - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_arithmetic() {
        let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4d::new(5.0, 6.0, 7.0, 8.0);

        let sum = v1 + v2;
        assert_eq!(sum, Vec4d::new(6.0, 8.0, 10.0, 12.0));

        let diff = v2 - v1;
        assert_eq!(diff, Vec4d::new(4.0, 4.0, 4.0, 4.0));

        let scaled = v1 * 2.0;
        assert_eq!(scaled, Vec4d::new(2.0, 4.0, 6.0, 8.0));

        let divided = v2 / 2.0;
        assert_eq!(divided, Vec4d::new(2.5, 3.0, 3.5, 4.0));

        let neg = -v1;
        assert_eq!(neg, Vec4d::new(-1.0, -2.0, -3.0, -4.0));
    }

    #[test]
    fn test_scalar_left_mul() {
        let v = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        let scaled = 3.0 * v;
        assert_eq!(scaled, Vec4d::new(3.0, 6.0, 9.0, 12.0));
    }

    #[test]
    fn test_is_close() {
        let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4d::new(1.0 + 1e-10, 2.0, 3.0, 4.0);
        assert!(v1.is_close(&v2, 1e-9));
        assert!(!v1.is_close(&v2, 1e-11));
    }

    #[test]
    fn test_projection() {
        let v = Vec4d::new(3.0, 4.0, 5.0, 6.0);
        let axis = Vec4d::x_axis();
        let proj = v.projection(&axis);
        assert!((proj.x - 3.0).abs() < 1e-10);
        assert!(proj.y.abs() < 1e-10);
        assert!(proj.z.abs() < 1e-10);
        assert!(proj.w.abs() < 1e-10);
    }

    #[test]
    fn test_comp_mult_div() {
        let v1 = Vec4d::new(2.0, 3.0, 4.0, 5.0);
        let v2 = Vec4d::new(6.0, 7.0, 8.0, 9.0);

        let mult = v1.comp_mult(&v2);
        assert_eq!(mult, Vec4d::new(12.0, 21.0, 32.0, 45.0));

        let div = mult.comp_div(&v2);
        assert!(div.is_close(&v1, 1e-10));
    }

    #[test]
    fn test_min_max() {
        let v1 = Vec4d::new(1.0, 4.0, 2.0, 6.0);
        let v2 = Vec4d::new(3.0, 2.0, 5.0, 4.0);

        assert_eq!(v1.min(&v2), Vec4d::new(1.0, 2.0, 2.0, 4.0));
        assert_eq!(v1.max(&v2), Vec4d::new(3.0, 4.0, 5.0, 6.0));
    }

    #[test]
    fn test_abs_floor_ceil_round() {
        let v = Vec4d::new(-1.5, 2.7, -0.3, 4.5);
        assert_eq!(v.abs(), Vec4d::new(1.5, 2.7, 0.3, 4.5));
        assert_eq!(v.floor(), Vec4d::new(-2.0, 2.0, -1.0, 4.0));
        assert_eq!(v.ceil(), Vec4d::new(-1.0, 3.0, 0.0, 5.0));
        assert_eq!(v.round(), Vec4d::new(-2.0, 3.0, 0.0, 5.0));
    }

    #[test]
    fn test_from_array() {
        let v: Vec4d = [1.0, 2.0, 3.0, 4.0].into();
        assert_eq!(v, Vec4d::new(1.0, 2.0, 3.0, 4.0));

        let arr: [f64; 4] = v.into();
        assert_eq!(arr, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_from_tuple() {
        let v: Vec4d = (1.0, 2.0, 3.0, 4.0).into();
        assert_eq!(v, Vec4d::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn test_display() {
        let v = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(format!("{}", v), "(1, 2, 3, 4)");
    }

    #[test]
    fn test_vec4i() {
        let v1 = Vec4i::new(1, 2, 3, 4);
        let v2 = Vec4i::new(5, 6, 7, 8);

        assert_eq!(v1 + v2, Vec4i::new(6, 8, 10, 12));
        assert_eq!(v2 - v1, Vec4i::new(4, 4, 4, 4));
        assert_eq!(v1 * 2, Vec4i::new(2, 4, 6, 8));
        assert_eq!(-v1, Vec4i::new(-1, -2, -3, -4));
        assert_eq!(v1.dot(&v2), 70);
    }

    #[test]
    fn test_helper_functions() {
        assert_eq!(vec4d(1.0, 2.0, 3.0, 4.0), Vec4d::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(vec4f(1.0, 2.0, 3.0, 4.0), Vec4f::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(vec4i(1, 2, 3, 4), Vec4i::new(1, 2, 3, 4));
    }

    #[test]
    fn test_free_functions() {
        let v1 = Vec4d::x_axis();
        let v2 = Vec4d::y_axis();

        assert!((dot(&v1, &v2)).abs() < 1e-10);
        assert!((length(&v1) - 1.0).abs() < 1e-10);

        let v3 = Vec4d::new(1.0, 2.0, 2.0, 4.0);
        let n = normalized(&v3);
        assert!((n.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_close_boundary_le() {
        // H-vt-1: is_close uses <= so diff == eps should be true
        let v1 = Vec4d::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4d::new(1.5, 2.0, 3.0, 4.0);
        assert!(v1.is_close(&v2, 0.5)); // diff == eps => true with <=
        assert!(!v1.is_close(&v2, 0.49)); // diff > eps => false
    }
}
