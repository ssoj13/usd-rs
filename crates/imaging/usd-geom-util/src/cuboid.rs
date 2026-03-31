// Cuboid (box) mesh generator

use super::mesh_generator::{vec3d_helpers, vec3f_helpers};
use super::tokens::InterpolationTokens;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::{MeshTopology, tokens as px_osd_tokens};

/// Cuboid mesh generator
///
/// Generates a rectangular cuboid (box) centered at the origin with dimensions
/// along X, Y, and Z axes. The generated mesh has 8 vertices and 6 quad faces.
pub struct CuboidMeshGenerator;

impl CuboidMeshGenerator {
    /// Compute number of points for a cuboid
    pub fn compute_num_points() -> usize {
        8
    }

    /// Compute number of normals for a cuboid
    pub fn compute_num_normals() -> usize {
        // One normal per face (uniform interpolation)
        6
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::UNIFORM
    }

    /// Generate topology for a cuboid
    pub fn generate_topology() -> MeshTopology {
        // 6 faces, each with 4 vertices
        let counts = vec![4, 4, 4, 4, 4, 4];
        let indices = vec![
            0, 1, 2, 3, // Front face
            4, 5, 6, 7, // Back face
            0, 6, 5, 1, // Right face
            4, 7, 3, 2, // Left face
            0, 3, 7, 6, // Top face
            4, 2, 1, 5, // Bottom face
        ];

        MeshTopology::new(
            (*px_osd_tokens::BILINEAR).clone(),
            (*px_osd_tokens::RIGHT_HANDED).clone(),
            counts,
            indices,
        )
    }

    /// Generate points for a cuboid (f32 version)
    ///
    /// # Arguments
    /// * `x_length` - Length along X axis
    /// * `y_length` - Length along Y axis
    /// * `z_length` - Length along Z axis
    /// * `transform` - Optional transform matrix to apply
    pub fn generate_points_f32(
        x_length: f32,
        y_length: f32,
        z_length: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        let mut points = Vec::with_capacity(8);

        let x = 0.5 * x_length;
        let y = 0.5 * y_length;
        let z = 0.5 * z_length;

        vec3f_helpers::write_point(&mut points, Vec3f::new(x, y, z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, y, z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, -y, z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(x, -y, z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, -y, -z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(-x, y, -z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(x, y, -z), transform);
        vec3f_helpers::write_point(&mut points, Vec3f::new(x, -y, -z), transform);

        points
    }

    /// Generate points for a cuboid (f64 version)
    pub fn generate_points_f64(
        x_length: f64,
        y_length: f64,
        z_length: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        let mut points = Vec::with_capacity(8);

        let x = 0.5 * x_length;
        let y = 0.5 * y_length;
        let z = 0.5 * z_length;

        vec3d_helpers::write_point(&mut points, Vec3d::new(x, y, z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, y, z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, -y, z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(x, -y, z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, -y, -z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(-x, y, -z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(x, y, -z), transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(x, -y, -z), transform);

        points
    }

    /// Generate normals for a cuboid (f32 version)
    ///
    /// Returns 6 normals, one per face (uniform interpolation).
    pub fn generate_normals_f32(transform: Option<&Matrix4d>) -> Vec<Vec3f> {
        let mut normals = Vec::with_capacity(6);

        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, 1.0), transform); // Front
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, -1.0), transform); // Back
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 1.0, 0.0), transform); // Right
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, -1.0, 0.0), transform); // Left
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(1.0, 0.0, 0.0), transform); // Top
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(-1.0, 0.0, 0.0), transform); // Bottom

        normals
    }

    /// Generate normals for a cuboid (f64 version)
    pub fn generate_normals_f64(transform: Option<&Matrix4d>) -> Vec<Vec3d> {
        let mut normals = Vec::with_capacity(6);

        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, 1.0), transform);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, -1.0), transform);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 1.0, 0.0), transform);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, -1.0, 0.0), transform);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(1.0, 0.0, 0.0), transform);
        vec3d_helpers::write_dir(&mut normals, Vec3d::new(-1.0, 0.0, 0.0), transform);

        normals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_num_points() {
        assert_eq!(CuboidMeshGenerator::compute_num_points(), 8);
    }

    #[test]
    fn test_compute_num_normals() {
        assert_eq!(CuboidMeshGenerator::compute_num_normals(), 6);
    }

    #[test]
    fn test_normals_interpolation() {
        assert_eq!(
            CuboidMeshGenerator::normals_interpolation(),
            InterpolationTokens::UNIFORM
        );
    }

    #[test]
    fn test_generate_topology() {
        let topo = CuboidMeshGenerator::generate_topology();
        let counts = topo.face_vertex_counts();
        let indices = topo.face_vertex_indices();

        // 6 faces
        assert_eq!(counts.len(), 6);
        // All quads
        assert!(counts.iter().all(|&c| c == 4));
        // 6 faces * 4 vertices each
        assert_eq!(indices.len(), 24);
    }

    #[test]
    fn test_generate_points_f32() {
        let points = CuboidMeshGenerator::generate_points_f32(2.0, 3.0, 4.0, None);
        assert_eq!(points.len(), 8);

        // Check that we have points at +/- half dimensions
        assert_eq!(points[0], Vec3f::new(1.0, 1.5, 2.0));
        assert_eq!(points[1], Vec3f::new(-1.0, 1.5, 2.0));
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = CuboidMeshGenerator::generate_normals_f32(None);
        assert_eq!(normals.len(), 6);

        // Check first normal (front face)
        assert_eq!(normals[0], Vec3f::new(0.0, 0.0, 1.0));

        // All normals should be normalized
        for normal in normals {
            let length = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
            assert!((length - 1.0).abs() < 1e-6);
        }
    }
}
