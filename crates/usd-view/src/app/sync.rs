//! View settings <-> menu state synchronization.

use crate::data_model::DrawMode;
use crate::menus::{ColorCorrection, RenderMode};

use super::ViewerApp;

impl ViewerApp {
    fn refresh_scene_cameras_if_needed(&mut self) {
        usd_trace::trace_scope!("viewer_refresh_scene_cameras_if_needed");
        if !self.scene_cameras_dirty {
            return;
        }

        self.menu_state.scene_cameras =
            enumerate_scene_cameras(self.data_model.root.stage.as_deref());
        self.scene_cameras_dirty = false;

        if let Some(active) = self.data_model.view.active_camera_path.clone() {
            let camera_still_exists = self
                .menu_state
                .scene_cameras
                .iter()
                .any(|(path, _)| path == &active);
            if !camera_still_exists {
                self.data_model
                    .view
                    .set_active_camera(None, self.data_model.root.stage.as_deref());
            }
        }
    }

    pub(crate) fn apply_view_settings_to_menu_state(&mut self) {
        self.menu_state.render_mode = match self.data_model.view.draw_mode {
            DrawMode::Wireframe => RenderMode::Wireframe,
            DrawMode::WireframeOnSurface => RenderMode::WireframeOnSurface,
            DrawMode::ShadedSmooth => RenderMode::SmoothShaded,
            DrawMode::ShadedFlat => RenderMode::FlatShaded,
            DrawMode::GeometryOnly => RenderMode::GeomOnly,
            DrawMode::Points => RenderMode::Points,
            DrawMode::GeomSmooth => RenderMode::GeomSmooth,
            DrawMode::GeomFlat => RenderMode::GeomFlat,
            DrawMode::HiddenSurfaceWireframe => RenderMode::HiddenSurfaceWireframe,
            DrawMode::Bounds => RenderMode::Bounds,
        };

        self.menu_state.pick_mode = self.data_model.view.pick_mode;

        self.menu_state.show_guide_prims = self.data_model.view.display_guide;
        self.menu_state.show_proxy_prims = self.data_model.view.display_proxy;
        self.menu_state.show_render_prims = self.data_model.view.display_render;

        self.menu_state.show_all_bboxes = self.data_model.view.show_bboxes;
        self.menu_state.show_aa_bboxes = self.data_model.view.show_aa_bbox;
        self.menu_state.show_ob_bboxes = self.data_model.view.show_ob_box;
        self.menu_state.show_bboxes_during_playback = self.data_model.view.show_bbox_playback;

        self.menu_state.auto_clipping_planes = self.data_model.view.auto_compute_clipping_planes;
        self.menu_state.cull_backfaces = self.data_model.view.cull_backfaces;
        self.menu_state.enable_scene_materials = self.data_model.view.enable_scene_materials;
        self.menu_state.enable_scene_lights = self.data_model.view.enable_scene_lights;

        self.menu_state.camera_mask_outline = self.data_model.view.show_mask_outline;
        self.menu_state.camera_reticles_inside = self.data_model.view.show_reticles_inside;
        self.menu_state.camera_reticles_outside = self.data_model.view.show_reticles_outside;

        self.menu_state.ambient_light_only = self.data_model.view.ambient_light_only;
        self.menu_state.dome_light_enabled = self.data_model.view.dome_light_enabled;
        self.menu_state.dome_light_textures_visible =
            self.data_model.view.dome_light_textures_visible;
        self.menu_state.display_camera_oracles = self.data_model.view.display_camera_oracles;
        self.menu_state.redraw_on_scrub = self.data_model.view.redraw_on_scrub;
        self.menu_state.interpolation_held = self.data_model.view.interpolation_held;
        self.menu_state.use_extents_hint = self.data_model.view.use_extents_hint;
        self.menu_state.camera_mask_color = self.data_model.view.camera_mask_color;
        self.menu_state.camera_reticles_color = self.data_model.view.camera_reticles_color;

        self.menu_state.show_hud = self.data_model.view.show_hud;
        self.menu_state.show_hud_info = self.data_model.view.show_hud_info;
        self.menu_state.show_hud_complexity = self.data_model.view.show_hud_complexity;
        self.menu_state.show_hud_performance = self.data_model.view.show_hud_performance;
        self.menu_state.show_hud_gpu_stats = self.data_model.view.show_hud_gpu_stats;
        self.menu_state.show_hud_vbo_info = self.data_model.view.show_hud_vbo_info;

        self.menu_state.show_inactive_prims = self.data_model.view.show_inactive_prims;
        self.menu_state.show_prototype_prims = self.data_model.view.show_all_prototype_prims;
        self.menu_state.show_undefined_prims = self.data_model.view.show_undefined_prims;
        self.menu_state.show_abstract_prims = self.data_model.view.show_abstract_prims;
        self.menu_state.show_prim_display_names = self.data_model.view.show_prim_display_names;
        self.menu_state.rollover_prim_info = self.data_model.view.rollover_prim_info;

        // Color correction mode
        self.menu_state.color_correction = match self.data_model.view.color_correction_mode {
            crate::data_model::ColorCorrectionMode::Disabled => ColorCorrection::Disabled,
            crate::data_model::ColorCorrectionMode::SRGB => ColorCorrection::SRGB,
            crate::data_model::ColorCorrectionMode::OpenColorIO => ColorCorrection::OpenColorIO,
        };

        // OCIO settings: DataModel → MenuState
        self.menu_state.ocio_display = self.data_model.view.ocio_settings.display.clone();
        self.menu_state.ocio_view = self.data_model.view.ocio_settings.view.clone();
        self.menu_state.ocio_colorspace = self.data_model.view.ocio_settings.color_space.clone();
        self.menu_state.ocio_looks = self.data_model.view.ocio_settings.looks.clone();

        // Populate available OCIO displays from viewport's OCIO state (lazy-loaded).
        if self.menu_state.ocio_displays.is_empty()
            && self.menu_state.color_correction == ColorCorrection::OpenColorIO
        {
            self.viewport_state.ocio_state.load_config();
            self.menu_state.ocio_displays =
                self.viewport_state.ocio_state.available_displays().to_vec();
            self.menu_state.ocio_colorspaces = self
                .viewport_state
                .ocio_state
                .available_colorspaces()
                .to_vec();
        }

        // Populate renderer plugins from Engine (lazy, once)
        if self.menu_state.renderer_plugins.is_empty() {
            let plugins = self.engine.get_renderer_plugins();
            self.menu_state.renderer_plugins = plugins
                .iter()
                .map(|id| {
                    let name = usd_imaging::gl::Engine::get_renderer_display_name(id);
                    (id.as_str().to_string(), name)
                })
                .collect();
            self.menu_state.current_renderer =
                self.engine.get_current_renderer_id().as_str().to_string();
        }

        // Populate AOVs from Engine (lazy, once)
        if self.menu_state.renderer_aovs.is_empty() {
            let aovs = self.engine.get_renderer_aovs();
            self.menu_state.renderer_aovs = aovs.iter().map(|t| t.as_str().to_string()).collect();
        }

        self.refresh_scene_cameras_if_needed();

        // Sync active camera path from DataModel
        self.menu_state.active_camera_path = self.data_model.view.active_camera_path.clone();
    }

    pub(crate) fn apply_menu_state_to_view_settings(&mut self) {
        self.data_model.view.draw_mode = match self.menu_state.render_mode {
            RenderMode::Wireframe => DrawMode::Wireframe,
            RenderMode::WireframeOnSurface => DrawMode::WireframeOnSurface,
            RenderMode::SmoothShaded => DrawMode::ShadedSmooth,
            RenderMode::FlatShaded => DrawMode::ShadedFlat,
            RenderMode::Points => DrawMode::Points,
            RenderMode::GeomOnly => DrawMode::GeometryOnly,
            RenderMode::GeomFlat => DrawMode::GeomFlat,
            RenderMode::GeomSmooth => DrawMode::GeomSmooth,
            RenderMode::HiddenSurfaceWireframe => DrawMode::HiddenSurfaceWireframe,
            RenderMode::Bounds => DrawMode::Bounds,
        };

        self.data_model.view.pick_mode = self.menu_state.pick_mode;

        // Track purpose/extents changes to invalidate bbox/xform caches
        let old_guide = self.data_model.view.display_guide;
        let old_proxy = self.data_model.view.display_proxy;
        let old_render = self.data_model.view.display_render;
        let old_extents = self.data_model.view.use_extents_hint;

        self.data_model.view.display_guide = self.menu_state.show_guide_prims;
        self.data_model.view.display_proxy = self.menu_state.show_proxy_prims;
        self.data_model.view.display_render = self.menu_state.show_render_prims;

        self.data_model.view.show_bboxes = self.menu_state.show_all_bboxes
            || self.menu_state.show_aa_bboxes
            || self.menu_state.show_ob_bboxes;
        self.data_model.view.show_aa_bbox = self.menu_state.show_aa_bboxes;
        self.data_model.view.show_ob_box = self.menu_state.show_ob_bboxes;
        self.data_model.view.show_bbox_playback = self.menu_state.show_bboxes_during_playback;

        self.data_model.view.auto_compute_clipping_planes = self.menu_state.auto_clipping_planes;
        self.data_model.view.cull_backfaces = self.menu_state.cull_backfaces;
        self.data_model.view.enable_scene_materials = self.menu_state.enable_scene_materials;
        self.data_model.view.enable_scene_lights = self.menu_state.enable_scene_lights;

        self.data_model.view.show_mask_outline = self.menu_state.camera_mask_outline;
        self.data_model.view.show_reticles_inside = self.menu_state.camera_reticles_inside;
        self.data_model.view.show_reticles_outside = self.menu_state.camera_reticles_outside;

        self.data_model.view.ambient_light_only = self.menu_state.ambient_light_only;
        self.data_model.view.dome_light_enabled = self.menu_state.dome_light_enabled;
        self.data_model.view.dome_light_textures_visible =
            self.menu_state.dome_light_textures_visible;
        self.data_model.view.display_camera_oracles = self.menu_state.display_camera_oracles;
        self.data_model.view.redraw_on_scrub = self.menu_state.redraw_on_scrub;
        self.data_model.view.interpolation_held = self.menu_state.interpolation_held;
        // Apply interpolation type to stage so attribute value resolution uses it
        if let Some(ref stage) = self.data_model.root.stage {
            use usd_core::InterpolationType;
            stage.set_interpolation_type(if self.data_model.view.interpolation_held {
                InterpolationType::Held
            } else {
                InterpolationType::Linear
            });
        }
        self.data_model.view.use_extents_hint = self.menu_state.use_extents_hint;

        // Clear caches when purpose visibility or extents_hint changes
        // (per Python rootDataModel.py:109-157)
        if self.data_model.view.display_guide != old_guide
            || self.data_model.view.display_proxy != old_proxy
            || self.data_model.view.display_render != old_render
            || self.data_model.view.use_extents_hint != old_extents
        {
            self.data_model.clear_caches();
        }

        self.data_model.view.camera_mask_color = self.menu_state.camera_mask_color;
        self.data_model.view.camera_reticles_color = self.menu_state.camera_reticles_color;

        self.data_model.view.show_hud = self.menu_state.show_hud;
        self.data_model.view.show_hud_info = self.menu_state.show_hud_info;
        self.data_model.view.show_hud_complexity = self.menu_state.show_hud_complexity;
        self.data_model.view.show_hud_performance = self.menu_state.show_hud_performance;
        self.data_model.view.show_hud_gpu_stats = self.menu_state.show_hud_gpu_stats;
        self.data_model.view.show_hud_vbo_info = self.menu_state.show_hud_vbo_info;

        self.data_model.view.show_inactive_prims = self.menu_state.show_inactive_prims;
        self.data_model.view.show_all_prototype_prims = self.menu_state.show_prototype_prims;
        self.data_model.view.show_undefined_prims = self.menu_state.show_undefined_prims;
        self.data_model.view.show_abstract_prims = self.menu_state.show_abstract_prims;
        self.data_model.view.show_prim_display_names = self.menu_state.show_prim_display_names;
        // show_tooltips from Preferences gates the rollover prim info feature:
        // if the user disables tooltips globally, rollover is suppressed even if
        // the Show menu toggle is on.
        self.data_model.view.rollover_prim_info =
            self.menu_state.rollover_prim_info && self.prefs_state.settings.show_tooltips;

        // Color correction mode
        self.data_model.view.color_correction_mode = match self.menu_state.color_correction {
            ColorCorrection::Disabled => crate::data_model::ColorCorrectionMode::Disabled,
            ColorCorrection::SRGB => crate::data_model::ColorCorrectionMode::SRGB,
            ColorCorrection::OpenColorIO => crate::data_model::ColorCorrectionMode::OpenColorIO,
        };

        // OCIO settings: MenuState → DataModel
        self.data_model.view.ocio_settings.display = self.menu_state.ocio_display.clone();
        self.data_model.view.ocio_settings.view = self.menu_state.ocio_view.clone();
        self.data_model.view.ocio_settings.looks = self.menu_state.ocio_looks.clone();
        self.data_model.view.ocio_settings.color_space = self.menu_state.ocio_colorspace.clone();

        // Sync grid preference into data model so viewport can read it
        self.data_model.view.show_grid = self.prefs_state.settings.show_grid;

        // Sync preferences -> data_model for fields that have direct equivalents
        self.data_model.view.font_size = self.prefs_state.settings.font_size as u32;
        self.data_model.view.free_camera_fov = self.prefs_state.settings.default_fov as f64;

        // Sync free camera clip plane overrides from toolbar dialog
        self.data_model.view.free_camera_override_near = self.free_camera_override_near;
        self.data_model.view.free_camera_override_far = self.free_camera_override_far;

        // Camera sensitivity (Preferences > Camera)
        self.data_model.view.tumble_speed = self.prefs_state.settings.tumble_speed;
        self.data_model.view.zoom_speed = self.prefs_state.settings.zoom_speed;
        // Default clip planes used when neither auto-clip nor dialog override is active
        self.data_model.view.default_near_clip = self.prefs_state.settings.near_clip;
        self.data_model.view.default_far_clip = self.prefs_state.settings.far_clip;

        // Axis overlay (Preferences > Viewport)
        self.data_model.view.show_axes = self.prefs_state.settings.show_axes;

        // NOTE: background ClearColor and default_render_mode DrawMode are applied once
        // at startup in launcher.rs (after prefs are restored) so that live menu changes
        // are not stomped on every frame by this sync function.

        // NOTE: selection highlight_color is applied at startup in launcher.rs.
        // It is NOT re-applied here to avoid overriding live View menu changes.

        // Default material roughness (Preferences > Materials).
        // NOTE: default_material_ambient and default_material_specular are controlled
        // by the "Adjust Default Material" dialog and stored in ViewSettingsDataModel
        // directly — they are NOT overridden by prefs here (which would stomp the
        // dialog changes). Only roughness (a separate viewport hint) comes from prefs.
        self.data_model.view.default_material_roughness =
            self.prefs_state.settings.default_roughness;
    }
}

/// Map an arbitrary sRGB color to the nearest HighlightColor enum variant.
///
/// Uses squared-distance in linear RGB so yellow and cyan are handled correctly.
pub(crate) fn nearest_highlight_color(c: egui::Color32) -> crate::data_model::HighlightColor {
    use crate::data_model::HighlightColor;

    // Named candidates as (R, G, B) in 0-255
    let candidates: &[(u8, u8, u8, HighlightColor)] = &[
        (255, 255, 255, HighlightColor::White),
        (255, 255, 0, HighlightColor::Yellow),
        (0, 255, 255, HighlightColor::Cyan),
    ];

    let r = c.r() as i32;
    let g = c.g() as i32;
    let b = c.b() as i32;

    candidates
        .iter()
        .min_by_key(|(cr, cg, cb, _)| {
            let dr = r - *cr as i32;
            let dg = g - *cg as i32;
            let db = b - *cb as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(_, _, _, variant)| *variant)
        .unwrap_or(HighlightColor::Yellow)
}

/// Traverse stage for UsdGeomCamera prims. Returns (path, display_name) pairs.
fn enumerate_scene_cameras(stage: Option<&usd_core::Stage>) -> Vec<(String, String)> {
    usd_trace::trace_scope!("viewer_enumerate_scene_cameras");
    let Some(stage) = stage else {
        return Vec::new();
    };
    let camera_token = usd_tf::Token::new("Camera");
    let mut cameras = Vec::new();
    let mut stack = vec![stage.get_pseudo_root()];
    while let Some(prim) = stack.pop() {
        if !prim.is_valid() {
            continue;
        }
        if prim.is_a(&camera_token) {
            let path = prim.path().to_string();
            let name = prim.name().to_string();
            cameras.push((path, name));
        }
        for child in prim.get_all_children() {
            stack.push(child);
        }
    }
    cameras.sort_by(|a, b| a.0.cmp(&b.0));
    cameras
}
