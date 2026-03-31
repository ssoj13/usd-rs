//! Toolbar, theme toggle, and dialog drawing for the viewer.

use usd_sdf::TimeCode;

use crate::menus::draw_menu_bar;
use crate::panels::preferences::UiTheme;

use usd_shade::tokens as usd_shade_tokens;

use super::ViewerApp;

impl ViewerApp {
    pub(crate) fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        self.apply_view_settings_to_menu_state();

        // Full menu bar
        let menu_actions = draw_menu_bar(
            ui,
            &mut self.menu_state,
            &self.recent_files,
            &mut self.data_model.view.complexity,
            &mut self.data_model.view.clear_color,
            &mut self.data_model.view.highlight_color,
            &mut self.data_model.view.sel_highlight_mode,
            &mut self.data_model.view.camera_mask_mode,
        );

        // Keep viewport behavior in sync with menu state.
        self.apply_menu_state_to_view_settings();

        // Dispatch menu-triggered app actions
        let actions: Vec<_> = menu_actions.actions.clone();
        for action in &actions {
            self.dispatch_action(action, ui.ctx());
        }
        if let Some(path) = menu_actions.open_recent {
            self.load_file(&path);
        }
        if menu_actions.reopen_stage {
            if let Some(path) = self.data_model.root.file_path.clone() {
                self.load_file(&path);
            }
        }
        if menu_actions.open_preferences {
            self.prefs_state.open();
        }
        if menu_actions.select_bound_preview_material {
            let paths = self.data_model.selection.get_paths().to_vec();
            self.select_bound_material_for_paths(&paths, &usd_shade_tokens::tokens().preview);
        }
        if menu_actions.select_bound_full_material {
            let paths = self.data_model.selection.get_paths().to_vec();
            self.select_bound_material_for_paths(&paths, &usd_shade_tokens::tokens().full);
        }
        if menu_actions.select_preview_binding_rel {
            let paths = self.data_model.selection.get_paths().to_vec();
            self.select_binding_rel_for_paths(&paths, &usd_shade_tokens::tokens().preview);
        }
        if menu_actions.select_full_binding_rel {
            let paths = self.data_model.selection.get_paths().to_vec();
            self.select_binding_rel_for_paths(&paths, &usd_shade_tokens::tokens().full);
        }
        if menu_actions.reset_layout {
            self.dock_state = if self.config.no_render {
                crate::dock::default_dock_state_no_render()
            } else {
                crate::dock::default_dock_state()
            };
            self.saved_dock_state = None;
            self.current_layout = None;
            self.layout_delete_armed = false;
        }
        if menu_actions.save_overrides {
            self.save_overrides_dialog();
        }
        if menu_actions.save_flattened {
            self.save_flattened_dialog();
        }
        if menu_actions.save_image {
            self.save_image_dialog();
        }
        if menu_actions.copy_image {
            self.copy_image_to_clipboard(ui.ctx());
        }
        if menu_actions.expand_all {
            self.expand_all_prims();
        }
        if menu_actions.collapse_all {
            self.collapse_all_prims();
        }
        if let Some(depth) = menu_actions.expand_to_depth {
            self.expand_to_depth(depth);
        }
        if menu_actions.adjust_free_camera {
            self.free_camera_dialog_open = true;
        }
        if menu_actions.adjust_default_material {
            self.default_material_dialog_open = true;
        }
        if menu_actions.load_hdri {
            self.load_hdri_dialog();
        }
        if menu_actions.open_debug_flags {
            self.debug_flags_state.open();
        }
        if menu_actions.open_validation {
            self.validation_state.open();
        }
        if menu_actions.toggle_debug_logging {
            // Toggle log level between Info and Debug
            let new_level = if self.menu_state.debug_logging {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            };
            log::set_max_level(new_level);
            tracing::info!(
                "Debug logging: {}",
                if self.menu_state.debug_logging {
                    "ON"
                } else {
                    "OFF"
                }
            );
        }
        // Sync render stats overlay -> HUD GPU stats toggle
        if self.menu_state.show_render_stats_overlay {
            self.menu_state.show_hud_gpu_stats = true;
        }

        // Playback toolbar row: always shown when a stage is loaded.
        // For static scenes (no time range), controls are disabled but visible
        // — C++ _setPlaybackAvailability disables, not hides.
        if !self.config.no_render && self.data_model.root.stage.is_some() {
            // C++ _setPlaybackAvailability: enabled = len(timeSamples) > 1
            let playback_enabled = self.data_model.root.playback_available();
            // C++ line 2084: if disabled but playing, stop playback
            if !playback_enabled && self.playback.is_playing() {
                self.playback.stop();
            }
            // Sync step size from view settings
            self.playback.set_step_size(self.data_model.view.step_size);
            ui.horizontal(|ui| {
                if !playback_enabled {
                    ui.disable();
                }
                // Reverse play button
                let rev_label = if self.playback.is_playing() && self.playback.is_reverse() {
                    "[Rev]"
                } else {
                    "Rev"
                };
                if ui
                    .button(rev_label)
                    .on_hover_text("Reverse playback (Shift+Space)")
                    .clicked()
                {
                    self.playback.toggle_reverse();
                    ui.ctx().request_repaint();
                }

                // Play/Stop toggle
                let play_text = if self.playback.is_playing() && !self.playback.is_reverse() {
                    "[Play]"
                } else {
                    "Play"
                };
                if ui.button(play_text).clicked() {
                    self.playback.toggle_play();
                    ui.ctx().request_repaint();
                }

                // Frame slider.
                //
                // Match usdview tracking semantics:
                // - with Redraw On Scrub enabled, stage time updates continuously;
                // - otherwise the slider still moves, but the expensive stage-time
                //   update is deferred until release.
                let start = self.data_model.root.frame_range_start();
                let end = self.data_model.root.frame_range_end();
                let mut time_value = self.playback.current_frame();
                ui.label("Frame:");
                let slider = egui::Slider::new(&mut time_value, start..=end);
                let resp = ui.add(slider);
                if resp.drag_started() {
                    self.playback.scrub_start();
                }
                if resp.changed() {
                    // Snap to nearest authored time sample if available
                    let snapped = crate::data_model::snap_to_nearest(
                        time_value,
                        &self.data_model.root.stage_time_samples,
                    );
                    self.playback.set_frame(snapped);
                    if self.data_model.view.redraw_on_scrub || !self.playback.is_scrubbing() {
                        self.data_model.root.current_time = TimeCode::new(snapped);
                        ui.ctx().request_repaint();
                    }
                }
                if resp.drag_stopped() {
                    self.playback.scrub_end();
                    if !self.data_model.view.redraw_on_scrub {
                        self.data_model.root.current_time =
                            TimeCode::new(self.playback.current_frame());
                        ui.ctx().request_repaint();
                    }
                }

                // Editable current frame field (per Python frameField)
                let mut frame_edit = time_value;
                let dv = egui::DragValue::new(&mut frame_edit)
                    .range(start..=end)
                    .speed(0.5)
                    .fixed_decimals(0);
                if ui.add(dv).changed() {
                    self.playback.set_frame(frame_edit);
                    self.data_model.root.current_time = TimeCode::new(frame_edit);
                }

                if ui.button("<<").clicked() {
                    let (start, _) = self.playback.frame_range();
                    self.playback.set_frame(start);
                    self.data_model.root.current_time =
                        TimeCode::new(self.playback.current_frame());
                }
                if ui.button("<").clicked() {
                    let step = self.playback.step_size();
                    self.playback.step_backward(step);
                    self.data_model.root.current_time =
                        TimeCode::new(self.playback.current_frame());
                }
                if ui.button(">").clicked() {
                    let step = self.playback.step_size();
                    self.playback.step_forward(step);
                    self.data_model.root.current_time =
                        TimeCode::new(self.playback.current_frame());
                }
                if ui.button(">>").clicked() {
                    let (_, end) = self.playback.frame_range();
                    self.playback.set_frame(end);
                    self.data_model.root.current_time =
                        TimeCode::new(self.playback.current_frame());
                }

                // Loop toggle
                let loop_text = if self.playback.is_looping() {
                    "[Loop]"
                } else {
                    "Loop"
                };
                if ui.button(loop_text).clicked() {
                    self.playback.toggle_looping();
                }

                // Editable playback sub-range (range_begin / range_end).
                // When set, these restrict the slider to a sub-range of the stage.
                // Matches the C++ usdview rangeBegin/rangeEnd toolbar fields.
                if self.data_model.root.has_frame_range() {
                    let stage_start = self.data_model.root.stage_start();
                    let stage_end = self.data_model.root.stage_end();

                    ui.separator();

                    // range_begin DragValue — falls back to stage start when None
                    let mut begin_val = self.data_model.root.range_begin.unwrap_or(stage_start);
                    let begin_changed = ui
                        .add(
                            egui::DragValue::new(&mut begin_val)
                                .speed(1.0)
                                .range(stage_start..=stage_end)
                                .prefix("Range: "),
                        )
                        .changed();
                    if begin_changed {
                        // Only store as override when it differs from the stage default
                        self.data_model.root.range_begin = if begin_val == stage_start {
                            None
                        } else {
                            Some(begin_val)
                        };
                        // Clamp range_end so it stays >= range_begin
                        if let Some(end) = self.data_model.root.range_end {
                            if end < begin_val {
                                self.data_model.root.range_end = Some(begin_val);
                            }
                        }
                        self.rebuild_timeline_from_view_settings();
                    }

                    ui.label("..");

                    // range_end DragValue — falls back to stage end when None
                    let mut end_val = self.data_model.root.range_end.unwrap_or(stage_end);
                    let end_changed = ui
                        .add(
                            egui::DragValue::new(&mut end_val)
                                .speed(1.0)
                                .range(stage_start..=stage_end)
                                .suffix(" ]"),
                        )
                        .changed();
                    if end_changed {
                        self.data_model.root.range_end = if end_val == stage_end {
                            None
                        } else {
                            Some(end_val)
                        };
                        // Clamp range_begin so it stays <= range_end
                        if let Some(begin) = self.data_model.root.range_begin {
                            if begin > end_val {
                                self.data_model.root.range_begin = Some(end_val);
                            }
                        }
                        self.rebuild_timeline_from_view_settings();
                    }

                    // Reset button — clears both overrides back to full stage range
                    let has_override = self.data_model.root.range_begin.is_some()
                        || self.data_model.root.range_end.is_some();
                    if has_override
                        && ui
                            .small_button("x")
                            .on_hover_text("Reset to full stage range")
                            .clicked()
                    {
                        self.data_model.root.range_begin = None;
                        self.data_model.root.range_end = None;
                        self.rebuild_timeline_from_view_settings();
                    }

                    // Static stage extents label for reference
                    ui.label(format!("(stage: {:.0}..{:.0})", stage_start, stage_end));
                }
            });
        }
    }

    pub(crate) fn apply_theme_preference(&self, ctx: &egui::Context) {
        let want_dark = matches!(self.prefs_state.settings.ui_theme, UiTheme::Dark);
        let have_dark = ctx.style().visuals.dark_mode;
        if want_dark != have_dark {
            if want_dark {
                ctx.set_visuals(egui::Visuals::dark());
            } else {
                ctx.set_visuals(egui::Visuals::light());
            }
        }
    }

    pub(crate) fn draw_theme_toggle(&mut self, ctx: &egui::Context) {
        egui::Area::new("theme_toggle_area".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    self.draw_layout_selector(ui);
                    ui.add_space(4.0);
                    // Theme toggle button
                    let dark_mode = ctx.style().visuals.dark_mode;
                    let label = if dark_mode { "Light mode" } else { "Dark mode" };
                    if ui.small_button(label).clicked() {
                        if dark_mode {
                            self.prefs_state.settings.ui_theme = UiTheme::Light;
                            ctx.set_visuals(egui::Visuals::light());
                        } else {
                            self.prefs_state.settings.ui_theme = UiTheme::Dark;
                            ctx.set_visuals(egui::Visuals::dark());
                        }
                    }
                });
            });
    }

    /// Layout selector: text input for new name + ComboBox + delete button.
    fn draw_layout_selector(&mut self, ui: &mut egui::Ui) {
        let current_ron = ron::to_string(&self.dock_state).unwrap_or_default();

        // Text input: type new name + Enter to save
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.layout_name_input)
                .desired_width(80.0)
                .hint_text("New layout...")
                .font(egui::TextStyle::Small),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let name = self.layout_name_input.trim().to_string();
            if !name.is_empty() {
                self.layouts.insert(name.clone(), current_ron.clone());
                self.current_layout = Some(name);
                self.layout_name_input.clear();
                self.layout_delete_armed = false;
            }
        }

        // ComboBox showing saved layouts
        let display = self
            .current_layout
            .as_deref()
            .unwrap_or("Layouts")
            .to_string();

        let mut layout_to_apply: Option<String> = None;

        egui::ComboBox::from_id_salt("layout_selector")
            .selected_text(&display)
            .width(100.0)
            .show_ui(ui, |ui| {
                let mut names: Vec<String> = self.layouts.keys().cloned().collect();
                names.sort();
                for name in &names {
                    let selected = self.current_layout.as_deref() == Some(name.as_str());
                    if ui.selectable_label(selected, name).clicked() {
                        layout_to_apply = Some(name.clone());
                    }
                }
            });

        // Apply selected layout
        if let Some(name) = layout_to_apply {
            if let Some(ron_str) = self.layouts.get(&name).cloned() {
                if let Ok(restored) =
                    ron::from_str::<egui_dock::DockState<crate::dock::DockTab>>(&ron_str)
                {
                    self.dock_state = restored;
                    self.saved_dock_state = None;
                }
            }
            self.current_layout = Some(name);
            self.layout_delete_armed = false;
        }

        // Delete button: first click arms (red), second deletes
        if self.current_layout.is_some() {
            let armed = self.layout_delete_armed;
            let btn_text = "x";
            let btn = if armed {
                egui::Button::new(egui::RichText::new(btn_text).color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(200, 50, 50))
            } else {
                egui::Button::new(btn_text)
            };
            let del_resp = ui.add(btn);
            let tooltip = if armed {
                "Click again to confirm delete"
            } else {
                "Delete current layout"
            };
            del_resp.clone().on_hover_text(tooltip);
            if del_resp.clicked() {
                if armed {
                    // Confirmed: delete layout
                    if let Some(name) = self.current_layout.take() {
                        self.layouts.remove(&name);
                    }
                    self.layout_delete_armed = false;
                } else {
                    self.layout_delete_armed = true;
                }
            }
            // Disarm if clicked elsewhere
            if armed && !del_resp.hovered() && ui.input(|i| i.pointer.any_click()) {
                self.layout_delete_armed = false;
            }
        }

        // Auto-save: update current layout when dock changes
        if let Some(ref name) = self.current_layout.clone() {
            if let Some(saved_ron) = self.layouts.get(name) {
                if *saved_ron != current_ron {
                    self.layouts.insert(name.clone(), current_ron);
                }
            }
        }
    }

    /// Draws Free Camera and Default Material dialogs.
    pub(crate) fn draw_dialogs(&mut self, ctx: &egui::Context) {
        // Free Camera dialog
        if self.free_camera_dialog_open {
            let mut open = self.free_camera_dialog_open;
            egui::Window::new("Adjust Free Camera")
                .open(&mut open)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let mut use_near = self.free_camera_override_near.is_some();
                        ui.checkbox(&mut use_near, "Override Near");
                        if use_near {
                            let mut val = self
                                .free_camera_override_near
                                .unwrap_or(self.data_model.view.default_near_clip.max(0.001));
                            ui.add(
                                egui::DragValue::new(&mut val)
                                    .speed(0.01)
                                    .range(0.001..=1e6),
                            );
                            self.free_camera_override_near = Some(val);
                        } else {
                            self.free_camera_override_near = None;
                            ui.label("(auto)");
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut use_far = self.free_camera_override_far.is_some();
                        ui.checkbox(&mut use_far, "Override Far");
                        if use_far {
                            let mut val = self
                                .free_camera_override_far
                                .unwrap_or(self.data_model.view.default_far_clip.max(0.1));
                            ui.add(egui::DragValue::new(&mut val).speed(1.0).range(0.1..=1e9));
                            self.free_camera_override_far = Some(val);
                        } else {
                            self.free_camera_override_far = None;
                            ui.label("(auto)");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("FOV:");
                        ui.add(
                            egui::DragValue::new(&mut self.data_model.view.free_camera_fov)
                                .speed(0.5)
                                .range(1.0..=179.0)
                                .suffix("\u{00B0}"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.data_model.view.lock_free_camera_aspect,
                            "Lock Aspect Ratio",
                        );
                        // Auto-enable camera mask when locking aspect
                        // (C++ viewSettingsDataModel.py:426-429)
                        if self.data_model.view.lock_free_camera_aspect
                            && self.data_model.view.camera_mask_mode
                                == crate::data_model::CameraMaskMode::None
                        {
                            self.data_model.view.camera_mask_mode =
                                crate::data_model::CameraMaskMode::Full;
                        }
                    });
                });
            self.free_camera_dialog_open = open;
            // Repaint while dialog is open so camera param changes take effect
            ctx.request_repaint();
        }

        // Default Material dialog
        if self.default_material_dialog_open {
            let mut open = self.default_material_dialog_open;
            egui::Window::new("Adjust Default Material")
                .open(&mut open)
                .resizable(false)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Ambient:");
                        ui.add(egui::Slider::new(
                            &mut self.data_model.view.default_material_ambient,
                            0.0..=1.0,
                        ));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Specular:");
                        ui.add(egui::Slider::new(
                            &mut self.data_model.view.default_material_specular,
                            0.0..=1.0,
                        ));
                    });
                    if ui.button("Reset").clicked() {
                        self.data_model.view.default_material_ambient = 0.2;
                        self.data_model.view.default_material_specular = 0.1;
                    }
                });
            self.default_material_dialog_open = open;
        }
    }
}
