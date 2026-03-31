//! Prim tree rendering: main UI entry point, toolbar, row drawing, context menu.

use std::collections::HashSet;

use egui::{Color32, FontId, Ui};
use usd_core::Prim;
use usd_sdf::Path;

use crate::data_model::DataModel;

use super::state::{PrimTreeAction, PrimTreeState};
use super::view_item::{PrimDrawMode, PrimViewItem, PrimVisibility};
use super::{
    ARROW_W, CLR_ANCESTOR_BG, CLR_HAS_ARCS, CLR_INSTANCE, CLR_NORMAL, CLR_PROTOTYPE, CLR_SELECTED,
    DRAWMODE_COL_W, FlatTreeRow, GUIDES_COL_W, INDENT_PX, ROW_HEIGHT, VIS_COL_W,
};

// ---------------------------------------------------------------------------
// Main UI entry point
// ---------------------------------------------------------------------------

/// Draws the enhanced prim tree panel with virtual scrolling.
///
/// Separates data collection (rebuild when dirty) from rendering (show_rows).
/// Only ~20-50 visible rows are rendered per frame instead of all 10K+.
pub fn ui_prim_tree_enhanced(
    ui: &mut Ui,
    data_model: &DataModel,
    state: &mut PrimTreeState,
    no_render: bool,
) {
    state.actions.clear();

    // Toolbar: search + filters
    draw_toolbar(ui, state);

    ui.separator();

    let Some(root) = data_model.root.pseudo_root() else {
        ui.label("No stage loaded.");
        return;
    };

    let tc = data_model.root.current_time;

    // Detect filter/search changes -> set dirty
    state.check_dirty();

    // Rebuild flattened row list only when dirty
    if state.tree_dirty {
        state.rebuild_flat_list(&root, tc);
    }

    let total_rows = state.flat_rows.len();
    if total_rows == 0 {
        ui.label("No prims match current filters.");
        return;
    }

    // Arrow key and F key navigation.
    // Must run before ScrollArea so scroll_to_row is set for this frame.
    handle_arrow_nav(ui, data_model, state);
    handle_f_key(ui, data_model, state);

    // Pre-compute ancestor set for selection highlighting (once per frame, not per row)
    let ancestor_set = build_ancestor_set(&data_model.selection.prims);

    // Grab scroll target before lending state to the closure.
    let scroll_target = state.scroll_to_row.take();

    // Virtual scrolling: only render rows in visible viewport range
    // Compact spacing for dense tree display
    ui.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, ROW_HEIGHT, total_rows, |ui, row_range| {
            // If keyboard nav requested a scroll, emit a scroll hint for the
            // target row when it enters (or is about to enter) the viewport.
            if let Some(target_idx) = scroll_target {
                let target_y = ui.min_rect().min.y + target_idx as f32 * ROW_HEIGHT;
                let target_rect = egui::Rect::from_min_size(
                    egui::pos2(0.0, target_y),
                    egui::vec2(1.0, ROW_HEIGHT),
                );
                ui.scroll_to_rect(target_rect, Some(egui::Align::Center));
            }

            for row_idx in row_range {
                let row = state.flat_rows[row_idx].clone();
                draw_flat_row(ui, &row, data_model, state, &ancestor_set, no_render);
            }
        });

    // Prim legend at the bottom (collapsible)
    draw_prim_legend(ui);
}

/// Search bar and filter toggles.
fn draw_toolbar(ui: &mut Ui, state: &mut PrimTreeState) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        let search_id = ui.make_persistent_id("prim_tree_search");
        let resp = ui.add(egui::TextEdit::singleline(&mut state.search.query).id(search_id));
        if state.request_search_focus {
            resp.request_focus();
            state.request_search_focus = false;
        }
        if resp.changed() {
            state.search.last_match_idx = 0;
        }
        ui.checkbox(&mut state.search.use_regex, "Regex");
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.filters.show_inactive, "Inactive");
        ui.checkbox(&mut state.filters.show_prototypes, "Prototypes");
        ui.checkbox(&mut state.filters.show_undefined, "Undefined");
        ui.checkbox(&mut state.filters.show_abstract, "Abstract");
        ui.separator();
        ui.checkbox(&mut state.filters.use_display_names, "Display Names");
        ui.separator();
        // P2-12: Column show/hide dropdown (ref: headerContextMenu.py)
        ui.menu_button("Columns", |ui| {
            ui.checkbox(&mut state.show_type_column, "Type");
            ui.checkbox(&mut state.show_vis_column, "Vis");
            ui.checkbox(&mut state.show_draw_mode_column, "Draw Mode");
            ui.checkbox(&mut state.show_guides_column, "Guides");
        });
    });
}

/// Render a single flat row in the virtual scroll viewport.
fn draw_flat_row(
    ui: &mut Ui,
    row: &FlatTreeRow,
    data_model: &DataModel,
    state: &mut PrimTreeState,
    ancestor_set: &HashSet<String>,
    no_render: bool,
) {
    let Some(item) = state.cache.get(&row.path_key).cloned() else {
        return;
    };

    let is_selected = data_model.selection.is_selected(&item.path);
    let is_ancestor = ancestor_set.contains(&row.path_key);

    // Allocate a fixed-height row
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );

    // P1-2: Ancestor highlight background
    if is_ancestor {
        ui.painter().rect_filled(rect, 0.0, CLR_ANCESTOR_BG);
    }

    // Selected highlight -- subtle dark blue-gold, like Maya/Houdini
    if is_selected {
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_rgb(62, 56, 38));
    }

    // Hover highlight
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.0,
            Color32::from_rgba_premultiplied(255, 255, 255, 15),
        );
    }

    let use_dn = state.filters.use_display_names;
    let indent = INDENT_PX * row.depth as f32 + 4.0;
    let text_y = rect.min.y + (ROW_HEIGHT - 14.0) / 2.0; // center text vertically

    let mut arrow_rect_opt: Option<egui::Rect> = None;

    // Draw expand/collapse arrow
    if row.has_children {
        let arrow_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + indent, rect.min.y),
            egui::vec2(ARROW_W, ROW_HEIGHT),
        );
        arrow_rect_opt = Some(arrow_rect);
        let arrow_center = arrow_rect.center();
        let painter = ui.painter();

        if row.is_expanded {
            // Down triangle (expanded)
            let pts = vec![
                egui::pos2(arrow_center.x - 4.0, arrow_center.y - 2.0),
                egui::pos2(arrow_center.x + 4.0, arrow_center.y - 2.0),
                egui::pos2(arrow_center.x, arrow_center.y + 4.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(180, 180, 180),
                egui::Stroke::NONE,
            ));
        } else {
            // Right triangle (collapsed)
            let pts = vec![
                egui::pos2(arrow_center.x - 2.0, arrow_center.y - 4.0),
                egui::pos2(arrow_center.x + 4.0, arrow_center.y),
                egui::pos2(arrow_center.x - 2.0, arrow_center.y + 4.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(140, 140, 140),
                egui::Stroke::NONE,
            ));
        }

        // Detect click on arrow area to toggle expand (Shift = recursive)
        let arrow_resp = ui.interact(
            arrow_rect,
            ui.id().with(("arrow", &row.path_key)),
            egui::Sense::click(),
        );
        if arrow_resp.clicked() {
            if ui.input(|i| i.modifiers.shift) {
                // Shift-click: recursive expand/collapse
                if let Some(stage) = &data_model.root.stage {
                    if let Some(prim) = stage.get_prim_at_path(&item.path) {
                        if row.is_expanded {
                            state.collapse_recursive(&prim);
                        } else {
                            state.expand_recursive(&prim);
                        }
                    }
                }
            } else {
                state.toggle_expanded(&row.path_key);
            }
        }
    }

    // Build label text
    let display_name = if use_dn {
        item.display_name.as_deref().unwrap_or(&item.name)
    } else {
        &item.name
    };
    let type_suffix = if !state.show_type_column || item.type_name.is_empty() {
        String::new()
    } else {
        format!("  [{}]", item.type_name)
    };
    let full_label = format!("{}{}", display_name, type_suffix);

    // P1-5: Unloaded prims are dimmed via text_color() which now checks is_loaded.
    let color = if is_selected {
        CLR_SELECTED
    } else {
        item.text_color()
    };

    let font = FontId::proportional(11.0);
    let text_x = rect.min.x
        + indent
        + if row.has_children {
            ARROW_W
        } else {
            ARROW_W * 0.5
        };

    // Draw label
    let galley = ui.painter().layout_no_wrap(full_label, font.clone(), color);
    ui.painter()
        .galley(egui::pos2(text_x, text_y), galley, color);

    // -----------------------------------------------------------------------
    // P1-1, P1-4, P1-7: Right-side fixed-width columns (right-to-left).
    // Track each column rect for P1-7 column-click discrimination.
    // Layout: [DRAWMODE] [GUIDES] [VIS]  <- right edge
    // -----------------------------------------------------------------------
    let mut right_edge = rect.max.x - 2.0;

    // --- DRAWMODE column (P1-1): interactive ComboBox for model prims ---
    let draw_mode_rect = if state.show_draw_mode_column && item.is_model {
        let col_rect = egui::Rect::from_min_size(
            egui::pos2(right_edge - DRAWMODE_COL_W, rect.min.y),
            egui::vec2(DRAWMODE_COL_W, ROW_HEIGHT),
        );
        right_edge -= DRAWMODE_COL_W + 2.0;

        let is_editing = state.draw_mode_edit_path.as_deref() == Some(row.path_key.as_str());

        if is_editing {
            // Render an inline ComboBox for draw mode selection.
            let mut current_mode = item.draw_mode;
            let combo_id = ui.id().with(("dm_combo", &row.path_key));

            // Use a child_ui clipped to the column rect so ComboBox renders correctly.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(col_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let mut mode_changed = false;
            egui::ComboBox::from_id_salt(combo_id)
                .width(DRAWMODE_COL_W - 4.0)
                .selected_text(current_mode.label())
                .show_ui(&mut child, |ui| {
                    for mode in PrimDrawMode::ALL {
                        if ui
                            .selectable_label(current_mode == *mode, mode.label())
                            .clicked()
                        {
                            current_mode = *mode;
                            mode_changed = true;
                        }
                    }
                });

            if mode_changed {
                state.actions.push(PrimTreeAction::SetDrawMode(
                    item.path.clone(),
                    current_mode.label().to_string(),
                ));
                state.draw_mode_edit_path = None;
                // Invalidate cache entry so it's reloaded next frame.
                state.cache.remove(&row.path_key);
                state.tree_dirty = true;
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.draw_mode_edit_path = None;
            }
        } else {
            // Static draw mode label. Authored overrides show brighter.
            let dm_color = if item.draw_mode_inherited {
                Color32::from_rgb(120, 120, 100)
            } else {
                Color32::from_rgb(200, 180, 100)
            };
            let dm_galley = ui.painter().layout_no_wrap(
                item.draw_mode.label().to_string(),
                FontId::proportional(10.0),
                dm_color,
            );
            ui.painter().galley(
                egui::pos2(col_rect.min.x + 2.0, text_y + 2.0),
                dm_galley,
                dm_color,
            );

            // Clear button ("x") for authored (non-inherited) draw mode overrides.
            if !item.draw_mode_inherited {
                let clear_rect = egui::Rect::from_min_size(
                    egui::pos2(col_rect.max.x - 12.0, rect.min.y + 2.0),
                    egui::vec2(10.0, ROW_HEIGHT - 4.0),
                );
                let clear_resp = ui.interact(
                    clear_rect,
                    ui.id().with(("dm_clear", &row.path_key)),
                    egui::Sense::click(),
                );
                let clr_color = if clear_resp.hovered() {
                    Color32::from_rgb(255, 100, 100)
                } else {
                    Color32::from_rgb(160, 80, 80)
                };
                ui.painter().text(
                    clear_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "x",
                    FontId::proportional(9.0),
                    clr_color,
                );
                if clear_resp.clicked() {
                    state
                        .actions
                        .push(PrimTreeAction::ClearDrawMode(item.path.clone()));
                    state.cache.remove(&row.path_key);
                    state.tree_dirty = true;
                }
            }
        }
        Some(col_rect)
    } else {
        None
    };

    // --- GUIDES column (P1-4): toggle guide visibility ---
    let guides_rect = if state.show_guides_column && item.supports_guides {
        let col_rect = egui::Rect::from_min_size(
            egui::pos2(right_edge - GUIDES_COL_W, rect.min.y),
            egui::vec2(GUIDES_COL_W, ROW_HEIGHT),
        );
        right_edge -= GUIDES_COL_W + 2.0;

        let (g_text, g_color) = match item.guide_visibility {
            PrimVisibility::Invisible => ("I", Color32::from_rgb(180, 120, 60)),
            _ => ("G", Color32::from_rgb(100, 180, 100)),
        };
        let g_galley =
            ui.painter()
                .layout_no_wrap(g_text.to_string(), FontId::proportional(10.0), g_color);
        let gx = col_rect.center().x - g_galley.size().x * 0.5;
        ui.painter()
            .galley(egui::pos2(gx, text_y + 2.0), g_galley, g_color);
        Some(col_rect)
    } else {
        None
    };

    // --- VIS column (P1-3): visibility indicator with inherited-invisible styling ---
    let vis_rect = if state.show_vis_column {
        let col_rect = egui::Rect::from_min_size(
            egui::pos2(right_edge - VIS_COL_W, rect.min.y),
            egui::vec2(VIS_COL_W, ROW_HEIGHT),
        );
        // right_edge not needed further

        // P1-3: Color and style differ for explicitly invisible vs inherited-invisible.
        let vis_color = match item.visibility {
            PrimVisibility::Invisible => Color32::from_rgb(200, 80, 80),
            PrimVisibility::InheritedInvisible => Color32::from_rgb(160, 60, 60),
            _ => Color32::from_rgb(80, 200, 80),
        };
        let vis_text = item.visibility.label();
        let vis_galley = ui.painter().layout_no_wrap(
            vis_text.to_string(),
            // Italic font for inherited-invisible (P1-3 visual distinction).
            FontId::proportional(11.0),
            vis_color,
        );
        let gx = col_rect.center().x - vis_galley.size().x * 0.5;
        ui.painter()
            .galley(egui::pos2(gx, text_y + 1.0), vis_galley, vis_color);
        Some(col_rect)
    } else {
        None
    };

    // -----------------------------------------------------------------------
    // P1-7: Column-based click routing.
    // VIS col -> ToggleVis, GUIDES col -> ToggleGuides,
    // DRAWMODE col -> open inline editor.
    // Name/Type area -> select prim.
    // -----------------------------------------------------------------------
    if response.clicked() {
        let click_pos = response.interact_pointer_pos();

        let in_vis = click_pos
            .zip(vis_rect)
            .map(|(p, r)| r.contains(p))
            .unwrap_or(false);
        let in_guides = click_pos
            .zip(guides_rect)
            .map(|(p, r)| r.contains(p))
            .unwrap_or(false);
        let in_drawmode = click_pos
            .zip(draw_mode_rect)
            .map(|(p, r)| r.contains(p))
            .unwrap_or(false);

        if in_vis {
            // VIS column: toggle visibility without selecting the prim (P1-7).
            state
                .actions
                .push(PrimTreeAction::ToggleVis(item.path.clone()));
        } else if in_guides {
            // GUIDES column: toggle guide vis without selecting the prim (P1-7).
            state
                .actions
                .push(PrimTreeAction::ToggleGuides(item.path.clone()));
        } else if in_drawmode && item.is_model {
            // DRAWMODE column: open inline ComboBox (P1-1, P1-7).
            state.draw_mode_edit_path = Some(row.path_key.clone());
        } else if ui.input(|i| i.modifiers.shift) && row.has_children {
            // Shift-click on name: expand/collapse (like arrow Shift-click)
            if let Some(stage) = &data_model.root.stage {
                if let Some(prim) = stage.get_prim_at_path(&item.path) {
                    if row.is_expanded {
                        state.collapse_recursive(&prim);
                    } else {
                        state.expand_recursive(&prim);
                    }
                }
            }
        } else {
            // Name / Type area: select prim.
            state
                .actions
                .push(PrimTreeAction::Select(item.path.clone()));
        }
    }

    // Double-click on row toggles expand/collapse (outside arrow hotspot).
    if response.double_clicked() && row.has_children {
        let pointer_pos = response.interact_pointer_pos();
        let clicked_arrow = match (pointer_pos, arrow_rect_opt) {
            (Some(pos), Some(arrow_rect)) => arrow_rect.contains(pos),
            _ => false,
        };
        if !clicked_arrow {
            if ui.input(|i| i.modifiers.shift) {
                // Shift-double-click: recursive expand/collapse.
                if let Some(stage) = &data_model.root.stage {
                    if let Some(prim) = stage.get_prim_at_path(&item.path) {
                        if row.is_expanded {
                            state.collapse_recursive(&prim);
                        } else {
                            state.expand_recursive(&prim);
                        }
                    }
                }
            } else {
                state.toggle_expanded(&row.path_key);
            }
        }
    }

    // Tooltip on hover (only when Rollover Prim Info is enabled in Show menu)
    if data_model.view.rollover_prim_info {
        response.clone().on_hover_text_at_pointer(item.tooltip());
    }

    // Context menu
    draw_context_menu_flat(&response, &item, state, data_model, no_render);
}

/// Dense spacing for context menus (matches menu_bar compact_menu).
fn compact_menu(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.spacing_mut().button_padding.y = 1.0;
}

/// Context menu matching C++ reference primContextMenuItems.py.
/// Resolves the Prim from stage only when the menu actually opens (rare).
fn draw_context_menu_flat(
    response: &egui::Response,
    item: &PrimViewItem,
    state: &mut PrimTreeState,
    data_model: &DataModel,
    no_render: bool,
) {
    response.context_menu(|ui| {
        compact_menu(ui);

        // Resolve prim lazily (only when context menu is open)
        let prim = data_model.root.prim_at_path(&item.path);

        // -- 1. Jump to Enclosing Model --
        if let Some(ref prim) = prim {
            if let Some(model) = get_enclosing_model(prim) {
                if ui
                    .button(format!("Jump to Enclosing Model ({})", model.name()))
                    .clicked()
                {
                    state
                        .actions
                        .push(PrimTreeAction::JumpToEnclosingModel(model.path().clone()));
                    ui.close();
                }
            }
        }

        // -- 2-3. Select Bound Material (preview / full) --
        if ui.button("Select Bound Preview Material").clicked() {
            state
                .actions
                .push(PrimTreeAction::SelectBoundPreviewMaterial(
                    item.path.clone(),
                ));
            ui.close();
        }
        if ui.button("Select Bound Full Material").clicked() {
            state
                .actions
                .push(PrimTreeAction::SelectBoundFullMaterial(item.path.clone()));
            ui.close();
        }

        ui.separator();

        // -- 4-8. Visibility controls --
        if ui
            .add(egui::Button::new("Make Visible").shortcut_text("Shift+H"))
            .clicked()
        {
            state
                .actions
                .push(PrimTreeAction::MakeVisible(item.path.clone()));
            ui.close();
        }
        if ui
            .add(egui::Button::new("Make Invisible").shortcut_text("Ctrl+H"))
            .clicked()
        {
            state
                .actions
                .push(PrimTreeAction::MakeInvisible(item.path.clone()));
            ui.close();
        }
        if ui.button("Vis Only").clicked() {
            state
                .actions
                .push(PrimTreeAction::VisOnly(item.path.clone()));
            ui.close();
        }
        if ui.button("Remove Session Visibility").clicked() {
            state
                .actions
                .push(PrimTreeAction::RemoveSessionVis(item.path.clone()));
            ui.close();
        }

        ui.separator();

        // -- 9-11. Load / Unload (only for prims with payloads) (P1-5) --
        if item.has_payload {
            if ui
                .add_enabled(!item.is_loaded, egui::Button::new("Load"))
                .clicked()
            {
                state
                    .actions
                    .push(PrimTreeAction::LoadPayload(item.path.clone()));
                ui.close();
            }
            if ui
                .add_enabled(!item.is_loaded, egui::Button::new("Load with Descendants"))
                .clicked()
            {
                state
                    .actions
                    .push(PrimTreeAction::LoadPayloadWithDescendants(
                        item.path.clone(),
                    ));
                ui.close();
            }
            if ui
                .add_enabled(item.is_loaded, egui::Button::new("Unload"))
                .clicked()
            {
                state
                    .actions
                    .push(PrimTreeAction::UnloadPayload(item.path.clone()));
                ui.close();
            }
            ui.separator();
        }

        // -- 12-13. Activate / Deactivate --
        if ui
            .add_enabled(!item.is_active, egui::Button::new("Activate"))
            .clicked()
        {
            state
                .actions
                .push(PrimTreeAction::Activate(item.path.clone()));
            ui.close();
        }
        if ui
            .add_enabled(item.is_active, egui::Button::new("Deactivate"))
            .clicked()
        {
            state
                .actions
                .push(PrimTreeAction::Deactivate(item.path.clone()));
            ui.close();
        }

        ui.separator();

        // -- 14-16. Copy operations --
        if ui.button("Copy Prim Path").clicked() {
            ui.ctx().copy_text(item.path.to_string());
            state
                .actions
                .push(PrimTreeAction::CopyPath(item.path.clone()));
            ui.close();
        }
        if ui.button("Copy Prim Name").clicked() {
            ui.ctx().copy_text(item.name.clone());
            state
                .actions
                .push(PrimTreeAction::CopyPrimName(item.name.clone()));
            ui.close();
        }
        if let Some(ref prim) = prim {
            if let Some(model) = get_enclosing_model(prim) {
                if ui.button("Copy Model Path").clicked() {
                    let mp = model.path().clone();
                    ui.ctx().copy_text(mp.to_string());
                    state.actions.push(PrimTreeAction::CopyModelPath(mp));
                    ui.close();
                }
            }
        }

        // -- Frame in viewport (only when rendering) (P1-6) --
        if !no_render {
            ui.separator();
            if ui
                .add(egui::Button::new("Frame in Viewport").shortcut_text("F"))
                .clicked()
            {
                state.actions.push(PrimTreeAction::Frame(item.path.clone()));
                ui.close();
            }
        }

        // -- Camera-specific --
        if item.is_camera {
            ui.separator();
            if ui.button("Set As Active Camera").clicked() {
                state
                    .actions
                    .push(PrimTreeAction::SetAsActiveCamera(item.path.clone()));
                ui.close();
            }
        }

        // -- RenderSettings-specific --
        if item.is_render_settings {
            ui.separator();
            if ui.button("Set As Active Render Settings").clicked() {
                state
                    .actions
                    .push(PrimTreeAction::SetAsActiveRenderSettings(item.path.clone()));
                ui.close();
            }
        }

        // -- RenderPass-specific --
        if item.is_render_pass {
            ui.separator();
            if ui.button("Set As Active Render Pass").clicked() {
                state
                    .actions
                    .push(PrimTreeAction::SetAsActiveRenderPass(item.path.clone()));
                ui.close();
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Prim legend: color key for tree display (matches C++ primLegend.py).
fn draw_prim_legend(ui: &mut Ui) {
    egui::CollapsingHeader::new(
        egui::RichText::new("Prim Legend")
            .small()
            .color(Color32::GRAY),
    )
    .default_open(false)
    .show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let entries: &[(&str, Color32)] = &[
            ("Normal", CLR_NORMAL),
            ("Has Arcs", CLR_HAS_ARCS),
            ("Instance", CLR_INSTANCE),
            ("Prototype", CLR_PROTOTYPE),
            ("Selected", CLR_SELECTED),
            ("Ancestor of Selected", CLR_ANCESTOR_BG),
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for (label, color) in entries {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, *color);
                    ui.label(egui::RichText::new(*label).color(*color).small());
                });
            }
        });
        // Font style legend (per primLegendUI.ui: abstract, undefined, defined)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Normal").small());
            ui.label(egui::RichText::new("= Abstract (class)").small());
            ui.label(egui::RichText::new("Italic").italics().small());
            ui.label(egui::RichText::new("= Undefined (over)").small());
            ui.label(egui::RichText::new("Bold").strong().small());
            ui.label(egui::RichText::new("= Defined (def)").small());
        });
        // Active/Inactive
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Dimmed colors")
                    .small()
                    .color(Color32::from_rgb(120, 120, 120)),
            );
            ui.label(egui::RichText::new("= Inactive / Unloaded prims").small());
        });
        // Column legend
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("VIS: V=visible, I=invisible (italic=inherited invisible)")
                    .small(),
            );
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("G=guides visible, I=guides invisible | draw mode label")
                    .small(),
            );
        });
    });
}

// ---------------------------------------------------------------------------
// P1-6: Arrow-key navigation (Maya outliner style) + F key framing
// ---------------------------------------------------------------------------

/// Handle Up/Down/Left/Right arrow key navigation for the prim tree.
///
/// - Up/Down: move selection to adjacent visible row.
/// - Right on collapsed node: expand it. Right on expanded node: select first child.
/// - Left on expanded node: collapse it. Left on collapsed node: select parent.
///
/// Skipped when Ctrl is held (reserved for other shortcuts) or when no prim
/// is currently selected. The panel must be receiving keyboard input — we check
/// by asking egui whether any text-edit widget has focus (if so, we yield).
fn handle_arrow_nav(ui: &mut Ui, data_model: &DataModel, state: &mut PrimTreeState) {
    // Don't steal keys while the search box (or any text field) has focus.
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }

    // Read all key events in one input closure to avoid repeated borrows.
    let (up, down, left, right) = ui.input(|i| {
        // Ignore if Ctrl is held — those combos belong to other shortcuts.
        if i.modifiers.ctrl {
            return (false, false, false, false);
        }
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
        )
    });

    if !up && !down && !left && !right {
        return;
    }

    // Need a current selection to navigate from.
    let Some(sel_path) = data_model.selection.prims.first() else {
        // No selection: Down/Right selects the very first visible row.
        if (down || right) && !state.flat_rows.is_empty() {
            if let Some(first) = Path::from_string(&state.flat_rows[0].path_key) {
                state.actions.push(PrimTreeAction::Select(first));
                state.scroll_to_row = Some(0);
            }
        }
        return;
    };

    let sel_key = sel_path.to_string();

    // Find the index of the currently selected row in the flat list.
    let Some(cur_idx) = state.flat_rows.iter().position(|r| r.path_key == sel_key) else {
        // Selected prim is not visible (filtered/collapsed). Just do nothing.
        return;
    };

    let total = state.flat_rows.len();

    if up {
        // Move to the previous visible row (clamp at 0).
        if cur_idx > 0 {
            let new_idx = cur_idx - 1;
            if let Some(p) = Path::from_string(&state.flat_rows[new_idx].path_key) {
                state.actions.push(PrimTreeAction::Select(p));
                state.scroll_to_row = Some(new_idx);
            }
        }
    } else if down {
        // Move to the next visible row (clamp at end).
        if cur_idx + 1 < total {
            let new_idx = cur_idx + 1;
            if let Some(p) = Path::from_string(&state.flat_rows[new_idx].path_key) {
                state.actions.push(PrimTreeAction::Select(p));
                state.scroll_to_row = Some(new_idx);
            }
        }
    } else if right {
        let row = &state.flat_rows[cur_idx];
        if row.has_children && !row.is_expanded {
            // Expand the current node.
            let key = row.path_key.clone();
            state.toggle_expanded(&key);
            // Stay on the same row after expand.
            state.scroll_to_row = Some(cur_idx);
        } else if row.has_children && row.is_expanded {
            // Already expanded: descend into first child (next row, one level deeper).
            let cur_depth = row.depth;
            // Collect to avoid borrow conflict with state.actions below.
            let child = state
                .flat_rows
                .iter()
                .enumerate()
                .skip(cur_idx + 1)
                .find(|(_, r)| r.depth == cur_depth + 1)
                .and_then(|(idx, r)| Path::from_string(&r.path_key).map(|p| (idx, p)));
            if let Some((child_idx, child_path)) = child {
                state.actions.push(PrimTreeAction::Select(child_path));
                state.scroll_to_row = Some(child_idx);
            }
        }
        // Leaf node: Right does nothing (matches Maya behavior).
    } else if left {
        let row = state.flat_rows[cur_idx].clone();
        if row.has_children && row.is_expanded {
            // Collapse the current node.
            state.toggle_expanded(&row.path_key);
            state.scroll_to_row = Some(cur_idx);
        } else {
            // Navigate to parent: strip last path component.
            if let Some(parent_key) = parent_path_str(&row.path_key) {
                let parent = state
                    .flat_rows
                    .iter()
                    .enumerate()
                    .find(|(_, r)| r.path_key == parent_key)
                    .and_then(|(idx, r)| Path::from_string(&r.path_key).map(|p| (idx, p)));
                if let Some((parent_idx, parent_path)) = parent {
                    state.actions.push(PrimTreeAction::Select(parent_path));
                    state.scroll_to_row = Some(parent_idx);
                }
            }
        }
    }
}

/// Handle F key: frame selected prim in viewport (P1-6).
/// Python reference: keyPressEvent -> KeyboardShortcuts.FramingKey -> FrameSelection().
/// We emit a Frame action when F is pressed and a prim is selected.
fn handle_f_key(ui: &mut Ui, data_model: &DataModel, state: &mut PrimTreeState) {
    // Don't steal F from search box or other text fields.
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }

    let f_pressed =
        ui.input(|i| !i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::F));

    if f_pressed {
        if let Some(path) = data_model.selection.prims.first() {
            state.actions.push(PrimTreeAction::Frame(path.clone()));
        }
    }
}

/// Strip the last component from a USD path string, returning the parent.
/// e.g. "/World/Foo/Bar" -> Some("/World/Foo"), "/World" -> Some("/"), "/" -> None.
fn parent_path_str(path: &str) -> Option<String> {
    if path == "/" || path.is_empty() {
        return None;
    }
    match path.rfind('/') {
        Some(0) => Some("/".to_string()), // parent is pseudo-root
        Some(pos) => Some(path[..pos].to_string()),
        None => None,
    }
}

/// Build a set of all ancestor path strings for the given selection.
/// Pre-computed once per frame for O(1) lookup per row (P1-2).
fn build_ancestor_set(selection: &[Path]) -> HashSet<String> {
    let mut set = HashSet::new();
    for sel_path in selection {
        let path_str = sel_path.to_string();
        // Walk up the path by splitting on '/'
        // e.g. "/World/Foo/Bar" -> ancestors: "/World/Foo", "/World"
        let parts: Vec<&str> = path_str.split('/').collect();
        for i in 1..parts.len() {
            let ancestor = parts[..i].join("/");
            if !ancestor.is_empty() && ancestor != path_str {
                set.insert(ancestor);
            }
        }
    }
    set
}

/// Gets the enclosing model prim.
fn get_enclosing_model(prim: &Prim) -> Option<Prim> {
    let mut current = prim.parent();
    while current.is_valid() {
        if current.is_model() {
            return Some(current);
        }
        current = current.parent();
    }
    None
}
