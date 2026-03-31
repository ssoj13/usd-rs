//! Viewport right-click context menu (matches usdviewq reference).

use crate::data_model::{DataModel, DrawMode as ViewDrawMode};
use crate::keyboard::AppAction;

/// Right-click context menu for the viewport, matching usdviewq reference.
pub(super) fn viewport_context_menu(
    response: &egui::Response,
    _ui: &mut egui::Ui,
    data_model: &mut DataModel,
    actions: &mut Vec<AppAction>,
) {
    response.context_menu(|ui| {
        // Dense spacing like menu bar
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.spacing_mut().button_padding.y = 1.0;

        if ui
            .add(egui::Button::new("Frame Selected").shortcut_text("F"))
            .clicked()
        {
            actions.push(AppAction::FrameSelected);
            ui.close();
        }

        if ui.button("Toggle Viewer Mode").clicked() {
            actions.push(AppAction::ToggleViewerMode);
            ui.close();
        }

        // Render Mode submenu (radio selection)
        ui.menu_button("Render Mode", |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.spacing_mut().button_padding.y = 1.0;
            let current = data_model.view.draw_mode;
            for &mode in ViewDrawMode::ALL {
                if ui.selectable_label(current == mode, mode.name()).clicked() {
                    data_model.view.draw_mode = mode;
                    ui.close();
                }
            }
        });

        ui.separator();

        // Select Bound Preview Material (per C++ primContextMenuItems.py)
        if ui.button("Select Bound Preview Material").clicked() {
            // Dispatch via SelectBoundPreviewMaterial through the prim tree action
            // system. We push a custom viewport action flag instead.
            actions.push(AppAction::SelectBoundPreviewMaterial);
            ui.close();
        }

        // Select Bound Full Material (per C++ primContextMenuItems.py)
        if ui.button("Select Bound Full Material").clicked() {
            actions.push(AppAction::SelectBoundFullMaterial);
            ui.close();
        }

        ui.separator();

        if ui
            .add(egui::Button::new("Make Invisible").shortcut_text("Ctrl+H"))
            .clicked()
        {
            actions.push(AppAction::MakeInvisible);
            ui.close();
        }

        if ui.button("Vis Only").clicked() {
            actions.push(AppAction::VisOnly);
            ui.close();
        }

        ui.separator();

        // Copy selected prim path to clipboard
        if ui.button("Copy Prim Path").clicked() {
            actions.push(AppAction::CopyPrimPath);
            ui.close();
        }

        ui.separator();

        // Activate / Deactivate selected prims
        if ui.button("Activate").clicked() {
            actions.push(AppAction::ActivateSelected);
            ui.close();
        }
        if ui.button("Deactivate").clicked() {
            actions.push(AppAction::DeactivateSelected);
            ui.close();
        }

        ui.separator();

        // Load / Unload payloads on selected prims
        if ui.button("Load").clicked() {
            actions.push(AppAction::LoadSelected);
            ui.close();
        }
        if ui.button("Unload").clicked() {
            actions.push(AppAction::UnloadSelected);
            ui.close();
        }

        ui.separator();

        if ui.button("Reset View").clicked() {
            actions.push(AppAction::ResetView);
            ui.close();
        }
    });
}
