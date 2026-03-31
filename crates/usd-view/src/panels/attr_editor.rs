//! Attribute value editor dialog.
//!
//! Modal egui::Window for editing complex attribute values:
//! - Arrays displayed as scrollable tables
//! - Matrices as editable 4x4 grids
//! - Large strings in multiline text editors
//! - Changes applied to session layer
//!
//! Reference: attributeValueEditor.py from usdviewq.

use egui::{Color32, RichText, Ui};
use std::sync::Arc;
use usd_core::{Attribute, EditTarget, Stage};
use usd_sdf::TimeCode;
use usd_vt::Value;

use crate::formatting;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Persistent state for the attribute editor dialog.
#[derive(Debug, Default)]
pub struct AttrEditorState {
    /// Whether the dialog is open.
    pub open: bool,
    /// Prim path of the attribute being edited.
    pub prim_path: Option<usd_sdf::Path>,
    /// Attribute name being edited.
    pub attr_name: Option<String>,
    /// Type name string of the attribute.
    pub type_name: String,
    /// Cached string representation for read-only complex values.
    pub cached_display: String,
    /// Edit buffer for string/token editing.
    pub string_buf: String,
    /// Edit buffers for matrix editing (4x4 = 16 cells).
    pub matrix_buf: [String; 16],
    /// Whether the value was modified (needs save).
    pub dirty: bool,
    /// Kind of editor to show.
    pub editor_kind: EditorKind,
    /// Edit buffers for array rows (index -> value string).
    pub array_bufs: Vec<String>,
}

/// What kind of editor to show in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorKind {
    /// Read-only display (for unsupported types).
    #[default]
    ReadOnly,
    /// Single-line string/token editor.
    StringEdit,
    /// Multiline string editor.
    MultilineEdit,
    /// 4x4 matrix editor grid.
    MatrixEdit,
    /// Scrollable array table.
    ArrayView,
}

impl AttrEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the editor for a specific attribute.
    pub fn open_for(
        &mut self,
        prim_path: &usd_sdf::Path,
        attr_name: &str,
        attr: &Attribute,
        tc: TimeCode,
    ) {
        self.open = true;
        self.prim_path = Some(prim_path.clone());
        self.attr_name = Some(attr_name.to_string());
        self.type_name = attr.type_name().to_string();
        self.dirty = false;

        let val_opt = attr.get(tc);

        // Determine editor kind based on type
        let base = self.type_name.trim();
        let is_array = base.ends_with("[]");

        if is_array {
            self.editor_kind = EditorKind::ArrayView;
            // Populate array buffers
            self.array_bufs.clear();
            if let Some(ref val) = val_opt {
                let count = val.array_size();
                // Show full value as cached display
                self.cached_display = formatting::fmt_val(val);
                // Individual elements as strings (for potential future editing)
                for _ in 0..count.min(1000) {
                    // Array element extraction is complex; show full display
                }
            } else {
                self.cached_display = "None".to_string();
            }
        } else if base.contains("matrix") || base.contains("Matrix") {
            self.editor_kind = EditorKind::MatrixEdit;
            // Try to extract 16 doubles from the value
            self.matrix_buf = Default::default();
            if let Some(ref val) = val_opt {
                // Try extracting as raw display and parse
                let display = formatting::fmt_val(val);
                self.cached_display = display;
                // Parse matrix values from the formatted output
                if let Some(ref val) = val_opt {
                    let floats = extract_matrix_values(val);
                    for (i, f) in floats.iter().enumerate().take(16) {
                        self.matrix_buf[i] = format!("{:.6}", f);
                    }
                }
            }
        } else if base == "string" || base == "token" || base == "asset" {
            let s = val_opt
                .as_ref()
                .map(|v| {
                    v.get::<String>()
                        .cloned()
                        .or_else(|| v.get::<usd_tf::Token>().map(|t| t.get_text().to_string()))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            // Use multiline for long strings
            if s.len() > 100 || s.contains('\n') {
                self.editor_kind = EditorKind::MultilineEdit;
            } else {
                self.editor_kind = EditorKind::StringEdit;
            }
            self.string_buf = s;
            self.cached_display = String::new();
        } else {
            self.editor_kind = EditorKind::ReadOnly;
            self.cached_display = val_opt
                .as_ref()
                .map(|v| formatting::fmt_val(v))
                .unwrap_or_else(|| "None".to_string());
        }
    }

    /// Close the editor.
    pub fn close(&mut self) {
        self.open = false;
        self.prim_path = None;
        self.attr_name = None;
        self.dirty = false;
    }
}

/// Try to extract 16 f64 values from a matrix VtValue.
fn extract_matrix_values(val: &Value) -> Vec<f64> {
    // Attempt to read as string and parse numbers
    let s = formatting::fmt_val(val);
    let mut result = Vec::new();
    for part in s.split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e') {
        if let Ok(f) = part.parse::<f64>() {
            result.push(f);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Main UI
// ---------------------------------------------------------------------------

/// Draw the attribute editor dialog (if open).
pub fn ui_attr_editor(
    ctx: &egui::Context,
    state: &mut AttrEditorState,
    stage: Option<&Arc<Stage>>,
) {
    if !state.open {
        return;
    }

    let title = format!("Edit: {}", state.attr_name.as_deref().unwrap_or("(none)"));

    let mut open = state.open;
    egui::Window::new(title)
        .open(&mut open)
        .resizable(true)
        .default_size([500.0, 400.0])
        .show(ctx, |ui| {
            // Header info
            ui.horizontal(|ui| {
                if let Some(ref path) = state.prim_path {
                    ui.label(RichText::new(format!("Prim: {}", path)).small());
                }
                ui.label(
                    RichText::new(format!("Type: {}", state.type_name))
                        .small()
                        .color(Color32::from_rgb(160, 160, 160)),
                );
            });
            ui.separator();

            match state.editor_kind {
                EditorKind::ReadOnly => {
                    draw_readonly(ui, &state.cached_display);
                }
                EditorKind::StringEdit => {
                    draw_string_edit(ui, &mut state.string_buf, &mut state.dirty);
                }
                EditorKind::MultilineEdit => {
                    draw_multiline_edit(ui, &mut state.string_buf, &mut state.dirty);
                }
                EditorKind::MatrixEdit => {
                    draw_matrix_edit(ui, &mut state.matrix_buf, &mut state.dirty);
                }
                EditorKind::ArrayView => {
                    draw_array_view(ui, &state.cached_display);
                }
            }

            ui.separator();

            // Apply / Cancel buttons
            ui.horizontal(|ui| {
                let can_apply = state.dirty && stage.is_some();
                if ui
                    .add_enabled(can_apply, egui::Button::new("Apply to Session"))
                    .clicked()
                {
                    apply_edit(state, stage);
                }
                if ui.button("Close").clicked() {
                    state.close();
                }
            });
        });

    if !open {
        state.close();
    }
}

// ---------------------------------------------------------------------------
// Editor sub-views
// ---------------------------------------------------------------------------

fn draw_readonly(ui: &mut Ui, display: &str) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(RichText::new(display).monospace().small());
        });
}

fn draw_string_edit(ui: &mut Ui, buf: &mut String, dirty: &mut bool) {
    let resp = ui.add(
        egui::TextEdit::singleline(buf)
            .desired_width(ui.available_width())
            .hint_text("value"),
    );
    if resp.changed() {
        *dirty = true;
    }
}

fn draw_multiline_edit(ui: &mut Ui, buf: &mut String, dirty: &mut bool) {
    let resp = ui.add(
        egui::TextEdit::multiline(buf)
            .desired_width(ui.available_width())
            .desired_rows(10)
            .code_editor(),
    );
    if resp.changed() {
        *dirty = true;
    }
}

fn draw_matrix_edit(ui: &mut Ui, cells: &mut [String; 16], dirty: &mut bool) {
    ui.label(RichText::new("4x4 Matrix:").strong());
    egui::Grid::new("matrix_grid")
        .num_columns(4)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for row in 0..4 {
                for col in 0..4 {
                    let idx = row * 4 + col;
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut cells[idx])
                            .desired_width(80.0)
                            .font(egui::TextStyle::Monospace),
                    );
                    if resp.changed() {
                        *dirty = true;
                    }
                }
                ui.end_row();
            }
        });
}

fn draw_array_view(ui: &mut Ui, display: &str) {
    ui.label(RichText::new("Array contents (read-only):").strong());
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(300.0)
        .show(ui, |ui| {
            ui.label(RichText::new(display).monospace().small());
        });
}

// ---------------------------------------------------------------------------
// Apply edits to session layer
// ---------------------------------------------------------------------------

fn apply_edit(state: &mut AttrEditorState, stage: Option<&Arc<Stage>>) {
    let Some(stage) = stage else { return };
    let Some(session) = stage.get_session_layer() else {
        return;
    };
    let Some(ref prim_path) = state.prim_path else {
        return;
    };
    let Some(ref attr_name) = state.attr_name else {
        return;
    };
    let Some(prim) = stage.get_prim_at_path(prim_path) else {
        return;
    };
    let Some(attr) = prim.get_attribute(attr_name) else {
        return;
    };

    let prev = stage.get_edit_target();
    stage.set_edit_target(EditTarget::for_local_layer(session));

    let tc = TimeCode::default();
    let base = state.type_name.trim();

    match state.editor_kind {
        EditorKind::StringEdit | EditorKind::MultilineEdit => {
            let new_val = if base == "token" {
                Value::from(usd_tf::Token::new(&state.string_buf))
            } else {
                Value::from(state.string_buf.clone())
            };
            let _ = attr.set(new_val, tc);
        }
        EditorKind::MatrixEdit => {
            // Parse 16 f64 values from buffers
            let mut vals = [0.0f64; 16];
            for (i, cell) in state.matrix_buf.iter().enumerate() {
                vals[i] = cell.parse::<f64>().unwrap_or(0.0);
            }
            // Set as Matrix4d (most common)
            let mat = usd_gf::Matrix4d::new(
                vals[0], vals[1], vals[2], vals[3], vals[4], vals[5], vals[6], vals[7], vals[8],
                vals[9], vals[10], vals[11], vals[12], vals[13], vals[14], vals[15],
            );
            let _ = attr.set(Value::from(mat), tc);
        }
        _ => {} // ReadOnly / ArrayView: no edits to apply
    }

    stage.set_edit_target(prev);
    state.dirty = false;
}
