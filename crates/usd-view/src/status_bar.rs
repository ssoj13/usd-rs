//! Status bar panel (bottom of window).
//!
//! Shows: selected prim path | current frame | FPS/timing info.

use crate::data_model::DataModel;
use crate::playback::PlaybackState;

/// Runtime stats for status bar display.
#[derive(Debug, Clone, Default)]
pub struct StatusBarInfo {
    /// Measured FPS (frames per second).
    pub fps: f64,
    /// Render time in milliseconds.
    pub render_ms: f64,
    /// Current presentation surface format (if known).
    pub display_format: Option<String>,
    /// Whether the current presentation surface is HDR-capable.
    pub hdr_present: bool,
}

/// Draws the bottom status bar.
pub fn draw_status_bar(
    ui: &mut egui::Ui,
    data_model: &DataModel,
    playback: &PlaybackState,
    info: &StatusBarInfo,
) {
    ui.horizontal(|ui| {
        // Left: selected prim path + type + variants
        let prim_path = data_model
            .selection
            .focus_path()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "<no selection>".to_string());

        // Resolve type name and variant selections for focused prim
        let (type_str, variant_str) = if let Some(prim) = data_model.first_selected_prim() {
            let tn = prim.type_name();
            let type_name = if tn.is_empty() {
                "Prim".to_string()
            } else {
                tn.to_string()
            };

            // Collect active variant selections
            let vsets = prim.get_variant_sets();
            let names = vsets.get_names();
            let vars: Vec<String> = names
                .iter()
                .filter_map(|name| {
                    let sel = vsets.get_variant_selection(name);
                    if sel.is_empty() {
                        None
                    } else {
                        Some(format!("{}={}", name, sel))
                    }
                })
                .collect();
            let vs = if vars.is_empty() {
                String::new()
            } else {
                format!(" | Variants: {{{}}}", vars.join(", "))
            };
            (type_name, vs)
        } else {
            (String::new(), String::new())
        };

        // Build status label: "Path: /foo | Type: Mesh | Variants: {x=y}"
        if type_str.is_empty() {
            ui.label(&prim_path);
        } else {
            ui.label(format!(
                "Path: {} | Type: {}{}",
                prim_path, type_str, variant_str
            ));
        }

        ui.separator();

        // Center: frame info
        let frame = playback.current_frame();
        let (start, end) = playback.frame_range();
        let play_icon = if playback.is_playing() {
            "Playing"
        } else {
            "Stopped"
        };
        ui.label(format!(
            "Frame: {:.1} [{:.0}..{:.0}] {}",
            frame, start, end, play_icon
        ));

        // Right-aligned: timing
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{:.1} FPS | {:.1}ms", info.fps, info.render_ms));
            if let Some(format) = &info.display_format {
                let label = if info.hdr_present { "HDR" } else { "SDR" };
                ui.label(format!("[{} {}]", label, format));
            }
            if playback.is_looping() {
                ui.label("[Loop]");
            }
        });
    });
}
