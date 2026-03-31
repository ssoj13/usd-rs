//! Line and line segment types for 3D geometry.
//!
//! - `Line` - Infinite 3D line defined by point + normalized direction
//! - `LineSeg` - Finite 3D line segment defined by two endpoints
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Line, LineSeg, Vec3d};
//!
//! // Infinite line along X axis
//! let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
//!
//! // Line segment from (0,0,0) to (10,0,0)
//! let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
//! assert!((seg.length() - 10.0).abs() < 1e-10);
//! ```

use crate::ray::Ray;
use crate::vec3::Vec3d;
use std::fmt;

/// An infinite 3D line.
///
/// Defined parametrically as `p(t) = origin + t * direction` for t in (-inf, inf).
/// The direction is normalized to unit length.
///
/// # Examples
///
/// ```
/// use usd_gf::{Line, Vec3d};
///
/// let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(2.0, 0.0, 0.0));
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
pub struct Line {
    /// Point on the line.
    origin: Vec3d,
    /// Normalized direction vector.
    direction: Vec3d,
}

impl Line {
    /// Creates a new line from a point and direction.
    ///
    /// The direction is normalized. Returns the original direction length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Line, Vec3d};
    ///
    /// let line = Line::new(Vec3d::new(1.0, 2.0, 3.0), Vec3d::new(0.0, 2.0, 0.0));
    /// assert!((line.direction().y - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn new(origin: Vec3d, direction: Vec3d) -> Self {
        let normalized = direction.normalized();
        Self {
            origin,
            direction: normalized,
        }
    }

    /// Creates a line and returns it along with the original direction length.
    #[must_use]
    pub fn new_with_length(origin: Vec3d, direction: Vec3d) -> (Self, f64) {
        let len = direction.length();
        let normalized = if len > f64::EPSILON {
            Vec3d::new(direction.x / len, direction.y / len, direction.z / len)
        } else {
            Vec3d::new(1.0, 0.0, 0.0)
        };
        (
            Self {
                origin,
                direction: normalized,
            },
            len,
        )
    }

    /// Returns the origin point.
    #[inline]
    #[must_use]
    pub fn origin(&self) -> &Vec3d {
        &self.origin
    }

    /// Returns the normalized direction.
    #[inline]
    #[must_use]
    pub fn direction(&self) -> &Vec3d {
        &self.direction
    }

    /// Sets the line from point and direction.
    ///
    /// Returns the original direction length.
    pub fn set(&mut self, origin: Vec3d, direction: Vec3d) -> f64 {
        let len = direction.length();
        self.origin = origin;
        if len > f64::EPSILON {
            self.direction = Vec3d::new(direction.x / len, direction.y / len, direction.z / len);
        }
        len
    }

    /// Returns the point at parametric distance t.
    ///
    /// Since direction is normalized, t is the actual distance from origin.
    #[inline]
    #[must_use]
    pub fn point(&self, t: f64) -> Vec3d {
        Vec3d::new(
            self.origin.x + t * self.direction.x,
            self.origin.y + t * self.direction.y,
            self.origin.z + t * self.direction.z,
        )
    }

    /// Finds the closest point on the line to the given point.
    ///
    /// Returns the closest point and the parametric distance.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Line, Vec3d};
    ///
    /// let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
    /// let (closest, t) = line.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));
    ///
    /// assert!((closest.x - 5.0).abs() < 1e-10);
    /// assert!((closest.y - 0.0).abs() < 1e-10);
    /// assert!((t - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn find_closest_point(&self, point: &Vec3d) -> (Vec3d, f64) {
        let v = Vec3d::new(
            point.x - self.origin.x,
            point.y - self.origin.y,
            point.z - self.origin.z,
        );

        // Direction is already normalized, so dot with itself is 1
        // t = (point - origin) · direction
        let t = v.x * self.direction.x + v.y * self.direction.y + v.z * self.direction.z;

        (self.point(t), t)
    }
}

impl Default for Line {
    fn default() -> Self {
        Self {
            origin: Vec3d::new(0.0, 0.0, 0.0),
            direction: Vec3d::new(1.0, 0.0, 0.0),
        }
    }
}

impl PartialEq for Line {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin && self.direction == other.direction
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}, {}) -> ({}, {}, {})]",
            self.origin.x,
            self.origin.y,
            self.origin.z,
            self.direction.x,
            self.direction.y,
            self.direction.z
        )
    }
}

/// A 3D line segment.
///
/// Defined by two endpoints. Internally stored as a Line plus length.
/// Parametric form: `p(t) = p0 + t * (p1 - p0)` for t in [0, 1].
///
/// # Examples
///
/// ```
/// use usd_gf::{LineSeg, Vec3d};
///
/// let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
///
/// assert!((seg.length() - 10.0).abs() < 1e-10);
///
/// // Midpoint at t=0.5
/// let mid = seg.point(0.5);
/// assert!((mid.x - 5.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct LineSeg {
    /// The underlying infinite line.
    line: Line,
    /// Length of the segment.
    length: f64,
}

impl LineSeg {
    /// Creates a line segment from two endpoints.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{LineSeg, Vec3d};
    ///
    /// let seg = LineSeg::new(
    ///     Vec3d::new(0.0, 0.0, 0.0),
    ///     Vec3d::new(3.0, 4.0, 0.0)
    /// );
    /// assert!((seg.length() - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn new(p0: Vec3d, p1: Vec3d) -> Self {
        let direction = p1 - p0;
        let (line, length) = Line::new_with_length(p0, direction);
        Self { line, length }
    }

    /// Returns the start point (p0).
    #[inline]
    #[must_use]
    pub fn start(&self) -> &Vec3d {
        self.line.origin()
    }

    /// Returns the end point (p1).
    #[inline]
    #[must_use]
    pub fn end(&self) -> Vec3d {
        self.line.point(self.length)
    }

    /// Returns the normalized direction.
    #[inline]
    #[must_use]
    pub fn direction(&self) -> &Vec3d {
        self.line.direction()
    }

    /// Returns the length of the segment.
    #[inline]
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Returns the point at parametric distance t in [0, 1].
    ///
    /// t=0 returns p0, t=1 returns p1.
    #[inline]
    #[must_use]
    pub fn point(&self, t: f64) -> Vec3d {
        self.line.point(t * self.length)
    }

    /// Finds the closest point on the segment to the given point.
    ///
    /// Returns the closest point and the parametric distance t in [0, 1].
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{LineSeg, Vec3d};
    ///
    /// let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
    ///
    /// // Point projects onto segment
    /// let (closest, t) = seg.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));
    /// assert!((closest.x - 5.0).abs() < 1e-10);
    /// assert!((t - 0.5).abs() < 1e-10);
    ///
    /// // Point beyond segment end
    /// let (closest2, t2) = seg.find_closest_point(&Vec3d::new(15.0, 0.0, 0.0));
    /// assert!((closest2.x - 10.0).abs() < 1e-10);
    /// assert!((t2 - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn find_closest_point(&self, point: &Vec3d) -> (Vec3d, f64) {
        let (_, t_line) = self.line.find_closest_point(point);

        // Clamp to segment bounds
        let t_clamped = if self.length > f64::EPSILON {
            (t_line / self.length).clamp(0.0, 1.0)
        } else {
            0.0
        };

        (self.point(t_clamped), t_clamped)
    }
}

impl Default for LineSeg {
    fn default() -> Self {
        Self {
            line: Line::default(),
            length: 1.0,
        }
    }
}

impl PartialEq for LineSeg {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line && self.length == other.length
    }
}

impl fmt::Display for LineSeg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let end = self.end();
        write!(
            f,
            "[({}, {}, {}) - ({}, {}, {})]",
            self.line.origin().x,
            self.line.origin().y,
            self.line.origin().z,
            end.x,
            end.y,
            end.z
        )
    }
}

/// Finds the closest points between a ray and an infinite line.
///
/// Returns None if they are nearly parallel.
/// Otherwise returns ((ray_point, ray_t), (line_point, line_t)).
/// Ray's parametric distance ray_t is in direction-length units (Ray::point(ray_t)).
#[must_use]
pub fn find_closest_points_ray_line(
    ray: &Ray,
    line: &Line,
) -> Option<((Vec3d, f64), (Vec3d, f64))> {
    let len = ray.direction().length();
    if len < f64::EPSILON {
        return None;
    }
    let ray_line = Line::new(*ray.start_point(), *ray.direction());

    if let Some(((_, rd), (lp, ld))) = find_closest_points_line_line(&ray_line, line) {
        // Clamp ray's parametric distance to >= 0 (ray only extends forward)
        let rd_clamped = rd.max(0.0);
        // Ray's t is in direction-length units: ray.point(t) = start + t * direction
        // Line uses normalized dir, so line_t is in world units. ray_t = line_t / len.
        let ray_t = rd_clamped / len;
        let ray_point = ray.point(ray_t);
        Some(((ray_point, ray_t), (lp, ld)))
    } else {
        None
    }
}

/// Finds the closest points between a ray and a line segment.
///
/// Returns None if they are nearly parallel.
/// Otherwise returns ((ray_point, ray_t), (seg_point, seg_t)) where seg_t in [0, 1].
#[must_use]
pub fn find_closest_points_ray_line_seg(
    ray: &Ray,
    seg: &LineSeg,
) -> Option<((Vec3d, f64), (Vec3d, f64))> {
    let len = ray.direction().length();
    if len < f64::EPSILON {
        return None;
    }
    let ray_line = Line::new(*ray.start_point(), *ray.direction());

    if let Some(((_, rd), (seg_point, t2_seg))) = find_closest_points_line_seg(&ray_line, seg) {
        let rd_clamped = rd.max(0.0);
        let ray_t = rd_clamped / len;
        let ray_point = ray.point(ray_t);

        Some(((ray_point, ray_t), (seg_point, t2_seg)))
    } else {
        None
    }
}

/// Finds the closest points between two infinite 3D lines.
///
/// Returns `None` if the lines are nearly parallel (denominator < 1e-6).
/// Otherwise returns `((point1, t1), (point2, t2))` where `t1`, `t2` are
/// the parametric distances along each line's direction.
///
/// Matches C++ `GfFindClosestPoints(GfLine, GfLine, ...)`.
#[must_use]
pub fn find_closest_points_line_line(l1: &Line, l2: &Line) -> Option<((Vec3d, f64), (Vec3d, f64))> {
    let p1 = l1.origin();
    let d1 = l1.direction();
    let p2 = l2.origin();
    let d2 = l2.direction();

    // Build the linear system (from C++ reference):
    //   t2 * a - t1 * b = c
    //   t2 * d - t1 * e = f
    // where e = a (d2·d1 == d1·d2)
    let a = d1.x * d2.x + d1.y * d2.y + d1.z * d2.z; // d1 . d2
    let b = d1.x * d1.x + d1.y * d1.y + d1.z * d1.z; // d1 . d1
    let c = (d1.x * p1.x + d1.y * p1.y + d1.z * p1.z) - (d1.x * p2.x + d1.y * p2.y + d1.z * p2.z); // d1.p1 - d1.p2
    let d = d2.x * d2.x + d2.y * d2.y + d2.z * d2.z; // d2 . d2
    let e = a; // d2 . d1 == a
    let f = (d2.x * p1.x + d2.y * p1.y + d2.z * p1.z) - (d2.x * p2.x + d2.y * p2.y + d2.z * p2.z); // d2.p1 - d2.p2

    let denom = a * e - b * d;

    // Denominator == 0 means lines are parallel; no unique solution.
    if denom.abs() < 1e-6 {
        return None;
    }

    let t1 = (c * d - a * f) / denom;
    let t2 = (c * e - b * f) / denom;

    Some(((l1.point(t1), t1), (l2.point(t2), t2)))
}

/// Finds the closest points between an infinite 3D line and a 3D line segment.
///
/// The segment parameter `t2` is clamped to `[0, 1]`. If clamping occurs,
/// the line's closest point is recomputed against the clamped segment endpoint.
///
/// Returns `None` if line and segment are nearly parallel.
/// Returns `((line_point, t1), (seg_point, t2))` where `t2` is in `[0, 1]`.
///
/// Matches C++ `GfFindClosestPoints(GfLine, GfLineSeg, ...)`.
#[must_use]
pub fn find_closest_points_line_seg(
    line: &Line,
    seg: &LineSeg,
) -> Option<((Vec3d, f64), (Vec3d, f64))> {
    // Treat the segment's underlying line as infinite to get unclamped parameters.
    // seg._line in C++ stores the line with the same origin and normalized direction.
    let seg_line = Line {
        origin: *seg.start(),
        direction: *seg.direction(),
    };

    let ((cp1, mut lt1), (_, lt2)) = find_closest_points_line_line(line, &seg_line)?;

    // Clamp segment parameter from line-units to [0, 1].
    let t2 = if seg.length > f64::EPSILON {
        (lt2 / seg.length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut cp2 = seg.point(t2);

    // If clamping occurred, recompute the line's closest point to the clamped endpoint.
    let mut line_point = cp1;
    if t2 <= 0.0 || t2 >= 1.0 {
        let (new_cp1, new_lt1) = line.find_closest_point(&cp2);
        line_point = new_cp1;
        lt1 = new_lt1;
        // cp2 is already the clamped endpoint — assign for clarity
        cp2 = seg.point(t2);
    }

    Some(((line_point, lt1), (cp2, t2)))
}

/// Finds the closest points between two 3D line segments.
///
/// Both segment parameters are clamped independently to `[0, 1]`.
///
/// Returns `None` if segments are nearly parallel.
/// Returns `((seg1_point, t1), (seg2_point, t2))` where `t1`, `t2` are in `[0, 1]`.
///
/// Matches C++ `GfFindClosestPoints(GfLineSeg, GfLineSeg, ...)`.
#[must_use]
pub fn find_closest_points_seg_seg(
    seg1: &LineSeg,
    seg2: &LineSeg,
) -> Option<((Vec3d, f64), (Vec3d, f64))> {
    // Build underlying infinite lines for each segment.
    let line1 = Line {
        origin: *seg1.start(),
        direction: *seg1.direction(),
    };
    let line2 = Line {
        origin: *seg2.start(),
        direction: *seg2.direction(),
    };

    let ((_, lt1), (_, lt2)) = find_closest_points_line_line(&line1, &line2)?;

    // Clamp both parameters to [0, 1] independently, matching C++ behavior.
    let t1 = if seg1.length > f64::EPSILON {
        (lt1 / seg1.length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t2 = if seg2.length > f64::EPSILON {
        (lt2 / seg2.length).clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(((seg1.point(t1), t1), (seg2.point(t2), t2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Line tests
    #[test]
    fn test_line_new() {
        let line = Line::new(Vec3d::new(1.0, 2.0, 3.0), Vec3d::new(0.0, 2.0, 0.0));
        assert!((line.direction().y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_point() {
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let p = line.point(5.0);
        assert!((p.x - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_find_closest_point() {
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let (closest, t) = line.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));

        assert!((closest.x - 5.0).abs() < 1e-10);
        assert!((closest.y - 0.0).abs() < 1e-10);
        assert!((t - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_find_closest_point_negative_t() {
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let (closest, t) = line.find_closest_point(&Vec3d::new(-5.0, 0.0, 0.0));

        assert!((closest.x - (-5.0)).abs() < 1e-10);
        assert!((t - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_line_default() {
        let line: Line = Default::default();
        assert!((line.direction().x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_display() {
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let s = format!("{}", line);
        assert!(s.contains("->"));
    }

    // LineSeg tests
    #[test]
    fn test_line_seg_new() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        assert!((seg.length() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_start_end() {
        let seg = LineSeg::new(Vec3d::new(1.0, 2.0, 3.0), Vec3d::new(4.0, 5.0, 6.0));
        assert!((seg.start().x - 1.0).abs() < 1e-10);
        assert!((seg.end().x - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_point() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));

        let start = seg.point(0.0);
        assert!((start.x - 0.0).abs() < 1e-10);

        let mid = seg.point(0.5);
        assert!((mid.x - 5.0).abs() < 1e-10);

        let end = seg.point(1.0);
        assert!((end.x - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_find_closest_point_inside() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        let (closest, t) = seg.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));

        assert!((closest.x - 5.0).abs() < 1e-10);
        assert!((t - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_find_closest_point_before() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        let (closest, t) = seg.find_closest_point(&Vec3d::new(-5.0, 0.0, 0.0));

        assert!((closest.x - 0.0).abs() < 1e-10);
        assert!((t - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_find_closest_point_after() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        let (closest, t) = seg.find_closest_point(&Vec3d::new(15.0, 0.0, 0.0));

        assert!((closest.x - 10.0).abs() < 1e-10);
        assert!((t - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_default() {
        let seg: LineSeg = Default::default();
        assert!((seg.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_seg_display() {
        let seg = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let s = format!("{}", seg);
        assert!(s.contains("-"));
    }

    // ===== GfFindClosestPoints overload tests =====

    const EPS: f64 = 1e-9;

    // Overload 1: Line vs Line
    #[test]
    fn test_find_closest_points_line_line() {
        // X-axis line and a vertical line offset in Z: closest points at (5,0,0) and (5,0,1).
        let l1 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l2 = Line::new(Vec3d::new(5.0, 0.0, 1.0), Vec3d::new(0.0, 1.0, 0.0));

        let result = find_closest_points_line_line(&l1, &l2);
        assert!(result.is_some());
        let ((p1, t1), (p2, t2)) = result.unwrap();
        assert!((p1.x - 5.0).abs() < EPS, "p1.x={}", p1.x);
        assert!((p1.y - 0.0).abs() < EPS);
        assert!((p1.z - 0.0).abs() < EPS);
        assert!((t1 - 5.0).abs() < EPS, "t1={}", t1);
        assert!((p2.x - 5.0).abs() < EPS);
        assert!((p2.z - 1.0).abs() < EPS, "p2.z={}", p2.z);
        assert!((t2 - 0.0).abs() < EPS, "t2={}", t2);
    }

    #[test]
    fn test_find_closest_points_line_line_skew() {
        // Two skew lines: l1 along X at y=0,z=0; l2 along Y at x=3,z=2.
        // Closest: (3,0,0) on l1, (3,0,2) on l2.
        let l1 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l2 = Line::new(Vec3d::new(3.0, 0.0, 2.0), Vec3d::new(0.0, 1.0, 0.0));
        let result = find_closest_points_line_line(&l1, &l2);
        assert!(result.is_some());
        let ((p1, _t1), (p2, _t2)) = result.unwrap();
        assert!((p1.x - 3.0).abs() < EPS);
        assert!((p1.z - 0.0).abs() < EPS);
        assert!((p2.x - 3.0).abs() < EPS);
        assert!((p2.z - 2.0).abs() < EPS);
    }

    #[test]
    fn test_find_closest_points_line_line_parallel() {
        // Parallel lines return None.
        let l1 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l2 = Line::new(Vec3d::new(0.0, 1.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        assert!(find_closest_points_line_line(&l1, &l2).is_none());
    }

    #[test]
    fn test_find_closest_points_line_line_intersecting() {
        // Intersecting lines: closest points are the same intersection point.
        let l1 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l2 = Line::new(Vec3d::new(3.0, 0.0, 0.0), Vec3d::new(0.0, 1.0, 0.0));
        let result = find_closest_points_line_line(&l1, &l2);
        assert!(result.is_some());
        let ((p1, _), (p2, _)) = result.unwrap();
        assert!((p1.x - p2.x).abs() < EPS);
        assert!((p1.y - p2.y).abs() < EPS);
        assert!((p1.z - p2.z).abs() < EPS);
    }

    // Overload 2: Line vs LineSeg
    #[test]
    fn test_find_closest_points_line_seg_interior() {
        // Line along X; segment from (5,-1,1) to (5,1,1) — closest points at (5,0,0) and (5,0,1).
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let seg = LineSeg::new(Vec3d::new(5.0, -1.0, 1.0), Vec3d::new(5.0, 1.0, 1.0));
        let result = find_closest_points_line_seg(&line, &seg);
        assert!(result.is_some());
        let ((p1, t1), (p2, t2)) = result.unwrap();
        assert!((p1.x - 5.0).abs() < EPS, "p1.x={}", p1.x);
        assert!((p1.z - 0.0).abs() < EPS);
        assert!((t1 - 5.0).abs() < EPS, "t1={}", t1);
        assert!((p2.x - 5.0).abs() < EPS);
        assert!((p2.z - 1.0).abs() < EPS);
        assert!((t2 - 0.5).abs() < EPS, "t2={}", t2); // midpoint of segment
    }

    #[test]
    fn test_find_closest_points_line_seg_clamped_start() {
        // Segment starts past the perpendicular — t2 should clamp to 0.0.
        // Line along X; segment from (5,2,1) to (5,10,1).
        // Unclamped t2_line would be negative (below start), clamp to 0 => seg start (5,2,1).
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let seg = LineSeg::new(Vec3d::new(5.0, 2.0, 1.0), Vec3d::new(5.0, 10.0, 1.0));
        let result = find_closest_points_line_seg(&line, &seg);
        assert!(result.is_some());
        let ((p1, _t1), (p2, t2)) = result.unwrap();
        assert!((t2 - 0.0).abs() < EPS, "t2 should be 0.0, got {}", t2);
        // p2 is segment start
        assert!((p2.x - 5.0).abs() < EPS);
        assert!((p2.y - 2.0).abs() < EPS);
        assert!((p2.z - 1.0).abs() < EPS);
        // p1 is the closest point on line to the clamped p2
        assert!((p1.x - 5.0).abs() < EPS);
        assert!((p1.y - 0.0).abs() < EPS);
        assert!((p1.z - 0.0).abs() < EPS);
    }

    #[test]
    fn test_find_closest_points_line_seg_parallel() {
        // Parallel line and segment — None.
        let line = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let seg = LineSeg::new(Vec3d::new(0.0, 1.0, 0.0), Vec3d::new(10.0, 1.0, 0.0));
        assert!(find_closest_points_line_seg(&line, &seg).is_none());
    }

    // Overload 3: LineSeg vs LineSeg
    #[test]
    fn test_find_closest_points_seg_seg() {
        // seg1 along X [0,10]; seg2 vertical at x=5, z=1 [y=0..10].
        // Closest interior points: (5,0,0) and (5,0,1).
        let seg1 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        let seg2 = LineSeg::new(Vec3d::new(5.0, 0.0, 1.0), Vec3d::new(5.0, 10.0, 1.0));

        let result = find_closest_points_seg_seg(&seg1, &seg2);
        assert!(result.is_some());
        let ((p1, t1), (p2, t2)) = result.unwrap();
        assert!((p1.x - 5.0).abs() < EPS, "p1.x={}", p1.x);
        assert!((p2.x - 5.0).abs() < EPS, "p2.x={}", p2.x);
        assert!((p2.z - 1.0).abs() < EPS, "p2.z={}", p2.z);
        assert!((t1 - 0.5).abs() < EPS, "t1={}", t1);
        assert!((t2 - 0.0).abs() < EPS, "t2={}", t2);
    }

    #[test]
    fn test_find_closest_points_seg_seg_clamped() {
        // seg1 along X [0,1]; seg2 along Y at x=10,z=1 [0,1].
        // Underlying lines are perpendicular — NOT parallel.
        // Infinite intersection: t1_line=10 >> length(1), t2_line=0.
        // Clamped: t1=1 => p1=(1,0,0), t2=0 => p2=(10,0,1).
        let seg1 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let seg2 = LineSeg::new(Vec3d::new(10.0, 0.0, 1.0), Vec3d::new(10.0, 1.0, 1.0));
        let result = find_closest_points_seg_seg(&seg1, &seg2);
        assert!(result.is_some(), "expected Some but got None");
        let ((p1, t1), (p2, t2)) = result.unwrap();
        assert!((t1 - 1.0).abs() < EPS, "t1={}", t1);
        assert!((t2 - 0.0).abs() < EPS, "t2={}", t2);
        assert!((p1.x - 1.0).abs() < EPS, "p1.x={}", p1.x);
        assert!((p2.x - 10.0).abs() < EPS, "p2.x={}", p2.x);
    }

    #[test]
    fn test_find_closest_points_seg_seg_parallel() {
        // Parallel segments — None.
        let seg1 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        let seg2 = LineSeg::new(Vec3d::new(0.0, 1.0, 0.0), Vec3d::new(10.0, 1.0, 0.0));
        assert!(find_closest_points_seg_seg(&seg1, &seg2).is_none());
    }

    #[test]
    fn test_line_equality() {
        let l1 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l2 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let l3 = Line::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(0.0, 1.0, 0.0));

        assert!(l1 == l2);
        assert!(l1 != l3);
    }

    #[test]
    fn test_line_seg_equality() {
        let s1 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let s2 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let s3 = LineSeg::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(2.0, 0.0, 0.0));

        assert!(s1 == s2);
        assert!(s1 != s3);
    }
}
