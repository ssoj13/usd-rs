//! USD Validation panel — P1-12.
//!
//! Runs registered validators against the current stage and displays results
//! in a filterable table. Clicking a result navigates to the offending prim.
//!
//! Reference: `validationWidget.py` (Pixar), simplified for egui (no Qt splitters,
//! no drag-and-drop validator selection — just a toolbar + results table).

use std::sync::{Arc, Mutex};

use usd_core::PrimRange;
use usd_core::prim::Prim;
use usd_core::stage::Stage;
use usd_sdf::Path;
use usd_tf::Token;
use usd_validation::{
    ErrorType, ValidationContext, ValidationError, ValidationRegistry, ValidationTimeRange,
};

// ============================================================================
// Data types
// ============================================================================

/// A single flattened row in the results table.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub error_type: ErrorType,
    /// Short machine-readable error name.
    pub error_name: String,
    /// Name of the validator that produced this result.
    pub validator_name: String,
    /// Comma-separated list of prim/property/layer paths for display.
    pub sites: String,
    /// Human-readable message.
    pub message: String,
    /// First prim path found in the sites (for navigation on double-click).
    pub nav_path: Option<String>,
}

impl ValidationResult {
    fn from_error(err: &ValidationError) -> Self {
        let mut site_strs: Vec<String> = Vec::new();
        let mut nav_path: Option<String> = None;

        for site in err.get_sites() {
            let path = site.get_path().to_string();
            if nav_path.is_none() && site.is_prim() {
                nav_path = Some(path.clone());
            }
            site_strs.push(path);
        }

        Self {
            error_type: err.get_type(),
            error_name: err.get_name().as_str().to_string(),
            validator_name: err.get_validator_name().as_str().to_string(),
            sites: site_strs.join(", "),
            message: err.get_message().to_string(),
            nav_path,
        }
    }

    /// True if the row matches the text filter (any column).
    fn matches_filter(&self, lower: &str) -> bool {
        if lower.is_empty() {
            return true;
        }
        self.validator_name.to_lowercase().contains(lower)
            || self.error_name.to_lowercase().contains(lower)
            || self.sites.to_lowercase().contains(lower)
            || self.message.to_lowercase().contains(lower)
    }
}

// ============================================================================
// Panel state
// ============================================================================

/// State for the USD Validation floating window.
#[derive(Debug, Default)]
pub struct ValidationPanelState {
    pub open: bool,
    /// All validation results from the last run.
    results: Vec<ValidationResult>,
    /// Text filter applied to all columns.
    filter_text: String,
    /// Show Error-severity rows.
    show_errors: bool,
    /// Show Warn-severity rows.
    show_warnings: bool,
    /// Show Info-severity rows.
    show_info: bool,
    /// Currently highlighted row index (after click).
    selected_row: Option<usize>,
    /// True while validation is running in background thread.
    running: bool,
    /// Shared channel for receiving async validation results.
    async_results: Arc<Mutex<Option<Vec<ValidationResult>>>>,
    /// Prim path navigation request produced by double-click.
    pub navigate_to: Option<String>,
    /// Validate prims-from-selection (vs whole stage).
    /// Ref: validationWidget.py:266 primsSetFromSelection
    pub validate_selection: bool,
    /// Include descendant prims when validating from selection.
    /// Ref: validationWidget.py:276 includeDescendantPrims
    pub include_descendants: bool,
    /// P1-18: Available validators with enabled state.
    /// Lazily populated from ValidationRegistry on first open.
    validator_list: Vec<(Token, bool)>,
    /// Whether validator_list has been initialized.
    validators_loaded: bool,
    /// Show validator picker collapsible.
    show_validator_picker: bool,
}

impl ValidationPanelState {
    pub fn new() -> Self {
        Self {
            show_errors: true,
            show_warnings: true,
            show_info: true,
            ..Default::default()
        }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    /// P1-17: Run validators in background thread.
    /// `selected_paths` -- current prim selection (used when validate_selection is on).
    pub fn run_validation(&mut self, stage: &Arc<Stage>, selected_paths: &[Path]) {
        if self.running {
            return; // already running
        }
        self.running = true;
        self.results.clear();
        self.selected_row = None;
        self.navigate_to = None;

        // P1-18: Build context from selected validators only
        self.ensure_validators();
        let enabled_names: Vec<Token> = self
            .validator_list
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name.clone())
            .collect();
        let all_enabled = enabled_names.len() == self.validator_list.len();

        // Capture state for the background thread
        let stage = Arc::clone(stage);
        let sel_paths: Vec<Path> = selected_paths.to_vec();
        let validate_sel = self.validate_selection;
        let include_desc = self.include_descendants;
        let results_slot = Arc::clone(&self.async_results);

        std::thread::spawn(move || {
            let ctx = if all_enabled {
                ValidationContext::all()
            } else {
                ValidationContext::from_names(&enabled_names)
            };
            let time_range = ValidationTimeRange::default();

            let errors = if validate_sel && !sel_paths.is_empty() {
                let has_root = sel_paths.iter().any(|p| p.is_absolute_root_path());
                if has_root {
                    ctx.validate_all(&stage, &time_range)
                } else {
                    let mut prims: Vec<Prim> = Vec::new();
                    for path in &sel_paths {
                        if let Some(prim) = stage.get_prim_at_path(path) {
                            if include_desc {
                                for p in PrimRange::from_prim(&prim) {
                                    prims.push(p);
                                }
                            } else {
                                prims.push(prim);
                            }
                        }
                    }
                    ctx.validate_prims(&prims, &time_range)
                }
            } else {
                ctx.validate_all(&stage, &time_range)
            };

            let rows: Vec<ValidationResult> = errors
                .iter()
                .filter(|e| e.get_type() != ErrorType::None)
                .map(ValidationResult::from_error)
                .collect();

            // Deliver results
            if let Ok(mut slot) = results_slot.lock() {
                *slot = Some(rows);
            }
        });
    }

    /// Poll for async results. Call every frame.
    fn poll_results(&mut self) {
        if !self.running {
            return;
        }
        if let Ok(mut slot) = self.async_results.lock() {
            if let Some(rows) = slot.take() {
                self.results = rows;
                self.running = false;
            }
        }
    }

    // -- Counts --

    fn count_errors(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.error_type == ErrorType::Error)
            .count()
    }

    fn count_warnings(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.error_type == ErrorType::Warn)
            .count()
    }

    fn count_info(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.error_type == ErrorType::Info)
            .count()
    }

    /// Lazily load available validator names from the registry.
    fn ensure_validators(&mut self) {
        if self.validators_loaded {
            return;
        }
        let reg = ValidationRegistry::get_instance();
        let mut names: Vec<Token> = reg.get_all_validator_names();
        names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        self.validator_list = names.into_iter().map(|n| (n, true)).collect();
        self.validators_loaded = true;
    }
}

// ============================================================================
// UI colours
// ============================================================================

fn color_for(et: ErrorType) -> egui::Color32 {
    match et {
        ErrorType::Error => egui::Color32::from_rgb(220, 53, 69),
        ErrorType::Warn => egui::Color32::from_rgb(218, 165, 32),
        ErrorType::Info => egui::Color32::from_rgb(23, 162, 184),
        ErrorType::None => egui::Color32::from_rgb(108, 117, 125),
    }
}

fn label_for(et: ErrorType) -> &'static str {
    match et {
        ErrorType::Error => "ERR",
        ErrorType::Warn => "WARN",
        ErrorType::Info => "INFO",
        ErrorType::None => "-",
    }
}

// ============================================================================
// Draw function
// ============================================================================

/// Draw the validation panel window. Call every frame.
///
/// `stage` — current stage (None = no stage loaded, Run button is disabled).
///
/// Returns the prim path the user double-clicked (for navigation), if any.
pub fn ui_validation(
    ctx: &egui::Context,
    state: &mut ValidationPanelState,
    stage: Option<&Arc<Stage>>,
    selected_paths: &[Path],
) -> Option<String> {
    if !state.open {
        return None;
    }

    // P1-17: Poll for async validation results each frame
    state.poll_results();

    let mut nav_request: Option<String> = None;

    // Extract `open` to avoid conflicting borrows (`&mut state.open` vs `state` in closure).
    let mut open = state.open;
    egui::Window::new("USD Validation")
        .resizable(true)
        .default_size([860.0, 520.0])
        .min_size([480.0, 240.0])
        .collapsible(true)
        .open(&mut open)
        .show(ctx, |ui| {
            // ---- Toolbar ----
            ui.horizontal(|ui| {
                let run_label = if state.running {
                    "Running..."
                } else {
                    "Run Validation"
                };
                let can_run = stage.is_some() && !state.running;
                if ui
                    .add_enabled(can_run, egui::Button::new(run_label))
                    .clicked()
                {
                    if let Some(s) = stage {
                        state.run_validation(s, selected_paths);
                    }
                }

                ui.separator();

                // P2-22: Validate selection only (ref: validationWidget.py:266-282)
                ui.checkbox(&mut state.validate_selection, "Selection Only");
                ui.add_enabled(
                    state.validate_selection,
                    egui::Checkbox::new(&mut state.include_descendants, "+ Descendants"),
                );

                ui.separator();

                // P1-18: Validator picker toggle
                state.ensure_validators();
                let total_v = state.validator_list.len();
                let enabled_v = state.validator_list.iter().filter(|(_, on)| *on).count();
                let v_label = format!("Validators ({}/{})", enabled_v, total_v);
                if ui
                    .selectable_label(state.show_validator_picker, &v_label)
                    .clicked()
                {
                    state.show_validator_picker = !state.show_validator_picker;
                }

                ui.separator();
                ui.label("Filter:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.filter_text)
                        .desired_width(180.0)
                        .hint_text("Search..."),
                );
                if ui.small_button("x").clicked() {
                    state.filter_text.clear();
                }

                ui.separator();

                // Severity toggles
                let err_count = state.count_errors();
                let warn_count = state.count_warnings();
                let info_count = state.count_info();

                toggle_button(
                    ui,
                    &mut state.show_errors,
                    &format!("ERR ({})", err_count),
                    egui::Color32::from_rgb(220, 53, 69),
                );
                toggle_button(
                    ui,
                    &mut state.show_warnings,
                    &format!("WARN ({})", warn_count),
                    egui::Color32::from_rgb(218, 165, 32),
                );
                toggle_button(
                    ui,
                    &mut state.show_info,
                    &format!("INFO ({})", info_count),
                    egui::Color32::from_rgb(23, 162, 184),
                );

                // Right-align summary
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let total = state.results.len();
                    ui.label(egui::RichText::new(format!("{} results", total)).weak());
                });
            });

            // P1-18: Validator picker collapsible section
            if state.show_validator_picker {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("All").clicked() {
                            for (_, on) in state.validator_list.iter_mut() {
                                *on = true;
                            }
                        }
                        if ui.small_button("None").clicked() {
                            for (_, on) in state.validator_list.iter_mut() {
                                *on = false;
                            }
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("validator_picker")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for (name, enabled) in state.validator_list.iter_mut() {
                                ui.checkbox(enabled, name.as_str());
                            }
                        });
                });
            }

            ui.separator();

            // ---- Results table ----
            if state.results.is_empty() {
                if state.running {
                    ui.centered_and_justified(|ui| {
                        ui.label("Running validation...");
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("No validation results. Click \"Run Validation\".")
                                .weak(),
                        );
                    });
                }
                return;
            }

            let filter_lower = state.filter_text.to_lowercase();

            // Collect visible rows (indices into state.results).
            let visible: Vec<usize> = state
                .results
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    let type_ok = match r.error_type {
                        ErrorType::Error => state.show_errors,
                        ErrorType::Warn => state.show_warnings,
                        ErrorType::Info => state.show_info,
                        ErrorType::None => false,
                    };
                    type_ok && r.matches_filter(&filter_lower)
                })
                .map(|(i, _)| i)
                .collect();

            if visible.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No results match the current filter.").weak());
                });
                return;
            }

            // Header row
            egui::Grid::new("validation_header")
                .num_columns(5)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Type").strong());
                    ui.label(egui::RichText::new("Validator").strong());
                    ui.label(egui::RichText::new("Error").strong());
                    ui.label(egui::RichText::new("Sites").strong());
                    ui.label(egui::RichText::new("Message").strong());
                    ui.end_row();
                });

            ui.separator();

            // Scrollable body
            egui::ScrollArea::vertical()
                .id_salt("validation_results")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("validation_body")
                        .num_columns(5)
                        .min_col_width(60.0)
                        .striped(true)
                        .show(ui, |ui| {
                            for &idx in &visible {
                                let row = &state.results[idx];
                                let is_selected = state.selected_row == Some(idx);
                                let col = color_for(row.error_type);

                                // Type badge
                                ui.label(
                                    egui::RichText::new(label_for(row.error_type))
                                        .color(col)
                                        .strong()
                                        .monospace(),
                                );

                                // Validator name
                                let v_text = if row.validator_name.is_empty() {
                                    "-".to_string()
                                } else {
                                    row.validator_name.clone()
                                };

                                // Selectable row spans (egui grid): make the validator
                                // cell the "click target" and treat double-click specially.
                                let resp = ui.selectable_label(is_selected, &v_text);
                                if resp.clicked() {
                                    state.selected_row = Some(idx);
                                }
                                if resp.double_clicked() {
                                    if let Some(ref path) = row.nav_path {
                                        nav_request = Some(path.clone());
                                    }
                                }

                                // Error name
                                ui.label(&row.error_name);

                                // Sites (truncate for display)
                                let sites_display = if row.sites.len() > 60 {
                                    format!("{}...", &row.sites[..57])
                                } else {
                                    row.sites.clone()
                                };
                                let site_resp = ui.add(egui::Label::new(&sites_display).truncate());
                                if !row.sites.is_empty() {
                                    site_resp.on_hover_text(&row.sites);
                                }

                                // Message (truncate, show full on hover)
                                let msg_short = if row.message.len() > 80 {
                                    format!("{}...", &row.message[..77])
                                } else {
                                    row.message.clone()
                                };
                                let msg_resp = ui.add(egui::Label::new(&msg_short).truncate());
                                msg_resp.on_hover_text(&row.message);

                                ui.end_row();
                            }
                        });
                });
        });
    state.open = open;

    // Propagate navigation request produced during the frame.
    if let Some(ref path) = state.navigate_to.take() {
        nav_request = Some(path.clone());
    }

    nav_request
}

// ============================================================================
// Helper: coloured toggle button
// ============================================================================

/// A small toggle button that changes background colour when active.
fn toggle_button(ui: &mut egui::Ui, active: &mut bool, label: &str, active_color: egui::Color32) {
    let style = ui.style();
    let bg = if *active {
        active_color
    } else {
        style.visuals.widgets.inactive.bg_fill
    };
    let text_color = if *active {
        egui::Color32::WHITE
    } else {
        style.visuals.text_color()
    };

    let btn = egui::Button::new(egui::RichText::new(label).color(text_color).small())
        .fill(bg)
        .small();

    if ui.add(btn).clicked() {
        *active = !*active;
    }
}
