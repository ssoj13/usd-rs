// Capsule mesh generator

use super::mesh_generator::{
    CapStyle, MeshScalar, compute_num_capped_quad_topology_points, generate_capped_quad_topology,
    generate_unit_arc_xy, lerp, vec3d_helpers, vec3f_helpers,
};
use super::tokens::InterpolationTokens;
use std::f32::consts::PI as PI_F32;
use std::f64::consts::PI as PI_F64;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::MeshTopology;

/// Capsule mesh generator
///
/// Generates a capsule (cylinder capped by two hemispheres) centered at the origin.
/// The height is aligned with the Z axis and represents just the cylindrical portion.
/// Supports different radii for top and bottom caps.
pub struct CapsuleMeshGenerator;

impl CapsuleMeshGenerator {
    /// Minimum radial segments (forms triangle-like cross section)
    pub const MIN_NUM_RADIAL: usize = 3;

    /// Minimum axial divisions per hemisphere cap
    pub const MIN_NUM_CAP_AXIAL: usize = 1;

    /// Compute number of points for a capsule
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments
    /// * `num_cap_axial` - Number of axial divisions per cap
    /// * `closed_sweep` - Whether sweep is 360 degrees
    pub fn compute_num_points(
        num_radial: usize,
        num_cap_axial: usize,
        closed_sweep: bool,
    ) -> usize {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return 0;
        }

        compute_num_capped_quad_topology_points(
            num_radial,
            (2 * (num_cap_axial - 1)) + 1, // numQuadStrips
            CapStyle::SharedEdge,          // bottomCapStyle
            CapStyle::SharedEdge,          // topCapStyle
            closed_sweep,
        )
    }

    /// Compute number of normals (same as points for vertex interpolation)
    pub fn compute_num_normals(
        num_radial: usize,
        num_cap_axial: usize,
        closed_sweep: bool,
    ) -> usize {
        Self::compute_num_points(num_radial, num_cap_axial, closed_sweep)
    }

    /// Get normals interpolation mode
    pub fn normals_interpolation() -> &'static str {
        InterpolationTokens::VERTEX
    }

    /// Generate topology for a capsule
    pub fn generate_topology(
        num_radial: usize,
        num_cap_axial: usize,
        closed_sweep: bool,
    ) -> MeshTopology {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return MeshTopology::default();
        }

        generate_capped_quad_topology(
            num_radial,
            (2 * (num_cap_axial - 1)) + 1,
            CapStyle::SharedEdge,
            CapStyle::SharedEdge,
            closed_sweep,
        )
    }

    /// Compute number of axial divisions for bottom cap
    fn compute_num_bottom_cap_axial<T: MeshScalar>(
        num_cap_axial: usize,
        latitude_range: T,
    ) -> usize {
        // Distribute axial divisions proportionally to the sphere portion
        let pi = T::from_f64(std::f64::consts::PI);
        let half_pi = T::from_f64(std::f64::consts::PI * 0.5);
        let two = T::from_f64(2.0);

        let result = ((two * T::from_f64(num_cap_axial as f64)) * (half_pi + latitude_range) / pi)
            .round()
            .to_f64() as usize;

        // Clamp to ensure top cap has at least MIN_NUM_CAP_AXIAL
        let max_num = (2 * num_cap_axial) - Self::MIN_NUM_CAP_AXIAL;
        result.max(Self::MIN_NUM_CAP_AXIAL).min(max_num)
    }

    /// Compute number of axial divisions for top cap
    fn compute_num_top_cap_axial(num_cap_axial: usize, num_bottom_cap_axial: usize) -> usize {
        (2 * num_cap_axial) - num_bottom_cap_axial
    }

    /// Generate points for a capsule (f32 version)
    ///
    /// # Arguments
    /// * `num_radial` - Number of radial segments
    /// * `num_cap_axial` - Number of axial divisions per cap
    /// * `bottom_radius` - Bottom cap radius
    /// * `top_radius` - Top cap radius
    /// * `height` - Cylindrical portion height
    /// * `sweep_degrees` - Sweep angle (360 for complete capsule)
    /// * `transform` - Optional transform matrix
    pub fn generate_points_f32(
        num_radial: usize,
        num_cap_axial: usize,
        bottom_radius: f32,
        top_radius: f32,
        height: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, num_cap_axial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        // Calculate sphere parameters for different radii
        let mut offset0 = 0.0f32;
        let mut offset1 = 0.0f32;
        let mut radius0 = bottom_radius;
        let mut radius1 = top_radius;
        let mut latitude_range = 0.0f32;

        if (bottom_radius - top_radius).abs() > 1e-6 && height != 0.0 {
            // Calculate adjustments for tangent continuity (see C++ comments)
            let slope = (bottom_radius - top_radius) / height;
            offset0 = -(slope * bottom_radius);
            offset1 = -(slope * top_radius);
            radius0 = (bottom_radius * bottom_radius + offset0 * offset0).sqrt();
            radius1 = (top_radius * top_radius + offset1 * offset1).sqrt();
            latitude_range = slope.atan();
        }

        offset0 -= 0.5 * height;
        offset1 += 0.5 * height;

        let num_cap_axial0 = Self::compute_num_bottom_cap_axial(num_cap_axial, latitude_range);
        let num_cap_axial1 = Self::compute_num_top_cap_axial(num_cap_axial, num_cap_axial0);

        // Bottom pole
        vec3f_helpers::write_point(
            &mut points,
            Vec3f::new(0.0, 0.0, offset0 - radius0),
            transform,
        );

        // Bottom hemisphere latitude rings
        for ax_idx in 1..=num_cap_axial0 {
            let lat_angle = lerp(
                (ax_idx as f32) / (num_cap_axial0 as f32),
                -0.5 * PI_F32,
                latitude_range,
            );
            let rad_scale = radius0 * lat_angle.cos();
            let latitude = offset0 + radius0 * lat_angle.sin();

            vec3f_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        // Top hemisphere latitude rings
        for ax_idx in 0..num_cap_axial1 {
            let lat_angle = lerp(
                (ax_idx as f32) / (num_cap_axial1 as f32),
                latitude_range,
                0.5 * PI_F32,
            );
            let rad_scale = radius1 * lat_angle.cos();
            let latitude = offset1 + radius1 * lat_angle.sin();

            vec3f_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        // Top pole
        vec3f_helpers::write_point(
            &mut points,
            Vec3f::new(0.0, 0.0, offset1 + radius1),
            transform,
        );

        points
    }

    /// Generate points for a capsule (f64 version)
    pub fn generate_points_f64(
        num_radial: usize,
        num_cap_axial: usize,
        bottom_radius: f64,
        top_radius: f64,
        height: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_points = Self::compute_num_points(num_radial, num_cap_axial, closed_sweep);
        let mut points = Vec::with_capacity(num_points);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        let mut offset0 = 0.0f64;
        let mut offset1 = 0.0f64;
        let mut radius0 = bottom_radius;
        let mut radius1 = top_radius;
        let mut latitude_range = 0.0f64;

        if (bottom_radius - top_radius).abs() > 1e-6 && height != 0.0 {
            let slope = (bottom_radius - top_radius) / height;
            offset0 = -(slope * bottom_radius);
            offset1 = -(slope * top_radius);
            radius0 = (bottom_radius * bottom_radius + offset0 * offset0).sqrt();
            radius1 = (top_radius * top_radius + offset1 * offset1).sqrt();
            latitude_range = slope.atan();
        }

        offset0 -= 0.5 * height;
        offset1 += 0.5 * height;

        let num_cap_axial0 = Self::compute_num_bottom_cap_axial(num_cap_axial, latitude_range);
        let num_cap_axial1 = Self::compute_num_top_cap_axial(num_cap_axial, num_cap_axial0);

        vec3d_helpers::write_point(
            &mut points,
            Vec3d::new(0.0, 0.0, offset0 - radius0),
            transform,
        );

        for ax_idx in 1..=num_cap_axial0 {
            let lat_angle = lerp(
                (ax_idx as f64) / (num_cap_axial0 as f64),
                -0.5 * PI_F64,
                latitude_range,
            );
            let rad_scale = radius0 * lat_angle.cos();
            let latitude = offset0 + radius0 * lat_angle.sin();

            vec3d_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        for ax_idx in 0..num_cap_axial1 {
            let lat_angle = lerp(
                (ax_idx as f64) / (num_cap_axial1 as f64),
                latitude_range,
                0.5 * PI_F64,
            );
            let rad_scale = radius1 * lat_angle.cos();
            let latitude = offset1 + radius1 * lat_angle.sin();

            vec3d_helpers::write_arc(&mut points, rad_scale, &ring_xy, latitude, transform);
        }

        vec3d_helpers::write_point(
            &mut points,
            Vec3d::new(0.0, 0.0, offset1 + radius1),
            transform,
        );

        points
    }

    /// Generate normals for a capsule (f32 version)
    pub fn generate_normals_f32(
        num_radial: usize,
        num_cap_axial: usize,
        bottom_radius: f32,
        top_radius: f32,
        height: f32,
        sweep_degrees: f32,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3f> {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_normals = Self::compute_num_normals(num_radial, num_cap_axial, closed_sweep);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        let mut latitude_range = 0.0f32;
        if (bottom_radius - top_radius).abs() > 1e-6 && height != 0.0 {
            let slope = (bottom_radius - top_radius) / height;
            latitude_range = slope.atan();
        }

        let num_cap_axial0 = Self::compute_num_bottom_cap_axial(num_cap_axial, latitude_range);
        let num_cap_axial1 = Self::compute_num_top_cap_axial(num_cap_axial, num_cap_axial0);

        // Bottom pole normal
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, -1.0), transform);

        // Bottom hemisphere normals
        for ax_idx in 1..=num_cap_axial0 {
            let lat_angle = lerp(
                (ax_idx as f32) / (num_cap_axial0 as f32),
                -0.5 * PI_F32,
                latitude_range,
            );
            let rad_scale = lat_angle.cos();
            let latitude = lat_angle.sin();

            vec3f_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        }

        // Top hemisphere normals
        for ax_idx in 0..num_cap_axial1 {
            let lat_angle = lerp(
                (ax_idx as f32) / (num_cap_axial1 as f32),
                latitude_range,
                0.5 * PI_F32,
            );
            let rad_scale = lat_angle.cos();
            let latitude = lat_angle.sin();

            vec3f_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        }

        // Top pole normal
        vec3f_helpers::write_dir(&mut normals, Vec3f::new(0.0, 0.0, 1.0), transform);

        normals
    }

    /// Generate normals for a capsule (f64 version)
    pub fn generate_normals_f64(
        num_radial: usize,
        num_cap_axial: usize,
        bottom_radius: f64,
        top_radius: f64,
        height: f64,
        sweep_degrees: f64,
        transform: Option<&Matrix4d>,
    ) -> Vec<Vec3d> {
        if num_radial < Self::MIN_NUM_RADIAL || num_cap_axial < Self::MIN_NUM_CAP_AXIAL {
            return Vec::new();
        }

        let closed_sweep = (sweep_degrees - 360.0).abs() < 1e-6;
        let num_normals = Self::compute_num_normals(num_radial, num_cap_axial, closed_sweep);
        let mut normals = Vec::with_capacity(num_normals);

        let ring_xy = generate_unit_arc_xy(num_radial, sweep_degrees);

        let mut latitude_range = 0.0f64;
        if (bottom_radius - top_radius).abs() > 1e-6 && height != 0.0 {
            let slope = (bottom_radius - top_radius) / height;
            latitude_range = slope.atan();
        }

        let num_cap_axial0 = Self::compute_num_bottom_cap_axial(num_cap_axial, latitude_range);
        let num_cap_axial1 = Self::compute_num_top_cap_axial(num_cap_axial, num_cap_axial0);

        vec3d_helpers::write_dir(&mut normals, Vec3d::new(0.0, 0.0, -1.0), transform);

        for ax_idx in 1..=num_cap_axial0 {
            let lat_angle = lerp(
                (ax_idx as f64) / (num_cap_axial0 as f64),
                -0.5 * PI_F64,
                latitude_range,
            );
            let rad_scale = lat_angle.cos();
            let latitude = lat_angle.sin();

            vec3d_helpers::write_arc_dir(&mut normals, rad_scale, &ring_xy, latitude, transform);
        }

        for ax_idx in 0..num_cap_axial1 {
            let lat_angle = lerp(
                (ax_idx as f64) / (num_cap_axial1 as f64),
                latitude_range,
                0.5 * PI_F64,
            );
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
        let count = CapsuleMeshGenerator::compute_num_points(8, 4, true);
        // Should be 2 poles + rings for two hemisphere caps + cylindrical section
        assert!(count > 0);
    }

    #[test]
    fn test_min_constraints() {
        assert_eq!(CapsuleMeshGenerator::compute_num_points(2, 4, true), 0);
        assert_eq!(CapsuleMeshGenerator::compute_num_points(8, 0, true), 0);
    }

    #[test]
    fn test_generate_points_f32() {
        let points = CapsuleMeshGenerator::generate_points_f32(8, 4, 1.0, 1.0, 2.0, 360.0, None);
        let expected = CapsuleMeshGenerator::compute_num_points(8, 4, true);
        assert_eq!(points.len(), expected);
    }

    #[test]
    fn test_generate_normals_f32() {
        let normals = CapsuleMeshGenerator::generate_normals_f32(8, 4, 1.0, 1.0, 2.0, 360.0, None);
        let expected = CapsuleMeshGenerator::compute_num_normals(8, 4, true);
        assert_eq!(normals.len(), expected);

        // All normals should be normalized
        for normal in &normals {
            let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_different_radii() {
        // Test with different top and bottom radii
        let points = CapsuleMeshGenerator::generate_points_f32(8, 4, 1.0, 0.5, 2.0, 360.0, None);
        assert!(points.len() > 0);
    }
}
