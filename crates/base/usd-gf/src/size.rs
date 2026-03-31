//! Size types for representing 2D and 3D extents.
//!
//! Size types use `usize` for non-negative dimension counts.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Size2, Size3};
//!
//! let size = Size2::new(100, 200);
//! assert_eq!(size.width(), 100);
//! assert_eq!(size.height(), 200);
//!
//! let volume = Size3::new(10, 20, 30);
//! assert_eq!(volume.depth(), 30);
//! ```

use crate::vec2::Vec2i;
use crate::vec3::Vec3i;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};

/// 2D size with unsigned dimensions (width, height).
///
/// Component-wise operations, including multiplication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Size2 {
    data: [usize; 2],
}

impl Size2 {
    /// Creates a new Size2.
    #[inline]
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: [width, height],
        }
    }

    /// Creates from an array.
    #[inline]
    #[must_use]
    pub fn from_array(arr: [usize; 2]) -> Self {
        Self { data: arr }
    }

    /// Returns the width (first component).
    #[inline]
    #[must_use]
    pub fn width(&self) -> usize {
        self.data[0]
    }

    /// Returns the height (second component).
    #[inline]
    #[must_use]
    pub fn height(&self) -> usize {
        self.data[1]
    }

    /// Sets the width.
    #[inline]
    pub fn set_width(&mut self, w: usize) {
        self.data[0] = w;
    }

    /// Sets the height.
    #[inline]
    pub fn set_height(&mut self, h: usize) {
        self.data[1] = h;
    }

    /// Sets both dimensions.
    #[inline]
    pub fn set(&mut self, width: usize, height: usize) {
        self.data[0] = width;
        self.data[1] = height;
    }

    /// Converts to Vec2i.
    #[must_use]
    pub fn to_vec2i(&self) -> Vec2i {
        Vec2i::new(self.data[0] as i32, self.data[1] as i32)
    }
}

impl From<Vec2i> for Size2 {
    fn from(v: Vec2i) -> Self {
        Self::new(v.x.max(0) as usize, v.y.max(0) as usize)
    }
}

impl From<Size2> for Vec2i {
    fn from(s: Size2) -> Self {
        s.to_vec2i()
    }
}

impl Index<usize> for Size2 {
    type Output = usize;
    fn index(&self, i: usize) -> &Self::Output {
        &self.data[i]
    }
}

impl IndexMut<usize> for Size2 {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.data[i]
    }
}

impl Add for Size2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.data[0] + rhs.data[0], self.data[1] + rhs.data[1])
    }
}

impl AddAssign for Size2 {
    fn add_assign(&mut self, rhs: Self) {
        self.data[0] += rhs.data[0];
        self.data[1] += rhs.data[1];
    }
}

impl Sub for Size2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.data[0].saturating_sub(rhs.data[0]),
            self.data[1].saturating_sub(rhs.data[1]),
        )
    }
}

impl SubAssign for Size2 {
    fn sub_assign(&mut self, rhs: Self) {
        self.data[0] = self.data[0].saturating_sub(rhs.data[0]);
        self.data[1] = self.data[1].saturating_sub(rhs.data[1]);
    }
}

impl Mul for Size2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.data[0] * rhs.data[0], self.data[1] * rhs.data[1])
    }
}

impl MulAssign for Size2 {
    fn mul_assign(&mut self, rhs: Self) {
        self.data[0] *= rhs.data[0];
        self.data[1] *= rhs.data[1];
    }
}

impl Mul<usize> for Size2 {
    type Output = Self;
    fn mul(self, rhs: usize) -> Self::Output {
        Self::new(self.data[0] * rhs, self.data[1] * rhs)
    }
}

impl MulAssign<usize> for Size2 {
    fn mul_assign(&mut self, rhs: usize) {
        self.data[0] *= rhs;
        self.data[1] *= rhs;
    }
}

impl Div<usize> for Size2 {
    type Output = Self;
    fn div(self, rhs: usize) -> Self::Output {
        Self::new(self.data[0] / rhs, self.data[1] / rhs)
    }
}

impl DivAssign<usize> for Size2 {
    fn div_assign(&mut self, rhs: usize) {
        self.data[0] /= rhs;
        self.data[1] /= rhs;
    }
}

impl fmt::Display for Size2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.data[0], self.data[1])
    }
}

/// 3D size with unsigned dimensions (width, height, depth).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Size3 {
    data: [usize; 3],
}

impl Size3 {
    /// Creates a new Size3.
    #[inline]
    #[must_use]
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self {
            data: [width, height, depth],
        }
    }

    /// Creates from an array.
    #[inline]
    #[must_use]
    pub fn from_array(arr: [usize; 3]) -> Self {
        Self { data: arr }
    }

    /// Returns the width (first component).
    #[inline]
    #[must_use]
    pub fn width(&self) -> usize {
        self.data[0]
    }

    /// Returns the height (second component).
    #[inline]
    #[must_use]
    pub fn height(&self) -> usize {
        self.data[1]
    }

    /// Returns the depth (third component).
    #[inline]
    #[must_use]
    pub fn depth(&self) -> usize {
        self.data[2]
    }

    /// Sets all dimensions.
    #[inline]
    pub fn set(&mut self, width: usize, height: usize, depth: usize) {
        self.data[0] = width;
        self.data[1] = height;
        self.data[2] = depth;
    }

    /// Converts to Vec3i.
    #[must_use]
    pub fn to_vec3i(&self) -> Vec3i {
        Vec3i::new(
            self.data[0] as i32,
            self.data[1] as i32,
            self.data[2] as i32,
        )
    }
}

impl From<Vec3i> for Size3 {
    fn from(v: Vec3i) -> Self {
        Self::new(
            v.x.max(0) as usize,
            v.y.max(0) as usize,
            v.z.max(0) as usize,
        )
    }
}

impl From<Size3> for Vec3i {
    fn from(s: Size3) -> Self {
        s.to_vec3i()
    }
}

impl Index<usize> for Size3 {
    type Output = usize;
    fn index(&self, i: usize) -> &Self::Output {
        &self.data[i]
    }
}

impl IndexMut<usize> for Size3 {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.data[i]
    }
}

impl Add for Size3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.data[0] + rhs.data[0],
            self.data[1] + rhs.data[1],
            self.data[2] + rhs.data[2],
        )
    }
}

impl AddAssign for Size3 {
    fn add_assign(&mut self, rhs: Self) {
        self.data[0] += rhs.data[0];
        self.data[1] += rhs.data[1];
        self.data[2] += rhs.data[2];
    }
}

impl Sub for Size3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.data[0].saturating_sub(rhs.data[0]),
            self.data[1].saturating_sub(rhs.data[1]),
            self.data[2].saturating_sub(rhs.data[2]),
        )
    }
}

impl SubAssign for Size3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.data[0] = self.data[0].saturating_sub(rhs.data[0]);
        self.data[1] = self.data[1].saturating_sub(rhs.data[1]);
        self.data[2] = self.data[2].saturating_sub(rhs.data[2]);
    }
}

impl Mul for Size3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.data[0] * rhs.data[0],
            self.data[1] * rhs.data[1],
            self.data[2] * rhs.data[2],
        )
    }
}

impl MulAssign for Size3 {
    fn mul_assign(&mut self, rhs: Self) {
        self.data[0] *= rhs.data[0];
        self.data[1] *= rhs.data[1];
        self.data[2] *= rhs.data[2];
    }
}

impl Mul<usize> for Size3 {
    type Output = Self;
    fn mul(self, rhs: usize) -> Self::Output {
        Self::new(self.data[0] * rhs, self.data[1] * rhs, self.data[2] * rhs)
    }
}

impl MulAssign<usize> for Size3 {
    fn mul_assign(&mut self, rhs: usize) {
        self.data[0] *= rhs;
        self.data[1] *= rhs;
        self.data[2] *= rhs;
    }
}

impl Div<usize> for Size3 {
    type Output = Self;
    fn div(self, rhs: usize) -> Self::Output {
        Self::new(self.data[0] / rhs, self.data[1] / rhs, self.data[2] / rhs)
    }
}

impl DivAssign<usize> for Size3 {
    fn div_assign(&mut self, rhs: usize) {
        self.data[0] /= rhs;
        self.data[1] /= rhs;
        self.data[2] /= rhs;
    }
}

impl fmt::Display for Size3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.data[0], self.data[1], self.data[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size2_new() {
        let s = Size2::new(100, 200);
        assert_eq!(s.width(), 100);
        assert_eq!(s.height(), 200);
    }

    #[test]
    fn test_size2_default() {
        let s: Size2 = Default::default();
        assert_eq!(s.width(), 0);
        assert_eq!(s.height(), 0);
    }

    #[test]
    fn test_size2_index() {
        let s = Size2::new(10, 20);
        assert_eq!(s[0], 10);
        assert_eq!(s[1], 20);
    }

    #[test]
    fn test_size2_add() {
        let a = Size2::new(10, 20);
        let b = Size2::new(5, 5);
        let c = a + b;
        assert_eq!(c.width(), 15);
        assert_eq!(c.height(), 25);
    }

    #[test]
    fn test_size2_sub() {
        let a = Size2::new(10, 20);
        let b = Size2::new(5, 5);
        let c = a - b;
        assert_eq!(c.width(), 5);
        assert_eq!(c.height(), 15);
    }

    #[test]
    fn test_size2_mul() {
        let a = Size2::new(10, 20);
        let b = Size2::new(2, 3);
        let c = a * b;
        assert_eq!(c.width(), 20);
        assert_eq!(c.height(), 60);
    }

    #[test]
    fn test_size2_mul_scalar() {
        let a = Size2::new(10, 20);
        let c = a * 3;
        assert_eq!(c.width(), 30);
        assert_eq!(c.height(), 60);
    }

    #[test]
    fn test_size2_div_scalar() {
        let a = Size2::new(30, 60);
        let c = a / 3;
        assert_eq!(c.width(), 10);
        assert_eq!(c.height(), 20);
    }

    #[test]
    fn test_size2_to_vec2i() {
        let s = Size2::new(100, 200);
        let v = s.to_vec2i();
        assert_eq!(v.x, 100);
        assert_eq!(v.y, 200);
    }

    #[test]
    fn test_size2_display() {
        let s = Size2::new(100, 200);
        assert_eq!(format!("{}", s), "(100, 200)");
    }

    #[test]
    fn test_size3_new() {
        let s = Size3::new(10, 20, 30);
        assert_eq!(s.width(), 10);
        assert_eq!(s.height(), 20);
        assert_eq!(s.depth(), 30);
    }

    #[test]
    fn test_size3_add() {
        let a = Size3::new(10, 20, 30);
        let b = Size3::new(1, 2, 3);
        let c = a + b;
        assert_eq!(c.width(), 11);
        assert_eq!(c.height(), 22);
        assert_eq!(c.depth(), 33);
    }

    #[test]
    fn test_size3_display() {
        let s = Size3::new(10, 20, 30);
        assert_eq!(format!("{}", s), "(10, 20, 30)");
    }
}
