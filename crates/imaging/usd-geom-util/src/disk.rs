// Disk mesh generator

use super::mesh_generator::{
    CapStyle, compute_num_capped_quad_topology_points, generate_capped_quad_topology,
    generate_unit_arc_xy, vec3d_helpers, vec3f_helpers,
};
use super::tokens::InterpolationTokens;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::MeshTopology;

/// Disk mesh generator
///
/// Generates a circular disk in the XY plane, centered at the origin.
/// The disk is a triangle fan from center to edge.
pub struct DiskMeshGenerator;

impl DiskMeshGenerator {
    /// Minimum radial segments (forms triangle)
    pub const MIN_NUM_RADIAL: usize = 3;

    /// Compute number of points for a disk
    pub fn compute_num_points(num_radial: usize, closed_sweep: bool) -> usize {
        if num_radial < Self::MIN_NUM_RADIAL {
            return 0;
        }

        compute_num_capped_quad_topology_points(
            num_radial,
            0,                      // numQuadStrips (no quad strips, just a triangle fan)
            CapStyle::None,         // bottomCapStyle
            CapStyle::SeparateEdge, // topCapStyle (the disk itself)
            closed_sweep,
        )
    }

    /// Compute number of normals for a disk
    pub fn compute_num_normals() -> usize {
        // Single normal for entire disk (constant interpolation)
        1
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::CONSTANT
    }

    /// Generate topology for a disk
    pub fn generate_topology(num_radial: usize, closed_sweep: bool) -> MeshTopology {
        if num_radial < Self::MIN_NUM_RADIAL {
            return MeshTopology::default();
        }

        generate_capped_quad_topology(
            num_radial,
            0, // No quad strips
            CapStyle::None,
            CapStyle::SeparateEdge,
            closed_sweep,
        )
    }

    /// Generate points for a disk (f32 version)
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments
    /// * `radius` - Disk radius
    /// * `sweep_degrees` - Sweep angle (360 for complete disk)
    /// * `transform` - Optional transform matrix
    pub fn generate_points_f32(
        num_radial: usize,
        radius: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        // Outer edge ring
        vec3f_helpers::write_arc(&mut points, radius, &ring_xy, 0.0, transform);

        // Center point
        vec3f_helpers::write_point(&mut points, Vec3f::new(0.0, 0.0, 0.0), transform);

        points
    }

    /// Generate points for a disk (f64 version)
    pub fn generate_points_f64(
        num_radial: usize,
        radius: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        vec3d_helpers::write_arc(&mut points, radius, &ring_xy, 0.0, transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(0.0, 0.0, 0.0), transform);

        points
    }

    /// Generate normals for a disk (f32 version)
    ///
    /// Returns a single normal pointing in +Z direction (constant interpolation).
    pub fn generate_normals_f32(transform: Option<&Matrix4d>) -> Vec<Vec3f> {
        let mut normals = Vec::with_capacity(1);
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, 1.0), transform);
        normals
    }

    /// Generate normals for a disk (f64 version)
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
        let count = DiskMeshGenerator::compute_num_points(8, true);
        // 8 edge points + 1 center point
        assert_eq!(count, 9);
    }

    #[test]
    fn test_compute_num_normals() {
        assert_eq!(DiskMeshGenerator::compute_num_normals(), 1);
    }

    #[test]
    fn test_min_constraints() {
        assert_eq!(DiskMeshGenerator::compute_num_points(2, true), 0);
    }

    #[test]
    fn test_normals_interpolation() {
        assert_eq!(
            DiskMeshGenerator::normals_interpolation(),
            InterpolationTokens::CONSTANT
        );
    }

    #[test]
    fn test_generate_points_f32() {
        let points = DiskMeshGenerator::generate_points_f32(8, 2.0, 360.0, None);
        let expected = DiskMeshGenerator::compute_num_points(8, true);
        assert_eq!(points.len(), expected);

        // Last point should be center
        assert_eq!(points[points.len() - 1], Vec3f::new(0.0, 0.0, 0.0));

        // First edge point should be at (radius, 0, 0)
        let first = points[0];
        assert!((first.x - 2.0).abs() < 1e-6);
        assert!(first.y.abs() < 1e-6);
        assert!(first.z.abs() < 1e-6);
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = DiskMeshGenerator::generate_normals_f32(None);
        assert_eq!(normals.len(), 1);
        assert_eq!(normals[0], Vec3f::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_generate_topology() {
        let topo = DiskMeshGenerator::generate_topology(8, true);
        let counts = topo.face_vertex_counts();

        // 8 triangles forming a fan
        assert_eq!(counts.len(), 8);
        assert!(counts.iter().all(|&c| c == 3));
    }
}
