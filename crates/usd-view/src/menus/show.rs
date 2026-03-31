//! Show menu (prim tree filters, expand/collapse, columns).

use super::{MenuActions, MenuState, compact_menu};

pub(super) fn show_menu(ui: &mut egui::Ui, menu_state: &mut MenuState, result: &mut MenuActions) {
    ui.menu_button("Show", |ui| {
        compact_menu(ui);
        if ui.button("Expand All").clicked() {
            result.expand_all = true;
            ui.close();
        }

        if ui.button("Collapse All").clicked() {
            result.collapse_all = true;
            ui.close();
        }

        ui.menu_button("Prim View Depth", |ui| {
            for level in 1..=8 {
                if ui.button(format!("Level {}", level)).clicked() {
                    result.expand_to_depth = Some(level);
                    ui.close();
                }
            }
        });

        ui.separator();

        ui.checkbox(&mut menu_state.show_inactive_prims, "Show Inactive Prims");
        ui.checkbox(
            &mut menu_state.show_prototype_prims,
            "Show All Prototype Prims",
        );
        ui.checkbox(&mut menu_state.show_undefined_prims, "Show Undefined Prims");
        ui.checkbox(&mut menu_state.show_abstract_prims, "Show Abstract Prims");

        ui.separator();

        ui.checkbox(
            &mut menu_state.show_prim_display_names,
            "Show Prim Display Names",
        );
        ui.checkbox(&mut menu_state.rollover_prim_info, "Rollover Prim Info");

        ui.separator();

        ui.menu_button("Columns", |ui| {
            ui.checkbox(&mut menu_state.show_type_column, "Type");
            ui.checkbox(&mut menu_state.show_vis_column, "Visibility");
            ui.checkbox(&mut menu_state.show_guides_column, "Guides");
            ui.checkbox(&mut menu_state.show_draw_mode_column, "Draw Mode");
        });
    });
}
