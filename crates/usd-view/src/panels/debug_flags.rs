//! TF_DEBUG flags dialog.
//!
//! Lists all registered debug symbols as checkboxes. Toggling a checkbox
//! enables/disables the corresponding TfDebug symbol at runtime.

/// State for the TF_DEBUG flags dialog.
#[derive(Debug, Default)]
pub struct DebugFlagsState {
    pub open: bool,
    /// Filter text for narrowing the symbol list.
    filter: String,
    /// Cached sorted symbol names (refreshed on open).
    cached_names: Vec<String>,
    /// Cached prefix groups: (prefix, count) sorted.
    cached_prefixes: Vec<(String, usize)>,
    /// Currently selected prefix filter (empty = show all).
    selected_prefix: String,
}

impl DebugFlagsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.open = true;
        self.refresh();
    }

    fn refresh(&mut self) {
        let mut names = usd_tf::debug::Debug::get_symbol_names();
        names.sort();
        // Build prefix groups (e.g. "HD", "SDF", "USD", "TF")
        let mut prefix_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for name in &names {
            let prefix = extract_prefix(name);
            *prefix_counts.entry(prefix).or_default() += 1;
        }
        self.cached_prefixes = prefix_counts.into_iter().collect();
        self.cached_names = names;
    }
}

/// Extract prefix from debug symbol name (e.g. "HD_RENDER" -> "HD").
fn extract_prefix(name: &str) -> String {
    // Split on '_' and take all uppercase segments as prefix.
    // e.g. "HD_RENDER_DRAW" -> "HD", "SDF_LAYER" -> "SDF"
    if let Some(idx) = name.find('_') {
        name[..idx].to_string()
    } else {
        name.to_string()
    }
}

/// Draw the TF_DEBUG flags window. Call every frame from `update()`.
pub fn ui_debug_flags(ctx: &egui::Context, state: &mut DebugFlagsState) {
    if !state.open {
        return;
    }

    let mut close = false;

    egui::Window::new("TF_DEBUG Flags")
        .resizable(true)
        .default_size([600.0, 500.0])
        .min_size([400.0, 200.0])
        .collapsible(true)
        .show(ctx, |ui| {
            // Top: filter + bulk controls
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut state.filter);
                if ui.button("Refresh").clicked() {
                    state.refresh();
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Enable All").clicked() {
                    usd_tf::debug::Debug::enable_all();
                }
                if ui.button("Disable All").clicked() {
                    usd_tf::debug::Debug::disable_all();
                }
            });

            ui.separator();

            // Two-pane layout: left = prefix groups, right = flags.
            // Ref: debugFlagsWidget.py (left prefix list, right flags table).
            let filter_lower = state.filter.to_lowercase();

            ui.columns(2, |cols| {
                // Left pane: prefix group list
                cols[0].vertical(|ui| {
                    ui.label(egui::RichText::new("Groups").strong());
                    egui::ScrollArea::vertical()
                        .id_salt("debug_prefix_list")
                        .auto_shrink([false, false])
                        .max_height(ui.available_height() - 30.0)
                        .show(ui, |ui| {
                            // "All" entry
                            let all_selected = state.selected_prefix.is_empty();
                            if ui
                                .selectable_label(
                                    all_selected,
                                    format!("All ({})", state.cached_names.len()),
                                )
                                .clicked()
                            {
                                state.selected_prefix.clear();
                            }
                            ui.separator();
                            for (prefix, count) in &state.cached_prefixes {
                                let selected = state.selected_prefix == *prefix;
                                if ui
                                    .selectable_label(selected, format!("{} ({})", prefix, count))
                                    .clicked()
                                {
                                    state.selected_prefix = prefix.clone();
                                }
                            }
                        });
                });

                // Right pane: filtered flags
                cols[1].vertical(|ui| {
                    let filtered: Vec<&String> = state
                        .cached_names
                        .iter()
                        .filter(|n| {
                            let matches_filter =
                                filter_lower.is_empty() || n.to_lowercase().contains(&filter_lower);
                            let matches_prefix = state.selected_prefix.is_empty()
                                || extract_prefix(n) == state.selected_prefix;
                            matches_filter && matches_prefix
                        })
                        .collect();

                    ui.label(format!(
                        "{} / {} symbols",
                        filtered.len(),
                        state.cached_names.len()
                    ));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("debug_flags_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for name in &filtered {
                                let mut enabled = usd_tf::debug::Debug::is_enabled(name);
                                let desc = usd_tf::debug::Debug::get_symbol_description(name)
                                    .unwrap_or_default();

                                let resp = ui.checkbox(&mut enabled, *name);
                                if resp.changed() {
                                    if enabled {
                                        usd_tf::debug::Debug::enable(name);
                                    } else {
                                        usd_tf::debug::Debug::disable(name);
                                    }
                                }
                                if !desc.is_empty() {
                                    resp.on_hover_text(&desc);
                                }
                            }
                        });
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });

    if close {
        state.open = false;
    }
}
