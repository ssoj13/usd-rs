//! Window menu.

use super::{MenuActions, compact_menu};

pub(super) fn window_menu(ui: &mut egui::Ui, result: &mut MenuActions) {
    ui.menu_button("Window", |ui| {
        compact_menu(ui);
        // Toggle viewer-only mode (hides all panels except viewport)
        if ui
            .add(egui::Button::new("Toggle Viewer-Only Mode").shortcut_text("F11"))
            .clicked()
        {
            result
                .actions
                .push(crate::keyboard::AppAction::ToggleViewerMode);
            ui.close();
        }

        ui.separator();

        // Open spline/animation curve viewer panel
        if ui.button("Spline Viewer").clicked() {
            result
                .actions
                .push(crate::keyboard::AppAction::OpenSplineViewer);
            ui.close();
        }

        ui.separator();

        // Reset dock layout to defaults
        if ui.button("Reset Layout").clicked() {
            result.reset_layout = true;
            ui.close();
        }

        ui.separator();

        if ui
            .add(egui::Button::new("Preferences...").shortcut_text("Ctrl+,"))
            .clicked()
        {
            result.open_preferences = true;
            ui.close();
        }
    });
}
