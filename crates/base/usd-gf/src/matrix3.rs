//! 3x3 matrix types.
//!
//! This module provides Matrix3 types for 3x3 matrix math operations.
//! Matrices are stored in row-major order.
//!
//! # 3D Transformations
//!
//! Matrix3 can represent rotations and scales in 3D space. Vectors are
//! treated as row vectors by convention:
//!
//! - Transformation matrices work with row vectors
//! - When multiplying matrices, the left matrix applies a more local transform
//! - Example: R * S rotates a row vector, then scales it
//!
//! # Examples
//!
//! ```
//! use usd_gf::matrix3::{Matrix3d, Matrix3f};
//! use usd_gf::vec3::Vec3d;
//!
//! // Create matrices
//! let m = Matrix3d::identity();
//! let scaled = Matrix3d::from_scale(2.0);
//!
//! // Matrix operations
//! let product = m * scaled;
//! let inverse = scaled.inverse();
//!
//! // Matrix-vector multiplication
//! let v = Vec3d::new(1.0, 2.0, 3.0);
//! let transformed = m * v;
//! ```

use crate::limits::MIN_VECTOR_LENGTH;
use crate::traits::{GfMatrix, Scalar};
use crate::vec3::Vec3;
use num_traits::Float;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 3x3 matrix with scalar type `T`, stored in row-major order.
///
/// Matrix elements are accessed as `matrix[row][col]`.
///
/// # Examples
///
/// ```
/// use usd_gf::matrix3::Matrix3d;
///
/// let m = Matrix3d::identity();
/// assert_eq!(m[0][0], 1.0);
/// assert_eq!(m[1][1], 1.0);
/// assert_eq!(m[2][2], 1.0);
/// assert_eq!(m[0][1], 0.0);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Matrix3<T> {
    /// Matrix data in row-major order.
    data: [[T; 3]; 3],
}

// Mark Matrix3 as a gf matrix type
impl<T> GfMatrix for Matrix3<T> {}

/// Type alias for 3x3 double-precision matrix.
pub type Matrix3d = Matrix3<f64>;

/// Type alias for 3x3 single-precision matrix.
pub type Matrix3f = Matrix3<f32>;

impl<T: Scalar> Matrix3<T> {
    /// Number of rows in the matrix.
    pub const NUM_ROWS: usize = 3;
    /// Number of columns in the matrix.
    pub const NUM_COLS: usize = 3;

    /// Creates a new matrix from individual elements (row-major order).
    ///
    /// # Arguments
    ///
    /// * `m00..m22` - Elements in row-major order (mRC = row R, column C)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::new(
    ///     1.0, 2.0, 3.0,
    ///     4.0, 5.0, 6.0,
    ///     7.0, 8.0, 9.0,
    /// );
    /// assert_eq!(m[0][0], 1.0);
    /// assert_eq!(m[0][1], 2.0);
    /// assert_eq!(m[2][2], 9.0);
    /// ```
    #[inline]
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(m00: T, m01: T, m02: T, m10: T, m11: T, m12: T, m20: T, m21: T, m22: T) -> Self {
        Self {
            data: [[m00, m01, m02], [m10, m11, m12], [m20, m21, m22]],
        }
    }

    /// Creates a matrix from a 2D array (row-major order).
    #[inline]
    #[must_use]
    pub fn from_array(data: [[T; 3]; 3]) -> Self {
        Self { data }
    }

    /// Creates a matrix from a pointer to a 3x3 array.
    ///
    /// Matches C++ `GfMatrix3d(const double m[3][3])` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 3x3 array of `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const [T; 3]) -> Self {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            let row2 = *ptr.add(2);
            Self {
                data: [
                    [row0[0], row0[1], row0[2]],
                    [row1[0], row1[1], row1[2]],
                    [row2[0], row2[1], row2[2]],
                ],
            }
        }
    }

    /// Sets the matrix from a pointer to a 3x3 array.
    ///
    /// Matches C++ `Set(const double m[3][3])` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 3x3 array of `T` values.
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const [T; 3]) {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            let row2 = *ptr.add(2);
            self.data[0] = row0;
            self.data[1] = row1;
            self.data[2] = row2;
        }
    }

    /// Sets the matrix from a 2D array (row-major order).
    ///
    /// Matches C++ `Set(const double m[3][3])` method.
    #[inline]
    pub fn set_from_array(&mut self, arr: [[T; 3]; 3]) {
        self.data = arr;
    }

    /// Creates the identity matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::identity();
    /// assert_eq!(m[0][0], 1.0);
    /// assert_eq!(m[1][1], 1.0);
    /// assert_eq!(m[2][2], 1.0);
    /// assert_eq!(m[0][1], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self::from_diagonal_values(T::ONE, T::ONE, T::ONE)
    }

    /// Creates a zero matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::zero();
    /// assert_eq!(m[0][0], 0.0);
    /// assert_eq!(m[1][1], 0.0);
    /// assert_eq!(m[2][2], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self::from_diagonal_values(T::ZERO, T::ZERO, T::ZERO)
    }

    /// Creates a diagonal matrix with the given diagonal elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::from_diagonal_values(2.0, 3.0, 4.0);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 3.0);
    /// assert_eq!(m[2][2], 4.0);
    /// assert_eq!(m[0][1], 0.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_diagonal_values(d0: T, d1: T, d2: T) -> Self {
        Self::new(
            d0,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            d1,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            d2,
        )
    }

    /// Creates a diagonal matrix from a vector.
    #[inline]
    #[must_use]
    pub fn from_diagonal_vec(v: &Vec3<T>) -> Self {
        Self::from_diagonal_values(v.x, v.y, v.z)
    }

    /// Creates a uniform scale matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::from_scale(2.0);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 2.0);
    /// assert_eq!(m[2][2], 2.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_scale(s: T) -> Self {
        Self::from_diagonal_values(s, s, s)
    }

    /// Creates a non-uniform scale matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let scale = Vec3d::new(2.0, 3.0, 4.0);
    /// let m = Matrix3d::from_scale_vec(&scale);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 3.0);
    /// assert_eq!(m[2][2], 4.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_scale_vec(scale: &Vec3<T>) -> Self {
        Self::from_diagonal_values(scale.x, scale.y, scale.z)
    }

    /// Sets all elements from individual values.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn set(&mut self, m00: T, m01: T, m02: T, m10: T, m11: T, m12: T, m20: T, m21: T, m22: T) {
        self.data = [[m00, m01, m02], [m10, m11, m12], [m20, m21, m22]];
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
    pub fn set_diagonal(&mut self, d0: T, d1: T, d2: T) {
        *self = Self::from_diagonal_values(d0, d1, d2);
    }

    /// Gets a row of the matrix as a Vec3.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix3d::new(
    ///     1.0, 2.0, 3.0,
    ///     4.0, 5.0, 6.0,
    ///     7.0, 8.0, 9.0,
    /// );
    /// assert_eq!(m.row(0), Vec3d::new(1.0, 2.0, 3.0));
    /// assert_eq!(m.row(1), Vec3d::new(4.0, 5.0, 6.0));
    /// assert_eq!(m.row(2), Vec3d::new(7.0, 8.0, 9.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn row(&self, i: usize) -> Vec3<T> {
        Vec3 {
            x: self.data[i][0],
            y: self.data[i][1],
            z: self.data[i][2],
        }
    }

    /// Gets a column of the matrix as a Vec3.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix3d::new(
    ///     1.0, 2.0, 3.0,
    ///     4.0, 5.0, 6.0,
    ///     7.0, 8.0, 9.0,
    /// );
    /// assert_eq!(m.column(0), Vec3d::new(1.0, 4.0, 7.0));
    /// assert_eq!(m.column(1), Vec3d::new(2.0, 5.0, 8.0));
    /// assert_eq!(m.column(2), Vec3d::new(3.0, 6.0, 9.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn column(&self, j: usize) -> Vec3<T> {
        Vec3 {
            x: self.data[0][j],
            y: self.data[1][j],
            z: self.data[2][j],
        }
    }

    /// Sets a row of the matrix from a Vec3.
    #[inline]
    pub fn set_row(&mut self, i: usize, v: &Vec3<T>) {
        self.data[i][0] = v.x;
        self.data[i][1] = v.y;
        self.data[i][2] = v.z;
    }

    /// Sets a column of the matrix from a Vec3.
    #[inline]
    pub fn set_column(&mut self, j: usize, v: &Vec3<T>) {
        self.data[0][j] = v.x;
        self.data[1][j] = v.y;
        self.data[2][j] = v.z;
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
        // SAFETY: Matrix3 is repr(C) with contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 9) }
    }

    /// Returns the data as a 2D array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [[T; 3]; 3] {
        self.data
    }

    /// Returns the transpose of the matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::new(
    ///     1.0, 2.0, 3.0,
    ///     4.0, 5.0, 6.0,
    ///     7.0, 8.0, 9.0,
    /// );
    /// let t = m.transpose();
    /// assert_eq!(t[0][1], 4.0);
    /// assert_eq!(t[1][0], 2.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn transpose(&self) -> Self {
        Self::new(
            self.data[0][0],
            self.data[1][0],
            self.data[2][0],
            self.data[0][1],
            self.data[1][1],
            self.data[2][1],
            self.data[0][2],
            self.data[1][2],
            self.data[2][2],
        )
    }

    /// Returns the determinant of the matrix.
    ///
    /// Uses the rule of Sarrus (cofactor expansion along first row).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::identity();
    /// assert!((m.determinant() - 1.0).abs() < 1e-10);
    ///
    /// let m2 = Matrix3d::from_scale(2.0);
    /// assert!((m2.determinant() - 8.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn determinant(&self) -> T {
        // Cofactor expansion along first row:
        // det = a00*(a11*a22 - a12*a21) - a01*(a10*a22 - a12*a20) + a02*(a10*a21 - a11*a20)
        let a00 = self.data[0][0];
        let a01 = self.data[0][1];
        let a02 = self.data[0][2];
        let a10 = self.data[1][0];
        let a11 = self.data[1][1];
        let a12 = self.data[1][2];
        let a20 = self.data[2][0];
        let a21 = self.data[2][1];
        let a22 = self.data[2][2];

        a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
            + a02 * (a10 * a21 - a11 * a20)
    }

    /// Returns the inverse of the matrix, or None if singular.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let m = Matrix3d::new(
    ///     1.0, 0.0, 0.0,
    ///     0.0, 2.0, 0.0,
    ///     0.0, 0.0, 4.0,
    /// );
    /// let inv = m.inverse().unwrap();
    /// let product = m * inv;
    /// // Should be close to identity
    /// assert!((product[0][0] - 1.0).abs() < 1e-10);
    /// assert!((product[1][1] - 1.0).abs() < 1e-10);
    /// assert!((product[2][2] - 1.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        self.inverse_with_eps(T::EPSILON)
    }

    /// Returns the inverse with custom epsilon for singularity check.
    #[must_use]
    pub fn inverse_with_eps(&self, eps: T) -> Option<Self> {
        let det = self.determinant();
        if det.abs() <= eps {
            return None;
        }

        let a00 = self.data[0][0];
        let a01 = self.data[0][1];
        let a02 = self.data[0][2];
        let a10 = self.data[1][0];
        let a11 = self.data[1][1];
        let a12 = self.data[1][2];
        let a20 = self.data[2][0];
        let a21 = self.data[2][1];
        let a22 = self.data[2][2];

        // Compute cofactors (adjugate matrix elements)
        let c00 = a11 * a22 - a12 * a21;
        let c01 = -(a10 * a22 - a12 * a20);
        let c02 = a10 * a21 - a11 * a20;
        let c10 = -(a01 * a22 - a02 * a21);
        let c11 = a00 * a22 - a02 * a20;
        let c12 = -(a00 * a21 - a01 * a20);
        let c20 = a01 * a12 - a02 * a11;
        let c21 = -(a00 * a12 - a02 * a10);
        let c22 = a00 * a11 - a01 * a10;

        let inv_det = T::ONE / det;

        // Inverse = adjugate / det = transpose(cofactors) / det
        Some(Self::new(
            c00 * inv_det,
            c10 * inv_det,
            c20 * inv_det,
            c01 * inv_det,
            c11 * inv_det,
            c21 * inv_det,
            c02 * inv_det,
            c12 * inv_det,
            c22 * inv_det,
        ))
    }

    /// Returns the inverse and determinant, or None if singular.
    #[inline]
    #[must_use]
    pub fn inverse_and_det(&self) -> Option<(Self, T)> {
        self.inverse_and_det_with_eps(T::EPSILON)
    }

    /// Returns the inverse and determinant with custom epsilon.
    #[must_use]
    pub fn inverse_and_det_with_eps(&self, eps: T) -> Option<(Self, T)> {
        let det = self.determinant();
        if det.abs() <= eps {
            return None;
        }

        let a00 = self.data[0][0];
        let a01 = self.data[0][1];
        let a02 = self.data[0][2];
        let a10 = self.data[1][0];
        let a11 = self.data[1][1];
        let a12 = self.data[1][2];
        let a20 = self.data[2][0];
        let a21 = self.data[2][1];
        let a22 = self.data[2][2];

        // Compute cofactors
        let c00 = a11 * a22 - a12 * a21;
        let c01 = -(a10 * a22 - a12 * a20);
        let c02 = a10 * a21 - a11 * a20;
        let c10 = -(a01 * a22 - a02 * a21);
        let c11 = a00 * a22 - a02 * a20;
        let c12 = -(a00 * a21 - a01 * a20);
        let c20 = a01 * a12 - a02 * a11;
        let c21 = -(a00 * a12 - a02 * a10);
        let c22 = a00 * a11 - a01 * a10;

        let inv_det = T::ONE / det;

        Some((
            Self::new(
                c00 * inv_det,
                c10 * inv_det,
                c20 * inv_det,
                c01 * inv_det,
                c11 * inv_det,
                c21 * inv_det,
                c02 * inv_det,
                c12 * inv_det,
                c22 * inv_det,
            ),
            det,
        ))
    }

    /// Returns whether this matrix is approximately equal to another.
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        for i in 0..3 {
            for j in 0..3 {
                if (self.data[i][j] - other.data[i][j]).abs() >= eps {
                    return false;
                }
            }
        }
        true
    }

    /// Returns the diagonal elements as a vector.
    #[inline]
    #[must_use]
    pub fn diagonal(&self) -> Vec3<T> {
        Vec3 {
            x: self.data[0][0],
            y: self.data[1][1],
            z: self.data[2][2],
        }
    }

    /// Returns the trace (sum of diagonal elements).
    #[inline]
    #[must_use]
    pub fn trace(&self) -> T {
        self.data[0][0] + self.data[1][1] + self.data[2][2]
    }

    /// Returns the sign of the determinant.
    ///
    /// Returns 1 for right-handed, -1 for left-handed, 0 for singular.
    #[inline]
    #[must_use]
    pub fn handedness(&self) -> T {
        let det = self.determinant();
        if det > T::ZERO {
            T::ONE
        } else if det < T::ZERO {
            -T::ONE
        } else {
            T::ZERO
        }
    }

    /// Returns true if the matrix forms a right-handed coordinate system.
    #[inline]
    #[must_use]
    pub fn is_right_handed(&self) -> bool {
        self.handedness() == T::ONE
    }

    /// Returns true if the matrix forms a left-handed coordinate system.
    #[inline]
    #[must_use]
    pub fn is_left_handed(&self) -> bool {
        self.handedness() == -T::ONE
    }
}

/// Methods specific to floating-point matrices.
impl<T: Scalar + Float> Matrix3<T> {
    /// Creates a rotation matrix around the given axis by the given angle (radians).
    ///
    /// Uses Rodrigues' rotation formula.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// // 90 degree rotation around Z axis
    /// let rot = Matrix3d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// // Rotating X axis should give Y axis
    /// let x = Vec3d::new(1.0, 0.0, 0.0);
    /// let result = rot * x;
    /// assert!((result.x).abs() < 1e-10);
    /// assert!((result.y - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_rotation(axis: Vec3<T>, angle_radians: T) -> Self {
        let len_sq = axis.x * axis.x + axis.y * axis.y + axis.z * axis.z;
        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);

        if len_sq < min_len * min_len {
            return Self::identity();
        }

        let len = len_sq.sqrt();
        let x = axis.x / len;
        let y = axis.y / len;
        let z = axis.z / len;

        let c = angle_radians.cos();
        let s = angle_radians.sin();
        let t = T::ONE - c;

        // Rodrigues' rotation formula (column-vector convention for M * v)
        Self::new(
            t * x * x + c,
            t * x * y - s * z,
            t * x * z + s * y,
            t * x * y + s * z,
            t * y * y + c,
            t * y * z - s * x,
            t * x * z - s * y,
            t * y * z + s * x,
            t * z * z + c,
        )
    }

    /// Makes the matrix orthonormal in place using Gram-Schmidt.
    ///
    /// Returns true if the iteration converged, false otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix3::Matrix3d;
    ///
    /// let mut m = Matrix3d::from_scale(2.0);
    /// m.orthonormalize();
    /// // After orthonormalization, should be identity
    /// assert!(m.is_close(&Matrix3d::identity(), 1e-10));
    /// ```
    pub fn orthonormalize(&mut self) -> bool {
        // Gram-Schmidt orthonormalization on rows
        let mut r0 = self.row(0);
        let mut r1 = self.row(1);
        let mut r2 = self.row(2);

        // Normalize first row
        let len0 = (r0.x * r0.x + r0.y * r0.y + r0.z * r0.z).sqrt();
        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);
        if len0 < min_len {
            return false;
        }
        r0.x /= len0;
        r0.y /= len0;
        r0.z /= len0;

        // Orthogonalize second row to first
        let dot01 = r1.x * r0.x + r1.y * r0.y + r1.z * r0.z;
        r1.x -= dot01 * r0.x;
        r1.y -= dot01 * r0.y;
        r1.z -= dot01 * r0.z;

        // Normalize second row
        let len1 = (r1.x * r1.x + r1.y * r1.y + r1.z * r1.z).sqrt();
        if len1 < min_len {
            return false;
        }
        r1.x /= len1;
        r1.y /= len1;
        r1.z /= len1;

        // Orthogonalize third row to first two
        let dot02 = r2.x * r0.x + r2.y * r0.y + r2.z * r0.z;
        let dot12 = r2.x * r1.x + r2.y * r1.y + r2.z * r1.z;
        r2.x -= dot02 * r0.x + dot12 * r1.x;
        r2.y -= dot02 * r0.y + dot12 * r1.y;
        r2.z -= dot02 * r0.z + dot12 * r1.z;

        // Normalize third row
        let len2 = (r2.x * r2.x + r2.y * r2.y + r2.z * r2.z).sqrt();
        if len2 < min_len {
            return false;
        }
        r2.x /= len2;
        r2.y /= len2;
        r2.z /= len2;

        self.set_row(0, &r0);
        self.set_row(1, &r1);
        self.set_row(2, &r2);

        true
    }

    /// Returns an orthonormalized copy of the matrix.
    #[must_use]
    pub fn orthonormalized(&self) -> Self {
        let mut result = *self;
        result.orthonormalize();
        result
    }

    /// Sets this matrix to the rotation specified by the quaternion.
    ///
    /// Matches C++ `GfMatrix3d::SetRotate(GfQuatd const &rot)`.
    pub fn set_rotate_quat(&mut self, rot: &crate::quat::Quat<T>) -> &mut Self {
        self.set_rotate_from_quat(rot.real(), rot.imaginary());
        self
    }

    /// Sets rotation from the real part and imaginary vector of a quaternion.
    fn set_rotate_from_quat(&mut self, r: T, i: &Vec3<T>) {
        let two = T::from(2.0).unwrap();
        self.data[0][0] = T::ONE - two * (i.y * i.y + i.z * i.z);
        self.data[0][1] = two * (i.x * i.y + i.z * r);
        self.data[0][2] = two * (i.z * i.x - i.y * r);

        self.data[1][0] = two * (i.x * i.y - i.z * r);
        self.data[1][1] = T::ONE - two * (i.z * i.z + i.x * i.x);
        self.data[1][2] = two * (i.y * i.z + i.x * r);

        self.data[2][0] = two * (i.z * i.x + i.y * r);
        self.data[2][1] = two * (i.y * i.z - i.x * r);
        self.data[2][2] = T::ONE - two * (i.y * i.y + i.x * i.x);
    }

    /// Creates a matrix from a quaternion rotation.
    ///
    /// Matches C++ `GfMatrix3d(GfQuatd const &rot)` constructor.
    #[must_use]
    pub fn from_quat(rot: &crate::quat::Quat<T>) -> Self {
        let mut m = Self::identity();
        m.set_rotate_quat(rot);
        m
    }

    /// Sets this matrix to a uniform scale.
    ///
    /// Matches C++ `GfMatrix3d::SetScale(double)`.
    pub fn set_scale_uniform(&mut self, s: T) -> &mut Self {
        self.data[0] = [s, T::ZERO, T::ZERO];
        self.data[1] = [T::ZERO, s, T::ZERO];
        self.data[2] = [T::ZERO, T::ZERO, s];
        self
    }

    /// Sets this matrix to a non-uniform scale.
    ///
    /// Matches C++ `GfMatrix3d::SetScale(GfVec3d const &)`.
    pub fn set_scale_nonuniform(&mut self, s: &Vec3<T>) -> &mut Self {
        self.data[0] = [s.x, T::ZERO, T::ZERO];
        self.data[1] = [T::ZERO, s.y, T::ZERO];
        self.data[2] = [T::ZERO, T::ZERO, s.z];
        self
    }

    /// Extracts a rotation quaternion from this matrix.
    ///
    /// Adapted from Open Inventor SbRotation::SetValue(SbMatrix).
    /// Matches C++ `GfMatrix3d::ExtractRotationQuaternion()`.
    #[must_use]
    pub fn extract_rotation_quaternion(&self) -> crate::quaternion::Quaternion {
        // Find largest diagonal element
        let m = &self.data;
        let i = if m[0][0].to_f64().unwrap() > m[1][1].to_f64().unwrap() {
            if m[0][0].to_f64().unwrap() > m[2][2].to_f64().unwrap() {
                0
            } else {
                2
            }
        } else {
            if m[1][1].to_f64().unwrap() > m[2][2].to_f64().unwrap() {
                1
            } else {
                2
            }
        };

        let m00 = m[0][0].to_f64().unwrap();
        let m11 = m[1][1].to_f64().unwrap();
        let m22 = m[2][2].to_f64().unwrap();
        let trace = m00 + m11 + m22;
        let mii = [m00, m11, m22][i];

        let (r, im) = if trace > mii {
            let r = 0.5 * (trace + 1.0).sqrt();
            let s = 1.0 / (4.0 * r);
            let im = crate::vec3::Vec3d::new(
                (m[1][2].to_f64().unwrap() - m[2][1].to_f64().unwrap()) * s,
                (m[2][0].to_f64().unwrap() - m[0][2].to_f64().unwrap()) * s,
                (m[0][1].to_f64().unwrap() - m[1][0].to_f64().unwrap()) * s,
            );
            (r, im)
        } else {
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            let q = 0.5
                * (m[i][i].to_f64().unwrap()
                    - m[j][j].to_f64().unwrap()
                    - m[k][k].to_f64().unwrap()
                    + 1.0)
                    .sqrt();
            let s = 1.0 / (4.0 * q);
            let mut im = crate::vec3::Vec3d::zero();
            im[i] = q;
            im[j] = (m[i][j].to_f64().unwrap() + m[j][i].to_f64().unwrap()) * s;
            im[k] = (m[k][i].to_f64().unwrap() + m[i][k].to_f64().unwrap()) * s;
            let r = (m[j][k].to_f64().unwrap() - m[k][j].to_f64().unwrap()) * s;
            (r, im)
        };

        crate::quaternion::Quaternion::new(r.clamp(-1.0, 1.0), im)
    }

    /// Extracts a Rotation from this matrix.
    ///
    /// Matches C++ `GfMatrix3d::ExtractRotation()`.
    #[must_use]
    pub fn extract_rotation(&self) -> crate::rotation::Rotation {
        crate::rotation::Rotation::from_quaternion(&self.extract_rotation_quaternion())
    }

    /// Decomposes the rotation into angles about three given axes.
    ///
    /// Matches C++ `GfMatrix3d::DecomposeRotation(axis0, axis1, axis2)`.
    #[must_use]
    pub fn decompose_rotation(&self, axis0: &Vec3<T>, axis1: &Vec3<T>, axis2: &Vec3<T>) -> Vec3<T> {
        let rot = self.extract_rotation();
        let ax0 = crate::vec3::Vec3d::new(
            axis0.x.to_f64().unwrap(),
            axis0.y.to_f64().unwrap(),
            axis0.z.to_f64().unwrap(),
        );
        let ax1 = crate::vec3::Vec3d::new(
            axis1.x.to_f64().unwrap(),
            axis1.y.to_f64().unwrap(),
            axis1.z.to_f64().unwrap(),
        );
        let ax2 = crate::vec3::Vec3d::new(
            axis2.x.to_f64().unwrap(),
            axis2.y.to_f64().unwrap(),
            axis2.z.to_f64().unwrap(),
        );
        let result = rot.decompose(&ax0, &ax1, &ax2);
        Vec3 {
            x: T::from(result.x).unwrap(),
            y: T::from(result.y).unwrap(),
            z: T::from(result.z).unwrap(),
        }
    }
}

// Default - identity matrix
impl<T: Scalar> Default for Matrix3<T> {
    fn default() -> Self {
        Self::identity()
    }
}

// Indexing by row
impl<T> Index<usize> for Matrix3<T> {
    type Output = [T; 3];

    #[inline]
    fn index(&self, i: usize) -> &[T; 3] {
        &self.data[i]
    }
}

impl<T> IndexMut<usize> for Matrix3<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut [T; 3] {
        &mut self.data[i]
    }
}

// Equality
impl<T: PartialEq> PartialEq for Matrix3<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T: Eq> Eq for Matrix3<T> {}

// Hash
impl<T: Hash> Hash for Matrix3<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for row in &self.data {
            for elem in row {
                elem.hash(state);
            }
        }
    }
}

// Negation
impl<T: Scalar> Neg for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(
            -self.data[0][0],
            -self.data[0][1],
            -self.data[0][2],
            -self.data[1][0],
            -self.data[1][1],
            -self.data[1][2],
            -self.data[2][0],
            -self.data[2][1],
            -self.data[2][2],
        )
    }
}

// Matrix addition
impl<T: Scalar> Add for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] + rhs.data[0][0],
            self.data[0][1] + rhs.data[0][1],
            self.data[0][2] + rhs.data[0][2],
            self.data[1][0] + rhs.data[1][0],
            self.data[1][1] + rhs.data[1][1],
            self.data[1][2] + rhs.data[1][2],
            self.data[2][0] + rhs.data[2][0],
            self.data[2][1] + rhs.data[2][1],
            self.data[2][2] + rhs.data[2][2],
        )
    }
}

impl<T: Scalar> AddAssign for Matrix3<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        for i in 0..3 {
            for j in 0..3 {
                self.data[i][j] = self.data[i][j] + rhs.data[i][j];
            }
        }
    }
}

// Matrix subtraction
impl<T: Scalar> Sub for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] - rhs.data[0][0],
            self.data[0][1] - rhs.data[0][1],
            self.data[0][2] - rhs.data[0][2],
            self.data[1][0] - rhs.data[1][0],
            self.data[1][1] - rhs.data[1][1],
            self.data[1][2] - rhs.data[1][2],
            self.data[2][0] - rhs.data[2][0],
            self.data[2][1] - rhs.data[2][1],
            self.data[2][2] - rhs.data[2][2],
        )
    }
}

impl<T: Scalar> SubAssign for Matrix3<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        for i in 0..3 {
            for j in 0..3 {
                self.data[i][j] = self.data[i][j] - rhs.data[i][j];
            }
        }
    }
}

// Matrix-matrix multiplication
impl<T: Scalar> Mul for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            // Row 0
            self.data[0][0] * rhs.data[0][0]
                + self.data[0][1] * rhs.data[1][0]
                + self.data[0][2] * rhs.data[2][0],
            self.data[0][0] * rhs.data[0][1]
                + self.data[0][1] * rhs.data[1][1]
                + self.data[0][2] * rhs.data[2][1],
            self.data[0][0] * rhs.data[0][2]
                + self.data[0][1] * rhs.data[1][2]
                + self.data[0][2] * rhs.data[2][2],
            // Row 1
            self.data[1][0] * rhs.data[0][0]
                + self.data[1][1] * rhs.data[1][0]
                + self.data[1][2] * rhs.data[2][0],
            self.data[1][0] * rhs.data[0][1]
                + self.data[1][1] * rhs.data[1][1]
                + self.data[1][2] * rhs.data[2][1],
            self.data[1][0] * rhs.data[0][2]
                + self.data[1][1] * rhs.data[1][2]
                + self.data[1][2] * rhs.data[2][2],
            // Row 2
            self.data[2][0] * rhs.data[0][0]
                + self.data[2][1] * rhs.data[1][0]
                + self.data[2][2] * rhs.data[2][0],
            self.data[2][0] * rhs.data[0][1]
                + self.data[2][1] * rhs.data[1][1]
                + self.data[2][2] * rhs.data[2][1],
            self.data[2][0] * rhs.data[0][2]
                + self.data[2][1] * rhs.data[1][2]
                + self.data[2][2] * rhs.data[2][2],
        )
    }
}

impl<T: Scalar> MulAssign for Matrix3<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(
            self.data[0][0] * rhs,
            self.data[0][1] * rhs,
            self.data[0][2] * rhs,
            self.data[1][0] * rhs,
            self.data[1][1] * rhs,
            self.data[1][2] * rhs,
            self.data[2][0] * rhs,
            self.data[2][1] * rhs,
            self.data[2][2] * rhs,
        )
    }
}

impl<T: Scalar> MulAssign<T> for Matrix3<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        for i in 0..3 {
            for j in 0..3 {
                self.data[i][j] = self.data[i][j] * rhs;
            }
        }
    }
}

// Scalar on left
impl Mul<Matrix3<f64>> for f64 {
    type Output = Matrix3<f64>;

    #[inline]
    fn mul(self, rhs: Matrix3<f64>) -> Matrix3<f64> {
        rhs * self
    }
}

impl Mul<Matrix3<f32>> for f32 {
    type Output = Matrix3<f32>;

    #[inline]
    fn mul(self, rhs: Matrix3<f32>) -> Matrix3<f32> {
        rhs * self
    }
}

// Matrix-vector multiplication (M * v = column vector result)
impl<T: Scalar> Mul<Vec3<T>> for Matrix3<T> {
    type Output = Vec3<T>;

    #[inline]
    fn mul(self, v: Vec3<T>) -> Vec3<T> {
        Vec3 {
            x: self.data[0][0] * v.x + self.data[0][1] * v.y + self.data[0][2] * v.z,
            y: self.data[1][0] * v.x + self.data[1][1] * v.y + self.data[1][2] * v.z,
            z: self.data[2][0] * v.x + self.data[2][1] * v.y + self.data[2][2] * v.z,
        }
    }
}

// Vector-matrix multiplication (v * M = row vector result)
impl Mul<Matrix3<f64>> for Vec3<f64> {
    type Output = Vec3<f64>;

    #[inline]
    fn mul(self, m: Matrix3<f64>) -> Vec3<f64> {
        Vec3 {
            x: self.x * m.data[0][0] + self.y * m.data[1][0] + self.z * m.data[2][0],
            y: self.x * m.data[0][1] + self.y * m.data[1][1] + self.z * m.data[2][1],
            z: self.x * m.data[0][2] + self.y * m.data[1][2] + self.z * m.data[2][2],
        }
    }
}

impl Mul<Matrix3<f32>> for Vec3<f32> {
    type Output = Vec3<f32>;

    #[inline]
    fn mul(self, m: Matrix3<f32>) -> Vec3<f32> {
        Vec3 {
            x: self.x * m.data[0][0] + self.y * m.data[1][0] + self.z * m.data[2][0],
            y: self.x * m.data[0][1] + self.y * m.data[1][1] + self.z * m.data[2][1],
            z: self.x * m.data[0][2] + self.y * m.data[1][2] + self.z * m.data[2][2],
        }
    }
}

// Matrix division (m1 / m2 = m1 * inverse(m2))
#[allow(clippy::suspicious_arithmetic_impl)]
impl<T: Scalar> Div for Matrix3<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inverse().expect("Cannot divide by singular matrix")
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Matrix3<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[[{}, {}, {}], [{}, {}, {}], [{}, {}, {}]]",
            self.data[0][0],
            self.data[0][1],
            self.data[0][2],
            self.data[1][0],
            self.data[1][1],
            self.data[1][2],
            self.data[2][0],
            self.data[2][1],
            self.data[2][2],
        )
    }
}

/// Creates a Matrix3d from elements (row-major order).
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matrix3d(
    m00: f64,
    m01: f64,
    m02: f64,
    m10: f64,
    m11: f64,
    m12: f64,
    m20: f64,
    m21: f64,
    m22: f64,
) -> Matrix3d {
    Matrix3d::new(m00, m01, m02, m10, m11, m12, m20, m21, m22)
}

/// Creates a Matrix3f from elements (row-major order).
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matrix3f(
    m00: f32,
    m01: f32,
    m02: f32,
    m10: f32,
    m11: f32,
    m12: f32,
    m20: f32,
    m21: f32,
    m22: f32,
) -> Matrix3f {
    Matrix3f::new(m00, m01, m02, m10, m11, m12, m20, m21, m22)
}

// Cross-type conversions (matching C++ explicit conversions)
impl From<Matrix3f> for Matrix3d {
    fn from(other: Matrix3f) -> Self {
        Self::new(
            other[0][0] as f64,
            other[0][1] as f64,
            other[0][2] as f64,
            other[1][0] as f64,
            other[1][1] as f64,
            other[1][2] as f64,
            other[2][0] as f64,
            other[2][1] as f64,
            other[2][2] as f64,
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Matrix3f> for Matrix3d {
    fn eq(&self, other: &Matrix3f) -> bool {
        (self[0][0] - other[0][0] as f64).abs() < f64::EPSILON
            && (self[0][1] - other[0][1] as f64).abs() < f64::EPSILON
            && (self[0][2] - other[0][2] as f64).abs() < f64::EPSILON
            && (self[1][0] - other[1][0] as f64).abs() < f64::EPSILON
            && (self[1][1] - other[1][1] as f64).abs() < f64::EPSILON
            && (self[1][2] - other[1][2] as f64).abs() < f64::EPSILON
            && (self[2][0] - other[2][0] as f64).abs() < f64::EPSILON
            && (self[2][1] - other[2][1] as f64).abs() < f64::EPSILON
            && (self[2][2] - other[2][2] as f64).abs() < f64::EPSILON
    }
}

// Global function (matching C++ GfIsClose)
/// Tests for equality within a given tolerance.
///
/// Matches C++ `GfIsClose(GfMatrix3d const &m1, GfMatrix3d const &m2, double tolerance)`.
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(m1: &Matrix3<T>, m2: &Matrix3<T>, tolerance: T) -> bool {
    m1.is_close(m2, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3d;
    use std::f64::consts::PI;

    #[test]
    fn test_new() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[0][1], 2.0);
        assert_eq!(m[0][2], 3.0);
        assert_eq!(m[1][0], 4.0);
        assert_eq!(m[2][2], 9.0);
    }

    #[test]
    fn test_identity() {
        let m = Matrix3d::identity();
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[1][1], 1.0);
        assert_eq!(m[2][2], 1.0);
        assert_eq!(m[0][1], 0.0);
        assert_eq!(m[0][2], 0.0);
        assert_eq!(m[1][0], 0.0);
    }

    #[test]
    fn test_zero() {
        let m = Matrix3d::zero();
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(m[i][j], 0.0);
            }
        }
    }

    #[test]
    fn test_from_diagonal() {
        let m = Matrix3d::from_diagonal_values(2.0, 3.0, 4.0);
        assert_eq!(m[0][0], 2.0);
        assert_eq!(m[1][1], 3.0);
        assert_eq!(m[2][2], 4.0);
        assert_eq!(m[0][1], 0.0);
        assert_eq!(m[1][2], 0.0);
    }

    #[test]
    fn test_row_column() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m.row(0), Vec3d::new(1.0, 2.0, 3.0));
        assert_eq!(m.row(1), Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(m.row(2), Vec3d::new(7.0, 8.0, 9.0));
        assert_eq!(m.column(0), Vec3d::new(1.0, 4.0, 7.0));
        assert_eq!(m.column(1), Vec3d::new(2.0, 5.0, 8.0));
        assert_eq!(m.column(2), Vec3d::new(3.0, 6.0, 9.0));
    }

    #[test]
    fn test_transpose() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let t = m.transpose();
        assert_eq!(t[0][1], 4.0);
        assert_eq!(t[1][0], 2.0);
        assert_eq!(t[0][2], 7.0);
        assert_eq!(t[2][0], 3.0);
    }

    #[test]
    fn test_determinant() {
        let identity = Matrix3d::identity();
        assert!((identity.determinant() - 1.0).abs() < 1e-10);

        let scale = Matrix3d::from_scale(2.0);
        assert!((scale.determinant() - 8.0).abs() < 1e-10);

        // Singular matrix (row 3 = row 1 + row 2)
        let singular = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 5.0, 7.0, 9.0);
        assert!(singular.determinant().abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        let m = Matrix3d::new(1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 4.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;

        assert!(product.is_close(&Matrix3d::identity(), 1e-10));
    }

    #[test]
    fn test_inverse_non_diagonal() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 0.0, 1.0, 4.0, 5.0, 6.0, 0.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;

        assert!(product.is_close(&Matrix3d::identity(), 1e-10));
    }

    #[test]
    fn test_singular_inverse() {
        let m = Matrix3d::new(
            1.0, 2.0, 3.0, 2.0, 4.0, 6.0, // Row 2 = 2 * Row 1
            7.0, 8.0, 9.0,
        );
        assert!(m.inverse().is_none());
    }

    #[test]
    fn test_matrix_addition() {
        let m1 = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let m2 = Matrix3d::new(9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0);
        let sum = m1 + m2;
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(sum[i][j], 10.0);
            }
        }
    }

    #[test]
    fn test_matrix_subtraction() {
        let m1 = Matrix3d::new(9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0);
        let m2 = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let diff = m1 - m2;
        assert_eq!(diff[0][0], 8.0);
        assert_eq!(diff[1][1], 0.0);
        assert_eq!(diff[2][2], -8.0);
    }

    #[test]
    fn test_matrix_multiplication() {
        let m1 = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let m2 = Matrix3d::new(9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0);
        let product = m1 * m2;
        // First row of result:
        // [1*9+2*6+3*3, 1*8+2*5+3*2, 1*7+2*4+3*1] = [30, 24, 18]
        assert_eq!(product[0][0], 30.0);
        assert_eq!(product[0][1], 24.0);
        assert_eq!(product[0][2], 18.0);
    }

    #[test]
    fn test_scalar_multiplication() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let scaled = m * 2.0;
        assert_eq!(scaled[0][0], 2.0);
        assert_eq!(scaled[1][1], 10.0);
        assert_eq!(scaled[2][2], 18.0);

        let scaled2 = 3.0 * m;
        assert_eq!(scaled2[0][0], 3.0);
    }

    #[test]
    fn test_matrix_vector_multiplication() {
        let m = Matrix3d::new(1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0);
        let v = Vec3d::new(1.0, 2.0, 3.0);
        let result = m * v;
        assert_eq!(result, Vec3d::new(1.0, 4.0, 9.0));
    }

    #[test]
    fn test_vector_matrix_multiplication() {
        let m = Matrix3d::new(1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0);
        let v = Vec3d::new(1.0, 2.0, 3.0);
        let result = v * m;
        assert_eq!(result, Vec3d::new(1.0, 4.0, 9.0));
    }

    #[test]
    fn test_negation() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let neg = -m;
        assert_eq!(neg[0][0], -1.0);
        assert_eq!(neg[1][1], -5.0);
        assert_eq!(neg[2][2], -9.0);
    }

    #[test]
    fn test_diagonal_and_trace() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m.diagonal(), Vec3d::new(1.0, 5.0, 9.0));
        assert!((m.trace() - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_rotation() {
        // 90 degree rotation around Z axis
        let rot = Matrix3d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);

        // Rotating X axis should give Y axis
        let x = Vec3d::new(1.0, 0.0, 0.0);
        let result = rot * x;
        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
        assert!(result.z.abs() < 1e-10);

        // Rotating Y axis should give -X axis
        let y = Vec3d::new(0.0, 1.0, 0.0);
        let result2 = rot * y;
        assert!((result2.x + 1.0).abs() < 1e-10);
        assert!(result2.y.abs() < 1e-10);
        assert!(result2.z.abs() < 1e-10);
    }

    #[test]
    fn test_orthonormalize() {
        let mut m = Matrix3d::from_scale(2.0);
        assert!(m.orthonormalize());
        assert!(m.is_close(&Matrix3d::identity(), 1e-10));
    }

    #[test]
    fn test_handedness() {
        let identity = Matrix3d::identity();
        assert!(identity.is_right_handed());
        assert!(!identity.is_left_handed());

        // Flip one axis to get left-handed
        let left = Matrix3d::from_diagonal_values(-1.0, 1.0, 1.0);
        assert!(left.is_left_handed());
        assert!(!left.is_right_handed());
    }

    #[test]
    fn test_identity_multiplication() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let i = Matrix3d::identity();

        assert_eq!(m * i, m);
        assert_eq!(i * m, m);
    }

    #[test]
    fn test_rotation_inverse() {
        let axis = Vec3d::new(1.0, 1.0, 1.0);
        let angle = PI / 4.0;
        let rot = Matrix3d::from_rotation(axis, angle);
        let inv = rot.inverse().unwrap();
        let rot_neg = Matrix3d::from_rotation(axis, -angle);

        assert!(rot_neg.is_close(&inv, 1e-10));
    }

    #[test]
    fn test_helper_functions() {
        let m = matrix3d(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[2][2], 9.0);

        let mf = matrix3f(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(mf[0][0], 1.0);
    }

    #[test]
    fn test_display() {
        let m = Matrix3d::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let s = format!("{}", m);
        assert!(s.contains("1"));
        assert!(s.contains("9"));
    }
}
