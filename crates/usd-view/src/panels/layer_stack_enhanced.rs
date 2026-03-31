//! Enhanced layer stack panel.
//!
//! Shows the layer stack with file sizes, mute indicators, layer offsets,
//! and a full context menu (mute/unmute, open in editor, open in usdview,
//! copy identifier/path/object path).
//!
//! Two modes:
//! - Root Stack: all layers in the stage layer stack.
//! - Prim Stack: only layers that have a spec at the selected prim path,
//!   with specifier (def/over/class), type name, and session/root highlights.

use std::process::Command;
use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use usd_core::Stage;
use usd_sdf::{Layer, LayerOffset, Path, Specifier};

use crate::data_model::DataModel;
use crate::formatting;

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Actions emitted by the layer stack panel for the caller to process.
#[derive(Debug, Clone)]
pub enum LayerStackAction {
    CopyText(String),
    MuteLayer(String),
    UnmuteLayer(String),
    OpenInEditor(String),
    OpenInUsdview(String),
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Which layer stack view is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackMode {
    /// Show all layers in the stage root layer stack.
    #[default]
    Root,
    /// Show only layers that contribute a spec to the selected prim.
    Prim,
}

/// Persistent state for the enhanced layer stack panel.
#[derive(Debug, Default)]
pub struct LayerStackPanelState {
    pub actions: Vec<LayerStackAction>,
    pub mode: StackMode,
}

impl LayerStackPanelState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Main UI entry point
// ---------------------------------------------------------------------------

/// Draws the enhanced layer stack panel.
pub fn ui_layer_stack_enhanced(
    ui: &mut Ui,
    data_model: &DataModel,
    state: &mut LayerStackPanelState,
) {
    state.actions.clear();

    let Some(ref stage) = data_model.root.stage else {
        ui.label("No stage loaded.");
        return;
    };

    // Mode toggle bar.
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.mode, StackMode::Root, "Root Stack");
        ui.selectable_value(&mut state.mode, StackMode::Prim, "Prim Stack");
    });
    ui.separator();

    match state.mode {
        StackMode::Root => draw_root_layer_stack(ui, stage, state),
        StackMode::Prim => draw_prim_layer_stack(ui, stage, data_model, state),
    }
}

// ---------------------------------------------------------------------------
// Root layer stack
// ---------------------------------------------------------------------------

fn draw_root_layer_stack(ui: &mut Ui, stage: &Arc<Stage>, state: &mut LayerStackPanelState) {
    let layers = stage.layer_stack();
    ui.heading("Root Layer Stack");
    ui.separator();

    if layers.is_empty() {
        ui.label("No layers.");
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let identity = LayerOffset::identity();
            for (i, layer) in layers.iter().enumerate() {
                draw_layer_row(ui, stage, i, layer, None, &identity, state);
            }
        });
}

// ---------------------------------------------------------------------------
// Prim-specific layer stack
// ---------------------------------------------------------------------------

fn draw_prim_layer_stack(
    ui: &mut Ui,
    stage: &Arc<Stage>,
    data_model: &DataModel,
    state: &mut LayerStackPanelState,
) {
    let Some(prim) = data_model.first_selected_prim() else {
        ui.label("Select a prim to see its contributing specs.");
        return;
    };

    if prim.path().is_absolute_root_path() {
        ui.label("Select a non-root prim to see its contributing specs.");
        return;
    }

    let stack = prim.get_prim_stack_with_layer_offsets();
    ui.heading(format!("Prim Stack: {}", prim.path()));
    ui.separator();

    if stack.is_empty() {
        ui.label("No contributing specs.");
        return;
    }

    // Collect root/session identifiers for badge highlights.
    let root_id = stage.get_root_layer().identifier().to_string();
    let session_id = stage
        .get_session_layer()
        .map(|l| l.identifier().to_string());

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, (spec, offset)) in stack.iter().enumerate() {
                let Some(layer) = spec.layer().upgrade() else {
                    continue;
                };
                let spec_path = spec.path();
                let specifier = spec.specifier();
                // Keep Token alive so we can borrow its text.
                let type_token = spec.type_name();
                draw_prim_spec_row(
                    ui,
                    stage,
                    i,
                    &layer,
                    spec_path,
                    offset,
                    specifier,
                    type_token.get_text(),
                    &root_id,
                    session_id.as_deref(),
                    state,
                );
            }
        });
}

// ---------------------------------------------------------------------------
// Root-stack row
// ---------------------------------------------------------------------------

fn draw_layer_row(
    ui: &mut Ui,
    stage: &Stage,
    idx: usize,
    layer: &Arc<Layer>,
    spec_path: Option<Path>,
    offset: &LayerOffset,
    state: &mut LayerStackPanelState,
) {
    let identifier = layer.identifier().to_string();
    let resolved_path = layer.get_resolved_path();
    let is_muted = stage.is_layer_muted(&identifier);

    // File size.
    let file_size = resolved_path
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len());

    // Offset display.
    let offset_str = if offset.is_identity() {
        String::new()
    } else {
        format!(" (offset={}, scale={})", offset.offset(), offset.scale())
    };

    // Size display.
    let size_str = file_size
        .map(|s| format!(" [{}]", formatting::fmt_size(s)))
        .unwrap_or_default();

    // Build label.
    let label_base = if let Some(ref p) = spec_path {
        format!("{}: {} | {}{}{}", idx, identifier, p, offset_str, size_str)
    } else {
        format!("{}: {}{}{}", idx, identifier, offset_str, size_str)
    };

    // Muted layers get strikethrough + dim color.
    let label = if is_muted {
        RichText::new(format!("{} (MUTED)", label_base))
            .color(Color32::from_rgb(120, 120, 120))
            .strikethrough()
    } else {
        RichText::new(&label_base).color(Color32::from_rgb(200, 200, 200))
    };

    let row_resp = ui.horizontal(|ui| {
        // Mute indicator badge.
        if is_muted {
            ui.label(
                RichText::new("M")
                    .color(Color32::from_rgb(200, 80, 80))
                    .strong()
                    .small(),
            )
            .on_hover_text("Layer is muted");
        }
        ui.add(egui::Label::new(label).sense(egui::Sense::click()));
    });

    // Context menu.
    row_resp.response.context_menu(|ui| {
        // Mute / Unmute
        if is_muted {
            if ui.button("Unmute Layer").clicked() {
                stage.unmute_layer(&identifier);
                state
                    .actions
                    .push(LayerStackAction::UnmuteLayer(identifier.clone()));
                ui.close();
            }
        } else if ui.button("Mute Layer").clicked() {
            stage.mute_layer(&identifier);
            state
                .actions
                .push(LayerStackAction::MuteLayer(identifier.clone()));
            ui.close();
        }

        ui.separator();

        // Open in editor / usdview.
        if let Some(ref path) = resolved_path {
            let size_info = file_size
                .map(|s| format!(" ({})", formatting::fmt_size(s)))
                .unwrap_or_default();
            if ui
                .button(format!("Open Layer In Editor{}", size_info))
                .clicked()
            {
                if let Err(e) = open_layer_in_editor(path) {
                    tracing::warn!("Open in editor: {}", e);
                }
                state
                    .actions
                    .push(LayerStackAction::OpenInEditor(path.clone()));
                ui.close();
            }

            if ui.button("Open Layer In usdview").clicked() {
                if let Err(e) = open_layer_in_usdview(path) {
                    tracing::warn!("Open in usdview: {}", e);
                }
                state
                    .actions
                    .push(LayerStackAction::OpenInUsdview(path.clone()));
                ui.close();
            }
        }

        ui.separator();

        // Copy operations.
        if ui.button("Copy Layer Identifier").clicked() {
            ui.ctx().copy_text(identifier.clone());
            state
                .actions
                .push(LayerStackAction::CopyText(identifier.clone()));
            ui.close();
        }

        if let Some(ref path) = resolved_path {
            if ui.button("Copy Layer Path").clicked() {
                ui.ctx().copy_text(path.clone());
                state.actions.push(LayerStackAction::CopyText(path.clone()));
                ui.close();
            }
        }

        if let Some(ref p) = spec_path {
            let ps = p.to_string();
            if ui.button("Copy Object Path").clicked() {
                ui.ctx().copy_text(ps.clone());
                state.actions.push(LayerStackAction::CopyText(ps));
                ui.close();
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Prim-stack spec row
// ---------------------------------------------------------------------------

/// Draws one spec entry in Prim Stack mode.
///
/// Badges: S (session, purple) / R (root, gold) / M (muted, red).
/// Label format: `{idx}: {layer_id} | {spec_path} <TypeName> [specifier{offset}] [size]`
#[allow(clippy::too_many_arguments)]
fn draw_prim_spec_row(
    ui: &mut Ui,
    stage: &Stage,
    idx: usize,
    layer: &Arc<Layer>,
    spec_path: Path,
    offset: &LayerOffset,
    specifier: Specifier,
    type_name: &str,
    root_id: &str,
    session_id: Option<&str>,
    state: &mut LayerStackPanelState,
) {
    let identifier = layer.identifier().to_string();
    let resolved_path = layer.get_resolved_path();
    let is_muted = stage.is_layer_muted(&identifier);

    // Determine layer role for badge + color.
    let is_session = session_id.map_or(false, |sid| identifier == sid);
    let is_root = !is_session && identifier == root_id;

    // File size.
    let file_size = resolved_path
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len());

    // Offset display.
    let offset_str = if offset.is_identity() {
        String::new()
    } else {
        format!(" offset={} scale={}", offset.offset(), offset.scale())
    };

    // Size display.
    let size_str = file_size
        .map(|s| format!(" [{}]", formatting::fmt_size(s)))
        .unwrap_or_default();

    // Specifier text.
    let spec_str = match specifier {
        Specifier::Def => "def",
        Specifier::Over => "over",
        Specifier::Class => "class",
    };

    // Type name suffix — omit when empty.
    let type_suffix = if type_name.is_empty() {
        String::new()
    } else {
        format!(" <{}>", type_name)
    };

    // Full row label.
    // Example: "0: session.usda | /World/Cube <Mesh> [over] [4.2 KB]"
    let label_base = format!(
        "{}: {} | {}{} [{}{}]{}{}",
        idx,
        identifier,
        spec_path,
        type_suffix,
        spec_str,
        offset_str,
        if is_muted { " (MUTED)" } else { "" },
        size_str,
    );

    // Row color: session=purple, root=gold, sublayer=light grey.
    let base_color = if is_session {
        Color32::from_rgb(180, 130, 220)
    } else if is_root {
        Color32::from_rgb(220, 185, 80)
    } else {
        Color32::from_rgb(200, 200, 200)
    };

    let label = if is_muted {
        RichText::new(&label_base)
            .color(Color32::from_rgb(110, 110, 110))
            .strikethrough()
    } else {
        RichText::new(&label_base).color(base_color)
    };

    let row_resp = ui.horizontal(|ui| {
        // Layer role badge.
        if is_session {
            ui.label(
                RichText::new("S")
                    .color(Color32::from_rgb(180, 130, 220))
                    .strong()
                    .small(),
            )
            .on_hover_text("Session layer");
        } else if is_root {
            ui.label(
                RichText::new("R")
                    .color(Color32::from_rgb(220, 185, 80))
                    .strong()
                    .small(),
            )
            .on_hover_text("Root layer");
        }
        // Mute badge.
        if is_muted {
            ui.label(
                RichText::new("M")
                    .color(Color32::from_rgb(200, 80, 80))
                    .strong()
                    .small(),
            )
            .on_hover_text("Layer is muted");
        }
        ui.add(egui::Label::new(label).sense(egui::Sense::click()));
    });

    // Context menu — same capabilities as root-stack rows.
    row_resp.response.context_menu(|ui| {
        if is_muted {
            if ui.button("Unmute Layer").clicked() {
                stage.unmute_layer(&identifier);
                state
                    .actions
                    .push(LayerStackAction::UnmuteLayer(identifier.clone()));
                ui.close();
            }
        } else if ui.button("Mute Layer").clicked() {
            stage.mute_layer(&identifier);
            state
                .actions
                .push(LayerStackAction::MuteLayer(identifier.clone()));
            ui.close();
        }

        ui.separator();

        if let Some(ref path) = resolved_path {
            let size_info = file_size
                .map(|s| format!(" ({})", formatting::fmt_size(s)))
                .unwrap_or_default();
            if ui
                .button(format!("Open Layer In Editor{}", size_info))
                .clicked()
            {
                if let Err(e) = open_layer_in_editor(path) {
                    tracing::warn!("Open in editor: {}", e);
                }
                state
                    .actions
                    .push(LayerStackAction::OpenInEditor(path.clone()));
                ui.close();
            }
            if ui.button("Open Layer In usdview").clicked() {
                if let Err(e) = open_layer_in_usdview(path) {
                    tracing::warn!("Open in usdview: {}", e);
                }
                state
                    .actions
                    .push(LayerStackAction::OpenInUsdview(path.clone()));
                ui.close();
            }
        }

        ui.separator();

        if ui.button("Copy Layer Identifier").clicked() {
            ui.ctx().copy_text(identifier.clone());
            state
                .actions
                .push(LayerStackAction::CopyText(identifier.clone()));
            ui.close();
        }
        if let Some(ref path) = resolved_path {
            if ui.button("Copy Layer Path").clicked() {
                ui.ctx().copy_text(path.clone());
                state.actions.push(LayerStackAction::CopyText(path.clone()));
                ui.close();
            }
        }
        let ps = spec_path.to_string();
        if ui.button("Copy Object Path").clicked() {
            ui.ctx().copy_text(ps.clone());
            state.actions.push(LayerStackAction::CopyText(ps));
            ui.close();
        }
    });
}

// ---------------------------------------------------------------------------
// External editor / usdview launch
// ---------------------------------------------------------------------------

/// Opens layer file in editor.
/// Priority: usdedit -> USD_EDITOR -> EDITOR -> VISUAL -> platform default.
fn open_layer_in_editor(layer_path: &str) -> Result<(), String> {
    let path = std::path::PathBuf::from(layer_path);
    if !path.exists() {
        return Err(format!("Layer file not found: {}", layer_path));
    }

    // Try usdedit first.
    if cmd_exists("usdedit") {
        Command::new("usdedit")
            .args(["-n", layer_path])
            .spawn()
            .map_err(|e| format!("Failed to spawn usdedit: {}", e))?;
        return Ok(());
    }

    // Try environment variable editors.
    let editor = std::env::var("USD_EDITOR")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok());

    if let Some(editor) = editor {
        let exe = editor.split_whitespace().next().unwrap_or(&editor);
        if cmd_exists(exe) {
            Command::new(exe)
                .arg(layer_path)
                .spawn()
                .map_err(|e| format!("Failed to spawn {}: {}", exe, e))?;
            return Ok(());
        }
    }

    // Fall back to platform default.
    open_platform_default(layer_path)
}

/// Opens a layer file in a new usdview (this executable) instance.
fn open_layer_in_usdview(layer_path: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot get executable path: {}", e))?;
    Command::new(&exe)
        .arg(layer_path)
        .spawn()
        .map_err(|e| format!("Failed to spawn usdview: {}", e))?;
    Ok(())
}

/// Returns true if `cmd` exists on PATH.
fn cmd_exists(cmd: &str) -> bool {
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Opens a file with the OS default application.
fn open_platform_default(path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("No method to open file on this platform.".to_string())
}
