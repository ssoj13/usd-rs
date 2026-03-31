//! Graphics Foundation (gf) - Math utilities for USD.
//!
//! This module provides mathematical types and utilities used throughout USD:
//!
//! - Basic math functions (sqrt, sin, cos, lerp, clamp, etc.)
//! - Vector types (Vec2, Vec3, Vec4 in various precisions)
//! - Matrix types (Matrix2, Matrix3, Matrix4 in various precisions)
//! - Quaternion types
//! - Geometric primitives (Ray, Plane, BBox3d, Frustum)
//! - Color and color space utilities
//!
//! # CRITICAL: Row-Vector Convention (Imath / OpenUSD standard)
//!
//! All matrices follow the **row-vector** convention, matching C++ OpenUSD / Imath:
//!
//! - **Transform a point**: `v' = v * M` (point is a row vector on the LEFT)
//! - **Compose transforms**: `combined = local * parent` (left-to-right application order)
//! - **View-Projection**: `clip = point * (View * Proj)` — NOT `(Proj * View) * point`
//! - **Translation** is stored in **row 3**: `m[3][0], m[3][1], m[3][2]`
//! - **Storage** is row-major: `m[row][col]`
//!
//! This is the OPPOSITE of OpenGL/GLM column-vector convention (`v' = M * v`).
//! When projecting points to screen, use **columns** of the VP matrix:
//! ```text
//! clip_x = dot(point, column(0))  // NOT dot(row(0), point)
//! clip_y = dot(point, column(1))
//! clip_w = dot(point, column(3))
//! ```
//!
//! # Math Functions
//!
//! Basic math functions are provided for consistency with C++ OpenUSD:
//!
//! ```
//! use usd_gf::math::*;
//!
//! let angle_deg = radians_to_degrees(std::f64::consts::PI);
//! assert!((angle_deg - 180.0).abs() < 1e-10);
//!
//! let value = lerp(0.5, 0.0, 10.0);
//! assert!((value - 5.0).abs() < 1e-10);
//! ```

pub mod bbox3d;
pub mod camera;
pub mod color;
pub mod color_space;
pub mod dual_quat;
pub mod frustum;
pub mod gamma;
pub mod half;
pub mod homogeneous;
pub mod interval;
pub mod limits;
pub mod line;
pub mod line2d;
pub mod math;
pub mod matrix2;
pub mod matrix3;
pub mod matrix4;
pub mod multi_interval;
pub mod numeric_cast;
pub mod ostream_helpers;
pub mod plane;
pub mod quat;
pub mod quaternion;
pub mod range;
pub mod ray;
pub mod rect;
pub mod rotation;
pub mod size;
pub mod traits;
pub mod transform;
pub mod vec2;
pub mod vec3;
pub mod vec4;

pub use bbox3d::BBox3d;
pub use camera::{
    APERTURE_UNIT, Camera, CameraProjection, DEFAULT_HORIZONTAL_APERTURE,
    DEFAULT_VERTICAL_APERTURE, FOCAL_LENGTH_UNIT, FOVDirection,
};
pub use color::{Color, is_close as color_is_close};
pub use color_space::{ColorSpace, ColorSpaceName};
pub use dual_quat::{
    DualQuat, DualQuatd, DualQuatf, DualQuath, dual_quat_dot, dual_quatd, dual_quatf,
};
pub use frustum::{Frustum, ProjectionType};
pub use gamma::{
    DISPLAY_GAMMA, GammaCorrect, apply_gamma, display_to_linear, get_display_gamma,
    linear_to_display,
};
pub use half::{GfHalf, Half, hash_value as half_hash_value, print_bits as half_print_bits};
pub use homogeneous::{
    homogeneous_cross, homogeneous_cross_f, homogenize, homogenize_f, project, project_f,
};
pub use interval::Interval;
pub use limits::*;
pub use line::{
    Line, LineSeg, find_closest_points_line_line, find_closest_points_line_seg,
    find_closest_points_ray_line, find_closest_points_ray_line_seg, find_closest_points_seg_seg,
};
pub use line2d::{
    Line2d, LineSeg2d, find_closest_points_line2d_line2d, find_closest_points_line2d_seg2d,
    find_closest_points_seg2d_seg2d,
};
pub use math::{
    abs, acos, asin, atan, atan2, ceil, clamp, comp_div, comp_mult, cos, degrees_to_radians, dot,
    exp, floor, is_close, lerp, log, max, max3, max4, max5, min, min3, min4, min5, modulo,
    modulo_f32, pow, radians_to_degrees, round, sgn, sin, sin_cos, smooth_ramp, smooth_step, sqr,
    sqrt, tan,
};
pub use matrix2::{Matrix2, Matrix2d, Matrix2f, matrix2d, matrix2f};
pub use matrix3::{Matrix3, Matrix3d, Matrix3f, matrix3d, matrix3f};
pub use matrix4::{Matrix4, Matrix4d, Matrix4f, matrix4d, matrix4f};
pub use multi_interval::MultiInterval;
pub use plane::{Plane, fit_plane_to_points};
pub use quat::{Quat, Quatd, Quatf, Quath, dot as quat_dot, quatd, quatf, slerp as quat_slerp};
pub use quaternion::{Quaternion, dot as quaternion_dot, slerp as quaternion_slerp};
pub use range::{Range1, Range1d, Range1f, Range2, Range2d, Range2f, Range3, Range3d, Range3f};
pub use ray::Ray;
pub use rect::Rect2i;
pub use rotation::Rotation;
pub use size::{Size2, Size3};
pub use traits::*;
pub use transform::Transform;

/// Register all Gf types with the Tf type registry.
/// Called during initialization to break the tf<->gf circular dependency.
/// Matches C++ TF_REGISTRY_FUNCTION(TfType) in pxr/base/gf/*.cpp.
pub fn register_gf_types() {
    use usd_tf::register_type;
    register_type::<Half>("GfHalf", 2, true);
    register_type::<Vec2d>("GfVec2d", std::mem::size_of::<Vec2d>(), true);
    register_type::<Vec2f>("GfVec2f", std::mem::size_of::<Vec2f>(), true);
    register_type::<vec2::Vec2h>("GfVec2h", std::mem::size_of::<vec2::Vec2h>(), true);
    register_type::<vec2::Vec2i>("GfVec2i", std::mem::size_of::<vec2::Vec2i>(), true);
    register_type::<Vec3d>("GfVec3d", std::mem::size_of::<Vec3d>(), true);
    register_type::<Vec3f>("GfVec3f", std::mem::size_of::<Vec3f>(), true);
    register_type::<vec3::Vec3h>("GfVec3h", std::mem::size_of::<vec3::Vec3h>(), true);
    register_type::<vec3::Vec3i>("GfVec3i", std::mem::size_of::<vec3::Vec3i>(), true);
    register_type::<Vec4d>("GfVec4d", std::mem::size_of::<Vec4d>(), true);
    register_type::<Vec4f>("GfVec4f", std::mem::size_of::<Vec4f>(), true);
    register_type::<vec4::Vec4h>("GfVec4h", std::mem::size_of::<vec4::Vec4h>(), true);
    register_type::<vec4::Vec4i>("GfVec4i", std::mem::size_of::<vec4::Vec4i>(), true);
    register_type::<Matrix2d>("GfMatrix2d", std::mem::size_of::<Matrix2d>(), true);
    register_type::<Matrix2f>("GfMatrix2f", std::mem::size_of::<Matrix2f>(), true);
    register_type::<Matrix3d>("GfMatrix3d", std::mem::size_of::<Matrix3d>(), true);
    register_type::<Matrix3f>("GfMatrix3f", std::mem::size_of::<Matrix3f>(), true);
    register_type::<Matrix4d>("GfMatrix4d", std::mem::size_of::<Matrix4d>(), true);
    register_type::<Matrix4f>("GfMatrix4f", std::mem::size_of::<Matrix4f>(), true);
    register_type::<Quatd>("GfQuatd", std::mem::size_of::<Quatd>(), true);
    register_type::<Quatf>("GfQuatf", std::mem::size_of::<Quatf>(), true);
    register_type::<Quath>("GfQuath", std::mem::size_of::<Quath>(), true);
    register_type::<Quaternion>("GfQuaternion", std::mem::size_of::<Quaternion>(), true);
    register_type::<DualQuatd>("GfDualQuatd", std::mem::size_of::<DualQuatd>(), true);
    register_type::<DualQuatf>("GfDualQuatf", std::mem::size_of::<DualQuatf>(), true);
    register_type::<DualQuath>("GfDualQuath", std::mem::size_of::<DualQuath>(), true);
    register_type::<Range1d>("GfRange1d", std::mem::size_of::<Range1d>(), true);
    register_type::<Range1f>("GfRange1f", std::mem::size_of::<Range1f>(), true);
    register_type::<Range2d>("GfRange2d", std::mem::size_of::<Range2d>(), true);
    register_type::<Range2f>("GfRange2f", std::mem::size_of::<Range2f>(), true);
    register_type::<Range3d>("GfRange3d", std::mem::size_of::<Range3d>(), true);
    register_type::<Range3f>("GfRange3f", std::mem::size_of::<Range3f>(), true);
    register_type::<Rect2i>("GfRect2i", std::mem::size_of::<Rect2i>(), true);
    register_type::<Ray>("GfRay", std::mem::size_of::<Ray>(), true);
    register_type::<Plane>("GfPlane", std::mem::size_of::<Plane>(), true);
    register_type::<Interval>("GfInterval", std::mem::size_of::<Interval>(), true);
    register_type::<MultiInterval>(
        "GfMultiInterval",
        std::mem::size_of::<MultiInterval>(),
        true,
    );
    register_type::<Frustum>("GfFrustum", std::mem::size_of::<Frustum>(), true);
    register_type::<Transform>("GfTransform", std::mem::size_of::<Transform>(), true);
    register_type::<Size2>("GfSize2", std::mem::size_of::<Size2>(), true);
    register_type::<Size3>("GfSize3", std::mem::size_of::<Size3>(), true);
    register_type::<Rotation>("GfRotation", std::mem::size_of::<Rotation>(), true);
    register_type::<Line>("GfLine", std::mem::size_of::<Line>(), true);
    register_type::<Line2d>("GfLine2d", std::mem::size_of::<Line2d>(), true);
    register_type::<LineSeg>("GfLineSeg", std::mem::size_of::<LineSeg>(), true);
    register_type::<LineSeg2d>("GfLineSeg2d", std::mem::size_of::<LineSeg2d>(), true);
    register_type::<Color>("GfColor", std::mem::size_of::<Color>(), false);
    register_type::<ColorSpace>("GfColorSpace", std::mem::size_of::<ColorSpace>(), false);
    register_type::<BBox3d>("GfBBox3d", std::mem::size_of::<BBox3d>(), true);
}
pub use vec2::{Vec2, Vec2d, Vec2f, Vec2h, Vec2i, vec2d, vec2f, vec2i};
pub use vec3::{Vec3, Vec3d, Vec3f, Vec3h, Vec3i, cross, slerp, vec3d, vec3f, vec3i};
pub use vec4::{Vec4, Vec4d, Vec4f, Vec4h, Vec4i, vec4d, vec4f, vec4i};
