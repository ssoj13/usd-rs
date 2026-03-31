//! Imath-compatible vector, color, and matrix types.
//!
//! All types use `#[repr(C)]` to guarantee binary compatibility with the
//! corresponding Imath types used by OSL/OIIO.

use std::fmt;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

use crate::Float;

// ---------------------------------------------------------------------------
// Vec2
// ---------------------------------------------------------------------------

/// 2D vector, binary-compatible with `Imath::Vec2<float>`.
#[derive(Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Vec2 {
    pub x: Float,
    pub y: Float,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    #[inline]
    pub const fn new(x: Float, y: Float) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn splat(v: Float) -> Self {
        Self { x: v, y: v }
    }

    #[inline]
    pub fn dot(self, other: Self) -> Float {
        self.x * other.x + self.y * other.y
    }

    #[inline]
    pub fn length_squared(self) -> Float {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> Float {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len > 0.0 { self / len } else { Self::ZERO }
    }
}

impl fmt::Debug for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec2({}, {})", self.x, self.y)
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl From<Float> for Vec2 {
    #[inline]
    fn from(v: Float) -> Self {
        Self::splat(v)
    }
}

impl From<[Float; 2]> for Vec2 {
    #[inline]
    fn from(a: [Float; 2]) -> Self {
        Self { x: a[0], y: a[1] }
    }
}

impl From<Vec2> for [Float; 2] {
    #[inline]
    fn from(v: Vec2) -> Self {
        [v.x, v.y]
    }
}

impl Index<usize> for Vec2 {
    type Output = Float;
    #[inline]
    fn index(&self, i: usize) -> &Float {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("Vec2 index {i} out of range"),
        }
    }
}

impl IndexMut<usize> for Vec2 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut Float {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => panic!("Vec2 index {i} out of range"),
        }
    }
}

impl Neg for Vec2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<Float> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Float) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vec2> for Float {
    type Output = Vec2;
    #[inline]
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl Div<Float> for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Float) -> Self {
        let inv = 1.0 / rhs;
        Self {
            x: self.x * inv,
            y: self.y * inv,
        }
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl MulAssign<Float> for Vec2 {
    #[inline]
    fn mul_assign(&mut self, rhs: Float) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl DivAssign<Float> for Vec2 {
    #[inline]
    fn div_assign(&mut self, rhs: Float) {
        let inv = 1.0 / rhs;
        self.x *= inv;
        self.y *= inv;
    }
}

// ---------------------------------------------------------------------------
// Vec3
// ---------------------------------------------------------------------------

/// 3D vector, binary-compatible with `Imath::Vec3<float>`.
#[derive(Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Vec3 {
    pub x: Float,
    pub y: Float,
    pub z: Float,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    #[inline]
    pub const fn new(x: Float, y: Float, z: Float) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn splat(v: Float) -> Self {
        Self { x: v, y: v, z: v }
    }

    #[inline]
    pub fn dot(self, other: Self) -> Float {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    #[inline]
    pub fn length_squared(self) -> Float {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> Float {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len > 0.0 { self / len } else { Self::ZERO }
    }

    #[inline]
    pub fn distance(self, other: Self) -> Float {
        (self - other).length()
    }

    /// Component-wise multiplication.
    #[inline]
    pub fn comp_mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }

    /// Component-wise division.
    #[inline]
    pub fn comp_div(self, other: Self) -> Self {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
        }
    }

    /// Reflect vector off a surface with the given normal.
    #[inline]
    pub fn reflect(self, normal: Self) -> Self {
        self - 2.0 * self.dot(normal) * normal
    }

    /// Face-forward: flip `self` if it points away from `I` relative to `Nref`.
    #[inline]
    pub fn faceforward(self, i: Self, nref: Self) -> Self {
        if nref.dot(i) < 0.0 { self } else { -self }
    }
}

impl fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec3({}, {}, {})", self.x, self.y, self.z)
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl From<Float> for Vec3 {
    #[inline]
    fn from(v: Float) -> Self {
        Self::splat(v)
    }
}

impl From<[Float; 3]> for Vec3 {
    #[inline]
    fn from(a: [Float; 3]) -> Self {
        Self {
            x: a[0],
            y: a[1],
            z: a[2],
        }
    }
}

impl From<Vec3> for [Float; 3] {
    #[inline]
    fn from(v: Vec3) -> Self {
        [v.x, v.y, v.z]
    }
}

impl Index<usize> for Vec3 {
    type Output = Float;
    #[inline]
    fn index(&self, i: usize) -> &Float {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 index {i} out of range"),
        }
    }
}

impl IndexMut<usize> for Vec3 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut Float {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("Vec3 index {i} out of range"),
        }
    }
}

impl Neg for Vec3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul<Float> for Vec3 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Float) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Mul<Vec3> for Float {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        Vec3 {
            x: self * rhs.x,
            y: self * rhs.y,
            z: self * rhs.z,
        }
    }
}

impl Div<Float> for Vec3 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Float) -> Self {
        let inv = 1.0 / rhs;
        Self {
            x: self.x * inv,
            y: self.y * inv,
            z: self.z * inv,
        }
    }
}

impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl SubAssign for Vec3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl MulAssign<Float> for Vec3 {
    #[inline]
    fn mul_assign(&mut self, rhs: Float) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl DivAssign<Float> for Vec3 {
    #[inline]
    fn div_assign(&mut self, rhs: Float) {
        let inv = 1.0 / rhs;
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
    }
}

// ---------------------------------------------------------------------------
// Color3 — type alias with same layout as Vec3
// ---------------------------------------------------------------------------

/// RGB color, binary-compatible with `Imath::Color3<float>`.
/// Same physical layout as [`Vec3`] (three contiguous `f32`).
pub type Color3 = Vec3;

// ---------------------------------------------------------------------------
// Matrix22
// ---------------------------------------------------------------------------

/// 2×2 matrix, row-major, binary-compatible with `Imath::Matrix22<float>`.
#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Matrix22 {
    pub m: [[Float; 2]; 2],
}

impl Matrix22 {
    pub const ZERO: Self = Self { m: [[0.0; 2]; 2] };

    pub const IDENTITY: Self = Self {
        m: [[1.0, 0.0], [0.0, 1.0]],
    };

    #[inline]
    pub const fn new(m00: Float, m01: Float, m10: Float, m11: Float) -> Self {
        Self {
            m: [[m00, m01], [m10, m11]],
        }
    }

    #[inline]
    pub fn determinant(&self) -> Float {
        self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]
    }

    #[inline]
    pub fn transpose(&self) -> Self {
        Self::new(self.m[0][0], self.m[1][0], self.m[0][1], self.m[1][1])
    }

    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < Float::EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self::new(
            self.m[1][1] * inv_det,
            -self.m[0][1] * inv_det,
            -self.m[1][0] * inv_det,
            self.m[0][0] * inv_det,
        ))
    }
}

impl Default for Matrix22 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl fmt::Debug for Matrix22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matrix22([{}, {}], [{}, {}])",
            self.m[0][0], self.m[0][1], self.m[1][0], self.m[1][1]
        )
    }
}

impl Mul for Matrix22 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..2 {
            for j in 0..2 {
                r.m[i][j] = self.m[i][0] * rhs.m[0][j] + self.m[i][1] * rhs.m[1][j];
            }
        }
        r
    }
}

// ---------------------------------------------------------------------------
// Matrix33
// ---------------------------------------------------------------------------

/// 3×3 matrix, row-major, binary-compatible with `Imath::Matrix33<float>`.
#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Matrix33 {
    pub m: [[Float; 3]; 3],
}

impl Matrix33 {
    pub const ZERO: Self = Self { m: [[0.0; 3]; 3] };

    pub const IDENTITY: Self = Self {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    pub fn determinant(&self) -> Float {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    pub fn transpose(&self) -> Self {
        let m = &self.m;
        Self {
            m: [
                [m[0][0], m[1][0], m[2][0]],
                [m[0][1], m[1][1], m[2][1]],
                [m[0][2], m[1][2], m[2][2]],
            ],
        }
    }

    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < Float::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        let m = &self.m;
        Some(Self {
            m: [
                [
                    (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv,
                    (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
                    (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
                ],
                [
                    (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv,
                    (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
                    (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
                ],
                [
                    (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv,
                    (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
                    (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
                ],
            ],
        })
    }
}

impl Default for Matrix33 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl fmt::Debug for Matrix33 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Matrix33({:?})", self.m)
    }
}

impl Mul for Matrix33 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..3 {
            for j in 0..3 {
                r.m[i][j] = self.m[i][0] * rhs.m[0][j]
                    + self.m[i][1] * rhs.m[1][j]
                    + self.m[i][2] * rhs.m[2][j];
            }
        }
        r
    }
}

// ---------------------------------------------------------------------------
// Matrix44
// ---------------------------------------------------------------------------

/// 4×4 matrix, row-major, binary-compatible with `Imath::Matrix44<float>`.
#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Matrix44 {
    pub m: [[Float; 4]; 4],
}

impl Matrix44 {
    pub const ZERO: Self = Self { m: [[0.0; 4]; 4] };

    pub const IDENTITY: Self = Self {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Construct from a flat 16-element array in row-major order.
    pub fn from_row_major(arr: &[f32; 16]) -> Self {
        Self {
            m: [
                [arr[0], arr[1], arr[2], arr[3]],
                [arr[4], arr[5], arr[6], arr[7]],
                [arr[8], arr[9], arr[10], arr[11]],
                [arr[12], arr[13], arr[14], arr[15]],
            ],
        }
    }

    /// Construct from a flat 16-element array in column-major order (transposes on load).
    pub fn from_col_major(arr: &[f32; 16]) -> Self {
        Self {
            m: [
                [arr[0], arr[4], arr[8], arr[12]],
                [arr[1], arr[5], arr[9], arr[13]],
                [arr[2], arr[6], arr[10], arr[14]],
                [arr[3], arr[7], arr[11], arr[15]],
            ],
        }
    }

    /// Deprecated alias for `from_row_major`.
    #[deprecated(note = "use from_row_major instead")]
    pub fn from_cols_array(arr: &[f32; 16]) -> Self {
        Self::from_row_major(arr)
    }

    /// Multiply two 4×4 matrices.
    pub fn mul_matrix(&self, rhs: &Self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..4 {
            for j in 0..4 {
                r.m[i][j] = self.m[i][0] * rhs.m[0][j]
                    + self.m[i][1] * rhs.m[1][j]
                    + self.m[i][2] * rhs.m[2][j]
                    + self.m[i][3] * rhs.m[3][j];
            }
        }
        r
    }

    /// Transpose the matrix.
    pub fn transpose(&self) -> Self {
        let m = &self.m;
        Self {
            m: [
                [m[0][0], m[1][0], m[2][0], m[3][0]],
                [m[0][1], m[1][1], m[2][1], m[3][1]],
                [m[0][2], m[1][2], m[2][2], m[3][2]],
                [m[0][3], m[1][3], m[2][3], m[3][3]],
            ],
        }
    }

    /// Transform a point (applies full transform including translation).
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        // Imath row-vector convention: v' = v * M
        let m = &self.m;
        let w = p.x * m[0][3] + p.y * m[1][3] + p.z * m[2][3] + m[3][3];
        let inv_w = if w.abs() > Float::EPSILON {
            1.0 / w
        } else {
            1.0
        };
        Vec3 {
            x: (p.x * m[0][0] + p.y * m[1][0] + p.z * m[2][0] + m[3][0]) * inv_w,
            y: (p.x * m[0][1] + p.y * m[1][1] + p.z * m[2][1] + m[3][1]) * inv_w,
            z: (p.x * m[0][2] + p.y * m[1][2] + p.z * m[2][2] + m[3][2]) * inv_w,
        }
    }

    /// Transform a vector (no translation, no perspective divide).
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        // Imath row-vector convention: v' = v * M (no translation)
        let m = &self.m;
        Vec3 {
            x: v.x * m[0][0] + v.y * m[1][0] + v.z * m[2][0],
            y: v.x * m[0][1] + v.y * m[1][1] + v.z * m[2][1],
            z: v.x * m[0][2] + v.y * m[1][2] + v.z * m[2][2],
        }
    }

    /// Transform a normal (multiplied by inverse-transpose of upper 3×3).
    pub fn transform_normal(&self, n: Vec3) -> Vec3 {
        // Normal transform = inverse-transpose: n' = n * (M^-1)^T
        // Matching C++: multDirMatrix(inlinedTransposed(M.inverse()), v, result)
        if let Some(inv) = self.inverse() {
            // transpose of inv, then multDirMatrix (= transform_vector on transposed)
            let m = &inv.m;
            Vec3 {
                x: n.x * m[0][0] + n.y * m[0][1] + n.z * m[0][2],
                y: n.x * m[1][0] + n.y * m[1][1] + n.z * m[1][2],
                z: n.x * m[2][0] + n.y * m[2][1] + n.z * m[2][2],
            }
        } else {
            // Singular matrix — fallback to transform_vector
            self.transform_vector(n)
        }
    }

    /// Compute the determinant.
    pub fn determinant(&self) -> Float {
        let m = &self.m;
        let a0 = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        let a1 = m[0][0] * m[1][2] - m[0][2] * m[1][0];
        let a2 = m[0][0] * m[1][3] - m[0][3] * m[1][0];
        let a3 = m[0][1] * m[1][2] - m[0][2] * m[1][1];
        let a4 = m[0][1] * m[1][3] - m[0][3] * m[1][1];
        let a5 = m[0][2] * m[1][3] - m[0][3] * m[1][2];
        let b0 = m[2][0] * m[3][1] - m[2][1] * m[3][0];
        let b1 = m[2][0] * m[3][2] - m[2][2] * m[3][0];
        let b2 = m[2][0] * m[3][3] - m[2][3] * m[3][0];
        let b3 = m[2][1] * m[3][2] - m[2][2] * m[3][1];
        let b4 = m[2][1] * m[3][3] - m[2][3] * m[3][1];
        let b5 = m[2][2] * m[3][3] - m[2][3] * m[3][2];
        a0 * b5 - a1 * b4 + a2 * b3 + a3 * b2 - a4 * b1 + a5 * b0
    }

    /// Compute the inverse. Returns `None` if the matrix is singular.
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.m;
        let a0 = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        let a1 = m[0][0] * m[1][2] - m[0][2] * m[1][0];
        let a2 = m[0][0] * m[1][3] - m[0][3] * m[1][0];
        let a3 = m[0][1] * m[1][2] - m[0][2] * m[1][1];
        let a4 = m[0][1] * m[1][3] - m[0][3] * m[1][1];
        let a5 = m[0][2] * m[1][3] - m[0][3] * m[1][2];
        let b0 = m[2][0] * m[3][1] - m[2][1] * m[3][0];
        let b1 = m[2][0] * m[3][2] - m[2][2] * m[3][0];
        let b2 = m[2][0] * m[3][3] - m[2][3] * m[3][0];
        let b3 = m[2][1] * m[3][2] - m[2][2] * m[3][1];
        let b4 = m[2][1] * m[3][3] - m[2][3] * m[3][1];
        let b5 = m[2][2] * m[3][3] - m[2][3] * m[3][2];

        let det = a0 * b5 - a1 * b4 + a2 * b3 + a3 * b2 - a4 * b1 + a5 * b0;
        if det.abs() < Float::EPSILON {
            return None;
        }
        let inv = 1.0 / det;

        Some(Self {
            m: [
                [
                    (m[1][1] * b5 - m[1][2] * b4 + m[1][3] * b3) * inv,
                    (-m[0][1] * b5 + m[0][2] * b4 - m[0][3] * b3) * inv,
                    (m[3][1] * a5 - m[3][2] * a4 + m[3][3] * a3) * inv,
                    (-m[2][1] * a5 + m[2][2] * a4 - m[2][3] * a3) * inv,
                ],
                [
                    (-m[1][0] * b5 + m[1][2] * b2 - m[1][3] * b1) * inv,
                    (m[0][0] * b5 - m[0][2] * b2 + m[0][3] * b1) * inv,
                    (-m[3][0] * a5 + m[3][2] * a2 - m[3][3] * a1) * inv,
                    (m[2][0] * a5 - m[2][2] * a2 + m[2][3] * a1) * inv,
                ],
                [
                    (m[1][0] * b4 - m[1][1] * b2 + m[1][3] * b0) * inv,
                    (-m[0][0] * b4 + m[0][1] * b2 - m[0][3] * b0) * inv,
                    (m[3][0] * a4 - m[3][1] * a2 + m[3][3] * a0) * inv,
                    (-m[2][0] * a4 + m[2][1] * a2 - m[2][3] * a0) * inv,
                ],
                [
                    (-m[1][0] * b3 + m[1][1] * b1 - m[1][2] * b0) * inv,
                    (m[0][0] * b3 - m[0][1] * b1 + m[0][2] * b0) * inv,
                    (-m[3][0] * a3 + m[3][1] * a1 - m[3][2] * a0) * inv,
                    (m[2][0] * a3 - m[2][1] * a1 + m[2][2] * a0) * inv,
                ],
            ],
        })
    }

    /// Create a translation matrix.
    pub fn translate(t: Vec3) -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, t.x],
                [0.0, 1.0, 0.0, t.y],
                [0.0, 0.0, 1.0, t.z],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create a uniform scale matrix.
    pub fn scale(s: Vec3) -> Self {
        Self {
            m: [
                [s.x, 0.0, 0.0, 0.0],
                [0.0, s.y, 0.0, 0.0],
                [0.0, 0.0, s.z, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

impl Default for Matrix44 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl fmt::Debug for Matrix44 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Matrix44({:?})", self.m)
    }
}

impl Mul for Matrix44 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.mul_matrix(&rhs)
    }
}

// ---------------------------------------------------------------------------
// Layout assertions
// ---------------------------------------------------------------------------
const _: () = assert!(std::mem::size_of::<Vec2>() == 8);
const _: () = assert!(std::mem::size_of::<Vec3>() == 12);
const _: () = assert!(std::mem::size_of::<Matrix22>() == 16);
const _: () = assert!(std::mem::size_of::<Matrix33>() == 36);
const _: () = assert!(std::mem::size_of::<Matrix44>() == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_basic() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
        assert_eq!(a - b, Vec3::new(-3.0, -3.0, -3.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a.dot(b), 32.0);
    }

    #[test]
    fn test_vec3_cross() {
        let x = Vec3::X;
        let y = Vec3::Y;
        let z = x.cross(y);
        assert!((z.x - 0.0).abs() < 1e-6);
        assert!((z.y - 0.0).abs() < 1e-6);
        assert!((z.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_vec3_normalize() {
        let v = Vec3::new(3.0, 0.0, 4.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert!((n.x - 0.6).abs() < 1e-6);
        assert!((n.z - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_matrix44_identity() {
        let m = Matrix44::IDENTITY;
        let p = Vec3::new(1.0, 2.0, 3.0);
        let tp = m.transform_point(p);
        assert!((tp.x - 1.0).abs() < 1e-6);
        assert!((tp.y - 2.0).abs() < 1e-6);
        assert!((tp.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_matrix44_inverse() {
        let m = Matrix44::translate(Vec3::new(1.0, 2.0, 3.0));
        let inv = m.inverse().unwrap();
        let result = m * inv;
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (result.m[i][j] - expected).abs() < 1e-5,
                    "m[{i}][{j}] = {} expected {expected}",
                    result.m[i][j]
                );
            }
        }
    }

    #[test]
    fn test_matrix44_determinant() {
        assert!((Matrix44::IDENTITY.determinant() - 1.0).abs() < 1e-6);
        let s = Matrix44::scale(Vec3::new(2.0, 3.0, 4.0));
        assert!((s.determinant() - 24.0).abs() < 1e-5);
    }
}
