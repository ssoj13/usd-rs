//! Composition arc viewer panel.
//!
//! Shows all composition arcs for the selected prim:
//! Reference, Payload, Inherit, Specialize, VariantSet.
//! Per-arc: type icon, target layer identifier, target path.
//! Clickable navigation to target prims.

use egui::{Color32, RichText, Ui};
use usd_core::Prim;
use usd_sdf::Path;

use crate::data_model::DataModel;

// ---------------------------------------------------------------------------
// Arc type classification
// ---------------------------------------------------------------------------

/// Composition arc type matching PcpArcType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcType {
    Reference,
    Payload,
    Inherit,
    Specialize,
    VariantSet,
}

impl ArcType {
    /// Short label for display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Reference => "Ref",
            Self::Payload => "Pay",
            Self::Inherit => "Inh",
            Self::Specialize => "Spc",
            Self::VariantSet => "Var",
        }
    }

    /// Full name for tooltip.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Reference => "Reference",
            Self::Payload => "Payload",
            Self::Inherit => "Inherit",
            Self::Specialize => "Specialize",
            Self::VariantSet => "VariantSet",
        }
    }

    /// Color for the arc type badge.
    pub fn color(&self) -> Color32 {
        match self {
            Self::Reference => Color32::from_rgb(135, 206, 250), // light blue
            Self::Payload => Color32::from_rgb(177, 207, 153),   // green
            Self::Inherit => Color32::from_rgb(222, 158, 46),    // gold
            Self::Specialize => Color32::from_rgb(230, 150, 230), // purple
            Self::VariantSet => Color32::from_rgb(200, 200, 200), // gray
        }
    }
}

// ---------------------------------------------------------------------------
// Arc entry
// ---------------------------------------------------------------------------

/// A single composition arc entry for display.
#[derive(Debug, Clone)]
pub struct ArcEntry {
    pub arc_type: ArcType,
    /// Layer identifier (e.g. file path or anonymous layer id).
    pub layer_id: String,
    /// Target prim path within the layer.
    pub target_path: String,
    /// Introduction path (where the arc is authored).
    pub intro_path: String,
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Actions from the composition panel.
#[derive(Debug, Clone)]
pub enum CompositionAction {
    /// Navigate to the target prim path.
    SelectPath(Path),
    /// Copy text to clipboard.
    CopyText(String),
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Persistent state for the composition panel.
#[derive(Debug, Default)]
pub struct CompositionPanelState {
    pub actions: Vec<CompositionAction>,
}

impl CompositionPanelState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Arc collection from prim
// ---------------------------------------------------------------------------

/// Collect all composition arcs for a prim.
fn collect_arcs(prim: &Prim) -> Vec<ArcEntry> {
    let mut arcs = Vec::new();

    // References — from the prim stack spec layers
    // We inspect the prim stack for contributing specs and their layers
    let prim_stack = prim.get_prim_stack();
    for spec in &prim_stack {
        let layer = match spec.layer().upgrade() {
            Some(l) => l,
            None => continue,
        };
        let layer_id = layer.identifier().to_string();

        let spec_path_str = spec.path().to_string();

        // References via layer-level API (returns Reference with asset_path/prim_path)
        if let Some(ref_list_op) = layer.get_reference_list_op(&spec.path()) {
            for r in ref_list_op
                .get_prepended_items()
                .iter()
                .chain(ref_list_op.get_appended_items().iter())
            {
                arcs.push(ArcEntry {
                    arc_type: ArcType::Reference,
                    layer_id: if r.asset_path().is_empty() {
                        layer_id.clone()
                    } else {
                        r.asset_path().to_string()
                    },
                    target_path: r.prim_path().to_string(),
                    intro_path: spec_path_str.clone(),
                });
            }
        }

        // Payloads via layer-level API
        if let Some(pay_list_op) = layer.get_payload_list_op(&spec.path()) {
            for p in pay_list_op
                .get_prepended_items()
                .iter()
                .chain(pay_list_op.get_appended_items().iter())
            {
                arcs.push(ArcEntry {
                    arc_type: ArcType::Payload,
                    layer_id: if p.asset_path().is_empty() {
                        layer_id.clone()
                    } else {
                        p.asset_path().to_string()
                    },
                    target_path: p.prim_path().to_string(),
                    intro_path: spec_path_str.clone(),
                });
            }
        }

        // Inherits via PrimSpec path list
        let inherits = spec.inherits_list();
        for inh_path in inherits
            .get_prepended_items()
            .iter()
            .chain(inherits.get_appended_items().iter())
        {
            arcs.push(ArcEntry {
                arc_type: ArcType::Inherit,
                layer_id: layer_id.clone(),
                target_path: inh_path.to_string(),
                intro_path: spec_path_str.clone(),
            });
        }

        // Specializes via PrimSpec path list
        let specializes = spec.specializes_list();
        for spc_path in specializes
            .get_prepended_items()
            .iter()
            .chain(specializes.get_appended_items().iter())
        {
            arcs.push(ArcEntry {
                arc_type: ArcType::Specialize,
                layer_id: layer_id.clone(),
                target_path: spc_path.to_string(),
                intro_path: spec_path_str.clone(),
            });
        }
    }

    // Variant sets (from the prim itself, not specs)
    let variant_sets = prim.get_variant_sets();
    let vs_names = variant_sets.get_names();
    for vs_name in &vs_names {
        let vs = variant_sets.get_variant_set(vs_name.as_str());
        let sel = vs.get_variant_selection();
        arcs.push(ArcEntry {
            arc_type: ArcType::VariantSet,
            layer_id: format!("{}={}", vs_name, sel),
            target_path: format!(
                "{{{}={}}}",
                vs_name,
                if sel.is_empty() { "<none>" } else { &sel }
            ),
            intro_path: prim.path().to_string(),
        });
    }

    arcs
}

// ---------------------------------------------------------------------------
// Main UI
// ---------------------------------------------------------------------------

/// Draws the composition arc viewer for the selected prim.
pub fn ui_composition(ui: &mut Ui, data_model: &DataModel, state: &mut CompositionPanelState) {
    state.actions.clear();

    let Some(prim) = data_model.first_selected_prim() else {
        ui.label("Select a prim to view composition arcs.");
        return;
    };

    ui.heading(format!("Composition: {}", prim.path()));
    ui.separator();

    let arcs = collect_arcs(&prim);

    if arcs.is_empty() {
        ui.label("No composition arcs.");
        return;
    }

    // Group by arc type for clarity
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Legend
            ui.horizontal(|ui| {
                for arc_type in [
                    ArcType::Reference,
                    ArcType::Payload,
                    ArcType::Inherit,
                    ArcType::Specialize,
                    ArcType::VariantSet,
                ] {
                    ui.label(
                        RichText::new(arc_type.label())
                            .color(arc_type.color())
                            .small()
                            .strong(),
                    );
                }
            });
            ui.separator();

            // All arcs in order
            for arc in &arcs {
                draw_arc_row(ui, arc, state);
            }
        });
}

/// Draw a single arc entry row.
fn draw_arc_row(ui: &mut Ui, arc: &ArcEntry, state: &mut CompositionPanelState) {
    let resp = ui.horizontal(|ui| {
        // Type badge
        ui.label(
            RichText::new(arc.arc_type.label())
                .color(arc.arc_type.color())
                .strong()
                .small(),
        )
        .on_hover_text(arc.arc_type.name());

        // Layer identifier
        ui.label(
            RichText::new(&arc.layer_id)
                .color(Color32::from_rgb(180, 180, 180))
                .small(),
        );

        // Target path (clickable for navigation)
        if !arc.target_path.is_empty() && arc.arc_type != ArcType::VariantSet {
            if let Some(path) = Path::from_string(&arc.target_path) {
                if ui.link(&arc.target_path).clicked() {
                    state.actions.push(CompositionAction::SelectPath(path));
                }
            } else {
                ui.label(RichText::new(&arc.target_path).color(Color32::from_rgb(200, 200, 200)));
            }
        } else {
            ui.label(RichText::new(&arc.target_path).color(Color32::from_rgb(200, 200, 200)));
        }

        // Introduction site
        if !arc.intro_path.is_empty() {
            ui.label(
                RichText::new(format!("@ {}", arc.intro_path))
                    .color(Color32::from_rgb(120, 120, 120))
                    .small(),
            );
        }
    });

    // Context menu
    resp.response.context_menu(|ui| {
        if ui.button("Copy Layer Identifier").clicked() {
            ui.ctx().copy_text(arc.layer_id.clone());
            state
                .actions
                .push(CompositionAction::CopyText(arc.layer_id.clone()));
            ui.close();
        }
        if ui.button("Copy Target Path").clicked() {
            ui.ctx().copy_text(arc.target_path.clone());
            state
                .actions
                .push(CompositionAction::CopyText(arc.target_path.clone()));
            ui.close();
        }
        if !arc.target_path.is_empty() && arc.arc_type != ArcType::VariantSet {
            if let Some(path) = Path::from_string(&arc.target_path) {
                if ui.button("Navigate to Target").clicked() {
                    state.actions.push(CompositionAction::SelectPath(path));
                    ui.close();
                }
            }
        }
    });
}
