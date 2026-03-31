//! Centralized JSON persistence for all viewer state.
//!
//! Single file: <config_dir>/usdview/app_state.json
//! Replaces scattered eframe storage and RecentFiles JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::panels::preferences::{
    BgPreset, DefaultLightType, DefaultRenderMode, PreferencesSettings, UiTheme,
};

// ---------------------------------------------------------------------------
// Serde-friendly mirror of PreferencesSettings
// ---------------------------------------------------------------------------

/// JSON-serializable mirror of PreferencesSettings.
/// Uses [u8; 3] for colors (sRGB) instead of egui::Color32.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PreferencesSettingsJson {
    pub ui_theme: String,
    pub font_size: f32,
    pub redraw_on_scrub: bool,
    pub step_size: f64,
    pub show_tooltips: bool,
    pub default_render_mode: String,
    pub background: String,
    pub show_hud: bool,
    pub show_axes: bool,
    pub show_grid: bool,
    pub selection_highlight: [u8; 3],
    pub default_fov: f32,
    pub near_clip: f64,
    pub far_clip: f64,
    pub tumble_speed: f32,
    pub zoom_speed: f32,
    pub default_light_type: String,
    pub ambient_intensity: f32,
    pub key_light_intensity: f32,
    pub default_diffuse: [u8; 3],
    pub default_specular: [u8; 3],
    pub default_roughness: f32,
    pub tessellation_level: u32,
    pub gpu_instancing: bool,
    pub renderer_plugin: String,
}

impl Default for PreferencesSettingsJson {
    fn default() -> Self {
        let d = PreferencesSettings::default();
        Self::from(&d)
    }
}

impl From<&PreferencesSettings> for PreferencesSettingsJson {
    fn from(s: &PreferencesSettings) -> Self {
        Self {
            ui_theme: s.ui_theme.name().to_string(),
            font_size: s.font_size,
            redraw_on_scrub: s.redraw_on_scrub,
            step_size: s.step_size,
            show_tooltips: s.show_tooltips,
            default_render_mode: s.default_render_mode.name().to_string(),
            background: s.background.name().to_string(),
            show_hud: s.show_hud,
            show_axes: s.show_axes,
            show_grid: s.show_grid,
            selection_highlight: [
                s.selection_highlight_color.r(),
                s.selection_highlight_color.g(),
                s.selection_highlight_color.b(),
            ],
            default_fov: s.default_fov,
            near_clip: s.near_clip,
            far_clip: s.far_clip,
            tumble_speed: s.tumble_speed,
            zoom_speed: s.zoom_speed,
            default_light_type: s.default_light_type.name().to_string(),
            ambient_intensity: s.ambient_intensity,
            key_light_intensity: s.key_light_intensity,
            default_diffuse: [
                s.default_diffuse.r(),
                s.default_diffuse.g(),
                s.default_diffuse.b(),
            ],
            default_specular: [
                s.default_specular.r(),
                s.default_specular.g(),
                s.default_specular.b(),
            ],
            default_roughness: s.default_roughness,
            tessellation_level: s.tessellation_level,
            gpu_instancing: s.gpu_instancing,
            renderer_plugin: s.renderer_plugin.clone(),
        }
    }
}

impl PreferencesSettingsJson {
    /// Convert back to PreferencesSettings, matching string names to enum variants.
    pub fn to_prefs(&self) -> PreferencesSettings {
        PreferencesSettings {
            ui_theme: match self.ui_theme.as_str() {
                "Light" => UiTheme::Light,
                _ => UiTheme::Dark,
            },
            font_size: self.font_size,
            redraw_on_scrub: self.redraw_on_scrub,
            step_size: self.step_size,
            show_tooltips: self.show_tooltips,
            default_render_mode: match self.default_render_mode.as_str() {
                "Flat Shaded" => DefaultRenderMode::FlatShaded,
                "Wireframe" => DefaultRenderMode::Wireframe,
                "Wireframe On Surface" => DefaultRenderMode::WireframeOnSurface,
                "Points" => DefaultRenderMode::Points,
                _ => DefaultRenderMode::SmoothShaded,
            },
            background: match self.background.as_str() {
                "Black" => BgPreset::Black,
                "Gradient" => BgPreset::Gradient,
                _ => BgPreset::DarkGrey,
            },
            show_hud: self.show_hud,
            show_axes: self.show_axes,
            show_grid: self.show_grid,
            selection_highlight_color: egui::Color32::from_rgb(
                self.selection_highlight[0],
                self.selection_highlight[1],
                self.selection_highlight[2],
            ),
            default_fov: self.default_fov,
            near_clip: self.near_clip,
            far_clip: self.far_clip,
            tumble_speed: self.tumble_speed,
            zoom_speed: self.zoom_speed,
            default_light_type: match self.default_light_type.as_str() {
                "Distant Light" => DefaultLightType::DistantLight,
                "Sphere Light" => DefaultLightType::SphereLight,
                "None" => DefaultLightType::None,
                _ => DefaultLightType::DomeLight,
            },
            ambient_intensity: self.ambient_intensity,
            key_light_intensity: self.key_light_intensity,
            default_diffuse: egui::Color32::from_rgb(
                self.default_diffuse[0],
                self.default_diffuse[1],
                self.default_diffuse[2],
            ),
            default_specular: egui::Color32::from_rgb(
                self.default_specular[0],
                self.default_specular[1],
                self.default_specular[2],
            ),
            default_roughness: self.default_roughness,
            tessellation_level: self.tessellation_level,
            gpu_instancing: self.gpu_instancing,
            renderer_plugin: self.renderer_plugin.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level state struct
// ---------------------------------------------------------------------------

/// Current persist format version.
pub const PERSIST_VERSION: u32 = 1;

/// All persisted application state, stored as a single JSON file.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppPersistState {
    /// Format version for future migration support.
    pub version: u32,
    /// Last successfully loaded file.
    pub last_file: Option<PathBuf>,
    /// Recent files list (most recent first).
    pub recent_files: Vec<PathBuf>,
    /// All preference settings.
    pub preferences: PreferencesSettingsJson,
    /// RON-serialized DockState<DockTab> (kept as RON for egui_dock compat).
    pub dock_layout: Option<String>,
    /// RON-serialized ViewSettingsDataModel (kept as RON for compat).
    pub view_settings: Option<String>,
    /// Window position [x, y] (managed by eframe persist_window, stored for reference).
    pub window_pos: Option<[f32; 2]>,
    /// Window size [w, h].
    pub window_size: Option<[f32; 2]>,
    /// Named dock layouts: name -> RON-serialized DockState<DockTab>.
    #[serde(default)]
    pub layouts: HashMap<String, String>,
    /// Currently active layout name (None = no named layout selected).
    #[serde(default)]
    pub current_layout: Option<String>,
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

/// Returns path to the JSON state file: <config_dir>/usdview/app_state.json
fn state_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("usdview").join("app_state.json"))
}

/// Load state from disk. Returns default on missing file or parse error.
pub fn load_state() -> AppPersistState {
    usd_trace::trace_scope!("persistence_load_state");
    let Some(path) = state_path() else {
        return AppPersistState::default();
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return AppPersistState::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Save state to disk. Creates parent directories as needed; silently ignores errors.
///
/// Uses write-to-temp + rename pattern for crash safety — a partial write
/// won't corrupt the existing state file.
pub fn save_state(state: &AppPersistState) {
    let Some(path) = state_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string_pretty(state) else {
        return;
    };
    // Write to temp file first, then rename for crash safety.
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &json).is_ok() {
        // On Windows rename fails if target exists — remove first.
        let _ = std::fs::remove_file(&path);
        if std::fs::rename(&tmp, &path).is_err() {
            // Fallback: direct write + clean up temp.
            let _ = std::fs::write(&path, &json);
            let _ = std::fs::remove_file(&tmp);
        }
    } else {
        // Temp write failed — fall back to direct write.
        let _ = std::fs::write(&path, json);
    }
}

/// Delete the state file (used by --clearsettings).
pub fn delete_state() {
    if let Some(path) = state_path() {
        let _ = std::fs::remove_file(&path);
    }
}
