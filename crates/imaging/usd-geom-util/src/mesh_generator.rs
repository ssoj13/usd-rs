// Base mesh generator functionality and common topology helpers

use std::f64::consts::PI;
use usd_gf::{Matrix4d, Vec3d, Vec3f};
use usd_px_osd::{MeshTopology, tokens as px_osd_tokens};

/// Cap style for capped quad topology generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapStyle {
    /// No cap - topology ends without a triangle fan
    None,
    /// Cap with shared edge - triangle fan shares vertices with adjacent quad strip
    SharedEdge,
    /// Cap with separate edge - triangle fan has its own ring of vertices
    SeparateEdge,
}

/// Helper trait for types that can be used as mesh scalars (f32, f64)
pub trait MeshScalar: Copy + num_traits::Float + 'static {
    /// Convert f64 value to Self
    fn from_f64(v: f64) -> Self;

    /// Convert self to f64
    fn to_f64(self) -> f64;
}

impl MeshScalar for f32 {
    /// Convert f64 to f32
    #[inline]
    fn from_f64(v: f64) -> Self {
        v as f32
    }

    /// Convert f32 to f64
    #[inline]
    fn to_f64(self) -> f64 {
        self as f64
    }
}

impl MeshScalar for f64 {
    /// Convert f64 to f64 (identity)
    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }

    /// Convert f64 to f64 (identity)
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
}

/// Convert degrees to radians
#[inline]
pub fn degrees_to_radians<T: MeshScalar>(degrees: T) -> T {
    T::from_f64(degrees.to_f64() * PI / 180.0)
}

/// Clamp value between min and max
#[inline]
pub fn clamp<T: MeshScalar>(v: T, min: T, max: T) -> T {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Linear interpolation between a and b
#[inline]
pub fn lerp<T: MeshScalar>(t: T, a: T, b: T) -> T {
    a + t * (b - a)
}

/// Check if two values are close within tolerance
#[inline]
pub fn is_close<T: MeshScalar>(a: T, b: T, tolerance: T) -> bool {
    (a - b).abs() < tolerance
}

/// Compute number of radial points based on sweep closure
///
/// For a closed ring (360 degrees), first and last points are the same topologically,
/// so we only need numRadial points. For an open arc, we need numRadial + 1 points.
pub fn compute_num_radial_points(num_radial: usize, closed_sweep: bool) -> usize {
    if closed_sweep {
        num_radial
    } else {
        num_radial + 1
    }
}

/// Compute total number of points for capped quad topology
///
/// This topology consists of:
/// - Optional bottom triangle fan (cap)
/// - One or more quad strips forming rings
/// - Optional top triangle fan (cap)
pub fn compute_num_capped_quad_topology_points(
    num_radial: usize,
    num_quad_strips: usize,
    bottom_cap_style: CapStyle,
    top_cap_style: CapStyle,
    closed_sweep: bool,
) -> usize {
    let num_radial_pts = compute_num_radial_points(num_radial, closed_sweep);

    // Base points for quad strips (one ring per strip edge, plus one for the final edge)
    let mut result = num_radial_pts * (num_quad_strips + 1);

    // Add bottom cap points
    if bottom_cap_style != CapStyle::None {
        // Pole point
        result += 1;
        if bottom_cap_style == CapStyle::SeparateEdge {
            // Additional ring for separate edge
            result += num_radial_pts;
        }
    }

    // Add top cap points
    if top_cap_style != CapStyle::None {
        // Pole point
        result += 1;
        if num_quad_strips > 0 && top_cap_style == CapStyle::SeparateEdge {
            // Additional ring for separate edge
            result += num_radial_pts;
        }
    }

    result
}

/// Generate topology for capped quad primitive
///
/// Creates a mesh topology consisting of optional triangle fans at bottom/top
/// and quad strips connecting them. This pattern is used by sphere, cylinder,
/// cone, capsule, and disk generators.
pub fn generate_capped_quad_topology(
    num_radial: usize,
    num_quad_strips: usize,
    bottom_cap_style: CapStyle,
    top_cap_style: CapStyle,
    closed_sweep: bool,
) -> MeshTopology {
    let num_tri_strips = (if bottom_cap_style != CapStyle::None {
        1
    } else {
        0
    }) + (if top_cap_style != CapStyle::None {
        1
    } else {
        0
    });
    let num_tris = num_tri_strips * num_radial;
    let num_quads = num_quad_strips * num_radial;

    let mut counts = Vec::with_capacity(num_quads + num_tris);
    let mut indices = Vec::with_capacity((4 * num_quads) + (3 * num_tris));

    let num_radial_pts = compute_num_radial_points(num_radial, closed_sweep);
    let mut pt_idx: usize = 0;

    // Bottom triangle fan
    if bottom_cap_style != CapStyle::None {
        let bottom_pt_idx = pt_idx;
        pt_idx += 1;

        for rad_idx in 0..num_radial {
            counts.push(3);
            indices.push((pt_idx + ((rad_idx + 1) % num_radial_pts)) as i32);
            indices.push((pt_idx + rad_idx) as i32);
            indices.push(bottom_pt_idx as i32);
        }

        // Adjust if edge is not shared
        if bottom_cap_style == CapStyle::SeparateEdge {
            pt_idx += num_radial_pts;
        }
    }

    // Middle quads
    for _strip_idx in 0..num_quad_strips {
        for rad_idx in 0..num_radial {
            counts.push(4);
            indices.push((pt_idx + rad_idx) as i32);
            indices.push((pt_idx + ((rad_idx + 1) % num_radial_pts)) as i32);
            indices.push((pt_idx + ((rad_idx + 1) % num_radial_pts) + num_radial_pts) as i32);
            indices.push((pt_idx + rad_idx + num_radial_pts) as i32);
        }
        pt_idx += num_radial_pts;
    }

    // Top triangle fan
    if top_cap_style != CapStyle::None {
        // Adjust if edge is not shared
        if num_quad_strips > 0 && top_cap_style == CapStyle::SeparateEdge {
            pt_idx += num_radial_pts;
        }

        let top_pt_idx = pt_idx + num_radial_pts;
        for rad_idx in 0..num_radial {
            counts.push(3);
            indices.push((pt_idx + rad_idx) as i32);
            indices.push((pt_idx + ((rad_idx + 1) % num_radial_pts)) as i32);
            indices.push(top_pt_idx as i32);
        }
    }

    MeshTopology::new(
        (*px_osd_tokens::CATMULL_CLARK).clone(),
        (*px_osd_tokens::RIGHT_HANDED).clone(),
        counts,
        indices,
    )
}

/// Generate unit circular arc in XY plane
///
/// Returns a vector of [x, y] pairs forming a circular arc of unit radius.
/// For a closed sweep (360 degrees), the first and last points are NOT duplicated.
pub fn generate_unit_arc_xy<T: MeshScalar>(num_radial: usize, sweep_degrees: T) -> Vec<[T; 2]> {
    let two_pi = T::from_f64(2.0 * PI);
    let sweep_radians = degrees_to_radians(sweep_degrees);
    let sweep = clamp(sweep_radians, -two_pi, two_pi);
    let closed_sweep = is_close(sweep.abs(), two_pi, T::from_f64(1e-6));
    let num_pts = compute_num_radial_points(num_radial, closed_sweep);

    let mut result = Vec::with_capacity(num_pts);
    for rad_idx in 0..num_pts {
        // Longitude range: [0, sweep]
        let long_angle = (T::from_f64(rad_idx as f64) / T::from_f64(num_radial as f64)) * sweep;
        result.push([long_angle.cos(), long_angle.sin()]);
    }

    result
}

/// Helper functions for writing points with transforms - f32 version
pub mod vec3f_helpers {
    use super::*;

    /// Write a single point, optionally transforming
    pub fn write_point(points: &mut Vec<Vec3f>, pt: Vec3f, transform: Option<&Matrix4d>) {
        if let Some(xform) = transform {
            let pt_d = Vec3d::new(pt.x as f64, pt.y as f64, pt.z as f64);
            let transformed = xform.transform_point(&pt_d);
            points.push(Vec3f::new(
                transformed.x as f32,
                transformed.y as f32,
                transformed.z as f32,
            ));
        } else {
            points.push(pt);
        }
    }

    /// Write points from arc data
    pub fn write_arc(
        points: &mut Vec<Vec3f>,
        scale_xy: f32,
        arc_xy: &[[f32; 2]],
        arc_z: f32,
        transform: Option<&Matrix4d>,
    ) {
        for xy in arc_xy {
            let pt = Vec3f::new(scale_xy * xy[0], scale_xy * xy[1], arc_z);
            write_point(points, pt, transform);
        }
    }

    /// Write a single direction vector, optionally transforming
    pub fn write_dir(dirs: &mut Vec<Vec3f>, dir: Vec3f, transform: Option<&Matrix4d>) {
        if let Some(xform) = transform {
            let dir_d = Vec3d::new(dir.x as f64, dir.y as f64, dir.z as f64);
            let transformed = xform.transform_dir(&dir_d);
            dirs.push(Vec3f::new(
                transformed.x as f32,
                transformed.y as f32,
                transformed.z as f32,
            ));
        } else {
            dirs.push(dir);
        }
    }

    /// Write direction vectors from arc data
    pub fn write_arc_dir(
        dirs: &mut Vec<Vec3f>,
        scale_xy: f32,
        arc_xy: &[[f32; 2]],
        arc_z: f32,
        transform: Option<&Matrix4d>,
    ) {
        for xy in arc_xy {
            let dir = Vec3f::new(scale_xy * xy[0], scale_xy * xy[1], arc_z);
            write_dir(dirs, dir, transform);
        }
    }
}

/// Helper functions for writing points with transforms - f64 version
pub mod vec3d_helpers {
    use super::*;

    /// Write a single point, optionally transforming
    pub fn write_point(points: &mut Vec<Vec3d>, pt: Vec3d, transform: Option<&Matrix4d>) {
        if let Some(xform) = transform {
            points.push(xform.transform_point(&pt));
        } else {
            points.push(pt);
        }
    }

    /// Write points from arc data
    pub fn write_arc(
        points: &mut Vec<Vec3d>,
        scale_xy: f64,
        arc_xy: &[[f64; 2]],
        arc_z: f64,
        transform: Option<&Matrix4d>,
    ) {
        for xy in arc_xy {
            let pt = Vec3d::new(scale_xy * xy[0], scale_xy * xy[1], arc_z);
            write_point(points, pt, transform);
        }
    }

    /// Write a single direction vector, optionally transforming
    pub fn write_dir(dirs: &mut Vec<Vec3d>, dir: Vec3d, transform: Option<&Matrix4d>) {
        if let Some(xform) = transform {
            dirs.push(xform.transform_dir(&dir));
        } else {
            dirs.push(dir);
        }
    }

    /// Write direction vectors from arc data
    pub fn write_arc_dir(
        dirs: &mut Vec<Vec3d>,
        scale_xy: f64,
        arc_xy: &[[f64; 2]],
        arc_z: f64,
        transform: Option<&Matrix4d>,
    ) {
        for xy in arc_xy {
            let dir = Vec3d::new(scale_xy * xy[0], scale_xy * xy[1], arc_z);
            write_dir(dirs, dir, transform);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_degrees_to_radians() {
        let rad = degrees_to_radians(180.0f32);
        assert!((rad - std::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn test_compute_num_radial_points() {
        assert_eq!(compute_num_radial_points(8, true), 8);
        assert_eq!(compute_num_radial_points(8, false), 9);
    }

    #[test]
    fn test_generate_unit_arc_xy() {
        let arc = generate_unit_arc_xy::<f32>(4, 360.0);
        assert_eq!(arc.len(), 4);

        // First point should be at (1, 0)
        assert!((arc[0][0] - 1.0).abs() < 1e-6);
        assert!(arc[0][1].abs() < 1e-6);
    }

    #[test]
    fn test_compute_num_capped_quad_points() {
        // Sphere: bottom and top shared edge caps
        let count = compute_num_capped_quad_topology_points(
            8,
            2,
            CapStyle::SharedEdge,
            CapStyle::SharedEdge,
            true,
        );
        // 2 pole points + 3 rings of 8 points
        assert_eq!(count, 2 + 3 * 8);
    }
}
