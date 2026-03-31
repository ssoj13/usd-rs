// Sphere mesh generator

use super::mesh_generator::{
    CapStyle, compute_num_capped_quad_topology_points, generate_capped_quad_topology,
    generate_unit_arc_xy, vec3d_helpers, vec3f_helpers,
};
use super::tokens::InterpolationTokens;
use std::f32::consts::PI as PI_F32;
use std::f64::consts::PI as PI_F64;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::MeshTopology;

/// Sphere mesh generator
///
/// Generates a UV sphere with circular cross-sections in the XY plane,
/// centered at the origin. The sphere is made up of latitude rings
/// with vertices at numAxial locations along the Z axis.
pub struct SphereMeshGenerator;

impl SphereMeshGenerator {
    /// Minimum number of radial segments (longitude divisions)
    pub const MIN_NUM_RADIAL: usize = 3;

    /// Minimum number of axial divisions (latitude divisions)
    pub const MIN_NUM_AXIAL: usize = 2;

    /// Compute number of points for a sphere
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments (longitude)
    /// * `num_axial` - Number of axial divisions (latitude)
    /// * `closed_sweep` - Whether sweep is 360 degrees
    pub fn compute_num_points(num_radial: usize, num_axial: usize, closed_sweep: bool) -> usize {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return 0;
        }

        compute_num_capped_quad_topology_points(
            num_radial,
            num_axial - 2,        // numQuadStrips
            CapStyle::SharedEdge, // bottomCapStyle
            CapStyle::SharedEdge, // topCapStyle
            closed_sweep,
        )
    }

    /// Compute number of normals (same as points for vertex interpolation)
    pub fn compute_num_normals(num_radial: usize, num_axial: usize, closed_sweep: bool) -> usize {
        Self::compute_num_points(num_radial, num_axial, closed_sweep)
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::VERTEX
    }

    /// Generate topology for a sphere
    pub fn generate_topology(
        num_radial: usize,
        num_axial: usize,
        closed_sweep: bool,
    ) -> MeshTopology {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return MeshTopology::default();
        }

        generate_capped_quad_topology(
            num_radial,
            num_axial - 2,
            CapStyle::SharedEdge,
            CapStyle::SharedEdge,
            closed_sweep,
        )
    }

    /// Generate points for a sphere (f32 version)
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments
    /// * `num_axial` - Number of axial divisions
    /// * `radius` - Sphere radius
    /// * `sweep_degrees` - Sweep angle (360 for complete sphere)
    /// * `transform` - Optional transform matrix
    pub fn generate_points_f32(
        num_radial: usize,
        num_axial: usize,
        radius: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return Vec::new();
        }

        let num_points =
            Self::compute_num_points(num_radial, num_axial, (sweep_degrees - 360.0).abs() < 1e-6);
        let mut points = Vec::with_capacity(num_points);

        // Generate unit arc in XY plane
        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        // Bottom pole point
        vec3f_helpers::write_point(&mut points, Vec3f::new(0.0, 0.0, -radius), transform);

        // Latitude rings (excluding poles)
        for ax_idx in 1..num_axial {
            // Latitude range: (-0.5pi, 0.5pi)
            let lat_angle = ((ax_idx as f32 / num_axial as f32) - 0.5) * PI_F32;
            let rad_scale = radius * lat_angle.cos();
            let latitude = radius * lat_angle.sin();

            vec3f_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        // Top pole point
        vec3f_helpers::write_point(&mut points, Vec3f::new(0.0, 0.0, radius), transform);

        points
    }

    /// Generate points for a sphere (f64 version)
    pub fn generate_points_f64(
        num_radial: usize,
        num_axial: usize,
        radius: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return Vec::new();
        }

        let num_points =
            Self::compute_num_points(num_radial, num_axial, (sweep_degrees - 360.0).abs() < 1e-6);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        vec3d_helpers::write_point(&mut points, Vec3d::new(0.0, 0.0, -radius), transform);

        for ax_idx in 1..num_axial {
            let lat_angle = ((ax_idx as f64 / num_axial as f64) - 0.5) * PI_F64;
            let rad_scale = radius * lat_angle.cos();
            let latitude = radius * lat_angle.sin();

            vec3d_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        vec3d_helpers::write_point(&mut points, Vec3d::new(0.0, 0.0, radius), transform);

        points
    }

    /// Generate normals for a sphere (f32 version)
    ///
    /// Normals for a sphere are the same as the points when radius is 1.
    pub fn generate_normals_f32(
        num_radial: usize,
        num_axial: usize,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return Vec::new();
        }

        let num_normals =
            Self::compute_num_normals(num_radial, num_axial, (sweep_degrees - 360.0).abs() < 1e-6);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        // Bottom pole normal
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, -1.0), transform);

        // Latitude rings
        for ax_idx in 1..num_axial {
            let lat_angle = ((ax_idx as f32 / num_axial as f32) - 0.5) * PI_F32;
            let rad_scale = lat_angle.cos();
            let latitude = lat_angle.sin();

            vec3f_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        }

        // Top pole normal
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, 1.0), transform);

        normals
    }

    /// Generate normals for a sphere (f64 version)
    pub fn generate_normals_f64(
        num_radial: usize,
        num_axial: usize,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL || num_axial < Self::MIN_NUM_AXIAL {
            return Vec::new();
        }

        let num_normals =
            Self::compute_num_normals(num_radial, num_axial, (sweep_degrees - 360.0).abs() < 1e-6);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, -1.0), transform);

        for ax_idx in 1..num_axial {
            let lat_angle = ((ax_idx as f64 / num_axial as f64) - 0.5) * PI_F64;
            let rad_scale = lat_angle.cos();
            let latitude = lat_angle.sin();

            vec3d_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        }

        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, 1.0), transform);

        normals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_num_points() {
        // 4 radial x 4 axial closed sphere
        let count = SphereMeshGenerator::compute_num_points(4, 4, true);
        // 2 poles + 3 rings of 4 points (numAxial - 1)
        assert_eq!(count, 2 + 3 * 4);
    }

    #[test]
    fn test_min_constraints() {
        assert_eq!(SphereMeshGenerator::compute_num_points(2, 4, true), 0);
        assert_eq!(SphereMeshGenerator::compute_num_points(4, 1, true), 0);
    }

    #[test]
    fn test_generate_points_f32() {
        let points = SphereMeshGenerator::generate_points_f32(8, 6, 1.0, 360.0, None);
        let expected = SphereMeshGenerator::compute_num_points(8, 6, true);
        assert_eq!(points.len(), expected);

        // Bottom pole should be at (0, 0, -radius)
        assert_eq!(points[0], Vec3f::new(0.0, 0.0, -1.0));

        // Top pole should be at (0, 0, radius)
        assert_eq!(points[points.len() - 1], Vec3f::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = SphereMeshGenerator::generate_normals_f32(8, 6, 360.0, None);
        let expected = SphereMeshGenerator::compute_num_normals(8, 6, true);
        assert_eq!(normals.len(), expected);

        // All normals should be normalized
        for normal in &normals {
            let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-5,
                "Normal not normalized: length = {}",
                len
            );
        }
    }

    #[test]
    fn test_generate_topology() {
        let topo = SphereMeshGenerator::generate_topology(8, 6, true);
        let counts = topo.face_vertex_counts();
        let indices = topo.face_vertex_indices();

        // Should have triangles at poles and quads in middle
        // 2 triangle fans (8 tris each) + 4 quad strips (8 quads each)
        let num_tris = 2 * 8;
        let num_quads = 4 * 8;
        assert_eq!(counts.len(), num_tris + num_quads);

        // Verify index count
        assert_eq!(indices.len(), num_tris * 3 + num_quads * 4);
    }
}
