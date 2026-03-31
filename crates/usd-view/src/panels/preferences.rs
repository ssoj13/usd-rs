//! Settings dialog — playa style.
//!
//! Left panel: category list. Right panel: settings for the selected category.
//! Opened via Edit > Preferences... or Ctrl+,

use egui::{Color32, ComboBox, DragValue, Slider, Ui};

// ---------------------------------------------------------------------------
// Category enum
// ---------------------------------------------------------------------------

/// Top-level preference categories shown in the left tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefCategory {
    #[default]
    General,
    Viewport,
    Camera,
    Lighting,
    Materials,
    Performance,
    Plugins,
}

impl PrefCategory {
    /// Display name shown in the category list.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Viewport => "Viewport",
            Self::Camera => "Camera",
            Self::Lighting => "Lighting",
            Self::Materials => "Materials",
            Self::Performance => "Performance",
            Self::Plugins => "Plugins",
        }
    }

    pub const ALL: &'static [PrefCategory] = &[
        Self::General,
        Self::Viewport,
        Self::Camera,
        Self::Lighting,
        Self::Materials,
        Self::Performance,
        Self::Plugins,
    ];

    /// Small icon shown before the category label in the sidebar.
    pub fn icon(self) -> &'static str {
        match self {
            Self::General => "G",
            Self::Viewport => "V",
            Self::Camera => "C",
            Self::Lighting => "L",
            Self::Materials => "M",
            Self::Performance => "P",
            Self::Plugins => "X",
        }
    }
}

/// Application UI theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    Light,
    #[default]
    Dark,
}

impl UiTheme {
    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

// ---------------------------------------------------------------------------
// Viewport sub-settings
// ---------------------------------------------------------------------------

/// Viewport background preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgPreset {
    Black,
    #[default]
    DarkGrey,
    Gradient,
}

impl BgPreset {
    pub fn name(self) -> &'static str {
        match self {
            Self::Black => "Black",
            Self::DarkGrey => "Dark Grey",
            Self::Gradient => "Gradient",
        }
    }
}

/// Default render mode for the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultRenderMode {
    #[default]
    SmoothShaded,
    FlatShaded,
    Wireframe,
    WireframeOnSurface,
    Points,
}

impl DefaultRenderMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::SmoothShaded => "Smooth Shaded",
            Self::FlatShaded => "Flat Shaded",
            Self::Wireframe => "Wireframe",
            Self::WireframeOnSurface => "Wireframe On Surface",
            Self::Points => "Points",
        }
    }
}

// ---------------------------------------------------------------------------
// Default light type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultLightType {
    #[default]
    DomeLight,
    DistantLight,
    SphereLight,
    None,
}

impl DefaultLightType {
    pub fn name(self) -> &'static str {
        match self {
            Self::DomeLight => "Dome Light",
            Self::DistantLight => "Distant Light",
            Self::SphereLight => "Sphere Light",
            Self::None => "None",
        }
    }
}

// ---------------------------------------------------------------------------
// PreferencesState — all settings + dialog open/category state
// ---------------------------------------------------------------------------

/// All preference settings, persisted across sessions.
#[derive(Debug, Clone)]
pub struct PreferencesSettings {
    // General
    pub ui_theme: UiTheme,
    pub font_size: f32,
    pub redraw_on_scrub: bool,
    pub step_size: f64,
    pub show_tooltips: bool,

    // Viewport
    pub default_render_mode: DefaultRenderMode,
    pub background: BgPreset,
    pub show_hud: bool,
    pub show_axes: bool,
    pub show_grid: bool,
    pub selection_highlight_color: Color32,

    // Camera
    pub default_fov: f32,
    pub near_clip: f64,
    pub far_clip: f64,
    pub tumble_speed: f32,
    pub zoom_speed: f32,

    // Lighting
    pub default_light_type: DefaultLightType,
    pub ambient_intensity: f32,
    pub key_light_intensity: f32,

    // Materials
    pub default_diffuse: Color32,
    pub default_specular: Color32,
    pub default_roughness: f32,

    // Performance
    pub tessellation_level: u32,
    pub gpu_instancing: bool,

    // Plugins
    pub renderer_plugin: String,
}

impl Default for PreferencesSettings {
    fn default() -> Self {
        Self {
            // General
            ui_theme: UiTheme::default(),
            font_size: 11.0,
            redraw_on_scrub: true,
            step_size: 1.0,
            show_tooltips: true,

            // Viewport
            default_render_mode: DefaultRenderMode::default(),
            background: BgPreset::default(),
            show_hud: true,
            show_axes: true,
            show_grid: true,
            selection_highlight_color: Color32::from_rgb(255, 220, 0),

            // Camera
            default_fov: 60.0,
            near_clip: 0.1,
            far_clip: 10000.0,
            tumble_speed: 1.0,
            zoom_speed: 1.0,

            // Lighting
            default_light_type: DefaultLightType::default(),
            ambient_intensity: 0.1,
            key_light_intensity: 1.0,

            // Materials
            default_diffuse: Color32::from_rgb(180, 180, 180),
            default_specular: Color32::from_rgb(255, 255, 255),
            default_roughness: 0.5,

            // Performance
            tessellation_level: 4,
            gpu_instancing: true,

            // Plugins
            renderer_plugin: "Storm (wgpu)".to_string(),
        }
    }
}

/// Dialog state: open flag + selected category.
#[derive(Debug, Default)]
pub struct PreferencesState {
    pub open: bool,
    pub category: PrefCategory,
    pub settings: PreferencesSettings,
}

impl PreferencesState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open dialog.
    pub fn open(&mut self) {
        self.open = true;
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Draw the Settings window. Call every frame from `update()`.
/// Playa-style: TreeView left, content right, X-button to close.
pub fn ui_preferences(ctx: &egui::Context, state: &mut PreferencesState) {
    if !state.open {
        return;
    }

    egui::Window::new("Settings")
        .id(egui::Id::new("settings_window"))
        .open(&mut state.open)
        .resizable(true)
        .default_size([700.0, 500.0])
        .min_size([500.0, 400.0])
        .collapsible(false)
        .show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Left panel: category list (fixed width)
                        ui.vertical(|ui| {
                            ui.set_width(180.0);
                            ui.add_space(4.0);

                            for &cat in PrefCategory::ALL {
                                let selected = state.category == cat;
                                let label = format!(" {}  {}", cat.icon(), cat.label());
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 24.0],
                                        egui::Button::new(label).selected(selected).frame(false),
                                    )
                                    .clicked()
                                {
                                    state.category = cat;
                                }
                            }

                            // Reset button at bottom of sidebar
                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(4.0);
                            if ui.button("Reset to Defaults").clicked() {
                                state.settings = PreferencesSettings::default();
                            }
                        });

                        ui.separator();

                        // Right panel: category title + settings
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.heading(state.category.label());
                            ui.separator();
                            ui.add_space(4.0);

                            draw_category(ui, state.category, &mut state.settings);
                        });
                    });
                });
        });
}

/// Dispatch to per-category settings panel.
fn draw_category(ui: &mut Ui, cat: PrefCategory, s: &mut PreferencesSettings) {
    match cat {
        PrefCategory::General => draw_general(ui, s),
        PrefCategory::Viewport => draw_viewport(ui, s),
        PrefCategory::Camera => draw_camera(ui, s),
        PrefCategory::Lighting => draw_lighting(ui, s),
        PrefCategory::Materials => draw_materials(ui, s),
        PrefCategory::Performance => draw_performance(ui, s),
        PrefCategory::Plugins => draw_plugins(ui, s),
    }
}

// --- helper: labeled row ------------------------------------------------

fn row(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        // Fixed-width label column for alignment
        ui.add_sized([160.0, 18.0], egui::Label::new(label));
        add_contents(ui);
    });
    ui.add_space(2.0);
}

// ---------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------

fn draw_general(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Theme", |ui| {
        ComboBox::from_id_salt("pref_ui_theme")
            .selected_text(s.ui_theme.name())
            .width(110.0)
            .show_ui(ui, |ui| {
                for theme in [UiTheme::Light, UiTheme::Dark] {
                    ui.selectable_value(&mut s.ui_theme, theme, theme.name());
                }
            });
    });

    row(ui, "Font size", |ui| {
        ui.add(
            Slider::new(&mut s.font_size, 8.0..=18.0)
                .step_by(1.0)
                .suffix("pt"),
        );
    });

    row(ui, "Redraw on scrub", |ui| {
        ui.checkbox(&mut s.redraw_on_scrub, "");
    });

    row(ui, "Step size", |ui| {
        ui.add(
            DragValue::new(&mut s.step_size)
                .speed(0.1)
                .range(0.001..=100.0),
        );
    });

    row(ui, "Show tooltips", |ui| {
        ui.checkbox(&mut s.show_tooltips, "");
    });
}

// ---------------------------------------------------------------------------
// Viewport
// ---------------------------------------------------------------------------

fn draw_viewport(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Default render mode", |ui| {
        ComboBox::from_id_salt("pref_render_mode")
            .selected_text(s.default_render_mode.name())
            .width(160.0)
            .show_ui(ui, |ui| {
                for mode in [
                    DefaultRenderMode::SmoothShaded,
                    DefaultRenderMode::FlatShaded,
                    DefaultRenderMode::Wireframe,
                    DefaultRenderMode::WireframeOnSurface,
                    DefaultRenderMode::Points,
                ] {
                    ui.selectable_value(&mut s.default_render_mode, mode, mode.name());
                }
            });
    });

    row(ui, "Background", |ui| {
        ComboBox::from_id_salt("pref_bg")
            .selected_text(s.background.name())
            .width(120.0)
            .show_ui(ui, |ui| {
                for preset in [BgPreset::DarkGrey, BgPreset::Black, BgPreset::Gradient] {
                    ui.selectable_value(&mut s.background, preset, preset.name());
                }
            });
    });

    row(ui, "Show HUD", |ui| {
        ui.checkbox(&mut s.show_hud, "");
    });

    row(ui, "Show axes", |ui| {
        ui.checkbox(&mut s.show_axes, "");
    });

    row(ui, "Show grid", |ui| {
        ui.checkbox(&mut s.show_grid, "");
    });

    row(ui, "Selection highlight", |ui| {
        ui.color_edit_button_srgba(&mut s.selection_highlight_color);
    });
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

fn draw_camera(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Default FOV", |ui| {
        ui.add(Slider::new(&mut s.default_fov, 10.0..=120.0).suffix("deg"));
    });

    row(ui, "Near clip", |ui| {
        ui.add(
            DragValue::new(&mut s.near_clip)
                .speed(0.01)
                .range(0.0001..=1000.0),
        );
    });

    row(ui, "Far clip", |ui| {
        ui.add(
            DragValue::new(&mut s.far_clip)
                .speed(10.0)
                .range(1.0..=1_000_000.0),
        );
    });

    row(ui, "Tumble speed", |ui| {
        ui.add(Slider::new(&mut s.tumble_speed, 0.1..=10.0).logarithmic(true));
    });

    row(ui, "Zoom speed", |ui| {
        ui.add(Slider::new(&mut s.zoom_speed, 0.1..=10.0).logarithmic(true));
    });
}

// ---------------------------------------------------------------------------
// Lighting
// ---------------------------------------------------------------------------

fn draw_lighting(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Default light type", |ui| {
        ComboBox::from_id_salt("pref_light_type")
            .selected_text(s.default_light_type.name())
            .width(130.0)
            .show_ui(ui, |ui| {
                for lt in [
                    DefaultLightType::DomeLight,
                    DefaultLightType::DistantLight,
                    DefaultLightType::SphereLight,
                    DefaultLightType::None,
                ] {
                    ui.selectable_value(&mut s.default_light_type, lt, lt.name());
                }
            });
    });

    row(ui, "Ambient intensity", |ui| {
        ui.add(Slider::new(&mut s.ambient_intensity, 0.0..=1.0).step_by(0.01));
    });

    row(ui, "Key light intensity", |ui| {
        ui.add(Slider::new(&mut s.key_light_intensity, 0.0..=2.0).step_by(0.01));
    });
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

fn draw_materials(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Default diffuse", |ui| {
        ui.color_edit_button_srgba(&mut s.default_diffuse);
    });

    row(ui, "Default specular", |ui| {
        ui.color_edit_button_srgba(&mut s.default_specular);
    });

    row(ui, "Default roughness", |ui| {
        ui.add(Slider::new(&mut s.default_roughness, 0.0..=1.0).step_by(0.01));
    });
}

// ---------------------------------------------------------------------------
// Performance
// ---------------------------------------------------------------------------

fn draw_performance(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Tessellation level", |ui| {
        ui.add(
            DragValue::new(&mut s.tessellation_level)
                .speed(1)
                .range(1..=16),
        );
    });

    row(ui, "GPU instancing", |ui| {
        ui.checkbox(&mut s.gpu_instancing, "");
    });
}

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

fn draw_plugins(ui: &mut Ui, s: &mut PreferencesSettings) {
    row(ui, "Renderer plugin", |ui| {
        ui.label(&s.renderer_plugin);
    });

    ui.add_space(8.0);
    ui.label("Active render delegates:");
    ui.separator();
    // List known plugins (read-only info)
    for name in &["Storm (wgpu)", "Embree (CPU)"] {
        ui.horizontal(|ui| {
            let active = *name == s.renderer_plugin.as_str();
            let icon = if active { ">>" } else { "  " };
            ui.label(icon);
            ui.label(*name);
        });
    }
}
