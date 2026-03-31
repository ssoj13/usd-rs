//! Plane type for representing 3D planes.
//!
//! A plane is defined by a unit normal and distance from origin.
//! Can also represent a half-space (the side of the plane in the direction
//! of the normal).
//!
//! # Plane equation
//!
//! The plane satisfies: `normal.x * x + normal.y * y + normal.z * z = distance`
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Plane, Vec3d};
//!
//! // Create XY plane at z=0
//! let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 0.0);
//!
//! // Check distance from point to plane
//! let p = Vec3d::new(0.0, 0.0, 5.0);
//! assert!((plane.distance(&p) - 5.0).abs() < 1e-10);
//! ```

use crate::matrix4::Matrix4d;
use crate::range::Range3d;
use crate::vec3::{Vec3d, cross};
use crate::vec4::Vec4d;
use std::fmt;

/// A 3-dimensional plane.
///
/// Represented by a unit normal vector and the distance from the origin.
/// The plane equation is: `normal.dot(p) = distance`.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Plane {
    /// Unit normal vector.
    normal: Vec3d,
    /// Distance from origin along the normal.
    distance: f64,
}

impl Plane {
    /// Creates a plane from a normal and distance from origin.
    ///
    /// The normal is normalized to unit length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Plane, Vec3d};
    ///
    /// let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 2.0), 5.0);
    /// assert!((plane.normal().z - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_normal_distance(normal: Vec3d, distance: f64) -> Self {
        let normalized = normal.normalized();
        Self {
            normal: normalized,
            distance,
        }
    }

    /// Creates a plane from a normal and a point on the plane.
    ///
    /// The normal is normalized to unit length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Plane, Vec3d};
    ///
    /// let plane = Plane::from_normal_point(
    ///     Vec3d::new(0.0, 0.0, 1.0),
    ///     Vec3d::new(0.0, 0.0, 5.0)
    /// );
    /// assert!((plane.distance_from_origin() - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_normal_point(normal: Vec3d, point: Vec3d) -> Self {
        let normalized = normal.normalized();
        let distance = normalized.x * point.x + normalized.y * point.y + normalized.z * point.z;
        Self {
            normal: normalized,
            distance,
        }
    }

    /// Creates a plane from three points.
    ///
    /// The normal is constructed from the cross product of (p1 - p0) and (p2 - p0).
    /// Results are undefined if points are collinear.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Plane, Vec3d};
    ///
    /// let plane = Plane::from_three_points(
    ///     Vec3d::new(0.0, 0.0, 0.0),
    ///     Vec3d::new(1.0, 0.0, 0.0),
    ///     Vec3d::new(0.0, 1.0, 0.0)
    /// );
    /// // Should be the XY plane with normal (0,0,1)
    /// assert!((plane.normal().z - 1.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_three_points(p0: Vec3d, p1: Vec3d, p2: Vec3d) -> Self {
        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let normal = cross(&v1, &v2);
        Self::from_normal_point(normal, p0)
    }

    /// Creates a plane from the equation coefficients (a, b, c, d).
    ///
    /// The equation is: `a*x + b*y + c*z + d = 0`.
    /// Note: d = -distance in our representation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Plane, Vec4d};
    ///
    /// // z = 5 => 0*x + 0*y + 1*z - 5 = 0
    /// let plane = Plane::from_equation(Vec4d::new(0.0, 0.0, 1.0, -5.0));
    /// assert!((plane.distance_from_origin() - 5.0).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn from_equation(eqn: Vec4d) -> Self {
        let normal = Vec3d::new(eqn.x, eqn.y, eqn.z);
        let len = normal.length();
        if len > f64::EPSILON {
            let inv_len = 1.0 / len;
            Self {
                normal: Vec3d::new(eqn.x * inv_len, eqn.y * inv_len, eqn.z * inv_len),
                distance: -eqn.w * inv_len,
            }
        } else {
            Self {
                normal: Vec3d::new(0.0, 0.0, 1.0),
                distance: 0.0,
            }
        }
    }

    /// Returns the unit-length normal vector.
    #[inline]
    #[must_use]
    pub fn normal(&self) -> &Vec3d {
        &self.normal
    }

    /// Returns the distance from the origin along the normal.
    #[inline]
    #[must_use]
    pub fn distance_from_origin(&self) -> f64 {
        self.distance
    }

    /// Returns the equation coefficients as (a, b, c, d) where ax + by + cz + d = 0.
    #[must_use]
    pub fn equation(&self) -> Vec4d {
        Vec4d::new(self.normal.x, self.normal.y, self.normal.z, -self.distance)
    }

    /// Returns the signed distance from point to plane.
    ///
    /// Positive if point is on the side of the normal, negative otherwise.
    #[inline]
    #[must_use]
    pub fn distance(&self, point: &Vec3d) -> f64 {
        point.x * self.normal.x + point.y * self.normal.y + point.z * self.normal.z - self.distance
    }

    /// Projects a point onto the plane.
    #[inline]
    #[must_use]
    pub fn project(&self, point: &Vec3d) -> Vec3d {
        let d = self.distance(point);
        Vec3d::new(
            point.x - d * self.normal.x,
            point.y - d * self.normal.y,
            point.z - d * self.normal.z,
        )
    }

    /// Flips the plane normal if necessary so point is in the positive half-space.
    pub fn reorient(&mut self, point: &Vec3d) {
        if self.distance(point) < 0.0 {
            self.normal = Vec3d::new(-self.normal.x, -self.normal.y, -self.normal.z);
            self.distance = -self.distance;
        }
    }

    /// Returns true if the point is on the plane or in the positive half-space.
    #[inline]
    #[must_use]
    pub fn intersects_positive_half_space_point(&self, point: &Vec3d) -> bool {
        self.distance(point) >= 0.0
    }

    /// Returns true if the bounding box is at least partially in the positive half-space.
    #[must_use]
    pub fn intersects_positive_half_space_box(&self, bbox: &Range3d) -> bool {
        // Find the corner most in the direction of the normal
        let min = bbox.min();
        let max = bbox.max();

        // P-vertex: the corner that is furthest along the normal direction
        let px = if self.normal.x >= 0.0 { max.x } else { min.x };
        let py = if self.normal.y >= 0.0 { max.y } else { min.y };
        let pz = if self.normal.z >= 0.0 { max.z } else { min.z };

        let p_vertex = Vec3d::new(px, py, pz);
        self.distance(&p_vertex) >= 0.0
    }

    /// Transforms the plane by the given matrix.
    ///
    /// Uses the inverse transpose for correct normal transformation.
    pub fn transform(&mut self, matrix: &Matrix4d) {
        // A point on the plane
        let point_on_plane = Vec3d::new(
            self.normal.x * self.distance,
            self.normal.y * self.distance,
            self.normal.z * self.distance,
        );

        // Transform point and get new point on plane
        let new_point = matrix.transform_point(&point_on_plane);

        // For normal, we need to use the inverse transpose
        // But we can also compute it by transforming two vectors in the plane
        // and taking their cross product. For now, use the approximation
        // with transform_dir and re-normalize.
        let transformed_normal = matrix.transform_dir(&self.normal).normalized();

        *self = Self::from_normal_point(transformed_normal, new_point);
    }
}

impl Default for Plane {
    fn default() -> Self {
        // XY plane at origin
        Self {
            normal: Vec3d::new(0.0, 0.0, 1.0),
            distance: 0.0,
        }
    }
}

impl PartialEq for Plane {
    fn eq(&self, other: &Self) -> bool {
        self.normal == other.normal && self.distance == other.distance
    }
}

impl fmt::Display for Plane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}, {}, {}) {}]",
            self.normal.x, self.normal.y, self.normal.z, self.distance
        )
    }
}

/// Fits a plane to the given points using linear least squares.
///
/// Matches C++ `GfFitPlaneToPoints(const std::vector<GfVec3d>& points, GfPlane* fitPlane)`.
///
/// Requires at least 3 non-collinear points. Returns `None` if the points are
/// degenerate (fewer than 3, or all collinear).
///
/// Algorithm: minimises sum of squared distances from points to plane `ax+by+cz+d=0`.
/// Points are first recentred at their centroid (eliminating d). The system is
/// then solved in whichever of the three cases (a=1, b=1, c=1) has the largest
/// |det(AᵀA)|, matching C++ GfFitPlaneToPoints exactly.
///
/// # Examples
///
/// ```
/// use usd_gf::{Plane, Vec3d, fit_plane_to_points};
///
/// let points = vec![
///     Vec3d::new(0.0, 0.0, 0.0),
///     Vec3d::new(1.0, 0.0, 0.0),
///     Vec3d::new(0.0, 1.0, 0.0),
/// ];
/// let plane = fit_plane_to_points(&points).unwrap();
/// // XY plane: normal should be (0, 0, ±1)
/// assert!((plane.normal().z.abs() - 1.0).abs() < 1e-10);
/// ```
pub fn fit_plane_to_points(points: &[Vec3d]) -> Option<Plane> {
    if points.len() < 3 {
        return None;
    }

    // Centroid — plane passes through here, eliminates the d term.
    let n = points.len() as f64;
    let mut centroid = Vec3d::new(0.0, 0.0, 0.0);
    for p in points {
        centroid.x += p.x;
        centroid.y += p.y;
        centroid.z += p.z;
    }
    centroid.x /= n;
    centroid.y /= n;
    centroid.z /= n;

    // Accumulate scatter sums over offsets from centroid.
    let (mut xx, mut xy, mut xz, mut yy, mut yz, mut zz) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for p in points {
        let ox = p.x - centroid.x;
        let oy = p.y - centroid.y;
        let oz = p.z - centroid.z;
        xx += ox * ox;
        xy += ox * oy;
        xz += ox * oz;
        yy += oy * oy;
        yz += oy * oz;
        zz += oz * oz;
    }

    // Three linear-least-squares cases — pick the one with largest |det(AᵀA)|:
    //   case 1: a=1  =>  AᵀA = [[yy, yz],[yz, zz]],  AᵀB = [-xy, -xz]
    //   case 2: b=1  =>  AᵀA = [[xx, xz],[xz, zz]],  AᵀB = [-xy, -yz]
    //   case 3: c=1  =>  AᵀA = [[xx, xy],[xy, yy]],  AᵀB = [-xz, -yz]
    let det1 = (yy * zz - yz * yz).abs();
    let det2 = (xx * zz - xz * xz).abs();
    let det3 = (xx * yy - xy * xy).abs();

    // Solve 2x2 system via Cramer's rule: [[p,q],[q,r]] * [u,v] = [s,t]
    // inverse = (1/det) * [[r,-q],[-q,p]]  =>  u=(r*s-q*t)/det, v=(p*t-q*s)/det
    let equation: Vec3d;
    if det1 > 0.0 && det1 > det2 && det1 > det3 {
        // a=1, solve for b,c
        let inv = 1.0 / (yy * zz - yz * yz);
        let b = (zz * (-xy) - yz * (-xz)) * inv;
        let c = (yy * (-xz) - yz * (-xy)) * inv;
        equation = Vec3d::new(1.0, b, c);
    } else if det2 > 0.0 && det2 > det3 {
        // b=1, solve for a,c
        let inv = 1.0 / (xx * zz - xz * xz);
        let a = (zz * (-xy) - xz * (-yz)) * inv;
        let c = (xx * (-yz) - xz * (-xy)) * inv;
        equation = Vec3d::new(a, 1.0, c);
    } else if det3 > 0.0 {
        // c=1, solve for a,b
        let inv = 1.0 / (xx * yy - xy * xy);
        let a = (yy * (-xz) - xy * (-yz)) * inv;
        let b = (xx * (-yz) - xy * (-xz)) * inv;
        equation = Vec3d::new(a, b, 1.0);
    } else {
        // All determinants zero — points are collinear, no unique plane.
        return None;
    }

    // Place the plane at the centroid: d = -(a,b,c) . centroid
    let d = -(equation.x * centroid.x + equation.y * centroid.y + equation.z * centroid.z);
    Some(Plane::from_equation(Vec4d::new(
        equation.x, equation.y, equation.z, d,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_normal_distance() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 2.0), 5.0);
        assert!((plane.normal().z - 1.0).abs() < 1e-10);
        assert!((plane.distance_from_origin() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_normal_point() {
        let plane = Plane::from_normal_point(Vec3d::new(0.0, 0.0, 1.0), Vec3d::new(5.0, 5.0, 10.0));
        assert!((plane.distance_from_origin() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_three_points() {
        // XY plane
        let plane = Plane::from_three_points(
            Vec3d::new(0.0, 0.0, 0.0),
            Vec3d::new(1.0, 0.0, 0.0),
            Vec3d::new(0.0, 1.0, 0.0),
        );
        assert!((plane.normal().z.abs() - 1.0).abs() < 1e-10);
        assert!(plane.distance_from_origin().abs() < 1e-10);
    }

    #[test]
    fn test_from_equation() {
        // z = 5 => z - 5 = 0 => (0,0,1,-5)
        let plane = Plane::from_equation(Vec4d::new(0.0, 0.0, 1.0, -5.0));
        assert!((plane.normal().z - 1.0).abs() < 1e-10);
        assert!((plane.distance_from_origin() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_distance() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 0.0);

        let p1 = Vec3d::new(0.0, 0.0, 5.0);
        assert!((plane.distance(&p1) - 5.0).abs() < 1e-10);

        let p2 = Vec3d::new(0.0, 0.0, -3.0);
        assert!((plane.distance(&p2) - (-3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_project() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 0.0);

        let p = Vec3d::new(5.0, 3.0, 10.0);
        let projected = plane.project(&p);

        assert!((projected.x - 5.0).abs() < 1e-10);
        assert!((projected.y - 3.0).abs() < 1e-10);
        assert!(projected.z.abs() < 1e-10);
    }

    #[test]
    fn test_equation() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);
        let eq = plane.equation();

        assert!((eq.x - 0.0).abs() < 1e-10);
        assert!((eq.y - 0.0).abs() < 1e-10);
        assert!((eq.z - 1.0).abs() < 1e-10);
        assert!((eq.w - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_reorient() {
        let mut plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 0.0);

        // Point below plane should flip it
        let p = Vec3d::new(0.0, 0.0, -1.0);
        plane.reorient(&p);

        assert!((plane.normal().z - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_intersects_positive_half_space_point() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 0.0);

        assert!(plane.intersects_positive_half_space_point(&Vec3d::new(0.0, 0.0, 1.0)));
        assert!(plane.intersects_positive_half_space_point(&Vec3d::new(0.0, 0.0, 0.0)));
        assert!(!plane.intersects_positive_half_space_point(&Vec3d::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn test_intersects_positive_half_space_box() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);

        // Box entirely above plane (z > 5)
        let above = Range3d::new(Vec3d::new(0.0, 0.0, 6.0), Vec3d::new(1.0, 1.0, 7.0));
        assert!(plane.intersects_positive_half_space_box(&above));

        // Box entirely below plane (z < 5)
        let below = Range3d::new(Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 1.0, 4.0));
        assert!(!plane.intersects_positive_half_space_box(&below));

        // Box straddling plane
        let straddle = Range3d::new(Vec3d::new(0.0, 0.0, 4.0), Vec3d::new(1.0, 1.0, 6.0));
        assert!(plane.intersects_positive_half_space_box(&straddle));
    }

    #[test]
    fn test_default() {
        let plane: Plane = Default::default();
        assert!((plane.normal().z - 1.0).abs() < 1e-10);
        assert!(plane.distance_from_origin().abs() < 1e-10);
    }

    // ---- fit_plane_to_points tests ----

    /// Projects a point onto a plane defined by unit normal `n` and distance `dist`.
    fn project_onto_plane(n: Vec3d, dist: f64, p: Vec3d) -> Vec3d {
        let signed_dist = n.x * p.x + n.y * p.y + n.z * p.z - dist;
        Vec3d::new(
            p.x - signed_dist * n.x,
            p.y - signed_dist * n.y,
            p.z - signed_dist * n.z,
        )
    }

    #[test]
    fn test_fit_plane_fewer_than_3_points() {
        // < 3 points => None
        assert!(fit_plane_to_points(&[]).is_none());
        assert!(fit_plane_to_points(&[Vec3d::new(0.0, 0.0, 0.0)]).is_none());
        assert!(
            fit_plane_to_points(&[Vec3d::new(0.0, 0.0, 0.0), Vec3d::new(1.0, 0.0, 0.0)]).is_none()
        );
    }

    #[test]
    fn test_fit_plane_collinear() {
        // Three collinear points along X-axis => None
        let pts = vec![
            Vec3d::new(0.0, 0.0, 0.0),
            Vec3d::new(1.0, 0.0, 0.0),
            Vec3d::new(2.0, 0.0, 0.0),
        ];
        assert!(fit_plane_to_points(&pts).is_none());
    }

    #[test]
    fn test_fit_plane_xy_exact() {
        // Three points exactly on the XY plane
        let pts = vec![
            Vec3d::new(0.0, 0.0, 0.0),
            Vec3d::new(1.0, 0.0, 0.0),
            Vec3d::new(0.0, 1.0, 0.0),
        ];
        let plane = fit_plane_to_points(&pts).expect("should fit XY plane");
        // Normal must be parallel to Z-axis
        assert!(
            (plane.normal().z.abs() - 1.0).abs() < 1e-10,
            "normal z={}",
            plane.normal().z
        );
        assert!(plane.distance_from_origin().abs() < 1e-10);
    }

    #[test]
    fn test_fit_plane_xy_non_unit_coords() {
        // Same XY plane with non-unit coordinates
        let pts = vec![
            Vec3d::new(0.0, 0.0, 0.0),
            Vec3d::new(1.5, 0.0, 0.0),
            Vec3d::new(0.0, 3.2, 0.0),
        ];
        let plane = fit_plane_to_points(&pts).expect("should fit XY plane");
        assert!((plane.normal().z.abs() - 1.0).abs() < 1e-10);
        assert!(plane.distance_from_origin().abs() < 1e-10);
    }

    #[test]
    fn test_fit_plane_complicated_3pts() {
        // Plane: 3x + 4y + 5 = 0  =>  normal=(0.6, 0.8, 0), dist=-1.0
        // (GfVec4d equation: ax+by+cz+d=0 where d=5 means dist = -d/|n| = -1)
        // Actually: from_equation normalises so dist = -eqn.w / len
        // len=5, eqn.w=5 => dist = -5/5 = -1.0
        // Use |dot| = 1 check (allows flip)
        let n_ref = Vec3d::new(0.6, 0.8, 0.0); // (3,4,0)/5
        let dist_ref = -1.0_f64; // -5/5

        let a = project_onto_plane(n_ref, dist_ref, Vec3d::new(2.0, 3.0, 6.0));
        let b = project_onto_plane(n_ref, dist_ref, Vec3d::new(34.0, -2.0, 2.0));
        let c = project_onto_plane(n_ref, dist_ref, Vec3d::new(-3.0, 7.0, -8.0));

        let plane = fit_plane_to_points(&[a, b, c]).expect("should fit");
        let dot = plane.normal().x * n_ref.x + plane.normal().y * n_ref.y;
        assert!(
            (dot.abs() - 1.0).abs() < 1e-6,
            "normal not parallel, dot={}",
            dot
        );
        assert!(
            (plane.distance_from_origin().abs() - dist_ref.abs()).abs() < 1e-6,
            "dist={}",
            plane.distance_from_origin()
        );
    }

    #[test]
    fn test_fit_plane_complicated_5pts() {
        // Same plane as above but 5 points (least-squares overdetermined)
        let n_ref = Vec3d::new(0.6, 0.8, 0.0);
        let dist_ref = -1.0_f64;

        let a = project_onto_plane(n_ref, dist_ref, Vec3d::new(2.0, 3.0, 6.0));
        let b = project_onto_plane(n_ref, dist_ref, Vec3d::new(34.0, -2.0, 2.0));
        let c = project_onto_plane(n_ref, dist_ref, Vec3d::new(-3.0, 7.0, -8.0));
        let d = project_onto_plane(n_ref, dist_ref, Vec3d::new(4.0, 1.0, 1.0));
        let e = project_onto_plane(n_ref, dist_ref, Vec3d::new(87.0, 67.0, 92.0));

        let plane = fit_plane_to_points(&[a, b, c, d, e]).expect("should fit");
        let dot = plane.normal().x * n_ref.x + plane.normal().y * n_ref.y;
        assert!((dot.abs() - 1.0).abs() < 1e-6, "dot={}", dot);
        assert!(
            (plane.distance_from_origin().abs() - dist_ref.abs()).abs() < 1e-6,
            "dist={}",
            plane.distance_from_origin()
        );
    }

    #[test]
    fn test_fit_plane_noisy_diagonal() {
        // Points roughly on plane normal=(1,-1,0)/sqrt(2) through origin,
        // mirroring the C++ Python test (places=2 accuracy).
        let pts = vec![
            Vec3d::new(1.1, 1.0, 5.0),
            Vec3d::new(1.0, 1.1, 2.0),
            Vec3d::new(2.0, 2.1, -4.0),
            Vec3d::new(2.1, 2.0, 1.0),
            Vec3d::new(25.3, 25.2, 3.0),
            Vec3d::new(25.1, 25.4, 6.0),
        ];
        let plane = fit_plane_to_points(&pts).expect("should fit");
        let len = (2.0_f64).sqrt();
        let exp_nx = 1.0 / len;
        let exp_ny = -1.0 / len;
        let dot = (plane.normal().x * exp_nx + plane.normal().y * exp_ny).abs();
        assert!(
            dot > 0.99,
            "normal not close to (1,-1,0)/sqrt2, dot={}",
            dot
        );
        assert!(plane.distance_from_origin().abs() < 0.5);
    }

    #[test]
    fn test_display() {
        let plane = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);
        let s = format!("{}", plane);
        assert!(s.contains("5"));
    }

    #[test]
    fn test_equality() {
        let p1 = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);
        let p2 = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 5.0);
        let p3 = Plane::from_normal_distance(Vec3d::new(0.0, 0.0, 1.0), 6.0);

        assert!(p1 == p2);
        assert!(p1 != p3);
    }
}
