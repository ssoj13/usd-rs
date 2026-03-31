//! Prim tree panel.
//!
//! Scene hierarchy with selection, context menu, and variant set UI.
//! Reference: primTreeWidget.py, primContextMenuItems.py, variantComboBox.py

use egui::{Color32, Ui};
use usd_core::Prim;
use usd_sdf::Path;

use crate::data_model::DataModel;

/// Actions that the prim tree can request (e.g. from context menu).
#[derive(Debug, Clone)]
pub enum PrimTreeAction {
    Select(Path),
    CopyPath(Path),
    CopyModelPath(Path),
    Frame(Path),
    ToggleVisibility(Path),
    VisOnly(Path),
    LoadPayload(Path),
    UnloadPayload(Path),
    Activate(Path),
    Deactivate(Path),
    SetAsActiveCamera(Path),
}

/// Colors matching usdview prim tree (UIPrimTreeColors in common.py).
fn selected_color() -> Color32 {
    Color32::from_rgb(189, 155, 84)
}
fn ancestor_of_selected_color() -> Color32 {
    // C++: QColor(189, 155, 84, 50) — same hue but ~20% alpha for dimmed ancestor highlight
    Color32::from_rgba_unmultiplied(189, 155, 84, 50)
}
fn default_color() -> Color32 {
    Color32::from_rgb(227, 227, 227)
}

/// Returns true if path is an ancestor of any selected path.
fn is_ancestor_of_selected(path: &Path, selection: &[Path]) -> bool {
    selection
        .iter()
        .any(|sel| sel.has_prefix(path) && sel != path)
}

/// Draws a prim node and its children using an explicit depth guard to prevent
/// stack overflow on pathologically deep scenes (>512 levels).
fn draw_prim_node(
    ui: &mut Ui,
    prim: &Prim,
    data_model: &DataModel,
    on_select: &mut impl FnMut(Path),
    actions: &mut Vec<PrimTreeAction>,
    no_render: bool,
) {
    draw_prim_node_depth(ui, prim, data_model, on_select, actions, no_render, 0);
}

/// Inner implementation with depth tracking.
fn draw_prim_node_depth(
    ui: &mut Ui,
    prim: &Prim,
    data_model: &DataModel,
    on_select: &mut impl FnMut(Path),
    actions: &mut Vec<PrimTreeAction>,
    no_render: bool,
    depth: usize,
) {
    if !prim.is_valid() {
        return;
    }
    // Guard against stack overflow from degenerate scene graphs.
    if depth > 512 {
        ui.label("... (too deep)");
        return;
    }

    let path = prim.path().clone();
    let name = prim.name().to_string();
    let type_name = prim.type_name().to_string();
    // Use get_all_children to show overs/classes too (not just 'def' prims)
    let children = prim.get_all_children();

    let is_selected = data_model.selection.is_selected(&path);
    let is_ancestor = is_ancestor_of_selected(&path, &data_model.selection.prims);

    let label = format!("{}  [{}]", name, type_name);
    let text_color = if is_selected {
        selected_color()
    } else if is_ancestor {
        ancestor_of_selected_color()
    } else {
        default_color()
    };

    let response = egui::CollapsingHeader::new(egui::RichText::new(label).color(text_color))
        .default_open(children.len() < 20)
        .show(ui, |ui| {
            // Draw mode per prim for models (reference: DrawModeWidget)
            if prim.is_model() {
                if let Some(ref stage) = data_model.root.stage {
                    // Use new() directly — is_model() already confirmed schema applicability,
                    // and apply() has side effects (registers API) inappropriate for read-only UI
                    let api = usd_geom::model_api::ModelAPI::new(prim.clone());
                    {
                        let current = api.compute_model_draw_mode(None);
                        let current_str = current.get_text().to_string();
                        let modes = ["default", "origin", "bounds", "cards", "inherited"];
                        ui.horizontal(|ui| {
                            ui.label("Draw:");
                            egui::ComboBox::from_id_salt(
                                ui.make_persistent_id(path.to_string() + ":drawMode"),
                            )
                            .selected_text(&current_str)
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                for mode in modes {
                                    if ui.selectable_label(current_str == mode, mode).clicked() {
                                        if let (Some(session), Some(attr)) = (
                                            stage.get_session_layer(),
                                            api.create_model_draw_mode_attr(Some(
                                                usd_tf::Token::new(mode),
                                            ))
                                            .or_else(|| api.get_model_draw_mode_attr()),
                                        ) {
                                            let prev = stage.get_edit_target();
                                            stage.set_edit_target(
                                                usd_core::EditTarget::for_local_layer(session),
                                            );
                                            let _ = attr.set(
                                                usd_vt::Value::from(usd_tf::Token::new(mode)),
                                                usd_sdf::TimeCode::default(),
                                            );
                                            stage.set_edit_target(prev);
                                        }
                                        ui.close();
                                    }
                                }
                            });
                        });
                    }
                }
            }

            // Variant sets inline (reference: variantComboBox.py)
            let variant_sets = prim.get_variant_sets();
            let names = variant_sets.get_names();
            for vs_name in names {
                let vs = variant_sets.get_variant_set(vs_name.as_str());
                let variants = vs.get_variant_names();
                if !variants.is_empty() {
                    let sel = vs.get_variant_selection();
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", vs_name));
                        let current = sel;
                        egui::ComboBox::from_id_salt(
                            ui.make_persistent_id(path.to_string() + vs_name.as_str()),
                        )
                        .selected_text(&current)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for v in &variants {
                                if ui.selectable_label(&current == v, v).clicked() {
                                    let _ = vs.set_variant_selection(v);
                                    ui.close();
                                }
                            }
                        });
                    });
                }
            }
            for child in children {
                draw_prim_node_depth(
                    ui,
                    &child,
                    data_model,
                    on_select,
                    actions,
                    no_render,
                    depth + 1,
                );
            }
        });

    if response.header_response.clicked() {
        on_select(path.clone());
    }

    response.header_response.context_menu(|ui| {
        if ui.button("Select").clicked() {
            actions.push(PrimTreeAction::Select(path.clone()));
            ui.close();
        }
        if ui.button("Copy path").clicked() {
            actions.push(PrimTreeAction::CopyPath(path.clone()));
            ui.close();
        }
        if !no_render && ui.button("Frame in view").clicked() {
            actions.push(PrimTreeAction::Frame(path.clone()));
            ui.close();
        }
        ui.separator();
        let imageable = usd_geom::imageable::Imageable::new(prim.clone());
        if imageable.is_valid() {
            let tc = data_model.root.current_time;
            let vis = imageable.compute_visibility(tc);
            let is_visible = vis == usd_geom::tokens::usd_geom_tokens().inherited;
            if ui
                .button(if is_visible {
                    "Make Invisible"
                } else {
                    "Make Visible"
                })
                .clicked()
            {
                actions.push(PrimTreeAction::ToggleVisibility(path.clone()));
                ui.close();
            }
        }
        // Vis Only (isolate visibility)
        if ui.button("Vis Only").clicked() {
            actions.push(PrimTreeAction::VisOnly(path.clone()));
            ui.close();
        }
        ui.separator();
        // Activate / Deactivate
        if prim.is_active() {
            if ui.button("Deactivate").clicked() {
                actions.push(PrimTreeAction::Deactivate(path.clone()));
                ui.close();
            }
        } else {
            if ui.button("Activate").clicked() {
                actions.push(PrimTreeAction::Activate(path.clone()));
                ui.close();
            }
        }
        if prim.has_payload() {
            let is_loaded = prim.is_loaded();
            if ui
                .button(if is_loaded { "Unload" } else { "Load" })
                .clicked()
            {
                if is_loaded {
                    actions.push(PrimTreeAction::UnloadPayload(path.clone()));
                } else {
                    actions.push(PrimTreeAction::LoadPayload(path.clone()));
                }
                ui.close();
            }
        }
        if let Some(model) = get_enclosing_model(prim) {
            ui.separator();
            if ui
                .button(format!("Copy Model Path ({})", model.name()))
                .clicked()
            {
                actions.push(PrimTreeAction::CopyModelPath(model.path().clone()));
                ui.close();
            }
        }
        if is_camera_prim(prim) {
            ui.separator();
            if ui.button("Set As Active Camera").clicked() {
                actions.push(PrimTreeAction::SetAsActiveCamera(path.clone()));
                ui.close();
            }
        }
    });
}

/// Gets the enclosing model prim for hierarchy display (reference: GetEnclosingModelPrim).
fn get_enclosing_model(prim: &Prim) -> Option<Prim> {
    let mut current = prim.clone();
    while current.is_valid() {
        if current.is_model() {
            return Some(current);
        }
        current = current.parent();
    }
    None
}

fn is_camera_prim(prim: &Prim) -> bool {
    prim.get_type_name().to_string() == "Camera"
}

/// Draws the full prim tree.
pub fn ui_prim_tree(
    ui: &mut Ui,
    data_model: &DataModel,
    on_select: &mut impl FnMut(Path),
    actions: &mut Vec<PrimTreeAction>,
    no_render: bool,
) {
    let Some(root) = data_model.root.pseudo_root() else {
        ui.label("No stage loaded.");
        return;
    };

    actions.clear();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for child in root.get_all_children() {
            draw_prim_node(ui, &child, data_model, on_select, actions, no_render);
        }
    });

    // --- Prim legend (P1-13) ---
    // Collapsible color key explaining prim tree highlight colors.
    draw_prim_legend(ui);
}

/// Prim tree color legend (matches C++ UIPrimTreeColors).
fn draw_prim_legend(ui: &mut Ui) {
    egui::CollapsingHeader::new(egui::RichText::new("Legend").small().color(Color32::GRAY))
        .default_open(false)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let entries: &[(&str, Color32)] = &[
                ("Selected", selected_color()),
                ("Ancestor of selected", ancestor_of_selected_color()),
                ("Default", default_color()),
                ("Inactive/abstract", Color32::from_rgb(120, 120, 120)),
                ("Undefined (over)", Color32::from_rgb(100, 100, 100)),
                ("Has payload", Color32::from_rgb(150, 190, 230)),
            ];
            for (label, color) in entries {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, *color);
                    ui.label(egui::RichText::new(*label).small());
                });
            }
        });
}
