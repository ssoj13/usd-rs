//! MaterialX data types -- vectors, colors, matrices, type strings.
//!
//! Matches C++ Types.h / Types.cpp from MaterialXCore.
//! Row-major storage; matrix indexing is [row][col].
//! Vector ops follow the row-vector convention (v' = v * M).

use std::fmt;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

// ---------------------------------------------------------------------------
// Type-string constants (MaterialX spec)
// ---------------------------------------------------------------------------

pub const DEFAULT_TYPE_STRING: &str = "color3";
pub const FILENAME_TYPE_STRING: &str = "filename";
pub const GEOMNAME_TYPE_STRING: &str = "geomname";
pub const STRING_TYPE_STRING: &str = "string";
pub const BSDF_TYPE_STRING: &str = "BSDF";
pub const EDF_TYPE_STRING: &str = "EDF";
pub const VDF_TYPE_STRING: &str = "VDF";
pub const SURFACE_SHADER_TYPE_STRING: &str = "surfaceshader";
pub const DISPLACEMENT_SHADER_TYPE_STRING: &str = "displacementshader";
pub const VOLUME_SHADER_TYPE_STRING: &str = "volumeshader";
pub const LIGHT_SHADER_TYPE_STRING: &str = "lightshader";
pub const MATERIAL_TYPE_STRING: &str = "material";
pub const SURFACE_MATERIAL_NODE_STRING: &str = "surfacematerial";
pub const VOLUME_MATERIAL_NODE_STRING: &str = "volumematerial";
pub const MULTI_OUTPUT_TYPE_STRING: &str = "multioutput";
pub const NONE_TYPE_STRING: &str = "none";

pub const VALUE_STRING_TRUE: &str = "true";
pub const VALUE_STRING_FALSE: &str = "false";

pub const NAME_PREFIX_SEPARATOR: char = ':';
pub const NAME_PATH_SEPARATOR: char = '/';
pub const ARRAY_VALID_SEPARATORS: &str = ", ";
pub const ARRAY_PREFERRED_SEPARATOR: char = ',';

// ---------------------------------------------------------------------------
// Vector2
// ---------------------------------------------------------------------------

/// Two-component float vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2(pub [f32; 2]);

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self([x, y])
    }
    pub fn splat(s: f32) -> Self {
        Self([s, s])
    }

    pub fn x(&self) -> f32 {
        self.0[0]
    }
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    /// 2-D "cross" product (scalar z-component of 3-D cross).
    pub fn cross(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[1] - self.0[1] * rhs.0[0]
    }

    pub fn dot(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[0] + self.0[1] * rhs.0[1]
    }

    pub fn magnitude(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(&self) -> Self {
        *self / self.magnitude()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl Index<usize> for Vector2 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl IndexMut<usize> for Vector2 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}

// component-wise vector ops
impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self([self.0[0] + rhs.0[0], self.0[1] + rhs.0[1]])
    }
}
impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self([self.0[0] - rhs.0[0], self.0[1] - rhs.0[1]])
    }
}
impl Mul for Vector2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self([self.0[0] * rhs.0[0], self.0[1] * rhs.0[1]])
    }
}
impl Div for Vector2 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self([self.0[0] / rhs.0[0], self.0[1] / rhs.0[1]])
    }
}

// scalar ops
impl Mul<f32> for Vector2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s])
    }
}
impl Div<f32> for Vector2 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self([self.0[0] / s, self.0[1] / s])
    }
}

// assign ops
impl AddAssign for Vector2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Vector2 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Vector2 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Vector2 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Vector2 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Vector2 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Vector2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1]])
    }
}

impl fmt::Display for Vector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.0[0], self.0[1])
    }
}

// ---------------------------------------------------------------------------
// Vector3
// ---------------------------------------------------------------------------

/// Three-component float vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3(pub [f32; 3]);

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self([x, y, z])
    }
    pub fn splat(s: f32) -> Self {
        Self([s, s, s])
    }

    pub fn x(&self) -> f32 {
        self.0[0]
    }
    pub fn y(&self) -> f32 {
        self.0[1]
    }
    pub fn z(&self) -> f32 {
        self.0[2]
    }

    pub fn dot(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[0] + self.0[1] * rhs.0[1] + self.0[2] * rhs.0[2]
    }

    /// Standard 3-D cross product.
    pub fn cross(&self, rhs: &Self) -> Self {
        Self([
            self.0[1] * rhs.0[2] - self.0[2] * rhs.0[1],
            self.0[2] * rhs.0[0] - self.0[0] * rhs.0[2],
            self.0[0] * rhs.0[1] - self.0[1] * rhs.0[0],
        ])
    }

    pub fn magnitude(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(&self) -> Self {
        *self / self.magnitude()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl Index<usize> for Vector3 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl IndexMut<usize> for Vector3 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
        ])
    }
}
impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
        ])
    }
}
impl Mul for Vector3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
        ])
    }
}
impl Div for Vector3 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self([
            self.0[0] / rhs.0[0],
            self.0[1] / rhs.0[1],
            self.0[2] / rhs.0[2],
        ])
    }
}

impl Mul<f32> for Vector3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s, self.0[2] * s])
    }
}
impl Div<f32> for Vector3 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self([self.0[0] / s, self.0[1] / s, self.0[2] / s])
    }
}

impl AddAssign for Vector3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Vector3 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Vector3 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Vector3 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Vector3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Vector3 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Vector3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2]])
    }
}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.0[0], self.0[1], self.0[2])
    }
}

// ---------------------------------------------------------------------------
// Vector4
// ---------------------------------------------------------------------------

/// Four-component float vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector4(pub [f32; 4]);

impl Vector4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w])
    }
    pub fn splat(s: f32) -> Self {
        Self([s, s, s, s])
    }

    pub fn x(&self) -> f32 {
        self.0[0]
    }
    pub fn y(&self) -> f32 {
        self.0[1]
    }
    pub fn z(&self) -> f32 {
        self.0[2]
    }
    pub fn w(&self) -> f32 {
        self.0[3]
    }

    pub fn dot(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[0] + self.0[1] * rhs.0[1] + self.0[2] * rhs.0[2] + self.0[3] * rhs.0[3]
    }

    pub fn magnitude(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(&self) -> Self {
        *self / self.magnitude()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl Index<usize> for Vector4 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl IndexMut<usize> for Vector4 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}

impl Add for Vector4 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
            self.0[3] + rhs.0[3],
        ])
    }
}
impl Sub for Vector4 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
            self.0[3] - rhs.0[3],
        ])
    }
}
impl Mul for Vector4 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
            self.0[3] * rhs.0[3],
        ])
    }
}
impl Div for Vector4 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self([
            self.0[0] / rhs.0[0],
            self.0[1] / rhs.0[1],
            self.0[2] / rhs.0[2],
            self.0[3] / rhs.0[3],
        ])
    }
}

impl Mul<f32> for Vector4 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s, self.0[2] * s, self.0[3] * s])
    }
}
impl Div<f32> for Vector4 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self([self.0[0] / s, self.0[1] / s, self.0[2] / s, self.0[3] / s])
    }
}

impl AddAssign for Vector4 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Vector4 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Vector4 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Vector4 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Vector4 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Vector4 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Vector4 {
    type Output = Self;
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2], -self.0[3]])
    }
}

impl fmt::Display for Vector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

// ---------------------------------------------------------------------------
// Color3
// ---------------------------------------------------------------------------

/// Three-component color (RGB), f32.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color3(pub [f32; 3]);

impl Color3 {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }
    pub fn splat(s: f32) -> Self {
        Self([s, s, s])
    }

    pub fn r(&self) -> f32 {
        self.0[0]
    }
    pub fn g(&self) -> f32 {
        self.0[1]
    }
    pub fn b(&self) -> f32 {
        self.0[2]
    }

    pub fn dot(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[0] + self.0[1] * rhs.0[1] + self.0[2] * rhs.0[2]
    }

    pub fn magnitude(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(&self) -> Self {
        *self / self.magnitude()
    }

    /// Linear RGB -> sRGB encoding (per-channel).
    pub fn linear_to_srgb(&self) -> Self {
        let enc = |v: f32| -> f32 {
            if v <= 0.003_130_8 {
                v * 12.92
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };
        Self([enc(self.0[0]), enc(self.0[1]), enc(self.0[2])])
    }

    /// sRGB encoding -> linear RGB (per-channel).
    pub fn srgb_to_linear(&self) -> Self {
        let dec = |v: f32| -> f32 {
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        Self([dec(self.0[0]), dec(self.0[1]), dec(self.0[2])])
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl Index<usize> for Color3 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl IndexMut<usize> for Color3 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}

impl Add for Color3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
        ])
    }
}
impl Sub for Color3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
        ])
    }
}
impl Mul for Color3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
        ])
    }
}
impl Div for Color3 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self([
            self.0[0] / rhs.0[0],
            self.0[1] / rhs.0[1],
            self.0[2] / rhs.0[2],
        ])
    }
}

impl Mul<f32> for Color3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s, self.0[2] * s])
    }
}
impl Div<f32> for Color3 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self([self.0[0] / s, self.0[1] / s, self.0[2] / s])
    }
}

impl AddAssign for Color3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Color3 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Color3 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Color3 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Color3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Color3 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Color3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2]])
    }
}

impl fmt::Display for Color3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.0[0], self.0[1], self.0[2])
    }
}

// ---------------------------------------------------------------------------
// Color4
// ---------------------------------------------------------------------------

/// Four-component color (RGBA), f32.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color4(pub [f32; 4]);

impl Color4 {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }
    pub fn splat(s: f32) -> Self {
        Self([s, s, s, s])
    }

    pub fn r(&self) -> f32 {
        self.0[0]
    }
    pub fn g(&self) -> f32 {
        self.0[1]
    }
    pub fn b(&self) -> f32 {
        self.0[2]
    }
    pub fn a(&self) -> f32 {
        self.0[3]
    }

    pub fn dot(&self, rhs: &Self) -> f32 {
        self.0[0] * rhs.0[0] + self.0[1] * rhs.0[1] + self.0[2] * rhs.0[2] + self.0[3] * rhs.0[3]
    }

    pub fn magnitude(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(&self) -> Self {
        *self / self.magnitude()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl Index<usize> for Color4 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl IndexMut<usize> for Color4 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}

impl Add for Color4 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
            self.0[3] + rhs.0[3],
        ])
    }
}
impl Sub for Color4 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
            self.0[3] - rhs.0[3],
        ])
    }
}
impl Mul for Color4 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
            self.0[3] * rhs.0[3],
        ])
    }
}
impl Div for Color4 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self([
            self.0[0] / rhs.0[0],
            self.0[1] / rhs.0[1],
            self.0[2] / rhs.0[2],
            self.0[3] / rhs.0[3],
        ])
    }
}

impl Mul<f32> for Color4 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s, self.0[2] * s, self.0[3] * s])
    }
}
impl Div<f32> for Color4 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        Self([self.0[0] / s, self.0[1] / s, self.0[2] / s, self.0[3] / s])
    }
}

impl AddAssign for Color4 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Color4 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Color4 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Color4 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Color4 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Color4 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Color4 {
    type Output = Self;
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2], -self.0[3]])
    }
}

impl fmt::Display for Color4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

// ---------------------------------------------------------------------------
// Matrix33  (row-major, [row][col])
// ---------------------------------------------------------------------------

/// 3x3 float matrix, row-major.  Row-vector convention: v' = v * M.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix33(pub [[f32; 3]; 3]);

impl Matrix33 {
    /// Identity matrix constant.
    pub const IDENTITY: Self = Self([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Construct from 9 scalars in row-major order.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m20: f32,
        m21: f32,
        m22: f32,
    ) -> Self {
        Self([[m00, m01, m02], [m10, m11, m12], [m20, m21, m22]])
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// Return transposed copy.
    pub fn transposed(&self) -> Self {
        let m = &self.0;
        Self([
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ])
    }

    /// Scalar determinant.
    pub fn determinant(&self) -> f32 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2])
            + m[0][1] * (m[1][2] * m[2][0] - m[2][2] * m[1][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[2][0] * m[1][1])
    }

    /// Classical adjugate (transpose of cofactor matrix).
    pub fn adjugate(&self) -> Self {
        let m = &self.0;
        Self::new(
            m[1][1] * m[2][2] - m[2][1] * m[1][2],
            m[2][1] * m[0][2] - m[0][1] * m[2][2],
            m[0][1] * m[1][2] - m[1][1] * m[0][2],
            m[1][2] * m[2][0] - m[2][2] * m[1][0],
            m[2][2] * m[0][0] - m[0][2] * m[2][0],
            m[0][2] * m[1][0] - m[1][2] * m[0][0],
            m[1][0] * m[2][1] - m[2][0] * m[1][1],
            m[2][0] * m[0][1] - m[0][0] * m[2][1],
            m[0][0] * m[1][1] - m[1][0] * m[0][1],
        )
    }

    /// Inverse via adjugate / determinant.
    pub fn inversed(&self) -> Self {
        self.adjugate() / self.determinant()
    }

    // --- vector transform (row-vector convention: v' = v * M) ---

    /// Full matrix-vector multiply: result[j] = sum_i v[i] * M[i][j].
    pub fn multiply(&self, v: &Vector3) -> Vector3 {
        let m = &self.0;
        Vector3::new(
            v[0] * m[0][0] + v[1] * m[1][0] + v[2] * m[2][0],
            v[0] * m[0][1] + v[1] * m[1][1] + v[2] * m[2][1],
            v[0] * m[0][2] + v[1] * m[1][2] + v[2] * m[2][2],
        )
    }

    /// Transform a 2-D point (w=1).
    pub fn transform_point(&self, v: &Vector2) -> Vector2 {
        let r = self.multiply(&Vector3::new(v[0], v[1], 1.0));
        Vector2::new(r[0], r[1])
    }

    /// Transform a 2-D direction vector (w=0).
    pub fn transform_vector(&self, v: &Vector2) -> Vector2 {
        let r = self.multiply(&Vector3::new(v[0], v[1], 0.0));
        Vector2::new(r[0], r[1])
    }

    /// Transform a 3-D normal (by inverse-transpose).
    pub fn transform_normal(&self, v: &Vector3) -> Vector3 {
        self.inversed().transposed().multiply(v)
    }

    // --- factory helpers ---

    /// Translation matrix for a 2-D point.
    pub fn create_translation(v: &Vector2) -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, v[0], v[1], 1.0)
    }

    /// Scale matrix for a 2-D scale vector.
    pub fn create_scale(v: &Vector2) -> Self {
        Self::new(v[0], 0.0, 0.0, 0.0, v[1], 0.0, 0.0, 0.0, 1.0)
    }

    /// Rotation matrix (radians).
    pub fn create_rotation(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0)
    }
}

impl Default for Matrix33 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

// index: [row]
impl Index<usize> for Matrix33 {
    type Output = [f32; 3];
    fn index(&self, i: usize) -> &[f32; 3] {
        &self.0[i]
    }
}
impl IndexMut<usize> for Matrix33 {
    fn index_mut(&mut self, i: usize) -> &mut [f32; 3] {
        &mut self.0[i]
    }
}

// component-wise +/-
impl Add for Matrix33 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        Self([
            [a[0][0] + b[0][0], a[0][1] + b[0][1], a[0][2] + b[0][2]],
            [a[1][0] + b[1][0], a[1][1] + b[1][1], a[1][2] + b[1][2]],
            [a[2][0] + b[2][0], a[2][1] + b[2][1], a[2][2] + b[2][2]],
        ])
    }
}
impl Sub for Matrix33 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        Self([
            [a[0][0] - b[0][0], a[0][1] - b[0][1], a[0][2] - b[0][2]],
            [a[1][0] - b[1][0], a[1][1] - b[1][1], a[1][2] - b[1][2]],
            [a[2][0] - b[2][0], a[2][1] - b[2][1], a[2][2] - b[2][2]],
        ])
    }
}

// scalar
impl Mul<f32> for Matrix33 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        let a = &self.0;
        Self([
            [a[0][0] * s, a[0][1] * s, a[0][2] * s],
            [a[1][0] * s, a[1][1] * s, a[1][2] * s],
            [a[2][0] * s, a[2][1] * s, a[2][2] * s],
        ])
    }
}
impl Div<f32> for Matrix33 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        self * (1.0 / s)
    }
}

// matrix multiply
impl Mul for Matrix33 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        let mut out = [[0.0f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    out[i][j] += a[i][k] * b[k][j];
                }
            }
        }
        Self(out)
    }
}

// matrix / matrix = self * rhs.inversed()
impl Div for Matrix33 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        self * rhs.inversed()
    }
}

impl AddAssign for Matrix33 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Matrix33 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Matrix33 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Matrix33 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Matrix33 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Matrix33 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Matrix33 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}

// ---------------------------------------------------------------------------
// Matrix44  (row-major, [row][col])
// ---------------------------------------------------------------------------

/// 4x4 float matrix, row-major.  Row-vector convention: v' = v * M.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix44(pub [[f32; 4]; 4]);

impl Matrix44 {
    /// Identity matrix constant.
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// 180-degree Y-rotation matrix: `createScale(Vector3(-1, 1, -1))`.
    /// Used as default environment rotation in hardware shader generation.
    pub const Y_ROTATION_PI: Self = Self([
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// Construct from 16 scalars in row-major order.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Self {
        Self([
            [m00, m01, m02, m03],
            [m10, m11, m12, m13],
            [m20, m21, m22, m23],
            [m30, m31, m32, m33],
        ])
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// Return transposed copy.
    pub fn transposed(&self) -> Self {
        let m = &self.0;
        Self([
            [m[0][0], m[1][0], m[2][0], m[3][0]],
            [m[0][1], m[1][1], m[2][1], m[3][1]],
            [m[0][2], m[1][2], m[2][2], m[3][2]],
            [m[0][3], m[1][3], m[2][3], m[3][3]],
        ])
    }

    /// Scalar determinant (Leibniz / cofactor expansion, matching C++).
    pub fn determinant(&self) -> f32 {
        let m = &self.0;
        m[0][0]
            * (m[1][1] * m[2][2] * m[3][3]
                + m[3][1] * m[1][2] * m[2][3]
                + m[2][1] * m[3][2] * m[1][3]
                - m[1][1] * m[3][2] * m[2][3]
                - m[2][1] * m[1][2] * m[3][3]
                - m[3][1] * m[2][2] * m[1][3])
            + m[0][1]
                * (m[1][2] * m[3][3] * m[2][0]
                    + m[2][2] * m[1][3] * m[3][0]
                    + m[3][2] * m[2][3] * m[1][0]
                    - m[1][2] * m[2][3] * m[3][0]
                    - m[3][2] * m[1][3] * m[2][0]
                    - m[2][2] * m[3][3] * m[1][0])
            + m[0][2]
                * (m[1][3] * m[2][0] * m[3][1]
                    + m[3][3] * m[1][0] * m[2][1]
                    + m[2][3] * m[3][0] * m[1][1]
                    - m[1][3] * m[3][0] * m[2][1]
                    - m[2][3] * m[1][0] * m[3][1]
                    - m[3][3] * m[2][0] * m[1][1])
            + m[0][3]
                * (m[1][0] * m[3][1] * m[2][2]
                    + m[2][0] * m[1][1] * m[3][2]
                    + m[3][0] * m[2][1] * m[1][2]
                    - m[1][0] * m[2][1] * m[3][2]
                    - m[3][0] * m[1][1] * m[2][2]
                    - m[2][0] * m[3][1] * m[1][2])
    }

    /// Classical adjugate.  Direct port of C++ Types.cpp.
    pub fn adjugate(&self) -> Self {
        let m = &self.0;
        Self::new(
            m[1][1] * m[2][2] * m[3][3] + m[3][1] * m[1][2] * m[2][3] + m[2][1] * m[3][2] * m[1][3]
                - m[1][1] * m[3][2] * m[2][3]
                - m[2][1] * m[1][2] * m[3][3]
                - m[3][1] * m[2][2] * m[1][3],
            m[0][1] * m[3][2] * m[2][3] + m[2][1] * m[0][2] * m[3][3] + m[3][1] * m[2][2] * m[0][3]
                - m[3][1] * m[0][2] * m[2][3]
                - m[2][1] * m[3][2] * m[0][3]
                - m[0][1] * m[2][2] * m[3][3],
            m[0][1] * m[1][2] * m[3][3] + m[3][1] * m[0][2] * m[1][3] + m[1][1] * m[3][2] * m[0][3]
                - m[0][1] * m[3][2] * m[1][3]
                - m[1][1] * m[0][2] * m[3][3]
                - m[3][1] * m[1][2] * m[0][3],
            m[0][1] * m[2][2] * m[1][3] + m[1][1] * m[0][2] * m[2][3] + m[2][1] * m[1][2] * m[0][3]
                - m[0][1] * m[1][2] * m[2][3]
                - m[2][1] * m[0][2] * m[1][3]
                - m[1][1] * m[2][2] * m[0][3],
            m[1][2] * m[3][3] * m[2][0] + m[2][2] * m[1][3] * m[3][0] + m[3][2] * m[2][3] * m[1][0]
                - m[1][2] * m[2][3] * m[3][0]
                - m[3][2] * m[1][3] * m[2][0]
                - m[2][2] * m[3][3] * m[1][0],
            m[0][2] * m[2][3] * m[3][0] + m[3][2] * m[0][3] * m[2][0] + m[2][2] * m[3][3] * m[0][0]
                - m[0][2] * m[3][3] * m[2][0]
                - m[2][2] * m[0][3] * m[3][0]
                - m[3][2] * m[2][3] * m[0][0],
            m[0][2] * m[3][3] * m[1][0] + m[1][2] * m[0][3] * m[3][0] + m[3][2] * m[1][3] * m[0][0]
                - m[0][2] * m[1][3] * m[3][0]
                - m[3][2] * m[0][3] * m[1][0]
                - m[1][2] * m[3][3] * m[0][0],
            m[0][2] * m[1][3] * m[2][0] + m[2][2] * m[0][3] * m[1][0] + m[1][2] * m[2][3] * m[0][0]
                - m[0][2] * m[2][3] * m[1][0]
                - m[1][2] * m[0][3] * m[2][0]
                - m[2][2] * m[1][3] * m[0][0],
            m[1][3] * m[2][0] * m[3][1] + m[3][3] * m[1][0] * m[2][1] + m[2][3] * m[3][0] * m[1][1]
                - m[1][3] * m[3][0] * m[2][1]
                - m[2][3] * m[1][0] * m[3][1]
                - m[3][3] * m[2][0] * m[1][1],
            m[0][3] * m[3][0] * m[2][1] + m[2][3] * m[0][0] * m[3][1] + m[3][3] * m[2][0] * m[0][1]
                - m[0][3] * m[2][0] * m[3][1]
                - m[3][3] * m[0][0] * m[2][1]
                - m[2][3] * m[3][0] * m[0][1],
            m[0][3] * m[1][0] * m[3][1] + m[3][3] * m[0][0] * m[1][1] + m[1][3] * m[3][0] * m[0][1]
                - m[0][3] * m[3][0] * m[1][1]
                - m[1][3] * m[0][0] * m[3][1]
                - m[3][3] * m[1][0] * m[0][1],
            m[0][3] * m[2][0] * m[1][1] + m[1][3] * m[0][0] * m[2][1] + m[2][3] * m[1][0] * m[0][1]
                - m[0][3] * m[1][0] * m[2][1]
                - m[2][3] * m[0][0] * m[1][1]
                - m[1][3] * m[2][0] * m[0][1],
            m[1][0] * m[3][1] * m[2][2] + m[2][0] * m[1][1] * m[3][2] + m[3][0] * m[2][1] * m[1][2]
                - m[1][0] * m[2][1] * m[3][2]
                - m[3][0] * m[1][1] * m[2][2]
                - m[2][0] * m[3][1] * m[1][2],
            m[0][0] * m[2][1] * m[3][2] + m[3][0] * m[0][1] * m[2][2] + m[2][0] * m[3][1] * m[0][2]
                - m[0][0] * m[3][1] * m[2][2]
                - m[2][0] * m[0][1] * m[3][2]
                - m[3][0] * m[2][1] * m[0][2],
            m[0][0] * m[3][1] * m[1][2] + m[1][0] * m[0][1] * m[3][2] + m[3][0] * m[1][1] * m[0][2]
                - m[0][0] * m[1][1] * m[3][2]
                - m[3][0] * m[0][1] * m[1][2]
                - m[1][0] * m[3][1] * m[0][2],
            m[0][0] * m[1][1] * m[2][2] + m[2][0] * m[0][1] * m[1][2] + m[1][0] * m[2][1] * m[0][2]
                - m[0][0] * m[2][1] * m[1][2]
                - m[1][0] * m[0][1] * m[2][2]
                - m[2][0] * m[1][1] * m[0][2],
        )
    }

    /// Inverse via adjugate / determinant.
    pub fn inversed(&self) -> Self {
        self.adjugate() / self.determinant()
    }

    // --- vector transforms (row-vector convention) ---

    /// Full matrix-vector multiply: result[j] = sum_i v[i] * M[i][j].
    pub fn multiply(&self, v: &Vector4) -> Vector4 {
        let m = &self.0;
        Vector4::new(
            v[0] * m[0][0] + v[1] * m[1][0] + v[2] * m[2][0] + v[3] * m[3][0],
            v[0] * m[0][1] + v[1] * m[1][1] + v[2] * m[2][1] + v[3] * m[3][1],
            v[0] * m[0][2] + v[1] * m[1][2] + v[2] * m[2][2] + v[3] * m[3][2],
            v[0] * m[0][3] + v[1] * m[1][3] + v[2] * m[2][3] + v[3] * m[3][3],
        )
    }

    /// Transform a 3-D point (w=1), drop w of result.
    pub fn transform_point(&self, v: &Vector3) -> Vector3 {
        let r = self.multiply(&Vector4::new(v[0], v[1], v[2], 1.0));
        Vector3::new(r[0], r[1], r[2])
    }

    /// Transform a 3-D direction vector (w=0).
    pub fn transform_vector(&self, v: &Vector3) -> Vector3 {
        let r = self.multiply(&Vector4::new(v[0], v[1], v[2], 0.0));
        Vector3::new(r[0], r[1], r[2])
    }

    /// Transform a 3-D normal (by inverse-transpose).
    pub fn transform_normal(&self, v: &Vector3) -> Vector3 {
        self.inversed().transposed().transform_vector(v)
    }

    // --- factory helpers ---

    pub fn create_translation(v: &Vector3) -> Self {
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, v[0], v[1], v[2], 1.0,
        )
    }

    pub fn create_scale(v: &Vector3) -> Self {
        Self::new(
            v[0], 0.0, 0.0, 0.0, 0.0, v[1], 0.0, 0.0, 0.0, 0.0, v[2], 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    pub fn create_rotation_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(
            1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    pub fn create_rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(
            c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    pub fn create_rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(
            c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }
}

impl Default for Matrix44 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

// index: [row]
impl Index<usize> for Matrix44 {
    type Output = [f32; 4];
    fn index(&self, i: usize) -> &[f32; 4] {
        &self.0[i]
    }
}
impl IndexMut<usize> for Matrix44 {
    fn index_mut(&mut self, i: usize) -> &mut [f32; 4] {
        &mut self.0[i]
    }
}

impl Add for Matrix44 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        let mut out = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = a[i][j] + b[i][j];
            }
        }
        Self(out)
    }
}
impl Sub for Matrix44 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        let mut out = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = a[i][j] - b[i][j];
            }
        }
        Self(out)
    }
}

impl Mul<f32> for Matrix44 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        let a = &self.0;
        let mut out = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = a[i][j] * s;
            }
        }
        Self(out)
    }
}
impl Div<f32> for Matrix44 {
    type Output = Self;
    fn div(self, s: f32) -> Self {
        self * (1.0 / s)
    }
}

// matrix multiply
impl Mul for Matrix44 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let (a, b) = (&self.0, &rhs.0);
        let mut out = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    out[i][j] += a[i][k] * b[k][j];
                }
            }
        }
        Self(out)
    }
}

impl Div for Matrix44 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        self * rhs.inversed()
    }
}

impl AddAssign for Matrix44 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for Matrix44 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for Matrix44 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for Matrix44 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}
impl MulAssign<f32> for Matrix44 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl DivAssign<f32> for Matrix44 {
    fn div_assign(&mut self, s: f32) {
        *self = *self / s;
    }
}

impl Neg for Matrix44 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPS
    }

    fn vec3_approx(a: Vector3, b: Vector3) -> bool {
        approx_eq(a[0], b[0]) && approx_eq(a[1], b[1]) && approx_eq(a[2], b[2])
    }

    fn mat33_approx(a: &Matrix33, b: &Matrix33) -> bool {
        (0..3).all(|i| (0..3).all(|j| approx_eq(a[i][j], b[i][j])))
    }

    fn mat44_approx(a: &Matrix44, b: &Matrix44) -> bool {
        (0..4).all(|i| (0..4).all(|j| approx_eq(a[i][j], b[i][j])))
    }

    // --- Vector2 ---

    #[test]
    fn vec2_ops() {
        let a = Vector2::new(1.0, 2.0);
        let b = Vector2::new(3.0, 4.0);
        assert_eq!(a + b, Vector2::new(4.0, 6.0));
        assert_eq!(a - b, Vector2::new(-2.0, -2.0));
        assert_eq!(a * b, Vector2::new(3.0, 8.0));
        assert_eq!(a * 2.0, Vector2::new(2.0, 4.0));
        assert_eq!(-a, Vector2::new(-1.0, -2.0));
    }

    #[test]
    fn vec2_geometric() {
        let a = Vector2::new(3.0, 4.0);
        assert!(approx_eq(a.magnitude(), 5.0));
        let n = a.normalized();
        assert!(approx_eq(n.magnitude(), 1.0));
        assert!(approx_eq(a.dot(&Vector2::new(1.0, 0.0)), 3.0));
        // cross is a scalar for 2-D
        let c = Vector2::new(1.0, 0.0).cross(&Vector2::new(0.0, 1.0));
        assert!(approx_eq(c, 1.0));
    }

    // --- Vector3 ---

    #[test]
    fn vec3_ops() {
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vector3::new(5.0, 7.0, 9.0));
        assert_eq!(b - a, Vector3::new(3.0, 3.0, 3.0));
        assert_eq!(a * 3.0, Vector3::new(3.0, 6.0, 9.0));
        assert_eq!(-a, Vector3::new(-1.0, -2.0, -3.0));
    }

    #[test]
    fn vec3_cross() {
        let x = Vector3::new(1.0, 0.0, 0.0);
        let y = Vector3::new(0.0, 1.0, 0.0);
        let z = x.cross(&y);
        assert!(vec3_approx(z, Vector3::new(0.0, 0.0, 1.0)));
    }

    #[test]
    fn vec3_dot_magnitude() {
        let a = Vector3::new(1.0, 2.0, 2.0);
        assert!(approx_eq(a.magnitude(), 3.0));
        assert!(approx_eq(a.dot(&Vector3::new(1.0, 0.0, 0.0)), 1.0));
    }

    // --- Vector4 ---

    #[test]
    fn vec4_ops() {
        let a = Vector4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(a * 2.0, Vector4::new(2.0, 4.0, 6.0, 8.0));
        assert_eq!(-a, Vector4::new(-1.0, -2.0, -3.0, -4.0));
        assert!(approx_eq(a.dot(&a), 30.0));
    }

    // --- Color3 sRGB ---

    #[test]
    fn color3_srgb_roundtrip() {
        let c = Color3::new(0.5, 0.2, 0.8);
        let encoded = c.linear_to_srgb();
        let decoded = encoded.srgb_to_linear();
        for i in 0..3 {
            assert!(approx_eq(c[i], decoded[i]));
        }
    }

    // --- Matrix33 ---

    #[test]
    fn mat33_identity() {
        let i = Matrix33::IDENTITY;
        assert!(i.is_identity());
        assert!(mat33_approx(&(i * i), &i));
    }

    #[test]
    fn mat33_transpose() {
        let m = Matrix33::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let t = m.transposed();
        assert!(approx_eq(t[0][1], m[1][0]));
        assert!(approx_eq(t[2][0], m[0][2]));
    }

    #[test]
    fn mat33_det_inverse() {
        let m = Matrix33::new(1.0, 2.0, 0.0, 0.0, 1.0, 3.0, 0.0, 0.0, 1.0);
        let det = m.determinant();
        assert!(approx_eq(det, 1.0));
        let inv = m.inversed();
        let prod = m * inv;
        assert!(mat33_approx(&prod, &Matrix33::IDENTITY));
    }

    #[test]
    fn mat33_multiply_vec() {
        let i = Matrix33::IDENTITY;
        let v = Vector3::new(1.0, 2.0, 3.0);
        assert!(vec3_approx(i.multiply(&v), v));
    }

    // --- Matrix44 ---

    #[test]
    fn mat44_identity() {
        let i = Matrix44::IDENTITY;
        assert!(i.is_identity());
        assert!(mat44_approx(&(i * i), &i));
    }

    #[test]
    fn mat44_transpose() {
        let m = Matrix44::new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let t = m.transposed();
        assert!(approx_eq(t[1][0], m[0][1]));
        assert!(approx_eq(t[3][0], m[0][3]));
    }

    #[test]
    fn mat44_det_inverse() {
        // Simple upper-triangular with unit diagonal => det=1, inv = itself "unrolled"
        let m = Matrix44::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        let det = m.determinant();
        assert!(approx_eq(det, 8.0));
        let inv = m.inversed();
        let prod = m * inv;
        assert!(mat44_approx(&prod, &Matrix44::IDENTITY));
    }

    #[test]
    fn mat44_transform_point() {
        let t = Matrix44::create_translation(&Vector3::new(1.0, 2.0, 3.0));
        let p = t.transform_point(&Vector3::new(0.0, 0.0, 0.0));
        assert!(vec3_approx(p, Vector3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn mat44_index_ops() {
        let mut m = Matrix44::IDENTITY;
        m[1][2] = 99.0;
        assert!(approx_eq(m[1][2], 99.0));
    }
}
