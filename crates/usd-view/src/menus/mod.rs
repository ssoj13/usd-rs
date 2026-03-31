//! Full menu bar matching usdview reference (appController menus).
//!
//! File, Edit, Navigation, View, Render, Show, Window menus with keyboard shortcuts.

mod debug;
mod edit;
mod file;
mod navigation;
mod render;
mod show;
mod view;
mod window;

use crate::data_model::{
    CameraMaskMode, ClearColor, HighlightColor, PickMode, SelectionHighlightMode,
};
use crate::keyboard::AppAction;
use crate::recent_files::RecentFiles;
use std::path::PathBuf;

/// Render mode for viewport (extended from DrawMode, matching C++ HdRenderModes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderMode {
    Wireframe,
    WireframeOnSurface,
    #[default]
    SmoothShaded,
    FlatShaded,
    Points,
    GeomOnly,
    GeomFlat,
    GeomSmooth,
    HiddenSurfaceWireframe,
    /// Renders per-prim bounding boxes (maps to DrawMode::Bounds).
    Bounds,
}

impl RenderMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::WireframeOnSurface => "WireframeOnSurface",
            Self::SmoothShaded => "Smooth Shaded",
            Self::FlatShaded => "Flat Shaded",
            Self::Points => "Points",
            Self::GeomOnly => "Geom Only",
            Self::GeomFlat => "Geom Flat",
            Self::GeomSmooth => "Geom Smooth",
            Self::HiddenSurfaceWireframe => "Hidden Surface Wireframe",
            Self::Bounds => "Bounds",
        }
    }

    pub fn all() -> &'static [RenderMode] {
        &[
            Self::Wireframe,
            Self::WireframeOnSurface,
            Self::SmoothShaded,
            Self::FlatShaded,
            Self::Points,
            Self::GeomOnly,
            Self::GeomFlat,
            Self::GeomSmooth,
            Self::HiddenSurfaceWireframe,
            Self::Bounds,
        ]
    }
}

/// Color correction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorCorrection {
    Disabled,
    #[default]
    SRGB,
    OpenColorIO,
}

impl ColorCorrection {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::SRGB => "sRGB",
            Self::OpenColorIO => "OpenColorIO",
        }
    }
}

/// Re-export refinement complexity for menu/action code.
pub use crate::data_model::RefinementComplexity;

/// Ordered list of complexity presets.
pub const COMPLEXITY_PRESETS: &[RefinementComplexity] = RefinementComplexity::ORDERED;

/// View settings controlled by the menu (extended beyond DataModel).
/// These are stored here to avoid modifying data_model.rs.
#[derive(Debug)]
pub struct MenuState {
    // View menu
    pub render_mode: RenderMode,
    pub color_correction: ColorCorrection,
    pub show_guide_prims: bool,
    pub show_proxy_prims: bool,
    pub show_render_prims: bool,
    pub show_all_bboxes: bool,
    pub show_aa_bboxes: bool,
    pub show_ob_bboxes: bool,
    pub show_bboxes_during_playback: bool,
    pub camera_mask_outline: bool,
    pub camera_reticles_inside: bool,
    pub camera_reticles_outside: bool,
    pub enable_scene_materials: bool,
    pub enable_scene_lights: bool,
    pub cull_backfaces: bool,
    pub auto_clipping_planes: bool,
    pub ambient_light_only: bool,
    pub dome_light_enabled: bool,
    pub dome_light_textures_visible: bool,
    pub display_camera_oracles: bool,
    pub redraw_on_scrub: bool,
    pub interpolation_held: bool,
    pub use_extents_hint: bool,
    pub camera_mask_color: [f32; 4],
    pub camera_reticles_color: [f32; 4],

    // OCIO settings (synced with DataModel::OcioSettings)
    pub ocio_display: String,
    pub ocio_view: String,
    pub ocio_looks: String,
    /// Available (display_name, [view_names]) — populated from OcioCpuState.
    pub ocio_displays: Vec<(String, Vec<String>)>,
    /// Available OCIO colorspace names — populated from OcioCpuState.
    pub ocio_colorspaces: Vec<String>,
    /// Selected OCIO colorspace (empty = config default).
    pub ocio_colorspace: String,

    // Render menu
    pub pick_mode: PickMode,
    pub render_paused: bool,
    pub render_stopped: bool,
    /// Camera orthographic mode (synced with FreeCamera).
    pub orthographic: bool,
    pub show_hud: bool,
    pub show_hud_info: bool,
    pub show_hud_complexity: bool,
    pub show_hud_performance: bool,
    pub show_hud_gpu_stats: bool,
    pub show_hud_vbo_info: bool,

    // Show menu (prim tree filters)
    pub show_inactive_prims: bool,
    pub show_prototype_prims: bool,
    pub show_undefined_prims: bool,
    pub show_abstract_prims: bool,
    pub show_prim_display_names: bool,
    pub rollover_prim_info: bool,

    // Show menu (column visibility)
    pub show_type_column: bool,
    pub show_vis_column: bool,
    pub show_draw_mode_column: bool,
    pub show_guides_column: bool,

    // -- Renderer / AOV (populated from Engine queries)
    /// Available renderer plugin IDs (e.g. "HdStormRendererPlugin").
    pub renderer_plugins: Vec<(String, String)>,
    /// Current renderer plugin ID.
    pub current_renderer: String,
    /// Available AOV names (e.g. "color", "depth").
    pub renderer_aovs: Vec<String>,
    /// Current AOV name.
    pub current_aov: String,

    // -- Camera selection
    /// Scene camera prim paths (enumerated from stage).
    pub scene_cameras: Vec<(String, String)>,
    /// Active camera path (None = free camera).
    pub active_camera_path: Option<String>,

    // -- Debug menu
    pub debug_logging: bool,
    pub show_render_stats_overlay: bool,
    /// Whether the validation panel is open (synced from panel state).
    pub open_validation: bool,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::default(),
            color_correction: ColorCorrection::default(),
            show_guide_prims: false,
            show_proxy_prims: true,
            show_render_prims: true,
            show_all_bboxes: false,
            show_aa_bboxes: false,
            show_ob_bboxes: false,
            show_bboxes_during_playback: false,
            camera_mask_outline: false,
            camera_reticles_inside: false,
            camera_reticles_outside: false,
            enable_scene_materials: true,
            enable_scene_lights: true,
            cull_backfaces: true,
            auto_clipping_planes: false,
            ambient_light_only: true,
            dome_light_enabled: false,
            dome_light_textures_visible: true,
            display_camera_oracles: false,
            redraw_on_scrub: true,
            interpolation_held: false,
            use_extents_hint: true,
            camera_mask_color: [0.1, 0.1, 0.1, 1.0],
            camera_reticles_color: [0.0, 0.7, 1.0, 1.0],
            ocio_display: String::new(),
            ocio_view: String::new(),
            ocio_looks: String::new(),
            ocio_displays: Vec::new(),
            ocio_colorspaces: Vec::new(),
            ocio_colorspace: String::new(),
            pick_mode: PickMode::default(),
            render_paused: false,
            render_stopped: false,
            orthographic: false,
            show_hud: true,
            show_hud_info: true,
            show_hud_complexity: true,
            show_hud_performance: true,
            show_hud_gpu_stats: false,
            show_hud_vbo_info: false,
            show_inactive_prims: false,
            show_prototype_prims: false,
            show_undefined_prims: false,
            show_abstract_prims: false,
            show_prim_display_names: true,
            rollover_prim_info: false,
            show_type_column: true,
            show_vis_column: true,
            show_draw_mode_column: true,
            show_guides_column: true,
            renderer_plugins: Vec::new(),
            current_renderer: String::new(),
            renderer_aovs: Vec::new(),
            current_aov: "color".to_string(),
            scene_cameras: Vec::new(),
            active_camera_path: None,
            debug_logging: false,
            show_render_stats_overlay: false,
            open_validation: false,
        }
    }
}

/// Collects all actions triggered by menu clicks for the caller to dispatch.
#[derive(Debug, Default)]
pub struct MenuActions {
    pub actions: Vec<AppAction>,
    /// File to open from recent files submenu.
    pub open_recent: Option<PathBuf>,
    /// Save overrides to this path.
    pub save_overrides: bool,
    /// Save flattened to this path.
    pub save_flattened: bool,
    /// Save viewer image.
    pub save_image: bool,
    /// Copy viewer image to clipboard.
    pub copy_image: bool,
    /// Expand all prims in tree.
    pub expand_all: bool,
    /// Collapse all prims in tree.
    pub collapse_all: bool,
    /// Reopen the current stage.
    pub reopen_stage: bool,
    /// Open preferences dialog.
    pub open_preferences: bool,
    /// Select bound preview material.
    pub select_bound_preview_material: bool,
    /// Select bound full material.
    pub select_bound_full_material: bool,
    /// Select preview material binding relationship.
    pub select_preview_binding_rel: bool,
    /// Select full material binding relationship.
    pub select_full_binding_rel: bool,
    /// Expand prim tree to a specific depth.
    pub expand_to_depth: Option<usize>,
    /// Reset dock layout to default.
    pub reset_layout: bool,
    /// Open Adjust Free Camera dialog.
    pub adjust_free_camera: bool,
    /// Open Adjust Default Material dialog.
    pub adjust_default_material: bool,
    /// Open HDRI file picker for fallback dome light.
    pub load_hdri: bool,
    /// Toggle debug logging level.
    pub toggle_debug_logging: bool,
    /// Open TF_DEBUG flags dialog.
    pub open_debug_flags: bool,
    /// Open USD Validation panel.
    pub open_validation: bool,
}

/// Draws the full menu bar. Returns actions to be dispatched by the caller.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu_bar(
    ui: &mut egui::Ui,
    menu_state: &mut MenuState,
    recent_files: &RecentFiles,
    complexity: &mut f64,
    clear_color: &mut ClearColor,
    highlight_color: &mut HighlightColor,
    selection_highlight: &mut SelectionHighlightMode,
    camera_mask: &mut CameraMaskMode,
) -> MenuActions {
    let mut result = MenuActions::default();

    egui::MenuBar::new().ui(ui, |ui| {
        // Compact menu spacing to match native look
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.spacing_mut().button_padding.y = 1.0;

        file::file_menu(ui, recent_files, &mut result);
        edit::edit_menu(ui, &mut result);
        navigation::navigation_menu(ui, menu_state, &mut result);
        view::view_menu(
            ui,
            menu_state,
            &mut result,
            complexity,
            clear_color,
            highlight_color,
            selection_highlight,
            camera_mask,
        );
        render::render_menu(ui, menu_state, &mut result);
        show::show_menu(ui, menu_state, &mut result);
        window::window_menu(ui, &mut result);
        debug::debug_menu(ui, menu_state, &mut result);
    });

    result
}

/// Apply compact spacing to dropdown menus for a native-like dense layout.
fn compact_menu(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.spacing_mut().button_padding.y = 1.0;
}
