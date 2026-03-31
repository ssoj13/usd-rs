//! Physics Mass Properties computation.
//!
//! Provides utilities for computing and combining mass properties including
//! inertia tensor diagonalization, translation, and rotation operations.
//!
//! Used by RigidBodyAPI::compute_mass_properties to aggregate mass properties
//! from multiple collision shapes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/massProperties.h`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::MassProperties;
//! use usd_gf::{Matrix3f, Matrix4f, Vec3f, Quatf};
//!
//! // Create mass properties for a shape
//! let mass = 10.0;
//! let inertia = Matrix3f::identity();
//! let com = Vec3f::new(0.0, 0.0, 0.0);
//! let props = MassProperties::new(mass, inertia, com);
//!
//! // Combine multiple mass properties
//! let combined = MassProperties::sum(&[props1, props2], &[xform1, xform2]);
//! ```

use usd_gf::{Matrix3f, Matrix4f, Quatf, Vec3f};

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert quaternion to 3x3 rotation matrix.
///
/// Uses standard quaternion-to-matrix conversion formula.
fn quat_to_matrix3(q: &Quatf) -> Matrix3f {
    let qr = q.real();
    let qi = q.imaginary();
    let qx = qi.x;
    let qy = qi.y;
    let qz = qi.z;

    // Standard quaternion to rotation matrix conversion
    let xx = qx * qx;
    let yy = qy * qy;
    let zz = qz * qz;
    let xy = qx * qy;
    let xz = qx * qz;
    let yz = qy * qz;
    let wx = qr * qx;
    let wy = qr * qy;
    let wz = qr * qz;

    Matrix3f::new(
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy - wz),
        2.0 * (xz + wy),
        2.0 * (xy + wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz - wx),
        2.0 * (xz - wy),
        2.0 * (yz + wx),
        1.0 - 2.0 * (xx + yy),
    )
}

/// Create a quaternion rotation around a specific axis.
///
/// # Arguments
/// * `axis` - Axis index (0=X, 1=Y, 2=Z)
/// * `s` - Sine component
/// * `c` - Cosine component (w component)
fn indexed_rotation(axis: u32, s: f32, c: f32) -> Quatf {
    let mut v = Vec3f::zero();
    v[axis as usize] = s;
    Quatf::from_components(c, v.x, v.y, v.z)
}

/// Get the next index in a cyclic 3-element sequence.
/// Returns: 0->1, 1->2, 2->0
fn get_next_index3(i: u32) -> u32 {
    (i + 1 + (i >> 1)) & 3
}

/// Diagonalize a 3x3 symmetric matrix using Jacobi iterations.
///
/// Finds eigenvalues (diagonal elements) and eigenvectors (rotation quaternion).
/// Used to convert an inertia tensor to principal axes form.
///
/// # Arguments
/// * `m` - Symmetric 3x3 matrix to diagonalize
/// * `mass_frame` - Output quaternion representing the rotation to principal axes
///
/// # Returns
/// Diagonal elements (eigenvalues) as Vec3f
pub fn diagonalize(m: &Matrix3f, mass_frame: &mut Quatf) -> Vec3f {
    const MAX_ITERS: u32 = 24;

    let mut q = Quatf::identity();
    let mut d = *m;

    for _ in 0..MAX_ITERS {
        // Build rotation matrix from quaternion
        let axes = quat_to_matrix3(&q);
        // Similarity transform: axes * m * axes^T
        d = axes * *m * axes.transpose();

        // Find largest off-diagonal element
        let d0 = d[1][2].abs();
        let d1 = d[0][2].abs();
        let d2 = d[0][1].abs();

        // Select rotation axis from largest off-diagonal
        let a = if d0 > d1 && d0 > d2 {
            0
        } else if d1 > d2 {
            1
        } else {
            2
        };

        let a1 = get_next_index3(a);
        let a2 = get_next_index3(a1);

        // Check convergence
        if d[a1 as usize][a2 as usize] == 0.0
            || (d[a1 as usize][a1 as usize] - d[a2 as usize][a2 as usize]).abs()
                > 2e6 * (2.0 * d[a1 as usize][a2 as usize]).abs()
        {
            break;
        }

        // cot(2 * phi), where phi is the rotation angle
        let w = (d[a1 as usize][a1 as usize] - d[a2 as usize][a2 as usize])
            / (2.0 * d[a1 as usize][a2 as usize]);
        let absw = w.abs();

        // Compute rotation quaternion
        let r = if absw > 1000.0 {
            // Small angle approximation
            indexed_rotation(a, 1.0 / (4.0 * w), 1.0)
        } else {
            let t = 1.0 / (absw + (w * w + 1.0).sqrt()); // |tan(phi)|
            let h = 1.0 / (t * t + 1.0).sqrt(); // |cos(phi)|
            let sign = if w >= 0.0 { 1.0 } else { -1.0 };
            indexed_rotation(a, ((1.0 - h) / 2.0).sqrt() * sign, ((1.0 + h) / 2.0).sqrt())
        };

        q = (q * r).normalized();
    }

    *mass_frame = q;

    // Extract diagonal elements
    Vec3f::new(d[0][0], d[1][1], d[2][2])
}

// ============================================================================
// MassProperties
// ============================================================================

/// Mass properties computation class.
///
/// Used to combine individual mass properties from multiple shapes and
/// produce final aggregate mass properties for a rigid body.
///
/// Contains:
/// - Mass (scalar)
/// - Center of mass (Vec3f)
/// - Inertia tensor (Matrix3f)
#[derive(Debug, Clone, Copy)]
pub struct MassProperties {
    /// Inertia tensor (3x3 symmetric matrix)
    inertia_tensor: Matrix3f,
    /// Center of mass position
    center_of_mass: Vec3f,
    /// Total mass
    mass: f32,
}

impl Default for MassProperties {
    fn default() -> Self {
        Self::new_identity()
    }
}

impl MassProperties {
    /// Create default mass properties with unit mass and identity inertia.
    pub fn new_identity() -> Self {
        Self {
            inertia_tensor: Matrix3f::identity(),
            center_of_mass: Vec3f::zero(),
            mass: 1.0,
        }
    }

    /// Construct from individual elements.
    ///
    /// # Arguments
    /// * `mass` - Total mass
    /// * `inertia` - 3x3 inertia tensor
    /// * `com` - Center of mass position
    pub fn new(mass: f32, inertia: Matrix3f, com: Vec3f) -> Self {
        Self {
            inertia_tensor: inertia,
            center_of_mass: com,
            mass,
        }
    }

    /// Scale mass properties by a factor.
    ///
    /// Scales both mass and inertia tensor (center of mass unchanged).
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            mass: self.mass * scale,
            inertia_tensor: self.inertia_tensor * scale,
            center_of_mass: self.center_of_mass,
        }
    }

    /// Translate the center of mass and adjust inertia tensor accordingly.
    ///
    /// Uses the parallel axis theorem to update the inertia tensor.
    pub fn translate(&mut self, t: &Vec3f) {
        self.inertia_tensor = Self::translate_inertia(&self.inertia_tensor, self.mass, t);
        self.center_of_mass += *t;
    }

    /// Get the diagonal inertia and corresponding principal axes rotation.
    ///
    /// Diagonalizes the inertia tensor to find principal moments of inertia
    /// and the rotation that transforms to principal axes.
    ///
    /// # Arguments
    /// * `inertia` - Inertia tensor to diagonalize
    /// * `mass_frame` - Output quaternion for principal axes rotation
    ///
    /// # Returns
    /// Principal moments of inertia (diagonal elements)
    pub fn get_mass_space_inertia(inertia: &Matrix3f, mass_frame: &mut Quatf) -> Vec3f {
        diagonalize(inertia, mass_frame)
    }

    /// Translate an inertia tensor using the parallel axis theorem.
    ///
    /// I' = I + m * (t^2 * E - t ⊗ t)
    /// where E is identity, t is translation, ⊗ is outer product
    ///
    /// # Arguments
    /// * `inertia` - Original inertia tensor
    /// * `mass` - Mass of the object
    /// * `t` - Translation vector
    ///
    /// # Returns
    /// Translated inertia tensor
    pub fn translate_inertia(inertia: &Matrix3f, mass: f32, t: &Vec3f) -> Matrix3f {
        // Build skew-symmetric matrix from translation.
        // NOTE: C++ uses SetColumn() which produces the transpose of what set_row() gives.
        // This means our `s` is the negation of the C++ version: s_rust = -s_cpp.
        // However, the final result is identical because (-S)(-S)^T = S * S^T.
        let mut s = Matrix3f::zero();
        s.set_row(0, &Vec3f::new(0.0, t.z, -t.y));
        s.set_row(1, &Vec3f::new(-t.z, 0.0, t.x));
        s.set_row(2, &Vec3f::new(t.y, -t.x, 0.0));

        // I' = s * s^T * mass + I
        s * s.transpose() * mass + *inertia
    }

    /// Rotate an inertia tensor around the center of mass.
    ///
    /// I' = R^T * I * R
    ///
    /// # Arguments
    /// * `inertia` - Original inertia tensor
    /// * `q` - Rotation quaternion
    ///
    /// # Returns
    /// Rotated inertia tensor
    pub fn rotate_inertia(inertia: &Matrix3f, q: &Quatf) -> Matrix3f {
        let m = quat_to_matrix3(q);
        m.transpose() * *inertia * m
    }

    /// Sum up multiple mass properties with their transforms.
    ///
    /// Combines mass, center of mass, and inertia tensors from multiple
    /// shapes into a single aggregate mass properties.
    ///
    /// # Arguments
    /// * `props` - Slice of mass properties to combine
    /// * `transforms` - Corresponding transforms for each mass properties
    ///
    /// # Returns
    /// Combined mass properties
    pub fn sum(props: &[MassProperties], transforms: &[Matrix4f]) -> MassProperties {
        assert_eq!(props.len(), transforms.len());

        let count = props.len();
        if count == 0 {
            return MassProperties::default();
        }

        // First pass: compute total mass and weighted center of mass
        let mut combined_mass = 0.0f32;
        let mut combined_com = Vec3f::zero();

        for i in 0..count {
            combined_mass += props[i].mass;
            let com_transformed = transforms[i].transform_point(&props[i].center_of_mass);
            combined_com += com_transformed * props[i].mass;
        }

        if combined_mass > 0.0 {
            combined_com /= combined_mass;
        }

        // Second pass: accumulate inertia tensors
        let mut combined_inertia = Matrix3f::zero();

        for i in 0..count {
            let com_transformed = transforms[i].transform_point(&props[i].center_of_mass);

            // Rotate inertia to world frame
            let rotation = transforms[i].extract_rotation_quat();
            let rotated_inertia = Self::rotate_inertia(&props[i].inertia_tensor, &rotation);

            // Translate to combined center of mass
            let offset = combined_com - com_transformed;
            let translated = Self::translate_inertia(&rotated_inertia, props[i].mass, &offset);

            combined_inertia += translated;
        }

        MassProperties::new(combined_mass, combined_inertia, combined_com)
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get the inertia tensor.
    #[inline]
    pub fn inertia_tensor(&self) -> &Matrix3f {
        &self.inertia_tensor
    }

    /// Set the inertia tensor.
    #[inline]
    pub fn set_inertia_tensor(&mut self, inertia: Matrix3f) {
        self.inertia_tensor = inertia;
    }

    /// Get the center of mass.
    #[inline]
    pub fn center_of_mass(&self) -> &Vec3f {
        &self.center_of_mass
    }

    /// Get the mass.
    #[inline]
    pub fn mass(&self) -> f32 {
        self.mass
    }

    /// Set the mass.
    #[inline]
    pub fn set_mass(&mut self, mass: f32) {
        self.mass = mass;
    }
}

impl std::ops::Mul<f32> for MassProperties {
    type Output = MassProperties;

    fn mul(self, scale: f32) -> Self::Output {
        self.scaled(scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mass_properties_default() {
        let props = MassProperties::default();
        assert_eq!(props.mass(), 1.0);
        assert_eq!(*props.center_of_mass(), Vec3f::zero());
    }

    #[test]
    fn test_mass_properties_scale() {
        let props = MassProperties::new(10.0, Matrix3f::identity(), Vec3f::zero());
        let scaled = props * 2.0;
        assert_eq!(scaled.mass(), 20.0);
    }

    #[test]
    fn test_get_next_index3() {
        assert_eq!(get_next_index3(0), 1);
        assert_eq!(get_next_index3(1), 2);
        assert_eq!(get_next_index3(2), 0);
    }

    #[test]
    fn test_translate_inertia() {
        let inertia = Matrix3f::identity();
        let translated =
            MassProperties::translate_inertia(&inertia, 1.0, &Vec3f::new(1.0, 0.0, 0.0));
        // Parallel axis theorem: adds m*d^2 to perpendicular axes
        assert!(translated[1][1] > 1.0);
        assert!(translated[2][2] > 1.0);
    }
}
