//! Compound linear transformation.
//!
//! [`Transform`] represents a linear transformation as individual components:
//! translation, rotation, scale, pivot position, and pivot orientation.
//!
//! When applied to a point, the transformation order is:
//! 1. Scale about pivot position with pivot orientation
//! 2. Rotate about pivot position
//! 3. Translate
//!
//! The cumulative matrix is: `M = -P * -O * S * O * R * P * T`
//!
//! where:
//! - T = translation matrix
//! - P = pivot position matrix
//! - R = rotation matrix
//! - O = pivot orientation matrix
//! - S = scale matrix
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Transform, Rotation, vec3d};
//!
//! let mut xf = Transform::new();
//! xf.set_translation(vec3d(10.0, 0.0, 0.0));
//! xf.set_rotation(Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 45.0));
//!
//! let mat = xf.matrix();
//! ```

use crate::math::degrees_to_radians;
use crate::matrix4::Matrix4d;
use crate::rotation::Rotation;
use crate::vec3::Vec3d;
use std::fmt;
use std::ops::{Mul, MulAssign};

/// A compound linear transformation.
///
/// Stores translation, rotation, scale, pivot position, and pivot orientation
/// as separate components that can be combined into a transformation matrix.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    /// Translation component.
    translation: Vec3d,
    /// Rotation component.
    rotation: Rotation,
    /// Scale factors.
    scale: Vec3d,
    /// Orientation for scaling (also called scale orientation).
    pivot_orientation: Rotation,
    /// Center of rotation and scaling.
    pivot_position: Vec3d,
}

impl Default for Transform {
    /// Creates an identity transformation.
    fn default() -> Self {
        Self {
            translation: Vec3d::new(0.0, 0.0, 0.0),
            rotation: Rotation::new(),
            scale: Vec3d::new(1.0, 1.0, 1.0),
            pivot_orientation: Rotation::new(),
            pivot_position: Vec3d::new(0.0, 0.0, 0.0),
        }
    }
}

impl Transform {
    /// Creates an identity transformation.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a transformation from all components.
    ///
    /// Parameter order matches USD 3.x convention.
    #[must_use]
    pub fn from_components(
        translation: Vec3d,
        rotation: Rotation,
        scale: Vec3d,
        pivot_position: Vec3d,
        pivot_orientation: Rotation,
    ) -> Self {
        Self {
            translation,
            rotation,
            scale,
            pivot_orientation,
            pivot_position,
        }
    }

    /// Creates a transformation from a matrix.
    ///
    /// Factors the matrix into components, preserving the current pivot position.
    #[must_use]
    pub fn from_matrix(matrix: &Matrix4d) -> Self {
        let mut xf = Self::new();
        xf.set_matrix(matrix);
        xf
    }

    /// Sets all transformation components.
    pub fn set(
        &mut self,
        translation: Vec3d,
        rotation: Rotation,
        scale: Vec3d,
        pivot_position: Vec3d,
        pivot_orientation: Rotation,
    ) -> &mut Self {
        self.translation = translation;
        self.rotation = rotation;
        self.scale = scale;
        self.pivot_position = pivot_position;
        self.pivot_orientation = pivot_orientation;
        self
    }

    /// Sets the transformation from a matrix, preserving pivot position.
    pub fn set_matrix(&mut self, m: &Matrix4d) -> &mut Self {
        // Create matrix without pivot: [P][m][P^-1]
        let m_pivot_pos = Matrix4d::from_translation(self.pivot_position);
        let m_pivot_pos_inv = Matrix4d::from_translation(-self.pivot_position);
        let m_no_pivot = m_pivot_pos * *m * m_pivot_pos_inv;

        // Factor the matrix (works even if singular)
        let factor_result = m_no_pivot.factor();

        if let Some((shear_rot, scale, rot, trans, _p)) = factor_result {
            self.scale = scale;
            self.translation = trans;
            self.rotation = rot.extract_rotation();

            // Set pivot orientation if scale is non-uniform
            if scale != Vec3d::new(1.0, 1.0, 1.0) {
                self.pivot_orientation = shear_rot.transpose().extract_rotation();
            } else {
                self.pivot_orientation = Rotation::new();
            }
        } else {
            // Singular matrix - try to extract what we can
            self.translation = m_no_pivot.extract_translation();
            self.rotation = m_no_pivot.extract_rotation();
            self.scale = Vec3d::new(1.0, 1.0, 1.0);
            self.pivot_orientation = Rotation::new();
        }
        self
    }

    /// Sets the transformation to identity.
    pub fn set_identity(&mut self) -> &mut Self {
        self.scale = Vec3d::new(1.0, 1.0, 1.0);
        self.pivot_orientation = Rotation::new();
        self.rotation = Rotation::new();
        self.pivot_position = Vec3d::new(0.0, 0.0, 0.0);
        self.translation = Vec3d::new(0.0, 0.0, 0.0);
        self
    }

    /// Sets the scale component.
    #[inline]
    pub fn set_scale(&mut self, scale: Vec3d) {
        self.scale = scale;
    }

    /// Sets the pivot orientation (scale orientation).
    #[inline]
    pub fn set_pivot_orientation(&mut self, orient: Rotation) {
        self.pivot_orientation = orient;
    }

    /// Sets the scale orientation (alias for pivot orientation).
    #[inline]
    pub fn set_scale_orientation(&mut self, orient: Rotation) {
        self.pivot_orientation = orient;
    }

    /// Sets the rotation component.
    #[inline]
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }

    /// Sets the pivot position (center of rotation and scaling).
    #[inline]
    pub fn set_pivot_position(&mut self, pos: Vec3d) {
        self.pivot_position = pos;
    }

    /// Sets the center (alias for pivot position).
    #[inline]
    pub fn set_center(&mut self, pos: Vec3d) {
        self.pivot_position = pos;
    }

    /// Sets the translation component.
    #[inline]
    pub fn set_translation(&mut self, translation: Vec3d) {
        self.translation = translation;
    }

    /// Returns the scale component.
    #[inline]
    #[must_use]
    pub fn scale(&self) -> Vec3d {
        self.scale
    }

    /// Returns the pivot orientation.
    #[inline]
    #[must_use]
    pub fn pivot_orientation(&self) -> &Rotation {
        &self.pivot_orientation
    }

    /// Returns the scale orientation (alias).
    #[inline]
    #[must_use]
    pub fn scale_orientation(&self) -> &Rotation {
        &self.pivot_orientation
    }

    /// Returns the rotation component.
    #[inline]
    #[must_use]
    pub fn rotation(&self) -> &Rotation {
        &self.rotation
    }

    /// Returns the pivot position.
    #[inline]
    #[must_use]
    pub fn pivot_position(&self) -> Vec3d {
        self.pivot_position
    }

    /// Returns the center (alias for pivot position).
    #[inline]
    #[must_use]
    pub fn center(&self) -> Vec3d {
        self.pivot_position
    }

    /// Returns the translation component.
    #[inline]
    #[must_use]
    pub fn translation(&self) -> Vec3d {
        self.translation
    }

    /// Returns the cumulative transformation matrix.
    ///
    /// M = -P * -O * S * O * R * P * T
    #[must_use]
    pub fn matrix(&self) -> Matrix4d {
        let zero = Vec3d::new(0.0, 0.0, 0.0);
        let one = Vec3d::new(1.0, 1.0, 1.0);

        let do_pivot = self.pivot_position != zero;
        let do_scale = self.scale != one;
        let do_scale_orient = self.pivot_orientation.angle() != 0.0;
        let do_rotation = self.rotation.angle() != 0.0;
        let do_translation = self.translation != zero;

        let mut mtx = Matrix4d::identity();
        let mut any_set = false;

        // Helper macro to accumulate matrix operations
        macro_rules! accum {
            ($op:expr) => {
                if any_set {
                    mtx *= $op;
                } else {
                    mtx = $op;
                    any_set = true;
                }
            };
        }

        // -P: translate to pivot origin
        if do_pivot {
            accum!(Matrix4d::from_translation(-self.pivot_position));
        }

        // Scale with orientation
        if do_scale {
            // -O: inverse pivot orientation
            if do_scale_orient {
                let inv = self.pivot_orientation.inverse();
                accum!(Matrix4d::from_rotation(
                    inv.axis(),
                    degrees_to_radians(inv.angle())
                ));
            }

            // S: scale
            accum!(Matrix4d::from_scale_vec(&self.scale));

            // O: pivot orientation
            if do_scale_orient {
                accum!(Matrix4d::from_rotation(
                    self.pivot_orientation.axis(),
                    degrees_to_radians(self.pivot_orientation.angle())
                ));
            }
        }

        // R: rotation
        if do_rotation {
            accum!(Matrix4d::from_rotation(
                self.rotation.axis(),
                degrees_to_radians(self.rotation.angle())
            ));
        }

        // P: translate back from pivot
        if do_pivot {
            accum!(Matrix4d::from_translation(self.pivot_position));
        }

        // T: translation
        if do_translation {
            accum!(Matrix4d::from_translation(self.translation));
        }

        // Silence unused assignment warning
        let _ = any_set;

        mtx
    }
}

impl PartialEq for Transform {
    /// Component-wise equality.
    fn eq(&self, other: &Self) -> bool {
        self.scale == other.scale
            && self.pivot_orientation == other.pivot_orientation
            && self.rotation == other.rotation
            && self.pivot_position == other.pivot_position
            && self.translation == other.translation
    }
}

impl Eq for Transform {}

impl MulAssign for Transform {
    /// Post-multiplies another transform into this one.
    fn mul_assign(&mut self, rhs: Self) {
        let combined = self.matrix() * rhs.matrix();
        self.set_matrix(&combined);
    }
}

impl Mul for Transform {
    type Output = Self;

    /// Returns the product of two transforms.
    fn mul(mut self, rhs: Self) -> Self::Output {
        self *= rhs;
        self
    }
}

impl fmt::Display for Transform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "( ({}, {}, {}, 0), ({}, {}, {}, {}), ({}, {}, {}, {}), ({}, {}, {}, 0), ({}, {}, {}, 0) )",
            self.scale.x,
            self.scale.y,
            self.scale.z,
            self.pivot_orientation.axis().x,
            self.pivot_orientation.axis().y,
            self.pivot_orientation.axis().z,
            self.pivot_orientation.angle(),
            self.rotation.axis().x,
            self.rotation.axis().y,
            self.rotation.axis().z,
            self.rotation.angle(),
            self.pivot_position.x,
            self.pivot_position.y,
            self.pivot_position.z,
            self.translation.x,
            self.translation.y,
            self.translation.z
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    #[test]
    fn test_default() {
        let xf = Transform::new();
        assert_eq!(xf.scale(), vec3d(1.0, 1.0, 1.0));
        assert_eq!(xf.translation(), vec3d(0.0, 0.0, 0.0));
        assert_eq!(xf.rotation().angle(), 0.0);
    }

    #[test]
    fn test_identity_matrix() {
        let xf = Transform::new();
        let mat = xf.matrix();
        assert_eq!(mat, Matrix4d::identity());
    }

    #[test]
    fn test_translation() {
        let mut xf = Transform::new();
        xf.set_translation(vec3d(10.0, 20.0, 30.0));

        let mat = xf.matrix();
        // Translation is in row 3
        assert!((mat[3][0] - 10.0).abs() < 1e-10);
        assert!((mat[3][1] - 20.0).abs() < 1e-10);
        assert!((mat[3][2] - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_scale() {
        let mut xf = Transform::new();
        xf.set_scale(vec3d(2.0, 3.0, 4.0));

        let mat = xf.matrix();
        assert!((mat[0][0] - 2.0).abs() < 1e-10);
        assert!((mat[1][1] - 3.0).abs() < 1e-10);
        assert!((mat[2][2] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_rotation() {
        let mut xf = Transform::new();
        xf.set_rotation(Rotation::from_axis_angle(vec3d(0.0, 0.0, 1.0), 90.0));

        let mat = xf.matrix();
        let point = vec3d(1.0, 0.0, 0.0);
        let result = mat.transform_point(&point);

        // 90 degrees around Z takes X to Y
        assert!((result.x).abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_from_components() {
        let xf = Transform::from_components(
            vec3d(5.0, 0.0, 0.0),
            Rotation::new(),
            vec3d(2.0, 2.0, 2.0),
            vec3d(0.0, 0.0, 0.0),
            Rotation::new(),
        );

        assert_eq!(xf.translation(), vec3d(5.0, 0.0, 0.0));
        assert_eq!(xf.scale(), vec3d(2.0, 2.0, 2.0));
    }

    #[test]
    fn test_equality() {
        let xf1 = Transform::new();
        let xf2 = Transform::new();
        assert_eq!(xf1, xf2);
    }

    #[test]
    fn test_display() {
        let xf = Transform::new();
        let s = format!("{}", xf);
        assert!(s.contains("1, 1, 1"));
    }
}
