//! Integer rectangle type.
//!
//! Rect2i represents a 2D rectangle with integer coordinates.
//! Width and height are inclusive: width = max_x - min_x + 1.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Rect2i, Vec2i};
//!
//! let rect = Rect2i::from_min_size(Vec2i::new(0, 0), 100, 50);
//! assert_eq!(rect.width(), 100);
//! assert_eq!(rect.height(), 50);
//! assert!(rect.contains(&Vec2i::new(50, 25)));
//! ```

use crate::vec2::Vec2i;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign};

/// A 2D rectangle with integer coordinates.
///
/// Min and max corners are inclusive. Width = max_x - min_x + 1.
/// An empty rectangle has width or height <= 0.
///
/// # Examples
///
/// ```
/// use usd_gf::{Rect2i, Vec2i};
///
/// let rect = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(9, 9));
/// assert_eq!(rect.width(), 10);
/// assert_eq!(rect.height(), 10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Rect2i {
    min: Vec2i,
    max: Vec2i,
}

impl Rect2i {
    /// Creates a rectangle from min and max corners.
    #[inline]
    #[must_use]
    pub fn new(min: Vec2i, max: Vec2i) -> Self {
        Self { min, max }
    }

    /// Creates a rectangle from min corner and size (width, height).
    ///
    /// The max corner is computed as: max = min + (width-1, height-1).
    #[inline]
    #[must_use]
    pub fn from_min_size(min: Vec2i, width: i32, height: i32) -> Self {
        Self {
            min,
            max: Vec2i::new(min.x + width - 1, min.y + height - 1),
        }
    }

    /// Creates an empty rectangle.
    ///
    /// Empty rectangles have max < min.
    #[inline]
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min: Vec2i::new(0, 0),
            max: Vec2i::new(-1, -1),
        }
    }

    /// Returns the min corner.
    #[inline]
    #[must_use]
    pub fn min(&self) -> &Vec2i {
        &self.min
    }

    /// Returns the max corner.
    #[inline]
    #[must_use]
    pub fn max(&self) -> &Vec2i {
        &self.max
    }

    /// Returns the X of min corner.
    #[inline]
    #[must_use]
    pub fn min_x(&self) -> i32 {
        self.min.x
    }

    /// Returns the Y of min corner.
    #[inline]
    #[must_use]
    pub fn min_y(&self) -> i32 {
        self.min.y
    }

    /// Returns the X of max corner.
    #[inline]
    #[must_use]
    pub fn max_x(&self) -> i32 {
        self.max.x
    }

    /// Returns the Y of max corner.
    #[inline]
    #[must_use]
    pub fn max_y(&self) -> i32 {
        self.max.y
    }

    /// Sets the min corner.
    #[inline]
    pub fn set_min(&mut self, min: Vec2i) {
        self.min = min;
    }

    /// Sets the max corner.
    #[inline]
    pub fn set_max(&mut self, max: Vec2i) {
        self.max = max;
    }

    /// Sets the min X.
    #[inline]
    pub fn set_min_x(&mut self, x: i32) {
        self.min.x = x;
    }

    /// Sets the min Y.
    #[inline]
    pub fn set_min_y(&mut self, y: i32) {
        self.min.y = y;
    }

    /// Sets the max X.
    #[inline]
    pub fn set_max_x(&mut self, x: i32) {
        self.max.x = x;
    }

    /// Sets the max Y.
    #[inline]
    pub fn set_max_y(&mut self, y: i32) {
        self.max.y = y;
    }

    /// Returns the width (max_x - min_x + 1).
    ///
    /// Returns 0 or negative for empty rectangles.
    #[inline]
    #[must_use]
    pub fn width(&self) -> i32 {
        self.max.x - self.min.x + 1
    }

    /// Returns the height (max_y - min_y + 1).
    ///
    /// Returns 0 or negative for empty rectangles.
    #[inline]
    #[must_use]
    pub fn height(&self) -> i32 {
        self.max.y - self.min.y + 1
    }

    /// Returns the size as (width, height).
    #[inline]
    #[must_use]
    pub fn size(&self) -> Vec2i {
        Vec2i::new(self.width(), self.height())
    }

    /// Returns the center point.
    #[inline]
    #[must_use]
    pub fn center(&self) -> Vec2i {
        Vec2i::new((self.min.x + self.max.x) / 2, (self.min.y + self.max.y) / 2)
    }

    /// Returns the area (width * height).
    ///
    /// Returns 0 for empty rectangles.
    #[inline]
    #[must_use]
    pub fn area(&self) -> u64 {
        let w = self.width();
        let h = self.height();
        if w <= 0 || h <= 0 {
            0
        } else {
            w as u64 * h as u64
        }
    }

    /// Returns true if the rectangle is null (both width and height are 0).
    #[inline]
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.width() == 0 && self.height() == 0
    }

    /// Returns true if the rectangle is empty (width <= 0 or height <= 0).
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }

    /// Returns true if the rectangle is valid (not empty).
    #[inline]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.is_empty()
    }

    /// Returns a normalized rectangle (non-negative width/height).
    ///
    /// Swaps min/max if needed.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let min_x = self.min.x.min(self.max.x);
        let min_y = self.min.y.min(self.max.y);
        let max_x = self.min.x.max(self.max.x);
        let max_y = self.min.y.max(self.max.y);
        Self {
            min: Vec2i::new(min_x, min_y),
            max: Vec2i::new(max_x, max_y),
        }
    }

    /// Translates the rectangle by the given displacement.
    pub fn translate(&mut self, displacement: &Vec2i) {
        self.min.x += displacement.x;
        self.min.y += displacement.y;
        self.max.x += displacement.x;
        self.max.y += displacement.y;
    }

    /// Returns true if the rectangle contains the point.
    #[inline]
    #[must_use]
    pub fn contains(&self, point: &Vec2i) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Returns the intersection of two rectangles.
    ///
    /// If either is empty, returns that empty rectangle.
    #[must_use]
    pub fn get_intersection(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *self;
        }
        if other.is_empty() {
            return *other;
        }

        Self {
            min: Vec2i::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            max: Vec2i::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        }
    }

    /// Returns the union of two rectangles.
    ///
    /// If one is empty, returns the other.
    #[must_use]
    pub fn get_union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }

        Self {
            min: Vec2i::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Vec2i::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }
}

impl Default for Rect2i {
    fn default() -> Self {
        Self::empty()
    }
}

impl PartialEq for Rect2i {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

impl Eq for Rect2i {}

impl Hash for Rect2i {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.min.x.hash(state);
        self.min.y.hash(state);
        self.max.x.hash(state);
        self.max.y.hash(state);
    }
}

impl Add for Rect2i {
    type Output = Self;
    /// Union of two rectangles.
    fn add(self, rhs: Self) -> Self::Output {
        self.get_union(&rhs)
    }
}

impl AddAssign for Rect2i {
    /// Union with another rectangle.
    fn add_assign(&mut self, rhs: Self) {
        *self = self.get_union(&rhs);
    }
}

impl fmt::Display for Rect2i {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}):({}, {})]",
            self.min.x, self.min.y, self.max.x, self.max.y
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(9, 9));
        assert_eq!(r.width(), 10);
        assert_eq!(r.height(), 10);
    }

    #[test]
    fn test_from_min_size() {
        let r = Rect2i::from_min_size(Vec2i::new(5, 10), 20, 30);
        assert_eq!(r.min_x(), 5);
        assert_eq!(r.min_y(), 10);
        assert_eq!(r.width(), 20);
        assert_eq!(r.height(), 30);
        assert_eq!(r.max_x(), 24);
        assert_eq!(r.max_y(), 39);
    }

    #[test]
    fn test_empty() {
        let r = Rect2i::empty();
        assert!(r.is_empty());
        assert!(r.is_null());
        assert_eq!(r.area(), 0);
    }

    #[test]
    fn test_area() {
        let r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(9, 9));
        assert_eq!(r.area(), 100);
    }

    #[test]
    fn test_center() {
        let r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        let c = r.center();
        assert_eq!(c.x, 5);
        assert_eq!(c.y, 5);
    }

    #[test]
    fn test_contains() {
        let r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        assert!(r.contains(&Vec2i::new(5, 5)));
        assert!(r.contains(&Vec2i::new(0, 0)));
        assert!(r.contains(&Vec2i::new(10, 10)));
        assert!(!r.contains(&Vec2i::new(-1, 5)));
        assert!(!r.contains(&Vec2i::new(11, 5)));
    }

    #[test]
    fn test_intersection() {
        let r1 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        let r2 = Rect2i::new(Vec2i::new(5, 5), Vec2i::new(15, 15));
        let i = r1.get_intersection(&r2);

        assert_eq!(i.min_x(), 5);
        assert_eq!(i.min_y(), 5);
        assert_eq!(i.max_x(), 10);
        assert_eq!(i.max_y(), 10);
    }

    #[test]
    fn test_union() {
        let r1 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(5, 5));
        let r2 = Rect2i::new(Vec2i::new(10, 10), Vec2i::new(15, 15));
        let u = r1.get_union(&r2);

        assert_eq!(u.min_x(), 0);
        assert_eq!(u.min_y(), 0);
        assert_eq!(u.max_x(), 15);
        assert_eq!(u.max_y(), 15);
    }

    #[test]
    fn test_translate() {
        let mut r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        r.translate(&Vec2i::new(5, 5));

        assert_eq!(r.min_x(), 5);
        assert_eq!(r.min_y(), 5);
        assert_eq!(r.max_x(), 15);
        assert_eq!(r.max_y(), 15);
    }

    #[test]
    fn test_normalized() {
        let r = Rect2i::new(Vec2i::new(10, 10), Vec2i::new(0, 0));
        let n = r.normalized();

        assert_eq!(n.min_x(), 0);
        assert_eq!(n.max_x(), 10);
    }

    #[test]
    fn test_add() {
        let r1 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(5, 5));
        let r2 = Rect2i::new(Vec2i::new(10, 10), Vec2i::new(15, 15));
        let u = r1 + r2;

        assert_eq!(u.min_x(), 0);
        assert_eq!(u.max_x(), 15);
    }

    #[test]
    fn test_display() {
        let r = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        let s = format!("{}", r);
        assert!(s.contains("0") && s.contains("10"));
    }

    #[test]
    fn test_equality() {
        let r1 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        let r2 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(10, 10));
        let r3 = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(5, 5));

        assert!(r1 == r2);
        assert!(r1 != r3);
    }
}
