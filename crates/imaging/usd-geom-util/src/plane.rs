// Plane mesh generator

use super::mesh_generator::{vec3d_helpers, vec3f_helpers};
use super::tokens::InterpolationTokens;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::{MeshTopology, tokens as px_osd_tokens};

/// Plane mesh generator
///
/// Generates a rectangular plane in the XY plane, centered at the origin
/// with dimensions along X and Y axes. The plane is a single quad.
pub struct PlaneMeshGenerator;

impl PlaneMeshGenerator {
    /// Compute number of points for a plane
    pub fn compute_num_points() -> usize {
        4
    }

    /// Compute number of normals for a plane
    pub fn compute_num_normals() -> usize {
        // Single normal for entire plane (constant interpolation)
        1
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::CONSTANT
    }

    /// Generate topology for a plane
    pub fn generate_topology() -> MeshTopology {
        let counts = vec![4]; // One quad
        let indices = vec![0, 1, 2, 3];

        MeshTopology::new(
            (*px_osd_tokens::BILINEAR).clone(),
            (*px_osd_tokens::RIGHT_HANDED).clone(),
            counts,
            indices,
        )
    }

    /// Generate points for a plane (f32 version)
    ///
    /// # Arguments
    /// * `x_length` - Length along X axis
    /// * `y_length` - Length along Y axis
    /// * `transform` - Optional transform matrix
    pub fn generate_points_f32(
        x_length: f32,
        y_length: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        let mut points = Vec::with_capacity(4);

        let x = 0.5 * x_length;
        let y = 0.5 * y_length;

        vec3f_helpers::write_point(&mut points, Vec3f::new(x, y, 0.0), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, y, 0.0), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, -y, 0.0), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(x, -y, 0.0), transform);

        points
    }

    /// Generate points for a plane (f64 version)
    pub fn generate_points_f64(
        x_length: f64,
        y_length: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        let mut points = Vec::with_capacity(4);

        let x = 0.5 * x_length;
        let y = 0.5 * y_length;

        vec3d_helpers::write_point(&mut points, Vec3d::new(x, y, 0.0), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, y, 0.0), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, -y, 0.0), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(x, -y, 0.0), transform);

        points
    }

    /// Generate normals for a plane (f32 version)
    ///
    /// Returns a single normal pointing in +Z direction (constant interpolation).
    pub fn generate_normals_f32(transform: Option<&Matrix4d>) -> Vec<Vec3f> {
        let mut normals = Vec::with_capacity(1);
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, 1.0), transform);
        normals
    }

    /// Generate normals for a plane (f64 version)
    pub fn generate_normals_f64(transform: Option<&Matrix4d>) -> Vec<Vec3d> {
        let mut normals = Vec::with_capacity(1);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, 1.0), transform);
        normals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_num_points() {
        assert_eq!(PlaneMeshGenerator::compute_num_points(), 4);
    }

    #[test]
    fn test_compute_num_normals() {
        assert_eq!(PlaneMeshGenerator::compute_num_normals(), 1);
    }

    #[test]
    fn test_normals_interpolation() {
        assert_eq!(
            PlaneMeshGenerator::normals_interpolation(),
            InterpolationTokens::CONSTANT
        );
    }

    #[test]
    fn test_generate_topology() {
        let topo = PlaneMeshGenerator::generate_topology();
        let counts = topo.face_vertex_counts();
        let indices = topo.face_vertex_indices();

        // Single quad
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0], 4);
        assert_eq!(indices.len(), 4);
    }

    #[test]
    fn test_generate_points_f32() {
        let points = PlaneMeshGenerator::generate_points_f32(4.0, 6.0, None);
        assert_eq!(points.len(), 4);

        // Check corners
        assert_eq!(points[0], Vec3f::new(2.0, 3.0, 0.0));
        assert_eq!(points[1], Vec3f::new(-2.0, 3.0, 0.0));
        assert_eq!(points[2], Vec3f::new(-2.0, -3.0, 0.0));
        assert_eq!(points[3], Vec3f::new(2.0, -3.0, 0.0));
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = PlaneMeshGenerator::generate_normals_f32(None);
        assert_eq!(normals.len(), 1);
        assert_eq!(normals[0], Vec3f::new(0.0, 0.0, 1.0));
    }
}
