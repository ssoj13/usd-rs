//! 3D viewport panel.
//!
//! Orbit/pan/zoom camera controls, pick-to-select.
//! Renders USD scene via UsdImagingGL Engine.
//!
//! On wgpu builds, the fast path presents the engine color target directly as a
//! native egui texture and runs sRGB / OCIO on the GPU. CPU readback remains
//! only as a fallback path and for explicit capture/export.
//!
//! # Convention
//!
//! Row-vector convention (Imath/OpenUSD standard): `v' = v * M`.
//! - View matrix: world-to-camera, translation in row 3.
//! - Projection matrix: camera-to-clip.
//! - VP = View * Proj. Point projection: `clip = point * VP`.
//! - When manually projecting points (overlays, picking), use `VP.column(i)`
//!   to extract clip coordinates — NOT `VP.row(i)`. Using rows would give
//!   the wrong (column-vector / OpenGL-style) result.

mod color_correction;
mod context_menu;
#[cfg(feature = "wgpu")]
mod gpu_display;

use std::time::{Duration, Instant};

use egui::{Color32, Sense, Stroke};

use usd_gf::{Matrix4d, Vec2i, Vec3d};
use usd_imaging::gl::{DrawMode as EngineDrawMode, Engine, PickParams, RenderParams};
use usd_sdf::Path;
use usd_tf::Token;

/// Rotation matrix converting Z-up world to Y-up camera convention.
/// Rx(-90deg) in row-vector form: X stays, Y -> -Z, Z -> Y.
// Z-up correction is now handled inside FreeCamera (matches C++ freeCamera.py).
// The YZUpInvMatrix is part of the camera transform chain, not a post-hoc view fix.
use usd_camera_util::{ConformWindowPolicy, conform_window_matrix};

use crate::camera::FreeCamera;
use crate::data_model::{DataModel, DrawMode as ViewDrawMode, PickMode, SelectionHighlightMode};
use crate::keyboard::AppAction;
// Access via data_model.root / data_model.view / data_model.selection
use crate::bounds::compute_world_bound_for_purposes;
use crate::panels::camera_controls::{self, CameraAction, InteractionMode};
use crate::panels::hud::HudState;
use crate::panels::overlays::{self, OverlayState};
use crate::panels::pick;

use color_correction::{OcioCpuState, color_correct, update_viewport_texture};
use context_menu::viewport_context_menu;
#[cfg(feature = "wgpu")]
use gpu_display::ViewportGpuState;

/// Persistent viewport state across frames.
///
/// Caches the egui texture handle and tracks viewport dimensions
/// to avoid per-frame texture re-creation. Also owns HUD and overlay state.
pub struct ViewportState {
    /// Cached egui texture for the rendered scene
    texture_handle: Option<egui::TextureHandle>,
    /// Last rendered dimensions (for resize detection)
    last_width: usize,
    last_height: usize,
    /// HUD corner stats overlay.
    pub hud: HudState,
    /// Visual overlays (axes, bboxes, camera mask, reticles).
    pub overlays: OverlayState,
    /// Timestamp of last frame start (for FPS tracking).
    last_frame_time: Option<Instant>,
    /// Marquee selection: screen-space drag start (viewport-local coords).
    marquee_start: Option<egui::Pos2>,
    /// Marquee selection: last completed rectangle (for future frustum picking).
    pub marquee_rect: Option<egui::Rect>,
    /// Prim path under cursor (for hover-highlight).
    pub highlighted_prim: Option<Path>,
    /// OCIO CPU color correction state (config + cached processor).
    pub(crate) ocio_state: OcioCpuState,
    /// True once the viewport has produced a visible frame.
    pub has_presented_frame: bool,
    /// Native wgpu texture presentation state.
    #[cfg(feature = "wgpu")]
    pub(crate) gpu_state: ViewportGpuState,
    /// Per-phase CPU timing breakdown (milliseconds).
    pub phase_times: PhaseTimes,
    /// Camera undo stack — snapshots pushed at drag start.
    camera_undo: Vec<FreeCamera>,
    /// Short-lived repaint lease after viewport interaction.
    ///
    /// Native input delivery can be bursty or coalesced, especially for wheel
    /// zoom and slow drags. Keeping a small trailing repaint window prevents the
    /// viewer from collapsing to one frame per sporadic OS event while the user
    /// is still manipulating the camera.
    interaction_hot_until: Option<Instant>,
}

/// Per-phase CPU timing breakdown for one viewport frame.
#[derive(Default, Clone, Copy)]
pub struct PhaseTimes {
    /// Render dispatch (eng.render) time in ms.
    pub render_ms: f64,
    /// GPU readback time in ms.
    pub readback_ms: f64,
    /// Color correction time in ms.
    pub color_correct_ms: f64,
    /// Texture upload to egui time in ms.
    pub tex_upload_ms: f64,
}

impl ViewportState {
    /// Create empty viewport state.
    pub fn new() -> Self {
        Self {
            texture_handle: None,
            last_width: 0,
            last_height: 0,
            hud: HudState::new(),
            overlays: OverlayState::new(),
            last_frame_time: None,
            marquee_start: None,
            marquee_rect: None,
            highlighted_prim: None,
            ocio_state: OcioCpuState::default(),
            has_presented_frame: false,
            #[cfg(feature = "wgpu")]
            gpu_state: ViewportGpuState::new(),
            phase_times: PhaseTimes::default(),
            camera_undo: Vec::new(),
            interaction_hot_until: None,
        }
    }

    /// Push camera state to undo stack (capped at 32 entries).
    pub fn push_camera_undo(&mut self, camera: &FreeCamera) {
        const MAX_UNDO: usize = 32;
        if self.camera_undo.len() >= MAX_UNDO {
            self.camera_undo.remove(0);
        }
        self.camera_undo.push(camera.clone());
    }

    /// Pop and return the most recent camera state, if any.
    pub fn pop_camera_undo(&mut self) -> Option<FreeCamera> {
        self.camera_undo.pop()
    }

    /// True if camera undo stack is non-empty.
    pub fn can_undo_camera(&self) -> bool {
        !self.camera_undo.is_empty()
    }

    #[cfg(feature = "wgpu")]
    pub fn configure_wgpu_render_state(&mut self, render_state: egui_wgpu::RenderState) {
        self.gpu_state.set_render_state(render_state);
    }
}

impl Default for ViewportState {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds RenderParams from DataModel for UsdImagingGL Engine.
fn build_render_params(data_model: &DataModel) -> RenderParams {
    use usd_imaging::gl::{CullStyle, DrawMode as EngineDrawMode};

    let draw_mode = match data_model.view.draw_mode {
        ViewDrawMode::Wireframe => EngineDrawMode::Wireframe,
        ViewDrawMode::WireframeOnSurface => EngineDrawMode::WireframeOnSurface,
        ViewDrawMode::ShadedSmooth => EngineDrawMode::ShadedSmooth,
        ViewDrawMode::ShadedFlat => EngineDrawMode::ShadedFlat,
        ViewDrawMode::GeometryOnly => EngineDrawMode::GeomOnly,
        ViewDrawMode::Points => EngineDrawMode::Points,
        ViewDrawMode::GeomSmooth => EngineDrawMode::GeomSmooth,
        ViewDrawMode::GeomFlat => EngineDrawMode::GeomFlat,
        // Base draw mode for build_render_params; actual 2-pass dispatch
        // happens in the render match below (depth prepass + wireframe overlay).
        ViewDrawMode::HiddenSurfaceWireframe => EngineDrawMode::ShadedSmooth,
        // Bounds mode skips geometry entirely; bbox overlays drawn separately.
        ViewDrawMode::Bounds => EngineDrawMode::GeomOnly,
    };

    let highlight = matches!(
        data_model.view.sel_highlight_mode,
        SelectionHighlightMode::Always
    ) || (matches!(
        data_model.view.sel_highlight_mode,
        SelectionHighlightMode::OnlyWhenPaused
    ) && !(data_model.root.playing || data_model.root.scrubbing));

    // When ambient_light_only is set, disable scene lights and dome light
    // so only the default ambient light contributes (matches C++ _ambientOnlyClicked).
    let (scene_lights, lighting, dome_light) = if data_model.view.ambient_light_only {
        (false, true, false)
    } else {
        (
            data_model.view.enable_scene_lights,
            data_model.view.enable_scene_lights,
            data_model.view.dome_light_enabled,
        )
    };

    let mut params = RenderParams {
        frame: data_model.root.current_time.into(),
        complexity: data_model.view.complexity as f32,
        draw_mode,
        show_guides: data_model.view.display_guide,
        show_proxy: data_model.view.display_proxy,
        show_render: data_model.view.display_render,
        cull_style: if data_model.view.cull_backfaces {
            CullStyle::BackUnlessDoubleSided
        } else {
            CullStyle::Nothing
        },
        enable_scene_materials: data_model.view.enable_scene_materials,
        enable_scene_lights: scene_lights,
        enable_lighting: lighting,
        dome_light_enabled: dome_light,
        dome_light_textures_visible: data_model.view.dome_light_textures_visible,
        clear_color: data_model.view.clear_color.to_vec4f(),
        highlight,
        default_material_ambient: data_model.view.default_material_ambient,
        default_material_specular: data_model.view.default_material_specular,
        ..Default::default()
    };

    // Build render tags based on purpose display flags
    let mut render_tags = vec![Token::new("geometry")]; // always include geometry
    if data_model.view.display_guide {
        render_tags.push(Token::new("guide"));
    }
    if data_model.view.display_proxy {
        render_tags.push(Token::new("proxy"));
    }
    if data_model.view.display_render {
        render_tags.push(Token::new("render"));
    }
    params.render_tags = render_tags;

    // GeomOnly/GeomFlat/GeomSmooth/HiddenSurfaceWireframe render with default
    // grey material, ignoring all authored UsdPreviewSurface materials (C++ parity).
    if matches!(
        data_model.view.draw_mode,
        ViewDrawMode::GeometryOnly
            | ViewDrawMode::GeomFlat
            | ViewDrawMode::GeomSmooth
            | ViewDrawMode::HiddenSurfaceWireframe
    ) {
        params.enable_scene_materials = false;
    }

    params
}

fn vertical_fov_from_projection(proj: &Matrix4d) -> Option<f64> {
    let m11 = proj[1][1];
    if !m11.is_finite() || m11.abs() < 1e-12 {
        return None;
    }
    Some((2.0 * (1.0 / m11.abs()).atan()).to_degrees())
}

/// Resolve the active (view, proj) matrix pair for picking.
///
/// Uses the scene camera if one is active and valid, otherwise falls back
/// to the free camera. Both click-pick and hover-pick paths share this.
fn resolve_cam_matrices(
    data_model: &DataModel,
    camera: &FreeCamera,
    aspect: f64,
) -> (Matrix4d, Matrix4d) {
    if let Some(cam_path) = data_model.view.active_camera() {
        if let Some(ref stage) = data_model.root.stage {
            let usd_cam = usd_app_utils::get_camera_at_path(stage.as_ref(), &cam_path);
            if usd_cam.prim().is_valid() {
                let tc = data_model.root.current_time.into();
                if let (Some(v), Some(p)) = (
                    usd_cam.compute_view_matrix(tc),
                    usd_cam.compute_projection_matrix(tc),
                ) {
                    return (v, p);
                }
            }
        }
    }
    (camera.view_matrix(), camera.projection_matrix(aspect))
}

fn build_pick_matrices_from_cursor(
    view: &Matrix4d,
    proj: &Matrix4d,
    viewport_rect: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(Matrix4d, Matrix4d)> {
    let w = viewport_rect.width().max(1.0) as f64;
    let h = viewport_rect.height().max(1.0) as f64;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Match usdview/OpenUSD picking approach: narrow the projection window to
    // the clicked pixel and keep the original view matrix unchanged.
    //
    // We apply the equivalent of glPickMatrix in NDC space:
    //   x' = (x - cx) * 2 / sx
    //   y' = (y - cy) * 2 / sy
    // for a 1x1 pixel window (sx = 2/w, sy = 2/h).
    // Match OpenUSD pick semantics: use integer pixel start/end and include end+1.
    let max_x = ((w as i64) - 1).max(0) as i32;
    let max_y = ((h as i64) - 1).max(0) as i32;
    let px = ((cursor.x as f64 - viewport_rect.min.x as f64).floor() as i32).clamp(0, max_x) as f64;
    let py = ((cursor.y as f64 - viewport_rect.min.y as f64).floor() as i32).clamp(0, max_y) as f64;

    let min_x_ndc = 2.0 * (px / w) - 1.0;
    let max_x_ndc = 2.0 * ((px + 1.0) / w) - 1.0;
    let min_y_ndc = 1.0 - 2.0 * ((py + 1.0) / h);
    let max_y_ndc = 1.0 - 2.0 * (py / h);

    let cx = 0.5 * (min_x_ndc + max_x_ndc);
    let cy = 0.5 * (min_y_ndc + max_y_ndc);
    let sx = (max_x_ndc - min_x_ndc).abs().max(1e-12);
    let sy = (max_y_ndc - min_y_ndc).abs().max(1e-12);

    let kx = 2.0 / sx;
    let ky = 2.0 / sy;
    let tx = -cx * kx;
    let ty = -cy * ky;

    let pick_matrix = Matrix4d::new(
        kx, 0.0, 0.0, 0.0, 0.0, ky, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    );
    Some((*view, *proj * pick_matrix))
}

/// Read the current frame back to CPU on demand.
///
/// This is no longer the normal presentation path on wgpu; it is used only for
/// fallback rendering and explicit capture/export actions.
pub(crate) fn capture_current_frame(
    engine: &mut Engine,
    viewport_state: &mut ViewportState,
    data_model: &DataModel,
) -> Option<(Vec<u8>, usize, usize)> {
    let cc_mode = data_model.view.color_correction_mode;
    let ocio_settings = &data_model.view.ocio_settings;

    #[cfg(feature = "wgpu")]
    if engine.render_color_readback_is_u8() {
        if let Some((pixels, width, height)) = engine.read_render_pixels_staged() {
            let corrected = color_correct(
                pixels,
                cc_mode,
                &mut viewport_state.ocio_state,
                ocio_settings,
            );
            return Some((corrected, width as usize, height as usize));
        }
    }

    #[cfg(feature = "wgpu")]
    {
        let pixels = engine.read_render_pixels()?;
        let corrected = color_correct(
            &pixels,
            cc_mode,
            &mut viewport_state.ocio_state,
            ocio_settings,
        );
        let width = engine.render_buffer_size().x.max(0) as usize;
        let height = engine.render_buffer_size().y.max(0) as usize;
        return Some((corrected, width, height));
    }

    #[cfg(not(feature = "wgpu"))]
    {
        let pixels = engine.read_render_pixels()?;
        let corrected = color_correct(
            &pixels,
            cc_mode,
            &mut viewport_state.ocio_state,
            ocio_settings,
        );
        let width = engine.render_buffer_size().x.max(0) as usize;
        let height = engine.render_buffer_size().y.max(0) as usize;
        Some((corrected, width, height))
    }
}

/// Read the current frame back as linear RGBA32F for HDR export.
///
/// This bypasses display color correction intentionally. EXR export should
/// preserve the engine color target, not the viewport display transform.
pub(crate) fn capture_current_frame_linear(
    engine: &mut Engine,
) -> Option<(Vec<f32>, usize, usize)> {
    let pixels = engine.read_render_pixels_linear_rgba32f()?;
    let width = engine.render_buffer_size().x.max(0) as usize;
    let height = engine.render_buffer_size().y.max(0) as usize;
    Some((pixels, width, height))
}

/// Viewport with Maya-style camera controls (Alt+LMB orbit, Alt+MMB pan, Alt+RMB dolly,
/// scroll zoom) and LMB click-to-select / drag-to-marquee.
/// When a stage is loaded, renders via UsdImagingGL Engine and displays the result.
pub fn ui_viewport(
    ui: &mut egui::Ui,
    has_stage: bool,
    camera: &mut FreeCamera,
    data_model: &mut DataModel,
    mut engine: Option<&mut Engine>,
    viewport_state: &mut ViewportState,
    scene_cameras: &[(String, String)],
    actions: &mut Vec<AppAction>,
) {
    let vp_entry_t0 = Instant::now();
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
    log::trace!(
        "[TRACE] viewport: entry + allocate: {:.1}ms",
        vp_entry_t0.elapsed().as_secs_f64() * 1000.0
    );

    // Pick on LMB click (not drag, not Shift which starts marquee).
    // Supports modifier-based selection:
    //   Click       = replace selection
    //   Ctrl+Click  = toggle in selection
    //   Click empty = deselect all
    if has_stage
        && response.clicked()
        && !response.dragged()
        && response.interact_pointer_pos().is_some()
    {
        let (shift_held, ctrl_held) = ui.input(|i| (i.modifiers.shift, i.modifiers.ctrl));
        if !shift_held {
            if let Some(ref stage) = data_model.root.stage {
                let Some(pos) = response.interact_pointer_pos() else {
                    return;
                };
                let aspect = (rect.width() / rect.height().max(0.01)) as f64;
                let (view, proj) = resolve_cam_matrices(data_model, camera, aspect);
                let pick_fov = vertical_fov_from_projection(&proj).unwrap_or_else(|| camera.fov());
                let pick_camera_pos = view
                    .inverse()
                    .map(|inv| inv.transform_point(&Vec3d::new(0.0, 0.0, 0.0)))
                    .unwrap_or_else(|| camera.position());
                let mut picked_path = None;

                if let Some(eng) = engine.as_deref_mut() {
                    usd_trace::trace_scope!("viewport_click_pick");
                    // Fast path: current-frame GPU pick AOV lookup.
                    // Falls back to the explicit full-resolution ID pass on miss.
                    #[cfg(feature = "wgpu")]
                    {
                        usd_trace::trace_scope!("viewport_click_pick_id_buffer");
                        let px = (pos.x - rect.min.x).floor() as i32;
                        let py = (pos.y - rect.min.y).floor() as i32;
                        // Click selection prioritizes correctness: use the explicit
                        // full-resolution GPU ID pass, not the cached hover path.
                        if let Some(hit) = eng.pick_at_pixel_via_id_pass(px, py) {
                            let (path, _idx) = if !hit.hit_instancer_path.is_empty()
                                && data_model.view.pick_mode == PickMode::Instances
                            {
                                (hit.hit_instancer_path.clone(), hit.hit_instance_index)
                            } else {
                                pick::adjust_picked_path(
                                    stage.as_ref(),
                                    &hit.hit_prim_path,
                                    data_model.view.pick_mode,
                                )
                            };
                            picked_path = Some(path);
                        }
                    }

                    // Fallback: 1x1 frustum GPU pick (re-renders a tiny frustum)
                    if picked_path.is_none() {
                        usd_trace::trace_scope!("viewport_click_pick_fallback");
                        if let Some((pick_view, pick_proj)) =
                            build_pick_matrices_from_cursor(&view, &proj, rect, pos)
                        {
                            let root = stage.get_pseudo_root();
                            let params = build_render_params(data_model);
                            if let Some(results) = eng.test_intersection(
                                &PickParams::default(),
                                &pick_view,
                                &pick_proj,
                                &root,
                                &params,
                            ) {
                                if let Some(first) = results.first() {
                                    let (path, _idx) = if !first.hit_instancer_path.is_empty()
                                        && data_model.view.pick_mode == PickMode::Instances
                                    {
                                        (first.hit_instancer_path.clone(), first.hit_instance_index)
                                    } else {
                                        pick::adjust_picked_path(
                                            stage.as_ref(),
                                            &first.hit_prim_path,
                                            data_model.view.pick_mode,
                                        )
                                    };
                                    picked_path = Some(path);
                                }
                            }
                        }
                    }
                }

                // CPU ray-bbox fallback (debug / non-wgpu builds)
                if picked_path.is_none() {
                    let picked = pick::pick_prim_at_ex(
                        stage.as_ref(),
                        rect.min.x as f64,
                        rect.min.y as f64,
                        rect.width() as f64,
                        rect.height() as f64,
                        pos.x as f64,
                        pos.y as f64,
                        pick_camera_pos,
                        &view,
                        pick_fov,
                        aspect,
                        data_model.root.current_time,
                        data_model.view.display_render,
                        data_model.view.display_proxy,
                        data_model.view.display_guide,
                        data_model.view.pick_mode,
                    );
                    picked_path = picked.map(|h| h.path);
                }

                // Apply selection based on modifiers (C++ HdxPickTask parity)
                match picked_path {
                    Some(path) => {
                        if ctrl_held {
                            // Ctrl+Click: toggle prim in selection
                            data_model.selection.toggle_path(path);
                        } else {
                            // Plain click: replace selection
                            data_model.selection.switch_to_path(path);
                        }
                    }
                    None => {
                        // Click empty space: deselect all (unless modifier held)
                        if !ctrl_held {
                            data_model.selection.set_paths(vec![]);
                        }
                    }
                }
            }
        }
    }

    // --- Marquee/box selection ---
    // Shift+LMB drag starts a selection rectangle. On release, prims whose
    // screen-space bounding box intersects the marquee are selected.
    if has_stage {
        let shift_held = ui.input(|i| i.modifiers.shift);
        if shift_held && response.dragged_by(egui::PointerButton::Primary) {
            // Start marquee on first drag frame
            if viewport_state.marquee_start.is_none() {
                if let Some(pos) = response.interact_pointer_pos() {
                    viewport_state.marquee_start = Some(pos);
                }
            }
        }
        // On release, do frustum pick and apply selection
        if let Some(start) = viewport_state.marquee_start {
            if response.drag_stopped() {
                if let Some(end) = ui.input(|i| i.pointer.hover_pos()) {
                    let r = egui::Rect::from_two_pos(start, end);
                    // Only pick if the drag was meaningful (>4px in each axis)
                    if r.width() > 4.0 && r.height() > 4.0 {
                        viewport_state.marquee_rect = Some(r);
                        log::debug!(
                            "[marquee] selection rect: ({:.0},{:.0})-({:.0},{:.0})",
                            r.min.x,
                            r.min.y,
                            r.max.x,
                            r.max.y
                        );

                        // Frustum pick: project each prim bbox to screen, test overlap
                        if let Some(ref stage) = data_model.root.stage {
                            let aspect = (rect.width() / rect.height().max(0.01)) as f64;
                            let (view, proj) = resolve_cam_matrices(data_model, camera, aspect);
                            let ctrl_held = ui.input(|i| i.modifiers.ctrl);
                            let hits = marquee_pick_prims(
                                stage.as_ref(),
                                &view,
                                &proj,
                                rect,
                                r,
                                data_model.root.current_time,
                                data_model.view.display_render,
                                data_model.view.display_proxy,
                                data_model.view.display_guide,
                            );
                            if ctrl_held {
                                // Ctrl+Shift+drag: toggle each hit in selection
                                for p in &hits {
                                    data_model.selection.toggle_path(p.clone());
                                }
                            } else {
                                data_model.selection.set_paths(hits);
                            }
                        }
                    }
                }
                viewport_state.marquee_start = None;
            }
        }
    }

    // When user manipulates the viewport while a stage camera is active,
    // switch to free camera initialized from the stage camera's transform.
    // Matches C++ stageView.py switchToFreeCamera() (line 2045-2076).
    if data_model.view.active_camera().is_some() {
        let wants_cam_move = ui.input(|i| {
            let alt = i.modifiers.alt;
            let any_button = i.pointer.button_down(egui::PointerButton::Primary)
                || i.pointer.button_down(egui::PointerButton::Middle)
                || i.pointer.button_down(egui::PointerButton::Secondary);
            let has_scroll = i.raw_scroll_delta.y.abs() > 0.0;
            (alt && any_button && response.dragged()) || has_scroll
        });
        if wants_cam_move {
            // Initialize free camera from the stage camera's GfCamera
            if let Some(ref stage) = data_model.root.stage {
                if let Some(cam_path) = data_model.view.active_camera() {
                    let usd_cam = usd_app_utils::get_camera_at_path(stage.as_ref(), &cam_path);
                    if usd_cam.prim().is_valid() {
                        let tc = data_model.root.current_time.into();
                        let gf_cam = usd_cam.get_camera(tc);
                        *camera = FreeCamera::from_gf_camera(&gf_cam, camera.is_z_up());
                        log::info!("Switched to free camera from stage camera '{}'", cam_path);
                    }
                }
            }
            // Clear stage camera -- now using free camera
            data_model.view.active_camera_path = None;
        }
    }

    // Push camera state to undo stack on drag start (before camera is modified).
    if response.drag_started_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Middle)
        || response.drag_started_by(egui::PointerButton::Secondary)
    {
        viewport_state.push_camera_undo(camera);
    }

    // Camera controls: orbit (default), pan-tilt, or walk depending on mode.
    match camera_controls::process_input(
        ui,
        &response,
        camera,
        data_model.view.tumble_speed,
        data_model.view.zoom_speed,
        rect.height() as f64,
        InteractionMode::Orbit,
    ) {
        CameraAction::Repaint => ui.ctx().request_repaint(),
        CameraAction::FrameSelected => {
            let paths = data_model.selection.prims.clone();
            if paths.is_empty() {
                camera_controls::frame_all(camera, data_model, 1.1);
            } else {
                camera_controls::frame_selected(camera, data_model, &paths, 1.1);
            }
            ui.ctx().request_repaint();
        }
        CameraAction::FrameAll => {
            camera_controls::frame_all(camera, data_model, 1.1);
            ui.ctx().request_repaint();
        }
        CameraAction::None => {}
    }

    // Right-click context menu (matches usdviewq viewport context menu)
    viewport_context_menu(&response, ui, data_model, actions);

    let is_animating = data_model.root.playing || data_model.root.scrubbing;
    let pre_render_ms = vp_entry_t0.elapsed().as_secs_f64() * 1000.0;
    if pre_render_ms > 50.0 {
        log::info!(
            "[TRACE] viewport: pre-render (pick+camera+menu): {:.1}ms",
            pre_render_ms
        );
    }

    // Render USD scene when we have stage and engine
    let mut has_rendered_image = false;
    let mut hud_clip_planes = camera.clip_planes();
    if has_stage {
        if let Some(ref mut eng) = engine {
            if let Some(ref stage) = data_model.root.stage {
                let root = stage.get_pseudo_root();
                let width = rect.width().max(1.0) as i32;
                let height = rect.height().max(1.0) as i32;
                let aspect = (rect.width() / rect.height().max(0.01)) as f64;

                // Apply free camera FOV from settings
                camera.set_fov(data_model.view.free_camera_fov);

                // Use USD camera when active_camera_path is set, else FreeCamera
                let (view_matrix, proj_matrix) =
                    if let Some(cam_path) = data_model.view.active_camera() {
                        let usd_cam = usd_app_utils::get_camera_at_path(stage.as_ref(), &cam_path);
                        if usd_cam.prim().is_valid() {
                            let tc = data_model.root.current_time.into();
                            let clip = usd_cam.get_camera(tc).clipping_range();
                            hud_clip_planes = (clip.min() as f64, clip.max() as f64);
                            match (
                                usd_cam.compute_view_matrix(tc),
                                usd_cam.compute_projection_matrix(tc),
                            ) {
                                (Some(v), Some(p)) => (v, p),
                                _ => (camera.view_matrix(), camera.projection_matrix(aspect)),
                            }
                        } else {
                            (camera.view_matrix(), camera.projection_matrix(aspect))
                        }
                    } else {
                        (camera.view_matrix(), camera.projection_matrix(aspect))
                    };

                // Clip plane adjustments only apply to the free camera.
                // When a USD scene camera is active, it has its own authored clip planes.
                let using_usd_cam = data_model.view.active_camera().is_some();
                let (view_matrix, proj_matrix) = if !using_usd_cam
                    && data_model.view.free_camera_override_near.is_some()
                    && data_model.view.free_camera_override_far.is_some()
                {
                    // User override from the Adjust Free Camera dialog
                    let near = data_model.view.free_camera_override_near.unwrap();
                    let far = data_model.view.free_camera_override_far.unwrap();
                    camera.set_clip_planes(near, far);
                    (view_matrix, camera.projection_matrix(aspect))
                } else if !using_usd_cam {
                    // Skip expensive stage bbox during animation — reuse last clip planes.
                    // compute_stage_bbox_for_view traverses ALL prims (~1.5s for flo.usdz).
                    // C++ usdview also skips full recompute during playback.
                    let (near, far) = if is_animating {
                        camera.clip_planes()
                    } else {
                        camera_controls::auto_clip_planes(data_model, camera)
                    };
                    log::debug!(
                        "[clip] stage bbox near={:.6} far={:.6} cam_dist={:.6} auto_toggle={}",
                        near,
                        far,
                        camera.dist(),
                        data_model.view.auto_compute_clipping_planes
                    );
                    // Keep the free-camera projection deterministic by
                    // feeding the computed near/far back through the camera
                    // object; HUD/debug consumers read the effective clip
                    // planes from the camera state after this point.
                    camera.set_clip_planes(near, far);
                    hud_clip_planes = (near, far);
                    (view_matrix, camera.projection_matrix(aspect))
                } else {
                    // USD camera: use its authored projection matrix unchanged.
                    // Apply ConformWindow to ensure no stretching when viewport aspect
                    // differs from the camera's authored aperture aspect.
                    let conformed_proj =
                        conform_window_matrix(proj_matrix, ConformWindowPolicy::Fit, aspect);
                    (view_matrix, conformed_proj)
                };

                // Z-up correction is now built into FreeCamera.view_matrix()

                eng.set_render_buffer_size(Vec2i::new(width, height));
                eng.set_camera_state(view_matrix, proj_matrix);
                log::debug!(
                    "[viewport] before set_time frame={}",
                    data_model.root.current_time
                );
                eng.set_time(data_model.root.current_time.into());
                log::debug!(
                    "[viewport] after set_time frame={}",
                    data_model.root.current_time
                );
                if eng.selected_paths() != data_model.selection.prims.as_slice() {
                    eng.set_selected(data_model.selection.prims.clone());
                }
                eng.set_selection_color(data_model.view.highlight_color.to_vec4f());
                eng.set_display_unloaded_prims_with_bounds(data_model.view.show_bboxes);

                let params = build_render_params(data_model);
                log::debug!(
                    "[viewport] render {}x{} draw_mode={:?}",
                    width,
                    height,
                    data_model.view.draw_mode
                );
                let render_t0 = Instant::now();
                match data_model.view.draw_mode {
                    ViewDrawMode::HiddenSurfaceWireframe => {
                        // Pass 1: depth prepass - rasterize filled geometry writing
                        // only depth, suppressing color output. Materials disabled
                        // so alpha-cutout surfaces don't create holes in the depth.
                        let mut prepass = params.clone();
                        prepass.draw_mode = EngineDrawMode::ShadedSmooth;
                        prepass.depth_only = true;
                        prepass.highlight = false;
                        prepass.enable_scene_materials = false;
                        eng.render(&root, &prepass);

                        // Pass 2: wireframe overlay, depth-tested against the
                        // prepass so back-facing / hidden edges are occluded.
                        // preserve_depth keeps the depth buffer from Pass 1.
                        let mut wire = params.clone();
                        wire.draw_mode = EngineDrawMode::Wireframe;
                        wire.preserve_depth = true;
                        wire.enable_scene_materials = false;
                        eng.render(&root, &wire);
                    }
                    ViewDrawMode::WireframeOnSurface => {
                        // Shaded surface first, then wireframe overlay on top.
                        let mut shaded = params.clone();
                        shaded.draw_mode = EngineDrawMode::ShadedSmooth;
                        eng.render(&root, &shaded);

                        let mut wire = params.clone();
                        wire.draw_mode = EngineDrawMode::Wireframe;
                        wire.enable_scene_materials = false;
                        eng.render(&root, &wire);
                    }
                    ViewDrawMode::Bounds => {
                        // Bounds mode: skip geometry, only draw bbox overlays.
                        // Clear the framebuffer so we get a clean background.
                        eng.render_clear(&params);
                        // BBox overlay is drawn later in draw_overlays.
                    }
                    _ => {
                        eng.render(&root, &params);
                    }
                }
                log::debug!(
                    "[viewport] after render frame={} draw_mode={:?}",
                    data_model.root.current_time,
                    data_model.view.draw_mode
                );

                // Feed render time to HUD (wall-clock, best estimate without GPU timers)
                let render_ms = render_t0.elapsed().as_secs_f64() * 1000.0;
                viewport_state.phase_times.render_ms = render_ms;
                viewport_state.hud.gpu_frame_ms = render_ms;
                log::trace!("[PERF] render dispatch: {:.2}ms", render_ms);

                // Color correction mode from view settings
                let cc_mode = data_model.view.color_correction_mode;
                let ocio_settings = &data_model.view.ocio_settings;
                viewport_state.phase_times.readback_ms = 0.0;
                viewport_state.phase_times.color_correct_ms = 0.0;
                viewport_state.phase_times.tex_upload_ms = 0.0;

                #[cfg(feature = "wgpu")]
                {
                    log::debug!(
                        "[viewport] before present_engine_color frame={}",
                        data_model.root.current_time
                    );
                    has_rendered_image = viewport_state.gpu_state.present_engine_color(
                        eng,
                        width as u32,
                        height as u32,
                        cc_mode,
                        &mut viewport_state.ocio_state,
                        ocio_settings,
                    );
                    log::debug!(
                        "[viewport] after present_engine_color frame={} presented={}",
                        data_model.root.current_time,
                        has_rendered_image
                    );
                }

                if !has_rendered_image {
                    let readback_t0 = Instant::now();
                    log::debug!(
                        "[viewport] before capture_current_frame frame={}",
                        data_model.root.current_time
                    );
                    if let Some((corrected, w, h)) =
                        capture_current_frame(eng, viewport_state, data_model)
                    {
                        viewport_state.phase_times.readback_ms =
                            readback_t0.elapsed().as_secs_f64() * 1000.0;
                        let tex_t0 = Instant::now();
                        update_viewport_texture(ui.ctx(), viewport_state, &corrected, w, h);
                        viewport_state.phase_times.tex_upload_ms =
                            tex_t0.elapsed().as_secs_f64() * 1000.0;
                        has_rendered_image = true;
                        log::debug!(
                            "[viewport] after capture_current_frame frame={} size={}x{}",
                            data_model.root.current_time,
                            w,
                            h
                        );
                    } else {
                        log::debug!(
                            "[viewport] capture_current_frame returned None frame={}",
                            data_model.root.current_time
                        );
                    }
                }
                let readback_ms = viewport_state.phase_times.readback_ms
                    + viewport_state.phase_times.color_correct_ms
                    + viewport_state.phase_times.tex_upload_ms;
                viewport_state.hud.readback_ms = readback_ms;
                log::trace!("[PERF] readback+cc: {:.2}ms", readback_ms);
            }
        }
    }

    // Track frame time for HUD FPS display
    let now = Instant::now();
    if let Some(last) = viewport_state.last_frame_time {
        viewport_state
            .hud
            .update_performance(now.duration_since(last));
    }
    viewport_state.last_frame_time = Some(now);

    // Background or rendered image
    if has_rendered_image {
        viewport_state.has_presented_frame = true;
        #[cfg(feature = "wgpu")]
        if let Some(texture_id) = viewport_state.gpu_state.texture_id() {
            let uv_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture_id, rect, uv_rect, Color32::WHITE);
        } else if let Some(ref texture) = viewport_state.texture_handle {
            let uv_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture.id(), rect, uv_rect, Color32::WHITE);
        }
        #[cfg(not(feature = "wgpu"))]
        if let Some(ref texture) = viewport_state.texture_handle {
            let uv_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture.id(), rect, uv_rect, Color32::WHITE);
        }
    } else {
        let bg = if has_stage {
            data_model.clear_color_color32()
        } else {
            Color32::from_rgb(24, 24, 28)
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        let hint = if has_stage {
            "Alt+LMB: orbit  Alt+MMB/RMB: pan  Scroll: zoom  F: frame  Click: select"
        } else {
            "Open a USD file (Ctrl+O) or drag & drop"
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            hint,
            egui::FontId::proportional(14.0),
            Color32::from_gray(128),
        );
    }

    log::trace!("[TRACE] viewport: after render+present, entering hover/overlay section");
    let hover_t0 = Instant::now();

    // --- Hover-highlight + Rollover prim info tooltip (P1-10) ---
    // When hovering with no button, pick the prim under cursor for highlight.
    // Also show tooltip if rollover_prim_info is enabled.
    viewport_state.highlighted_prim = None;
    if has_stage && response.hovered() {
        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if rect.contains(hover_pos) {
                // Only pick when not actively dragging (orbit or marquee)
                let any_drag = response.dragged();
                if !any_drag {
                    if let Some(ref stage) = data_model.root.stage {
                        if let Some(ref mut eng) = engine {
                            usd_trace::trace_scope!("viewport_hover_pick");
                            // Hover highlight must stay on the cheap current-frame GPU path.
                            // If the cached pick AOVs miss, we leave hover empty instead of
                            // paying an expensive exact-pick fallback on the UI thread.
                            let mut hover_prim_path: Option<usd_sdf::Path> = None;
                            #[cfg(feature = "wgpu")]
                            {
                                usd_trace::trace_scope!("viewport_hover_pick_id_buffer");
                                let px = (hover_pos.x - rect.min.x).floor() as i32;
                                let py = (hover_pos.y - rect.min.y).floor() as i32;
                                if let Some(hit) = eng.pick_at_pixel_from_current_aovs(px, py) {
                                    hover_prim_path = Some(hit.hit_prim_path.clone());
                                }
                            }

                            // Store highlighted prim for hover-highlight (P1-10)
                            if let Some(ref prim_path) = hover_prim_path {
                                viewport_state.highlighted_prim = Some(prim_path.clone());

                                // P1-6: Rich rollover prim info tooltip
                                // (ref: appController.py:5121-5290 onRollover)
                                if data_model.view.rollover_prim_info {
                                    if let Some(prim) = stage.get_prim_at_path(prim_path) {
                                        let tc = data_model.root.current_time;
                                        let tip = build_rollover_tooltip(&prim, tc);

                                        #[allow(deprecated)]
                                        egui::show_tooltip_at(
                                            ui.ctx(),
                                            ui.layer_id(),
                                            egui::Id::new("rollover_prim_info"),
                                            hover_pos + egui::vec2(16.0, 16.0),
                                            |ui| {
                                                ui.style_mut().wrap_mode =
                                                    Some(egui::TextWrapMode::Extend);
                                                for line in &tip {
                                                    ui.label(line.clone());
                                                }
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(ref mut eng) = engine {
        if let Some(ref prim_path) = viewport_state.highlighted_prim {
            let needs_update =
                eng.located_paths().len() != 1 || eng.located_paths().first() != Some(prim_path);
            if needs_update {
                eng.set_located(vec![prim_path.clone()]);
            }
        } else if !eng.located_paths().is_empty() {
            eng.clear_located();
        }
    }

    let hover_ms = hover_t0.elapsed().as_secs_f64() * 1000.0;
    if hover_ms > 5.0 {
        log::info!("[TRACE] viewport: hover_pick: {:.1}ms", hover_ms);
    }
    let overlay_t0 = Instant::now();

    // --- Overlays (drawn on top of the rendered image) ---

    // Sync HUD fields from data_model
    viewport_state
        .hud
        .set_complexity(data_model.view.complexity);
    viewport_state.hud.camera_name = match data_model.view.active_camera() {
        Some(path) => {
            let s = path.to_string();
            s.rsplit('/').next().unwrap_or("USD Cam").to_string()
        }
        None => "Free".to_string(),
    };
    // Sync HUD visibility toggles from data_model
    viewport_state.hud.visible = data_model.view.show_hud;
    viewport_state.hud.show_info = data_model.view.show_hud_info;
    viewport_state.hud.show_complexity = data_model.view.show_hud_complexity;
    viewport_state.hud.show_performance = data_model.view.show_hud_performance;
    viewport_state.hud.show_gpu_stats = data_model.view.show_hud_gpu_stats;
    viewport_state.hud.show_vbo_info = data_model.view.show_hud_vbo_info;

    // Feed per-phase timing into HUD
    viewport_state.hud.phase_render_ms = viewport_state.phase_times.render_ms;
    viewport_state.hud.phase_readback_ms = viewport_state.phase_times.readback_ms;
    viewport_state.hud.phase_cc_ms = viewport_state.phase_times.color_correct_ms;
    viewport_state.hud.phase_tex_ms = viewport_state.phase_times.tex_upload_ms;

    // Feed render stats from engine to HUD
    if let Some(ref eng) = engine {
        let (draw_items, meshes, _verts) = eng.render_stats();
        viewport_state.hud.draw_item_count = draw_items;
        viewport_state.hud.mesh_count = meshes;
        // gpu_frame_ms is fed from render() wall-clock timing above
    }

    // Feed current clip planes into HUD
    let (near, far) = hud_clip_planes;
    viewport_state.hud.near_clip = near;
    viewport_state.hud.far_clip = far;

    // Match usdview: avoid GUI-heavy HUD/BBox refresh work while the timeline is
    // actively animating or being scrubbed.
    let viewport_interacting = response.dragged()
        || ui.input(|i| {
            i.pointer.any_down()
                || i.raw_scroll_delta.x.abs() > 0.0
                || i.raw_scroll_delta.y.abs() > 0.0
        });
    let is_animating_or_interacting =
        data_model.root.playing || data_model.root.scrubbing || viewport_interacting;

    if viewport_interacting {
        viewport_state.interaction_hot_until = Some(Instant::now() + Duration::from_millis(180));
    }
    let interaction_hot = viewport_state
        .interaction_hot_until
        .map(|until| until > Instant::now())
        .unwrap_or(false);

    // Keep the viewport hot while the user is actively interacting with it,
    // and for a short trailing window afterwards.
    //
    // A one-shot `request_repaint()` is not robust enough here: on Windows,
    // wheel zoom and slow drags can arrive as sparse/coalesced events, which
    // makes the viewer advance one frame per burst even though each frame is
    // cheap. The short lease keeps camera motion visually continuous without
    // turning the viewport into an always-on animation loop.
    if interaction_hot {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }

    // Update prim stats when stage is present (throttled in HudState)
    if !is_animating_or_interacting {
        if let Some(ref stage) = data_model.root.stage {
            viewport_state.hud.update_prim_stats(stage.as_ref());
        }
    }

    // Determine current view/proj for overlay projections
    let aspect = (rect.width() / rect.height().max(0.01)) as f64;
    // Z-up correction is now built into FreeCamera.view_matrix()
    let view_matrix = camera.view_matrix();
    let proj_matrix = camera.projection_matrix(aspect);

    // Grid (drawn before other overlays -- below axis indicator).
    // Adaptive: spacing and extent scale with camera distance so the grid
    // stays useful at any zoom level, matching C++ usdview behavior.
    if data_model.view.show_grid {
        let cam_dist = camera.dist();
        // Snap spacing to a power-of-10 that keeps ~20-40 visible cells
        let raw = cam_dist / 20.0;
        let spacing = 10.0_f64.powf(raw.log10().floor());
        let extent = spacing * 20.0;
        overlays::draw_grid(
            ui,
            rect,
            &view_matrix,
            &proj_matrix,
            extent,
            spacing,
            10,
            camera.is_z_up(),
        );
    }

    // Camera mask (driven by View > Camera Mask menu)
    if data_model.view.camera_mask_mode != crate::data_model::CameraMaskMode::None {
        // Use locked free-camera aspect or viewport aspect as mask ratio
        let mask_aspect = if data_model.view.lock_free_camera_aspect {
            (rect.width() / rect.height().max(1.0)) as f64
        } else {
            viewport_state.overlays.camera_aspect
        };
        if mask_aspect > 0.0 {
            let mask_color = color32_from_f32(data_model.view.camera_mask_color);
            overlays::draw_camera_mask(ui, rect, mask_aspect, mask_color);
        }
        // Draw mask outline if enabled
        if data_model.view.show_mask_outline {
            overlays::draw_camera_mask_outline(ui, rect, mask_aspect);
        }
    }

    // Camera reticles (driven by View > Camera Reticles menu)
    if data_model.view.show_reticles_inside || data_model.view.show_reticles_outside {
        let reticle_color = color32_from_f32(data_model.view.camera_reticles_color);
        overlays::draw_reticles(ui, rect, reticle_color);
    }

    // Sync axes visibility from preferences via data_model
    viewport_state.overlays.show_axes = data_model.view.show_axes;

    // Axis indicator
    if viewport_state.overlays.show_axes {
        overlays::draw_axes(ui, rect, &view_matrix);
    }

    // Camera oracle frustum wireframes for scene cameras (when viewing through free camera)
    if data_model.view.display_camera_oracles && data_model.view.active_camera().is_none() {
        if let Some(ref stage) = data_model.root.stage {
            let tc: usd_sdf::TimeCode = data_model.root.current_time;
            // Iterate scene cameras and draw frustum wireframes
            for prim in stage.traverse() {
                if prim.type_name() == "Camera" {
                    let usd_cam = usd_geom::camera::Camera::new(prim);
                    if usd_cam.prim().is_valid() {
                        if let (Some(cv), Some(cp)) = (
                            usd_cam.compute_view_matrix(tc.into()),
                            usd_cam.compute_projection_matrix(tc.into()),
                        ) {
                            overlays::draw_camera_oracle(
                                ui,
                                rect,
                                &cv,
                                &cp,
                                &view_matrix,
                                &proj_matrix,
                            );
                        }
                    }
                }
            }
        }
    }

    // Bounds draw mode: draw per-prim bounding boxes for all prims in the scene.
    if data_model.view.draw_mode == ViewDrawMode::Bounds {
        if let Some(ref stage) = data_model.root.stage {
            let tc = data_model.root.current_time;
            let bbox_color = overlays::contrasting_bbox_color(data_model.clear_color_color32());
            draw_all_prim_bboxes(
                ui,
                rect,
                stage.as_ref(),
                tc,
                &view_matrix,
                &proj_matrix,
                bbox_color,
            );
        }
    }

    // BBox overlay for selected prims (suppressed during playback unless show_bbox_playback)
    let is_playing = data_model.root.playing || data_model.root.scrubbing;
    let show_bbox = (data_model.view.show_bboxes
        || data_model.view.show_aa_bbox
        || data_model.view.show_ob_box)
        && (!is_playing || data_model.view.show_bbox_playback);
    if show_bbox && !data_model.selection.prims.is_empty() {
        if let Some(ref stage) = data_model.root.stage {
            let tc = data_model.root.current_time;
            let bbox_color = overlays::contrasting_bbox_color(data_model.clear_color_color32());
            let (bmin, bmax) =
                compute_selection_bbox_for_overlay(stage.as_ref(), &data_model.selection.prims, tc);
            // Only draw if bbox is non-degenerate
            if bmin.x < bmax.x || bmin.y < bmax.y || bmin.z < bmax.z {
                overlays::draw_bbox(ui, rect, bmin, bmax, &view_matrix, &proj_matrix, bbox_color);
            }
        }
    }

    // Scene bbox overlay (full scene, different color from selection bbox)
    if data_model.view.show_bboxes && (!is_playing || data_model.view.show_bbox_playback) {
        if let Some((bmin, bmax)) = engine.as_ref().and_then(|e| e.scene_bbox()) {
            let scene_bmin = Vec3d::new(bmin[0] as f64, bmin[1] as f64, bmin[2] as f64);
            let scene_bmax = Vec3d::new(bmax[0] as f64, bmax[1] as f64, bmax[2] as f64);
            if scene_bmin.x < scene_bmax.x
                || scene_bmin.y < scene_bmax.y
                || scene_bmin.z < scene_bmax.z
            {
                overlays::draw_bbox(
                    ui,
                    rect,
                    scene_bmin,
                    scene_bmax,
                    &view_matrix,
                    &proj_matrix,
                    Color32::from_rgb(100, 100, 200),
                );
            }
        }
    }

    // --- Marquee rectangle overlay (P1-9) ---
    // Draw the selection rectangle while dragging.
    if let Some(start) = viewport_state.marquee_start {
        if let Some(cur) = ui.input(|i| i.pointer.hover_pos()) {
            let marquee = egui::Rect::from_two_pos(start, cur);
            let fill = Color32::from_rgba_unmultiplied(70, 130, 200, 40);
            let stroke = Stroke::new(1.0, Color32::from_rgb(100, 160, 230));
            ui.painter()
                .rect(marquee, 0.0, fill, stroke, egui::StrokeKind::Outside);
        }
    }

    // HUD (drawn on top of rendered image)
    let hud_top_y = viewport_state.hud.draw(ui, rect);

    // Viewport-local camera selector.
    //
    // Keep this on the cached scene-camera list from `ViewerApp::menu_state`
    // instead of re-enumerating cameras in the viewport every frame. The menu
    // path already refreshes that cache on stage-change invalidation, so the
    // overlay can stay interactive without reintroducing the old hot-frame
    // camera enumeration cost.
    if has_stage {
        let active_camera_path = data_model.view.active_camera_path.clone();
        let selected_label = active_camera_path
            .as_ref()
            .and_then(|path| {
                scene_cameras
                    .iter()
                    .find(|(cam_path, _)| cam_path == path)
                    .map(|(_, name)| name.as_str())
            })
            .unwrap_or("Free Camera");

        let area_pos = egui::pos2(rect.right() - 210.0, rect.top() + 28.0);
        egui::Area::new(egui::Id::new("viewport_camera_selector"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 160))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Camera")
                                    .monospace()
                                    .color(Color32::from_rgb(218, 165, 32)),
                            );
                            egui::ComboBox::from_id_salt("viewport_camera_combo")
                                .width(140.0)
                                .selected_text(selected_label)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            active_camera_path.is_none(),
                                            "Free Camera",
                                        )
                                        .clicked()
                                    {
                                        actions.push(AppAction::SetCamera(None));
                                        ui.close();
                                    }
                                    if !scene_cameras.is_empty() {
                                        ui.separator();
                                        for (cam_path, cam_name) in scene_cameras {
                                            let is_selected = active_camera_path.as_deref()
                                                == Some(cam_path.as_str());
                                            if ui.selectable_label(is_selected, cam_name).clicked()
                                            {
                                                actions.push(AppAction::SetCamera(Some(
                                                    cam_path.clone(),
                                                )));
                                                ui.close();
                                            }
                                        }
                                    }
                                });
                        });
                    });
            });
    }

    // --- Hover-highlight prim label (P1-10) ---
    // Drawn AFTER HUD, positioned above the bottom-left HUD block.
    if let Some(ref hp) = viewport_state.highlighted_prim {
        let text = hp.to_string();
        let pos = egui::pos2(rect.min.x + 8.0, hud_top_y - 6.0);
        // Shadow for readability
        ui.painter().text(
            pos + egui::vec2(1.0, 1.0),
            egui::Align2::LEFT_BOTTOM,
            &text,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(0, 0, 0),
        );
        ui.painter().text(
            pos,
            egui::Align2::LEFT_BOTTOM,
            &text,
            egui::FontId::monospace(11.0),
            Color32::from_rgb(180, 220, 255),
        );
    }

    let overlay_ms = overlay_t0.elapsed().as_secs_f64() * 1000.0;
    if overlay_ms > 5.0 {
        log::info!("[TRACE] viewport: overlays+hud: {:.1}ms", overlay_ms);
    }
    log::trace!("[TRACE] viewport: exit ui_viewport");
}

/// Compute union AABB of selected prim world bounds (for overlay draw).
fn compute_selection_bbox_for_overlay(
    stage: &usd_core::Stage,
    paths: &[usd_sdf::Path],
    time: usd_sdf::TimeCode,
) -> (usd_gf::vec3::Vec3d, usd_gf::vec3::Vec3d) {
    use usd_gf::vec3::Vec3d;
    let t = usd_geom::tokens::usd_geom_tokens();
    let purposes = vec![
        t.default_.clone(),
        t.proxy.clone(),
        t.render.clone(),
        t.guide.clone(),
    ];
    let mut bmin = Vec3d::new(f64::MAX, f64::MAX, f64::MAX);
    let mut bmax = Vec3d::new(f64::MIN, f64::MIN, f64::MIN);

    for path in paths {
        if let Some(prim) = stage.get_prim_at_path(path) {
            let imageable = usd_geom::imageable::Imageable::new(prim);
            if imageable.is_valid() {
                let bbox = compute_world_bound_for_purposes(&imageable, time, &purposes);
                let range = bbox.compute_aligned_range();
                let lo = range.min();
                let hi = range.max();
                bmin.x = bmin.x.min(lo.x);
                bmin.y = bmin.y.min(lo.y);
                bmin.z = bmin.z.min(lo.z);
                bmax.x = bmax.x.max(hi.x);
                bmax.y = bmax.y.max(hi.y);
                bmax.z = bmax.z.max(hi.z);
            }
        }
    }
    (bmin, bmax)
}

/// Convert [f32; 4] RGBA to egui Color32.
fn color32_from_f32(c: [f32; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}

/// Known geometry type names (Boundable leaf prims that have renderable extent).
const GEOM_TYPE_NAMES: &[&str] = &[
    "Mesh",
    "BasisCurves",
    "Points",
    "NurbsCurves",
    "NurbsPatch",
    "Capsule",
    "Cone",
    "Cube",
    "Cylinder",
    "Sphere",
    "Plane",
    "TetMesh",
    "PointInstancer",
];

/// Returns true if prim is a leaf geometry type (Boundable with extent).
fn is_geom_prim(prim: &usd_core::Prim) -> bool {
    let tn = prim.type_name();
    let name = tn.as_str();
    GEOM_TYPE_NAMES.iter().any(|&g| g == name)
}

/// Draw bounding boxes for all renderable prims in Bounds draw mode.
///
/// Only leaf geometry prims (Mesh, Curves, etc.) get individual bboxes.
/// Intermediate Xform/Scope prims are skipped to avoid redundant parent boxes.
fn draw_all_prim_bboxes(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    stage: &usd_core::Stage,
    time: usd_sdf::TimeCode,
    view_matrix: &Matrix4d,
    proj_matrix: &Matrix4d,
    color: Color32,
) {
    use usd_gf::vec3::Vec3d;

    let t = usd_geom::tokens::usd_geom_tokens();
    let purposes = vec![
        t.default_.clone(),
        t.proxy.clone(),
        t.render.clone(),
        t.guide.clone(),
    ];

    // Traverse all prims, draw per-prim bboxes only for leaf geometry
    for prim in stage.traverse() {
        if !is_geom_prim(&prim) {
            continue;
        }
        let imageable = usd_geom::imageable::Imageable::new(prim);
        if !imageable.is_valid() {
            continue;
        }
        let bbox = compute_world_bound_for_purposes(&imageable, time, &purposes);
        let range = bbox.compute_aligned_range();
        let lo = range.min();
        let hi = range.max();
        // Skip degenerate bboxes
        if lo.x >= hi.x && lo.y >= hi.y && lo.z >= hi.z {
            continue;
        }
        let bmin = Vec3d::new(lo.x, lo.y, lo.z);
        let bmax = Vec3d::new(hi.x, hi.y, hi.z);
        overlays::draw_bbox(ui, rect, bmin, bmax, view_matrix, proj_matrix, color);
    }
}

/// Marquee (box) selection: project each prim's world bbox to screen space
/// and collect prims whose screen AABB overlaps the marquee rectangle.
fn marquee_pick_prims(
    stage: &usd_core::Stage,
    view: &Matrix4d,
    proj: &Matrix4d,
    viewport_rect: egui::Rect,
    marquee: egui::Rect,
    time: usd_sdf::TimeCode,
    show_render: bool,
    show_proxy: bool,
    show_guide: bool,
) -> Vec<Path> {
    let t = usd_geom::tokens::usd_geom_tokens();
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

    let vp = *view * *proj;
    let vp_w = viewport_rect.width() as f64;
    let vp_h = viewport_rect.height() as f64;
    let vp_x = viewport_rect.min.x as f64;
    let vp_y = viewport_rect.min.y as f64;

    let mut hits = Vec::new();

    for prim in stage.traverse() {
        if !pick::is_pickable_prim_pub(&prim) {
            continue;
        }
        let imageable = usd_geom::imageable::Imageable::new(prim.clone());
        if !imageable.is_valid() {
            continue;
        }
        if imageable.compute_visibility(time) == t.invisible {
            continue;
        }
        let purpose = imageable.compute_purpose();
        if !purposes.contains(&purpose) {
            continue;
        }

        let bbox = compute_world_bound_for_purposes(&imageable, time, &purposes);
        let range = bbox.compute_aligned_range();
        if range.is_empty() {
            continue;
        }
        let lo = range.min();
        let hi = range.max();

        // Build 8 corners of world-space AABB
        let corners = [
            Vec3d::new(lo.x, lo.y, lo.z),
            Vec3d::new(hi.x, lo.y, lo.z),
            Vec3d::new(lo.x, hi.y, lo.z),
            Vec3d::new(hi.x, hi.y, lo.z),
            Vec3d::new(lo.x, lo.y, hi.z),
            Vec3d::new(hi.x, lo.y, hi.z),
            Vec3d::new(lo.x, hi.y, hi.z),
            Vec3d::new(hi.x, hi.y, hi.z),
        ];

        // Project to screen, accumulate screen-space AABB
        let mut s_min_x = f64::INFINITY;
        let mut s_min_y = f64::INFINITY;
        let mut s_max_x = f64::NEG_INFINITY;
        let mut s_max_y = f64::NEG_INFINITY;
        let mut behind_camera = false;

        for c in &corners {
            // Row-vector: clip = point * VP
            let cx = c.x * vp[0][0] + c.y * vp[1][0] + c.z * vp[2][0] + vp[3][0];
            let cy = c.x * vp[0][1] + c.y * vp[1][1] + c.z * vp[2][1] + vp[3][1];
            let cw = c.x * vp[0][3] + c.y * vp[1][3] + c.z * vp[2][3] + vp[3][3];

            if cw <= 0.0 {
                behind_camera = true;
                break;
            }

            let ndc_x = cx / cw;
            let ndc_y = cy / cw;

            // NDC [-1,1] → screen pixel
            let sx = vp_x + (ndc_x + 1.0) * 0.5 * vp_w;
            let sy = vp_y + (1.0 - ndc_y) * 0.5 * vp_h;

            s_min_x = s_min_x.min(sx);
            s_min_y = s_min_y.min(sy);
            s_max_x = s_max_x.max(sx);
            s_max_y = s_max_y.max(sy);
        }

        if behind_camera {
            continue;
        }

        // Check overlap with marquee rect
        let screen_rect = egui::Rect::from_min_max(
            egui::pos2(s_min_x as f32, s_min_y as f32),
            egui::pos2(s_max_x as f32, s_max_y as f32),
        );
        if marquee.intersects(screen_rect) {
            hits.push(prim.path().clone());
        }
    }

    hits
}

// ---------------------------------------------------------------------------
// P1-6: Rich rollover prim info tooltip
// (ref: appController.py:5121-5290 onRollover)
// ---------------------------------------------------------------------------

/// Build rollover tooltip lines for a hovered prim.
///
/// Returns a Vec of RichText lines matching the Python reference sections:
/// header, property summary, material, variantSets, instancing.
fn build_rollover_tooltip(prim: &usd_core::Prim, tc: usd_sdf::TimeCode) -> Vec<egui::RichText> {
    let mut lines: Vec<egui::RichText> = Vec::new();
    let path = prim.path();
    let type_name = prim.type_name();
    let type_str = if type_name.is_empty() {
        "Typeless"
    } else {
        type_name.as_str()
    };

    // Header: path + type
    lines.push(egui::RichText::new(path.to_string()).strong().size(12.0));
    lines.push(egui::RichText::new(format!("Type: {}", type_str)).size(11.0));

    // Property summary (ref: lines 5186-5234)
    let imageable = usd_geom::imageable::Imageable::new(prim.clone());
    if imageable.is_valid() {
        // Visibility
        let vis = imageable.compute_visibility(tc);
        lines.push(egui::RichText::new(format!("Visibility: {}", vis.as_str())).small());

        // Purpose
        let purpose = imageable.compute_purpose();
        if purpose.as_str() != "default" {
            let purpose_attr = imageable.get_purpose_attr();
            let direct = purpose_attr
                .get(tc)
                .and_then(|v| v.get::<usd_tf::Token>().cloned())
                .map(|t| t.get_text().to_string())
                .unwrap_or_default();
            let inherited = if direct == purpose.as_str() {
                ""
            } else {
                " (inherited)"
            };
            lines.push(
                egui::RichText::new(format!("Purpose: {}{}", purpose.as_str(), inherited)).small(),
            );
        }
    }

    // Gprim: doubleSided + orientation
    let gprim = usd_geom::gprim::Gprim::new(prim.clone());
    if gprim.is_valid() {
        if let Some(ds) = gprim.get_double_sided_attr().get(tc) {
            if let Some(&b) = ds.get::<bool>() {
                lines.push(egui::RichText::new(format!("doubleSided: {}", b)).small());
            }
        }
        if let Some(orient) = gprim.get_orientation_attr().get(tc) {
            if let Some(t) = orient.get::<usd_tf::Token>() {
                lines.push(egui::RichText::new(format!("orientation: {}", t.get_text())).small());
            }
        }
    }

    // Mesh: point count + subdivisionScheme
    let mesh = usd_geom::mesh::Mesh::new(prim.clone());
    if mesh.is_valid() {
        if let Some(pts) = mesh.point_based().get_points_attr().get(tc) {
            let n = pts.array_size();
            lines.push(egui::RichText::new(format!("{} points", n)).small());
        }
        if let Some(scheme) = mesh.get_subdivision_scheme_attr().get(tc) {
            if let Some(t) = scheme.get::<usd_tf::Token>() {
                lines.push(
                    egui::RichText::new(format!("subdivisionScheme: {}", t.get_text())).small(),
                );
            }
        }
    }

    // Material binding (ref: lines 5237-5273)
    let mat_api = usd_shade::MaterialBindingAPI::new(prim.clone());
    let mut binding_rel: Option<usd_core::Relationship> = None;
    let purpose_tok = usd_shade::tokens::tokens().all_purpose.clone();
    let mat = mat_api.compute_bound_material(&purpose_tok, &mut binding_rel, false);
    if mat.is_valid() {
        lines.push(egui::RichText::new(format!("Material: {}", mat.get_prim().path())).small());
    } else {
        lines.push(
            egui::RichText::new("No assigned Material")
                .small()
                .color(egui::Color32::from_rgb(140, 140, 140)),
        );
    }

    // VariantSets (ref: lines 5175-5181)
    let vsets = prim.get_variant_sets();
    let vset_names = vsets.get_names();
    if !vset_names.is_empty() {
        lines.push(egui::RichText::new("VariantSets:").strong().small());
        for name in &vset_names {
            let vs = vsets.get_variant_set(name);
            let sel = vs.get_variant_selection();
            lines.push(egui::RichText::new(format!("  {} = {}", name, sel)).small());
        }
    }

    // Instancing (ref: lines 5277-5283)
    if prim.is_instance() {
        let proto = prim.get_prototype();
        if proto.is_valid() {
            lines.push(egui::RichText::new(format!("Instance of: {}", proto.path())).small());
        }
    }

    lines
}
