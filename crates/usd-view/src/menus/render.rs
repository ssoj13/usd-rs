//! Render menu.

use crate::data_model::PickMode;
use crate::keyboard::AppAction;

use super::{MenuActions, MenuState, compact_menu};

pub(super) fn render_menu(ui: &mut egui::Ui, menu_state: &mut MenuState, result: &mut MenuActions) {
    ui.menu_button("Render", |ui| {
        compact_menu(ui);
        // Pick Mode submenu
        ui.menu_button("Pick Mode", |ui| {
            for mode in [
                PickMode::Prims,
                PickMode::Models,
                PickMode::Instances,
                PickMode::Prototypes,
            ] {
                if ui
                    .selectable_label(menu_state.pick_mode == mode, mode.name())
                    .clicked()
                {
                    menu_state.pick_mode = mode;
                    ui.close();
                }
            }
        });

        ui.separator();

        // HUD submenu
        ui.menu_button("HUD", |ui| {
            ui.checkbox(&mut menu_state.show_hud, "Show HUD");
            ui.checkbox(&mut menu_state.show_hud_info, "Info");
            ui.checkbox(&mut menu_state.show_hud_complexity, "Complexity");
            ui.checkbox(&mut menu_state.show_hud_performance, "Performance");
            ui.checkbox(&mut menu_state.show_hud_gpu_stats, "GPU Stats");
            ui.checkbox(&mut menu_state.show_hud_vbo_info, "VBO Info");
        });

        ui.separator();

        // Renderer plugin selection (queried from Engine)
        ui.menu_button("Renderer", |ui| {
            if menu_state.renderer_plugins.is_empty() {
                ui.label("(no renderers)");
            } else {
                for (plugin_id, display_name) in &menu_state.renderer_plugins {
                    let selected = menu_state.current_renderer == *plugin_id;
                    if ui.selectable_label(selected, display_name).clicked() {
                        result
                            .actions
                            .push(AppAction::SetRenderer(plugin_id.clone()));
                        ui.close();
                    }
                }
            }
        });

        // AOV selection (queried from Engine)
        ui.menu_button("AOV", |ui| {
            if menu_state.renderer_aovs.is_empty() {
                ui.label("(no AOVs)");
            } else {
                for aov_name in &menu_state.renderer_aovs {
                    let selected = menu_state.current_aov == *aov_name;
                    if ui.selectable_label(selected, aov_name).clicked() {
                        result.actions.push(AppAction::SetAOV(aov_name.clone()));
                        ui.close();
                    }
                }
            }
        });

        // Renderer settings submenu (matches Python menuRendererSettings)
        ui.menu_button("Renderer Settings", |ui| {
            ui.label("(no settings)");
        });

        // Renderer commands — populated by render delegate plugin system
        ui.menu_button("Renderer Commands", |ui| {
            ui.label("(none available)");
        });

        ui.separator();

        // Pause / Stop render
        let pause_label = if menu_state.render_paused {
            "Resume Renderer"
        } else {
            "Pause Renderer"
        };
        if ui
            .add(egui::Button::new(pause_label).shortcut_text("Ctrl+P"))
            .clicked()
        {
            result.actions.push(AppAction::PauseRender);
            ui.close();
        }

        if ui
            .add(egui::Button::new("Stop Renderer").shortcut_text("Ctrl+\\"))
            .clicked()
        {
            result.actions.push(AppAction::StopRender);
            ui.close();
        }
    });
}
