//! Ray type for intersection testing.
//!
//! A ray consists of an origin point and a direction vector.
//! The ray extends infinitely in the positive direction from the origin.
//!
//! Note: By default, the direction vector is NOT normalized to unit length.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Ray, Vec3d};
//!
//! let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
//!
//! // Get point at distance 5 along the ray
//! let p = ray.point(5.0);
//! assert!((p.x - 5.0).abs() < 1e-10);
//! ```

use crate::bbox3d::BBox3d;
use crate::matrix4::Matrix4d;
use crate::plane::Plane;
use crate::range::Range3d;
use crate::vec3::{Vec3d, cross};
use std::fmt;

/// A 3-dimensional ray for intersection testing.
///
/// Consists of an origin (start point) and a direction.
/// Points on the ray: `origin + t * direction` for t >= 0.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Ray {
    /// Starting point of the ray.
    start_point: Vec3d,
    /// Direction vector (not necessarily normalized).
    direction: Vec3d,
}

impl Ray {
    /// Creates a new ray with given origin and direction.
    ///
    /// The direction is NOT normalized.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Vec3d};
    ///
    /// let ray = Ray::new(
    ///     Vec3d::new(0.0, 0.0, 0.0),
    ///     Vec3d::new(0.0, 1.0, 0.0)
    /// );
    /// assert_eq!(ray.direction().y, 1.0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(start_point: Vec3d, direction: Vec3d) -> Self {
        Self {
            start_point,
            direction,
        }
    }

    /// Creates a ray from start point to end point.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Vec3d};
    ///
    /// let ray = Ray::from_endpoints(
    ///     Vec3d::new(0.0, 0.0, 0.0),
    ///     Vec3d::new(10.0, 0.0, 0.0)
    /// );
    /// assert!((ray.direction().x - 10.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_endpoints(start: Vec3d, end: Vec3d) -> Self {
        Self {
            start_point: start,
            direction: end - start,
        }
    }

    /// Returns the starting point of the ray.
    #[inline]
    #[must_use]
    pub fn start_point(&self) -> &Vec3d {
        &self.start_point
    }

    /// Returns the direction vector (not necessarily unit length).
    #[inline]
    #[must_use]
    pub fn direction(&self) -> &Vec3d {
        &self.direction
    }

    /// Sets the start point and direction.
    #[inline]
    pub fn set(&mut self, start_point: Vec3d, direction: Vec3d) {
        self.start_point = start_point;
        self.direction = direction;
    }

    /// Sets endpoints (computes direction from end - start).
    #[inline]
    pub fn set_endpoints(&mut self, start: Vec3d, end: Vec3d) {
        self.start_point = start;
        self.direction = end - start;
    }

    /// Returns the point at parametric distance t along the ray.
    ///
    /// point(t) = start_point + t * direction
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Vec3d};
    ///
    /// let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(2.0, 0.0, 0.0));
    /// let p = ray.point(3.0);
    /// assert!((p.x - 6.0).abs() < 1e-10);
    /// ```
    #[inline]
    #[must_use]
    pub fn point(&self, distance: f64) -> Vec3d {
        Vec3d::new(
            self.start_point.x + distance * self.direction.x,
            self.start_point.y + distance * self.direction.y,
            self.start_point.z + distance * self.direction.z,
        )
    }

    /// Transforms the ray by the given matrix.
    ///
    /// The origin is transformed as a point, direction as a vector.
    pub fn transform(&mut self, matrix: &Matrix4d) {
        self.start_point = matrix.transform_point(&self.start_point);
        self.direction = matrix.transform_dir(&self.direction);
    }

    /// Returns a transformed copy of the ray.
    #[must_use]
    pub fn transformed(&self, matrix: &Matrix4d) -> Self {
        let mut result = *self;
        result.transform(matrix);
        result
    }

    /// Finds the closest point on the ray to the given point.
    ///
    /// Returns the closest point and optionally the parametric distance.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Vec3d};
    ///
    /// let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
    /// let (closest, dist) = ray.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));
    ///
    /// assert!((closest.x - 5.0).abs() < 1e-10);
    /// assert!((closest.y - 0.0).abs() < 1e-10);
    /// assert!((dist - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn find_closest_point(&self, point: &Vec3d) -> (Vec3d, f64) {
        let v = Vec3d::new(
            point.x - self.start_point.x,
            point.y - self.start_point.y,
            point.z - self.start_point.z,
        );

        let d_dot_d = self.direction.x * self.direction.x
            + self.direction.y * self.direction.y
            + self.direction.z * self.direction.z;

        if d_dot_d < f64::EPSILON {
            return (self.start_point, 0.0);
        }

        let d_dot_v = self.direction.x * v.x + self.direction.y * v.y + self.direction.z * v.z;

        // Clamp to ray (t >= 0)
        let t = (d_dot_v / d_dot_d).max(0.0);

        (self.point(t), t)
    }

    /// Intersects the ray with a plane.
    ///
    /// Returns Some((distance, front_facing)) if intersection exists.
    /// `front_facing` is true if ray hits the front of the plane (same side as normal).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Plane, Vec3d};
    ///
    /// let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(0.0, 0.0, 1.0));
    /// let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);
    ///
    /// if let Some((dist, front)) = ray.intersect_plane(&plane) {
    ///     assert!((dist - 5.0).abs() < 1e-10);
    ///     assert!(!front); // Hit back of plane
    /// }
    /// ```
    #[must_use]
    pub fn intersect_plane(&self, plane: &Plane) -> Option<(f64, bool)> {
        let normal = plane.normal();

        // d · n
        let d_dot_n =
            self.direction.x * normal.x + self.direction.y * normal.y + self.direction.z * normal.z;

        // Check if ray is parallel to plane
        if d_dot_n.abs() < f64::EPSILON {
            return None;
        }

        // t = (distance - start · n) / (d · n)
        let start_dot_n = self.start_point.x * normal.x
            + self.start_point.y * normal.y
            + self.start_point.z * normal.z;

        let t = (plane.distance_from_origin() - start_dot_n) / d_dot_n;

        // Ray only goes forward
        if t < 0.0 {
            return None;
        }

        let front_facing = d_dot_n < 0.0;
        Some((t, front_facing))
    }

    /// Intersects the ray with an axis-aligned box.
    ///
    /// Returns Some((enter_distance, exit_distance)) if intersection exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Ray, Range3d, Vec3d};
    ///
    /// let ray = Ray::new(Vec3d::new(-5.0, 0.5, 0.5), Vec3d::new(1.0, 0.0, 0.0));
    /// let bbox = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 1.0));
    ///
    /// if let Some((enter, exit)) = ray.intersect_range(&bbox) {
    ///     assert!((enter - 5.0).abs() < 1e-10);
    ///     assert!((exit - 6.0).abs() < 1e-10);
    /// }
    /// ```
    #[must_use]
    pub fn intersect_range(&self, bbox: &Range3d) -> Option<(f64, f64)> {
        if bbox.is_empty() {
            return None;
        }

        let min = bbox.min();
        let max = bbox.max();
        if !min.x.is_finite()
            || !min.y.is_finite()
            || !min.z.is_finite()
            || !max.x.is_finite()
            || !max.y.is_finite()
            || !max.z.is_finite()
        {
            return None;
        }

        let mut t_min = f64::NEG_INFINITY;
        let mut t_max = f64::INFINITY;

        // X slab
        if self.direction.x.abs() < f64::EPSILON {
            if self.start_point.x < min.x || self.start_point.x > max.x {
                return None;
            }
        } else {
            let inv_d = 1.0 / self.direction.x;
            let mut t1 = (min.x - self.start_point.x) * inv_d;
            let mut t2 = (max.x - self.start_point.x) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }

        // Y slab
        if self.direction.y.abs() < f64::EPSILON {
            if self.start_point.y < min.y || self.start_point.y > max.y {
                return None;
            }
        } else {
            let inv_d = 1.0 / self.direction.y;
            let mut t1 = (min.y - self.start_point.y) * inv_d;
            let mut t2 = (max.y - self.start_point.y) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }

        // Z slab
        if self.direction.z.abs() < f64::EPSILON {
            if self.start_point.z < min.z || self.start_point.z > max.z {
                return None;
            }
        } else {
            let inv_d = 1.0 / self.direction.z;
            let mut t1 = (min.z - self.start_point.z) * inv_d;
            let mut t2 = (max.z - self.start_point.z) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }

        Some((t_min, t_max))
    }

    /// Intersects ray with an oriented bounding box (BBox3d).
    ///
    /// Transforms the ray to the bbox's local space and intersects with the axis-aligned range.
    /// Returns Some((enter_distance, exit_distance)) if intersection exists.
    #[must_use]
    pub fn intersect_bbox(&self, bbox: &BBox3d) -> Option<(f64, f64)> {
        let mut local_ray = *self;
        local_ray.transform(bbox.inverse_matrix());
        local_ray.intersect_range(bbox.range())
    }

    /// Intersects ray with an infinite cylinder.
    ///
    /// Cylinder axis passes through `origin` with direction `axis`, radius `radius`.
    /// Returns Some((enter_distance, exit_distance)) if intersection exists.
    #[must_use]
    pub fn intersect_cylinder(
        &self,
        origin: &Vec3d,
        axis: &Vec3d,
        radius: f64,
    ) -> Option<(f64, f64)> {
        let unit_axis = axis.normalized();
        let delta = *self.start_point() - *origin;

        // Project direction and delta onto plane perpendicular to axis
        let d_dot_a = self.direction().x * unit_axis.x
            + self.direction().y * unit_axis.y
            + self.direction().z * unit_axis.z;
        let u = Vec3d::new(
            self.direction().x - d_dot_a * unit_axis.x,
            self.direction().y - d_dot_a * unit_axis.y,
            self.direction().z - d_dot_a * unit_axis.z,
        );

        let delta_dot_a = delta.x * unit_axis.x + delta.y * unit_axis.y + delta.z * unit_axis.z;
        let v = Vec3d::new(
            delta.x - delta_dot_a * unit_axis.x,
            delta.y - delta_dot_a * unit_axis.y,
            delta.z - delta_dot_a * unit_axis.z,
        );

        let a = u.x * u.x + u.y * u.y + u.z * u.z;
        let b = 2.0 * (u.x * v.x + u.y * v.y + u.z * v.z);
        let c = v.x * v.x + v.y * v.y + v.z * v.z - radius * radius;

        self.solve_quadratic(a, b, c)
    }

    /// Intersects ray with a finite cone.
    ///
    /// Cone has base at `origin` with radius `radius`, apex at `origin + height * axis`.
    /// Returns Some((enter_distance, exit_distance)) if intersection exists.
    #[must_use]
    pub fn intersect_cone(
        &self,
        origin: &Vec3d,
        axis: &Vec3d,
        radius: f64,
        height: f64,
    ) -> Option<(f64, f64)> {
        let unit_axis = axis.normalized();
        let apex = *origin + unit_axis * height;

        let delta = *self.start_point() - apex;

        let d_dot_a = self.direction().x * unit_axis.x
            + self.direction().y * unit_axis.y
            + self.direction().z * unit_axis.z;
        let u = Vec3d::new(
            self.direction().x - d_dot_a * unit_axis.x,
            self.direction().y - d_dot_a * unit_axis.y,
            self.direction().z - d_dot_a * unit_axis.z,
        );

        let delta_dot_a = delta.x * unit_axis.x + delta.y * unit_axis.y + delta.z * unit_axis.z;
        let v = Vec3d::new(
            delta.x - delta_dot_a * unit_axis.x,
            delta.y - delta_dot_a * unit_axis.y,
            delta.z - delta_dot_a * unit_axis.z,
        );

        let cos2 = (height * height) / (height * height + radius * radius);
        let sin2 = 1.0 - cos2;

        let a = cos2 * (u.x * u.x + u.y * u.y + u.z * u.z) - sin2 * d_dot_a * d_dot_a;
        let b = 2.0 * (cos2 * (u.x * v.x + u.y * v.y + u.z * v.z) - sin2 * d_dot_a * delta_dot_a);
        let c = cos2 * (v.x * v.x + v.y * v.y + v.z * v.z) - sin2 * delta_dot_a * delta_dot_a;

        let (mut enter, mut exit) = self.solve_quadratic(a, b, c)?;

        // Filter to single cone: point must be between apex and base (dot(axis, point - apex) <= 0)
        let enter_pt = self.point(enter);
        let exit_pt = self.point(exit);
        let to_enter = Vec3d::new(
            enter_pt.x - apex.x,
            enter_pt.y - apex.y,
            enter_pt.z - apex.z,
        );
        let to_exit = Vec3d::new(exit_pt.x - apex.x, exit_pt.y - apex.y, exit_pt.z - apex.z);
        let enter_valid =
            to_enter.x * unit_axis.x + to_enter.y * unit_axis.y + to_enter.z * unit_axis.z <= 0.0;
        let exit_valid =
            to_exit.x * unit_axis.x + to_exit.y * unit_axis.y + to_exit.z * unit_axis.z <= 0.0;

        if !enter_valid && !exit_valid {
            return None;
        }
        if !enter_valid {
            enter = exit;
        } else if !exit_valid {
            exit = enter;
        }

        Some((enter, exit))
    }

    /// Intersects ray with a sphere.
    ///
    /// Returns Some((enter_distance, exit_distance)) if intersection exists.
    #[must_use]
    pub fn intersect_sphere(&self, center: &Vec3d, radius: f64) -> Option<(f64, f64)> {
        // Vector from ray origin to sphere center
        let oc = Vec3d::new(
            self.start_point.x - center.x,
            self.start_point.y - center.y,
            self.start_point.z - center.z,
        );

        let a = self.direction.x * self.direction.x
            + self.direction.y * self.direction.y
            + self.direction.z * self.direction.z;

        let b = 2.0 * (oc.x * self.direction.x + oc.y * self.direction.y + oc.z * self.direction.z);

        let c = oc.x * oc.x + oc.y * oc.y + oc.z * oc.z - radius * radius;

        self.solve_quadratic(a, b, c)
    }

    /// Intersects ray with a triangle.
    ///
    /// Returns Some((distance, barycentric, front_facing)) if intersection exists.
    /// Barycentric coordinates: intersection = bary.x * p0 + bary.y * p1 + bary.z * p2.
    #[must_use]
    pub fn intersect_triangle(
        &self,
        p0: &Vec3d,
        p1: &Vec3d,
        p2: &Vec3d,
        max_dist: f64,
    ) -> Option<(f64, Vec3d, bool)> {
        // Moller-Trumbore algorithm
        let edge1 = *p1 - *p0;
        let edge2 = *p2 - *p0;

        let h = cross(&self.direction, &edge2);
        let a = edge1.x * h.x + edge1.y * h.y + edge1.z * h.z;

        if a.abs() < f64::EPSILON {
            return None; // Ray parallel to triangle
        }

        let f = 1.0 / a;
        let s = self.start_point - *p0;

        let u = f * (s.x * h.x + s.y * h.y + s.z * h.z);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = cross(&s, &edge1);
        let v = f * (self.direction.x * q.x + self.direction.y * q.y + self.direction.z * q.z);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * (edge2.x * q.x + edge2.y * q.y + edge2.z * q.z);

        if t < 0.0 || t > max_dist {
            return None;
        }

        let w = 1.0 - u - v;
        let barycentric = Vec3d::new(w, u, v);
        let front_facing = a > 0.0;

        Some((t, barycentric, front_facing))
    }

    /// Solves the quadratic equation ax^2 + bx + c = 0.
    ///
    /// Returns Some((t1, t2)) where t1 <= t2. Negative values are valid
    /// (they indicate intersections behind the ray origin).
    /// Returns None only when there is no real solution (discriminant < 0)
    /// or when the quadratic is degenerate (a == 0 and b == 0).
    fn solve_quadratic(&self, a: f64, b: f64, c: f64) -> Option<(f64, f64)> {
        // Degenerate: linear equation bx + c = 0
        if a.abs() < f64::EPSILON {
            if b.abs() < f64::EPSILON {
                // c == 0 means all points are solutions (degenerate)
                return if c.abs() < f64::EPSILON { None } else { None };
            }
            let t = -c / b;
            return Some((t, t));
        }

        let discriminant = b * b - 4.0 * a * c;

        if discriminant < -1e-10 {
            return None;
        }

        let sqrt_d = discriminant.max(0.0).sqrt();
        let inv_2a = 1.0 / (2.0 * a);

        let mut t1 = (-b - sqrt_d) * inv_2a;
        let mut t2 = (-b + sqrt_d) * inv_2a;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        Some((t1, t2))
    }
}

impl Default for Ray {
    fn default() -> Self {
        Self {
            start_point: Vec3d::new(0.0, 0.0, 0.0),
            direction: Vec3d::new(0.0, 0.0, 1.0),
        }
    }
}

impl PartialEq for Ray {
    fn eq(&self, other: &Self) -> bool {
        self.start_point == other.start_point && self.direction == other.direction
    }
}

impl fmt::Display for Ray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}, {}) >> ({}, {}, {})]",
            self.start_point.x,
            self.start_point.y,
            self.start_point.z,
            self.direction.x,
            self.direction.y,
            self.direction.z
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ray = Ray::new(Vec3d::new(1.0, 2.0, 3.0), Vec3d::new(0.0, 1.0, 0.0));
        assert_eq!(ray.start_point().x, 1.0);
        assert_eq!(ray.direction().y, 1.0);
    }

    #[test]
    fn test_from_endpoints() {
        let ray = Ray::from_endpoints(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(10.0, 0.0, 0.0));
        assert!((ray.direction().x - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_point() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(2.0, 0.0, 0.0));
        let p = ray.point(3.0);
        assert!((p.x - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_find_closest_point() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));

        // Point offset from ray
        let (closest, dist) = ray.find_closest_point(&Vec3d::new(5.0, 3.0, 0.0));
        assert!((closest.x - 5.0).abs() < 1e-10);
        assert!((closest.y - 0.0).abs() < 1e-10);
        assert!((dist - 5.0).abs() < 1e-10);

        // Point behind ray origin
        let (closest2, dist2) = ray.find_closest_point(&Vec3d::new(-5.0, 0.0, 0.0));
        assert!((closest2.x - 0.0).abs() < 1e-10);
        assert!((dist2 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_plane() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(0.0, 0.0, 1.0));
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);

        let result = ray.intersect_plane(&plane);
        assert!(result.is_some());
        let (dist, front) = result.unwrap();
        assert!((dist - 5.0).abs() < 1e-10);
        assert!(!front); // Hitting back of plane
    }

    #[test]
    fn test_intersect_plane_parallel() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);

        assert!(ray.intersect_plane(&plane).is_none());
    }

    #[test]
    fn test_intersect_range() {
        let ray = Ray::new(Vec3d::new(-5.0, 0.5, 0.5), Vec3d::new(1.0, 0.0, 0.0));
        let bbox = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 1.0));

        let result = ray.intersect_range(&bbox);
        assert!(result.is_some());
        let (enter, exit) = result.unwrap();
        assert!((enter - 5.0).abs() < 1e-10);
        assert!((exit - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_range_miss() {
        let ray = Ray::new(Vec3d::new(-5.0, 5.0, 5.0), Vec3d::new(1.0, 0.0, 0.0));
        let bbox = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 1.0));

        assert!(ray.intersect_range(&bbox).is_none());
    }

    #[test]
    fn test_intersect_range_empty_returns_none() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let empty = Range3d::empty();
        assert!(ray.intersect_range(&empty).is_none());
    }

    #[test]
    fn test_intersect_sphere() {
        let ray = Ray::new(Vec3d::new(-5.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let center = Vec3d::new(0.0, 0.0, 0.0);
        let radius = 1.0;

        let result = ray.intersect_sphere(&center, radius);
        assert!(result.is_some());
        let (enter, exit) = result.unwrap();
        assert!((enter - 4.0).abs() < 1e-10);
        assert!((exit - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_sphere_miss() {
        let ray = Ray::new(Vec3d::new(-5.0, 5.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let center = Vec3d::new(0.0, 0.0, 0.0);
        let radius = 1.0;

        assert!(ray.intersect_sphere(&center, radius).is_none());
    }

    #[test]
    fn test_intersect_triangle() {
        let ray = Ray::new(Vec3d::new(0.25, 0.25, -1.0), Vec3d::new(0.0, 0.0, 1.0));

        let p0 = Vec3d::new(0.0, 0.0, 0.0);
        let p1 = Vec3d::new(1.0, 0.0, 0.0);
        let p2 = Vec3d::new(0.0, 1.0, 0.0);

        let result = ray.intersect_triangle(&p0, &p1, &p2, f64::INFINITY);
        assert!(result.is_some());
        let (dist, bary, front) = result.unwrap();
        assert!((dist - 1.0).abs() < 1e-10);
        assert!(front);

        // Verify barycentric coordinates
        assert!(bary.x >= 0.0 && bary.y >= 0.0 && bary.z >= 0.0);
        assert!(((bary.x + bary.y + bary.z) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_triangle_miss() {
        let ray = Ray::new(Vec3d::new(2.0, 2.0, -1.0), Vec3d::new(0.0, 0.0, 1.0));

        let p0 = Vec3d::new(0.0, 0.0, 0.0);
        let p1 = Vec3d::new(1.0, 0.0, 0.0);
        let p2 = Vec3d::new(0.0, 1.0, 0.0);

        assert!(
            ray.intersect_triangle(&p0, &p1, &p2, f64::INFINITY)
                .is_none()
        );
    }

    #[test]
    fn test_transform() {
        let mut ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let matrix = Matrix4d::from_translation(Vec3d::new(10.0, 0.0, 0.0));
        ray.transform(&matrix);

        assert!((ray.start_point().x - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_default() {
        let ray: Ray = Default::default();
        assert!((ray.start_point().x - 0.0).abs() < 1e-10);
        assert!((ray.direction().z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_display() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let s = format!("{}", ray);
        assert!(s.contains(">>"));
    }

    #[test]
    fn test_equality() {
        let r1 = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let r2 = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let r3 = Ray::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(0.0, 1.0, 0.0));

        assert!(r1 == r2);
        assert!(r1 != r3);
    }

    #[test]
    fn test_intersect_bbox() {
        use crate::{BBox3d, Range3d};

        let range = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 1.0));
        let bbox = BBox3d::from_range(range);
        let ray = Ray::new(Vec3d::new(-1.0, 0.5, 0.5), Vec3d::new(1.0, 0.0, 0.0));

        let result = ray.intersect_bbox(&bbox);
        assert!(result.is_some());
        let (enter, exit) = result.unwrap();
        assert!((enter - 1.0).abs() < 1e-10);
        assert!((exit - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_cylinder() {
        let ray = Ray::new(Vec3d::new(-5.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0));
        let origin = Vec3d::new(0.0, 0.0, 0.0);
        let axis = Vec3d::new(0.0, 0.0, 1.0);
        let radius = 1.0;

        let result = ray.intersect_cylinder(&origin, &axis, radius);
        assert!(result.is_some());
        let (enter, exit) = result.unwrap();
        assert!((enter - 4.0).abs() < 1e-10);
        assert!((exit - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_intersect_cone() {
        let ray = Ray::new(Vec3d::new(0.0, 0.0, -2.0), Vec3d::new(0.0, 0.0, 1.0));
        let origin = Vec3d::new(0.0, 0.0, 0.0);
        let axis = Vec3d::new(0.0, 0.0, 1.0);
        let radius = 1.0;
        let height = 1.0;

        let result = ray.intersect_cone(&origin, &axis, radius, height);
        assert!(result.is_some());
    }
}
