//! Bound-based picking (ray-prim intersection).
//!
//! Fallback when UsdImagingGL Engine.test_intersection is not available.
//! Uses ray-bbox intersection against world bounds of imageable prims.
//! Reference: stageView.py pick(), computePickFrustum().

use usd_core::Prim;
use usd_geom::imageable::Imageable;
use usd_geom::mesh::Mesh;
use usd_geom::tokens::usd_geom_tokens;
use usd_gf::matrix4::Matrix4d;
use usd_gf::ray::Ray;
use usd_gf::vec3::Vec3d;
use usd_sdf::{Path, TimeCode};

use crate::bounds::compute_world_bound_for_purposes;
use crate::data_model::PickMode;

const DEFAULT_NEAR: f64 = 1.0;

/// Result of a pick operation with extended info.
#[derive(Debug, Clone)]
pub struct PickResult {
    /// Path of the picked prim (post pick-mode adjustment).
    pub path: Path,
    /// Distance from camera (ray t value).
    pub distance: f64,
    /// Instance index (-1 if not an instance pick).
    pub instance_index: i32,
    /// World-space hit point (approximated from bbox entry).
    pub hit_point: Vec3d,
}

/// Converts viewport pixel coordinates to a world-space ray.
///
/// Uses camera position and view inverse to build ray through the pick point.
/// Ray starts at camera position, direction through the point on the near plane.
fn unproject_to_ray(
    viewport_min_x: f64,
    viewport_min_y: f64,
    viewport_width: f64,
    viewport_height: f64,
    mouse_x: f64,
    mouse_y: f64,
    camera_pos: Vec3d,
    view_matrix: &Matrix4d,
    fov_degrees: f64,
    aspect_ratio: f64,
) -> Option<Ray> {
    if viewport_width <= 0.0 || viewport_height <= 0.0 || aspect_ratio <= 0.0 {
        return None;
    }

    // Normalize to 0..1 (viewport relative)
    let nx = (mouse_x - viewport_min_x) / viewport_width;
    let ny = (mouse_y - viewport_min_y) / viewport_height;

    // NDC: x,y in [-1, 1], y flipped for OpenGL-style (bottom-up in NDC)
    let ndc_x = 2.0 * nx - 1.0;
    let ndc_y = -(2.0 * ny - 1.0);

    // Point on near plane in camera space (camera looks -Z)
    let half_fov = fov_degrees.to_radians() * 0.5;
    let tan_fov = half_fov.tan();
    let cam_x = ndc_x * aspect_ratio * tan_fov * DEFAULT_NEAR;
    let cam_y = ndc_y * tan_fov * DEFAULT_NEAR;
    let cam_z = -DEFAULT_NEAR;
    let near_cam = Vec3d::new(cam_x, cam_y, cam_z);

    let inv_view = view_matrix.inverse()?;
    let near_world = inv_view.transform_point(&near_cam);

    let dir = near_world - camera_pos;
    let len_sq = dir.x * dir.x + dir.y * dir.y + dir.z * dir.z;
    if len_sq < 1e-20 {
        return None;
    }

    Some(Ray::new(camera_pos, dir))
}

/// Build a world-space pick ray from viewport coordinates.
pub fn compute_pick_ray(
    viewport_min_x: f64,
    viewport_min_y: f64,
    viewport_width: f64,
    viewport_height: f64,
    mouse_x: f64,
    mouse_y: f64,
    camera_pos: Vec3d,
    view_matrix: &Matrix4d,
    fov_degrees: f64,
    aspect_ratio: f64,
) -> Option<Ray> {
    unproject_to_ray(
        viewport_min_x,
        viewport_min_y,
        viewport_width,
        viewport_height,
        mouse_x,
        mouse_y,
        camera_pos,
        view_matrix,
        fov_degrees,
        aspect_ratio,
    )
}

/// Recursively tracks the nearest ray-bbox hit for imageable prims.
fn collect_nearest_hit(
    prim: &Prim,
    ray: &Ray,
    time: TimeCode,
    included_purposes: &[usd_tf::Token],
    nearest: &mut Option<(f64, Path)>,
) {
    if !prim.is_valid() {
        return;
    }

    let imageable = Imageable::new(prim.clone());
    if imageable.is_valid() {
        if imageable.compute_visibility(time) == usd_geom_tokens().invisible {
            return;
        }
        let purpose = imageable.compute_purpose();
        if !included_purposes.is_empty() && !included_purposes.contains(&purpose) {
            for child in prim.get_children() {
                collect_nearest_hit(&child, ray, time, included_purposes, nearest);
            }
            return;
        }

        if is_pickable_prim(prim) {
            let bbox = compute_world_bound_for_purposes(&imageable, time, included_purposes);
            if let Some((enter, _exit)) = ray.intersect_bbox(&bbox) {
                if enter >= 0.0 {
                    let best_dist = nearest.as_ref().map_or(f64::INFINITY, |(dist, _)| *dist);
                    let candidate_dist = if prim.type_name() == "Mesh" {
                        mesh_hit_distance(prim, &imageable, ray, time, best_dist).unwrap_or(enter)
                    } else {
                        enter
                    };
                    match nearest {
                        Some((best, _)) if candidate_dist >= *best => {}
                        _ => *nearest = Some((candidate_dist, prim.path().clone())),
                    }
                }
            }
        }
    }

    for child in prim.get_children() {
        collect_nearest_hit(&child, ray, time, included_purposes, nearest);
    }
}

#[inline]
pub(crate) fn is_pickable_prim_pub(prim: &Prim) -> bool {
    is_pickable_prim(prim)
}

#[inline]
fn is_pickable_prim(prim: &Prim) -> bool {
    matches!(
        prim.type_name().as_str(),
        "Mesh"
            | "Points"
            | "BasisCurves"
            | "Curves"
            | "Sphere"
            | "Cube"
            | "Cone"
            | "Cylinder"
            | "Capsule"
            | "Plane"
    )
}

fn mesh_hit_distance(
    prim: &Prim,
    imageable: &Imageable,
    ray: &Ray,
    time: TimeCode,
    max_dist: f64,
) -> Option<f64> {
    if prim.type_name() != "Mesh" {
        return None;
    }

    let mesh = Mesh::new(prim.clone());
    let face_counts = read_int_array_attr(&mesh.get_face_vertex_counts_attr(), time);
    let face_indices = read_int_array_attr(&mesh.get_face_vertex_indices_attr(), time);
    if face_counts.is_empty() || face_indices.len() < 3 {
        return None;
    }

    let local_points = read_mesh_points(&mesh, time);
    if local_points.len() < 3 {
        return None;
    }

    let xf = imageable.compute_local_to_world_transform(time);
    let world_points: Vec<Vec3d> = local_points.iter().map(|p| xf.transform_point(p)).collect();

    let mut closest = max_dist;
    let mut found = false;
    let mut offset = 0usize;
    for count in face_counts {
        let count = count.max(0) as usize;
        if count < 3 {
            offset = offset.saturating_add(count);
            continue;
        }
        if offset + count > face_indices.len() {
            break;
        }

        let i0 = face_indices[offset];
        let Some(p0) = point_at(&world_points, i0) else {
            offset += count;
            continue;
        };
        for j in 1..(count - 1) {
            let i1 = face_indices[offset + j];
            let i2 = face_indices[offset + j + 1];
            let (Some(p1), Some(p2)) = (point_at(&world_points, i1), point_at(&world_points, i2))
            else {
                continue;
            };
            if let Some((dist, _bary, _front)) = ray.intersect_triangle(&p0, &p1, &p2, closest) {
                if dist >= 0.0 && dist < closest {
                    closest = dist;
                    found = true;
                }
            }
        }
        offset += count;
    }

    found.then_some(closest)
}

#[inline]
fn point_at(points: &[Vec3d], index: i32) -> Option<Vec3d> {
    usize::try_from(index)
        .ok()
        .and_then(|i| points.get(i))
        .cloned()
}

fn read_mesh_points(mesh: &Mesh, time: TimeCode) -> Vec<Vec3d> {
    let attr = mesh.point_based().get_points_attr();
    if !attr.is_valid() {
        return Vec::new();
    }

    let Some(val) = attr
        .get(time)
        .or_else(|| attr.get(TimeCode::default_time()))
    else {
        return Vec::new();
    };

    // as_vec_clone handles both Vec<T> and Array<T> storage
    if let Some(arr) = val.as_vec_clone::<usd_gf::Vec3f>() {
        return arr
            .iter()
            .map(|v| Vec3d::new(v[0] as f64, v[1] as f64, v[2] as f64))
            .collect();
    }
    if let Some(arr) = val.as_vec_clone::<usd_gf::Vec3d>() {
        return arr;
    }
    if let Some(arr) = val.as_vec_clone::<[f32; 3]>() {
        return arr
            .iter()
            .map(|v| Vec3d::new(v[0] as f64, v[1] as f64, v[2] as f64))
            .collect();
    }
    if let Some(arr) = val.as_vec_clone::<[f64; 3]>() {
        return arr.iter().map(|v| Vec3d::new(v[0], v[1], v[2])).collect();
    }
    if let Some(vec) = val.downcast::<Vec<usd_vt::Value>>() {
        return vec.iter().filter_map(value_to_vec3d).collect();
    }

    Vec::new()
}

fn read_int_array_attr(attr: &usd_core::Attribute, time: TimeCode) -> Vec<i32> {
    if !attr.is_valid() {
        return Vec::new();
    }
    let Some(val) = attr
        .get(time)
        .or_else(|| attr.get(TimeCode::default_time()))
    else {
        return Vec::new();
    };

    // as_vec_clone handles both Vec<T> and Array<T> storage
    if let Some(arr) = val.as_vec_clone::<i32>() {
        return arr;
    }
    if let Some(vec) = val.downcast::<Vec<usd_vt::Value>>() {
        return vec
            .iter()
            .filter_map(|v| {
                v.downcast_clone::<i32>()
                    .or_else(|| v.downcast_clone::<i64>().map(|n| n as i32))
            })
            .collect();
    }

    Vec::new()
}

fn value_to_vec3d(v: &usd_vt::Value) -> Option<Vec3d> {
    if let Some(p) = v.downcast_clone::<usd_gf::Vec3f>() {
        return Some(Vec3d::new(p[0] as f64, p[1] as f64, p[2] as f64));
    }
    if let Some(p) = v.downcast_clone::<usd_gf::Vec3d>() {
        return Some(p);
    }
    if let Some(p) = v.downcast_clone::<[f32; 3]>() {
        return Some(Vec3d::new(p[0] as f64, p[1] as f64, p[2] as f64));
    }
    if let Some(p) = v.downcast_clone::<[f64; 3]>() {
        return Some(Vec3d::new(p[0], p[1], p[2]));
    }
    let tuple = v.downcast::<Vec<usd_vt::Value>>()?;
    if tuple.len() != 3 {
        return None;
    }
    let x = value_to_f64(&tuple[0])?;
    let y = value_to_f64(&tuple[1])?;
    let z = value_to_f64(&tuple[2])?;
    Some(Vec3d::new(x, y, z))
}

fn value_to_f64(v: &usd_vt::Value) -> Option<f64> {
    v.downcast_clone::<f64>()
        .or_else(|| v.downcast_clone::<f32>().map(|n| n as f64))
        .or_else(|| v.downcast_clone::<i64>().map(|n| n as f64))
        .or_else(|| v.downcast_clone::<i32>().map(|n| n as f64))
}

/// Picks the closest prim under the given viewport coordinates.
///
/// Returns the path of the closest hit, or None if nothing hit.
/// Uses bound-based intersection (not mesh-accurate).
pub fn pick_prim_at(
    stage: &usd_core::Stage,
    viewport_min_x: f64,
    viewport_min_y: f64,
    viewport_width: f64,
    viewport_height: f64,
    mouse_x: f64,
    mouse_y: f64,
    camera_pos: Vec3d,
    view_matrix: &Matrix4d,
    fov_degrees: f64,
    aspect_ratio: f64,
    time: TimeCode,
    show_render: bool,
    show_proxy: bool,
    show_guide: bool,
) -> Option<Path> {
    pick_prim_at_ex(
        stage,
        viewport_min_x,
        viewport_min_y,
        viewport_width,
        viewport_height,
        mouse_x,
        mouse_y,
        camera_pos,
        view_matrix,
        fov_degrees,
        aspect_ratio,
        time,
        show_render,
        show_proxy,
        show_guide,
        PickMode::Prims,
    )
    .map(|r| r.path)
}

/// Extended pick with pick mode support.
///
/// Returns `PickResult` with adjusted path based on `pick_mode`.
pub fn pick_prim_at_ex(
    stage: &usd_core::Stage,
    viewport_min_x: f64,
    viewport_min_y: f64,
    viewport_width: f64,
    viewport_height: f64,
    mouse_x: f64,
    mouse_y: f64,
    camera_pos: Vec3d,
    view_matrix: &Matrix4d,
    fov_degrees: f64,
    aspect_ratio: f64,
    time: TimeCode,
    show_render: bool,
    show_proxy: bool,
    show_guide: bool,
    pick_mode: PickMode,
) -> Option<PickResult> {
    let ray = unproject_to_ray(
        viewport_min_x,
        viewport_min_y,
        viewport_width,
        viewport_height,
        mouse_x,
        mouse_y,
        camera_pos,
        view_matrix,
        fov_degrees,
        aspect_ratio,
    )?;

    let t = usd_geom_tokens();
    let mut purposes = vec![t.default_.clone()];
    if show_render {
        purposes.push(t.render.clone());
    }
    if show_proxy {
        purposes.push(t.proxy.clone());
    }
    if show_guide {
        purposes.push(t.guide.clone());
    }

    let root = stage.get_pseudo_root();
    let mut nearest = None;
    collect_nearest_hit(&root, &ray, time, &purposes, &mut nearest);
    let (dist, raw_path) = nearest?;

    // Compute approximate hit point
    let hit_point = ray.point(dist);

    // Adjust path based on pick mode
    let (adjusted_path, instance_index) = adjust_pick_path(stage, &raw_path, pick_mode);

    Some(PickResult {
        path: adjusted_path,
        distance: dist,
        instance_index,
        hit_point,
    })
}

/// Rollover pick — lightweight query returning path under cursor without click.
///
/// Same as `pick_prim_at` but intended for hover tooltips / status bar info.
pub fn rollover_pick(
    stage: &usd_core::Stage,
    viewport_min_x: f64,
    viewport_min_y: f64,
    viewport_width: f64,
    viewport_height: f64,
    mouse_x: f64,
    mouse_y: f64,
    camera_pos: Vec3d,
    view_matrix: &Matrix4d,
    fov_degrees: f64,
    aspect_ratio: f64,
    time: TimeCode,
    show_render: bool,
    show_proxy: bool,
    show_guide: bool,
) -> Option<Path> {
    // Identical to basic pick — caller decides when to invoke (e.g. on hover)
    pick_prim_at(
        stage,
        viewport_min_x,
        viewport_min_y,
        viewport_width,
        viewport_height,
        mouse_x,
        mouse_y,
        camera_pos,
        view_matrix,
        fov_degrees,
        aspect_ratio,
        time,
        show_render,
        show_proxy,
        show_guide,
    )
}

/// Adjust the raw picked path based on pick mode.
///
/// - `Prims`: return as-is.
/// - `Models`: walk up to enclosing model (is_model).
/// - `Instances`: walk up to instance root, return instance index.
/// - `Prototypes`: walk up to prototype root.
fn adjust_pick_path(stage: &usd_core::Stage, raw_path: &Path, mode: PickMode) -> (Path, i32) {
    match mode {
        PickMode::Prims => (raw_path.clone(), -1),

        PickMode::Models => {
            // Walk up from picked prim to find nearest model ancestor
            let model_path = walk_up_to_model(stage, raw_path);
            (model_path.unwrap_or_else(|| raw_path.clone()), -1)
        }

        PickMode::Instances => {
            // Walk up to find instance root
            let (inst_path, idx) = walk_up_to_instance(stage, raw_path);
            (inst_path.unwrap_or_else(|| raw_path.clone()), idx)
        }

        PickMode::Prototypes => {
            // Walk up to find prototype
            let proto_path = walk_up_to_prototype(stage, raw_path);
            (proto_path.unwrap_or_else(|| raw_path.clone()), -1)
        }
    }
}

/// Public wrapper for pick-mode path adjustment.
pub fn adjust_picked_path(stage: &usd_core::Stage, raw_path: &Path, mode: PickMode) -> (Path, i32) {
    adjust_pick_path(stage, raw_path, mode)
}

/// Walk up from `path` to find the nearest ancestor (or self) that is a model.
fn walk_up_to_model(stage: &usd_core::Stage, path: &Path) -> Option<Path> {
    let mut current = path.clone();
    loop {
        if let Some(prim) = stage.get_prim_at_path(&current) {
            if prim.is_model() {
                return Some(current);
            }
        }
        if current.is_root_prim_path() || current == Path::absolute_root() {
            return None;
        }
        current = current.get_parent_path();
    }
}

/// Walk up to find instance root. Returns (instance_path, instance_index).
/// Note: bbox picking cannot determine the specific instance index (that comes
/// from the renderer in C++), so we return ALL_INSTANCES (-1) to select all
/// instances of the prototype, matching C++ usdview convention.
fn walk_up_to_instance(stage: &usd_core::Stage, path: &Path) -> (Option<Path>, i32) {
    let mut current = path.clone();
    loop {
        if let Some(prim) = stage.get_prim_at_path(&current) {
            if prim.is_instance() {
                // ALL_INSTANCES: bbox picking has no renderer-provided instance index
                return (Some(current), -1);
            }
        }
        if current.is_root_prim_path() || current == Path::absolute_root() {
            return (None, -1);
        }
        current = current.get_parent_path();
    }
}

/// Walk up to find prototype root.
fn walk_up_to_prototype(stage: &usd_core::Stage, path: &Path) -> Option<Path> {
    let mut current = path.clone();
    loop {
        if let Some(prim) = stage.get_prim_at_path(&current) {
            if prim.is_in_prototype() {
                // Check if parent is NOT in prototype — this is the prototype root
                let parent = prim.parent();
                if !parent.is_in_prototype() {
                    return Some(current);
                }
            }
        }
        if current.is_root_prim_path() || current == Path::absolute_root() {
            return None;
        }
        current = current.get_parent_path();
    }
}
