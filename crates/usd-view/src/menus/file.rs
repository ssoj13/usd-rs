//! File menu.

use crate::keyboard::AppAction;
use crate::recent_files::RecentFiles;

use super::{MenuActions, compact_menu};

pub(super) fn file_menu(ui: &mut egui::Ui, recent_files: &RecentFiles, result: &mut MenuActions) {
    ui.menu_button("File", |ui| {
        compact_menu(ui);
        if ui
            .add(egui::Button::new("Open...").shortcut_text("Ctrl+O"))
            .clicked()
        {
            result.actions.push(AppAction::OpenFile);
            ui.close();
        }

        if ui.button("Save Overrides As...").clicked() {
            result.save_overrides = true;
            ui.close();
        }

        if ui.button("Save Flattened As...").clicked() {
            result.save_flattened = true;
            ui.close();
        }

        ui.separator();

        // Reload / reopen stage
        if ui.button("Reload All Layers").clicked() {
            result.actions.push(AppAction::ReloadAllLayers);
            ui.close();
        }

        if ui.button("Reopen Stage").clicked() {
            result.reopen_stage = true;
            ui.close();
        }

        ui.separator();

        if ui.button("Save Viewer Image...").clicked() {
            result.save_image = true;
            ui.close();
        }

        if ui.button("Copy Viewer Image").clicked() {
            result.copy_image = true;
            ui.close();
        }

        ui.separator();

        // Recent files submenu
        ui.menu_button("Recent Files", |ui| {
            if recent_files.is_empty() {
                ui.label("(none)");
            } else {
                for path in recent_files.list() {
                    let display = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    let tooltip = path.display().to_string();
                    if ui.button(display).on_hover_text(&tooltip).clicked() {
                        result.open_recent = Some(path.clone());
                        ui.close();
                    }
                }
            }
        });

        ui.separator();

        if ui
            .add(egui::Button::new("Quit").shortcut_text("Ctrl+Q"))
            .clicked()
        {
            result.actions.push(AppAction::Quit);
            ui.close();
        }
    });
}
