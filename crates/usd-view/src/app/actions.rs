//! Action dispatching for the viewer application.

use std::collections::HashSet;

use usd_core::EditTarget;
use usd_sdf::{Path, TimeCode};

use crate::camera::FreeCamera;
use crate::data_model::DrawMode;
use crate::dock::{default_dock_state, default_dock_state_no_render};
use crate::keyboard::{AppAction, KeyboardHandler};
use crate::menus::RenderMode;

use super::ViewerApp;

use usd_shade::material_binding_api::MaterialBindingAPI;

impl ViewerApp {
    /// Process keyboard shortcuts and return true if app should close.
    pub(crate) fn dispatch_actions(&mut self, ctx: &egui::Context) -> bool {
        let actions = KeyboardHandler::process(ctx);
        let mut close = false;
        for action in actions {
            match action {
                AppAction::Quit => close = true,
                AppAction::Escape => {
                    // Dismiss topmost open dialog; if none — quit
                    if !self.dismiss_topmost() {
                        close = true;
                    }
                }
                _ => self.dispatch_action(&action, ctx),
            }
        }
        close
    }

    /// Try to close the topmost open dialog/overlay. Returns true if something was dismissed.
    pub(crate) fn dismiss_topmost(&mut self) -> bool {
        // Dismiss Adjust dialogs before preferences
        if self.free_camera_dialog_open {
            self.free_camera_dialog_open = false;
            return true;
        }
        if self.default_material_dialog_open {
            self.default_material_dialog_open = false;
            return true;
        }
        if self.prefs_state.open {
            self.prefs_state.open = false;
            return true;
        }
        false
    }

    /// Execute a single application action.
    pub(crate) fn dispatch_action(&mut self, action: &AppAction, ctx: &egui::Context) {
        match action {
            AppAction::OpenFile => self.open_file_dialog(),
            AppAction::Quit | AppAction::Escape => {} // handled by caller
            AppAction::TogglePlay => {
                self.playback.toggle_play();
                ctx.request_repaint();
            }
            AppAction::ReversePlay => {
                self.playback.toggle_reverse();
                ctx.request_repaint();
            }
            AppAction::StepForward => {
                let step = self.playback.step_size();
                let before = self.playback.current_frame();
                self.playback.step_forward(step);
                let after = self.playback.current_frame();
                log::info!(
                    "[input] StepForward: before={} step={} after={}",
                    before,
                    step,
                    after
                );
                self.data_model.root.current_time = TimeCode::new(after);
            }
            AppAction::StepBackward => {
                let step = self.playback.step_size();
                let before = self.playback.current_frame();
                self.playback.step_backward(step);
                let after = self.playback.current_frame();
                log::info!(
                    "[input] StepBackward: before={} step={} after={}",
                    before,
                    step,
                    after
                );
                self.data_model.root.current_time = TimeCode::new(after);
            }
            AppAction::FrameSelected => {
                // Match usdview/DCC behavior: `F` frames the current selection,
                // and when there is no focused selection it degenerates to
                // frame-all instead of becoming a no-op.
                self.pre_frame_camera = Some(self.camera.clone());
                if let Some(path) = self.data_model.selection.focus_path().cloned() {
                    self.frame_prim_in_viewport(&path);
                } else {
                    crate::panels::camera_controls::frame_all(
                        &mut self.camera,
                        &self.data_model,
                        1.1,
                    );
                }
            }
            AppAction::FrameAll => {
                // Save current camera so Toggle Framed View can restore it.
                self.pre_frame_camera = Some(self.camera.clone());
                crate::panels::camera_controls::frame_all(&mut self.camera, &self.data_model, 1.1);
            }
            AppAction::IncrementComplexity => {
                // Step to next discrete level per RefinementComplexity::next()
                use crate::data_model::RefinementComplexity;
                let cur = RefinementComplexity::from_value(self.data_model.view.complexity);
                self.data_model.view.complexity = cur.next().value();
            }
            AppAction::DecrementComplexity => {
                // Step to previous discrete level per RefinementComplexity::prev()
                use crate::data_model::RefinementComplexity;
                let cur = RefinementComplexity::from_value(self.data_model.view.complexity);
                self.data_model.view.complexity = cur.prev().value();
            }
            AppAction::MakeVisible => {
                let paths = self.data_model.selection.get_paths().to_vec();
                if let Some(stage) = self.data_model.root.stage.clone() {
                    if let Some(session_layer) = stage.get_session_layer() {
                        let prev_target = stage.get_edit_target();
                        stage.set_edit_target(EditTarget::for_local_layer(session_layer));
                        let tc = TimeCode::default_time();
                        for path in &paths {
                            if let Some(prim) = stage.get_prim_at_path(path) {
                                let imageable = usd_geom::imageable::Imageable::new(prim);
                                if imageable.is_valid() {
                                    imageable.make_visible(tc);
                                }
                            }
                        }
                        stage.set_edit_target(prev_target);
                    }
                }
                self.prim_tree_state.invalidate();
            }
            AppAction::FindPrims => {
                self.prim_tree_state.request_search_focus = true;
            }
            AppAction::ReloadAllLayers => {
                if let Some(stage) = self.data_model.root.stage.as_ref() {
                    if let Err(e) = stage.reload() {
                        self.last_error = Some(format!("Failed to reload stage layers: {e}"));
                    } else {
                        self.invalidate_scene(super::InvalidateLevel::Reload);
                    }
                }
            }
            AppAction::ToggleLoop => {
                self.playback.toggle_looping();
            }
            AppAction::SelectStageRoot => {
                if let Some(root) = self.data_model.root.pseudo_root() {
                    self.data_model
                        .selection
                        .set_paths(vec![root.path().clone()]);
                }
            }
            AppAction::SelectModelRoot => {
                self.select_enclosing_models_for_selection();
            }
            AppAction::ResetView => {
                // Per Python _resetView(): clear selection to root, reset camera, frame
                self.data_model
                    .selection
                    .set_paths(vec![Path::absolute_root()]);
                self.camera = FreeCamera::new();
                // Frame root (like Python _frameSelection after reset)
                crate::panels::camera_controls::frame_all(&mut self.camera, &self.data_model, 1.1);
            }
            AppAction::ToggleViewerMode => {
                // Save current dock layout, restore previous (or default)
                let saved = std::mem::replace(
                    &mut self.dock_state,
                    if !self.config.no_render {
                        // Switching TO no_render
                        self.saved_dock_state
                            .take()
                            .unwrap_or_else(default_dock_state_no_render)
                    } else {
                        // Switching TO render
                        self.saved_dock_state
                            .take()
                            .unwrap_or_else(default_dock_state)
                    },
                );
                self.saved_dock_state = Some(saved);
                self.config.no_render = !self.config.no_render;
            }
            AppAction::OpenPreferences => {
                self.prefs_state.open();
            }
            AppAction::ToggleFullscreen => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                    !ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)),
                ));
            }
            AppAction::NextFile => self.navigate_sibling(1),
            AppAction::PrevFile => self.navigate_sibling(-1),
            AppAction::MakeInvisible => {
                let paths = self.data_model.selection.get_paths().to_vec();
                if let Some(stage) = self.data_model.root.stage.clone() {
                    if let Some(session_layer) = stage.get_session_layer() {
                        let prev_target = stage.get_edit_target();
                        stage.set_edit_target(EditTarget::for_local_layer(session_layer));
                        let tc = TimeCode::default_time();
                        for path in &paths {
                            if let Some(prim) = stage.get_prim_at_path(path) {
                                let imageable = usd_geom::imageable::Imageable::new(prim);
                                if imageable.is_valid() {
                                    imageable.make_invisible(tc);
                                }
                            }
                        }
                        stage.set_edit_target(prev_target);
                    }
                }
                self.prim_tree_state.invalidate();
            }
            AppAction::VisOnly => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.vis_only_paths(&paths);
            }
            AppAction::RemoveSessionVis => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.clear_session_visibility_for_paths(&paths);
            }
            AppAction::ResetAllSessionVis => {
                if let Some(stage) = self.data_model.root.stage.clone() {
                    if let Some(session_layer) = stage.get_session_layer() {
                        let prev_target = stage.get_edit_target();
                        stage.set_edit_target(EditTarget::for_local_layer(session_layer));
                        self.reset_session_visibility(&stage);
                        stage.set_edit_target(prev_target);
                    }
                }
                self.prim_tree_state.invalidate();
            }
            AppAction::LoadSelected => {
                let paths = self.data_model.selection.get_paths().to_vec();
                for path in &paths {
                    self.load_payload(path);
                }
                self.prim_tree_state.invalidate();
            }
            AppAction::UnloadSelected => {
                let paths = self.data_model.selection.get_paths().to_vec();
                for path in &paths {
                    self.unload_payload(path);
                }
                self.prim_tree_state.invalidate();
            }
            AppAction::ActivateSelected => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.set_selected_prims_activation(&paths, true);
            }
            AppAction::DeactivateSelected => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.set_selected_prims_activation(&paths, false);
            }
            AppAction::ToggleOrthographic => {
                // Toggle orthographic mode on the free camera and sync menu state.
                let new_ortho = !self.camera.is_orthographic();
                self.camera.set_orthographic(new_ortho);
                self.menu_state.orthographic = new_ortho;
            }
            AppAction::ToggleAutoClippingPlanes => {
                let v = &mut self.data_model.view.auto_compute_clipping_planes;
                *v = !*v;
                // Keep menu_state in sync
                self.menu_state.auto_clipping_planes = *v;
            }
            AppAction::PauseRender => {
                self.menu_state.render_paused = !self.menu_state.render_paused;
                if self.menu_state.render_paused {
                    self.engine.pause_renderer();
                } else {
                    self.engine.resume_renderer();
                    self.menu_state.render_stopped = false;
                }
            }
            AppAction::StopRender => {
                self.engine.stop_renderer();
                self.menu_state.render_stopped = true;
                self.menu_state.render_paused = false;
            }
            AppAction::SetRenderMode(mode) => {
                self.menu_state.render_mode = *mode;
                self.data_model.view.draw_mode = render_mode_to_draw_mode(*mode);
            }
            AppAction::SaveFile => {
                // Save stage root layer (Ctrl+S, matches C++ appController.py SaveFile)
                if let Some(ref stage) = self.data_model.root.stage {
                    match stage.save() {
                        Ok(_) => tracing::info!("Stage saved"),
                        Err(e) => {
                            let msg = format!("Failed to save stage: {e}");
                            tracing::error!("{}", msg);
                            self.last_error = Some(msg);
                        }
                    }
                } else {
                    self.last_error = Some("No stage loaded".to_string());
                }
            }
            AppAction::CopyPrimPath => {
                if let Some(path) = self.data_model.selection.focus_path() {
                    ctx.copy_text(path.to_string());
                }
            }
            AppAction::OpenSplineViewer => {
                // Add SplineViewer tab to dock if not already present
                use crate::dock::DockTab;
                let has_spline = self
                    .dock_state
                    .iter_all_tabs()
                    .any(|(_, tab)| *tab == DockTab::SplineViewer);
                if !has_spline {
                    use egui_dock::NodeIndex;
                    self.dock_state.main_surface_mut().split_below(
                        NodeIndex::root(),
                        0.7,
                        vec![DockTab::SplineViewer],
                    );
                }
            }
            AppAction::SelectBoundPreviewMaterial => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.select_bound_material_for_paths(&paths, &usd_shade::tokens::tokens().preview);
            }
            AppAction::SelectBoundFullMaterial => {
                let paths = self.data_model.selection.get_paths().to_vec();
                self.select_bound_material_for_paths(&paths, &usd_shade::tokens::tokens().full);
            }
            AppAction::ToggleFramedView => {
                // Swap the live camera with the saved pre-frame snapshot.
                // If no snapshot exists yet, this is a no-op.
                if let Some(saved) = self.pre_frame_camera.take() {
                    let current = self.camera.clone();
                    self.camera = saved;
                    self.pre_frame_camera = Some(current);
                }
            }
            AppAction::UndoCameraMove => {
                if let Some(prev) = self.viewport_state.pop_camera_undo() {
                    self.camera = prev;
                }
            }
            AppAction::SetCamera(path) => {
                let stage_ref = self.data_model.root.stage.as_deref();
                match path {
                    Some(cam_path) => {
                        let sdf_path = Path::from_string(cam_path);
                        self.data_model
                            .view
                            .set_active_camera(sdf_path.as_ref(), stage_ref);
                    }
                    None => {
                        self.data_model.view.set_active_camera(None, stage_ref);
                    }
                }
                self.menu_state.active_camera_path =
                    self.data_model.view.active_camera_path.clone();
            }
            AppAction::SetRenderer(plugin_id) => {
                let token = usd_tf::Token::new(plugin_id);
                self.engine.set_renderer_plugin(&token);
                self.menu_state.current_renderer = plugin_id.clone();
            }
            AppAction::SetAOV(aov_name) => {
                let token = usd_tf::Token::new(aov_name);
                self.engine.set_renderer_aov(&token);
                self.menu_state.current_aov = aov_name.clone();
            }
        }
    }

    /// Toggles visibility of the prim on the session layer (Make Visible / Make Invisible).
    #[allow(dead_code)]
    pub(crate) fn toggle_visibility(&mut self, path: &Path) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let imageable = usd_geom::imageable::Imageable::new(prim);
        if !imageable.is_valid() {
            return;
        }
        let tc = self.data_model.root.current_time;
        let vis = imageable.compute_visibility(tc);
        let inherited = usd_geom::tokens::usd_geom_tokens().inherited.clone();
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(EditTarget::for_local_layer(session_layer));
        if vis == inherited {
            imageable.make_invisible(tc);
        } else {
            imageable.make_visible(tc);
        }
        stage.set_edit_target(prev_target);
    }

    /// Returns the operation set for a context action:
    /// multi-selection if the clicked path is selected, otherwise just clicked path.
    pub(crate) fn action_paths_or_selection(&self, action_path: &Path) -> Vec<Path> {
        let selected = self.data_model.selection.get_paths();
        if !selected.is_empty() && self.data_model.selection.is_selected(action_path) {
            return selected.to_vec();
        }
        vec![action_path.clone()]
    }

    /// Activate/deactivate prims, skipping pseudoroot and prototype prims.
    ///
    /// Matches C++ `_setSelectedPrimsActivation`: operates at Sdf level,
    /// clears selection before deactivation, then triggers full resync.
    pub(crate) fn set_selected_prims_activation(&mut self, paths: &[Path], active: bool) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };
        // Filter valid paths (skip pseudoroot and prototype prims)
        let valid_paths: Vec<usd_sdf::Path> = paths
            .iter()
            .filter(|path| {
                stage.get_prim_at_path(path).map_or(false, |prim| {
                    !prim.is_pseudo_root() && !prim.is_in_prototype()
                })
            })
            .cloned()
            .collect();
        if valid_paths.is_empty() {
            return;
        }
        // C++ clears selection before deactivation to avoid holding
        // paths to prims that become invalid
        if !active {
            self.data_model.selection.clear();
        }
        // Author at Sdf level via set_active (uses create_prim_in_layer internally)
        for path in &valid_paths {
            if let Some(prim) = stage.get_prim_at_path(path) {
                if !prim.set_active(active) {
                    log::warn!("set_active({}) failed for {}", active, path);
                }
            }
        }
        // Full resync: delegate must re-traverse to pick up active flag changes
        self.invalidate_scene(super::InvalidateLevel::Reload);
    }

    /// Clear authored visibility opinions from the active edit target layer.
    pub(crate) fn reset_session_visibility(&self, stage: &usd_core::Stage) {
        let mut stack = vec![stage.get_pseudo_root()];
        while let Some(prim) = stack.pop() {
            if !prim.is_valid() {
                continue;
            }
            let imageable = usd_geom::imageable::Imageable::new(prim.clone());
            if imageable.is_valid() {
                let _ = imageable
                    .get_visibility_attr()
                    .clear(TimeCode::default_time());
            }
            for child in prim.get_all_children() {
                stack.push(child);
            }
        }
    }

    /// Make all root prims invisible in current edit target.
    pub(crate) fn invis_root_prims(&self, stage: &usd_core::Stage) {
        let tc = TimeCode::default_time();
        for prim in stage.get_pseudo_root().get_children() {
            let imageable = usd_geom::imageable::Imageable::new(prim);
            if imageable.is_valid() {
                imageable.make_invisible(tc);
            }
        }
    }

    /// Reference behavior: reset session visibility, hide roots, then reveal selected.
    pub(crate) fn vis_only_paths(&mut self, paths: &[Path]) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(EditTarget::for_local_layer(session_layer));

        self.reset_session_visibility(stage);
        self.invis_root_prims(stage);

        let tc = TimeCode::default_time();
        for path in paths {
            let Some(prim) = stage.get_prim_at_path(path) else {
                continue;
            };
            let imageable = usd_geom::imageable::Imageable::new(prim);
            if imageable.is_valid() {
                imageable.make_visible(tc);
            }
        }

        stage.set_edit_target(prev_target);
        self.prim_tree_state.invalidate();
    }

    /// Remove authored session-layer visibility opinions for given prim paths.
    pub(crate) fn clear_session_visibility_for_paths(&mut self, paths: &[Path]) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(EditTarget::for_local_layer(session_layer));

        let tc = TimeCode::default_time();
        for path in paths {
            let Some(prim) = stage.get_prim_at_path(path) else {
                continue;
            };
            let imageable = usd_geom::imageable::Imageable::new(prim);
            if imageable.is_valid() {
                let _ = imageable.get_visibility_attr().clear(tc);
            }
        }

        stage.set_edit_target(prev_target);
        self.prim_tree_state.invalidate();
    }

    /// Return nearest model ancestor path for a prim path (ancestor-only semantics).
    pub(crate) fn get_enclosing_model_path(&self, path: &Path) -> Option<Path> {
        let stage = self.data_model.root.stage.as_ref()?;
        let prim = stage.get_prim_at_path(path)?;
        let mut current = prim.parent();
        while current.is_valid() {
            if current.is_model() {
                return Some(current.path().clone());
            }
            current = current.parent();
        }
        None
    }

    /// Select model roots for selected prims (closest enclosing model, or self if none).
    pub(crate) fn select_enclosing_models_for_selection(&mut self) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };
        let selected_paths = self.data_model.selection.get_paths().to_vec();
        if selected_paths.is_empty() {
            return;
        }

        let mut next_selection = Vec::new();
        let mut seen = HashSet::new();
        for path in selected_paths {
            let Some(prim) = stage.get_prim_at_path(&path) else {
                continue;
            };
            let target = self
                .get_enclosing_model_path(&path)
                .unwrap_or_else(|| prim.path().clone());
            if seen.insert(target.clone()) {
                next_selection.push(target);
            }
        }

        self.data_model.selection.set_paths(next_selection);
    }

    /// Select bound materials for provided paths and purpose.
    pub(crate) fn select_bound_material_for_paths(
        &mut self,
        paths: &[Path],
        purpose: &usd_tf::Token,
    ) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };

        let mut selection = Vec::new();
        let mut seen = HashSet::new();

        for path in paths {
            let Some(prim) = stage.get_prim_at_path(path) else {
                continue;
            };
            let mut binding_rel = None;
            let material = MaterialBindingAPI::new(prim).compute_bound_material(
                purpose,
                &mut binding_rel,
                true,
            );
            if !material.is_valid() {
                continue;
            }
            let mat_prim = material.get_prim();
            if !mat_prim.is_valid() {
                continue;
            }
            let mat_path = mat_prim.path().clone();
            if seen.insert(mat_path.clone()) {
                selection.push(mat_path);
            }
        }

        self.data_model.selection.set_paths(selection);
    }

    /// Select prims that own the material binding relationship for the given paths and purpose.
    pub(crate) fn select_binding_rel_for_paths(&mut self, paths: &[Path], purpose: &usd_tf::Token) {
        let Some(stage) = self.data_model.root.stage.as_ref() else {
            return;
        };
        let mut selection = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            let Some(prim) = stage.get_prim_at_path(path) else {
                continue;
            };
            let mut binding_rel = None;
            let _material = MaterialBindingAPI::new(prim).compute_bound_material(
                purpose,
                &mut binding_rel,
                true,
            );
            if let Some(rel) = binding_rel {
                let prim_path = rel.prim_path();
                if seen.insert(prim_path.clone()) {
                    selection.push(prim_path);
                }
            }
        }
        self.data_model.selection.set_paths(selection);
    }

    /// Loads the payload at the given path.
    pub(crate) fn load_payload(&mut self, path: &Path) {
        if let Some(ref stage) = self.data_model.root.stage {
            stage.load(path, None);
        }
    }

    /// Unloads the payload at the given path.
    pub(crate) fn unload_payload(&mut self, path: &Path) {
        if let Some(ref stage) = self.data_model.root.stage {
            stage.unload(path);
        }
    }

    /// Frames the given prim in the viewport camera.
    pub fn frame_prim_in_viewport(&mut self, path: &usd_sdf::Path) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let imageable = usd_geom::imageable::Imageable::new(prim);
        if !imageable.is_valid() {
            return;
        }
        let tc = self.data_model.root.current_time;
        let bbox =
            crate::bounds::compute_world_bound_for_view(&imageable, tc, &self.data_model.view);
        let range = bbox.compute_aligned_range();
        if !range.is_empty() {
            let min = *range.min();
            let max = *range.max();
            if min.x.is_finite()
                && min.y.is_finite()
                && min.z.is_finite()
                && max.x.is_finite()
                && max.y.is_finite()
                && max.z.is_finite()
                && min.x.abs() <= 1.0e8
                && min.y.abs() <= 1.0e8
                && min.z.abs() <= 1.0e8
                && max.x.abs() <= 1.0e8
                && max.y.abs() <= 1.0e8
                && max.z.abs() <= 1.0e8
            {
                self.camera.frame_selection(min, max, 1.1);
            }
        }
    }

    /// Expand all prims in the prim tree.
    pub(crate) fn expand_all_prims(&mut self) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let root = stage.get_pseudo_root();
        self.prim_tree_state.expand_recursive(&root);
    }

    /// Collapse all prims in the prim tree.
    pub(crate) fn collapse_all_prims(&mut self) {
        if self.data_model.root.stage.is_none() {
            return;
        }
        // Clear entire expanded set — collapse_recursive("/") can't work
        // because prefix "//" doesn't match child paths like "/World".
        self.prim_tree_state.expanded.clear();
        self.prim_tree_state.tree_dirty = true;
    }

    /// Toggle visibility for a single prim on the session layer (P1-7 VIS column click).
    /// Matches Python toggleVis(): invisible -> inherited, else -> invisible.
    pub(crate) fn toggle_visibility_on_session(&mut self, path: &Path) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let imageable = usd_geom::imageable::Imageable::new(prim);
        if !imageable.is_valid() {
            return;
        }
        let tc = self.data_model.root.current_time;
        let vis = imageable.compute_visibility(tc);
        let tokens = usd_geom::tokens::usd_geom_tokens();
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(usd_core::EditTarget::for_local_layer(session_layer));
        if vis == tokens.invisible {
            // Currently invisible: make it inherited (visible)
            imageable.make_visible(tc);
        } else {
            // Currently visible/inherited: make it invisible
            imageable.make_invisible(tc);
        }
        stage.set_edit_target(prev_target);
        // Invalidate only the single cache entry so the row refreshes.
        self.prim_tree_state.cache.remove(&path.to_string());
        self.prim_tree_state.tree_dirty = true;
    }

    /// Set draw mode on the session layer for a model prim (P1-1).
    /// Applies GeomModelAPI and writes model:drawMode.
    pub(crate) fn set_draw_mode_on_session(&mut self, path: &Path, mode_str: &str) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(usd_core::EditTarget::for_local_layer(session_layer));
        // Apply API schema and write the draw mode token.
        if let Some(model_api) = usd_geom::model_api::ModelAPI::apply(&prim) {
            if let Some(attr) = model_api.create_model_draw_mode_attr(None) {
                attr.set(usd_tf::Token::new(mode_str), usd_sdf::TimeCode::default());
            }
        }
        stage.set_edit_target(prev_target);
        self.prim_tree_state.cache.remove(&path.to_string());
        self.prim_tree_state.tree_dirty = true;
    }

    /// Clear the session-layer draw mode override for a prim (P1-1 clear button).
    pub(crate) fn clear_draw_mode_on_session(&mut self, path: &Path) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let model_api = usd_geom::model_api::ModelAPI::new(prim);
        if let Some(attr) = model_api.get_model_draw_mode_attr() {
            let prev_target = stage.get_edit_target();
            stage.set_edit_target(usd_core::EditTarget::for_local_layer(session_layer));
            // Clear the authored default value on the session layer spec.
            attr.clear_authored_value();
            stage.set_edit_target(prev_target);
        }
        self.prim_tree_state.cache.remove(&path.to_string());
        self.prim_tree_state.tree_dirty = true;
    }

    /// Toggle guide visibility for a prim on the session layer (P1-4).
    /// Matches Python toggleGuides(): applies VisibilityAPI, flips visible <-> invisible.
    pub(crate) fn toggle_guides_on_session(&mut self, path: &Path) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let Some(session_layer) = stage.get_session_layer() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };
        let prev_target = stage.get_edit_target();
        stage.set_edit_target(usd_core::EditTarget::for_local_layer(session_layer));
        let vis_api = usd_geom::visibility_api::VisibilityAPI::apply(&prim);
        if vis_api.is_valid() {
            let attr = vis_api.create_guide_visibility_attr();
            if attr.is_valid() {
                let tokens = usd_geom::tokens::usd_geom_tokens();
                let tc = usd_sdf::TimeCode::default();
                let current = attr
                    .get(tc)
                    .and_then(|v| v.downcast::<usd_tf::Token>().cloned())
                    .unwrap_or_else(|| tokens.invisible.clone());
                let new_val = if current == tokens.visible {
                    tokens.invisible.clone()
                } else {
                    tokens.visible.clone()
                };
                attr.set(new_val, tc);
            }
        }
        stage.set_edit_target(prev_target);
        self.prim_tree_state.cache.remove(&path.to_string());
        self.prim_tree_state.tree_dirty = true;
    }

    /// Expand prim tree to a specific depth level.
    pub(crate) fn expand_to_depth(&mut self, depth: usize) {
        let Some(ref stage) = self.data_model.root.stage else {
            return;
        };
        let root = stage.get_pseudo_root();
        self.prim_tree_state.collapse_recursive(&root);
        self.prim_tree_state.expand_to_depth(&root, depth);
    }
}

/// Map menu RenderMode to the internal DrawMode used by the renderer.
fn render_mode_to_draw_mode(mode: RenderMode) -> DrawMode {
    match mode {
        RenderMode::Wireframe => DrawMode::Wireframe,
        RenderMode::WireframeOnSurface => DrawMode::WireframeOnSurface,
        RenderMode::SmoothShaded => DrawMode::ShadedSmooth,
        RenderMode::FlatShaded => DrawMode::ShadedFlat,
        RenderMode::Points => DrawMode::Points,
        RenderMode::GeomOnly => DrawMode::GeometryOnly,
        RenderMode::GeomFlat => DrawMode::GeomFlat,
        RenderMode::GeomSmooth => DrawMode::GeomSmooth,
        RenderMode::HiddenSurfaceWireframe => DrawMode::HiddenSurfaceWireframe,
        RenderMode::Bounds => DrawMode::Bounds,
    }
}
