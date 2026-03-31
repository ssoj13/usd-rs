//! Debug menu — logging toggles and Hydra scene debugger placeholder.

use super::{MenuActions, MenuState, compact_menu};

pub(super) fn debug_menu(ui: &mut egui::Ui, menu_state: &mut MenuState, result: &mut MenuActions) {
    ui.menu_button("Debug", |ui| {
        compact_menu(ui);

        // Toggle debug logging level
        let label = if menu_state.debug_logging {
            "Disable Debug Logging"
        } else {
            "Enable Debug Logging"
        };
        if ui.button(label).clicked() {
            menu_state.debug_logging = !menu_state.debug_logging;
            result.toggle_debug_logging = true;
            ui.close();
        }

        // Toggle render stats overlay (shown in HUD GPU Stats section)
        ui.checkbox(
            &mut menu_state.show_render_stats_overlay,
            "Show Render Stats Overlay",
        );

        ui.separator();

        // Hydra Scene Debugger placeholder
        ui.add_enabled(false, egui::Button::new("Hydra Scene Debugger..."));

        // USD Validation panel
        if ui.button("USD Validation...").clicked() {
            result.open_validation = true;
            ui.close();
        }

        // TF_DEBUG flags dialog
        if ui.button("TF_DEBUG Flags...").clicked() {
            result.open_debug_flags = true;
            ui.close();
        }
    });
}
