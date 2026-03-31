//! Mathematical interval with open/closed boundary conditions.
//!
//! [`Interval`] represents an interval on the real number line with
//! configurable boundary conditions (open or closed at each end).
//!
//! # Examples
//!
//! ```
//! use usd_gf::Interval;
//!
//! // Closed interval [0, 10]
//! let closed = Interval::new(0.0, 10.0, true, true);
//! assert!(closed.contains(0.0));
//! assert!(closed.contains(10.0));
//!
//! // Open interval (0, 10)
//! let open = Interval::new(0.0, 10.0, false, false);
//! assert!(!open.contains(0.0));
//! assert!(!open.contains(10.0));
//! assert!(open.contains(5.0));
//!
//! // Half-open interval [0, 10)
//! let half_open = Interval::new(0.0, 10.0, true, false);
//! assert!(half_open.contains(0.0));
//! assert!(!half_open.contains(10.0));
//! ```

use crate::math::max as gf_max;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Neg};

/// A boundary of an interval.
///
/// Stores the boundary value and whether it's closed (inclusive).
#[derive(Clone, Copy, Debug)]
struct Bound {
    /// Boundary value.
    value: f64,
    /// True if boundary is closed (inclusive), false if open (exclusive).
    closed: bool,
}

impl Bound {
    /// Creates a new boundary, forcing open boundaries for infinite values.
    fn new(value: f64, closed: bool) -> Self {
        // Infinite boundaries must be open
        let actual_closed = if value.is_infinite() { false } else { closed };
        Self {
            value,
            closed: actual_closed,
        }
    }
}

impl PartialEq for Bound {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.closed == other.closed
    }
}

impl Eq for Bound {}

impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bound {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.value < other.value {
            Ordering::Less
        } else if self.value > other.value {
            Ordering::Greater
        } else if self.closed && !other.closed {
            // Same value: closed < open (for minimum bounds)
            Ordering::Less
        } else if !self.closed && other.closed {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl Hash for Bound {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.to_bits().hash(state);
        self.closed.hash(state);
    }
}

/// A mathematical interval with configurable boundary conditions.
///
/// An interval can have open or closed boundaries at either end.
/// - Closed `[a, b]`: includes boundary values
/// - Open `(a, b)`: excludes boundary values
/// - Half-open `[a, b)` or `(a, b]`: one closed, one open
///
/// Empty intervals are represented when min > max or min == max with open boundary.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    /// Minimum boundary.
    min: Bound,
    /// Maximum boundary.
    max: Bound,
}

impl Default for Interval {
    /// Creates an empty open interval (0, 0).
    fn default() -> Self {
        Self {
            min: Bound::new(0.0, false),
            max: Bound::new(0.0, false),
        }
    }
}

impl Interval {
    /// Creates an empty open interval (0, 0).
    #[inline]
    #[must_use]
    pub fn new_empty() -> Self {
        Self::default()
    }

    /// Creates a closed interval representing a single point [val, val].
    #[must_use]
    pub fn from_point(val: f64) -> Self {
        Self {
            min: Bound::new(val, true),
            max: Bound::new(val, true),
        }
    }

    /// Creates an interval with the given bounds and boundary conditions.
    ///
    /// By default, creates a closed interval [min, max].
    #[must_use]
    pub fn new(min: f64, max: f64, min_closed: bool, max_closed: bool) -> Self {
        Self {
            min: Bound::new(min, min_closed),
            max: Bound::new(max, max_closed),
        }
    }

    /// Creates a closed interval [min, max].
    #[must_use]
    pub fn closed(min: f64, max: f64) -> Self {
        Self::new(min, max, true, true)
    }

    /// Creates an open interval (min, max).
    #[must_use]
    pub fn open(min: f64, max: f64) -> Self {
        Self::new(min, max, false, false)
    }

    /// Returns the full interval (-inf, inf).
    #[must_use]
    pub fn full() -> Self {
        Self::new(f64::NEG_INFINITY, f64::INFINITY, false, false)
    }

    /// Returns the full interval (-inf, inf).
    /// Alias for `full()` to match C++ API naming.
    ///
    /// Matches C++ `GfInterval::GetFullInterval()`.
    #[must_use]
    pub fn get_full_interval() -> Self {
        Self::full()
    }

    /// Returns the minimum value.
    #[inline]
    #[must_use]
    pub fn get_min(&self) -> f64 {
        self.min.value
    }

    /// Returns the maximum value.
    #[inline]
    #[must_use]
    pub fn get_max(&self) -> f64 {
        self.max.value
    }

    /// Sets the minimum value, preserving boundary condition.
    #[inline]
    pub fn set_min(&mut self, v: f64) {
        self.min = Bound::new(v, self.min.closed);
    }

    /// Sets the minimum value and boundary condition.
    #[inline]
    pub fn set_min_with_closed(&mut self, v: f64, closed: bool) {
        self.min = Bound::new(v, closed);
    }

    /// Sets the maximum value, preserving boundary condition.
    #[inline]
    pub fn set_max(&mut self, v: f64) {
        self.max = Bound::new(v, self.max.closed);
    }

    /// Sets the maximum value and boundary condition.
    #[inline]
    pub fn set_max_with_closed(&mut self, v: f64, closed: bool) {
        self.max = Bound::new(v, closed);
    }

    /// Returns true if the minimum boundary is closed.
    #[inline]
    #[must_use]
    pub fn is_min_closed(&self) -> bool {
        self.min.closed
    }

    /// Returns true if the maximum boundary is closed.
    #[inline]
    #[must_use]
    pub fn is_max_closed(&self) -> bool {
        self.max.closed
    }

    /// Returns true if the minimum boundary is open.
    #[inline]
    #[must_use]
    pub fn is_min_open(&self) -> bool {
        !self.min.closed
    }

    /// Returns true if the maximum boundary is open.
    #[inline]
    #[must_use]
    pub fn is_max_open(&self) -> bool {
        !self.max.closed
    }

    /// Returns true if the minimum value is finite.
    #[inline]
    #[must_use]
    pub fn is_min_finite(&self) -> bool {
        self.min.value.is_finite()
    }

    /// Returns true if the maximum value is finite.
    #[inline]
    #[must_use]
    pub fn is_max_finite(&self) -> bool {
        self.max.value.is_finite()
    }

    /// Returns true if both bounds are finite.
    #[inline]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.is_min_finite() && self.is_max_finite()
    }

    /// Returns true if the interval is empty.
    ///
    /// An interval is empty if min > max, or if min == max and either boundary is open.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        (self.min.value > self.max.value)
            || (self.min.value == self.max.value && (!self.min.closed || !self.max.closed))
    }

    /// Returns the size (width) of the interval.
    ///
    /// An empty interval has size 0.
    #[inline]
    #[must_use]
    pub fn size(&self) -> f64 {
        gf_max(0.0, self.max.value - self.min.value)
    }

    /// Returns true if the interval contains the given value.
    ///
    /// An empty interval contains no values.
    #[must_use]
    pub fn contains(&self, d: f64) -> bool {
        let above_min = d > self.min.value || (d == self.min.value && self.min.closed);
        let below_max = d < self.max.value || (d == self.max.value && self.max.closed);
        above_min && below_max
    }

    /// Returns true if this interval entirely contains the other interval.
    ///
    /// An empty interval contains no intervals, not even other empty intervals.
    #[must_use]
    pub fn contains_interval(&self, other: &Interval) -> bool {
        (*self & *other) == *other
    }

    /// Returns true if this interval intersects the other interval.
    #[must_use]
    pub fn intersects(&self, other: &Interval) -> bool {
        !(*self & *other).is_empty()
    }

    /// Returns the intersection of two intervals.
    #[must_use]
    pub fn intersection(&self, other: &Interval) -> Interval {
        *self & *other
    }

    /// Returns an interval bounding the union of two intervals.
    #[must_use]
    pub fn hull(&self, other: &Interval) -> Interval {
        *self | *other
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

impl Eq for Interval {}

impl PartialOrd for Interval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Interval {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.min.cmp(&other.min) {
            Ordering::Equal => self.max.cmp(&other.max),
            other => other,
        }
    }
}

impl Hash for Interval {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.min.hash(state);
        self.max.hash(state);
    }
}

// Interval arithmetic: intersection (&)
impl BitAnd for Interval {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut result = self;
        result &= rhs;
        result
    }
}

impl BitAndAssign for Interval {
    fn bitand_assign(&mut self, rhs: Self) {
        if self.is_empty() {
            // No change
        } else if rhs.is_empty() {
            *self = Interval::new_empty();
        } else {
            // Intersect min: take the larger (more restrictive) bound
            if self.min.value < rhs.min.value {
                self.min = rhs.min;
            } else if self.min.value == rhs.min.value {
                self.min.closed &= rhs.min.closed;
            }

            // Intersect max: take the smaller (more restrictive) bound
            if self.max.value > rhs.max.value {
                self.max = rhs.max;
            } else if self.max.value == rhs.max.value {
                self.max.closed &= rhs.max.closed;
            }
        }
    }
}

// Interval arithmetic: hull/union (|)
impl BitOr for Interval {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = self;
        result |= rhs;
        result
    }
}

impl BitOrAssign for Interval {
    fn bitor_assign(&mut self, rhs: Self) {
        if self.is_empty() {
            *self = rhs;
        } else if rhs.is_empty() {
            // No change
        } else {
            // Expand min: take the smaller (less restrictive) bound
            if self.min.value > rhs.min.value {
                self.min = rhs.min;
            } else if self.min.value == rhs.min.value {
                self.min.closed |= rhs.min.closed;
            }

            // Expand max: take the larger (less restrictive) bound
            if self.max.value < rhs.max.value {
                self.max = rhs.max;
            } else if self.max.value == rhs.max.value {
                self.max.closed |= rhs.max.closed;
            }
        }
    }
}

// Interval arithmetic: addition
impl Add for Interval {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut result = self;
        result += rhs;
        result
    }
}

impl AddAssign for Interval {
    fn add_assign(&mut self, rhs: Self) {
        if !rhs.is_empty() {
            self.min.value += rhs.min.value;
            self.max.value += rhs.max.value;
            self.min.closed &= rhs.min.closed;
            self.max.closed &= rhs.max.closed;
        }
    }
}

// Interval arithmetic: negation
impl Neg for Interval {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Interval::new(
            -self.max.value,
            -self.min.value,
            self.max.closed,
            self.min.closed,
        )
    }
}

// Interval arithmetic: subtraction
impl Sub for Interval {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl SubAssign for Interval {
    fn sub_assign(&mut self, rhs: Self) {
        *self += -rhs;
    }
}

// Helper functions for bound min/max
fn bound_min(a: Bound, b: Bound) -> Bound {
    if a.value < b.value || (a.value == b.value && a.closed && !b.closed) {
        a
    } else {
        b
    }
}

fn bound_max(a: Bound, b: Bound) -> Bound {
    if a.value < b.value || (a.value == b.value && !a.closed && b.closed) {
        b
    } else {
        a
    }
}

// Interval arithmetic: multiplication
impl Mul for Interval {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut result = self;
        result *= rhs;
        result
    }
}

impl MulAssign for Interval {
    fn mul_assign(&mut self, rhs: Self) {
        let a = Bound::new(
            self.min.value * rhs.min.value,
            self.min.closed && rhs.min.closed,
        );
        let b = Bound::new(
            self.min.value * rhs.max.value,
            self.min.closed && rhs.max.closed,
        );
        let c = Bound::new(
            self.max.value * rhs.min.value,
            self.max.closed && rhs.min.closed,
        );
        let d = Bound::new(
            self.max.value * rhs.max.value,
            self.max.closed && rhs.max.closed,
        );

        self.max = bound_max(bound_max(a, b), bound_max(c, d));
        self.min = bound_min(bound_min(a, b), bound_min(c, d));
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let min_bracket = if self.min.closed { '[' } else { '(' };
        let max_bracket = if self.max.closed { ']' } else { ')' };
        write!(
            f,
            "{}{}, {}{}",
            min_bracket, self.min.value, self.max.value, max_bracket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let i = Interval::default();
        assert!(i.is_empty());
        assert_eq!(i.get_min(), 0.0);
        assert_eq!(i.get_max(), 0.0);
    }

    #[test]
    fn test_from_point() {
        let i = Interval::from_point(5.0);
        assert!(!i.is_empty());
        assert!(i.is_min_closed());
        assert!(i.is_max_closed());
        assert!(i.contains(5.0));
        assert!(!i.contains(4.99));
        assert!(!i.contains(5.01));
    }

    #[test]
    fn test_closed() {
        let i = Interval::closed(0.0, 10.0);
        assert!(i.contains(0.0));
        assert!(i.contains(5.0));
        assert!(i.contains(10.0));
        assert!(!i.contains(-0.01));
        assert!(!i.contains(10.01));
    }

    #[test]
    fn test_open() {
        let i = Interval::open(0.0, 10.0);
        assert!(!i.contains(0.0));
        assert!(i.contains(5.0));
        assert!(!i.contains(10.0));
    }

    #[test]
    fn test_half_open() {
        let i = Interval::new(0.0, 10.0, true, false);
        assert!(i.contains(0.0));
        assert!(i.contains(5.0));
        assert!(!i.contains(10.0));
    }

    #[test]
    fn test_is_empty() {
        // Empty: min > max
        assert!(Interval::closed(10.0, 0.0).is_empty());
        // Empty: min == max with open boundary
        assert!(Interval::open(5.0, 5.0).is_empty());
        assert!(Interval::new(5.0, 5.0, true, false).is_empty());
        // Not empty: min == max with both closed
        assert!(!Interval::closed(5.0, 5.0).is_empty());
    }

    #[test]
    fn test_size() {
        assert_eq!(Interval::closed(0.0, 10.0).size(), 10.0);
        assert_eq!(Interval::closed(10.0, 0.0).size(), 0.0); // Empty interval
    }

    #[test]
    fn test_intersection() {
        let a = Interval::closed(0.0, 10.0);
        let b = Interval::closed(5.0, 15.0);
        let c = a & b;
        assert_eq!(c.get_min(), 5.0);
        assert_eq!(c.get_max(), 10.0);
        assert!(c.is_min_closed());
        assert!(c.is_max_closed());
    }

    #[test]
    fn test_intersection_empty() {
        let a = Interval::closed(0.0, 5.0);
        let b = Interval::closed(10.0, 15.0);
        let c = a & b;
        assert!(c.is_empty());
    }

    #[test]
    fn test_union() {
        let a = Interval::closed(0.0, 5.0);
        let b = Interval::closed(10.0, 15.0);
        let c = a | b;
        assert_eq!(c.get_min(), 0.0);
        assert_eq!(c.get_max(), 15.0);
    }

    #[test]
    fn test_addition() {
        let a = Interval::closed(1.0, 2.0);
        let b = Interval::closed(3.0, 4.0);
        let c = a + b;
        assert_eq!(c.get_min(), 4.0);
        assert_eq!(c.get_max(), 6.0);
    }

    #[test]
    fn test_subtraction() {
        let a = Interval::closed(5.0, 10.0);
        let b = Interval::closed(1.0, 2.0);
        let c = a - b;
        assert_eq!(c.get_min(), 3.0);
        assert_eq!(c.get_max(), 9.0);
    }

    #[test]
    fn test_negation() {
        let a = Interval::closed(1.0, 5.0);
        let b = -a;
        assert_eq!(b.get_min(), -5.0);
        assert_eq!(b.get_max(), -1.0);
    }

    #[test]
    fn test_multiplication() {
        let a = Interval::closed(2.0, 3.0);
        let b = Interval::closed(4.0, 5.0);
        let c = a * b;
        assert_eq!(c.get_min(), 8.0);
        assert_eq!(c.get_max(), 15.0);
    }

    #[test]
    fn test_contains_interval() {
        let outer = Interval::closed(0.0, 10.0);
        let inner = Interval::closed(2.0, 8.0);
        assert!(outer.contains_interval(&inner));
        assert!(!inner.contains_interval(&outer));
    }

    #[test]
    fn test_intersects() {
        let a = Interval::closed(0.0, 5.0);
        let b = Interval::closed(3.0, 10.0);
        let c = Interval::closed(6.0, 10.0);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_full() {
        let f = Interval::full();
        assert!(f.contains(0.0));
        assert!(f.contains(1e100));
        assert!(f.contains(-1e100));
        assert!(f.is_min_open());
        assert!(f.is_max_open());
    }

    #[test]
    fn test_infinite_bounds_are_open() {
        let i = Interval::new(f64::NEG_INFINITY, f64::INFINITY, true, true);
        // Infinite bounds are forced to be open
        assert!(i.is_min_open());
        assert!(i.is_max_open());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Interval::closed(0.0, 10.0)), "[0, 10]");
        assert_eq!(format!("{}", Interval::open(0.0, 10.0)), "(0, 10)");
        assert_eq!(
            format!("{}", Interval::new(0.0, 10.0, true, false)),
            "[0, 10)"
        );
    }

    #[test]
    fn test_equality() {
        let a = Interval::closed(0.0, 10.0);
        let b = Interval::closed(0.0, 10.0);
        let c = Interval::open(0.0, 10.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_ordering() {
        let a = Interval::closed(0.0, 5.0);
        let b = Interval::closed(1.0, 5.0);
        assert!(a < b);
    }
}
