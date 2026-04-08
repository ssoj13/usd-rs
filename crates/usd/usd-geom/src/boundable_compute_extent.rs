//! Plugin-based extent computation for UsdGeomBoundable.
//!
//! This module provides a registry for registering compute extent functions
//! for different Boundable prim types. This allows plugins to provide custom
//! extent computation logic for procedural geometry.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdGeom/boundableComputeExtent.h` and `.cpp`

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;

use usd_gf::Vec3f;
use usd_gf::matrix4::Matrix4d;
use usd_sdf::TimeCode;
use usd_tf::Token;

use usd_core::Attribute;
use usd_core::time_code::TimeCode as UsdTimeCode;
use usd_vt::Value;

use super::boundable::Boundable;

/// `Sdf.TimeCode` → `Usd.TimeCode` for attribute value resolution (default sentinel must stay default).
#[inline]
fn usd_time_from_sdf(time: TimeCode) -> UsdTimeCode {
    if time.is_default() {
        UsdTimeCode::default()
    } else {
        UsdTimeCode::new(time.value())
    }
}

/// Float array from authored `VtValue` (matches breadth of `UsdGeom` points width parsing).
fn float_vec_from_value(value: &Value) -> Vec<f32> {
    if let Some(w) = value.get::<Vec<f32>>() {
        return w.clone();
    }
    if let Some(w) = value.get::<usd_vt::Array<f32>>() {
        return w.iter().copied().collect();
    }
    if let Some(w) = value.get::<Vec<f64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = value.get::<usd_vt::Array<f64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(s) = value.get::<f32>() {
        return vec![*s];
    }
    if let Some(s) = value.get::<f64>() {
        return vec![*s as f32];
    }
    if let Some(s) = value.get::<i32>().copied() {
        return vec![s as f32];
    }
    if let Some(s) = value.get::<i64>().copied() {
        return vec![s as f32];
    }
    if let Some(w) = value.get::<Vec<i32>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = value.get::<usd_vt::Array<i32>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = value.get::<Vec<i64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = value.get::<usd_vt::Array<i64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    Vec::new()
}

/// Read an attribute value as `Vec<f32>` (widths, radii, etc.).
fn get_float_array_attr(attr: &Attribute, time: TimeCode) -> Vec<f32> {
    if !attr.is_valid() {
        return Vec::new();
    }
    let ut = usd_time_from_sdf(time);
    if let Some(v) = attr.get_typed_vec::<f32>(ut) {
        return v;
    }
    if let Some(v) = attr.get_typed_vec::<f64>(ut) {
        return v.iter().map(|&x| x as f32).collect();
    }
    if let Some(v) = attr.get_typed_vec::<i32>(ut) {
        return v.iter().map(|&x| x as f32).collect();
    }
    if let Some(v) = attr.get_typed_vec::<i64>(ut) {
        return v.iter().map(|&x| x as f32).collect();
    }
    let Some(value) = attr.get(ut) else {
        return Vec::new();
    };
    float_vec_from_value(&value)
}

// ============================================================================
// Type Hierarchy
// ============================================================================

/// Get parent type for a given USD geometry type.
/// Returns None for root types (Boundable, Typed) or unknown types.
fn get_parent_type(type_name: &str) -> Option<&'static str> {
    match type_name {
        // Gprim hierarchy (all derive from Boundable)
        "Gprim" => Some("Boundable"),
        "PointBased" => Some("Gprim"),
        "Mesh" => Some("PointBased"),
        "Points" => Some("PointBased"),
        "Curves" => Some("PointBased"),
        "BasisCurves" => Some("Curves"),
        "NurbsCurves" => Some("Curves"),
        "NurbsPatch" => Some("PointBased"),
        // Implicit geometry (derive from Gprim)
        "Sphere" => Some("Gprim"),
        "Cube" => Some("Gprim"),
        "Cylinder" => Some("Gprim"),
        "Cone" => Some("Gprim"),
        "Capsule" => Some("Gprim"),
        "Plane" => Some("Gprim"),
        // Volume types
        "Volume" => Some("Gprim"),
        "OpenVDBAsset" => Some("FieldBase"),
        "Field3DAsset" => Some("FieldBase"),
        "FieldBase" => Some("Xformable"),
        // Skeleton types
        "SkelRoot" => Some("Boundable"),
        "Skeleton" => Some("Boundable"),
        // Xformable types (not Boundable, but included for completeness)
        "Xformable" => Some("Imageable"),
        "Imageable" => Some("Typed"),
        // Root types or unknown
        _ => None,
    }
}

// ============================================================================
// ComputeExtentFunction type
// ============================================================================

/// Function type for computing extents of a Boundable prim.
///
/// Parameters:
/// - `boundable`: The Boundable prim to compute extents for
/// - `time`: The time code at which to compute extents  
/// - `transform`: Optional transform matrix to apply. If None, identity is assumed.
///
/// Returns `Some([min, max])` on success, `None` on failure.
///
/// The function must be thread-safe.
pub type ComputeExtentFunction = fn(&Boundable, TimeCode, Option<&Matrix4d>) -> Option<[Vec3f; 2]>;

// ============================================================================
// Function Registry
// ============================================================================

/// Registry for compute extent functions.
///
/// Maps prim type tokens to compute extent function implementations.
struct FunctionRegistry {
    /// Map from prim type name to compute function.
    registry: RwLock<HashMap<Token, ComputeExtentFunction>>,
}

impl FunctionRegistry {
    /// Create a new empty registry.
    fn new() -> Self {
        Self {
            registry: RwLock::new(HashMap::new()),
        }
    }

    /// Register a compute function for a prim type.
    fn register(&self, prim_type: Token, func: ComputeExtentFunction) -> bool {
        let mut registry = self.registry.write().expect("rwlock poisoned");
        if registry.contains_key(&prim_type) {
            eprintln!(
                "ComputeExtentFunction already registered for prim type '{}'",
                prim_type.as_str()
            );
            return false;
        }
        registry.insert(prim_type, func);
        true
    }

    /// Get the compute function for a prim type, walking up the type hierarchy.
    fn get(&self, prim_type: &Token) -> Option<ComputeExtentFunction> {
        let registry = self.registry.read().expect("rwlock poisoned");

        // First try exact match
        if let Some(&func) = registry.get(prim_type) {
            return Some(func);
        }

        // Walk type hierarchy to find parent with registered function
        let mut current_type = prim_type.as_str();
        while let Some(parent) = get_parent_type(current_type) {
            let parent_token = Token::new(parent);
            if let Some(&func) = registry.get(&parent_token) {
                return Some(func);
            }
            current_type = parent;
        }

        None
    }
}

/// Global function registry singleton.
static FUNCTION_REGISTRY: Lazy<FunctionRegistry> = Lazy::new(|| {
    let registry = FunctionRegistry::new();

    // Register built-in extent compute functions for standard geometry types
    register_builtin_functions(&registry);

    registry
});

/// Register built-in compute extent functions for standard USD geometry types.
fn register_builtin_functions(registry: &FunctionRegistry) {
    // Sphere
    registry.register(Token::new("Sphere"), compute_extent_sphere);

    // Cube
    registry.register(Token::new("Cube"), compute_extent_cube);

    // Cylinder
    registry.register(Token::new("Cylinder"), compute_extent_cylinder);

    // Cone
    registry.register(Token::new("Cone"), compute_extent_cone);

    // Capsule
    registry.register(Token::new("Capsule"), compute_extent_capsule);

    // Plane
    registry.register(Token::new("Plane"), compute_extent_plane);

    // Point-based types
    registry.register(Token::new("PointBased"), compute_extent_point_based);
    registry.register(Token::new("Mesh"), compute_extent_point_based);
    registry.register(Token::new("NurbsPatch"), compute_extent_point_based);

    // Points (with widths)
    registry.register(Token::new("Points"), compute_extent_points);

    // Curves types (with widths)
    registry.register(Token::new("BasisCurves"), compute_extent_curves);
    registry.register(Token::new("NurbsCurves"), compute_extent_curves);
    registry.register(Token::new("Curves"), compute_extent_curves);

    // PointInstancer
    registry.register(Token::new("PointInstancer"), compute_extent_point_instancer);

    // UsdLux light types (matches C++ plugInfo.json implementsComputeExtent)
    registry.register(Token::new("RectLight"), compute_extent_rect_light);
    registry.register(Token::new("DiskLight"), compute_extent_disk_light);
    registry.register(Token::new("SphereLight"), compute_extent_sphere_light);
    registry.register(Token::new("CylinderLight"), compute_extent_cylinder_light);
    registry.register(Token::new("PortalLight"), compute_extent_portal_light);
}

// ============================================================================
// Public API
// ============================================================================

/// Register a compute extent function for a Boundable prim type.
///
/// This allows plugins to provide custom extent computation for procedural
/// or custom geometry types.
///
/// # Arguments
///
/// * `prim_type` - The prim type token (e.g., "Sphere", "Mesh")
/// * `func` - The compute extent function
///
/// # Returns
///
/// `true` if registration succeeded, `false` if a function was already registered.
pub fn register_compute_extent_function(prim_type: Token, func: ComputeExtentFunction) -> bool {
    FUNCTION_REGISTRY.register(prim_type, func)
}

/// Compute extent for a Boundable prim using registered plugin functions.
///
/// Looks up the compute function for the prim's type (walking up the type
/// hierarchy if needed) and invokes it.
///
/// # Arguments
///
/// * `boundable` - The Boundable prim
/// * `time` - Time code at which to compute extent
/// * `transform` - Optional transform matrix to apply
///
/// # Returns
///
/// `Some([min, max])` if extent was computed, `None` if no function registered
/// or computation failed.
pub fn compute_extent_from_plugins(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    if !prim.is_valid() {
        return None;
    }

    let type_name = prim.type_name();
    let func = FUNCTION_REGISTRY.get(&type_name)?;

    func(boundable, time, transform)
}

// ============================================================================
// Built-in extent compute functions
// ============================================================================

/// Compute extent for a Sphere prim.
fn compute_extent_sphere(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::sphere::Sphere;

    let sphere = Sphere::new(boundable.prim().clone());
    let radius = sphere.get_radius(time).unwrap_or(1.0) as f32;

    let min = Vec3f::new(-radius, -radius, -radius);
    let max = Vec3f::new(radius, radius, radius);

    apply_transform([min, max], transform)
}

/// Compute extent for a Cube prim.
fn compute_extent_cube(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::cube::Cube;

    let cube = Cube::new(boundable.prim().clone());
    let size = cube.get_size(time).unwrap_or(2.0) as f32;
    let half = size / 2.0;

    let min = Vec3f::new(-half, -half, -half);
    let max = Vec3f::new(half, half, half);

    apply_transform([min, max], transform)
}

/// Compute extent for a Cylinder prim.
fn compute_extent_cylinder(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::cylinder::Cylinder;

    let cylinder = Cylinder::new(boundable.prim().clone());
    let radius = cylinder.get_radius(time).unwrap_or(1.0) as f32;
    let height = cylinder.get_height(time).unwrap_or(2.0) as f32;
    let half_height = height / 2.0;

    // Get axis (default Y)
    let axis = cylinder
        .get_axis(time)
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| "Y".to_string());

    let (min, max) = match axis.as_str() {
        "X" => (
            Vec3f::new(-half_height, -radius, -radius),
            Vec3f::new(half_height, radius, radius),
        ),
        "Z" => (
            Vec3f::new(-radius, -radius, -half_height),
            Vec3f::new(radius, radius, half_height),
        ),
        _ => (
            // Y (default)
            Vec3f::new(-radius, -half_height, -radius),
            Vec3f::new(radius, half_height, radius),
        ),
    };

    apply_transform([min, max], transform)
}

/// Compute extent for a Cone prim.
fn compute_extent_cone(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::cone::Cone;

    let cone = Cone::new(boundable.prim().clone());
    let radius = cone.get_radius(time).unwrap_or(1.0) as f32;
    let height = cone.get_height(time).unwrap_or(2.0) as f32;
    let half_height = height / 2.0;

    // Get axis (default Y)
    let axis = cone
        .get_axis(time)
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| "Y".to_string());

    let (min, max) = match axis.as_str() {
        "X" => (
            Vec3f::new(-half_height, -radius, -radius),
            Vec3f::new(half_height, radius, radius),
        ),
        "Z" => (
            Vec3f::new(-radius, -radius, -half_height),
            Vec3f::new(radius, radius, half_height),
        ),
        _ => (
            // Y (default)
            Vec3f::new(-radius, -half_height, -radius),
            Vec3f::new(radius, half_height, radius),
        ),
    };

    apply_transform([min, max], transform)
}

/// Compute extent for a Capsule prim.
fn compute_extent_capsule(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::capsule::Capsule;

    let capsule = Capsule::new(boundable.prim().clone());
    let radius = capsule.get_radius(time).unwrap_or(0.5) as f32;
    let height = capsule.get_height(time).unwrap_or(1.0) as f32;
    let half_height = height / 2.0 + radius; // Capsule includes hemisphere caps

    // Get axis (default Y)
    let axis = capsule
        .get_axis(time)
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| "Y".to_string());

    let (min, max) = match axis.as_str() {
        "X" => (
            Vec3f::new(-half_height, -radius, -radius),
            Vec3f::new(half_height, radius, radius),
        ),
        "Z" => (
            Vec3f::new(-radius, -radius, -half_height),
            Vec3f::new(radius, radius, half_height),
        ),
        _ => (
            // Y (default)
            Vec3f::new(-radius, -half_height, -radius),
            Vec3f::new(radius, half_height, radius),
        ),
    };

    apply_transform([min, max], transform)
}

/// Compute extent for a Plane prim.
fn compute_extent_plane(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::plane::Plane;

    let plane = Plane::new(boundable.prim().clone());
    let width = plane.get_width(time).unwrap_or(2.0) as f32;
    let length = plane.get_length(time).unwrap_or(2.0) as f32;
    let half_width = width / 2.0;
    let half_length = length / 2.0;

    // Get axis (default Z - plane lies in XY, normal along Z)
    let axis = plane
        .get_axis(time)
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| "Z".to_string());

    let (min, max) = match axis.as_str() {
        "X" => (
            Vec3f::new(0.0, -half_width, -half_length),
            Vec3f::new(0.0, half_width, half_length),
        ),
        "Y" => (
            Vec3f::new(-half_width, 0.0, -half_length),
            Vec3f::new(half_width, 0.0, half_length),
        ),
        _ => (
            // Z (default)
            Vec3f::new(-half_width, -half_length, 0.0),
            Vec3f::new(half_width, half_length, 0.0),
        ),
    };

    apply_transform([min, max], transform)
}

/// Compute extent for a PointBased/Mesh/NurbsPatch prim (extent from points only).
fn compute_extent_point_based(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::point_based::PointBased;

    let pb = PointBased::new(boundable.prim().clone());
    let points_attr = pb.get_points_attr();
    if !points_attr.is_valid() {
        return None;
    }
    let value = points_attr.get(usd_time_from_sdf(time))?;
    let points: Vec<Vec3f> = value.get::<Vec<Vec3f>>().cloned().or_else(|| {
        value
            .get::<usd_vt::Array<Vec3f>>()
            .map(|a| a.iter().cloned().collect())
    })?;
    if points.is_empty() {
        return None;
    }

    let mut extent = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
    match transform {
        Some(xform) => {
            if !PointBased::compute_extent_with_transform(&points, xform, &mut extent) {
                return None;
            }
        }
        None => {
            if !PointBased::compute_extent(&points, &mut extent) {
                return None;
            }
        }
    }
    Some(extent)
}

/// Compute extent for a Points prim (points + widths).
fn compute_extent_points(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::points::Points;

    let pts = Points::new(boundable.prim().clone());

    // Get points via PointBased
    let points_attr = pts.point_based().get_points_attr();
    if !points_attr.is_valid() {
        return None;
    }
    let value = points_attr.get(usd_time_from_sdf(time))?;
    let points: Vec<Vec3f> = value.get::<Vec<Vec3f>>().cloned().or_else(|| {
        value
            .get::<usd_vt::Array<Vec3f>>()
            .map(|a| a.iter().cloned().collect())
    })?;
    if points.is_empty() {
        return None;
    }

    // Get widths (try multiple stored types)
    let widths = get_float_array_attr(&pts.get_widths_attr(), time);

    let mut extent = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
    match transform {
        Some(xform) => {
            // Per-point sphere AABB union: for each point, transform its local
            // AABB (point +/- radius) and compute the global union.
            if !compute_extent_per_point_spheres(&points, &widths, xform, &mut extent) {
                return None;
            }
        }
        None => {
            if !Points::compute_extent(&points, &widths, &mut extent) {
                return None;
            }
        }
    }
    Some(extent)
}

/// Compute extent for Curves/BasisCurves/NurbsCurves (points + widths).
fn compute_extent_curves(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::curves::Curves;
    use super::point_based::PointBased;

    let pb = PointBased::new(boundable.prim().clone());
    let points_attr = pb.get_points_attr();
    if !points_attr.is_valid() {
        return None;
    }
    let value = points_attr.get(usd_time_from_sdf(time))?;
    let points: Vec<Vec3f> = value.get::<Vec<Vec3f>>().cloned().or_else(|| {
        value
            .get::<usd_vt::Array<Vec3f>>()
            .map(|a| a.iter().cloned().collect())
    })?;
    if points.is_empty() {
        return None;
    }

    // Get widths via Curves widths attr (try multiple stored types)
    let curves = Curves::new(boundable.prim().clone());
    let widths = get_float_array_attr(&curves.get_widths_attr(), time);

    let mut extent = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
    match transform {
        Some(xform) => {
            // Use max width and per-point AABB union
            let max_width = if !widths.is_empty() {
                widths.iter().fold(0.0f32, |a, &b| a.max(b))
            } else {
                0.0
            };
            let uniform_widths: Vec<f32> = vec![max_width; points.len()];
            if !compute_extent_per_point_spheres(&points, &uniform_widths, xform, &mut extent) {
                return None;
            }
        }
        None => {
            if !Curves::compute_extent(&points, &widths, &mut extent) {
                return None;
            }
        }
    }
    Some(extent)
}

/// Compute extent for a PointInstancer prim.
fn compute_extent_point_instancer(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    use super::point_instancer::PointInstancer;

    let pi = PointInstancer::new(boundable.prim().clone());
    let mut extent_arr = usd_vt::Array::new();
    if !pi.compute_extent_at_time_with_transform(&mut extent_arr, time, time, transform) {
        return None;
    }
    if extent_arr.len() < 2 {
        return None;
    }
    Some([extent_arr[0], extent_arr[1]])
}

/// Compute extent by treating each point as a sphere with the given width (diameter),
/// transforming each sphere's local AABB, and taking the union.
fn compute_extent_per_point_spheres(
    points: &[Vec3f],
    widths: &[f32],
    transform: &Matrix4d,
    extent: &mut [Vec3f; 2],
) -> bool {
    if points.is_empty() {
        return false;
    }

    let mut global_min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
    let mut global_max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

    for (i, point) in points.iter().enumerate() {
        let radius = if !widths.is_empty() && i < widths.len() {
            widths[i] * 0.5
        } else {
            0.0
        };

        let local_min = Vec3f::new(point.x - radius, point.y - radius, point.z - radius);
        let local_max = Vec3f::new(point.x + radius, point.y + radius, point.z + radius);

        // Transform all 8 corners of this local AABB
        let corners = [
            Vec3f::new(local_min.x, local_min.y, local_min.z),
            Vec3f::new(local_max.x, local_min.y, local_min.z),
            Vec3f::new(local_min.x, local_max.y, local_min.z),
            Vec3f::new(local_max.x, local_max.y, local_min.z),
            Vec3f::new(local_min.x, local_min.y, local_max.z),
            Vec3f::new(local_max.x, local_min.y, local_max.z),
            Vec3f::new(local_min.x, local_max.y, local_max.z),
            Vec3f::new(local_max.x, local_max.y, local_max.z),
        ];

        for corner in &corners {
            let c_d = usd_gf::vec3::Vec3d::new(corner.x as f64, corner.y as f64, corner.z as f64);
            let t_d = transform.transform_point(&c_d);
            let t = Vec3f::new(t_d.x as f32, t_d.y as f32, t_d.z as f32);
            global_min.x = global_min.x.min(t.x);
            global_min.y = global_min.y.min(t.y);
            global_min.z = global_min.z.min(t.z);
            global_max.x = global_max.x.max(t.x);
            global_max.y = global_max.y.max(t.y);
            global_max.z = global_max.z.max(t.z);
        }
    }

    extent[0] = global_min;
    extent[1] = global_max;
    true
}

/// Apply transform to extent, returning new extent that bounds the transformed box.
// ============================================================================
// UsdLux Light Extent Functions
// Matches C++ implementations in pxr/usd/usdLux/*Light.cpp
// ============================================================================

/// RectLight: flat rectangle in XY plane, extent = [-w/2,-h/2,0] to [w/2,h/2,0]
fn compute_extent_rect_light(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    let width = prim
        .get_attribute("inputs:width")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(1.0);
    let height = prim
        .get_attribute("inputs:height")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(1.0);
    let half = Vec3f::new(width * 0.5, height * 0.5, 0.0);
    apply_transform([Vec3f::new(-half.x, -half.y, 0.0), half], transform)
}

/// DiskLight: flat circle in XY plane, extent = [-r,-r,0] to [r,r,0]
fn compute_extent_disk_light(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    let radius = prim
        .get_attribute("inputs:radius")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(0.5);
    apply_transform(
        [
            Vec3f::new(-radius, -radius, 0.0),
            Vec3f::new(radius, radius, 0.0),
        ],
        transform,
    )
}

/// SphereLight: sphere, extent = [-r,-r,-r] to [r,r,r]
fn compute_extent_sphere_light(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    let radius = prim
        .get_attribute("inputs:radius")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(0.5);
    apply_transform(
        [
            Vec3f::new(-radius, -radius, -radius),
            Vec3f::new(radius, radius, radius),
        ],
        transform,
    )
}

/// CylinderLight: cylinder along X axis, extent = [-l/2,-r,-r] to [l/2,r,r]
fn compute_extent_cylinder_light(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    let radius = prim
        .get_attribute("inputs:radius")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(0.5);
    let length = prim
        .get_attribute("inputs:length")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(1.0);
    let half_len = length * 0.5;
    apply_transform(
        [
            Vec3f::new(-half_len, -radius, -radius),
            Vec3f::new(half_len, radius, radius),
        ],
        transform,
    )
}

/// PortalLight: flat rectangle in XY plane (same as RectLight)
fn compute_extent_portal_light(
    boundable: &Boundable,
    time: TimeCode,
    transform: Option<&Matrix4d>,
) -> Option<[Vec3f; 2]> {
    let prim = boundable.prim();
    let width = prim
        .get_attribute("inputs:width")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(1.0);
    let height = prim
        .get_attribute("inputs:height")
        .and_then(|a| a.get_typed::<f32>(time))
        .unwrap_or(1.0);
    let half = Vec3f::new(width * 0.5, height * 0.5, 0.0);
    apply_transform([Vec3f::new(-half.x, -half.y, 0.0), half], transform)
}

fn apply_transform(extent: [Vec3f; 2], transform: Option<&Matrix4d>) -> Option<[Vec3f; 2]> {
    let transform = match transform {
        Some(t) => t,
        None => return Some(extent),
    };

    let [min, max] = extent;

    // Transform all 8 corners of the bounding box
    let corners = [
        Vec3f::new(min.x, min.y, min.z),
        Vec3f::new(max.x, min.y, min.z),
        Vec3f::new(min.x, max.y, min.z),
        Vec3f::new(max.x, max.y, min.z),
        Vec3f::new(min.x, min.y, max.z),
        Vec3f::new(max.x, min.y, max.z),
        Vec3f::new(min.x, max.y, max.z),
        Vec3f::new(max.x, max.y, max.z),
    ];

    // Transform each corner and compute new AABB
    let mut new_min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
    let mut new_max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

    for corner in corners {
        let transformed = transform.transform_point(&corner.to_f64()).to_f32();
        new_min.x = new_min.x.min(transformed.x);
        new_min.y = new_min.y.min(transformed.y);
        new_min.z = new_min.z.min(transformed.z);
        new_max.x = new_max.x.max(transformed.x);
        new_max.y = new_max.y.max(transformed.y);
        new_max.z = new_max.z.max(transformed.z);
    }

    Some([new_min, new_max])
}

// Helper trait for Vec3f conversion
trait Vec3fConvert {
    fn to_f64(&self) -> usd_gf::Vec3d;
}

trait Vec3dConvert {
    fn to_f32(&self) -> Vec3f;
}

impl Vec3fConvert for Vec3f {
    fn to_f64(&self) -> usd_gf::Vec3d {
        usd_gf::Vec3d::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

impl Vec3dConvert for usd_gf::Vec3d {
    fn to_f32(&self) -> Vec3f {
        Vec3f::new(self.x as f32, self.y as f32, self.z as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        // Just test that the lazy registry initializes
        let _ = &*FUNCTION_REGISTRY;
    }

    #[test]
    fn test_builtin_registrations() {
        // Verify built-in functions are registered
        assert!(FUNCTION_REGISTRY.get(&Token::new("Sphere")).is_some());
        assert!(FUNCTION_REGISTRY.get(&Token::new("Cube")).is_some());
        assert!(FUNCTION_REGISTRY.get(&Token::new("Cylinder")).is_some());
        assert!(FUNCTION_REGISTRY.get(&Token::new("Cone")).is_some());
        assert!(FUNCTION_REGISTRY.get(&Token::new("Capsule")).is_some());
        assert!(FUNCTION_REGISTRY.get(&Token::new("Plane")).is_some());
    }

    #[test]
    fn test_unknown_type() {
        // Unknown type should return None
        assert!(FUNCTION_REGISTRY.get(&Token::new("UnknownType")).is_none());
    }

    #[test]
    fn test_type_hierarchy_lookup() {
        // Verify type hierarchy relationships
        assert_eq!(get_parent_type("Mesh"), Some("PointBased"));
        assert_eq!(get_parent_type("PointBased"), Some("Gprim"));
        assert_eq!(get_parent_type("Gprim"), Some("Boundable"));
        assert_eq!(get_parent_type("Sphere"), Some("Gprim"));
        assert_eq!(get_parent_type("BasisCurves"), Some("Curves"));
        assert_eq!(get_parent_type("Curves"), Some("PointBased"));
        // Root types have no parent
        assert_eq!(get_parent_type("Boundable"), None);
        assert_eq!(get_parent_type("Unknown"), None);
    }
}
