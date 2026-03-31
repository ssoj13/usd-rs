//! Edit menu.
//!
//! Reference order: Load/Unload, Activate/Deactivate, visibility ops.

use crate::keyboard::AppAction;

use super::{MenuActions, compact_menu};

pub(super) fn edit_menu(ui: &mut egui::Ui, result: &mut MenuActions) {
    ui.menu_button("Edit", |ui| {
        compact_menu(ui);

        // Load / Unload payloads
        if ui.button("Load").clicked() {
            result.actions.push(AppAction::LoadSelected);
            ui.close();
        }

        if ui.button("Unload").clicked() {
            result.actions.push(AppAction::UnloadSelected);
            ui.close();
        }

        ui.separator();

        // Activate / Deactivate prims
        if ui.button("Activate").clicked() {
            result.actions.push(AppAction::ActivateSelected);
            ui.close();
        }

        if ui.button("Deactivate").clicked() {
            result.actions.push(AppAction::DeactivateSelected);
            ui.close();
        }

        ui.separator();

        // Visibility operations
        if ui
            .add(egui::Button::new("Make Visible").shortcut_text("Shift+H"))
            .clicked()
        {
            result.actions.push(AppAction::MakeVisible);
            ui.close();
        }

        if ui.button("Vis Only").clicked() {
            result.actions.push(AppAction::VisOnly);
            ui.close();
        }

        if ui
            .add(egui::Button::new("Make Invisible").shortcut_text("Ctrl+H"))
            .clicked()
        {
            result.actions.push(AppAction::MakeInvisible);
            ui.close();
        }

        if ui.button("Remove Session Visibility").clicked() {
            result.actions.push(AppAction::RemoveSessionVis);
            ui.close();
        }

        if ui.button("Reset All Session Visibility").clicked() {
            result.actions.push(AppAction::ResetAllSessionVis);
            ui.close();
        }
    });
}
