//! Line and line segment types for 2D geometry.
//!
//! - `Line2d` - Infinite 2D line defined by point + normalized direction
//! - `LineSeg2d` - Finite 2D line segment defined by two endpoints
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Line2d, LineSeg2d, Vec2d};
//!
//! // Infinite line along X axis
//! let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
//!
//! // Line segment from (0,0) to (10,0)
//! let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
//! assert!((seg.length() - 10.0).abs() < 1e-10);
//! ```

use crate::math::{clamp, is_close};
use crate::vec2::Vec2d;
use std::fmt;

/// An infinite 2D line.
///
/// Defined parametrically as `p(t) = origin + t * direction` for t in (-inf, inf).
/// The direction is normalized to unit length.
///
/// # Examples
///
/// ```
/// use usd_gf::{Line2d, Vec2d};
///
/// let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(2.0, 0.0));
///
/// // Direction is normalized
/// assert!((line.direction().x - 1.0).abs() < 1e-10);
///
/// // Get point at parameter t=5
/// let p = line.point(5.0);
/// assert!((p.x - 5.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Line2d {
    /// Point on the line.
    origin: Vec2d,
    /// Normalized direction vector.
    direction: Vec2d,
}

impl Line2d {
    /// Creates a new line from a point and direction.
    ///
    /// The direction is normalized.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Line2d, Vec2d};
    ///
    /// let line = Line2d::new(Vec2d::new(1.0, 2.0), Vec2d::new(0.0, 2.0));
    /// assert!((line.direction().y - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn new(origin: Vec2d, direction: Vec2d) -> Self {
        let normalized = direction.normalized();
        Self {
            origin,
            direction: normalized,
        }
    }

    /// Creates a line and returns it along with the original direction length.
    #[must_use]
    pub fn new_with_length(origin: Vec2d, direction: Vec2d) -> (Self, f64) {
        let len = direction.length();
        let normalized = if len > f64::EPSILON {
            Vec2d::new(direction.x / len, direction.y / len)
        } else {
            Vec2d::new(1.0, 0.0)
        };
        (
            Self {
                origin,
                direction: normalized,
            },
            len,
        )
    }

    /// Sets the line from a point and direction, returns direction length.
    pub fn set(&mut self, origin: Vec2d, direction: Vec2d) -> f64 {
        self.origin = origin;
        let len = direction.length();
        self.direction = if len > f64::EPSILON {
            Vec2d::new(direction.x / len, direction.y / len)
        } else {
            Vec2d::new(1.0, 0.0)
        };
        len
    }

    /// Returns the origin point.
    #[inline]
    #[must_use]
    pub fn origin(&self) -> &Vec2d {
        &self.origin
    }

    /// Returns the normalized direction.
    #[inline]
    #[must_use]
    pub fn direction(&self) -> &Vec2d {
        &self.direction
    }

    /// Returns the point at parameter t.
    ///
    /// Since direction is normalized, t represents unit distance along the line.
    #[inline]
    #[must_use]
    pub fn point(&self, t: f64) -> Vec2d {
        Vec2d::new(
            self.origin.x + t * self.direction.x,
            self.origin.y + t * self.direction.y,
        )
    }

    /// Finds the closest point on the line to the given point.
    ///
    /// Returns the closest point and optionally the parameter t.
    #[must_use]
    pub fn find_closest_point(&self, point: &Vec2d) -> (Vec2d, f64) {
        // Vector from line origin to point
        let v = Vec2d::new(point.x - self.origin.x, point.y - self.origin.y);

        // Project onto direction (dot product)
        let t = v.x * self.direction.x + v.y * self.direction.y;

        (self.point(t), t)
    }
}

impl Default for Line2d {
    fn default() -> Self {
        Self {
            origin: Vec2d::new(0.0, 0.0),
            direction: Vec2d::new(1.0, 0.0),
        }
    }
}

impl PartialEq for Line2d {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin && self.direction == other.direction
    }
}

impl Eq for Line2d {}

impl fmt::Display for Line2d {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Line2d(origin=({}, {}), dir=({}, {}))",
            self.origin.x, self.origin.y, self.direction.x, self.direction.y
        )
    }
}

/// A finite 2D line segment.
///
/// Defined by two endpoints p0 and p1. Internally stores as a line plus length.
/// Parametrically: `p(t) = p0 + t * (p1 - p0)` for t in [0, 1].
///
/// # Examples
///
/// ```
/// use usd_gf::{LineSeg2d, Vec2d};
///
/// let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
/// assert!((seg.length() - 10.0).abs() < 1e-10);
///
/// // Midpoint
/// let mid = seg.point(0.5);
/// assert!((mid.x - 5.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct LineSeg2d {
    /// Underlying infinite line (normalized direction).
    line: Line2d,
    /// Length of the segment.
    length: f64,
}

impl LineSeg2d {
    /// Creates a new line segment from two endpoints.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{LineSeg2d, Vec2d};
    ///
    /// let seg = LineSeg2d::new(
    ///     Vec2d::new(0.0, 0.0),
    ///     Vec2d::new(3.0, 4.0)
    /// );
    /// assert!((seg.length() - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn new(p0: Vec2d, p1: Vec2d) -> Self {
        let direction = Vec2d::new(p1.x - p0.x, p1.y - p0.y);
        let (line, length) = Line2d::new_with_length(p0, direction);
        Self { line, length }
    }

    /// Returns the start point (p0).
    #[inline]
    #[must_use]
    pub fn start_point(&self) -> &Vec2d {
        self.line.origin()
    }

    /// Returns the end point (p1).
    #[inline]
    #[must_use]
    pub fn end_point(&self) -> Vec2d {
        self.line.point(self.length)
    }

    /// Returns the normalized direction from p0 to p1.
    #[inline]
    #[must_use]
    pub fn direction(&self) -> &Vec2d {
        self.line.direction()
    }

    /// Returns the length of the segment.
    #[inline]
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Returns the point at parameter t in [0, 1].
    ///
    /// - t=0 returns p0
    /// - t=1 returns p1
    /// - t=0.5 returns the midpoint
    #[inline]
    #[must_use]
    pub fn point(&self, t: f64) -> Vec2d {
        self.line.point(t * self.length)
    }

    /// Finds the closest point on the segment to the given point.
    ///
    /// Returns the closest point and the parameter t clamped to [0, 1].
    #[must_use]
    pub fn find_closest_point(&self, point: &Vec2d) -> (Vec2d, f64) {
        if self.length == 0.0 {
            return (*self.line.origin(), 0.0);
        }

        let (_, t) = self.line.find_closest_point(point);

        // Clamp t to segment range [0, 1]
        let t_clamped = clamp(t / self.length, 0.0, 1.0);

        (self.point(t_clamped), t_clamped)
    }

    /// Returns the underlying infinite line.
    #[inline]
    #[must_use]
    pub(crate) fn inner_line(&self) -> &Line2d {
        &self.line
    }
}

impl Default for LineSeg2d {
    fn default() -> Self {
        Self {
            line: Line2d::default(),
            length: 0.0,
        }
    }
}

impl PartialEq for LineSeg2d {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line && self.length == other.length
    }
}

impl Eq for LineSeg2d {}

impl fmt::Display for LineSeg2d {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p0 = self.start_point();
        let p1 = self.end_point();
        write!(f, "LineSeg2d(({}, {}) -> ({}, {}))", p0.x, p0.y, p1.x, p1.y)
    }
}

// ============================================================================
// Free functions for finding closest points
// ============================================================================

/// Finds the closest points between two infinite 2D lines.
///
/// Returns `None` if the lines are parallel (no unique closest points).
/// Otherwise returns `(p1, p2, t1, t2)` where:
/// - `p1` is the closest point on line1
/// - `p2` is the closest point on line2
/// - `t1` is the parameter on line1
/// - `t2` is the parameter on line2
#[must_use]
pub fn find_closest_points_line2d_line2d(
    l1: &Line2d,
    l2: &Line2d,
) -> Option<(Vec2d, Vec2d, f64, f64)> {
    let p1 = l1.origin();
    let d1 = l1.direction();
    let p2 = l2.origin();
    let d2 = l2.direction();

    // Solve the system:
    //   t2 * a - t1 * b = c
    //   t2 * d - t1 * e = f
    // where:
    //   a = d1.d2, b = d1.d1, c = d1.p1 - d1.p2
    //   d = d2.d2, e = d2.d1 (= a), f = d2.p1 - d2.p2
    let a = d1.x * d2.x + d1.y * d2.y;
    let b = d1.x * d1.x + d1.y * d1.y;
    let c = (d1.x * p1.x + d1.y * p1.y) - (d1.x * p2.x + d1.y * p2.y);
    let d = d2.x * d2.x + d2.y * d2.y;
    let e = a;
    let f = (d2.x * p1.x + d2.y * p1.y) - (d2.x * p2.x + d2.y * p2.y);

    let denom = a * e - b * d;

    // Lines are parallel if denominator is zero
    if is_close(denom, 0.0, 1e-6) {
        return None;
    }

    let t1 = (c * d - a * f) / denom;
    let t2 = (c * e - b * f) / denom;

    Some((l1.point(t1), l2.point(t2), t1, t2))
}

/// Finds the closest points between a 2D line and a 2D line segment.
///
/// Returns `None` if the line and segment are parallel.
/// Otherwise returns `(p1, p2, t1, t2)` where:
/// - `p1` is the closest point on the line
/// - `p2` is the closest point on the segment
/// - `t1` is the parameter on the line
/// - `t2` is the parameter on the segment (in [0, 1])
#[must_use]
pub fn find_closest_points_line2d_seg2d(
    line: &Line2d,
    seg: &LineSeg2d,
) -> Option<(Vec2d, Vec2d, f64, f64)> {
    let result = find_closest_points_line2d_line2d(line, seg.inner_line())?;

    let (mut cp1, _cp2, mut t1, t2) = result;

    // Clamp segment parameter to [0, 1]
    let t2_clamped = clamp(t2 / seg.length(), 0.0, 1.0);
    let cp2 = seg.point(t2_clamped);

    // If we clamped, recompute line's closest point to the clamped segment point
    if t2_clamped <= 0.0 || t2_clamped >= 1.0 {
        let (new_cp1, new_t1) = line.find_closest_point(&cp2);
        cp1 = new_cp1;
        t1 = new_t1;
    }

    Some((cp1, cp2, t1, t2_clamped))
}

/// Finds the closest points between two 2D line segments.
///
/// Returns `None` if the segments are parallel.
/// Otherwise returns `(p1, p2, t1, t2)` where:
/// - `p1` is the closest point on seg1
/// - `p2` is the closest point on seg2
/// - `t1` is the parameter on seg1 (in [0, 1])
/// - `t2` is the parameter on seg2 (in [0, 1])
#[must_use]
pub fn find_closest_points_seg2d_seg2d(
    seg1: &LineSeg2d,
    seg2: &LineSeg2d,
) -> Option<(Vec2d, Vec2d, f64, f64)> {
    let result = find_closest_points_line2d_line2d(seg1.inner_line(), seg2.inner_line())?;

    let (_cp1, _cp2, t1, t2) = result;

    // Clamp both parameters to [0, 1]
    let t1_clamped = clamp(t1 / seg1.length(), 0.0, 1.0);
    let t2_clamped = clamp(t2 / seg2.length(), 0.0, 1.0);

    let p1 = seg1.point(t1_clamped);
    let p2 = seg2.point(t2_clamped);

    Some((p1, p2, t1_clamped, t2_clamped))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    // ===== Line2d tests =====

    #[test]
    fn test_line2d_new() {
        let line = Line2d::new(Vec2d::new(1.0, 2.0), Vec2d::new(3.0, 0.0));
        assert_eq!(line.origin().x, 1.0);
        assert_eq!(line.origin().y, 2.0);
        // Direction should be normalized
        assert!(approx_eq(line.direction().x, 1.0));
        assert!(approx_eq(line.direction().y, 0.0));
    }

    #[test]
    fn test_line2d_point() {
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let p = line.point(5.0);
        assert!(approx_eq(p.x, 5.0));
        assert!(approx_eq(p.y, 0.0));
    }

    #[test]
    fn test_line2d_closest_point() {
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let point = Vec2d::new(5.0, 3.0);
        let (closest, t) = line.find_closest_point(&point);
        assert!(approx_eq(closest.x, 5.0));
        assert!(approx_eq(closest.y, 0.0));
        assert!(approx_eq(t, 5.0));
    }

    #[test]
    fn test_line2d_equality() {
        let l1 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let l2 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        assert_eq!(l1, l2);
    }

    #[test]
    fn test_line2d_display() {
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let s = format!("{}", line);
        assert!(s.contains("Line2d"));
    }

    // ===== LineSeg2d tests =====

    #[test]
    fn test_lineseg2d_new() {
        let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(3.0, 4.0));
        assert!(approx_eq(seg.length(), 5.0));
    }

    #[test]
    fn test_lineseg2d_endpoints() {
        let seg = LineSeg2d::new(Vec2d::new(1.0, 2.0), Vec2d::new(5.0, 6.0));
        assert_eq!(seg.start_point().x, 1.0);
        assert_eq!(seg.start_point().y, 2.0);
        let end = seg.end_point();
        assert!(approx_eq(end.x, 5.0));
        assert!(approx_eq(end.y, 6.0));
    }

    #[test]
    fn test_lineseg2d_point() {
        let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
        let mid = seg.point(0.5);
        assert!(approx_eq(mid.x, 5.0));
        assert!(approx_eq(mid.y, 0.0));
    }

    #[test]
    fn test_lineseg2d_closest_point_inside() {
        let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
        let point = Vec2d::new(5.0, 3.0);
        let (closest, t) = seg.find_closest_point(&point);
        assert!(approx_eq(closest.x, 5.0));
        assert!(approx_eq(closest.y, 0.0));
        assert!(approx_eq(t, 0.5));
    }

    #[test]
    fn test_lineseg2d_closest_point_clamped() {
        let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));

        // Point before segment
        let (closest1, t1) = seg.find_closest_point(&Vec2d::new(-5.0, 0.0));
        assert!(approx_eq(closest1.x, 0.0));
        assert!(approx_eq(t1, 0.0));

        // Point after segment
        let (closest2, t2) = seg.find_closest_point(&Vec2d::new(15.0, 0.0));
        assert!(approx_eq(closest2.x, 10.0));
        assert!(approx_eq(t2, 1.0));
    }

    #[test]
    fn test_lineseg2d_equality() {
        let s1 = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let s2 = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_lineseg2d_display() {
        let seg = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 1.0));
        let s = format!("{}", seg);
        assert!(s.contains("LineSeg2d"));
    }

    // ===== GfFindClosestPoints 2D overload tests =====

    // Overload 4: Line2d vs Line2d
    #[test]
    fn test_closest_points_lines2d_perpendicular() {
        // X-axis line and Y-axis line at x=5: they intersect at (5,0).
        let l1 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let l2 = Line2d::new(Vec2d::new(5.0, 0.0), Vec2d::new(0.0, 1.0));

        let result = find_closest_points_line2d_line2d(&l1, &l2);
        assert!(result.is_some());
        let (p1, p2, t1, t2) = result.unwrap();
        // Lines intersect at (5, 0)
        assert!(approx_eq(p1.x, 5.0));
        assert!(approx_eq(p1.y, 0.0));
        assert!(approx_eq(p2.x, 5.0));
        assert!(approx_eq(p2.y, 0.0));
        assert!(approx_eq(t1, 5.0));
        assert!(approx_eq(t2, 0.0));
    }

    #[test]
    fn test_closest_points_lines2d_oblique() {
        // l1: X-axis (origin 0,0 dir 1,0); l2: vertical (origin 3,4 dir 0,1).
        // These 2D lines INTERSECT. Solve:
        //   a=d1.d2=0, b=1, c=d1.p1-d1.p2=0-3=-3
        //   d=1, e=0, f=d2.p1-d2.p2=0-4=-4
        //   denom = a*e - b*d = -1
        //   t1 = (c*d - a*f)/denom = (-3)/(-1) = 3
        //   t2 = (c*e - b*f)/denom = (4)/(-1) = -4
        //   p1=(3,0), p2=(3,4)+(-4)*(0,1)=(3,0) — same intersection point.
        let l1 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let l2 = Line2d::new(Vec2d::new(3.0, 4.0), Vec2d::new(0.0, 1.0));
        let result = find_closest_points_line2d_line2d(&l1, &l2);
        assert!(result.is_some());
        let (p1, p2, t1, t2) = result.unwrap();
        // Intersection at (3, 0)
        assert!(approx_eq(p1.x, 3.0));
        assert!(approx_eq(p1.y, 0.0));
        assert!(approx_eq(p2.x, 3.0));
        assert!(approx_eq(p2.y, 0.0));
        assert!(approx_eq(t1, 3.0));
        assert!(approx_eq(t2, -4.0)); // parametric distance on l2 from (3,4) to intersection (3,0)
    }

    #[test]
    fn test_closest_points_lines2d_parallel() {
        // Parallel lines — None.
        let l1 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let l2 = Line2d::new(Vec2d::new(0.0, 1.0), Vec2d::new(1.0, 0.0));
        assert!(find_closest_points_line2d_line2d(&l1, &l2).is_none());
    }

    #[test]
    fn test_closest_points_lines2d_same_direction_antiparallel() {
        // Anti-parallel lines (same axis, opposite directions) — None.
        let l1 = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let l2 = Line2d::new(Vec2d::new(0.0, 2.0), Vec2d::new(-1.0, 0.0));
        assert!(find_closest_points_line2d_line2d(&l1, &l2).is_none());
    }

    // Overload 5: Line2d vs LineSeg2d
    #[test]
    fn test_closest_points_line2d_seg2d() {
        // Line along X; segment from (5,1) to (5,10).
        // Unclamped closest on seg's line: t2_line < 0 (segment below X-axis on its parametric axis).
        // After clamping t2=0, p2=(5,1), then recompute p1 on line closest to (5,1) => (5,0).
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let seg = LineSeg2d::new(Vec2d::new(5.0, 1.0), Vec2d::new(5.0, 10.0));

        let result = find_closest_points_line2d_seg2d(&line, &seg);
        assert!(result.is_some());
        let (p1, p2, _t1, t2) = result.unwrap();
        // p2 clamped to segment start (5,1)
        assert!(approx_eq(p1.x, 5.0));
        assert!(approx_eq(p1.y, 0.0));
        assert!(approx_eq(p2.x, 5.0));
        assert!(approx_eq(p2.y, 1.0));
        assert!(approx_eq(t2, 0.0));
    }

    #[test]
    fn test_closest_points_line2d_seg2d_interior() {
        // Line along X; segment from (5,-5) to (5,5).
        // Unclamped t2_line = 5.0 on seg's underlying line (origin at (5,-5), dir (0,1)).
        // t2 = 5/10 = 0.5 (interior) => p2=(5,0), p1=(5,0).
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let seg = LineSeg2d::new(Vec2d::new(5.0, -5.0), Vec2d::new(5.0, 5.0));
        let result = find_closest_points_line2d_seg2d(&line, &seg);
        assert!(result.is_some());
        let (p1, p2, t1, t2) = result.unwrap();
        assert!(approx_eq(p1.x, 5.0), "p1.x={}", p1.x);
        assert!(approx_eq(p1.y, 0.0), "p1.y={}", p1.y);
        assert!(approx_eq(p2.x, 5.0), "p2.x={}", p2.x);
        assert!(approx_eq(p2.y, 0.0), "p2.y={}", p2.y);
        assert!(approx_eq(t1, 5.0), "t1={}", t1);
        assert!(approx_eq(t2, 0.5), "t2={}", t2);
    }

    #[test]
    fn test_closest_points_line2d_seg2d_parallel() {
        // Parallel — None.
        let line = Line2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let seg = LineSeg2d::new(Vec2d::new(0.0, 1.0), Vec2d::new(10.0, 1.0));
        assert!(find_closest_points_line2d_seg2d(&line, &seg).is_none());
    }

    // Overload 6: LineSeg2d vs LineSeg2d
    #[test]
    fn test_closest_points_segs2d() {
        // seg1 along X [0,10]; seg2 vertical at x=5, y=1..10.
        // seg2's underlying line origin at (5,1), dir (0,1).
        // Unclamped t2_line on seg2's line: the closest to X-axis is t2_line=-1 (below start).
        // Clamped t2=0 => p2=(5,1). Clamped t1=5/10=0.5 => p1=(5,0).
        let seg1 = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
        let seg2 = LineSeg2d::new(Vec2d::new(5.0, 1.0), Vec2d::new(5.0, 10.0));

        let result = find_closest_points_seg2d_seg2d(&seg1, &seg2);
        assert!(result.is_some());
        let (p1, p2, t1, t2) = result.unwrap();
        assert!(approx_eq(p1.x, 5.0), "p1.x={}", p1.x);
        assert!(approx_eq(p1.y, 0.0), "p1.y={}", p1.y);
        assert!(approx_eq(p2.x, 5.0), "p2.x={}", p2.x);
        assert!(approx_eq(p2.y, 1.0), "p2.y={}", p2.y);
        assert!(approx_eq(t1, 0.5), "t1={}", t1);
        assert!(approx_eq(t2, 0.0), "t2={}", t2);
    }

    #[test]
    fn test_closest_points_segs2d_crossing() {
        // Crossing segments: seg1 from (0,5) to (10,5); seg2 from (5,0) to (5,10).
        // They cross at (5,5): t1=0.5, t2=0.5.
        let seg1 = LineSeg2d::new(Vec2d::new(0.0, 5.0), Vec2d::new(10.0, 5.0));
        let seg2 = LineSeg2d::new(Vec2d::new(5.0, 0.0), Vec2d::new(5.0, 10.0));
        let result = find_closest_points_seg2d_seg2d(&seg1, &seg2);
        assert!(result.is_some());
        let (p1, p2, t1, t2) = result.unwrap();
        assert!(approx_eq(p1.x, 5.0));
        assert!(approx_eq(p1.y, 5.0));
        assert!(approx_eq(p2.x, 5.0));
        assert!(approx_eq(p2.y, 5.0));
        assert!(approx_eq(t1, 0.5));
        assert!(approx_eq(t2, 0.5));
    }

    #[test]
    fn test_closest_points_segs2d_parallel() {
        // Parallel — None.
        let seg1 = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(10.0, 0.0));
        let seg2 = LineSeg2d::new(Vec2d::new(0.0, 1.0), Vec2d::new(10.0, 1.0));
        assert!(find_closest_points_seg2d_seg2d(&seg1, &seg2).is_none());
    }

    #[test]
    fn test_closest_points_segs2d_clamped_both() {
        // seg1 from (0,0) to (1,0) along X; seg2 from (10,0) to (10,1) vertical.
        // seg2 underlying line: origin=(10,0), dir=(0,1).
        // Underlying lines are perpendicular — NOT parallel, so they intersect.
        // Solve for infinite lines: intersection at (10,0).
        // Unclamped t1_line on seg1's line = 10.0 >> length(1.0), clamp t1=1.0 => p1=(1,0).
        // Unclamped t2_line on seg2's line = 0.0 / length(1.0) = 0.0, t2=0.0 => p2=(10,0).
        let seg1 = LineSeg2d::new(Vec2d::new(0.0, 0.0), Vec2d::new(1.0, 0.0));
        let seg2 = LineSeg2d::new(Vec2d::new(10.0, 0.0), Vec2d::new(10.0, 1.0));
        let result = find_closest_points_seg2d_seg2d(&seg1, &seg2);
        assert!(result.is_some(), "expected Some but got None");
        let (p1, p2, t1, t2) = result.unwrap();
        assert!(approx_eq(t1, 1.0), "t1={}", t1);
        assert!(approx_eq(t2, 0.0), "t2={}", t2);
        assert!(approx_eq(p1.x, 1.0), "p1.x={}", p1.x);
        assert!(approx_eq(p2.x, 10.0), "p2.x={}", p2.x);
    }
}
