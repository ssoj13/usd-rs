//! Navigation menu.

use crate::keyboard::AppAction;

use super::{MenuActions, MenuState, compact_menu};

pub(super) fn navigation_menu(ui: &mut egui::Ui, menu_state: &MenuState, result: &mut MenuActions) {
    ui.menu_button("Navigation", |ui| {
        compact_menu(ui);

        // Camera selection submenu (matches C++ Navigation > Cameras)
        ui.menu_button("Cameras", |ui| {
            // Free camera (always available)
            let is_free = menu_state.active_camera_path.is_none();
            if ui.selectable_label(is_free, "Free Camera").clicked() {
                result.actions.push(AppAction::SetCamera(None));
                ui.close();
            }
            if !menu_state.scene_cameras.is_empty() {
                ui.separator();
                for (cam_path, cam_name) in &menu_state.scene_cameras {
                    let selected =
                        menu_state.active_camera_path.as_deref() == Some(cam_path.as_str());
                    if ui.selectable_label(selected, cam_name).clicked() {
                        result
                            .actions
                            .push(AppAction::SetCamera(Some(cam_path.clone())));
                        ui.close();
                    }
                }
            }
        });

        ui.separator();

        // Find prims by name/path filter
        if ui
            .add(egui::Button::new("Find Prims").shortcut_text("Ctrl+F"))
            .clicked()
        {
            result.actions.push(AppAction::FindPrims);
            ui.close();
        }

        ui.separator();

        if ui.button("Select Stage Root").clicked() {
            result.actions.push(AppAction::SelectStageRoot);
            ui.close();
        }

        if ui.button("Select Enclosing Model").clicked() {
            result.actions.push(AppAction::SelectModelRoot);
            ui.close();
        }

        ui.separator();

        if ui.button("Select Bound Preview Material").clicked() {
            result.select_bound_preview_material = true;
            ui.close();
        }

        if ui.button("Select Bound Full Material").clicked() {
            result.select_bound_full_material = true;
            ui.close();
        }

        ui.separator();

        if ui.button("Select Preview Binding Relationship").clicked() {
            result.select_preview_binding_rel = true;
            ui.close();
        }

        if ui.button("Select Full Binding Relationship").clicked() {
            result.select_full_binding_rel = true;
            ui.close();
        }
    });
}
