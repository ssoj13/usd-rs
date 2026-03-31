//! Utility functions for homogeneous coordinates.
//!
//! Provides functions for working with 4D homogeneous vectors,
//! including projection to 3D Euclidean space and homogeneous cross product.
//!
//! # Homogeneous Coordinates
//!
//! A homogeneous coordinate `(x, y, z, w)` represents the 3D point
//! `(x/w, y/w, z/w)` when `w != 0`. When `w = 0`, the vector represents
//! a direction (point at infinity).
//!
//! # Examples
//!
//! ```
//! use usd_gf::homogeneous::{project, homogenize};
//! use usd_gf::{Vec4d, Vec3d};
//!
//! // Project a homogeneous point to 3D
//! let h = Vec4d::new(2.0, 4.0, 6.0, 2.0);
//! let p = project(h);
//! assert!((p.x - 1.0).abs() < 1e-10);
//! assert!((p.y - 2.0).abs() < 1e-10);
//! assert!((p.z - 3.0).abs() < 1e-10);
//!
//! // Homogenize a vector (normalize w to 1)
//! let normalized = homogenize(h);
//! assert!((normalized.w - 1.0).abs() < 1e-10);
//! ```

use crate::vec3::{Vec3d, Vec3f, cross};
use crate::vec4::{Vec4d, Vec4f};

/// Projects a homogeneous Vec4f into Euclidean 3D space.
///
/// Divides x, y, z by w. If w is 0, treats it as 1.
#[inline]
#[must_use]
pub fn project_f(v: Vec4f) -> Vec3f {
    let inv = if v.w != 0.0 { 1.0 / v.w } else { 1.0 };
    Vec3f::new(inv * v.x, inv * v.y, inv * v.z)
}

/// Projects a homogeneous Vec4d into Euclidean 3D space.
///
/// Divides x, y, z by w. If w is 0, treats it as 1.
#[inline]
#[must_use]
pub fn project(v: Vec4d) -> Vec3d {
    let inv = if v.w != 0.0 { 1.0 / v.w } else { 1.0 };
    Vec3d::new(inv * v.x, inv * v.y, inv * v.z)
}

/// Homogenizes a Vec4f (normalizes w to 1).
///
/// Divides all components by w. If w is 0, sets w to 1 first.
#[must_use]
pub fn homogenize_f(v: Vec4f) -> Vec4f {
    let w = if v.w == 0.0 { 1.0 } else { v.w };
    Vec4f::new(v.x / w, v.y / w, v.z / w, 1.0)
}

/// Homogenizes a Vec4d (normalizes w to 1).
///
/// Divides all components by w. If w is 0, sets w to 1 first.
#[must_use]
pub fn homogenize(v: Vec4d) -> Vec4d {
    let w = if v.w == 0.0 { 1.0 } else { v.w };
    Vec4d::new(v.x / w, v.y / w, v.z / w, 1.0)
}

/// Performs cross product on homogenized Vec4f vectors.
///
/// Homogenizes both vectors, then performs cross product on the
/// x, y, z components. Returns result as a homogenized Vec4f.
#[must_use]
pub fn homogeneous_cross_f(a: Vec4f, b: Vec4f) -> Vec4f {
    let ah = homogenize_f(a);
    let bh = homogenize_f(b);

    let av = Vec3f::new(ah.x, ah.y, ah.z);
    let bv = Vec3f::new(bh.x, bh.y, bh.z);
    let prod = cross(&av, &bv);

    Vec4f::new(prod.x, prod.y, prod.z, 1.0)
}

/// Performs cross product on homogenized Vec4d vectors.
///
/// Homogenizes both vectors, then performs cross product on the
/// x, y, z components. Returns result as a homogenized Vec4d.
#[must_use]
pub fn homogeneous_cross(a: Vec4d, b: Vec4d) -> Vec4d {
    let ah = homogenize(a);
    let bh = homogenize(b);

    let av = Vec3d::new(ah.x, ah.y, ah.z);
    let bv = Vec3d::new(bh.x, bh.y, bh.z);
    let prod = cross(&av, &bv);

    Vec4d::new(prod.x, prod.y, prod.z, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_project_simple() {
        let v = Vec4d::new(2.0, 4.0, 6.0, 2.0);
        let p = project(v);
        assert!(approx_eq(p.x, 1.0));
        assert!(approx_eq(p.y, 2.0));
        assert!(approx_eq(p.z, 3.0));
    }

    #[test]
    fn test_project_w_one() {
        let v = Vec4d::new(1.0, 2.0, 3.0, 1.0);
        let p = project(v);
        assert!(approx_eq(p.x, 1.0));
        assert!(approx_eq(p.y, 2.0));
        assert!(approx_eq(p.z, 3.0));
    }

    #[test]
    fn test_project_w_zero() {
        // When w=0, treat as w=1 (direction vector)
        let v = Vec4d::new(1.0, 2.0, 3.0, 0.0);
        let p = project(v);
        assert!(approx_eq(p.x, 1.0));
        assert!(approx_eq(p.y, 2.0));
        assert!(approx_eq(p.z, 3.0));
    }

    #[test]
    fn test_homogenize() {
        let v = Vec4d::new(2.0, 4.0, 6.0, 2.0);
        let h = homogenize(v);
        assert!(approx_eq(h.x, 1.0));
        assert!(approx_eq(h.y, 2.0));
        assert!(approx_eq(h.z, 3.0));
        assert!(approx_eq(h.w, 1.0));
    }

    #[test]
    fn test_homogenize_w_zero() {
        let v = Vec4d::new(1.0, 2.0, 3.0, 0.0);
        let h = homogenize(v);
        assert!(approx_eq(h.x, 1.0));
        assert!(approx_eq(h.y, 2.0));
        assert!(approx_eq(h.z, 3.0));
        assert!(approx_eq(h.w, 1.0));
    }

    #[test]
    fn test_homogeneous_cross() {
        // Cross product of X and Y axes should be Z axis
        let x = Vec4d::new(1.0, 0.0, 0.0, 1.0);
        let y = Vec4d::new(0.0, 1.0, 0.0, 1.0);
        let z = homogeneous_cross(x, y);

        assert!(approx_eq(z.x, 0.0));
        assert!(approx_eq(z.y, 0.0));
        assert!(approx_eq(z.z, 1.0));
        assert!(approx_eq(z.w, 1.0));
    }

    #[test]
    fn test_homogeneous_cross_scaled() {
        // Same cross product but with scaled homogeneous coords
        let x = Vec4d::new(2.0, 0.0, 0.0, 2.0); // Same as (1,0,0,1) when homogenized
        let y = Vec4d::new(0.0, 3.0, 0.0, 3.0); // Same as (0,1,0,1) when homogenized
        let z = homogeneous_cross(x, y);

        assert!(approx_eq(z.x, 0.0));
        assert!(approx_eq(z.y, 0.0));
        assert!(approx_eq(z.z, 1.0));
        assert!(approx_eq(z.w, 1.0));
    }

    #[test]
    fn test_project_f() {
        let v = Vec4f::new(2.0, 4.0, 6.0, 2.0);
        let p = project_f(v);
        assert!((p.x - 1.0).abs() < 1e-6);
        assert!((p.y - 2.0).abs() < 1e-6);
        assert!((p.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_homogenize_f() {
        let v = Vec4f::new(2.0, 4.0, 6.0, 2.0);
        let h = homogenize_f(v);
        assert!((h.x - 1.0).abs() < 1e-6);
        assert!((h.w - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_homogeneous_cross_f() {
        let x = Vec4f::new(1.0, 0.0, 0.0, 1.0);
        let y = Vec4f::new(0.0, 1.0, 0.0, 1.0);
        let z = homogeneous_cross_f(x, y);

        assert!((z.x).abs() < 1e-6);
        assert!((z.y).abs() < 1e-6);
        assert!((z.z - 1.0).abs() < 1e-6);
        assert!((z.w - 1.0).abs() < 1e-6);
    }
}
