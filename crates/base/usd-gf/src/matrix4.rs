//! 4x4 matrix types.
//!
//! This module provides Matrix4 types for 4x4 matrix math operations.
//! Matrices are stored in row-major order.
//!
//! # CRITICAL: Row-Vector Convention (Imath / OpenUSD standard)
//!
//! Matrix4 uses the **row-vector** convention, identical to C++ GfMatrix4d:
//!
//! - **Transform a point**: `v' = v * M` (row vector on the LEFT of the matrix)
//! - **Translation** is stored in **row 3**: `m[3][0]=tx, m[3][1]=ty, m[3][2]=tz`
//! - **Compose transforms**: `R * T` = rotate first, then translate
//! - **View-Projection chain**: `VP = View * Proj`, then `clip = point * VP`
//!
//! When projecting world points to clip/screen space, use `column()` to extract
//! the dot-product axis, NOT `row()`:
//! ```text
//! // CORRECT (row-vector): clip_x = point.x*col0.x + point.y*col0.y + point.z*col0.z + col0.w
//! // WRONG  (col-vector): clip_x = row0.x*point.x + row0.y*point.y + row0.z*point.z + row0.w
//! ```
//!
//! # 3D Transformations
//!
//! Matrix4 is the primary type for representing 3D transformations including:
//! - Translation (stored in the last row: `[tx, ty, tz, 1]`)
//! - Rotation (upper-left 3x3 submatrix)
//! - Scale (diagonal elements of upper-left 3x3)
//!
//! # Examples
//!
//! ```
//! use usd_gf::matrix4::{Matrix4d, Matrix4f};
//! use usd_gf::vec3::Vec3d;
//! use usd_gf::vec4::Vec4d;
//!
//! // Create transformation matrices
//! let m = Matrix4d::identity();
//! let trans = Matrix4d::from_translation(Vec3d::new(1.0, 2.0, 3.0));
//! let scale = Matrix4d::from_scale(2.0);
//!
//! // Compose transforms
//! let combined = scale * trans;
//!
//! // Transform a point
//! let point = Vec3d::new(1.0, 0.0, 0.0);
//! let result = combined.transform_point(&point);
//! ```

use crate::limits::MIN_ORTHO_TOLERANCE;
use crate::limits::MIN_VECTOR_LENGTH;
use crate::matrix3::Matrix3;
use crate::traits::{GfMatrix, Scalar};
use crate::vec3::Vec3;
use crate::vec4::Vec4;
use num_traits::Float;
use std::convert::From;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 4x4 matrix with scalar type `T`, stored in row-major order.
///
/// Matrix elements are accessed as `matrix[row][col]`.
///
/// # Examples
///
/// ```
/// use usd_gf::matrix4::Matrix4d;
///
/// let m = Matrix4d::identity();
/// assert_eq!(m[0][0], 1.0);
/// assert_eq!(m[3][3], 1.0);
/// assert_eq!(m[0][1], 0.0);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Matrix4<T> {
    /// Matrix data in row-major order.
    data: [[T; 4]; 4],
}

// Mark Matrix4 as a gf matrix type
impl<T> GfMatrix for Matrix4<T> {}

/// Type alias for 4x4 double-precision matrix.
pub type Matrix4d = Matrix4<f64>;

/// Type alias for 4x4 single-precision matrix.
pub type Matrix4f = Matrix4<f32>;

impl<T: Scalar> Matrix4<T> {
    /// Number of rows in the matrix.
    pub const NUM_ROWS: usize = 4;
    /// Number of columns in the matrix.
    pub const NUM_COLS: usize = 4;

    /// Creates a new matrix from individual elements (row-major order).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    ///
    /// let m = Matrix4d::new(
    ///     1.0, 0.0, 0.0, 0.0,
    ///     0.0, 1.0, 0.0, 0.0,
    ///     0.0, 0.0, 1.0, 0.0,
    ///     0.0, 0.0, 0.0, 1.0,
    /// );
    /// assert_eq!(m[0][0], 1.0);
    /// ```
    #[inline]
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        m00: T,
        m01: T,
        m02: T,
        m03: T,
        m10: T,
        m11: T,
        m12: T,
        m13: T,
        m20: T,
        m21: T,
        m22: T,
        m23: T,
        m30: T,
        m31: T,
        m32: T,
        m33: T,
    ) -> Self {
        Self {
            data: [
                [m00, m01, m02, m03],
                [m10, m11, m12, m13],
                [m20, m21, m22, m23],
                [m30, m31, m32, m33],
            ],
        }
    }

    /// Creates a matrix from a 2D array (row-major order).
    #[inline]
    #[must_use]
    pub fn from_array(data: [[T; 4]; 4]) -> Self {
        Self { data }
    }

    /// Creates a matrix from a pointer to a 4x4 array.
    ///
    /// Matches C++ `GfMatrix4d(const double m[4][4])` constructor.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 4x4 array of `T` values.
    #[inline]
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_ptr(ptr: *const [T; 4]) -> Self {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            let row2 = *ptr.add(2);
            let row3 = *ptr.add(3);
            Self {
                data: [
                    [row0[0], row0[1], row0[2], row0[3]],
                    [row1[0], row1[1], row1[2], row1[3]],
                    [row2[0], row2[1], row2[2], row2[3]],
                    [row3[0], row3[1], row3[2], row3[3]],
                ],
            }
        }
    }

    /// Sets the matrix from a pointer to a 4x4 array.
    ///
    /// Matches C++ `Set(const double m[4][4])` method.
    ///
    /// # Safety
    ///
    /// The pointer must point to a valid 4x4 array of `T` values.
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn set_from_ptr(&mut self, ptr: *const [T; 4]) {
        unsafe {
            let row0 = *ptr;
            let row1 = *ptr.add(1);
            let row2 = *ptr.add(2);
            let row3 = *ptr.add(3);
            self.data[0] = row0;
            self.data[1] = row1;
            self.data[2] = row2;
            self.data[3] = row3;
        }
    }

    /// Sets the matrix from a 2D array (row-major order).
    ///
    /// Matches C++ `Set(const double m[4][4])` method.
    #[inline]
    pub fn set_from_array(&mut self, arr: [[T; 4]; 4]) {
        self.data = arr;
    }

    /// Creates the identity matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    ///
    /// let m = Matrix4d::identity();
    /// for i in 0..4 {
    ///     for j in 0..4 {
    ///         let expected = if i == j { 1.0 } else { 0.0 };
    ///         assert_eq!(m[i][j], expected);
    ///     }
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn identity() -> Self {
        Self::from_diagonal_values(T::ONE, T::ONE, T::ONE, T::ONE)
    }

    /// Creates a zero matrix.
    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self::from_diagonal_values(T::ZERO, T::ZERO, T::ZERO, T::ZERO)
    }

    /// Creates a diagonal matrix with the given diagonal elements.
    #[inline]
    #[must_use]
    pub fn from_diagonal_values(d0: T, d1: T, d2: T, d3: T) -> Self {
        Self::new(
            d0,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            d1,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            d2,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            d3,
        )
    }

    /// Creates a diagonal matrix from a vector.
    #[inline]
    #[must_use]
    pub fn from_diagonal_vec(v: &Vec4<T>) -> Self {
        Self::from_diagonal_values(v.x, v.y, v.z, v.w)
    }

    /// Creates a uniform scale matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix4d::from_scale(2.0);
    /// let p = Vec3d::new(1.0, 1.0, 1.0);
    /// let result = m.transform_point(&p);
    /// assert!((result.x - 2.0).abs() < 1e-10);
    /// assert!((result.y - 2.0).abs() < 1e-10);
    /// assert!((result.z - 2.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_scale(s: T) -> Self {
        Self::from_diagonal_values(s, s, s, T::ONE)
    }

    /// Creates a non-uniform scale matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let scale = Vec3d::new(2.0, 3.0, 4.0);
    /// let m = Matrix4d::from_scale_vec(&scale);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 3.0);
    /// assert_eq!(m[2][2], 4.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_scale_vec(scale: &Vec3<T>) -> Self {
        Self::from_diagonal_values(scale.x, scale.y, scale.z, T::ONE)
    }

    /// Creates a translation matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let trans = Vec3d::new(1.0, 2.0, 3.0);
    /// let m = Matrix4d::from_translation(trans);
    /// assert_eq!(m[3][0], 1.0);
    /// assert_eq!(m[3][1], 2.0);
    /// assert_eq!(m[3][2], 3.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_translation(trans: Vec3<T>) -> Self {
        Self::new(
            T::ONE,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ONE,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ONE,
            T::ZERO,
            trans.x,
            trans.y,
            trans.z,
            T::ONE,
        )
    }

    /// Creates a matrix from a 3x3 rotation matrix and translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::matrix3::Matrix3d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let rot = Matrix3d::identity();
    /// let trans = Vec3d::new(1.0, 2.0, 3.0);
    /// let m = Matrix4d::from_rotation_translation(&rot, &trans);
    /// assert_eq!(m[3][0], 1.0);
    /// assert_eq!(m[3][1], 2.0);
    /// assert_eq!(m[3][2], 3.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_rotation_translation(rot: &Matrix3<T>, trans: &Vec3<T>) -> Self {
        Self::new(
            rot[0][0],
            rot[0][1],
            rot[0][2],
            T::ZERO,
            rot[1][0],
            rot[1][1],
            rot[1][2],
            T::ZERO,
            rot[2][0],
            rot[2][1],
            rot[2][2],
            T::ZERO,
            trans.x,
            trans.y,
            trans.z,
            T::ONE,
        )
    }

    /// Sets all elements from individual values.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &mut self,
        m00: T,
        m01: T,
        m02: T,
        m03: T,
        m10: T,
        m11: T,
        m12: T,
        m13: T,
        m20: T,
        m21: T,
        m22: T,
        m23: T,
        m30: T,
        m31: T,
        m32: T,
        m33: T,
    ) {
        self.data = [
            [m00, m01, m02, m03],
            [m10, m11, m12, m13],
            [m20, m21, m22, m23],
            [m30, m31, m32, m33],
        ];
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
    pub fn set_diagonal(&mut self, d0: T, d1: T, d2: T, d3: T) {
        *self = Self::from_diagonal_values(d0, d1, d2, d3);
    }

    /// Gets a row of the matrix as a Vec4.
    #[inline]
    #[must_use]
    pub fn row(&self, i: usize) -> Vec4<T> {
        Vec4 {
            x: self.data[i][0],
            y: self.data[i][1],
            z: self.data[i][2],
            w: self.data[i][3],
        }
    }

    /// Gets a column of the matrix as a Vec4.
    #[inline]
    #[must_use]
    pub fn column(&self, j: usize) -> Vec4<T> {
        Vec4 {
            x: self.data[0][j],
            y: self.data[1][j],
            z: self.data[2][j],
            w: self.data[3][j],
        }
    }

    /// Gets the first 3 elements of a row as a Vec3.
    #[inline]
    #[must_use]
    pub fn row3(&self, i: usize) -> Vec3<T> {
        Vec3 {
            x: self.data[i][0],
            y: self.data[i][1],
            z: self.data[i][2],
        }
    }

    /// Sets a row of the matrix from a Vec4.
    #[inline]
    pub fn set_row(&mut self, i: usize, v: &Vec4<T>) {
        self.data[i][0] = v.x;
        self.data[i][1] = v.y;
        self.data[i][2] = v.z;
        self.data[i][3] = v.w;
    }

    /// Sets a column of the matrix from a Vec4.
    #[inline]
    pub fn set_column(&mut self, j: usize, v: &Vec4<T>) {
        self.data[0][j] = v.x;
        self.data[1][j] = v.y;
        self.data[2][j] = v.z;
        self.data[3][j] = v.w;
    }

    /// Sets the first 3 elements of a row from a Vec3.
    #[inline]
    pub fn set_row3(&mut self, i: usize, v: &Vec3<T>) {
        self.data[i][0] = v.x;
        self.data[i][1] = v.y;
        self.data[i][2] = v.z;
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
        // SAFETY: Matrix4 is repr(C) with contiguous T values
        unsafe { std::slice::from_raw_parts(self.as_ptr(), 16) }
    }

    /// Returns the data as a 2D array.
    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [[T; 4]; 4] {
        self.data
    }

    /// Returns the transpose of the matrix.
    #[inline]
    #[must_use]
    pub fn transpose(&self) -> Self {
        Self::new(
            self.data[0][0],
            self.data[1][0],
            self.data[2][0],
            self.data[3][0],
            self.data[0][1],
            self.data[1][1],
            self.data[2][1],
            self.data[3][1],
            self.data[0][2],
            self.data[1][2],
            self.data[2][2],
            self.data[3][2],
            self.data[0][3],
            self.data[1][3],
            self.data[2][3],
            self.data[3][3],
        )
    }

    /// Returns the determinant of the upper 3x3 submatrix.
    ///
    /// Useful when matrix represents a linear transformation (rotation/scale).
    #[inline]
    #[must_use]
    pub fn determinant3(&self) -> T {
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

    /// Returns the determinant of the full 4x4 matrix.
    ///
    /// Uses Laplace expansion along the first row.
    #[must_use]
    pub fn determinant(&self) -> T {
        // Laplace expansion along first row using 3x3 minors
        let m = &self.data;

        // Minor determinants (3x3 submatrices)
        let minor0 = m[1][1] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[1][2] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            + m[1][3] * (m[2][1] * m[3][2] - m[2][2] * m[3][1]);

        let minor1 = m[1][0] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[1][2] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[1][3] * (m[2][0] * m[3][2] - m[2][2] * m[3][0]);

        let minor2 = m[1][0] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            - m[1][1] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[1][3] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]);

        let minor3 = m[1][0] * (m[2][1] * m[3][2] - m[2][2] * m[3][1])
            - m[1][1] * (m[2][0] * m[3][2] - m[2][2] * m[3][0])
            + m[1][2] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]);

        m[0][0] * minor0 - m[0][1] * minor1 + m[0][2] * minor2 - m[0][3] * minor3
    }

    /// Returns the inverse of the matrix, or None if singular.
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

        let m = &self.data;
        let inv_det = T::ONE / det;

        // Compute cofactor matrix and transpose (adjugate)
        // Using direct formula for 4x4 inverse

        // Cofactors for each element
        let c00 = m[1][1] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[1][2] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            + m[1][3] * (m[2][1] * m[3][2] - m[2][2] * m[3][1]);

        let c01 = -(m[1][0] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[1][2] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[1][3] * (m[2][0] * m[3][2] - m[2][2] * m[3][0]));

        let c02 = m[1][0] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            - m[1][1] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[1][3] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]);

        let c03 = -(m[1][0] * (m[2][1] * m[3][2] - m[2][2] * m[3][1])
            - m[1][1] * (m[2][0] * m[3][2] - m[2][2] * m[3][0])
            + m[1][2] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]));

        let c10 = -(m[0][1] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[0][2] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            + m[0][3] * (m[2][1] * m[3][2] - m[2][2] * m[3][1]));

        let c11 = m[0][0] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
            - m[0][2] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[0][3] * (m[2][0] * m[3][2] - m[2][2] * m[3][0]);

        let c12 = -(m[0][0] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
            - m[0][1] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
            + m[0][3] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]));

        let c13 = m[0][0] * (m[2][1] * m[3][2] - m[2][2] * m[3][1])
            - m[0][1] * (m[2][0] * m[3][2] - m[2][2] * m[3][0])
            + m[0][2] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]);

        let c20 = m[0][1] * (m[1][2] * m[3][3] - m[1][3] * m[3][2])
            - m[0][2] * (m[1][1] * m[3][3] - m[1][3] * m[3][1])
            + m[0][3] * (m[1][1] * m[3][2] - m[1][2] * m[3][1]);

        let c21 = -(m[0][0] * (m[1][2] * m[3][3] - m[1][3] * m[3][2])
            - m[0][2] * (m[1][0] * m[3][3] - m[1][3] * m[3][0])
            + m[0][3] * (m[1][0] * m[3][2] - m[1][2] * m[3][0]));

        let c22 = m[0][0] * (m[1][1] * m[3][3] - m[1][3] * m[3][1])
            - m[0][1] * (m[1][0] * m[3][3] - m[1][3] * m[3][0])
            + m[0][3] * (m[1][0] * m[3][1] - m[1][1] * m[3][0]);

        let c23 = -(m[0][0] * (m[1][1] * m[3][2] - m[1][2] * m[3][1])
            - m[0][1] * (m[1][0] * m[3][2] - m[1][2] * m[3][0])
            + m[0][2] * (m[1][0] * m[3][1] - m[1][1] * m[3][0]));

        let c30 = -(m[0][1] * (m[1][2] * m[2][3] - m[1][3] * m[2][2])
            - m[0][2] * (m[1][1] * m[2][3] - m[1][3] * m[2][1])
            + m[0][3] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]));

        let c31 = m[0][0] * (m[1][2] * m[2][3] - m[1][3] * m[2][2])
            - m[0][2] * (m[1][0] * m[2][3] - m[1][3] * m[2][0])
            + m[0][3] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]);

        let c32 = -(m[0][0] * (m[1][1] * m[2][3] - m[1][3] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][3] - m[1][3] * m[2][0])
            + m[0][3] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]));

        let c33 = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);

        // Transpose cofactor matrix and multiply by 1/det
        Some(Self::new(
            c00 * inv_det,
            c10 * inv_det,
            c20 * inv_det,
            c30 * inv_det,
            c01 * inv_det,
            c11 * inv_det,
            c21 * inv_det,
            c31 * inv_det,
            c02 * inv_det,
            c12 * inv_det,
            c22 * inv_det,
            c32 * inv_det,
            c03 * inv_det,
            c13 * inv_det,
            c23 * inv_det,
            c33 * inv_det,
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
        // Reuse inverse calculation (slightly inefficient but clear)
        self.inverse_with_eps(eps).map(|inv| (inv, det))
    }

    /// Returns whether this matrix is approximately equal to another.
    #[inline]
    #[must_use]
    pub fn is_close(&self, other: &Self, eps: T) -> bool {
        for i in 0..4 {
            for j in 0..4 {
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
    pub fn diagonal(&self) -> Vec4<T> {
        Vec4 {
            x: self.data[0][0],
            y: self.data[1][1],
            z: self.data[2][2],
            w: self.data[3][3],
        }
    }

    /// Returns the trace (sum of diagonal elements).
    #[inline]
    #[must_use]
    pub fn trace(&self) -> T {
        self.data[0][0] + self.data[1][1] + self.data[2][2] + self.data[3][3]
    }

    /// Extracts the rotation as a quaternion from the upper-left 3x3.
    ///
    /// Assumes the matrix contains a pure rotation (orthonormal).
    /// For matrices with scale/shear, call `orthonormalize()` first.
    #[must_use]
    pub fn extract_rotation_quat(&self) -> crate::quat::Quat<T> {
        use crate::quat::Quat;

        // Find largest diagonal element
        let i = if self.data[0][0] > self.data[1][1] {
            if self.data[0][0] > self.data[2][2] {
                0
            } else {
                2
            }
        } else if self.data[1][1] > self.data[2][2] {
            1
        } else {
            2
        };

        let trace = self.data[0][0] + self.data[1][1] + self.data[2][2] + self.data[3][3];

        if trace > self.data[i][i] {
            // Trace is largest
            let half = T::ONE / (T::ONE + T::ONE);
            let four = T::ONE + T::ONE + T::ONE + T::ONE;
            let r = half * trace.sqrt();
            let four_r = four * r;
            let ix = (self.data[1][2] - self.data[2][1]) / four_r;
            let iy = (self.data[2][0] - self.data[0][2]) / four_r;
            let iz = (self.data[0][1] - self.data[1][0]) / four_r;

            // Clamp real part to [-1, 1]
            let r_clamped = if r < -T::ONE {
                -T::ONE
            } else if r > T::ONE {
                T::ONE
            } else {
                r
            };

            Quat::new(
                r_clamped,
                Vec3 {
                    x: ix,
                    y: iy,
                    z: iz,
                },
            )
        } else {
            // One of the diagonal elements is largest
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            let half = T::ONE / (T::ONE + T::ONE);
            let four = T::ONE + T::ONE + T::ONE + T::ONE;

            let q_i = half
                * (self.data[i][i] - self.data[j][j] - self.data[k][k] + self.data[3][3]).sqrt();
            let four_q = four * q_i;

            let mut im = [T::ZERO, T::ZERO, T::ZERO];
            im[i] = q_i;
            im[j] = (self.data[i][j] + self.data[j][i]) / four_q;
            im[k] = (self.data[k][i] + self.data[i][k]) / four_q;
            let r = (self.data[j][k] - self.data[k][j]) / four_q;

            // Clamp real part to [-1, 1]
            let r_clamped = if r < -T::ONE {
                -T::ONE
            } else if r > T::ONE {
                T::ONE
            } else {
                r
            };

            Quat::new(
                r_clamped,
                Vec3 {
                    x: im[0],
                    y: im[1],
                    z: im[2],
                },
            )
        }
    }

    /// Extracts the upper-left 3x3 rotation/scale matrix.
    #[inline]
    #[must_use]
    pub fn extract_rotation_matrix(&self) -> Matrix3<T> {
        Matrix3::new(
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

    /// Extracts the translation (last row, first 3 elements).
    #[inline]
    #[must_use]
    pub fn extract_translation(&self) -> Vec3<T> {
        Vec3 {
            x: self.data[3][0],
            y: self.data[3][1],
            z: self.data[3][2],
        }
    }

    /// Sets the translation part of the matrix.
    #[inline]
    pub fn set_translation(&mut self, trans: &Vec3<T>) {
        self.data[3][0] = trans.x;
        self.data[3][1] = trans.y;
        self.data[3][2] = trans.z;
    }

    /// Sets the upper 3x3 rotation/scale part from a Matrix3.
    #[inline]
    pub fn set_rotation_matrix(&mut self, rot: &Matrix3<T>) {
        self.data[0][0] = rot[0][0];
        self.data[0][1] = rot[0][1];
        self.data[0][2] = rot[0][2];
        self.data[1][0] = rot[1][0];
        self.data[1][1] = rot[1][1];
        self.data[1][2] = rot[1][2];
        self.data[2][0] = rot[2][0];
        self.data[2][1] = rot[2][1];
        self.data[2][2] = rot[2][2];
    }

    /// Returns the sign of the determinant of the upper 3x3 matrix.
    ///
    /// Returns 1 for right-handed, -1 for left-handed, 0 for singular.
    #[inline]
    #[must_use]
    pub fn handedness(&self) -> T {
        let det = self.determinant3();
        if det > T::ZERO {
            T::ONE
        } else if det < T::ZERO {
            -T::ONE
        } else {
            T::ZERO
        }
    }

    /// Returns true if the upper 3x3 forms a right-handed coordinate system.
    #[inline]
    #[must_use]
    pub fn is_right_handed(&self) -> bool {
        self.handedness() == T::ONE
    }

    /// Returns true if the upper 3x3 forms a left-handed coordinate system.
    #[inline]
    #[must_use]
    pub fn is_left_handed(&self) -> bool {
        self.handedness() == -T::ONE
    }

    /// Returns true if the upper 3x3 row vectors are orthogonal.
    #[must_use]
    pub fn has_orthogonal_rows3(&self) -> bool {
        let axis0 = self.row3(0);
        let axis1 = self.row3(1);
        let axis2 = self.row3(2);

        let min_tol = T::from(MIN_ORTHO_TOLERANCE).unwrap_or(T::EPSILON);

        let dot01 = axis0.x * axis1.x + axis0.y * axis1.y + axis0.z * axis1.z;
        let dot02 = axis0.x * axis2.x + axis0.y * axis2.y + axis0.z * axis2.z;
        let dot12 = axis1.x * axis2.x + axis1.y * axis2.y + axis1.z * axis2.z;

        dot01.abs() < min_tol && dot02.abs() < min_tol && dot12.abs() < min_tol
    }

    /// Transforms a point (Vec3 treated as having w=1).
    ///
    /// This applies translation in addition to rotation/scale.
    /// For affine matrices, this is equivalent to `(v * M).xyz`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix4d::from_translation(Vec3d::new(1.0, 2.0, 3.0));
    /// let p = Vec3d::new(0.0, 0.0, 0.0);
    /// let result = m.transform_point(&p);
    /// assert!((result.x - 1.0).abs() < 1e-10);
    /// assert!((result.y - 2.0).abs() < 1e-10);
    /// assert!((result.z - 3.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn transform_point(&self, v: &Vec3<T>) -> Vec3<T> {
        // v * M where v.w = 1 (affine transformation)
        Vec3 {
            x: v.x * self.data[0][0]
                + v.y * self.data[1][0]
                + v.z * self.data[2][0]
                + self.data[3][0],
            y: v.x * self.data[0][1]
                + v.y * self.data[1][1]
                + v.z * self.data[2][1]
                + self.data[3][1],
            z: v.x * self.data[0][2]
                + v.y * self.data[1][2]
                + v.z * self.data[2][2]
                + self.data[3][2],
        }
    }

    /// Transforms a direction vector (Vec3 treated as having w=0).
    ///
    /// This ignores translation, applying only rotation/scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix4d::from_translation(Vec3d::new(100.0, 200.0, 300.0));
    /// let dir = Vec3d::new(1.0, 0.0, 0.0);
    /// let result = m.transform_dir(&dir);
    /// // Translation should not affect direction
    /// assert!((result.x - 1.0).abs() < 1e-10);
    /// assert!((result.y).abs() < 1e-10);
    /// assert!((result.z).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn transform_dir(&self, v: &Vec3<T>) -> Vec3<T> {
        // v * M where v.w = 0 (direction, no translation)
        Vec3 {
            x: v.x * self.data[0][0] + v.y * self.data[1][0] + v.z * self.data[2][0],
            y: v.x * self.data[0][1] + v.y * self.data[1][1] + v.z * self.data[2][1],
            z: v.x * self.data[0][2] + v.y * self.data[1][2] + v.z * self.data[2][2],
        }
    }

    /// Transforms a point by the matrix assuming an affine transform (w=1, col3 ignored).
    ///
    /// Unlike `transform()`, this skips the perspective divide and ignores the
    /// 4th column, assuming it is `(0, 0, 0, 1)`.
    /// C++ parity: `GfMatrix4d::TransformAffine(GfVec3d)`.
    #[inline]
    #[must_use]
    pub fn transform_affine(&self, v: &Vec3<T>) -> Vec3<T> {
        Vec3 {
            x: v.x * self.data[0][0]
                + v.y * self.data[1][0]
                + v.z * self.data[2][0]
                + self.data[3][0],
            y: v.x * self.data[0][1]
                + v.y * self.data[1][1]
                + v.z * self.data[2][1]
                + self.data[3][1],
            z: v.x * self.data[0][2]
                + v.y * self.data[1][2]
                + v.z * self.data[2][2]
                + self.data[3][2],
        }
    }
}

/// Methods specific to floating-point matrices.
impl<T: Scalar + Float> Matrix4<T> {
    /// Creates a rotation matrix around the given axis by the given angle (radians).
    ///
    /// Uses Rodrigues' rotation formula.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// // 90 degree rotation around Z axis
    /// let rot = Matrix4d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// let x = Vec3d::new(1.0, 0.0, 0.0);
    /// let result = rot.transform_dir(&x);
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

        // Rodrigues' rotation formula, transposed for row-vector convention (v * M)
        Self::new(
            t * x * x + c,
            t * x * y + s * z,
            t * x * z - s * y,
            T::ZERO,
            t * x * y - s * z,
            t * y * y + c,
            t * y * z + s * x,
            T::ZERO,
            t * x * z + s * y,
            t * y * z - s * x,
            t * z * z + c,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ONE,
        )
    }

    /// Creates a look-at view matrix.
    ///
    /// # Arguments
    ///
    /// * `eye` - Position of the camera
    /// * `center` - Point the camera is looking at
    /// * `up` - Up direction
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let eye = Vec3d::new(0.0, 0.0, 5.0);
    /// let center = Vec3d::new(0.0, 0.0, 0.0);
    /// let up = Vec3d::new(0.0, 1.0, 0.0);
    /// let view = Matrix4d::look_at(&eye, &center, &up);
    /// ```
    #[must_use]
    pub fn look_at(eye: &Vec3<T>, center: &Vec3<T>, up: &Vec3<T>) -> Self {
        // Forward direction (from eye to center)
        let mut fwd = Vec3 {
            x: center.x - eye.x,
            y: center.y - eye.y,
            z: center.z - eye.z,
        };
        let fwd_len = (fwd.x * fwd.x + fwd.y * fwd.y + fwd.z * fwd.z).sqrt();
        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);
        if fwd_len < min_len {
            return Self::identity();
        }
        fwd.x /= fwd_len;
        fwd.y /= fwd_len;
        fwd.z /= fwd_len;

        // Right direction = forward x up
        let mut right = Vec3 {
            x: fwd.y * up.z - fwd.z * up.y,
            y: fwd.z * up.x - fwd.x * up.z,
            z: fwd.x * up.y - fwd.y * up.x,
        };
        let right_len = (right.x * right.x + right.y * right.y + right.z * right.z).sqrt();
        if right_len < min_len {
            return Self::identity();
        }
        right.x /= right_len;
        right.y /= right_len;
        right.z /= right_len;

        // Recalculate up = right x forward
        let new_up = Vec3 {
            x: right.y * fwd.z - right.z * fwd.y,
            y: right.z * fwd.x - right.x * fwd.z,
            z: right.x * fwd.y - right.y * fwd.x,
        };

        // Build the view matrix (transpose of rotation * translation)
        Self::new(
            right.x,
            new_up.x,
            -fwd.x,
            T::ZERO,
            right.y,
            new_up.y,
            -fwd.y,
            T::ZERO,
            right.z,
            new_up.z,
            -fwd.z,
            T::ZERO,
            -(right.x * eye.x + right.y * eye.y + right.z * eye.z),
            -(new_up.x * eye.x + new_up.y * eye.y + new_up.z * eye.z),
            fwd.x * eye.x + fwd.y * eye.y + fwd.z * eye.z,
            T::ONE,
        )
    }

    /// Orthonormalizes the upper 3x3 rotation part in place.
    ///
    /// Uses Gram-Schmidt orthonormalization.
    /// Returns true if converged, false otherwise.
    pub fn orthonormalize(&mut self) -> bool {
        let mut r0 = self.row3(0);
        let mut r1 = self.row3(1);
        let mut r2 = self.row3(2);

        // Normalize first row
        let len0 = (r0.x * r0.x + r0.y * r0.y + r0.z * r0.z).sqrt();
        let min_len = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);
        if len0 < min_len {
            return false;
        }
        r0.x /= len0;
        r0.y /= len0;
        r0.z /= len0;

        // Orthogonalize and normalize second row
        let dot01 = r1.x * r0.x + r1.y * r0.y + r1.z * r0.z;
        r1.x -= dot01 * r0.x;
        r1.y -= dot01 * r0.y;
        r1.z -= dot01 * r0.z;
        let len1 = (r1.x * r1.x + r1.y * r1.y + r1.z * r1.z).sqrt();
        if len1 < min_len {
            return false;
        }
        r1.x /= len1;
        r1.y /= len1;
        r1.z /= len1;

        // Orthogonalize and normalize third row
        let dot02 = r2.x * r0.x + r2.y * r0.y + r2.z * r0.z;
        let dot12 = r2.x * r1.x + r2.y * r1.y + r2.z * r1.z;
        r2.x -= dot02 * r0.x + dot12 * r1.x;
        r2.y -= dot02 * r0.y + dot12 * r1.y;
        r2.z -= dot02 * r0.z + dot12 * r1.z;
        let len2 = (r2.x * r2.x + r2.y * r2.y + r2.z * r2.z).sqrt();
        if len2 < min_len {
            return false;
        }
        r2.x /= len2;
        r2.y /= len2;
        r2.z /= len2;

        self.set_row3(0, &r0);
        self.set_row3(1, &r1);
        self.set_row3(2, &r2);

        true
    }

    /// Returns an orthonormalized copy of the matrix.
    #[must_use]
    pub fn orthonormalized(&self) -> Self {
        let mut result = *self;
        result.orthonormalize();
        result
    }

    /// Removes scale and shear, leaving only rotation and translation.
    ///
    /// If decomposition fails, returns the original matrix.
    #[must_use]
    pub fn remove_scale_shear(&self) -> Self {
        let mut result = *self;
        if result.orthonormalize() {
            result
        } else {
            *self
        }
    }

    /// Extracts the scale factors from the matrix.
    ///
    /// Returns the length of each basis vector (columns 0-2), which represents
    /// the scale along each axis. This works correctly for matrices that are
    /// composed of rotation and non-uniform scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let m = Matrix4d::from_scale_vec(&Vec3d::new(2.0, 3.0, 4.0));
    /// let scale = m.extract_scale();
    /// assert!((scale.x - 2.0).abs() < 1e-10);
    /// assert!((scale.y - 3.0).abs() < 1e-10);
    /// assert!((scale.z - 4.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn extract_scale(&self) -> Vec3<T> {
        // Scale is the length of each basis vector (column)
        let sx = (self.data[0][0] * self.data[0][0]
            + self.data[0][1] * self.data[0][1]
            + self.data[0][2] * self.data[0][2])
            .sqrt();
        let sy = (self.data[1][0] * self.data[1][0]
            + self.data[1][1] * self.data[1][1]
            + self.data[1][2] * self.data[1][2])
            .sqrt();
        let sz = (self.data[2][0] * self.data[2][0]
            + self.data[2][1] * self.data[2][1]
            + self.data[2][2] * self.data[2][2])
            .sqrt();
        Vec3 {
            x: sx,
            y: sy,
            z: sz,
        }
    }

    /// Sets the matrix to specify a uniform scaling by `scale_factor`.
    ///
    /// Clears rotation and translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    ///
    /// let mut m = Matrix4d::identity();
    /// m.set_scale(2.0);
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 2.0);
    /// assert_eq!(m[2][2], 2.0);
    /// ```
    pub fn set_scale(&mut self, scale_factor: T) -> &mut Self {
        self.data[0][0] = scale_factor;
        self.data[0][1] = T::ZERO;
        self.data[0][2] = T::ZERO;
        self.data[0][3] = T::ZERO;
        self.data[1][0] = T::ZERO;
        self.data[1][1] = scale_factor;
        self.data[1][2] = T::ZERO;
        self.data[1][3] = T::ZERO;
        self.data[2][0] = T::ZERO;
        self.data[2][1] = T::ZERO;
        self.data[2][2] = scale_factor;
        self.data[2][3] = T::ZERO;
        self.data[3][0] = T::ZERO;
        self.data[3][1] = T::ZERO;
        self.data[3][2] = T::ZERO;
        self.data[3][3] = T::ONE;
        self
    }

    /// Sets the matrix to specify a non-uniform scaling by `scale_factors`.
    ///
    /// Clears rotation and translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let mut m = Matrix4d::identity();
    /// m.set_scale_vec(&Vec3d::new(2.0, 3.0, 4.0));
    /// assert_eq!(m[0][0], 2.0);
    /// assert_eq!(m[1][1], 3.0);
    /// assert_eq!(m[2][2], 4.0);
    /// ```
    pub fn set_scale_vec(&mut self, scale_factors: &Vec3<T>) -> &mut Self {
        self.data[0][0] = scale_factors.x;
        self.data[0][1] = T::ZERO;
        self.data[0][2] = T::ZERO;
        self.data[0][3] = T::ZERO;
        self.data[1][0] = T::ZERO;
        self.data[1][1] = scale_factors.y;
        self.data[1][2] = T::ZERO;
        self.data[1][3] = T::ZERO;
        self.data[2][0] = T::ZERO;
        self.data[2][1] = T::ZERO;
        self.data[2][2] = scale_factors.z;
        self.data[2][3] = T::ZERO;
        self.data[3][0] = T::ZERO;
        self.data[3][1] = T::ZERO;
        self.data[3][2] = T::ZERO;
        self.data[3][3] = T::ONE;
        self
    }

    /// Sets the matrix to specify a translation by `trans`.
    ///
    /// Clears rotation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let mut m = Matrix4d::identity();
    /// m.set_translate(&Vec3d::new(1.0, 2.0, 3.0));
    /// assert_eq!(m[3][0], 1.0);
    /// assert_eq!(m[3][1], 2.0);
    /// assert_eq!(m[3][2], 3.0);
    /// ```
    pub fn set_translate(&mut self, trans: &Vec3<T>) -> &mut Self {
        self.data[0][0] = T::ONE;
        self.data[0][1] = T::ZERO;
        self.data[0][2] = T::ZERO;
        self.data[0][3] = T::ZERO;
        self.data[1][0] = T::ZERO;
        self.data[1][1] = T::ONE;
        self.data[1][2] = T::ZERO;
        self.data[1][3] = T::ZERO;
        self.data[2][0] = T::ZERO;
        self.data[2][1] = T::ZERO;
        self.data[2][2] = T::ONE;
        self.data[2][3] = T::ZERO;
        self.data[3][0] = trans.x;
        self.data[3][1] = trans.y;
        self.data[3][2] = trans.z;
        self.data[3][3] = T::ONE;
        self
    }

    /// Sets the matrix to specify a translation by `trans`, without clearing rotation.
    ///
    /// Only modifies the translation part (last row).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::vec3::Vec3d;
    ///
    /// let mut m = Matrix4d::identity();
    /// m.set_translate_only(&Vec3d::new(1.0, 2.0, 3.0));
    /// assert_eq!(m[3][0], 1.0);
    /// assert_eq!(m[3][1], 2.0);
    /// assert_eq!(m[3][2], 3.0);
    /// ```
    pub fn set_translate_only(&mut self, trans: &Vec3<T>) -> &mut Self {
        self.data[3][0] = trans.x;
        self.data[3][1] = trans.y;
        self.data[3][2] = trans.z;
        self.data[3][3] = T::ONE;
        self
    }
}

/// Methods specific to floating-point matrices that support quaternion rotations.
impl<T: Scalar + Float> Matrix4<T> {
    /// Helper to set rotation from quaternion components.
    pub(crate) fn set_rotate_from_quat(&mut self, r: T, i: &Vec3<T>) {
        let two = T::ONE + T::ONE;
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

    /// Sets the matrix to specify a rotation equivalent to `rot`, and clears the translation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// let mut m = Matrix4d::identity();
    /// let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// m.set_rotate(&q);
    /// ```
    pub fn set_rotate(&mut self, rot: &crate::quat::Quat<T>) -> &mut Self {
        self.set_rotate_only(rot);
        self.data[0][3] = T::ZERO;
        self.data[1][3] = T::ZERO;
        self.data[2][3] = T::ZERO;
        self.data[3][0] = T::ZERO;
        self.data[3][1] = T::ZERO;
        self.data[3][2] = T::ZERO;
        self.data[3][3] = T::ONE;
        self
    }

    /// Sets the matrix to specify a rotation equivalent to `rot`, without clearing the translation.
    ///
    /// Only modifies the upper-left 3x3 rotation part.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::matrix4::Matrix4d;
    /// use usd_gf::quat::Quatd;
    /// use usd_gf::vec3::Vec3d;
    /// use std::f64::consts::PI;
    ///
    /// let mut m = Matrix4d::identity();
    /// let q = Quatd::from_axis_angle(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
    /// m.set_rotate_only(&q);
    /// ```
    pub fn set_rotate_only(&mut self, rot: &crate::quat::Quat<T>) -> &mut Self {
        self.set_rotate_from_quat(rot.real(), rot.imaginary());
        self
    }

    /// Sets the 3x3 rotation from a Matrix3, clearing translation/projection.
    ///
    /// Matches C++ `GfMatrix4d::SetRotate(GfMatrix3d const &mx)`.
    pub fn set_rotate_matrix3(&mut self, mx: &Matrix3<T>) -> &mut Self {
        self.set_rotation_matrix(mx);
        self.data[0][3] = T::ZERO;
        self.data[1][3] = T::ZERO;
        self.data[2][3] = T::ZERO;
        self.data[3][0] = T::ZERO;
        self.data[3][1] = T::ZERO;
        self.data[3][2] = T::ZERO;
        self.data[3][3] = T::ONE;
        self
    }

    /// Sets the 3x3 rotation from a Matrix3, preserving translation.
    ///
    /// Matches C++ `GfMatrix4d::SetRotateOnly(GfMatrix3d const &mx)`.
    pub fn set_rotate_only_matrix3(&mut self, mx: &Matrix3<T>) -> &mut Self {
        self.set_rotation_matrix(mx);
        self
    }

    /// Sets the transform from a rotation + translation.
    ///
    /// Matches C++ `GfMatrix4d::SetTransform(GfMatrix3d const &rotmx, GfVec3d const &translate)`.
    pub fn set_transform_matrix3(&mut self, rot: &Matrix3<T>, trans: &Vec3<T>) -> &mut Self {
        self.set_rotate_matrix3(rot);
        self.set_translate_only(trans)
    }

    /// Decomposes the rotation into three Euler-like angles about the given axes.
    ///
    /// Extracts rotation and calls Rotation::decompose.
    /// Matches C++ `GfMatrix4d::DecomposeRotation(axis0, axis1, axis2)`.
    #[must_use]
    pub fn decompose_rotation(&self, axis0: &Vec3<T>, axis1: &Vec3<T>, axis2: &Vec3<T>) -> Vec3<T> {
        let rot3 = self.extract_rotation_matrix();
        rot3.decompose_rotation(axis0, axis1, axis2)
    }
    /// Transforms a point with perspective divide.
    ///
    /// Treats the vector as a 4D point with w=1, transforms it,
    /// and divides by the resulting w coordinate.
    #[must_use]
    pub fn transform(&self, v: &Vec3<T>) -> Vec3<T> {
        let x =
            v.x * self.data[0][0] + v.y * self.data[1][0] + v.z * self.data[2][0] + self.data[3][0];
        let y =
            v.x * self.data[0][1] + v.y * self.data[1][1] + v.z * self.data[2][1] + self.data[3][1];
        let z =
            v.x * self.data[0][2] + v.y * self.data[1][2] + v.z * self.data[2][2] + self.data[3][2];
        let w =
            v.x * self.data[0][3] + v.y * self.data[1][3] + v.z * self.data[2][3] + self.data[3][3];

        let min_w = T::from(MIN_VECTOR_LENGTH).unwrap_or(T::EPSILON);
        if w.abs() < min_w {
            return Vec3 { x, y, z };
        }

        Vec3 {
            x: x / w,
            y: y / w,
            z: z / w,
        }
    }

    /// Factors the matrix into 5 components (matches C++ GfMatrix4::Factor).
    ///
    /// Returns `Some((r, s, u, t, p))` where:
    /// - `r` - Rotation matrix (eigenvectors of M * M^T)
    /// - `s` - Scale factors
    /// - `u` - Shear matrix (contains any remaining shear)
    /// - `t` - Translation
    /// - `p` - Projection matrix (always identity; reserved for future use)
    ///
    /// The factorization is: M = r * diag(s) * u * translate(t)
    ///
    /// Returns `None` if the matrix is singular (though it still computes an approximation).
    #[must_use]
    pub fn factor(&self) -> Option<(Self, Vec3<T>, Self, Vec3<T>, Self)> {
        let eps = T::from(1e-10).unwrap_or(T::EPSILON);

        // Extract upper 3x3 into a and translation into t
        let mut a = Self::identity();
        let mut t = Vec3 {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ZERO,
        };
        for i in 0..3 {
            for j in 0..3 {
                a.data[i][j] = self.data[i][j];
            }
            t[i] = self.data[3][i];
        }

        // Compute determinant of upper 3x3
        let det = a.determinant3();
        let det_sign = if det < T::ZERO { -T::ONE } else { T::ONE };
        let is_singular = det * det_sign < eps;

        // Compute B = A * A^T and find eigenvalues/eigenvectors
        let b = a * a.transpose();
        let (eigenvalues, eigenvectors) = b.jacobi3();

        // R = eigenvector matrix
        let r = Self::new(
            eigenvectors[0].x,
            eigenvectors[0].y,
            eigenvectors[0].z,
            T::ZERO,
            eigenvectors[1].x,
            eigenvectors[1].y,
            eigenvectors[1].z,
            T::ZERO,
            eigenvectors[2].x,
            eigenvectors[2].y,
            eigenvectors[2].z,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ZERO,
            T::ONE,
        );

        // s = sqrt(eigenvalues) with sign, s_inv = 1/s
        let mut s = Vec3 {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ZERO,
        };
        let mut s_inv = Self::identity();
        for i in 0..3 {
            s[i] = if eigenvalues[i] < eps {
                det_sign * eps
            } else {
                det_sign * eigenvalues[i].sqrt()
            };
            s_inv.data[i][i] = T::ONE / s[i];
        }

        // U = R * S^-1 * R^T * A
        let u = r * s_inv * r.transpose() * a;

        if is_singular {
            None
        } else {
            let p = Self::identity();
            Some((r, s, u, t, p))
        }
    }

    /// Jacobi eigenvalue algorithm for 3x3 symmetric matrix.
    ///
    /// Returns (eigenvalues, eigenvectors) for the upper-left 3x3.
    fn jacobi3(&self) -> (Vec3<T>, [Vec3<T>; 3]) {
        let mut eigenvalues = Vec3 {
            x: self.data[0][0],
            y: self.data[1][1],
            z: self.data[2][2],
        };

        let mut eigenvectors = [
            Vec3 {
                x: T::ONE,
                y: T::ZERO,
                z: T::ZERO,
            },
            Vec3 {
                x: T::ZERO,
                y: T::ONE,
                z: T::ZERO,
            },
            Vec3 {
                x: T::ZERO,
                y: T::ZERO,
                z: T::ONE,
            },
        ];

        let mut a = *self;
        let mut b = eigenvalues;
        let mut z = Vec3 {
            x: T::ZERO,
            y: T::ZERO,
            z: T::ZERO,
        };

        let two = T::ONE + T::ONE;
        let three = two + T::ONE;
        let _four = two + two;
        let hundred = T::from(100.0).unwrap_or(T::ONE);
        let point_two = T::from(0.2).unwrap_or(T::ONE / (two + three));

        for iter in 0..50 {
            // Sum of off-diagonal elements
            let mut sm = T::ZERO;
            for p in 0..2 {
                for q in (p + 1)..3 {
                    sm += a.data[p][q].abs();
                }
            }

            if sm == T::ZERO {
                break;
            }

            let thresh = if iter < 3 {
                point_two * sm / (three * three)
            } else {
                T::ZERO
            };

            for p in 0..3 {
                for q in (p + 1)..3 {
                    let g = hundred * a.data[p][q].abs();

                    if iter > 3
                        && (eigenvalues[p].abs() + g == eigenvalues[p].abs())
                        && (eigenvalues[q].abs() + g == eigenvalues[q].abs())
                    {
                        a.data[p][q] = T::ZERO;
                    } else if a.data[p][q].abs() > thresh {
                        let h = eigenvalues[q] - eigenvalues[p];
                        let t = if h.abs() + g == h.abs() {
                            a.data[p][q] / h
                        } else {
                            let theta = h / (two * a.data[p][q]);
                            let abs_theta = if theta < T::ZERO { -theta } else { theta };
                            T::ONE / (abs_theta + (T::ONE + theta * theta).sqrt())
                                * if theta < T::ZERO { -T::ONE } else { T::ONE }
                        };

                        let c = T::ONE / (T::ONE + t * t).sqrt();
                        let s = t * c;
                        let tau = s / (T::ONE + c);
                        let h = t * a.data[p][q];

                        z[p] -= h;
                        z[q] += h;
                        eigenvalues[p] -= h;
                        eigenvalues[q] += h;
                        a.data[p][q] = T::ZERO;

                        // Rotate rows 0..p
                        for j in 0..p {
                            let g = a.data[j][p];
                            let h = a.data[j][q];
                            a.data[j][p] = g - s * (h + g * tau);
                            a.data[j][q] = h + s * (g - h * tau);
                        }

                        // Rotate rows p+1..q
                        for j in (p + 1)..q {
                            let g = a.data[p][j];
                            let h = a.data[j][q];
                            a.data[p][j] = g - s * (h + g * tau);
                            a.data[j][q] = h + s * (g - h * tau);
                        }

                        // Rotate rows q+1..3
                        for j in (q + 1)..3 {
                            let g = a.data[p][j];
                            let h = a.data[q][j];
                            a.data[p][j] = g - s * (h + g * tau);
                            a.data[q][j] = h + s * (g - h * tau);
                        }

                        // Update eigenvectors
                        for ev in &mut eigenvectors {
                            let g = ev[p];
                            let h = ev[q];
                            ev[p] = g - s * (h + g * tau);
                            ev[q] = h + s * (g - h * tau);
                        }
                    }
                }
            }

            for p in 0..3 {
                eigenvalues[p] = b[p] + z[p];
                b[p] = eigenvalues[p];
                z[p] = T::ZERO;
            }
        }

        (eigenvalues, eigenvectors)
    }
}

// Default - identity matrix
impl<T: Scalar> Default for Matrix4<T> {
    fn default() -> Self {
        Self::identity()
    }
}

// Indexing by row
impl<T> Index<usize> for Matrix4<T> {
    type Output = [T; 4];

    #[inline]
    fn index(&self, i: usize) -> &[T; 4] {
        &self.data[i]
    }
}

impl<T> IndexMut<usize> for Matrix4<T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut [T; 4] {
        &mut self.data[i]
    }
}

// Equality
impl<T: PartialEq> PartialEq for Matrix4<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T: Eq> Eq for Matrix4<T> {}

// Hash
impl<T: Hash> Hash for Matrix4<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for row in &self.data {
            for elem in row {
                elem.hash(state);
            }
        }
    }
}

// Negation
impl<T: Scalar> Neg for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(
            -self.data[0][0],
            -self.data[0][1],
            -self.data[0][2],
            -self.data[0][3],
            -self.data[1][0],
            -self.data[1][1],
            -self.data[1][2],
            -self.data[1][3],
            -self.data[2][0],
            -self.data[2][1],
            -self.data[2][2],
            -self.data[2][3],
            -self.data[3][0],
            -self.data[3][1],
            -self.data[3][2],
            -self.data[3][3],
        )
    }
}

// Matrix addition
impl<T: Scalar> Add for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] + rhs.data[0][0],
            self.data[0][1] + rhs.data[0][1],
            self.data[0][2] + rhs.data[0][2],
            self.data[0][3] + rhs.data[0][3],
            self.data[1][0] + rhs.data[1][0],
            self.data[1][1] + rhs.data[1][1],
            self.data[1][2] + rhs.data[1][2],
            self.data[1][3] + rhs.data[1][3],
            self.data[2][0] + rhs.data[2][0],
            self.data[2][1] + rhs.data[2][1],
            self.data[2][2] + rhs.data[2][2],
            self.data[2][3] + rhs.data[2][3],
            self.data[3][0] + rhs.data[3][0],
            self.data[3][1] + rhs.data[3][1],
            self.data[3][2] + rhs.data[3][2],
            self.data[3][3] + rhs.data[3][3],
        )
    }
}

impl<T: Scalar> AddAssign for Matrix4<T> {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        for i in 0..4 {
            for j in 0..4 {
                self.data[i][j] = self.data[i][j] + rhs.data[i][j];
            }
        }
    }
}

// Matrix subtraction
impl<T: Scalar> Sub for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.data[0][0] - rhs.data[0][0],
            self.data[0][1] - rhs.data[0][1],
            self.data[0][2] - rhs.data[0][2],
            self.data[0][3] - rhs.data[0][3],
            self.data[1][0] - rhs.data[1][0],
            self.data[1][1] - rhs.data[1][1],
            self.data[1][2] - rhs.data[1][2],
            self.data[1][3] - rhs.data[1][3],
            self.data[2][0] - rhs.data[2][0],
            self.data[2][1] - rhs.data[2][1],
            self.data[2][2] - rhs.data[2][2],
            self.data[2][3] - rhs.data[2][3],
            self.data[3][0] - rhs.data[3][0],
            self.data[3][1] - rhs.data[3][1],
            self.data[3][2] - rhs.data[3][2],
            self.data[3][3] - rhs.data[3][3],
        )
    }
}

impl<T: Scalar> SubAssign for Matrix4<T> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        for i in 0..4 {
            for j in 0..4 {
                self.data[i][j] = self.data[i][j] - rhs.data[i][j];
            }
        }
    }
}

// Matrix-matrix multiplication (unrolled for performance)
impl<T: Scalar> Mul for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let a = &self.data;
        let b = &rhs.data;

        Self::new(
            // Row 0
            a[0][0] * b[0][0] + a[0][1] * b[1][0] + a[0][2] * b[2][0] + a[0][3] * b[3][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1] + a[0][2] * b[2][1] + a[0][3] * b[3][1],
            a[0][0] * b[0][2] + a[0][1] * b[1][2] + a[0][2] * b[2][2] + a[0][3] * b[3][2],
            a[0][0] * b[0][3] + a[0][1] * b[1][3] + a[0][2] * b[2][3] + a[0][3] * b[3][3],
            // Row 1
            a[1][0] * b[0][0] + a[1][1] * b[1][0] + a[1][2] * b[2][0] + a[1][3] * b[3][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1] + a[1][2] * b[2][1] + a[1][3] * b[3][1],
            a[1][0] * b[0][2] + a[1][1] * b[1][2] + a[1][2] * b[2][2] + a[1][3] * b[3][2],
            a[1][0] * b[0][3] + a[1][1] * b[1][3] + a[1][2] * b[2][3] + a[1][3] * b[3][3],
            // Row 2
            a[2][0] * b[0][0] + a[2][1] * b[1][0] + a[2][2] * b[2][0] + a[2][3] * b[3][0],
            a[2][0] * b[0][1] + a[2][1] * b[1][1] + a[2][2] * b[2][1] + a[2][3] * b[3][1],
            a[2][0] * b[0][2] + a[2][1] * b[1][2] + a[2][2] * b[2][2] + a[2][3] * b[3][2],
            a[2][0] * b[0][3] + a[2][1] * b[1][3] + a[2][2] * b[2][3] + a[2][3] * b[3][3],
            // Row 3
            a[3][0] * b[0][0] + a[3][1] * b[1][0] + a[3][2] * b[2][0] + a[3][3] * b[3][0],
            a[3][0] * b[0][1] + a[3][1] * b[1][1] + a[3][2] * b[2][1] + a[3][3] * b[3][1],
            a[3][0] * b[0][2] + a[3][1] * b[1][2] + a[3][2] * b[2][2] + a[3][3] * b[3][2],
            a[3][0] * b[0][3] + a[3][1] * b[1][3] + a[3][2] * b[2][3] + a[3][3] * b[3][3],
        )
    }
}

impl<T: Scalar> MulAssign for Matrix4<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

// Scalar multiplication
impl<T: Scalar> Mul<T> for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: T) -> Self {
        Self::new(
            self.data[0][0] * rhs,
            self.data[0][1] * rhs,
            self.data[0][2] * rhs,
            self.data[0][3] * rhs,
            self.data[1][0] * rhs,
            self.data[1][1] * rhs,
            self.data[1][2] * rhs,
            self.data[1][3] * rhs,
            self.data[2][0] * rhs,
            self.data[2][1] * rhs,
            self.data[2][2] * rhs,
            self.data[2][3] * rhs,
            self.data[3][0] * rhs,
            self.data[3][1] * rhs,
            self.data[3][2] * rhs,
            self.data[3][3] * rhs,
        )
    }
}

impl<T: Scalar> MulAssign<T> for Matrix4<T> {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        for i in 0..4 {
            for j in 0..4 {
                self.data[i][j] = self.data[i][j] * rhs;
            }
        }
    }
}

// Scalar on left
impl Mul<Matrix4<f64>> for f64 {
    type Output = Matrix4<f64>;

    #[inline]
    fn mul(self, rhs: Matrix4<f64>) -> Matrix4<f64> {
        rhs * self
    }
}

impl Mul<Matrix4<f32>> for f32 {
    type Output = Matrix4<f32>;

    #[inline]
    fn mul(self, rhs: Matrix4<f32>) -> Matrix4<f32> {
        rhs * self
    }
}

// Matrix-vector multiplication (M * v = column vector result)
impl<T: Scalar> Mul<Vec4<T>> for Matrix4<T> {
    type Output = Vec4<T>;

    #[inline]
    fn mul(self, v: Vec4<T>) -> Vec4<T> {
        Vec4 {
            x: self.data[0][0] * v.x
                + self.data[0][1] * v.y
                + self.data[0][2] * v.z
                + self.data[0][3] * v.w,
            y: self.data[1][0] * v.x
                + self.data[1][1] * v.y
                + self.data[1][2] * v.z
                + self.data[1][3] * v.w,
            z: self.data[2][0] * v.x
                + self.data[2][1] * v.y
                + self.data[2][2] * v.z
                + self.data[2][3] * v.w,
            w: self.data[3][0] * v.x
                + self.data[3][1] * v.y
                + self.data[3][2] * v.z
                + self.data[3][3] * v.w,
        }
    }
}

// Vector-matrix multiplication (v * M = row vector result)
impl Mul<Matrix4<f64>> for Vec4<f64> {
    type Output = Vec4<f64>;

    #[inline]
    fn mul(self, m: Matrix4<f64>) -> Vec4<f64> {
        Vec4 {
            x: self.x * m.data[0][0]
                + self.y * m.data[1][0]
                + self.z * m.data[2][0]
                + self.w * m.data[3][0],
            y: self.x * m.data[0][1]
                + self.y * m.data[1][1]
                + self.z * m.data[2][1]
                + self.w * m.data[3][1],
            z: self.x * m.data[0][2]
                + self.y * m.data[1][2]
                + self.z * m.data[2][2]
                + self.w * m.data[3][2],
            w: self.x * m.data[0][3]
                + self.y * m.data[1][3]
                + self.z * m.data[2][3]
                + self.w * m.data[3][3],
        }
    }
}

impl Mul<Matrix4<f32>> for Vec4<f32> {
    type Output = Vec4<f32>;

    #[inline]
    fn mul(self, m: Matrix4<f32>) -> Vec4<f32> {
        Vec4 {
            x: self.x * m.data[0][0]
                + self.y * m.data[1][0]
                + self.z * m.data[2][0]
                + self.w * m.data[3][0],
            y: self.x * m.data[0][1]
                + self.y * m.data[1][1]
                + self.z * m.data[2][1]
                + self.w * m.data[3][1],
            z: self.x * m.data[0][2]
                + self.y * m.data[1][2]
                + self.z * m.data[2][2]
                + self.w * m.data[3][2],
            w: self.x * m.data[0][3]
                + self.y * m.data[1][3]
                + self.z * m.data[2][3]
                + self.w * m.data[3][3],
        }
    }
}

// Matrix division (m1 / m2 = m1 * inverse(m2))
#[allow(clippy::suspicious_arithmetic_impl)]
impl<T: Scalar> Div for Matrix4<T> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inverse().expect("Cannot divide by singular matrix")
    }
}

// Display
impl<T: fmt::Display> fmt::Display for Matrix4<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[[{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}]]",
            self.data[0][0],
            self.data[0][1],
            self.data[0][2],
            self.data[0][3],
            self.data[1][0],
            self.data[1][1],
            self.data[1][2],
            self.data[1][3],
            self.data[2][0],
            self.data[2][1],
            self.data[2][2],
            self.data[2][3],
            self.data[3][0],
            self.data[3][1],
            self.data[3][2],
            self.data[3][3],
        )
    }
}

/// Creates a Matrix4d from elements (row-major order).
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matrix4d(
    m00: f64,
    m01: f64,
    m02: f64,
    m03: f64,
    m10: f64,
    m11: f64,
    m12: f64,
    m13: f64,
    m20: f64,
    m21: f64,
    m22: f64,
    m23: f64,
    m30: f64,
    m31: f64,
    m32: f64,
    m33: f64,
) -> Matrix4d {
    Matrix4d::new(
        m00, m01, m02, m03, m10, m11, m12, m13, m20, m21, m22, m23, m30, m31, m32, m33,
    )
}

/// Creates a Matrix4f from elements (row-major order).
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matrix4f(
    m00: f32,
    m01: f32,
    m02: f32,
    m03: f32,
    m10: f32,
    m11: f32,
    m12: f32,
    m13: f32,
    m20: f32,
    m21: f32,
    m22: f32,
    m23: f32,
    m30: f32,
    m31: f32,
    m32: f32,
    m33: f32,
) -> Matrix4f {
    Matrix4f::new(
        m00, m01, m02, m03, m10, m11, m12, m13, m20, m21, m22, m23, m30, m31, m32, m33,
    )
}

// Cross-type conversions (matching C++ explicit conversions)
// Conversion from quaternions to matrices
impl<T: Scalar + Float> From<&crate::quat::Quat<T>> for Matrix4<T> {
    fn from(quat: &crate::quat::Quat<T>) -> Self {
        let mut m = Self::identity();
        m.set_rotate(quat);
        m
    }
}

impl From<Matrix4f> for Matrix4d {
    fn from(other: Matrix4f) -> Self {
        Self::new(
            other[0][0] as f64,
            other[0][1] as f64,
            other[0][2] as f64,
            other[0][3] as f64,
            other[1][0] as f64,
            other[1][1] as f64,
            other[1][2] as f64,
            other[1][3] as f64,
            other[2][0] as f64,
            other[2][1] as f64,
            other[2][2] as f64,
            other[2][3] as f64,
            other[3][0] as f64,
            other[3][1] as f64,
            other[3][2] as f64,
            other[3][3] as f64,
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Matrix4f> for Matrix4d {
    fn eq(&self, other: &Matrix4f) -> bool {
        (self[0][0] - other[0][0] as f64).abs() < f64::EPSILON
            && (self[0][1] - other[0][1] as f64).abs() < f64::EPSILON
            && (self[0][2] - other[0][2] as f64).abs() < f64::EPSILON
            && (self[0][3] - other[0][3] as f64).abs() < f64::EPSILON
            && (self[1][0] - other[1][0] as f64).abs() < f64::EPSILON
            && (self[1][1] - other[1][1] as f64).abs() < f64::EPSILON
            && (self[1][2] - other[1][2] as f64).abs() < f64::EPSILON
            && (self[1][3] - other[1][3] as f64).abs() < f64::EPSILON
            && (self[2][0] - other[2][0] as f64).abs() < f64::EPSILON
            && (self[2][1] - other[2][1] as f64).abs() < f64::EPSILON
            && (self[2][2] - other[2][2] as f64).abs() < f64::EPSILON
            && (self[2][3] - other[2][3] as f64).abs() < f64::EPSILON
            && (self[3][0] - other[3][0] as f64).abs() < f64::EPSILON
            && (self[3][1] - other[3][1] as f64).abs() < f64::EPSILON
            && (self[3][2] - other[3][2] as f64).abs() < f64::EPSILON
            && (self[3][3] - other[3][3] as f64).abs() < f64::EPSILON
    }
}

// Global function (matching C++ GfIsClose)
/// Tests for equality within a given tolerance.
///
/// Matches C++ `GfIsClose(GfMatrix4d const &m1, GfMatrix4d const &m2, double tolerance)`.
#[inline]
#[must_use]
pub fn is_close<T: Scalar>(m1: &Matrix4<T>, m2: &Matrix4<T>, tolerance: T) -> bool {
    m1.is_close(m2, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3d;
    use crate::vec4::Vec4d;
    use std::f64::consts::PI;

    #[test]
    fn test_identity() {
        let m = Matrix4d::identity();
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert_eq!(m[i][j], expected);
            }
        }
    }

    #[test]
    fn test_zero() {
        let m = Matrix4d::zero();
        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(m[i][j], 0.0);
            }
        }
    }

    #[test]
    fn test_from_diagonal() {
        let m = Matrix4d::from_diagonal_values(1.0, 2.0, 3.0, 4.0);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[1][1], 2.0);
        assert_eq!(m[2][2], 3.0);
        assert_eq!(m[3][3], 4.0);
        assert_eq!(m[0][1], 0.0);
    }

    #[test]
    fn test_from_translation() {
        let t = Vec3d::new(1.0, 2.0, 3.0);
        let m = Matrix4d::from_translation(t);
        assert_eq!(m[3][0], 1.0);
        assert_eq!(m[3][1], 2.0);
        assert_eq!(m[3][2], 3.0);
        assert_eq!(m[3][3], 1.0);
    }

    #[test]
    fn test_transform_point_translation() {
        let trans = Matrix4d::from_translation(Vec3d::new(1.0, 2.0, 3.0));
        let p = Vec3d::new(0.0, 0.0, 0.0);
        let result = trans.transform_point(&p);
        assert!((result.x - 1.0).abs() < 1e-10);
        assert!((result.y - 2.0).abs() < 1e-10);
        assert!((result.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_dir_ignores_translation() {
        let trans = Matrix4d::from_translation(Vec3d::new(100.0, 200.0, 300.0));
        let dir = Vec3d::new(1.0, 0.0, 0.0);
        let result = trans.transform_dir(&dir);
        assert!((result.x - 1.0).abs() < 1e-10);
        assert!(result.y.abs() < 1e-10);
        assert!(result.z.abs() < 1e-10);
    }

    #[test]
    fn test_scale() {
        let scale = Matrix4d::from_scale(2.0);
        let p = Vec3d::new(1.0, 2.0, 3.0);
        let result = scale.transform_point(&p);
        assert!((result.x - 2.0).abs() < 1e-10);
        assert!((result.y - 4.0).abs() < 1e-10);
        assert!((result.z - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_rotation() {
        // 90 degree rotation around Z axis
        let rot = Matrix4d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), PI / 2.0);
        let x = Vec3d::new(1.0, 0.0, 0.0);
        let result = rot.transform_dir(&x);
        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
        assert!(result.z.abs() < 1e-10);
    }

    #[test]
    fn test_transpose() {
        let m = Matrix4d::new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let t = m.transpose();
        assert_eq!(t[0][1], 5.0);
        assert_eq!(t[1][0], 2.0);
        assert_eq!(t[0][3], 13.0);
    }

    #[test]
    fn test_determinant_identity() {
        let m = Matrix4d::identity();
        assert!((m.determinant() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_determinant_scale() {
        let m = Matrix4d::from_scale(2.0);
        // Scale matrix has det = 2^3 * 1 = 8
        assert!((m.determinant() - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_identity() {
        let m = Matrix4d::identity();
        let inv = m.inverse().unwrap();
        assert!(inv.is_close(&Matrix4d::identity(), 1e-10));
    }

    #[test]
    fn test_inverse_scale() {
        let m = Matrix4d::from_scale(2.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;
        assert!(product.is_close(&Matrix4d::identity(), 1e-10));
    }

    #[test]
    fn test_inverse_translation() {
        let m = Matrix4d::from_translation(Vec3d::new(1.0, 2.0, 3.0));
        let inv = m.inverse().unwrap();
        let product = m * inv;
        assert!(product.is_close(&Matrix4d::identity(), 1e-10));
    }

    #[test]
    fn test_inverse_rotation() {
        let m = Matrix4d::from_rotation(Vec3d::new(1.0, 1.0, 1.0), PI / 4.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;
        assert!(product.is_close(&Matrix4d::identity(), 1e-10));
    }

    #[test]
    fn test_matrix_multiplication() {
        let a = Matrix4d::from_translation(Vec3d::new(1.0, 0.0, 0.0));
        let b = Matrix4d::from_scale(2.0);
        let c = a * b;

        // Point at origin: first translate, then scale
        let p = Vec3d::new(0.0, 0.0, 0.0);
        let result = c.transform_point(&p);
        // Translation then scale: (0+1)*2 = 2
        assert!((result.x - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_scalar_multiplication() {
        let m = Matrix4d::identity();
        let scaled = m * 2.0;
        assert_eq!(scaled[0][0], 2.0);
        assert_eq!(scaled[1][1], 2.0);
    }

    #[test]
    fn test_matrix_vector_multiplication() {
        let m = Matrix4d::identity();
        let v = Vec4d::new(1.0, 2.0, 3.0, 1.0);
        let result = m * v;
        assert_eq!(result, v);
    }

    #[test]
    fn test_extract_translation() {
        let m = Matrix4d::from_translation(Vec3d::new(1.0, 2.0, 3.0));
        let t = m.extract_translation();
        assert!((t.x - 1.0).abs() < 1e-10);
        assert!((t.y - 2.0).abs() < 1e-10);
        assert!((t.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_extract_rotation_matrix() {
        let rot3 = crate::matrix3::Matrix3d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), PI / 4.0);
        let m = Matrix4d::from_rotation_translation(&rot3, &Vec3d::new(0.0, 0.0, 0.0));
        let extracted = m.extract_rotation_matrix();
        assert!(extracted.is_close(&rot3, 1e-10));
    }

    #[test]
    fn test_orthonormalize() {
        let mut m = Matrix4d::from_scale(2.0);
        assert!(m.orthonormalize());
        // Upper 3x3 should now be identity
        let rot = m.extract_rotation_matrix();
        assert!(rot.is_close(&crate::matrix3::Matrix3d::identity(), 1e-10));
    }

    #[test]
    fn test_handedness() {
        let m = Matrix4d::identity();
        assert!(m.is_right_handed());
        assert!(!m.is_left_handed());

        // Flip one axis
        let flip = Matrix4d::from_scale_vec(&Vec3d::new(-1.0, 1.0, 1.0));
        assert!(flip.is_left_handed());
        assert!(!flip.is_right_handed());
    }

    #[test]
    fn test_has_orthogonal_rows() {
        let m = Matrix4d::identity();
        assert!(m.has_orthogonal_rows3());

        // Rotation is orthogonal
        let rot = Matrix4d::from_rotation(Vec3d::new(1.0, 1.0, 1.0), PI / 4.0);
        assert!(rot.has_orthogonal_rows3());
    }

    #[test]
    fn test_look_at() {
        let eye = Vec3d::new(0.0, 0.0, 5.0);
        let center = Vec3d::new(0.0, 0.0, 0.0);
        let up = Vec3d::new(0.0, 1.0, 0.0);

        let view = Matrix4d::look_at(&eye, &center, &up);

        // Transform the eye point should give origin
        let transformed_eye = view.transform_point(&eye);
        assert!(transformed_eye.x.abs() < 1e-10);
        assert!(transformed_eye.y.abs() < 1e-10);
        assert!(transformed_eye.z.abs() < 1e-10);
    }

    #[test]
    fn test_negation() {
        let m = Matrix4d::identity();
        let neg = -m;
        assert_eq!(neg[0][0], -1.0);
        assert_eq!(neg[1][1], -1.0);
    }

    #[test]
    fn test_addition() {
        let a = Matrix4d::identity();
        let b = Matrix4d::identity();
        let sum = a + b;
        assert_eq!(sum[0][0], 2.0);
        assert_eq!(sum[0][1], 0.0);
    }

    #[test]
    fn test_subtraction() {
        let a = Matrix4d::identity();
        let b = Matrix4d::identity();
        let diff = a - b;
        assert!(diff.is_close(&Matrix4d::zero(), 1e-10));
    }

    #[test]
    fn test_helper_functions() {
        let m = matrix4d(
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        assert!(m.is_close(&Matrix4d::identity(), 1e-10));
    }

    #[test]
    fn test_display() {
        let m = Matrix4d::identity();
        let s = format!("{}", m);
        assert!(s.contains("1"));
        assert!(s.contains("0"));
    }

    #[test]
    fn test_singular_inverse() {
        // Singular matrix (row 1 = row 0)
        let m = Matrix4d::new(
            1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        );
        assert!(m.inverse().is_none());
    }
}
