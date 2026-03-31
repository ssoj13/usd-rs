//! 2D vector types.
//!
//! This module provides Vec2 types for 2D vector math operations.
//! Vec2 is generic over the scalar type, with type aliases for common types.
//!
//! # Examples
//!
//! ```
//! use usd_gf::vec2::{Vec2d, Vec2f, Vec2i};
//!
//! // Create vectors
//! let v1 = Vec2d::new(1.0, 2.0);
//! let v2 = Vec2d::new(3.0, 4.0);
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

/// A 2D vector with scalar type `T`.
///
/// Vec2 supports standard arithmetic operations, dot product, length,
/// normalization, and projection operations.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::Vec2d;
///
/// let v = Vec2d::new(3.0, 4.0);
/// assert!((v.length() - 5.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vec2<T> {
    /// X component.
    pub x: T,
    /// Y component.
    pub y: T,
}

// Mark Vec2 as a gf vector type
impl<T> GfVec for Vec2<T> {}

/// Type alias for 2D double-precision vector.
pub type Vec2d = Vec2<f64>;

/// Type alias for 2D single-precision vector.
pub type Vec2f = Vec2<f32>;

/// Type alias for 2D half-precision vector.
pub type Vec2h = Vec2<crate::half::Half>;

/// Type alias for 2D integer vector.
pub type Vec2i = Vec2<i32>;

impl<T: Scalar> Vec2<T> {
    /// Dimension of the vector.
    pub const DIMENSION: usize = 2;

    /// Creates a new vector with the given components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::new(1.0, 2.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// Creates a vector with all components set to the same value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::splat(5.0);
    /// assert_eq!(v.x, 5.0);
    /// assert_eq!(v.y, 5.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn splat(value: T) -> Self {
        Self { x: value, y: value }
    }

    /// Creates a zero vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::x_axis();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn x_axis() -> Self {
        Self {
            x: T::ONE,
            y: T::ZERO,
        }
    }

    /// Creates a unit vector along the Y-axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::y_axis();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn y_axis() -> Self {
        Self {
            x: T::ZERO,
            y: T::ONE,
        }
    }

    /// Creates a unit vector along the i-th axis (0=X, 1=Y).
    ///
    /// Returns the zero vector if `i >= 2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// assert_eq!(Vec2d::axis(0), Vec2d::x_axis());
    /// assert_eq!(Vec2d::axis(1), Vec2d::y_axis());
    /// assert_eq!(Vec2d::axis(2), Vec2d::zero());
    /// ```
    #[inline]
    #[must_use]
    pub fn axis(i: usize) -> Self {
        match i {
            0 => Self::x_axis(),
            1 => Self::y_axis(),
            _ => Self::zero(),
        }
    }

    /// Sets all components.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let mut v = Vec2d::zero();
    /// v.set(3.0, 4.0);
    /// assert_eq!(v.x, 3.0);
    /// assert_eq!(v.y, 4.0);
    /// ```
    #[inline]
    pub fn set(&mut self, x: T, y: T) {
        self.x = x;
        self.y = y;
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
        // SAFETY: Vec2 is repr(C) with two contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 2) }
    }

    /// Returns the data as a mutable slice.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: Vec2 is repr(C) with two contiguous T values
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), 2) }
    }

    /// Converts to an array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [T; 2] {
        [self.x, self.y]
    }

    /// Creates from an array.
    #[inline]
    #[must_use]
    pub fn from_array(arr: [T; 2]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
        }
    }

    /// Creates from a pointer to values.
    ///
    /// Matches C++ `GfVec2d(Scl const *p)` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 2 valid `T` values.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let arr = [1.0, 2.0];
    /// let v = unsafe { Vec2d::from_ptr(arr.as_ptr()) };
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// ```
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const T) -> Self {
        unsafe {
            Self {
                x: *ptr,
                y: *ptr.add(1),
            }
        }
    }

    /// Sets all elements from a pointer to values.
    ///
    /// Matches C++ `Set(double const *a)` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 2 valid `T` values.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let mut v = Vec2d::zero();
    /// let arr = [3.0, 4.0];
    /// unsafe { v.set_from_ptr(arr.as_ptr()) };
    /// assert_eq!(v.x, 3.0);
    /// assert_eq!(v.y, 4.0);
    /// ```
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const T) {
        unsafe {
            self.x = *ptr;
            self.y = *ptr.add(1);
        }
    }

    /// Returns the dot product of two vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v1 = Vec2d::new(1.0, 2.0);
    /// let v2 = Vec2d::new(3.0, 4.0);
    /// assert!((v1.dot(&v2) - 11.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> T {
        self.x * other.x + self.y * other.y
    }

    /// Returns the squared length of the vector.
    ///
    /// This is more efficient than `length()` when only comparing magnitudes.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::new(3.0, 4.0);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::new(3.0, 4.0);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::new(3.0, 4.0);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let mut v = Vec2d::new(3.0, 4.0);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v = Vec2d::new(3.0, 4.0);
    /// let axis = Vec2d::x_axis();
    /// let proj = v.projection(&axis);
    /// assert!((proj.x - 3.0).abs() < 1e-10);
    /// assert!(proj.y.abs() < 1e-10);
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
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v1 = Vec2d::new(2.0, 3.0);
    /// let v2 = Vec2d::new(4.0, 5.0);
    /// let result = v1.comp_mult(&v2);
    /// assert!((result.x - 8.0).abs() < 1e-10);
    /// assert!((result.y - 15.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn comp_mult(&self, other: &Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y)
    }

    /// Component-wise division.
    #[inline]
    #[must_use]
    pub fn comp_div(&self, other: &Self) -> Self {
        Self::new(self.x / other.x, self.y / other.y)
    }

    /// Returns whether this vector is approximately equal to another.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let v1 = Vec2d::new(1.0, 2.0);
    /// let v2 = Vec2d::new(1.0 + 1e-10, 2.0);
    /// assert!(v1.is_close(&v2, 1e-9));
    /// assert!(!v1.is_close(&v2, 1e-11));
    /// ```
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        (self.x - other.x).abs() <= eps && (self.y - other.y).abs() <= eps
    }

    /// Returns the component-wise minimum.
    #[inline]
    #[must_use]
    pub fn min(&self, other: &Self) -> Self {
        Self::new(
            if self.x < other.x { self.x } else { other.x },
            if self.y < other.y { self.y } else { other.y },
        )
    }

    /// Returns the component-wise maximum.
    #[inline]
    #[must_use]
    pub fn max(&self, other: &Self) -> Self {
        Self::new(
            if self.x > other.x { self.x } else { other.x },
            if self.y > other.y { self.y } else { other.y },
        )
    }

    /// Returns the component-wise absolute value.
    #[inline]
    #[must_use]
    pub fn abs(&self) -> Self {
        Self::new(self.x.abs(), self.y.abs())
    }

    /// Returns the component-wise floor.
    #[inline]
    #[must_use]
    pub fn floor(&self) -> Self {
        Self::new(self.x.floor(), self.y.floor())
    }

    /// Returns the component-wise ceiling.
    #[inline]
    #[must_use]
    pub fn ceil(&self) -> Self {
        Self::new(self.x.ceil(), self.y.ceil())
    }

    /// Returns the component-wise round.
    #[inline]
    #[must_use]
    pub fn round(&self) -> Self {
        Self::new(self.x.round(), self.y.round())
    }
}

// Default - zero vector
impl<T: Scalar> Default for Vec2<T> {
    fn default() -> Self {
        Self::zero()
    }
}

// From array
impl<T: Scalar> From<[T; 2]> for Vec2<T> {
    fn from(arr: [T; 2]) -> Self {
        Self::from_array(arr)
    }
}

// Into array
impl<T: Scalar> From<Vec2<T>> for [T; 2] {
    fn from(v: Vec2<T>) -> Self {
        v.to_array()
    }
}

// From tuple
impl<T: Scalar> From<(T, T)> for Vec2<T> {
    fn from((x, y): (T, T)) -> Self {
        Self::new(x, y)
    }
}

// Indexing
impl<T> Index<usize> for Vec2<T> {
    type Output = T;

    #[inline]
    fn index(&self, i: usize) -> &T {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("Vec2 index out of bounds: {}", i),
        }
    }
}

impl<T> IndexMut<usize> for Vec2<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut T {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => panic!("Vec2 index out of bounds: {}", i),
        }
    }
}

// Equality
impl<T: PartialEq> PartialEq for Vec2<T> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<T: Eq> Eq for Vec2<T> {}

// Hash
impl<T: Hash> Hash for Vec2<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

// Negation
impl<T: Scalar> Neg for Vec2<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

// Addition
impl<T: Scalar> Add for Vec2<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T: Scalar> AddAssign for Vec2<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
    }
}

// Subtraction
impl<T: Scalar> Sub for Vec2<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl<T: Scalar> SubAssign for Vec2<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Vec2<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl<T: Scalar> MulAssign<T> for Vec2<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.x = self.x * rhs;
        self.y = self.y * rhs;
    }
}

// Scalar multiplication (scalar on left) - requires specific implementations
impl Mul<Vec2<f64>> for f64 {
    type Output = Vec2<f64>;

    #[inline]
    fn mul(self, rhs: Vec2<f64>) -> Vec2<f64> {
        rhs * self
    }
}

impl Mul<Vec2<f32>> for f32 {
    type Output = Vec2<f32>;

    #[inline]
    fn mul(self, rhs: Vec2<f32>) -> Vec2<f32> {
        rhs * self
    }
}

// Scalar division
impl<T: Scalar> Div<T> for Vec2<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: T) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl<T: Scalar> DivAssign<T> for Vec2<T> {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        self.x = self.x / rhs;
        self.y = self.y / rhs;
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Vec2<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// Creates a Vec2d from components.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::vec2d;
///
/// let v = vec2d(1.0, 2.0);
/// assert_eq!(v.x, 1.0);
/// assert_eq!(v.y, 2.0);
/// ```
#[inline]
#[must_use]
pub fn vec2d(x: f64, y: f64) -> Vec2d {
    Vec2d::new(x, y)
}

/// Creates a Vec2f from components.
#[inline]
#[must_use]
pub fn vec2f(x: f32, y: f32) -> Vec2f {
    Vec2f::new(x, y)
}

/// Creates a Vec2i from components.
#[inline]
#[must_use]
pub fn vec2i(x: i32, y: i32) -> Vec2i {
    Vec2i { x, y }
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Vec2f> for Vec2d {
    fn from(other: Vec2f) -> Self {
        Self::new(other.x as f64, other.y as f64)
    }
}

impl From<Vec2h> for Vec2d {
    fn from(other: Vec2h) -> Self {
        Self::new(other.x.to_f64(), other.y.to_f64())
    }
}

impl From<Vec2i> for Vec2d {
    fn from(other: Vec2i) -> Self {
        Self::new(other.x as f64, other.y as f64)
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads).
// C++ uses implicit promotion and exact ==; we match that behavior.

// -- Vec2d vs others --
impl PartialEq<Vec2f> for Vec2d {
    fn eq(&self, other: &Vec2f) -> bool {
        self.x == other.x as f64 && self.y == other.y as f64
    }
}
impl PartialEq<Vec2h> for Vec2d {
    fn eq(&self, other: &Vec2h) -> bool {
        self.x == other.x.to_f64() && self.y == other.y.to_f64()
    }
}
impl PartialEq<Vec2i> for Vec2d {
    fn eq(&self, other: &Vec2i) -> bool {
        self.x == other.x as f64 && self.y == other.y as f64
    }
}

// -- Vec2f vs others --
impl PartialEq<Vec2d> for Vec2f {
    fn eq(&self, other: &Vec2d) -> bool {
        self.x as f64 == other.x && self.y as f64 == other.y
    }
}
impl PartialEq<Vec2h> for Vec2f {
    fn eq(&self, other: &Vec2h) -> bool {
        self.x == other.x.to_f32() && self.y == other.y.to_f32()
    }
}
impl PartialEq<Vec2i> for Vec2f {
    fn eq(&self, other: &Vec2i) -> bool {
        self.x == other.x as f32 && self.y == other.y as f32
    }
}

// -- Vec2h vs others --
impl PartialEq<Vec2d> for Vec2h {
    fn eq(&self, other: &Vec2d) -> bool {
        self.x.to_f64() == other.x && self.y.to_f64() == other.y
    }
}
impl PartialEq<Vec2f> for Vec2h {
    fn eq(&self, other: &Vec2f) -> bool {
        self.x.to_f32() == other.x && self.y.to_f32() == other.y
    }
}
impl PartialEq<Vec2i> for Vec2h {
    fn eq(&self, other: &Vec2i) -> bool {
        self.x.to_f64() == other.x as f64 && self.y.to_f64() == other.y as f64
    }
}

// -- Vec2i vs others --
impl PartialEq<Vec2d> for Vec2i {
    fn eq(&self, other: &Vec2d) -> bool {
        self.x as f64 == other.x && self.y as f64 == other.y
    }
}
impl PartialEq<Vec2f> for Vec2i {
    fn eq(&self, other: &Vec2f) -> bool {
        self.x as f32 == other.x && self.y as f32 == other.y
    }
}
impl PartialEq<Vec2h> for Vec2i {
    fn eq(&self, other: &Vec2h) -> bool {
        self.x as f64 == other.x.to_f64() && self.y as f64 == other.y.to_f64()
    }
}

// Global functions (matching C++ Gf* functions)
/// Returns component-wise multiplication of vectors.
///
/// Matches C++ `GfCompMult(GfVec2d const &v1, GfVec2d const &v2)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, comp_mult};
///
/// let v1 = Vec2d::new(2.0, 3.0);
/// let v2 = Vec2d::new(4.0, 5.0);
/// let result = comp_mult(&v1, &v2);
/// assert_eq!(result, Vec2d::new(8.0, 15.0));
/// ```
#[inline]
#[must_use]
pub fn comp_mult<T: Scalar>(v1: &Vec2<T>, v2: &Vec2<T>) -> Vec2<T> {
    v1.comp_mult(v2)
}

/// Returns component-wise quotient of vectors.
///
/// Matches C++ `GfCompDiv(GfVec2d const &v1, GfVec2d const &v2)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, comp_div};
///
/// let v1 = Vec2d::new(8.0, 15.0);
/// let v2 = Vec2d::new(2.0, 3.0);
/// let result = comp_div(&v1, &v2);
/// assert_eq!(result, Vec2d::new(4.0, 5.0));
/// ```
#[inline]
#[must_use]
pub fn comp_div<T: Scalar>(v1: &Vec2<T>, v2: &Vec2<T>) -> Vec2<T> {
    v1.comp_div(v2)
}

/// Returns the dot (inner) product of two vectors.
///
/// Matches C++ `GfDot(GfVec2d const &v1, GfVec2d const &v2)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, dot};
///
/// let v1 = Vec2d::new(1.0, 2.0);
/// let v2 = Vec2d::new(3.0, 4.0);
/// assert!((dot(&v1, &v2) - 11.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn dot<T: Scalar>(v1: &Vec2<T>, v2: &Vec2<T>) -> T {
    v1.dot(v2)
}

/// Returns the geometric length of a vector.
///
/// Matches C++ `GfGetLength(GfVec2d const &v)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, length};
///
/// let v = Vec2d::new(3.0, 4.0);
/// assert!((length(&v) - 5.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn length<T: Scalar>(v: &Vec2<T>) -> T {
    v.length()
}

/// Normalizes a vector in place to unit length, returning the length before normalization.
///
/// Matches C++ `GfNormalize(GfVec2d *v, double eps)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, normalize};
///
/// let mut v = Vec2d::new(3.0, 4.0);
/// let original_length = normalize(&mut v);
/// assert!((original_length - 5.0).abs() < 1e-10);
/// assert!((v.length() - 1.0).abs() < 1e-10);
/// ```
#[inline]
pub fn normalize<T: Scalar>(v: &mut Vec2<T>) -> T {
    v.normalize()
}

/// Returns a normalized (unit-length) vector with the same direction.
///
/// Matches C++ `GfGetNormalized(GfVec2d const &v, double eps)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, normalized};
///
/// let v = Vec2d::new(3.0, 4.0);
/// let n = normalized(&v);
/// assert!((n.length() - 1.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn normalized<T: Scalar>(v: &Vec2<T>) -> Vec2<T> {
    v.normalized()
}

/// Returns the projection of `a` onto `b`.
///
/// Matches C++ `GfGetProjection(GfVec2d const &a, GfVec2d const &b)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, projection};
///
/// let a = Vec2d::new(3.0, 4.0);
/// let b = Vec2d::x_axis();
/// let proj = projection(&a, &b);
/// assert!((proj.x - 3.0).abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn projection<T: Scalar>(a: &Vec2<T>, b: &Vec2<T>) -> Vec2<T> {
    a.projection(b)
}

/// Returns the orthogonal complement of `a.projection(b)`.
///
/// Matches C++ `GfGetComplement(GfVec2d const &a, GfVec2d const &b)`.
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, complement};
///
/// let a = Vec2d::new(3.0, 4.0);
/// let b = Vec2d::x_axis();
/// let comp = complement(&a, &b);
/// assert!(comp.x.abs() < 1e-10);
/// ```
#[inline]
#[must_use]
pub fn complement<T: Scalar>(a: &Vec2<T>, b: &Vec2<T>) -> Vec2<T> {
    a.complement(b)
}

/// Tests for equality within a given tolerance (component-wise).
///
/// Note: Uses component-wise (Chebyshev/L-inf) comparison, unlike C++
/// `GfIsClose` which uses Euclidean distance (L2 norm).
///
/// # Examples
///
/// ```
/// use usd_gf::vec2::{Vec2d, is_close};
///
/// let v1 = Vec2d::new(1.0, 2.0);
/// let v2 = Vec2d::new(1.0 + 1e-10, 2.0);
/// assert!(is_close(&v1, &v2, 1e-9));
/// ```
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(v1: &Vec2<T>, v2: &Vec2<T>, tolerance: T) -> bool {
    v1.is_close(v2, tolerance)
}

// Vec2i needs special handling since i32 doesn't impl Scalar
impl Vec2<i32> {
    /// Creates a new integer vector.
    #[inline]
    #[must_use]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Creates a zero vector.
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }

    /// Creates a vector with all components set to the same value.
    #[inline]
    #[must_use]
    pub fn splat(value: i32) -> Self {
        Self { x: value, y: value }
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(&self, other: &Self) -> i32 {
        self.x * other.x + self.y * other.y
    }
}

impl Default for Vec2<i32> {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for Vec2<i32> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2<i32> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<i32> for Vec2<i32> {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for Vec2<i32> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec2d::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
    }

    #[test]
    fn test_splat() {
        let v = Vec2d::splat(5.0);
        assert_eq!(v.x, 5.0);
        assert_eq!(v.y, 5.0);
    }

    #[test]
    fn test_zero() {
        let v = Vec2d::zero();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
    }

    #[test]
    fn test_axis() {
        assert_eq!(Vec2d::x_axis(), Vec2d::new(1.0, 0.0));
        assert_eq!(Vec2d::y_axis(), Vec2d::new(0.0, 1.0));
        assert_eq!(Vec2d::axis(0), Vec2d::x_axis());
        assert_eq!(Vec2d::axis(1), Vec2d::y_axis());
        assert_eq!(Vec2d::axis(2), Vec2d::zero());
    }

    #[test]
    fn test_indexing() {
        let mut v = Vec2d::new(1.0, 2.0);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 2.0);
        v[0] = 3.0;
        assert_eq!(v[0], 3.0);
    }

    #[test]
    fn test_dot() {
        let v1 = Vec2d::new(1.0, 2.0);
        let v2 = Vec2d::new(3.0, 4.0);
        assert!((v1.dot(&v2) - 11.0).abs() < 1e-10);
    }

    #[test]
    fn test_length() {
        let v = Vec2d::new(3.0, 4.0);
        assert!((v.length_squared() - 25.0).abs() < 1e-10);
        assert!((v.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize() {
        let v = Vec2d::new(3.0, 4.0);
        let n = v.normalized();
        assert!((n.length() - 1.0).abs() < 1e-10);
        assert!((n.x - 0.6).abs() < 1e-10);
        assert!((n.y - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_arithmetic() {
        let v1 = Vec2d::new(1.0, 2.0);
        let v2 = Vec2d::new(3.0, 4.0);

        let sum = v1 + v2;
        assert_eq!(sum, Vec2d::new(4.0, 6.0));

        let diff = v2 - v1;
        assert_eq!(diff, Vec2d::new(2.0, 2.0));

        let scaled = v1 * 2.0;
        assert_eq!(scaled, Vec2d::new(2.0, 4.0));

        let divided = v2 / 2.0;
        assert_eq!(divided, Vec2d::new(1.5, 2.0));

        let neg = -v1;
        assert_eq!(neg, Vec2d::new(-1.0, -2.0));
    }

    #[test]
    fn test_scalar_left_mul() {
        let v = Vec2d::new(1.0, 2.0);
        let scaled = 3.0 * v;
        assert_eq!(scaled, Vec2d::new(3.0, 6.0));
    }

    #[test]
    fn test_is_close() {
        let v1 = Vec2d::new(1.0, 2.0);
        let v2 = Vec2d::new(1.0 + 1e-10, 2.0);
        assert!(v1.is_close(&v2, 1e-9));
        assert!(!v1.is_close(&v2, 1e-11));
    }

    #[test]
    fn test_projection() {
        let v = Vec2d::new(3.0, 4.0);
        let axis = Vec2d::x_axis();
        let proj = v.projection(&axis);
        assert!((proj.x - 3.0).abs() < 1e-10);
        assert!(proj.y.abs() < 1e-10);
    }

    #[test]
    fn test_comp_mult_div() {
        let v1 = Vec2d::new(2.0, 3.0);
        let v2 = Vec2d::new(4.0, 5.0);

        let mult = v1.comp_mult(&v2);
        assert_eq!(mult, Vec2d::new(8.0, 15.0));

        let div = mult.comp_div(&v2);
        assert!(div.is_close(&v1, 1e-10));
    }

    #[test]
    fn test_min_max() {
        let v1 = Vec2d::new(1.0, 4.0);
        let v2 = Vec2d::new(3.0, 2.0);

        assert_eq!(v1.min(&v2), Vec2d::new(1.0, 2.0));
        assert_eq!(v1.max(&v2), Vec2d::new(3.0, 4.0));
    }

    #[test]
    fn test_abs_floor_ceil_round() {
        let v = Vec2d::new(-1.5, 2.7);
        assert_eq!(v.abs(), Vec2d::new(1.5, 2.7));
        assert_eq!(v.floor(), Vec2d::new(-2.0, 2.0));
        assert_eq!(v.ceil(), Vec2d::new(-1.0, 3.0));
        assert_eq!(v.round(), Vec2d::new(-2.0, 3.0));
    }

    #[test]
    fn test_from_array() {
        let v: Vec2d = [1.0, 2.0].into();
        assert_eq!(v, Vec2d::new(1.0, 2.0));

        let arr: [f64; 2] = v.into();
        assert_eq!(arr, [1.0, 2.0]);
    }

    #[test]
    fn test_from_tuple() {
        let v: Vec2d = (1.0, 2.0).into();
        assert_eq!(v, Vec2d::new(1.0, 2.0));
    }

    #[test]
    fn test_display() {
        let v = Vec2d::new(1.0, 2.0);
        assert_eq!(format!("{}", v), "(1, 2)");
    }

    #[test]
    fn test_vec2i() {
        let v1 = Vec2i::new(1, 2);
        let v2 = Vec2i::new(3, 4);

        assert_eq!(v1 + v2, Vec2i::new(4, 6));
        assert_eq!(v2 - v1, Vec2i::new(2, 2));
        assert_eq!(v1 * 2, Vec2i::new(2, 4));
        assert_eq!(-v1, Vec2i::new(-1, -2));
        assert_eq!(v1.dot(&v2), 11);
    }

    #[test]
    fn test_helper_functions() {
        assert_eq!(vec2d(1.0, 2.0), Vec2d::new(1.0, 2.0));
        assert_eq!(vec2f(1.0, 2.0), Vec2f::new(1.0, 2.0));
        assert_eq!(vec2i(1, 2), Vec2i::new(1, 2));
    }

    #[test]
    fn test_is_close_boundary_le() {
        // H-vt-1: is_close uses <= so diff == eps should be true
        let v1 = Vec2d::new(1.0, 2.0);
        let v2 = Vec2d::new(1.5, 2.0);
        assert!(v1.is_close(&v2, 0.5)); // diff == eps => true with <=
        assert!(!v1.is_close(&v2, 0.49)); // diff > eps => false
    }
}
