// Cylinder mesh generator

use super::mesh_generator::{
    CapStyle, compute_num_capped_quad_topology_points, generate_capped_quad_topology,
    generate_unit_arc_xy, vec3d_helpers, vec3f_helpers,
};
use super::tokens::InterpolationTokens;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::MeshTopology;

/// Cylinder mesh generator
///
/// Generates a cylinder with circular cross-sections in the XY plane,
/// centered at the origin with height aligned along Z axis.
/// Supports different top and bottom radii (truncated cone).
pub struct CylinderMeshGenerator;

impl CylinderMeshGenerator {
    /// Minimum number of radial segments for circular cross-section
    pub const MIN_NUM_RADIAL: usize = 3;

    /// Compute number of points for a cylinder
    pub fn compute_num_points(num_radial: usize, closed_sweep: bool) -> usize {
        if num_radial < Self::MIN_NUM_RADIAL {
            return 0;
        }

        compute_num_capped_quad_topology_points(
            num_radial,
            1,                      // numQuadStrips (one strip for cylinder body)
            CapStyle::SeparateEdge, // Bottom cap has separate edge for normals
            CapStyle::SeparateEdge, // Top cap has separate edge for normals
            closed_sweep,
        )
    }

    /// Compute number of normals (same as points for vertex interpolation)
    pub fn compute_num_normals(num_radial: usize, closed_sweep: bool) -> usize {
        Self::compute_num_points(num_radial, closed_sweep)
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::VERTEX
    }

    /// Generate topology for a cylinder
    pub fn generate_topology(num_radial: usize, closed_sweep: bool) -> MeshTopology {
        if num_radial < Self::MIN_NUM_RADIAL {
            return MeshTopology::default();
        }

        generate_capped_quad_topology(
            num_radial,
            1, // One quad strip
            CapStyle::SeparateEdge,
            CapStyle::SeparateEdge,
            closed_sweep,
        )
    }

    /// Generate points for a cylinder (f32 version)
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments
    /// * `bottom_radius` - Radius at bottom (Z = -height/2)
    /// * `top_radius` - Radius at top (Z = height/2)
    /// * `height` - Cylinder height
    /// * `sweep_degrees` - Sweep angle (360 for complete cylinder)
    /// * `transform` - Optional transform matrix
    pub fn generate_points_f32(
        num_radial: usize,
        bottom_radius: f32,
        top_radius: f32,
        height: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees.abs() - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);
        let z_max = 0.5 * height;
        let z_min = -z_max;

        // Bottom pole point
        vec3f_helpers::write_point(&mut points, Vec3f::new(0.0, 0.0, z_min), transform);

        // Two bottom rings at same Z (one for cap, one for cylinder body)
        vec3f_helpers::write_arc(&mut points, bottom_radius, &ring_xy, z_min, transform);
        vec3f_helpers::write_arc(&mut points, bottom_radius, &ring_xy, z_min, transform);

        // Two top rings at same Z
        vec3f_helpers::write_arc(&mut points, top_radius, &ring_xy, z_max, transform);
        vec3f_helpers::write_arc(&mut points, top_radius, &ring_xy, z_max, transform);

        // Top pole point
        vec3f_helpers::write_point(&mut points, Vec3f::new(0.0, 0.0, z_max), transform);

        points
    }

    /// Generate points for a cylinder (f64 version)
    pub fn generate_points_f64(
        num_radial: usize,
        bottom_radius: f64,
        top_radius: f64,
        height: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees.abs() - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);
        let z_max = 0.5 * height;
        let z_min = -z_max;

        vec3d_helpers::write_point(&mut points, Vec3d::new(0.0, 0.0, z_min), transform);
        vec3d_helpers::write_arc(&mut points, bottom_radius, &ring_xy, z_min, transform);
        vec3d_helpers::write_arc(&mut points, bottom_radius, &ring_xy, z_min, transform);
        vec3d_helpers::write_arc(&mut points, top_radius, &ring_xy, z_max, transform);
        vec3d_helpers::write_arc(&mut points, top_radius, &ring_xy, z_max, transform);
        vec3d_helpers::write_point(&mut points, Vec3d::new(0.0, 0.0, z_max), transform);

        points
    }

    /// Generate normals for a cylinder (f32 version)
    pub fn generate_normals_f32(
        num_radial: usize,
        bottom_radius: f32,
        top_radius: f32,
        height: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees.abs() - 360.0).abs() < 1e-6;
        let num_normals = Self::compute_num_normals(num_radial, closed_sweep);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        // Compute normals perpendicular to cylinder sides
        let (rad_scale, latitude) = if height != 0.0 {
            let slope = (bottom_radius - top_radius) / height;
            let rad_scale = 1.0 / (1.0 + slope * slope).sqrt();
            let latitude = slope * rad_scale;
            (rad_scale, latitude)
        } else {
            // Degenerate cylinder
            let latitude = if bottom_radius >= top_radius {
                1.0
            } else {
                -1.0
            };
            (0.0, latitude)
        };

        let base_normal = Vec3f::new(0.0, 0.0, -1.0);
        let top_normal = Vec3f::new(0.0, 0.0, 1.0);

        // Bottom pole normal
        vec3f_helpers::write_dir(&mut normals, base_normal, transform);

        // First bottom ring (part of base cap)
        for _ in 0..ring_xy.len() {
            vec3f_helpers::write_dir(&mut normals, base_normal, transform);
        }

        // Second bottom ring and first top ring (cylinder sides)
        vec3f_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        vec3f_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);

        // Second top ring (part of top cap)
        for _ in 0..ring_xy.len() {
            vec3f_helpers::write_dir(&mut normals, top_normal, transform);
        }

        // Top pole normal
        vec3f_helpers::write_dir(&mut normals, top_normal, transform);

        normals
    }

    /// Generate normals for a cylinder (f64 version)
    pub fn generate_normals_f64(
        num_radial: usize,
        bottom_radius: f64,
        top_radius: f64,
        height: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees.abs() - 360.0).abs() < 1e-6;
        let num_normals = Self::compute_num_normals(num_radial, closed_sweep);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        let (rad_scale, latitude) = if height != 0.0 {
            let slope = (bottom_radius - top_radius) / height;
            let rad_scale = 1.0 / (1.0 + slope * slope).sqrt();
            let latitude = slope * rad_scale;
            (rad_scale, latitude)
        } else {
            let latitude = if bottom_radius >= top_radius {
                1.0
            } else {
                -1.0
            };
            (0.0, latitude)
        };

        let base_normal = Vec3d::new(0.0, 0.0, -1.0);
        let top_normal = Vec3d::new(0.0, 0.0, 1.0);

        vec3d_helpers::write_dir(&mut normals, base_normal, transform);
        for _ in 0..ring_xy.len() {
            vec3d_helpers::write_dir(&mut normals, base_normal, transform);
        }
        vec3d_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        vec3d_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        for _ in 0..ring_xy.len() {
            vec3d_helpers::write_dir(&mut normals, top_normal, transform);
        }
        vec3d_helpers::write_dir(&mut normals, top_normal, transform);

        normals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_num_points() {
        let count = CylinderMeshGenerator::compute_num_points(8, true);
        // 2 poles + 4 rings of 8 points (2 bottom + 2 top)
        assert_eq!(count, 2 + 4 * 8);
    }

    #[test]
    fn test_min_constraints() {
        assert_eq!(CylinderMeshGenerator::compute_num_points(2, true), 0);
    }

    #[test]
    fn test_generate_points_f32() {
        let points = CylinderMeshGenerator::generate_points_f32(8, 1.0, 1.0, 2.0, 360.0, None);
        let expected = CylinderMeshGenerator::compute_num_points(8, true);
        assert_eq!(points.len(), expected);

        // Bottom pole at (0, 0, -1)
        assert_eq!(points[0], Vec3f::new(0.0, 0.0, -1.0));

        // Top pole at (0, 0, 1)
        assert_eq!(points[points.len() - 1], Vec3f::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = CylinderMeshGenerator::generate_normals_f32(8, 1.0, 1.0, 2.0, 360.0, None);
        let expected = CylinderMeshGenerator::compute_num_normals(8, true);
        assert_eq!(normals.len(), expected);

        // All normals should be normalized
        for normal in &normals {
            let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
        }
    }
}
