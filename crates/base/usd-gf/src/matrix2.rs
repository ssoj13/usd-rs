//! 2x2 matrix types.
//!
//! This module provides Matrix2 types for 2x2 matrix math operations.
//! Matrices are stored in row-major order.
//!
//! # Examples
//!
//! ```
//! use usd_gf::matrix2::{Matrix2d, Matrix2f};
//! use usd_gf::vec2::Vec2d;
//!
//! // Create matrices
//! let m = Matrix2d::identity();
//! let scaled = Matrix2d::from_diagonal(2.0, 3.0);
//!
//! // Matrix operations
//! let product = m * scaled;
//! let inverse = scaled.inverse();
//!
//! // Matrix-vector multiplication
//! let v = Vec2d::new(1.0, 2.0);
//! let transformed = m * v;
//! ```

use crate::traits::{GfMatrix, Scalar};
use crate::vec2::Vec2;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2x2 matrix with scalar type `T`, stored in row-major order.
///
/// Matrix elements are accessed as `matrix[row][col]`.
///
/// # Examples
///
/// ```
/// use usd_gf::matrix2::Matrix2d;
///
/// let m = Matrix2d::identity();
/// assert_eq!(m[0][0], 1.0);
/// assert_eq!(m[0][1], 0.0);
/// assert_eq!(m[1][0], 0.0);
/// assert_eq!(m[1][1], 1.0);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Matrix2<T> {
    /// Matrix data in row-major order: [[row0_col0, row0_col1], [row1_col0, row1_col1]]
    data: [[T; 2]; 2],
}

// Mark Matrix2 as a gf matrix type
impl<T> GfMatrix for Matrix2<T> {}

/// Type alias for 2x2 double-precision matrix.
pub type Matrix2d = Matrix2<f64>;

/// Type alias for 2x2 single-precision matrix.
pub type Matrix2f = Matrix2<f32>;

impl<T: Scalar> Matrix2<T> {
    /// Number of rows in the matrix.
    pub const NUM_ROWS: usize = 2;
    /// Number of columns in the matrix.
    pub const NUM_COLS: usize = 2;

    /// Creates a new matrix from individual elements (row-major order).
    ///
    /// # Arguments
    ///
    /// * `m00` - Element at row 0, column 0
    /// * `m01` - Element at row 0, column 1
    /// * `m10` - Element at row 1, column 0
    /// * `m11` - Element at row 1, column 1
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(m[0][0], 1.0);
    /// assert_eq!(m[0][1], 2.0);
    /// assert_eq!(m[1][0], 3.0);
    /// assert_eq!(m[1][1], 4.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(m00: T, m01: T, m10: T, m11: T) -> Self {
        Self {
            data: [[m00, m01], [m10, m11]],
        }
    }

    /// Creates a matrix from a 2D array (row-major order).
    #[inline]
    #[must_use]
    pub fn from_array(data: [[T; 2]; 2]) -> Self {
        Self { data }
    }

    /// Creates a matrix from a pointer to a 2x2 array.
    ///
    /// Matches C++ `GfMatrix2d(const double m[2][2])` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 2x2 array of `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const [T; 2]) -> Self {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            Self {
                data: [[row0[0], row0[1]], [row1[0], row1[1]]],
            }
        }
    }

    /// Creates a matrix from a pointer to a flat array of 4 elements (row-major order).
    ///
    /// # Safety
    ///
    /// The pointer must point to at least 4 valid `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_flat_ptr(ptr: *const T) -> Self {
        unsafe {
            Self {
                data: [[*ptr, *ptr.add(1)], [*ptr.add(2), *ptr.add(3)]],
            }
        }
    }

    /// Sets the matrix from a pointer to a 2x2 array.
    ///
    /// Matches C++ `Set(const double m[2][2])` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 2x2 array of `T` values.
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const [T; 2]) {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            self.data[0][0] = row0[0];
            self.data[0][1] = row0[1];
            self.data[1][0] = row1[0];
            self.data[1][1] = row1[1];
        }
    }

    /// Creates the identity matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::identity();
    /// assert_eq!(m[0][0], 1.0);
    /// assert_eq!(m[1][1], 1.0);
    /// assert_eq!(m[0][1], 0.0);
    /// assert_eq!(m[1][0], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self::from_diagonal(T::ONE, T::ONE)
    }

    /// Creates a zero matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::zero();
    /// assert_eq!(m[0][0], 0.0);
    /// assert_eq!(m[0][1], 0.0);
    /// assert_eq!(m[1][0], 0.0);
    /// assert_eq!(m[1][1], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self::from_diagonal(T::ZERO, T::ZERO)
    }

    /// Creates a diagonal matrix with the given diagonal elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::from_diagonal(2.0, 3.0);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 3.0);
    /// assert_eq!(m[0][1], 0.0);
    /// assert_eq!(m[1][0], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_diagonal(d0: T, d1: T) -> Self {
        Self::new(d0, T::ZERO, T::ZERO, d1)
    }

    /// Creates a diagonal matrix from a vector.
    #[inline]
    #[must_use]
    pub fn from_diagonal_vec(v: &Vec2<T>) -> Self {
        Self::from_diagonal(v.x, v.y)
    }

    /// Creates a uniform scale matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::from_scale(2.0);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 2.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_scale(s: T) -> Self {
        Self::from_diagonal(s, s)
    }

    /// Creates a rotation matrix for the given angle (in radians).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    /// use std::f64::consts::PI;
    ///
    /// let m = Matrix2d::from_rotation(PI / 2.0);
    /// // 90 degree rotation: [[0, -1], [1, 0]]
    /// assert!((m[0][0]).abs() < 1e-10);
    /// assert!((m[0][1] + 1.0).abs() < 1e-10);
    /// assert!((m[1][0] - 1.0).abs() < 1e-10);
    /// assert!((m[1][1]).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_rotation(angle_radians: T) -> Self {
        let c = angle_radians.cos();
        let s = angle_radians.sin();
        Self::new(c, -s, s, c)
    }

    /// Sets all elements from individual values.
    #[inline]
    pub fn set(&mut self, m00: T, m01: T, m10: T, m11: T) {
        self.data = [[m00, m01], [m10, m11]];
    }

    /// Sets the matrix from a 2D array (row-major order).
    ///
    /// Matches C++ `Set(const double m[2][2])` method.
    #[inline]
    pub fn set_from_array(&mut self, arr: [[T; 2]; 2]) {
        self.data = arr;
    }

    /// Sets this matrix to the identity matrix.
    #[inline]
    pub fn set_identity(&mut self) {
        *self = Self::identity();
    }

    /// Sets this matrix to zero.
    #[inline]
    pub fn set_zero(&mut self) {
        *self = Self::zero();
    }

    /// Sets this matrix to a diagonal matrix.
    #[inline]
    pub fn set_diagonal(&mut self, d0: T, d1: T) {
        *self = Self::from_diagonal(d0, d1);
    }

    /// Gets a row of the matrix as a Vec2.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(m.row(0), Vec2d::new(1.0, 2.0));
    /// assert_eq!(m.row(1), Vec2d::new(3.0, 4.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn row(&self, i: usize) -> Vec2<T> {
        Vec2 {
            x: self.data[i][0],
            y: self.data[i][1],
        }
    }

    /// Gets a column of the matrix as a Vec2.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    /// use usd_gf::vec2::Vec2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(m.column(0), Vec2d::new(1.0, 3.0));
    /// assert_eq!(m.column(1), Vec2d::new(2.0, 4.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn column(&self, j: usize) -> Vec2<T> {
        Vec2 {
            x: self.data[0][j],
            y: self.data[1][j],
        }
    }

    /// Sets a row of the matrix from a Vec2.
    #[inline]
    pub fn set_row(&mut self, i: usize, v: &Vec2<T>) {
        self.data[i][0] = v.x;
        self.data[i][1] = v.y;
    }

    /// Sets a column of the matrix from a Vec2.
    #[inline]
    pub fn set_column(&mut self, j: usize, v: &Vec2<T>) {
        self.data[0][j] = v.x;
        self.data[1][j] = v.y;
    }

    /// Returns a pointer to the underlying data.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        self.data.as_ptr() as *const T
    }

    /// Returns a mutable pointer to the underlying data.
    #[inline]
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.data.as_mut_ptr() as *mut T
    }

    /// Returns the data as a flat slice (row-major order).
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: Matrix2 is repr(C) with contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 4) }
    }

    /// Returns the data as a 2D array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [[T; 2]; 2] {
        self.data
    }

    /// Returns the transpose of the matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// let t = m.transpose();
    /// assert_eq!(t[0][0], 1.0);
    /// assert_eq!(t[0][1], 3.0);
    /// assert_eq!(t[1][0], 2.0);
    /// assert_eq!(t[1][1], 4.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn transpose(&self) -> Self {
        Self::new(
            self.data[0][0],
            self.data[1][0],
            self.data[0][1],
            self.data[1][1],
        )
    }

    /// Returns the determinant of the matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// assert!((m.determinant() - (-2.0)).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn determinant(&self) -> T {
        self.data[0][0] * self.data[1][1] - self.data[0][1] * self.data[1][0]
    }

    /// Returns the inverse of the matrix, or None if singular.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix2::Matrix2d;
    ///
    /// let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
    /// let inv = m.inverse().unwrap();
    /// let product = m * inv;
    /// // Should be close to identity
    /// assert!((product[0][0] - 1.0).abs() < 1e-10);
    /// assert!((product[1][1] - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        self.inverse_with_eps(T::EPSILON)
    }

    /// Returns the inverse with custom epsilon for singularity check.
    #[inline]
    #[must_use]
    pub fn inverse_with_eps(&self, eps: T) -> Option<Self> {
        let det = self.determinant();
        if det.abs() <= eps {
            return None;
        }
        let inv_det = T::ONE / det;
        Some(Self::new(
            self.data[1][1] * inv_det,
            -self.data[0][1] * inv_det,
            -self.data[1][0] * inv_det,
            self.data[0][0] * inv_det,
        ))
    }

    /// Returns the inverse and determinant, or None if singular.
    #[inline]
    #[must_use]
    pub fn inverse_and_det(&self) -> Option<(Self, T)> {
        self.inverse_and_det_with_eps(T::EPSILON)
    }

    /// Returns the inverse and determinant with custom epsilon.
    #[inline]
    #[must_use]
    pub fn inverse_and_det_with_eps(&self, eps: T) -> Option<(Self, T)> {
        let det = self.determinant();
        if det.abs() <= eps {
            return None;
        }
        let inv_det = T::ONE / det;
        Some((
            Self::new(
                self.data[1][1] * inv_det,
                -self.data[0][1] * inv_det,
                -self.data[1][0] * inv_det,
                self.data[0][0] * inv_det,
            ),
            det,
        ))
    }

    /// Returns whether this matrix is approximately equal to another.
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        (self.data[0][0] - other.data[0][0]).abs() < eps
            && (self.data[0][1] - other.data[0][1]).abs() < eps
            && (self.data[1][0] - other.data[1][0]).abs() < eps
            && (self.data[1][1] - other.data[1][1]).abs() < eps
    }

    /// Returns the diagonal elements as a vector.
    #[inline]
    #[must_use]
    pub fn diagonal(&self) -> Vec2<T> {
        Vec2 {
            x: self.data[0][0],
            y: self.data[1][1],
        }
    }

    /// Returns the trace (sum of diagonal elements).
    #[inline]
    #[must_use]
    pub fn trace(&self) -> T {
        self.data[0][0] + self.data[1][1]
    }
}

// Default - identity matrix
impl<T: Scalar> Default for Matrix2<T> {
    fn default() -> Self {
        Self::identity()
    }
}

// Indexing by row
impl<T> Index<usize> for Matrix2<T> {
    type Output = [T; 2];

    #[inline]
    fn index(&self, i: usize) -> &[T; 2] {
        &self.data[i]
    }
}

impl<T> IndexMut<usize> for Matrix2<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut [T; 2] {
        &mut self.data[i]
    }
}

// Equality
impl<T: PartialEq> PartialEq for Matrix2<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T: Eq> Eq for Matrix2<T> {}

// Hash
impl<T: Hash> Hash for Matrix2<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data[0][0].hash(state);
        self.data[0][1].hash(state);
        self.data[1][0].hash(state);
        self.data[1][1].hash(state);
    }
}

// Negation
impl<T: Scalar> Neg for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(
            -self.data[0][0],
            -self.data[0][1],
            -self.data[1][0],
            -self.data[1][1],
        )
    }
}

// Matrix addition
impl<T: Scalar> Add for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] + rhs.data[0][0],
            self.data[0][1] + rhs.data[0][1],
            self.data[1][0] + rhs.data[1][0],
            self.data[1][1] + rhs.data[1][1],
        )
    }
}

impl<T: Scalar> AddAssign for Matrix2<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.data[0][0] = self.data[0][0] + rhs.data[0][0];
        self.data[0][1] = self.data[0][1] + rhs.data[0][1];
        self.data[1][0] = self.data[1][0] + rhs.data[1][0];
        self.data[1][1] = self.data[1][1] + rhs.data[1][1];
    }
}

// Matrix subtraction
impl<T: Scalar> Sub for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] - rhs.data[0][0],
            self.data[0][1] - rhs.data[0][1],
            self.data[1][0] - rhs.data[1][0],
            self.data[1][1] - rhs.data[1][1],
        )
    }
}

impl<T: Scalar> SubAssign for Matrix2<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.data[0][0] = self.data[0][0] - rhs.data[0][0];
        self.data[0][1] = self.data[0][1] - rhs.data[0][1];
        self.data[1][0] = self.data[1][0] - rhs.data[1][0];
        self.data[1][1] = self.data[1][1] - rhs.data[1][1];
    }
}

// Matrix-matrix multiplication
impl<T: Scalar> Mul for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] * rhs.data[0][0] + self.data[0][1] * rhs.data[1][0],
            self.data[0][0] * rhs.data[0][1] + self.data[0][1] * rhs.data[1][1],
            self.data[1][0] * rhs.data[0][0] + self.data[1][1] * rhs.data[1][0],
            self.data[1][0] * rhs.data[0][1] + self.data[1][1] * rhs.data[1][1],
        )
    }
}

impl<T: Scalar> MulAssign for Matrix2<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(
            self.data[0][0] * rhs,
            self.data[0][1] * rhs,
            self.data[1][0] * rhs,
            self.data[1][1] * rhs,
        )
    }
}

impl<T: Scalar> MulAssign<T> for Matrix2<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        self.data[0][0] = self.data[0][0] * rhs;
        self.data[0][1] = self.data[0][1] * rhs;
        self.data[1][0] = self.data[1][0] * rhs;
        self.data[1][1] = self.data[1][1] * rhs;
    }
}

// Scalar on left
impl Mul<Matrix2<f64>> for f64 {
    type Output = Matrix2<f64>;

    #[inline]
    fn mul(self, rhs: Matrix2<f64>) -> Matrix2<f64> {
        rhs * self
    }
}

impl Mul<Matrix2<f32>> for f32 {
    type Output = Matrix2<f32>;

    #[inline]
    fn mul(self, rhs: Matrix2<f32>) -> Matrix2<f32> {
        rhs * self
    }
}

// Matrix-vector multiplication (M * v = column vector result)
impl<T: Scalar> Mul<Vec2<T>> for Matrix2<T> {
    type Output = Vec2<T>;

    #[inline]
    fn mul(self, v: Vec2<T>) -> Vec2<T> {
        Vec2 {
            x: self.data[0][0] * v.x + self.data[0][1] * v.y,
            y: self.data[1][0] * v.x + self.data[1][1] * v.y,
        }
    }
}

// Vector-matrix multiplication (v * M = row vector result)
impl Mul<Matrix2<f64>> for Vec2<f64> {
    type Output = Vec2<f64>;

    #[inline]
    fn mul(self, m: Matrix2<f64>) -> Vec2<f64> {
        Vec2 {
            x: self.x * m.data[0][0] + self.y * m.data[1][0],
            y: self.x * m.data[0][1] + self.y * m.data[1][1],
        }
    }
}

impl Mul<Matrix2<f32>> for Vec2<f32> {
    type Output = Vec2<f32>;

    #[inline]
    fn mul(self, m: Matrix2<f32>) -> Vec2<f32> {
        Vec2 {
            x: self.x * m.data[0][0] + self.y * m.data[1][0],
            y: self.x * m.data[0][1] + self.y * m.data[1][1],
        }
    }
}

// Matrix division (m1 / m2 = m1 * inverse(m2))
// Note: Using * in Div impl is correct for matrix division
#[allow(clippy::suspicious_arithmetic_impl)]
impl<T: Scalar> Div for Matrix2<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inverse().expect("Cannot divide by singular matrix")
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Matrix2<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[[{}, {}], [{}, {}]]",
            self.data[0][0], self.data[0][1], self.data[1][0], self.data[1][1]
        )
    }
}

/// Creates a Matrix2d from elements.
#[inline]
#[must_use]
pub fn matrix2d(m00: f64, m01: f64, m10: f64, m11: f64) -> Matrix2d {
    Matrix2d::new(m00, m01, m10, m11)
}

/// Creates a Matrix2f from elements.
#[inline]
#[must_use]
pub fn matrix2f(m00: f32, m01: f32, m10: f32, m11: f32) -> Matrix2f {
    Matrix2f::new(m00, m01, m10, m11)
}

// Cross-type conversions (matching C++ explicit conversions)
impl From<Matrix2f> for Matrix2d {
    fn from(other: Matrix2f) -> Self {
        Self::new(
            other[0][0] as f64,
            other[0][1] as f64,
            other[1][0] as f64,
            other[1][1] as f64,
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Matrix2f> for Matrix2d {
    fn eq(&self, other: &Matrix2f) -> bool {
        (self[0][0] - other[0][0] as f64).abs() < f64::EPSILON
            && (self[0][1] - other[0][1] as f64).abs() < f64::EPSILON
            && (self[1][0] - other[1][0] as f64).abs() < f64::EPSILON
            && (self[1][1] - other[1][1] as f64).abs() < f64::EPSILON
    }
}

// Global function (matching C++ GfIsClose)
/// Tests for equality within a given tolerance.
///
/// Matches C++ `GfIsClose(GfMatrix2d const &m1, GfMatrix2d const &m2, double tolerance)`.
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(m1: &Matrix2<T>, m2: &Matrix2<T>, tolerance: T) -> bool {
    m1.is_close(m2, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec2::Vec2d;
    use std::f64::consts::PI;

    #[test]
    fn test_new() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[0][1], 2.0);
        assert_eq!(m[1][0], 3.0);
        assert_eq!(m[1][1], 4.0);
    }

    #[test]
    fn test_identity() {
        let m = Matrix2d::identity();
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[0][1], 0.0);
        assert_eq!(m[1][0], 0.0);
        assert_eq!(m[1][1], 1.0);
    }

    #[test]
    fn test_zero() {
        let m = Matrix2d::zero();
        assert_eq!(m[0][0], 0.0);
        assert_eq!(m[0][1], 0.0);
        assert_eq!(m[1][0], 0.0);
        assert_eq!(m[1][1], 0.0);
    }

    #[test]
    fn test_from_diagonal() {
        let m = Matrix2d::from_diagonal(2.0, 3.0);
        assert_eq!(m[0][0], 2.0);
        assert_eq!(m[0][1], 0.0);
        assert_eq!(m[1][0], 0.0);
        assert_eq!(m[1][1], 3.0);
    }

    #[test]
    fn test_from_rotation() {
        let m = Matrix2d::from_rotation(PI / 2.0);
        // 90 degree rotation: [[0, -1], [1, 0]]
        assert!(m[0][0].abs() < 1e-10);
        assert!((m[0][1] + 1.0).abs() < 1e-10);
        assert!((m[1][0] - 1.0).abs() < 1e-10);
        assert!(m[1][1].abs() < 1e-10);
    }

    #[test]
    fn test_row_column() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(m.row(0), Vec2d::new(1.0, 2.0));
        assert_eq!(m.row(1), Vec2d::new(3.0, 4.0));
        assert_eq!(m.column(0), Vec2d::new(1.0, 3.0));
        assert_eq!(m.column(1), Vec2d::new(2.0, 4.0));
    }

    #[test]
    fn test_transpose() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let t = m.transpose();
        assert_eq!(t[0][0], 1.0);
        assert_eq!(t[0][1], 3.0);
        assert_eq!(t[1][0], 2.0);
        assert_eq!(t[1][1], 4.0);
    }

    #[test]
    fn test_determinant() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        assert!((m.determinant() - (-2.0)).abs() < 1e-10);

        let identity = Matrix2d::identity();
        assert!((identity.determinant() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;

        // Should be close to identity
        assert!((product[0][0] - 1.0).abs() < 1e-10);
        assert!((product[0][1]).abs() < 1e-10);
        assert!((product[1][0]).abs() < 1e-10);
        assert!((product[1][1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_singular_inverse() {
        let m = Matrix2d::new(1.0, 2.0, 2.0, 4.0); // Determinant = 0
        assert!(m.inverse().is_none());
    }

    #[test]
    fn test_matrix_addition() {
        let m1 = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let m2 = Matrix2d::new(5.0, 6.0, 7.0, 8.0);
        let sum = m1 + m2;
        assert_eq!(sum, Matrix2d::new(6.0, 8.0, 10.0, 12.0));
    }

    #[test]
    fn test_matrix_subtraction() {
        let m1 = Matrix2d::new(5.0, 6.0, 7.0, 8.0);
        let m2 = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let diff = m1 - m2;
        assert_eq!(diff, Matrix2d::new(4.0, 4.0, 4.0, 4.0));
    }

    #[test]
    fn test_matrix_multiplication() {
        let m1 = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let m2 = Matrix2d::new(5.0, 6.0, 7.0, 8.0);
        let product = m1 * m2;
        // [1,2] * [5,6]   = [1*5+2*7, 1*6+2*8] = [19, 22]
        // [3,4]   [7,8]     [3*5+4*7, 3*6+4*8]   [43, 50]
        assert_eq!(product, Matrix2d::new(19.0, 22.0, 43.0, 50.0));
    }

    #[test]
    fn test_scalar_multiplication() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let scaled = m * 2.0;
        assert_eq!(scaled, Matrix2d::new(2.0, 4.0, 6.0, 8.0));

        // Scalar on left
        let scaled2 = 3.0 * m;
        assert_eq!(scaled2, Matrix2d::new(3.0, 6.0, 9.0, 12.0));
    }

    #[test]
    fn test_matrix_vector_multiplication() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let v = Vec2d::new(5.0, 6.0);

        // Matrix * vector (column vector)
        let result = m * v;
        // [1,2] * [5] = [1*5+2*6] = [17]
        // [3,4]   [6]   [3*5+4*6]   [39]
        assert_eq!(result, Vec2d::new(17.0, 39.0));
    }

    #[test]
    fn test_vector_matrix_multiplication() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let v = Vec2d::new(5.0, 6.0);

        // Vector * matrix (row vector)
        let result = v * m;
        // [5,6] * [1,2] = [5*1+6*3, 5*2+6*4] = [23, 34]
        //         [3,4]
        assert_eq!(result, Vec2d::new(23.0, 34.0));
    }

    #[test]
    fn test_negation() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let neg = -m;
        assert_eq!(neg, Matrix2d::new(-1.0, -2.0, -3.0, -4.0));
    }

    #[test]
    fn test_diagonal_and_trace() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(m.diagonal(), Vec2d::new(1.0, 4.0));
        assert!((m.trace() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_close() {
        let m1 = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let m2 = Matrix2d::new(1.0 + 1e-10, 2.0, 3.0, 4.0);
        assert!(m1.is_close(&m2, 1e-9));
        assert!(!m1.is_close(&m2, 1e-11));
    }

    #[test]
    fn test_helper_functions() {
        assert_eq!(
            matrix2d(1.0, 2.0, 3.0, 4.0),
            Matrix2d::new(1.0, 2.0, 3.0, 4.0)
        );
        assert_eq!(
            matrix2f(1.0, 2.0, 3.0, 4.0),
            Matrix2f::new(1.0, 2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn test_display() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(format!("{}", m), "[[1, 2], [3, 4]]");
    }

    #[test]
    fn test_identity_multiplication() {
        let m = Matrix2d::new(1.0, 2.0, 3.0, 4.0);
        let i = Matrix2d::identity();

        assert_eq!(m * i, m);
        assert_eq!(i * m, m);
    }

    #[test]
    fn test_rotation_inverse() {
        let angle = PI / 4.0;
        let rot = Matrix2d::from_rotation(angle);
        let inv = rot.inverse().unwrap();
        let rot_neg = Matrix2d::from_rotation(-angle);

        // Inverse of rotation should be rotation by negative angle
        assert!(rot_neg.is_close(&inv, 1e-10));
    }
}
