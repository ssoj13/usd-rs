//! Range types for representing axis-aligned intervals and bounding boxes.
//!
//! This module provides range types for 1D, 2D, and 3D:
//!
//! - `Range1d`, `Range1f` - 1D intervals
//! - `Range2d`, `Range2f` - 2D axis-aligned rectangles
//! - `Range3d`, `Range3f` - 3D axis-aligned bounding boxes (AABB)
//!
//! All operations are component-wise and follow interval mathematics.
//! An empty range is one where max < min (for any component).
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Range3d, Vec3d};
//!
//! // Create a bounding box from corner points
//! let bbox = Range3d::new(
//!     Vec3d::new(0.0, 0.0, 0.0),
//!     Vec3d::new(1.0, 1.0, 1.0)
//! );
//!
//! // Check if a point is inside
//! let p = Vec3d::new(0.5, 0.5, 0.5);
//! assert!(bbox.contains_point(&p));
//!
//! // Get the center
//! let center = bbox.midpoint();
//! assert!((center.x - 0.5).abs() < 1e-10);
//! ```

use crate::traits::Scalar;
use crate::vec2::Vec2;
use crate::vec3::Vec3;
use num_traits::Float;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

// ============================================================================
// Range1 - 1D interval
// ============================================================================

/// 1-dimensional range (interval).
///
/// Represents an interval [min, max] on the real line.
/// An empty interval has max < min.
///
/// # Examples
///
/// ```
/// use usd_gf::Range1d;
///
/// let r = Range1d::new(0.0, 10.0);
/// assert!(!r.is_empty());
/// assert!(r.contains(5.0));
/// assert!(!r.contains(-1.0));
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Range1<T> {
    min: T,
    max: T,
}

/// Double-precision 1D range.
pub type Range1d = Range1<f64>;

/// Single-precision 1D range.
pub type Range1f = Range1<f32>;

impl<T: Scalar + Float> Range1<T> {
    /// Creates a new range with given min and max values.
    #[inline]
    #[must_use]
    pub fn new(min: T, max: T) -> Self {
        Self { min, max }
    }

    /// Creates an empty range (max < min).
    #[inline]
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min: T::max_value(),
            max: T::min_value(),
        }
    }

    /// Returns the minimum value.
    #[inline]
    #[must_use]
    pub fn min(&self) -> T {
        self.min
    }

    /// Returns the maximum value.
    #[inline]
    #[must_use]
    pub fn max(&self) -> T {
        self.max
    }

    /// Sets the minimum value.
    #[inline]
    pub fn set_min(&mut self, min: T) {
        self.min = min;
    }

    /// Sets the maximum value.
    #[inline]
    pub fn set_max(&mut self, max: T) {
        self.max = max;
    }

    /// Sets the range to empty.
    #[inline]
    pub fn set_empty(&mut self) {
        self.min = T::max_value();
        self.max = T::min_value();
    }

    /// Returns the size (length) of the range.
    #[inline]
    #[must_use]
    pub fn size(&self) -> T {
        self.max - self.min
    }

    /// Returns the midpoint of the range.
    ///
    /// Returns zero for empty ranges.
    #[inline]
    #[must_use]
    pub fn midpoint(&self) -> T {
        let half = T::ONE / (T::ONE + T::ONE);
        half * self.min + half * self.max
    }

    /// Returns true if the range is empty (max < min).
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min > self.max
    }

    /// Returns true if the point is inside the range (inclusive).
    #[inline]
    #[must_use]
    pub fn contains(&self, point: T) -> bool {
        point >= self.min && point <= self.max
    }

    /// Returns true if the other range is entirely inside this range.
    #[inline]
    #[must_use]
    pub fn contains_range(&self, other: &Self) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Returns true if the ranges don't overlap.
    #[inline]
    #[must_use]
    pub fn is_outside(&self, other: &Self) -> bool {
        other.max < self.min || other.min > self.max
    }

    /// Extends this range to include the given point.
    pub fn union_with_point(&mut self, point: T) {
        if point < self.min {
            self.min = point;
        }
        if point > self.max {
            self.max = point;
        }
    }

    /// Extends this range to include the given range.
    pub fn union_with(&mut self, other: &Self) {
        if other.min < self.min {
            self.min = other.min;
        }
        if other.max > self.max {
            self.max = other.max;
        }
    }

    /// Returns the union of two ranges.
    #[must_use]
    pub fn get_union(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.union_with(b);
        result
    }

    /// Intersects this range with the given range.
    pub fn intersect_with(&mut self, other: &Self) {
        if other.min > self.min {
            self.min = other.min;
        }
        if other.max < self.max {
            self.max = other.max;
        }
    }

    /// Returns the intersection of two ranges.
    #[must_use]
    pub fn get_intersection(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.intersect_with(b);
        result
    }

    /// Returns the squared distance from a point to the range.
    ///
    /// Returns 0 if the point is inside the range.
    #[must_use]
    pub fn distance_squared(&self, point: T) -> T {
        if point < self.min {
            let d = self.min - point;
            d * d
        } else if point > self.max {
            let d = point - self.max;
            d * d
        } else {
            T::ZERO
        }
    }
}

impl<T: Scalar + Float> Default for Range1<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T: Scalar + Float + PartialEq> PartialEq for Range1<T> {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Range1f> for Range1d {
    fn from(other: Range1f) -> Self {
        Self::new(other.min() as f64, other.max() as f64)
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Range1f> for Range1d {
    fn eq(&self, other: &Range1f) -> bool {
        (self.min - other.min() as f64).abs() < f64::EPSILON
            && (self.max - other.max() as f64).abs() < f64::EPSILON
    }
}

impl<T: Scalar + Float + Hash> Hash for Range1<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.min.hash(state);
        self.max.hash(state);
    }
}

impl<T: Scalar + Float> Add for Range1<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min + rhs.min,
            max: self.max + rhs.max,
        }
    }
}

impl<T: Scalar + Float> AddAssign for Range1<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.min = self.min + rhs.min;
        self.max = self.max + rhs.max;
    }
}

impl<T: Scalar + Float> Sub for Range1<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min - rhs.max,
            max: self.max - rhs.min,
        }
    }
}

impl<T: Scalar + Float> SubAssign for Range1<T> {
    fn sub_assign(&mut self, rhs: Self) {
        let new_min = self.min - rhs.max;
        let new_max = self.max - rhs.min;
        self.min = new_min;
        self.max = new_max;
    }
}

impl<T: Scalar + Float> Mul<T> for Range1<T> {
    type Output = Self;
    fn mul(self, m: T) -> Self::Output {
        if m > T::ZERO {
            Self {
                min: self.min * m,
                max: self.max * m,
            }
        } else {
            Self {
                min: self.max * m,
                max: self.min * m,
            }
        }
    }
}

impl<T: Scalar + Float> MulAssign<T> for Range1<T> {
    fn mul_assign(&mut self, m: T) {
        if m > T::ZERO {
            self.min = self.min * m;
            self.max = self.max * m;
        } else {
            let tmp = self.min;
            self.min = self.max * m;
            self.max = tmp * m;
        }
    }
}

impl<T: Scalar + Float> Div<T> for Range1<T> {
    type Output = Self;
    fn div(self, m: T) -> Self::Output {
        self * (T::ONE / m)
    }
}

impl<T: Scalar + Float> DivAssign<T> for Range1<T> {
    fn div_assign(&mut self, m: T) {
        *self *= T::ONE / m;
    }
}

impl<T: Scalar + Float + fmt::Display> fmt::Display for Range1<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.min, self.max)
    }
}

// ============================================================================
// Range2 - 2D axis-aligned rectangle
// ============================================================================

/// 2-dimensional range (axis-aligned rectangle).
///
/// An empty range has max < min for any component.
///
/// # Examples
///
/// ```
/// use usd_gf::{Range2d, Vec2d};
///
/// let r = Range2d::new(
///     Vec2d::new(0.0, 0.0),
///     Vec2d::new(10.0, 10.0)
/// );
/// assert!(!r.is_empty());
/// assert!(r.contains_point(&Vec2d::new(5.0, 5.0)));
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Range2<T> {
    min: Vec2<T>,
    max: Vec2<T>,
}

/// Double-precision 2D range.
pub type Range2d = Range2<f64>;

/// Single-precision 2D range.
pub type Range2f = Range2<f32>;

impl<T: Scalar + Float> Range2<T> {
    /// Creates a new range with given min and max corners.
    #[inline]
    #[must_use]
    pub fn new(min: Vec2<T>, max: Vec2<T>) -> Self {
        Self { min, max }
    }

    /// Creates an empty range.
    #[inline]
    #[must_use]
    pub fn empty() -> Self {
        let big = T::max_value();
        let small = T::min_value();
        Self {
            min: Vec2 { x: big, y: big },
            max: Vec2 { x: small, y: small },
        }
    }

    /// The unit square [0,1] x [0,1].
    #[must_use]
    pub fn unit_square() -> Self {
        Self {
            min: Vec2 {
                x: T::ZERO,
                y: T::ZERO,
            },
            max: Vec2 {
                x: T::ONE,
                y: T::ONE,
            },
        }
    }

    /// Returns the minimum corner.
    #[inline]
    #[must_use]
    pub fn min(&self) -> &Vec2<T> {
        &self.min
    }

    /// Returns the maximum corner.
    #[inline]
    #[must_use]
    pub fn max(&self) -> &Vec2<T> {
        &self.max
    }

    /// Sets the minimum corner.
    #[inline]
    pub fn set_min(&mut self, min: Vec2<T>) {
        self.min = min;
    }

    /// Sets the maximum corner.
    #[inline]
    pub fn set_max(&mut self, max: Vec2<T>) {
        self.max = max;
    }

    /// Sets the range to empty.
    #[inline]
    pub fn set_empty(&mut self) {
        let big = T::max_value();
        let small = T::min_value();
        self.min = Vec2 { x: big, y: big };
        self.max = Vec2 { x: small, y: small };
    }

    /// Returns the size (width, height).
    #[inline]
    #[must_use]
    pub fn size(&self) -> Vec2<T> {
        Vec2 {
            x: self.max.x - self.min.x,
            y: self.max.y - self.min.y,
        }
    }

    /// Returns the center point.
    #[inline]
    #[must_use]
    pub fn midpoint(&self) -> Vec2<T> {
        let half = T::ONE / (T::ONE + T::ONE);
        Vec2 {
            x: half * self.min.x + half * self.max.x,
            y: half * self.min.y + half * self.max.y,
        }
    }

    /// Returns true if the range is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }

    /// Returns true if the point is inside the range.
    #[inline]
    #[must_use]
    pub fn contains_point(&self, point: &Vec2<T>) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Returns true if the other range is entirely inside this range.
    #[inline]
    #[must_use]
    pub fn contains_range(&self, other: &Self) -> bool {
        self.contains_point(&other.min) && self.contains_point(&other.max)
    }

    /// Returns true if the ranges don't overlap.
    #[inline]
    #[must_use]
    pub fn is_outside(&self, other: &Self) -> bool {
        other.max.x < self.min.x
            || other.min.x > self.max.x
            || other.max.y < self.min.y
            || other.min.y > self.max.y
    }

    /// Extends this range to include the given point.
    pub fn union_with_point(&mut self, point: &Vec2<T>) {
        if point.x < self.min.x {
            self.min.x = point.x;
        }
        if point.y < self.min.y {
            self.min.y = point.y;
        }
        if point.x > self.max.x {
            self.max.x = point.x;
        }
        if point.y > self.max.y {
            self.max.y = point.y;
        }
    }

    /// Extends this range to include the given range.
    pub fn union_with(&mut self, other: &Self) {
        if other.min.x < self.min.x {
            self.min.x = other.min.x;
        }
        if other.min.y < self.min.y {
            self.min.y = other.min.y;
        }
        if other.max.x > self.max.x {
            self.max.x = other.max.x;
        }
        if other.max.y > self.max.y {
            self.max.y = other.max.y;
        }
    }

    /// Returns the union of two ranges.
    #[must_use]
    pub fn get_union(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.union_with(b);
        result
    }

    /// Intersects this range with the given range.
    pub fn intersect_with(&mut self, other: &Self) {
        if other.min.x > self.min.x {
            self.min.x = other.min.x;
        }
        if other.min.y > self.min.y {
            self.min.y = other.min.y;
        }
        if other.max.x < self.max.x {
            self.max.x = other.max.x;
        }
        if other.max.y < self.max.y {
            self.max.y = other.max.y;
        }
    }

    /// Returns the intersection of two ranges.
    #[must_use]
    pub fn get_intersection(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.intersect_with(b);
        result
    }

    /// Returns the squared distance from a point to the range.
    #[must_use]
    pub fn distance_squared(&self, point: &Vec2<T>) -> T {
        let mut dist_sq = T::ZERO;

        if point.x < self.min.x {
            let d = self.min.x - point.x;
            dist_sq += d * d;
        } else if point.x > self.max.x {
            let d = point.x - self.max.x;
            dist_sq += d * d;
        }

        if point.y < self.min.y {
            let d = self.min.y - point.y;
            dist_sq += d * d;
        } else if point.y > self.max.y {
            let d = point.y - self.max.y;
            dist_sq += d * d;
        }

        dist_sq
    }

    /// Returns the ith corner: 0=SW, 1=SE, 2=NW, 3=NE.
    #[must_use]
    pub fn corner(&self, i: usize) -> Vec2<T> {
        Vec2 {
            x: if i & 1 == 0 { self.min.x } else { self.max.x },
            y: if i & 2 == 0 { self.min.y } else { self.max.y },
        }
    }

    /// Returns the ith quadrant: 0=SW, 1=SE, 2=NW, 3=NE.
    #[must_use]
    pub fn quadrant(&self, i: usize) -> Self {
        let mid = self.midpoint();
        let min_x = if i & 1 == 0 { self.min.x } else { mid.x };
        let max_x = if i & 1 == 0 { mid.x } else { self.max.x };
        let min_y = if i & 2 == 0 { self.min.y } else { mid.y };
        let max_y = if i & 2 == 0 { mid.y } else { self.max.y };

        Self {
            min: Vec2 { x: min_x, y: min_y },
            max: Vec2 { x: max_x, y: max_y },
        }
    }
}

impl<T: Scalar + Float> Default for Range2<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T: Scalar + Float + PartialEq> PartialEq for Range2<T> {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Range2f> for Range2d {
    fn from(other: Range2f) -> Self {
        use crate::vec2::Vec2d;
        Self::new(
            Vec2d::new(other.min().x as f64, other.min().y as f64),
            Vec2d::new(other.max().x as f64, other.max().y as f64),
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Range2f> for Range2d {
    fn eq(&self, other: &Range2f) -> bool {
        (self.min.x - other.min().x as f64).abs() < f64::EPSILON
            && (self.min.y - other.min().y as f64).abs() < f64::EPSILON
            && (self.max.x - other.max().x as f64).abs() < f64::EPSILON
            && (self.max.y - other.max().y as f64).abs() < f64::EPSILON
    }
}

impl<T: Scalar + Float> Add for Range2<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min + rhs.min,
            max: self.max + rhs.max,
        }
    }
}

impl<T: Scalar + Float> AddAssign for Range2<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.min = self.min + rhs.min;
        self.max = self.max + rhs.max;
    }
}

impl<T: Scalar + Float> Sub for Range2<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min - rhs.max,
            max: self.max - rhs.min,
        }
    }
}

impl<T: Scalar + Float> SubAssign for Range2<T> {
    fn sub_assign(&mut self, rhs: Self) {
        let new_min = self.min - rhs.max;
        let new_max = self.max - rhs.min;
        self.min = new_min;
        self.max = new_max;
    }
}

impl<T: Scalar + Float> Mul<T> for Range2<T> {
    type Output = Self;
    fn mul(self, m: T) -> Self::Output {
        if m > T::ZERO {
            Self {
                min: self.min * m,
                max: self.max * m,
            }
        } else {
            Self {
                min: self.max * m,
                max: self.min * m,
            }
        }
    }
}

impl<T: Scalar + Float> MulAssign<T> for Range2<T> {
    fn mul_assign(&mut self, m: T) {
        if m > T::ZERO {
            self.min = self.min * m;
            self.max = self.max * m;
        } else {
            let tmp = self.min;
            self.min = self.max * m;
            self.max = tmp * m;
        }
    }
}

impl<T: Scalar + Float> Div<T> for Range2<T> {
    type Output = Self;
    fn div(self, m: T) -> Self::Output {
        self * (T::ONE / m)
    }
}

impl<T: Scalar + Float> DivAssign<T> for Range2<T> {
    fn div_assign(&mut self, m: T) {
        *self *= T::ONE / m;
    }
}

impl<T: Scalar + Float + fmt::Display> fmt::Display for Range2<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}), ({}, {})]",
            self.min.x, self.min.y, self.max.x, self.max.y
        )
    }
}

// ============================================================================
// Range3 - 3D axis-aligned bounding box
// ============================================================================

/// 3-dimensional range (axis-aligned bounding box).
///
/// An empty range has max < min for any component.
///
/// # Examples
///
/// ```
/// use usd_gf::{Range3d, Vec3d};
///
/// let bbox = Range3d::new(
///     Vec3d::new(0.0, 0.0, 0.0),
///     Vec3d::new(1.0, 1.0, 1.0)
/// );
/// assert!(!bbox.is_empty());
/// assert!(bbox.contains_point(&Vec3d::new(0.5, 0.5, 0.5)));
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Range3<T> {
    min: Vec3<T>,
    max: Vec3<T>,
}

/// Double-precision 3D range (AABB).
pub type Range3d = Range3<f64>;

/// Single-precision 3D range (AABB).
pub type Range3f = Range3<f32>;

impl<T: Scalar + Float> Range3<T> {
    /// Creates a new range with given min and max corners.
    #[inline]
    #[must_use]
    pub fn new(min: Vec3<T>, max: Vec3<T>) -> Self {
        Self { min, max }
    }

    /// Creates an empty range.
    #[inline]
    #[must_use]
    pub fn empty() -> Self {
        let big = T::max_value();
        let small = T::min_value();
        Self {
            min: Vec3 {
                x: big,
                y: big,
                z: big,
            },
            max: Vec3 {
                x: small,
                y: small,
                z: small,
            },
        }
    }

    /// The unit cube [0,1]^3.
    #[must_use]
    pub fn unit_cube() -> Self {
        Self {
            min: Vec3 {
                x: T::ZERO,
                y: T::ZERO,
                z: T::ZERO,
            },
            max: Vec3 {
                x: T::ONE,
                y: T::ONE,
                z: T::ONE,
            },
        }
    }

    /// Returns the minimum corner.
    #[inline]
    #[must_use]
    pub fn min(&self) -> &Vec3<T> {
        &self.min
    }

    /// Returns the maximum corner.
    #[inline]
    #[must_use]
    pub fn max(&self) -> &Vec3<T> {
        &self.max
    }

    /// Sets the minimum corner.
    #[inline]
    pub fn set_min(&mut self, min: Vec3<T>) {
        self.min = min;
    }

    /// Sets the maximum corner.
    #[inline]
    pub fn set_max(&mut self, max: Vec3<T>) {
        self.max = max;
    }

    /// Sets the range to empty.
    #[inline]
    pub fn set_empty(&mut self) {
        let big = T::max_value();
        let small = T::min_value();
        self.min = Vec3 {
            x: big,
            y: big,
            z: big,
        };
        self.max = Vec3 {
            x: small,
            y: small,
            z: small,
        };
    }

    /// Returns the size (width, height, depth).
    #[inline]
    #[must_use]
    pub fn size(&self) -> Vec3<T> {
        Vec3 {
            x: self.max.x - self.min.x,
            y: self.max.y - self.min.y,
            z: self.max.z - self.min.z,
        }
    }

    /// Returns the center point.
    #[inline]
    #[must_use]
    pub fn midpoint(&self) -> Vec3<T> {
        let half = T::ONE / (T::ONE + T::ONE);
        Vec3 {
            x: half * self.min.x + half * self.max.x,
            y: half * self.min.y + half * self.max.y,
            z: half * self.min.z + half * self.max.z,
        }
    }

    /// Returns true if the range is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    /// Returns true if the point is inside the range.
    #[inline]
    #[must_use]
    pub fn contains_point(&self, point: &Vec3<T>) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Returns true if the other range is entirely inside this range.
    #[inline]
    #[must_use]
    pub fn contains_range(&self, other: &Self) -> bool {
        self.contains_point(&other.min) && self.contains_point(&other.max)
    }

    /// Returns true if the ranges don't overlap.
    #[inline]
    #[must_use]
    pub fn is_outside(&self, other: &Self) -> bool {
        other.max.x < self.min.x
            || other.min.x > self.max.x
            || other.max.y < self.min.y
            || other.min.y > self.max.y
            || other.max.z < self.min.z
            || other.min.z > self.max.z
    }

    /// Extends this range to include the given point.
    pub fn union_with_point(&mut self, point: &Vec3<T>) {
        if point.x < self.min.x {
            self.min.x = point.x;
        }
        if point.y < self.min.y {
            self.min.y = point.y;
        }
        if point.z < self.min.z {
            self.min.z = point.z;
        }
        if point.x > self.max.x {
            self.max.x = point.x;
        }
        if point.y > self.max.y {
            self.max.y = point.y;
        }
        if point.z > self.max.z {
            self.max.z = point.z;
        }
    }

    /// Extends this range to include the given range.
    pub fn union_with(&mut self, other: &Self) {
        if other.min.x < self.min.x {
            self.min.x = other.min.x;
        }
        if other.min.y < self.min.y {
            self.min.y = other.min.y;
        }
        if other.min.z < self.min.z {
            self.min.z = other.min.z;
        }
        if other.max.x > self.max.x {
            self.max.x = other.max.x;
        }
        if other.max.y > self.max.y {
            self.max.y = other.max.y;
        }
        if other.max.z > self.max.z {
            self.max.z = other.max.z;
        }
    }

    /// Returns the union of two ranges.
    #[must_use]
    pub fn get_union(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.union_with(b);
        result
    }

    /// Intersects this range with the given range.
    pub fn intersect_with(&mut self, other: &Self) {
        if other.min.x > self.min.x {
            self.min.x = other.min.x;
        }
        if other.min.y > self.min.y {
            self.min.y = other.min.y;
        }
        if other.min.z > self.min.z {
            self.min.z = other.min.z;
        }
        if other.max.x < self.max.x {
            self.max.x = other.max.x;
        }
        if other.max.y < self.max.y {
            self.max.y = other.max.y;
        }
        if other.max.z < self.max.z {
            self.max.z = other.max.z;
        }
    }

    /// Returns the intersection of two ranges.
    #[must_use]
    pub fn get_intersection(a: &Self, b: &Self) -> Self {
        let mut result = *a;
        result.intersect_with(b);
        result
    }

    /// Returns the squared distance from a point to the range.
    #[must_use]
    pub fn distance_squared(&self, point: &Vec3<T>) -> T {
        let mut dist_sq = T::ZERO;

        if point.x < self.min.x {
            let d = self.min.x - point.x;
            dist_sq += d * d;
        } else if point.x > self.max.x {
            let d = point.x - self.max.x;
            dist_sq += d * d;
        }

        if point.y < self.min.y {
            let d = self.min.y - point.y;
            dist_sq += d * d;
        } else if point.y > self.max.y {
            let d = point.y - self.max.y;
            dist_sq += d * d;
        }

        if point.z < self.min.z {
            let d = self.min.z - point.z;
            dist_sq += d * d;
        } else if point.z > self.max.z {
            let d = point.z - self.max.z;
            dist_sq += d * d;
        }

        dist_sq
    }

    /// Returns the ith corner.
    ///
    /// Order: LDB(0), RDB(1), LUB(2), RUB(3), LDF(4), RDF(5), LUF(6), RUF(7).
    /// L/R = left/right (x), D/U = down/up (y), B/F = back/front (z).
    #[must_use]
    pub fn corner(&self, i: usize) -> Vec3<T> {
        Vec3 {
            x: if i & 1 == 0 { self.min.x } else { self.max.x },
            y: if i & 2 == 0 { self.min.y } else { self.max.y },
            z: if i & 4 == 0 { self.min.z } else { self.max.z },
        }
    }

    /// Returns the ith octant.
    ///
    /// Order: LDB(0), RDB(1), LUB(2), RUB(3), LDF(4), RDF(5), LUF(6), RUF(7).
    #[must_use]
    pub fn octant(&self, i: usize) -> Self {
        let mid = self.midpoint();
        let min_x = if i & 1 == 0 { self.min.x } else { mid.x };
        let max_x = if i & 1 == 0 { mid.x } else { self.max.x };
        let min_y = if i & 2 == 0 { self.min.y } else { mid.y };
        let max_y = if i & 2 == 0 { mid.y } else { self.max.y };
        let min_z = if i & 4 == 0 { self.min.z } else { mid.z };
        let max_z = if i & 4 == 0 { mid.z } else { self.max.z };

        Self {
            min: Vec3 {
                x: min_x,
                y: min_y,
                z: min_z,
            },
            max: Vec3 {
                x: max_x,
                y: max_y,
                z: max_z,
            },
        }
    }
}

impl<T: Scalar + Float> Default for Range3<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T: Scalar + Float + PartialEq> PartialEq for Range3<T> {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

// Cross-type conversions (matching C++ implicit conversions)
impl From<Range3f> for Range3d {
    fn from(other: Range3f) -> Self {
        use crate::vec3::Vec3d;
        Self::new(
            Vec3d::new(
                other.min().x as f64,
                other.min().y as f64,
                other.min().z as f64,
            ),
            Vec3d::new(
                other.max().x as f64,
                other.max().y as f64,
                other.max().z as f64,
            ),
        )
    }
}

// Cross-type equality comparisons (matching C++ operator== overloads)
impl PartialEq<Range3f> for Range3d {
    fn eq(&self, other: &Range3f) -> bool {
        (self.min.x - other.min().x as f64).abs() < f64::EPSILON
            && (self.min.y - other.min().y as f64).abs() < f64::EPSILON
            && (self.min.z - other.min().z as f64).abs() < f64::EPSILON
            && (self.max.x - other.max().x as f64).abs() < f64::EPSILON
            && (self.max.y - other.max().y as f64).abs() < f64::EPSILON
            && (self.max.z - other.max().z as f64).abs() < f64::EPSILON
    }
}

impl<T: Scalar + Float> Add for Range3<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min + rhs.min,
            max: self.max + rhs.max,
        }
    }
}

impl<T: Scalar + Float> AddAssign for Range3<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.min = self.min + rhs.min;
        self.max = self.max + rhs.max;
    }
}

impl<T: Scalar + Float> Sub for Range3<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min - rhs.max,
            max: self.max - rhs.min,
        }
    }
}

impl<T: Scalar + Float> SubAssign for Range3<T> {
    fn sub_assign(&mut self, rhs: Self) {
        let new_min = self.min - rhs.max;
        let new_max = self.max - rhs.min;
        self.min = new_min;
        self.max = new_max;
    }
}

impl<T: Scalar + Float> Mul<T> for Range3<T> {
    type Output = Self;
    fn mul(self, m: T) -> Self::Output {
        if m > T::ZERO {
            Self {
                min: self.min * m,
                max: self.max * m,
            }
        } else {
            Self {
                min: self.max * m,
                max: self.min * m,
            }
        }
    }
}

impl<T: Scalar + Float> MulAssign<T> for Range3<T> {
    fn mul_assign(&mut self, m: T) {
        if m > T::ZERO {
            self.min = self.min * m;
            self.max = self.max * m;
        } else {
            let tmp = self.min;
            self.min = self.max * m;
            self.max = tmp * m;
        }
    }
}

impl<T: Scalar + Float> Div<T> for Range3<T> {
    type Output = Self;
    fn div(self, m: T) -> Self::Output {
        self * (T::ONE / m)
    }
}

impl<T: Scalar + Float> DivAssign<T> for Range3<T> {
    fn div_assign(&mut self, m: T) {
        *self *= T::ONE / m;
    }
}

impl<T: Scalar + Float + fmt::Display> fmt::Display for Range3<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}, {}), ({}, {}, {})]",
            self.min.x, self.min.y, self.min.z, self.max.x, self.max.y, self.max.z
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec2::Vec2d;
    use crate::vec3::Vec3d;

    // Range1 tests
    #[test]
    fn test_range1_new() {
        let r = Range1d::new(0.0, 10.0);
        assert_eq!(r.min(), 0.0);
        assert_eq!(r.max(), 10.0);
    }

    #[test]
    fn test_range1_empty() {
        let r = Range1d::empty();
        assert!(r.is_empty());
    }

    #[test]
    fn test_range1_size() {
        let r = Range1d::new(0.0, 10.0);
        assert!((r.size() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_range1_midpoint() {
        let r = Range1d::new(0.0, 10.0);
        assert!((r.midpoint() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_range1_contains() {
        let r = Range1d::new(0.0, 10.0);
        assert!(r.contains(5.0));
        assert!(r.contains(0.0));
        assert!(r.contains(10.0));
        assert!(!r.contains(-1.0));
        assert!(!r.contains(11.0));
    }

    #[test]
    fn test_range1_union() {
        let r1 = Range1d::new(0.0, 5.0);
        let r2 = Range1d::new(3.0, 10.0);
        let u = Range1d::get_union(&r1, &r2);
        assert_eq!(u.min(), 0.0);
        assert_eq!(u.max(), 10.0);
    }

    #[test]
    fn test_range1_intersection() {
        let r1 = Range1d::new(0.0, 5.0);
        let r2 = Range1d::new(3.0, 10.0);
        let i = Range1d::get_intersection(&r1, &r2);
        assert_eq!(i.min(), 3.0);
        assert_eq!(i.max(), 5.0);
    }

    #[test]
    fn test_range1_distance() {
        let r = Range1d::new(0.0, 10.0);
        assert!((r.distance_squared(-2.0) - 4.0).abs() < 1e-10);
        assert!((r.distance_squared(12.0) - 4.0).abs() < 1e-10);
        assert!((r.distance_squared(5.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_range1_multiply() {
        let r = Range1d::new(1.0, 2.0);
        let scaled = r * 2.0;
        assert_eq!(scaled.min(), 2.0);
        assert_eq!(scaled.max(), 4.0);

        let neg_scaled = r * -1.0;
        assert_eq!(neg_scaled.min(), -2.0);
        assert_eq!(neg_scaled.max(), -1.0);
    }

    // Range2 tests
    #[test]
    fn test_range2_new() {
        let r = Range2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 10.0));
        assert_eq!(r.min().x, 0.0);
        assert_eq!(r.max().x, 10.0);
    }

    #[test]
    fn test_range2_empty() {
        let r = Range2d::empty();
        assert!(r.is_empty());
    }

    #[test]
    fn test_range2_contains() {
        let r = Range2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 10.0));
        assert!(r.contains_point(&Vec2d::new(5.0, 5.0)));
        assert!(!r.contains_point(&Vec2d::new(-1.0, 5.0)));
    }

    #[test]
    fn test_range2_corner() {
        let r = Range2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 10.0));
        assert_eq!(r.corner(0).x, 0.0); // SW
        assert_eq!(r.corner(0).y, 0.0);
        assert_eq!(r.corner(3).x, 10.0); // NE
        assert_eq!(r.corner(3).y, 10.0);
    }

    #[test]
    fn test_range2_unit_square() {
        let r = Range2d::unit_square();
        assert_eq!(r.min().x, 0.0);
        assert_eq!(r.max().x, 1.0);
    }

    // Range3 tests
    #[test]
    fn test_range3_new() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        assert_eq!(r.min().x, 0.0);
        assert_eq!(r.max().x, 10.0);
    }

    #[test]
    fn test_range3_empty() {
        let r = Range3d::empty();
        assert!(r.is_empty());
    }

    #[test]
    fn test_range3_contains() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        assert!(r.contains_point(&Vec3d::new(5.0, 5.0, 5.0)));
        assert!(!r.contains_point(&Vec3d::new(-1.0, 5.0, 5.0)));
    }

    #[test]
    fn test_range3_union() {
        let r1 = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(5.0, 5.0, 5.0));
        let r2 = Range3d::new(Vec3d::new(3.0, 3.0, 3.0), Vec3d::new(10.0, 10.0, 10.0));
        let u = Range3d::get_union(&r1, &r2);
        assert_eq!(u.min().x, 0.0);
        assert_eq!(u.max().x, 10.0);
    }

    #[test]
    fn test_range3_corner() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        assert_eq!(r.corner(0).x, 0.0); // LDB
        assert_eq!(r.corner(7).x, 10.0); // RUF
        assert_eq!(r.corner(7).y, 10.0);
        assert_eq!(r.corner(7).z, 10.0);
    }

    #[test]
    fn test_range3_octant() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        let oct = r.octant(0); // LDB
        assert_eq!(oct.min().x, 0.0);
        assert_eq!(oct.max().x, 5.0);
    }

    #[test]
    fn test_range3_unit_cube() {
        let r = Range3d::unit_cube();
        assert_eq!(r.min().x, 0.0);
        assert_eq!(r.max().x, 1.0);
    }

    #[test]
    fn test_range3_distance() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        assert!((r.distance_squared(&Vec3d::new(5.0, 5.0, 5.0)) - 0.0).abs() < 1e-10);
        assert!((r.distance_squared(&Vec3d::new(-1.0, 5.0, 5.0)) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_range3_is_outside() {
        let r1 = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(5.0, 5.0, 5.0));
        let r2 = Range3d::new(Vec3d::new(10.0, 10.0, 10.0), Vec3d::new(15.0, 15.0, 15.0));
        assert!(r1.is_outside(&r2));
    }

    #[test]
    fn test_range3_multiply() {
        let r = Range3d::new(Vec3d::new(1.0, 1.0, 1.0), Vec3d::new(2.0, 2.0, 2.0));
        let scaled = r * 2.0;
        assert_eq!(scaled.min().x, 2.0);
        assert_eq!(scaled.max().x, 4.0);
    }

    #[test]
    fn test_range3_midpoint() {
        let r = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 10.0, 10.0));
        let mid = r.midpoint();
        assert!((mid.x - 5.0).abs() < 1e-10);
        assert!((mid.y - 5.0).abs() < 1e-10);
        assert!((mid.z - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_range_default() {
        let r1: Range1d = Default::default();
        let r2: Range2d = Default::default();
        let r3: Range3d = Default::default();
        assert!(r1.is_empty());
        assert!(r2.is_empty());
        assert!(r3.is_empty());
    }

    #[test]
    fn test_range_display() {
        let r1 = Range1d::new(0.0, 10.0);
        let s1 = format!("{}", r1);
        assert!(s1.contains("0"));

        let r3 = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 1.0));
        let s3 = format!("{}", r3);
        assert!(s3.contains("1"));
    }
}
