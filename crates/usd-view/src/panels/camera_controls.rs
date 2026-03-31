//! Enhanced camera interaction for the viewport.
//!
//! Maya-style camera controls: Alt+LMB tumble, Alt+MMB pan, Alt+RMB dolly.
//! LMB without Alt is reserved for click-select and marquee drag.
//! F: frame selected, A: frame all. Scroll: zoom.
//! PanTilt mode (no alt, RMB): rotate camera around its own position.
//! Walk mode (WASD/QE): FPS-style movement with mouse look.
//! Auto-computes clipping planes from scene geometry.
//! Reference: usdviewq stageView.py camera interaction + freeCamera.py.

use usd_gf::vec3::Vec3d;
use usd_sdf::{Path, TimeCode};

use crate::bounds::compute_world_bound_for_view;
use crate::camera::FreeCamera;
use crate::data_model::DataModel;

/// Near/far defaults (same as free_camera module).
/// See `free_camera::DEFAULT_NEAR` doc for the small-scene scaling rationale.
const DEFAULT_NEAR: f64 = 1.0;
const DEFAULT_FAR: f64 = 2_000_000.0;
const MAX_REASONABLE_BOUND_ABS: f64 = 1.0e8;
/// Max far/near ratio before z-buffer precision degrades.
/// Reference: freeCamera.py `maxGoodZResolution = 5e4`.
const MAX_GOOD_Z_RESOLUTION: f64 = 5.0e4;

/// Camera interaction mode — determines what mouse drag does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionMode {
    /// Default: Alt+LMB tumble, Alt+MMB truck, scroll zoom.
    #[default]
    Orbit,
    /// PanTilt: rotate camera heading/pitch around camera position (center moves).
    PanTilt,
    /// Walk: WASD/QE move + mouse-look (FPS-style).
    Walk,
}

/// Action returned by camera input processing.
#[derive(Debug, Clone)]
pub enum CameraAction {
    /// No action.
    None,
    /// Camera was moved — request repaint.
    Repaint,
    /// Frame selected prims (F key).
    FrameSelected,
    /// Frame entire scene (Shift+F / A key).
    FrameAll,
}

/// Process viewport input and drive the camera accordingly.
///
/// Returns a `CameraAction` indicating what happened (for repaint requests, etc.).
///
/// Orbit mode (default, Maya-style):
/// - Alt+LMB: tumble (orbit)
/// - Alt+MMB or Alt+Ctrl+LMB: truck (pan)
/// - Alt+RMB: dolly (zoom by dragging up/down)
/// - MMB/RMB (no alt): pan
/// - scroll: zoom
/// LMB without Alt is reserved for selection (click) and marquee (drag).
///
/// PanTilt mode:
/// - LMB drag: pan/tilt camera around its own position (center moves, not camera)
/// - scroll: zoom (adjust dist)
///
/// Walk mode:
/// - WASD: move forward/back/left/right in camera-local horizontal plane
/// - Q/E: move up/down
/// - LMB/RMB drag: mouse-look (pan/tilt)
/// - scroll: zoom (adjust dist)
///
/// `tumble_speed` and `zoom_speed` are user-configured sensitivity multipliers.
pub fn process_input(
    ui: &egui::Ui,
    response: &egui::Response,
    camera: &mut FreeCamera,
    tumble_speed: f32,
    zoom_speed: f32,
    viewport_height: f64,
    mode: InteractionMode,
) -> CameraAction {
    if !response.hovered() {
        return CameraAction::None;
    }

    let input = ui.input(|i| i.clone());
    let ctrl = input.modifiers.ctrl;
    let delta = input.pointer.delta();
    let scroll = input.raw_scroll_delta.y;

    // Base sensitivity constants scaled by user multiplier
    let tumble_sens = 0.25_f64 * tumble_speed as f64;
    // Zoom factor per scroll tick
    let zoom_factor_base = 0.9_f64.powf(1.0 / zoom_speed as f64);
    // Dolly-drag sensitivity
    let dolly_sens = 0.005_f64 * zoom_speed as f64;

    // Keyboard shortcuts (same in all modes)
    if input.key_pressed(egui::Key::F) {
        return if input.modifiers.shift {
            CameraAction::FrameAll
        } else {
            CameraAction::FrameSelected
        };
    }
    if input.key_pressed(egui::Key::A) {
        return CameraAction::FrameAll;
    }

    let mut moved = false;

    match mode {
        InteractionMode::Orbit => {
            let shift = input.modifiers.shift;
            // LMB (no Shift): tumble orbit. Shift+LMB reserved for marquee select.
            if response.dragged_by(egui::PointerButton::Primary) && !shift && !ctrl {
                camera.tumble(delta.x as f64 * tumble_sens, delta.y as f64 * tumble_sens);
                moved = true;
            }
            // MMB or Ctrl+LMB: truck (pan)
            else if response.dragged_by(egui::PointerButton::Middle)
                || (ctrl && response.dragged_by(egui::PointerButton::Primary))
            {
                let scale = frustum_scale(camera, viewport_height);
                camera.truck(-delta.x as f64 * scale, delta.y as f64 * scale);
                moved = true;
            }
            // RMB: dolly (zoom by vertical drag)
            else if response.dragged_by(egui::PointerButton::Secondary) {
                let factor = 1.0 + delta.y as f64 * dolly_sens;
                camera.adjust_distance(factor);
                moved = true;
            }
        }

        InteractionMode::PanTilt => {
            // LMB or RMB drag: pan/tilt camera around its own position.
            // Reference freeCamera.py PanTilt(dPan, dTilt): rotates heading/pitch,
            // center point follows so camera stays fixed.
            if response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Secondary)
            {
                camera.pan_tilt(delta.x as f64 * tumble_sens, -delta.y as f64 * tumble_sens);
                moved = true;
            }
            // MMB: truck for fine adjustments
            else if response.dragged_by(egui::PointerButton::Middle) {
                let scale = frustum_scale(camera, viewport_height);
                camera.truck(-delta.x as f64 * scale, delta.y as f64 * scale);
                moved = true;
            }
        }

        InteractionMode::Walk => {
            // WASD movement: speed proportional to dist (scene-scale aware).
            // Walk speed: 0.5% of dist per frame at default zoom speed.
            let walk_speed = camera.dist() * 0.005 * zoom_speed as f64;
            let mut d_fwd = 0.0_f64;
            let mut d_right = 0.0_f64;
            let mut d_up = 0.0_f64;

            if input.key_down(egui::Key::W) {
                d_fwd += walk_speed;
            }
            if input.key_down(egui::Key::S) {
                d_fwd += -walk_speed;
            }
            if input.key_down(egui::Key::D) {
                d_right += walk_speed;
            }
            if input.key_down(egui::Key::A) {
                d_right += -walk_speed;
            }
            if input.key_down(egui::Key::E) {
                d_up += walk_speed;
            }
            if input.key_down(egui::Key::Q) {
                d_up += -walk_speed;
            }

            if d_fwd != 0.0 || d_right != 0.0 {
                camera.walk(d_fwd, d_right);
                moved = true;
            }
            if d_up != 0.0 {
                // Vertical movement: translate center along world up
                let up = if camera.is_z_up() {
                    usd_gf::vec3d(0.0, 0.0, 1.0)
                } else {
                    usd_gf::vec3d(0.0, 1.0, 0.0)
                };
                camera.set_center(camera.center() + up * d_up);
                moved = true;
            }

            // Mouse look (LMB or RMB): pan/tilt
            if response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Secondary)
            {
                camera.pan_tilt(delta.x as f64 * tumble_sens, -delta.y as f64 * tumble_sens);
                moved = true;
            }
        }
    }

    // Scroll wheel zoom (all modes) — exponential: factor^(scroll_ticks)
    if scroll != 0.0 {
        let ticks = (scroll / 40.0) as f64; // normalize: ~40px per notch
        let factor = zoom_factor_base.powf(ticks);
        camera.adjust_distance(factor);
        moved = true;
    }

    if moved {
        CameraAction::Repaint
    } else {
        CameraAction::None
    }
}

/// Computes the pan scale factor: frustum height / viewport height.
/// Ensures panning matches screen pixels.
#[inline]
fn frustum_scale(camera: &FreeCamera, viewport_height: f64) -> f64 {
    let fov_rad = camera.fov().to_radians();
    let frustum_height = 2.0 * camera.dist() * (fov_rad * 0.5).tan();
    frustum_height / viewport_height.max(1.0)
}

/// Frame the camera to fit the bounding box of selected prims.
///
/// If `paths` is empty, does nothing. Uses `compute_world_bound` per prim.
pub fn frame_selected(
    camera: &mut FreeCamera,
    data_model: &DataModel,
    paths: &[Path],
    frame_fit: f64,
) {
    if paths.is_empty() {
        return;
    }

    let stage = match data_model.root.stage.as_ref() {
        Some(s) => s,
        None => return,
    };

    let time = data_model.root.current_time;
    let (bbox_min, bbox_max) = match compute_selection_bbox(stage.as_ref(), data_model, paths, time)
    {
        Some(b) => b,
        None => return,
    };

    // Skip if degenerate
    let diag = bbox_max - bbox_min;
    if diag.x.abs() < 1e-12 && diag.y.abs() < 1e-12 && diag.z.abs() < 1e-12 {
        return;
    }
    if !is_reasonable_bbox(&bbox_min, &bbox_max) {
        return;
    }

    camera.frame_selection(bbox_min, bbox_max, frame_fit);
}

/// Frame the camera to fit the entire scene bounding box.
pub fn frame_all(camera: &mut FreeCamera, data_model: &DataModel, frame_fit: f64) {
    let Some((bmin, bmax)) = data_model.compute_stage_bbox_for_view() else {
        log::warn!("[frame_all] stage bbox is empty — cannot frame");
        return;
    };
    log::info!(
        "[frame_all] bbox min=({:.3},{:.3},{:.3}) max=({:.3},{:.3},{:.3})",
        bmin.x,
        bmin.y,
        bmin.z,
        bmax.x,
        bmax.y,
        bmax.z
    );
    if !is_finite_vec3(&bmin) || !is_finite_vec3(&bmax) {
        log::warn!("[frame_all] bbox not finite — cannot frame");
        return;
    }
    if !is_reasonable_bbox(&bmin, &bmax) {
        log::warn!("[frame_all] bbox not reasonable — cannot frame");
        return;
    }

    let diag = bmax - bmin;
    if diag.x.abs() < 1e-12 && diag.y.abs() < 1e-12 && diag.z.abs() < 1e-12 {
        log::warn!("[frame_all] bbox degenerate (zero size) — cannot frame");
        return;
    }

    camera.frame_selection(bmin, bmax, frame_fit);
}

/// Compute union of world bounding boxes for the given prim paths.
fn compute_selection_bbox(
    stage: &usd_core::Stage,
    data_model: &DataModel,
    paths: &[Path],
    time: TimeCode,
) -> Option<(Vec3d, Vec3d)> {
    let mut union_min = Vec3d::new(f64::MAX, f64::MAX, f64::MAX);
    let mut union_max = Vec3d::new(f64::MIN, f64::MIN, f64::MIN);
    let mut found_any = false;

    for path in paths {
        if let Some(prim) = stage.get_prim_at_path(path) {
            let imageable = usd_geom::imageable::Imageable::new(prim);
            if imageable.is_valid() {
                let bbox = compute_world_bound_for_view(&imageable, time, &data_model.view);
                let range = bbox.compute_aligned_range();
                if range.is_empty() {
                    continue;
                }
                let bmin = range.min();
                let bmax = range.max();
                if !is_finite_vec3(bmin) || !is_finite_vec3(bmax) {
                    continue;
                }
                if !is_reasonable_bbox(bmin, bmax) {
                    continue;
                }

                union_min.x = union_min.x.min(bmin.x);
                union_min.y = union_min.y.min(bmin.y);
                union_min.z = union_min.z.min(bmin.z);
                union_max.x = union_max.x.max(bmax.x);
                union_max.y = union_max.y.max(bmax.y);
                union_max.z = union_max.z.max(bmax.z);
                found_any = true;
            }
        }
    }

    if found_any {
        Some((union_min, union_max))
    } else {
        None
    }
}

#[inline]
fn is_finite_vec3(v: &Vec3d) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

#[inline]
fn is_reasonable_bbox(min: &Vec3d, max: &Vec3d) -> bool {
    if !is_finite_vec3(min) || !is_finite_vec3(max) {
        return false;
    }
    [min.x, min.y, min.z, max.x, max.y, max.z]
        .iter()
        .all(|v| v.abs() <= MAX_REASONABLE_BOUND_ABS)
}

/// Compute near/far clip distances by projecting all 8 bbox corners onto the camera ray.
///
/// Reference: freeCamera.py `_rangeOfBoxAlongRay()` + `setClippingPlanes()`.
/// Projects all 8 corners of the AABB onto the view direction, finds min/max signed
/// distances, then applies precision-near adjustment using `closest_visible_dist`.
///
/// For small scenes (bbox diagonal < 1 m), the near-plane floor is scaled by
/// `bbox_diag * 0.1` instead of the hardcoded `DEFAULT_NEAR = 1.0`. This
/// prevents Blender-style exports (mesh in meters + `xformOp:scale = 0.01`)
/// from having their geometry clipped by an oversized near plane.
///
/// Returns `(near, far)` with `far > near > 0`.
pub fn compute_auto_clip(
    cam_pos: Vec3d,
    view_dir: Vec3d,
    bbox_min: Vec3d,
    bbox_max: Vec3d,
    closest_visible_dist: Option<f64>,
    last_framed_cvd: f64,
) -> (f64, f64) {
    let mut min_dist = f64::INFINITY;
    let mut max_dist = f64::NEG_INFINITY;

    // Project all 8 AABB corners onto view ray
    for i in 0..8_u32 {
        let corner = Vec3d::new(
            if i & 1 != 0 { bbox_max.x } else { bbox_min.x },
            if i & 2 != 0 { bbox_max.y } else { bbox_min.y },
            if i & 4 != 0 { bbox_max.z } else { bbox_min.z },
        );
        let delta = corner - cam_pos;
        let t = delta.x * view_dir.x + delta.y * view_dir.y + delta.z * view_dir.z;
        if t < min_dist {
            min_dist = t;
        }
        if t > max_dist {
            max_dist = t;
        }
    }

    // Clamp to avoid clipping edge geometry (matches freeCamera.py _rangeOfBoxAlongRay).
    // Scale the floor by bbox diagonal so small scenes (e.g. Blender cm exports with
    // 0.01 scale) don't get clamped to DEFAULT_NEAR=1.0 which clips all geometry.
    let bbox_diag = ((bbox_max.x - bbox_min.x).powi(2)
        + (bbox_max.y - bbox_min.y).powi(2)
        + (bbox_max.z - bbox_min.z).powi(2))
    .sqrt();
    let near_floor = if bbox_diag < DEFAULT_NEAR {
        (bbox_diag * 0.1).max(1e-4)
    } else {
        DEFAULT_NEAR
    };
    if min_dist < near_floor {
        min_dist = near_floor;
    } else {
        min_dist *= 0.99;
    }
    max_dist *= 1.01;
    if max_dist <= min_dist {
        max_dist = min_dist + 1.0;
    }

    let mut computed_near = min_dist;
    let computed_far = max_dist;

    // Precision push: far/near ratio should not exceed MAX_GOOD_Z_RESOLUTION
    let precision_near = computed_far / MAX_GOOD_Z_RESOLUTION;

    if let Some(closest) = closest_visible_dist {
        let half_close = closest / 2.0;
        if closest < last_framed_cvd {
            // Zoomed in closer since last reframe: clamp half_close from below
            // so it is always >= computed_near and >= precision_near.
            let half_close = precision_near.max(half_close).max(computed_near);
            // half_close >= computed_near is guaranteed here, so only check precision push.
            if precision_near > computed_near {
                computed_near = ((precision_near + half_close) / 2.0).min(half_close);
            }
        } else if half_close < computed_near {
            // Very close visible geometry
            computed_near = half_close;
        } else if precision_near > computed_near {
            // Push near for better z-precision
            computed_near = ((precision_near + half_close) / 2.0).min(half_close);
        }
    }

    let near = computed_near.max(1e-4); // absolute floor
    let far = computed_far.max(near + 1.0);
    (near, far)
}

/// Auto-compute near/far clipping planes from scene geometry.
///
/// Builds the scene bbox from the stage, then calls `compute_auto_clip`.
/// Returns (near, far). Falls back to defaults if scene is empty/degenerate.
///
/// Uses ray-box projection matching freeCamera.py `setClippingPlanes()`.
pub fn auto_clip_planes(data_model: &DataModel, camera: &FreeCamera) -> (f64, f64) {
    let Some((bmin, bmax)) = data_model.compute_stage_bbox_for_view() else {
        return (DEFAULT_NEAR, DEFAULT_FAR);
    };

    compute_auto_clip(
        camera.position(),
        camera.view_direction(),
        bmin,
        bmax,
        camera.closest_visible_dist(),
        camera.last_framed_closest_dist(),
    )
}
