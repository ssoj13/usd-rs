//! Enhanced attribute inspector panel.
//!
//! Full attribute/relationship/computed properties viewer matching reference
//! attributeValueEditor.py + propertyLegendUI.py.
//!
//! Features:
//! - 3 collapsible sections: Attributes, Relationships, Computed Properties
//! - Value source coloring (Fallback, TimeSample, Default, None, ValueClips)
//! - Type-aware inline editing for scalars
//! - Lazy array display: shows first 1000 elements with truncation footer
//! - Pretty-printing for matrices, bboxes, vectors
//! - Context menu: copy name, copy value, select target path

use egui::{Color32, RichText, Ui};
use std::sync::Arc;
use usd_core::resolve_info::ResolveInfoSource;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, TimeCode};
use usd_vt::Value;

use crate::bounds::compute_world_bound_for_view;
use crate::data_model::DataModel;
use crate::formatting;

// ---------------------------------------------------------------------------
// Value source colors (matching reference propertyLegendUI)
// ---------------------------------------------------------------------------

/// Fallback (schema default) — dark yellow.
const CLR_FALLBACK: Color32 = Color32::from_rgb(222, 158, 46);
/// Time samples — green.
const CLR_TIME_SAMPLE: Color32 = Color32::from_rgb(177, 207, 153);
/// Default value (authored default, no time samples) — light blue.
const CLR_DEFAULT: Color32 = Color32::from_rgb(135, 206, 250);
/// No value — gray.
const CLR_NONE: Color32 = Color32::from_rgb(140, 140, 140);
/// Value clips — purple.
const CLR_VALUE_CLIPS: Color32 = Color32::from_rgb(230, 150, 230);
/// Spline — cyan-ish.
const CLR_SPLINE: Color32 = Color32::from_rgb(80, 200, 180);
/// Custom (non-attribute properties: relationships, custom attrs) — red.
const CLR_CUSTOM: Color32 = Color32::from_rgb(230, 132, 131);

/// Max elements to display for array attributes (lazy loading threshold).
/// Arrays larger than this show first N elements with a "...M more" footer.
const MAX_ARRAY_DISPLAY: usize = 1000;

// ---------------------------------------------------------------------------
// Property type icons (reference: propertyLegend.py:98-112)
// ---------------------------------------------------------------------------

/// Unicode icons for property types (compact, no external images needed).
const ICON_ATTRIBUTE: &str = "\u{25CF}"; // filled circle
const ICON_RELATIONSHIP: &str = "\u{25CB}"; // empty circle
const ICON_ATTR_WITH_CONN: &str = "\u{25C9}"; // circle with dot
const ICON_REL_WITH_TARGETS: &str = "\u{25CE}"; // bullseye
const ICON_TARGET: &str = "\u{2192}"; // arrow
const ICON_CONNECTION: &str = "\u{21C4}"; // bidirectional
const ICON_COMPOSED: &str = "\u{25C6}"; // diamond

/// Map resolve info source to display color.
fn source_color(src: ResolveInfoSource) -> Color32 {
    match src {
        ResolveInfoSource::Fallback => CLR_FALLBACK,
        ResolveInfoSource::TimeSamples => CLR_TIME_SAMPLE,
        ResolveInfoSource::Default => CLR_DEFAULT,
        ResolveInfoSource::None => CLR_NONE,
        ResolveInfoSource::ValueClips => CLR_VALUE_CLIPS,
        ResolveInfoSource::Spline => CLR_SPLINE,
    }
}

/// Short label for the source type.
fn source_label(src: ResolveInfoSource) -> &'static str {
    match src {
        ResolveInfoSource::Fallback => "fallback",
        ResolveInfoSource::TimeSamples => "time",
        ResolveInfoSource::Default => "default",
        ResolveInfoSource::None => "none",
        ResolveInfoSource::ValueClips => "clips",
        ResolveInfoSource::Spline => "spline",
    }
}

// ---------------------------------------------------------------------------
// Property view roles (reference: common.py PropertyViewDataRoles)
// ---------------------------------------------------------------------------

/// Role of a property row in the attribute view.
///
/// Mirrors `PropertyViewDataRoles` from the Python reference. Used to
/// conditionally show/hide context menu items per the reference filtering
/// logic in `attributeViewContextMenu.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyViewRole {
    /// Plain authored attribute.
    Attribute,
    /// Attribute that has authored connections.
    AttrWithConnection,
    /// Plain relationship (no targets).
    Relationship,
    /// Relationship with one or more forwarded targets.
    RelWithTargets,
    /// An individual target path node under a relationship.
    /// Not used in egui (targets are inline) but part of reference data model.
    // DEAD: variant never constructed, keep for Python reference parity (common.py PropertyViewDataRoles)
    #[allow(dead_code)]
    Target,
    /// An individual connection source under an attribute.
    /// Not used in egui (connections are inline) but part of reference data model.
    // DEAD: variant never constructed, keep for Python reference parity (common.py PropertyViewDataRoles)
    #[allow(dead_code)]
    Connection,
}

impl PropertyViewRole {
    /// Returns true if this role represents an individual target/connection
    /// path node (not a property itself). These show target-copy items only.
    fn is_target_like(self) -> bool {
        matches!(
            self,
            PropertyViewRole::Target | PropertyViewRole::Connection
        )
    }

    /// Returns true if this role is a property that owns targets/connections.
    fn has_targets(self) -> bool {
        matches!(
            self,
            PropertyViewRole::RelWithTargets | PropertyViewRole::AttrWithConnection
        )
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Actions the attribute panel can request.
#[derive(Debug, Clone)]
pub enum AttributeAction {
    /// Navigate to a target prim path (from clicking a relationship target).
    SelectPath(Path),
    /// Copy text to clipboard.
    CopyText(String),
    /// Jump to defining layer for an attribute (layer identifier).
    JumpToDefiningLayer { attr_name: String, layer_id: String },
    /// Open attribute in the value editor dialog.
    OpenInEditor { attr_name: String },
    /// View time-sampled attribute in SplineViewer panel.
    ViewSpline { attr_name: String },
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Cached attribute display data (avoids re-fetching every frame).
#[derive(Debug, Clone)]
struct CachedAttrRow {
    name: String,
    type_str: String,
    source: ResolveInfoSource,
    is_array: bool,
    /// Element count for array attributes (0 for scalars).
    array_len: usize,
    val_str: String,
}

/// Cached relationship display data.
#[derive(Debug, Clone)]
struct CachedRelRow {
    name: String,
    targets: Vec<Path>,
}

/// Persistent state for the enhanced attributes panel.
#[derive(Debug, Default)]
pub struct AttributesPanelState {
    /// Collected actions this frame.
    pub actions: Vec<AttributeAction>,
    /// Whether the attributes section is open.
    pub attrs_open: bool,
    /// Whether the relationships section is open.
    pub rels_open: bool,
    /// Whether the computed section is open.
    pub computed_open: bool,
    /// Inline edit buffers keyed by attribute name.
    pub edit_bufs: std::collections::HashMap<String, String>,
    /// P2-9: Per-attribute array page offset (keyed by attr name).
    array_page: std::collections::HashMap<String, usize>,
    // -- Attribute cache (invalidated on prim/time change) --
    /// Path of the prim these cached attrs belong to.
    cached_prim_path: Option<Path>,
    /// TimeCode used when cache was built.
    cached_time: Option<TimeCode>,
    /// Cached attribute rows.
    cached_attrs: Vec<CachedAttrRow>,
    /// Cached relationship rows.
    cached_rels: Vec<CachedRelRow>,
}

impl AttributesPanelState {
    pub fn new() -> Self {
        Self {
            attrs_open: true,
            rels_open: true,
            computed_open: true,
            ..Default::default()
        }
    }

    /// Rebuild attribute/relationship caches if prim or time changed.
    fn ensure_cache(&mut self, prim: &Prim, tc: TimeCode) {
        let prim_path = prim.path().clone();
        if self.cached_prim_path.as_ref() == Some(&prim_path) && self.cached_time == Some(tc) {
            return; // cache still valid
        }

        // Rebuild attribute cache
        self.cached_attrs.clear();
        for name in prim.get_attribute_names() {
            let name_str = name.to_string();
            let Some(attr) = prim.get_attribute(&name_str) else {
                continue;
            };
            let type_str = attr.type_name().to_string();
            let source = attr.get_resolve_info().source();
            let val = attr.get(tc);
            // Detect arrays by checking both type name and actual value
            let is_array = type_str.trim().ends_with("[]")
                || val.as_ref().map_or(false, |v| v.is_array_valued());
            let array_len = if is_array {
                val.as_ref().map_or(0, |v| v.array_size())
            } else {
                0
            };
            let val_str = val
                .as_ref()
                .map(|v| {
                    if is_array {
                        // Format with truncation at MAX_ARRAY_DISPLAY elements
                        formatting::fmt_val_n(v, MAX_ARRAY_DISPLAY)
                    } else {
                        formatting::fmt_val(v)
                    }
                })
                .unwrap_or_else(|| "None".to_string());
            self.cached_attrs.push(CachedAttrRow {
                name: name_str,
                type_str,
                source,
                is_array,
                array_len,
                val_str,
            });
        }

        // Rebuild relationship cache
        self.cached_rels.clear();
        for name in prim.get_relationship_names() {
            let name_str = name.to_string();
            let Some(rel) = prim.get_relationship(&name_str) else {
                continue;
            };
            let targets = rel.get_forwarded_targets();
            self.cached_rels.push(CachedRelRow {
                name: name_str,
                targets,
            });
        }

        self.cached_prim_path = Some(prim_path);
        self.cached_time = Some(tc);
    }

    /// Invalidate cache (e.g. after editing an attribute).
    pub fn invalidate_cache(&mut self) {
        self.cached_prim_path = None;
    }
}

// ---------------------------------------------------------------------------
// Main UI
// ---------------------------------------------------------------------------

/// Draws the enhanced attribute inspector panel.
pub fn ui_attributes_enhanced(
    ui: &mut Ui,
    data_model: &DataModel,
    state: &mut AttributesPanelState,
) {
    state.actions.clear();

    let Some(prim) = data_model.first_selected_prim() else {
        ui.label("Select a prim to inspect attributes.");
        return;
    };

    // Header: path + type
    ui.heading(prim.path().to_string());
    let type_name = prim.type_name().to_string();
    if !type_name.is_empty() {
        ui.label(RichText::new(format!("Type: {}", type_name)).small());
    }

    // Source color legend
    draw_legend(ui);
    ui.separator();

    let tc = data_model.root.current_time;
    let stage = data_model.root.stage.as_ref();

    // Rebuild cache only when prim or time changes
    state.ensure_cache(&prim, tc);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Section 1: Attributes (from cache)
            draw_attributes_section_cached(ui, &prim, tc, stage, state);

            // Section 2: Relationships (from cache)
            draw_relationships_section_cached(ui, state);

            // Section 3: Computed Properties
            draw_computed_section(ui, &prim, tc, data_model);
        });
}

/// Color legend bar with colored indicator squares (P1-14 parity with C++ propertyLegendUI).
fn draw_legend(ui: &mut Ui) {
    egui::CollapsingHeader::new(
        RichText::new("Property Source Legend")
            .small()
            .color(Color32::GRAY),
    )
    .default_open(true)
    .show(ui, |ui| {
        // Value source colors
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            // Reference: propertyLegend.py — 7 color entries
            let entries: &[(&str, Color32, bool)] = &[
                ("Fallback", CLR_FALLBACK, false),
                ("TimeSample", CLR_TIME_SAMPLE, true), // interpolated
                ("Default", CLR_DEFAULT, false),
                ("None", CLR_NONE, false),
                ("ValueClips", CLR_VALUE_CLIPS, true), // interpolated
                ("Custom", CLR_CUSTOM, false),
                ("Spline", CLR_SPLINE, true), // interpolated
            ];
            for &(label, color, interpolated) in entries {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, color);
                    // Interpolated sources get italic marker (ref: propertyLegend.py:86-95)
                    let text = if interpolated {
                        RichText::new(label).color(color).small().italics()
                    } else {
                        RichText::new(label).color(color).small()
                    };
                    ui.label(text);
                });
            }
        });
        // Property type icons legend
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (icon, label) in [
                (ICON_ATTRIBUTE, "Attribute"),
                (ICON_RELATIONSHIP, "Relationship"),
                (ICON_ATTR_WITH_CONN, "Attr+Connection"),
                (ICON_REL_WITH_TARGETS, "Rel+Targets"),
                (ICON_TARGET, "Target"),
                (ICON_CONNECTION, "Connection"),
                (ICON_COMPOSED, "Composed"),
            ] {
                ui.label(RichText::new(format!("{} {}", icon, label)).small());
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Attributes section
// ---------------------------------------------------------------------------

/// Draw attributes section from cached data.
fn draw_attributes_section_cached(
    ui: &mut Ui,
    prim: &Prim,
    tc: TimeCode,
    stage: Option<&Arc<Stage>>,
    state: &mut AttributesPanelState,
) {
    let count = state.cached_attrs.len();
    let header = format!("Attributes ({})", count);

    egui::CollapsingHeader::new(header)
        .default_open(true)
        .show(ui, |ui| {
            if count == 0 {
                ui.label("No attributes.");
                return;
            }

            egui::Grid::new("attrs_grid")
                .num_columns(3)
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("Name").strong().small());
                    ui.label(RichText::new("Type").strong().small());
                    ui.label(RichText::new("Value").strong().small());
                    ui.end_row();

                    // Iterate by index to avoid cloning the entire cached_attrs Vec.
                    // draw_cached_attr_row reads state.cached_attrs[idx] internally.
                    for idx in 0..count {
                        draw_cached_attr_row(ui, prim, idx, tc, stage, state);
                        ui.end_row();
                    }
                });
        });
}

/// Draw a single attribute row from cached data.
///
/// Takes `idx` into `state.cached_attrs` instead of a direct ref to avoid
/// cloning the entire display list on every frame.
fn draw_cached_attr_row(
    ui: &mut Ui,
    prim: &Prim,
    idx: usize,
    tc: TimeCode,
    stage: Option<&Arc<Stage>>,
    state: &mut AttributesPanelState,
) {
    // Read fields we need for rendering before any mutable borrow of state.
    let (name, type_str, source, is_array, array_len, val_str) = {
        let row = &state.cached_attrs[idx];
        (
            row.name.clone(),
            row.type_str.clone(),
            row.source,
            row.is_array,
            row.array_len,
            row.val_str.clone(),
        )
    };
    let color = source_color(source);

    // Property icon + Name column (colored by source)
    // Reference: propertyLegend.py — icon before name
    let has_connections = prim
        .get_attribute(&name)
        .map(|a| a.has_authored_connections())
        .unwrap_or(false);
    // Determine role for role-based context menu filtering
    // (reference: attributeViewContextMenu.py ShouldDisplay logic)
    let role = if has_connections {
        PropertyViewRole::AttrWithConnection
    } else {
        PropertyViewRole::Attribute
    };
    let icon = if has_connections {
        ICON_ATTR_WITH_CONN
    } else {
        ICON_ATTRIBUTE
    };
    let name_resp = ui.label(RichText::new(format!("{} {}", icon, name)).color(color));
    // Double-click on attribute name opens editor dialog
    if name_resp.double_clicked() {
        state.actions.push(AttributeAction::OpenInEditor {
            attr_name: name.clone(),
        });
    }
    name_resp.context_menu(|ui| {
        // "Copy Name" — hidden for individual Target/Connection nodes (reference:
        // CopyAttributeNameMenuItem.ShouldDisplay)
        if !role.is_target_like() {
            if ui.button("Copy Property Name").clicked() {
                ui.ctx().copy_text(name.clone());
                state.actions.push(AttributeAction::CopyText(name.clone()));
                ui.close();
            }
        }
        // Copy full attribute path — always shown
        let attr_path = format!("{}.{}", prim.path(), name);
        if ui.button("Copy Property Path").clicked() {
            ui.ctx().copy_text(attr_path.clone());
            state.actions.push(AttributeAction::CopyText(attr_path));
            ui.close();
        }
        // "Copy Value" — hidden for individual Target/Connection nodes (reference:
        // CopyAttributeValueMenuItem.ShouldDisplay)
        if !role.is_target_like() {
            if ui.button("Copy Property Value").clicked() {
                ui.ctx().copy_text(val_str.clone());
                state
                    .actions
                    .push(AttributeAction::CopyText(val_str.clone()));
                ui.close();
            }
        }
        // Open in editor dialog — always shown
        if ui.button("Open in Editor...").clicked() {
            state.actions.push(AttributeAction::OpenInEditor {
                attr_name: name.clone(),
            });
            ui.close();
        }
        // P2-11: "View Spline" — only for time-sampled/spline attributes
        if matches!(
            source,
            ResolveInfoSource::TimeSamples | ResolveInfoSource::Spline
        ) {
            if ui.button("View Spline").clicked() {
                state.actions.push(AttributeAction::ViewSpline {
                    attr_name: name.clone(),
                });
                ui.close();
            }
        }
        ui.separator();
        // "Jump to Defining Layer" — always shown when available
        if let Some(attr) = prim.get_attribute(&name) {
            let specs = attr.as_property().get_property_stack();
            if let Some(first_spec) = specs.first() {
                if let Some(layer) = first_spec.spec().layer().upgrade() {
                    let layer_id = layer.identifier().to_string();
                    let attr_name_c = name.clone();
                    if ui
                        .button(format!("Jump to Defining Layer ({})", &layer_id))
                        .clicked()
                    {
                        state.actions.push(AttributeAction::JumpToDefiningLayer {
                            attr_name: attr_name_c,
                            layer_id,
                        });
                        ui.close();
                    }
                }
            }
        }
    });

    // Type column
    let source_tag = source_label(source);
    ui.label(
        RichText::new(format!("{} [{}]", type_str, source_tag))
            .small()
            .color(Color32::from_rgb(160, 160, 160)),
    );

    // Value column
    if is_array {
        // P2-9: Paginated array view (ref: arrayAttributeView.py)
        let page_size = MAX_ARRAY_DISPLAY;
        let total_pages = if array_len == 0 {
            1
        } else {
            (array_len + page_size - 1) / page_size
        };
        let page = *state.array_page.get(&name).unwrap_or(&0);
        let page = page.min(total_pages.saturating_sub(1));
        let offset = page * page_size;
        let end = (offset + page_size).min(array_len);

        ui.vertical(|ui| {
            // Header: "Array [N elements]"
            ui.label(
                RichText::new(format!(
                    "Array [{} elements]",
                    formatting::fmt_int(array_len as i64)
                ))
                .small()
                .color(Color32::from_rgb(180, 180, 120)),
            );

            // Page controls (only if more than one page)
            if total_pages > 1 {
                ui.horizontal(|ui| {
                    // First page
                    if ui
                        .add_enabled(page > 0, egui::Button::new("|<").small())
                        .clicked()
                    {
                        state.array_page.insert(name.clone(), 0);
                    }
                    // Prev page
                    if ui
                        .add_enabled(page > 0, egui::Button::new("<").small())
                        .clicked()
                    {
                        state
                            .array_page
                            .insert(name.clone(), page.saturating_sub(1));
                    }
                    // Page indicator: "0-999 of 50000"
                    ui.label(
                        RichText::new(format!(
                            "{}-{} of {}",
                            offset,
                            end.saturating_sub(1),
                            array_len
                        ))
                        .small()
                        .color(Color32::from_rgb(160, 160, 160)),
                    );
                    // Next page
                    if ui
                        .add_enabled(page + 1 < total_pages, egui::Button::new(">").small())
                        .clicked()
                    {
                        state.array_page.insert(name.clone(), page + 1);
                    }
                    // Last page
                    if ui
                        .add_enabled(page + 1 < total_pages, egui::Button::new(">|").small())
                        .clicked()
                    {
                        state.array_page.insert(name.clone(), total_pages - 1);
                    }
                });
            }

            // Fetch current page slice from attribute and format with indices
            // (ref: arrayAttributeView.py — "idx: value" per row)
            let page_text = if let Some(attr) = prim.get_attribute(&name) {
                if let Some(val) = attr.get(tc) {
                    formatting::fmt_val_page(&val, offset, end)
                } else {
                    "None".to_string()
                }
            } else {
                val_str.clone()
            };

            let resp = ui.label(
                RichText::new(&page_text)
                    .color(Color32::from_rgb(200, 200, 200))
                    .monospace()
                    .small(),
            );
            resp.context_menu(|ui| {
                if ui.button("Copy Page").clicked() {
                    ui.ctx().copy_text(page_text.clone());
                    ui.close();
                }
                if ui.button("Copy All").clicked() {
                    ui.ctx().copy_text(val_str.clone());
                    ui.close();
                }
            });
        });
    } else {
        // For editable scalars, still need the live Attribute handle
        if let Some(attr) = prim.get_attribute(&name) {
            draw_scalar_edit(ui, &attr, &name, tc, stage, &type_str, state);
        } else {
            ui.label(RichText::new(&val_str).color(Color32::from_rgb(200, 200, 200)));
        }
    }
}

/// Editable scalar value widget.
fn draw_scalar_edit(
    ui: &mut Ui,
    attr: &Attribute,
    name: &str,
    tc: TimeCode,
    stage: Option<&Arc<Stage>>,
    type_str: &str,
    state: &mut AttributesPanelState,
) {
    let can_edit = stage.and_then(|s| s.get_session_layer()).is_some();
    let val_opt = attr.get(tc);

    let base_type = type_str.strip_suffix("[]").unwrap_or(type_str).trim();

    if can_edit {
        match base_type {
            "int" | "int64" => {
                if let Some(ref v) = val_opt {
                    if let Some(&i) = v.get::<i32>() {
                        let mut x = i as f64;
                        if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                            set_on_session(stage, attr, Value::from(x as i32), tc);
                        }
                        return;
                    }
                    if let Some(&i) = v.get::<i64>() {
                        let mut x = i as f64;
                        if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                            set_on_session(stage, attr, Value::from(x as i64), tc);
                        }
                        return;
                    }
                }
            }
            "float" | "double" | "half" => {
                if let Some(ref v) = val_opt {
                    if let Some(&f) = v.get::<f32>() {
                        let mut x = f as f64;
                        if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                            set_on_session(stage, attr, Value::from(x as f32), tc);
                        }
                        return;
                    }
                    if let Some(&f) = v.get::<f64>() {
                        let mut x = f;
                        if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                            set_on_session(stage, attr, Value::from(x), tc);
                        }
                        return;
                    }
                }
            }
            "bool" => {
                if let Some(ref v) = val_opt {
                    if let Some(&b) = v.get::<bool>() {
                        let mut x = b;
                        if ui.checkbox(&mut x, "").changed() {
                            set_on_session(stage, attr, Value::from(x), tc);
                        }
                        return;
                    }
                }
            }
            "string" | "token" => {
                let current = val_opt
                    .as_ref()
                    .and_then(|v| {
                        v.get::<String>()
                            .cloned()
                            .or_else(|| v.get::<usd_tf::Token>().map(|t| t.get_text().to_string()))
                    })
                    .unwrap_or_default();

                let buf = state
                    .edit_bufs
                    .entry(name.to_string())
                    .or_insert_with(|| current.clone());

                let resp = ui.add(
                    egui::TextEdit::singleline(buf)
                        .desired_width(120.0)
                        .hint_text("value"),
                );
                if resp.lost_focus() && *buf != current {
                    let new_val = if base_type == "token" {
                        Value::from(usd_tf::Token::new(buf))
                    } else {
                        Value::from(buf.clone())
                    };
                    set_on_session(stage, attr, new_val, tc);
                }
                return;
            }
            _ => {}
        }
    }

    // Read-only display
    let val_str = val_opt
        .as_ref()
        .map(|v| formatting::fmt_val(v))
        .unwrap_or_else(|| "None".to_string());

    let resp = ui.label(RichText::new(&val_str).color(Color32::from_rgb(200, 200, 200)));
    resp.context_menu(|ui| {
        if ui.button("Copy Value").clicked() {
            ui.ctx().copy_text(val_str.clone());
            ui.close();
        }
    });
}

/// Sets attribute value on session layer and restores edit target.
/// NOTE: caller should invalidate AttributesPanelState cache after this.
fn set_on_session(stage: Option<&Arc<Stage>>, attr: &Attribute, value: Value, tc: TimeCode) {
    let Some(stage) = stage else { return };
    let Some(session) = stage.get_session_layer() else {
        return;
    };
    let prev = stage.get_edit_target();
    stage.set_edit_target(usd_core::EditTarget::for_local_layer(session));
    let _ = attr.set(value, tc);
    stage.set_edit_target(prev);
}

// ---------------------------------------------------------------------------
// Relationships section
// ---------------------------------------------------------------------------

/// Draw relationships section from cached data.
fn draw_relationships_section_cached(ui: &mut Ui, state: &mut AttributesPanelState) {
    let rows: Vec<CachedRelRow> = state.cached_rels.clone();
    let header = format!("Relationships ({})", rows.len());

    egui::CollapsingHeader::new(header)
        .default_open(true)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.label("No relationships.");
                return;
            }

            for row in &rows {
                // Determine role for this relationship row
                // (reference: PropertyViewDataRoles.RELATIONSHIP vs RELATIONSHIP_WITH_TARGETS)
                let rel_role = if row.targets.is_empty() {
                    PropertyViewRole::Relationship
                } else {
                    PropertyViewRole::RelWithTargets
                };

                ui.horizontal(|ui| {
                    // Relationship icon: with targets or plain
                    let icon = if rel_role == PropertyViewRole::RelWithTargets {
                        ICON_REL_WITH_TARGETS
                    } else {
                        ICON_RELATIONSHIP
                    };
                    // Non-attribute properties use Custom color (ref: common.py:330-332)
                    ui.label(RichText::new(format!("{} {}", icon, row.name)).color(CLR_CUSTOM));
                    ui.label("->");

                    if row.targets.is_empty() {
                        ui.label(RichText::new("[]").color(Color32::from_rgb(140, 140, 140)));
                    } else {
                        for target in &row.targets {
                            let target_str = target.to_string();
                            if ui.link(&target_str).clicked() {
                                state
                                    .actions
                                    .push(AttributeAction::SelectPath(target.clone()));
                            }
                        }
                    }
                })
                .response
                .context_menu(|ui| {
                    // "Copy Name" — shown for relationship rows (not individual target nodes)
                    // (reference: CopyAttributeNameMenuItem.ShouldDisplay)
                    if ui.button("Copy Property Name").clicked() {
                        ui.ctx().copy_text(row.name.clone());
                        ui.close();
                    }

                    // "Copy/Select All Target Paths" — only for relationships with targets
                    // (reference: CopyAllTargetPathsMenuItem + SelectAllTargetPathsMenuItem)
                    if rel_role.has_targets() {
                        let all_paths: String = row
                            .targets
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        if ui.button("Copy Target Path(s) As Text").clicked() {
                            ui.ctx().copy_text(all_paths);
                            ui.close();
                        }
                        // "Select Target Path(s)" — navigate to the first target prim
                        // (reference: SelectAllTargetPathsMenuItem.RunCommand)
                        if ui.button("Select Target Path(s)").clicked() {
                            for target in &row.targets {
                                state
                                    .actions
                                    .push(AttributeAction::SelectPath(target.clone()));
                            }
                            ui.close();
                        }
                        ui.separator();
                        // Individual target copy/select items (reference: CopyTargetPathMenuItem
                        // + SelectTargetPathMenuItem shown for TARGET role children)
                        for target in &row.targets {
                            let ts = target.to_string();
                            if ui.button(format!("Copy Target: {}", &ts)).clicked() {
                                ui.ctx().copy_text(ts);
                                ui.close();
                            }
                            if ui.button(format!("Select Target: {}", target)).clicked() {
                                state
                                    .actions
                                    .push(AttributeAction::SelectPath(target.clone()));
                                ui.close();
                            }
                        }
                    }
                });
            }
        });
}

// ---------------------------------------------------------------------------
// Computed properties section
// ---------------------------------------------------------------------------

fn draw_computed_section(ui: &mut Ui, prim: &Prim, tc: TimeCode, data_model: &DataModel) {
    egui::CollapsingHeader::new("Computed Properties")
        .default_open(false)
        .show(ui, |ui| {
            // World BBox (collapsible with detailed info)
            let imageable = usd_geom::imageable::Imageable::new(prim.clone());
            if imageable.is_valid() {
                let bbox = compute_world_bound_for_view(&imageable, tc, &data_model.view);
                let range = bbox.compute_aligned_range();
                if !range.is_empty() {
                    let min = range.min();
                    let max = range.max();
                    // Summary line with min/max
                    let header_text = format!("World BBox: {}", formatting::fmt_bbox(min, max));
                    egui::CollapsingHeader::new(
                        RichText::new(header_text).color(Color32::from_rgb(180, 180, 220)),
                    )
                    .id_salt("computed_world_bbox")
                    .default_open(false)
                    .show(ui, |ui| {
                        // Full bbox detail: corners, center, dims, volume, matrix, flags
                        ui.label(
                            RichText::new(formatting::fmt_bbox3d(&bbox))
                                .monospace()
                                .small(),
                        );
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("World BBox:");
                        ui.label("(empty)");
                    });
                }

                // Local-to-World Transform (collapsible)
                let xform = imageable.compute_local_to_world_transform(tc);
                egui::CollapsingHeader::new(
                    RichText::new("Local-to-World Xform").color(Color32::from_rgb(180, 180, 220)),
                )
                .id_salt("computed_l2w")
                .default_open(false)
                .show(ui, |ui| {
                    let slice = xform.as_slice();
                    let data: [f64; 16] = std::array::from_fn(|i| slice[i]);
                    ui.label(
                        RichText::new(formatting::fmt_matrix4(&data))
                            .monospace()
                            .small(),
                    );
                });

                // Parent-to-World Transform (collapsible)
                let parent_xform = imageable.compute_parent_to_world_transform(tc);
                egui::CollapsingHeader::new(
                    RichText::new("Parent-to-World Xform").color(Color32::from_rgb(180, 180, 220)),
                )
                .id_salt("computed_p2w")
                .default_open(false)
                .show(ui, |ui| {
                    let slice = parent_xform.as_slice();
                    let data: [f64; 16] = std::array::from_fn(|i| slice[i]);
                    ui.label(
                        RichText::new(formatting::fmt_matrix4(&data))
                            .monospace()
                            .small(),
                    );
                });
            }

            // Resolved materials via ComputeBoundMaterial (ref: customAttributes.py:108-140)
            // Shows computed inherited material, not raw relationship targets
            {
                use usd_shade::material_binding_api::MaterialBindingAPI;
                let api = MaterialBindingAPI::new(prim.clone());
                if api.is_valid() {
                    // Preview purpose
                    let preview_tok = usd_shade::tokens::tokens().preview.clone();
                    let mut binding_rel: Option<usd_core::Relationship> = None;
                    let mat = api.compute_bound_material(&preview_tok, &mut binding_rel, false);
                    let preview_path = if mat.get_prim().is_valid() {
                        mat.get_prim().path().to_string()
                    } else {
                        "<unbound>".to_string()
                    };
                    // Full purpose
                    let full_tok = usd_shade::tokens::tokens().full.clone();
                    let mut binding_rel2: Option<usd_core::Relationship> = None;
                    let mat2 = api.compute_bound_material(&full_tok, &mut binding_rel2, false);
                    let full_path = if mat2.get_prim().is_valid() {
                        mat2.get_prim().path().to_string()
                    } else {
                        "<unbound>".to_string()
                    };

                    if preview_path != "<unbound>" || full_path != "<unbound>" {
                        ui.separator();
                        ui.label(
                            RichText::new("Resolved Materials:")
                                .color(Color32::from_rgb(180, 180, 220)),
                        );
                        ui.horizontal(|ui| {
                            ui.label("  Preview:");
                            ui.label(RichText::new(&preview_path).color(CLR_CUSTOM));
                        });
                        ui.horizontal(|ui| {
                            ui.label("  Full:");
                            ui.label(RichText::new(&full_path).color(CLR_CUSTOM));
                        });
                    }
                }
            }

            // Resolved Labels (UsdSemanticsLabelsAPI)
            // Reference: customAttributes.py ResolvedLabelsAttribute
            {
                use usd_semantics::LabelsAPI;
                let direct_taxonomies = LabelsAPI::get_direct_taxonomies(prim);
                let inherited_taxonomies = LabelsAPI::compute_inherited_taxonomies(prim);
                if !direct_taxonomies.is_empty() || !inherited_taxonomies.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new("Resolved Labels:").color(Color32::from_rgb(180, 180, 220)),
                    );
                    for tax in &direct_taxonomies {
                        let api = LabelsAPI::from_prim(prim, tax);
                        if let Some(attr) = api.get_labels_attr() {
                            let val_str = if let Some(val) = attr.get(tc) {
                                format!("{}", val)
                            } else {
                                "(none)".to_string()
                            };
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("  {}:", tax.get_text()));
                                ui.label(
                                    RichText::new(&val_str).color(Color32::from_rgb(140, 220, 140)),
                                );
                            });
                        }
                    }
                    // Show inherited-only taxonomies
                    for tax in &inherited_taxonomies {
                        if direct_taxonomies.contains(tax) {
                            continue;
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("  {} (inherited):", tax.get_text()));
                            ui.label(RichText::new("yes").color(Color32::from_rgb(140, 180, 220)));
                        });
                    }
                }
            }
        });
}
