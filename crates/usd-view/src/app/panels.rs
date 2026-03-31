//! Panel drawing methods for the viewer: viewport, prim tree, attributes, etc.

use usd_core::EditTarget;
use usd_sdf::TimeCode;

use crate::data_model::{CameraMaskMode, ClearColor, HighlightColor, SelectionHighlightMode};
use crate::panels::{
    attributes_enhanced, composition, layer_stack_enhanced, prim_tree_enhanced, spline_viewer,
    viewport,
};

use super::ViewerApp;

impl ViewerApp {
    /// Viewport panel.
    pub fn ui_viewport(&mut self, ui: &mut egui::Ui) {
        let mut actions = Vec::new();
        viewport::ui_viewport(
            ui,
            self.data_model.root.stage.is_some(),
            &mut self.camera,
            &mut self.data_model,
            Some(&mut self.engine),
            &mut self.viewport_state,
            &self.menu_state.scene_cameras,
            &mut actions,
        );
        // Dispatch context menu actions
        for action in &actions {
            self.dispatch_action(action, ui.ctx());
        }

        if let Some(ref progress) = self.loading_state {
            let rect = ui.max_rect();
            let card_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(16.0, 16.0),
                egui::vec2(320.0, 72.0),
            );
            ui.painter().rect_filled(
                card_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(8, 12, 18, 220),
            );
            ui.painter().text(
                card_rect.min + egui::vec2(12.0, 14.0),
                egui::Align2::LEFT_CENTER,
                &progress.message,
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            let bar_rect = egui::Rect::from_min_size(
                card_rect.min + egui::vec2(12.0, 32.0),
                egui::vec2(card_rect.width() - 24.0, 8.0),
            );
            ui.painter()
                .rect_filled(bar_rect, 4.0, egui::Color32::from_black_alpha(120));
            let fill_rect = egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(bar_rect.width() * progress.progress, bar_rect.height()),
            );
            ui.painter()
                .rect_filled(fill_rect, 4.0, egui::Color32::from_rgb(80, 160, 255));
            ui.painter().text(
                card_rect.min + egui::vec2(12.0, 54.0),
                egui::Align2::LEFT_CENTER,
                format!("{}", progress.phase),
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(200),
            );
        }

        // Auto-frame after first render uses the composed USD scene bound.
        //
        // This intentionally does not rely on engine-side mesh bookkeeping.
        // Binary and text forms of the same asset can distribute transforms
        // differently across wrapper xforms vs mesh locals, while the stage
        // world bound remains stable and matches usdviewq behavior.
        //
        // Keep requesting a repaint once the camera is reframed here: the
        // first presented image was rendered with the pre-load camera state,
        // so without an immediate second frame the user can get stuck looking
        // at a stale clipped image until the next input event.
        if self.auto_frame_pending {
            if let Some((min, max)) = self.data_model.compute_stage_bbox_for_view() {
                self.camera.frame_selection(min, max, 1.1);
                tracing::info!(
                    "[load] auto-frame from stage bbox: dist={:.2}",
                    self.camera.dist()
                );
                self.auto_frame_pending = false;
                ui.ctx().request_repaint();
            }
        }

        // Request repaint while progressive rprim sync is active
        if self.engine.is_progressive_sync_active() {
            ui.ctx().request_repaint();
        }
    }

    /// Prim tree panel (enhanced version).
    pub fn ui_prim_tree(&mut self, ui: &mut egui::Ui) {
        // Keep outliner filter toggles aligned with global view settings.
        self.prim_tree_state.filters.show_inactive = self.data_model.view.show_inactive_prims;
        self.prim_tree_state.filters.show_prototypes =
            self.data_model.view.show_all_prototype_prims;
        self.prim_tree_state.filters.show_undefined = self.data_model.view.show_undefined_prims;
        self.prim_tree_state.filters.show_abstract = self.data_model.view.show_abstract_prims;
        self.prim_tree_state.filters.use_display_names =
            self.data_model.view.show_prim_display_names;

        // Sync column visibility from menu state
        self.prim_tree_state.show_type_column = self.menu_state.show_type_column;
        self.prim_tree_state.show_vis_column = self.menu_state.show_vis_column;
        self.prim_tree_state.show_guides_column = self.menu_state.show_guides_column;
        self.prim_tree_state.show_draw_mode_column = self.menu_state.show_draw_mode_column;

        prim_tree_enhanced::ui_prim_tree_enhanced(
            ui,
            &self.data_model,
            &mut self.prim_tree_state,
            self.config.no_render,
        );

        // Propagate local outliner toggles back to shared settings/model.
        self.data_model.view.show_inactive_prims = self.prim_tree_state.filters.show_inactive;
        self.data_model.view.show_all_prototype_prims =
            self.prim_tree_state.filters.show_prototypes;
        self.data_model.view.show_undefined_prims = self.prim_tree_state.filters.show_undefined;
        self.data_model.view.show_abstract_prims = self.prim_tree_state.filters.show_abstract;
        self.data_model.view.show_prim_display_names =
            self.prim_tree_state.filters.use_display_names;

        // Drain actions collected during the frame and dispatch them
        let actions: Vec<_> = self.prim_tree_state.actions.drain(..).collect();
        for action in actions {
            match action {
                prim_tree_enhanced::PrimTreeAction::Select(path) => {
                    self.data_model.selection.switch_to_path(path);
                }
                prim_tree_enhanced::PrimTreeAction::CopyPath(path)
                | prim_tree_enhanced::PrimTreeAction::CopyModelPath(path) => {
                    ui.ctx().copy_text(path.to_string());
                }
                prim_tree_enhanced::PrimTreeAction::Frame(path) => {
                    self.frame_prim_in_viewport(&path);
                }
                prim_tree_enhanced::PrimTreeAction::MakeVisible(path) => {
                    if let Some(stage) = self.data_model.root.stage.clone() {
                        if let Some(session_layer) = stage.get_session_layer() {
                            let prev_target = stage.get_edit_target();
                            stage.set_edit_target(EditTarget::for_local_layer(session_layer));
                            if let Some(prim) = stage.get_prim_at_path(&path) {
                                let imageable = usd_geom::imageable::Imageable::new(prim);
                                if imageable.is_valid() {
                                    imageable.make_visible(TimeCode::default_time());
                                }
                            }
                            stage.set_edit_target(prev_target);
                        }
                    }
                    self.prim_tree_state.invalidate();
                }
                prim_tree_enhanced::PrimTreeAction::MakeInvisible(path) => {
                    if let Some(stage) = self.data_model.root.stage.clone() {
                        if let Some(session_layer) = stage.get_session_layer() {
                            let prev_target = stage.get_edit_target();
                            stage.set_edit_target(EditTarget::for_local_layer(session_layer));
                            if let Some(prim) = stage.get_prim_at_path(&path) {
                                let imageable = usd_geom::imageable::Imageable::new(prim);
                                if imageable.is_valid() {
                                    imageable.make_invisible(TimeCode::default_time());
                                }
                            }
                            stage.set_edit_target(prev_target);
                        }
                    }
                    self.prim_tree_state.invalidate();
                }
                prim_tree_enhanced::PrimTreeAction::LoadPayload(path) => {
                    self.load_payload(&path);
                }
                prim_tree_enhanced::PrimTreeAction::UnloadPayload(path) => {
                    self.unload_payload(&path);
                }
                prim_tree_enhanced::PrimTreeAction::SetAsActiveCamera(path) => {
                    // Validate prim is a Camera before setting (C++ cameraPrim setter)
                    if let Some(stage) = self.data_model.root.stage.as_ref() {
                        if let Some(prim) = stage.get_prim_at_path(&path) {
                            self.data_model.view.set_camera_prim(Some(&prim));
                        }
                    }
                }
                prim_tree_enhanced::PrimTreeAction::SetAsActiveRenderSettings(path) => {
                    // C++ UsdImagingGLEngine::SetActiveRenderSettingsPrimPath
                    self.engine.set_active_render_settings_prim_path(path);
                }
                prim_tree_enhanced::PrimTreeAction::SetAsActiveRenderPass(path) => {
                    // C++ UsdImagingGLEngine::SetActiveRenderPassPrimPath
                    self.engine.set_active_render_pass_prim_path(path);
                }
                prim_tree_enhanced::PrimTreeAction::Activate(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.set_selected_prims_activation(&paths, true);
                }
                prim_tree_enhanced::PrimTreeAction::Deactivate(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.set_selected_prims_activation(&paths, false);
                }
                prim_tree_enhanced::PrimTreeAction::VisOnly(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.vis_only_paths(&paths);
                }
                prim_tree_enhanced::PrimTreeAction::RemoveSessionVis(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.clear_session_visibility_for_paths(&paths);
                }
                prim_tree_enhanced::PrimTreeAction::JumpToEnclosingModel(path) => {
                    if let Some(model_path) = self.get_enclosing_model_path(&path) {
                        self.data_model.selection.switch_to_path(model_path);
                    }
                }
                prim_tree_enhanced::PrimTreeAction::SelectBoundPreviewMaterial(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.select_bound_material_for_paths(
                        &paths,
                        &usd_shade::tokens::tokens().preview,
                    );
                }
                prim_tree_enhanced::PrimTreeAction::SelectBoundFullMaterial(path) => {
                    let paths = self.action_paths_or_selection(&path);
                    self.select_bound_material_for_paths(&paths, &usd_shade::tokens::tokens().full);
                }
                prim_tree_enhanced::PrimTreeAction::CopyPrimName(name) => {
                    ui.ctx().copy_text(name);
                }
                prim_tree_enhanced::PrimTreeAction::SetVariantSelection {
                    path,
                    vset_name,
                    variant_name,
                } => {
                    // Apply the variant selection and mark the tree dirty so it recomposes.
                    if let Some(stage) = &self.data_model.root.stage {
                        if let Some(prim) = stage.get_prim_at_path(&path) {
                            prim.get_variant_sets()
                                .set_selection(&vset_name, &variant_name);
                            self.prim_tree_state.tree_dirty = true;
                        }
                    }
                }
                // P1-7: Toggle visibility via VIS column click (no prim selection).
                prim_tree_enhanced::PrimTreeAction::ToggleVis(path) => {
                    self.toggle_visibility_on_session(&path);
                }
                // P1-5: Load payload and all descendants.
                prim_tree_enhanced::PrimTreeAction::LoadPayloadWithDescendants(path) => {
                    if let Some(ref stage) = self.data_model.root.stage {
                        stage.load(&path, None);
                        // Also load all descendant payloads by traversing children.
                        if let Some(prim) = stage.get_prim_at_path(&path) {
                            let mut stack: Vec<usd_core::Prim> = prim.get_all_children();
                            while let Some(child) = stack.pop() {
                                if child.has_payload() {
                                    stage.load(child.path(), None);
                                }
                                stack.extend(child.get_all_children());
                            }
                        }
                    }
                    self.prim_tree_state.invalidate();
                }
                // P1-1: Set draw mode on the session layer.
                prim_tree_enhanced::PrimTreeAction::SetDrawMode(path, mode_str) => {
                    self.set_draw_mode_on_session(&path, &mode_str);
                }
                // P1-1: Clear session-layer draw mode override.
                prim_tree_enhanced::PrimTreeAction::ClearDrawMode(path) => {
                    self.clear_draw_mode_on_session(&path);
                }
                // P1-4: Toggle guide visibility.
                prim_tree_enhanced::PrimTreeAction::ToggleGuides(path) => {
                    self.toggle_guides_on_session(&path);
                }
            }
        }
    }

    /// Attributes panel (enhanced version).
    pub fn ui_attributes(&mut self, ui: &mut egui::Ui) {
        attributes_enhanced::ui_attributes_enhanced(ui, &self.data_model, &mut self.attrs_state);
        let actions: Vec<_> = self.attrs_state.actions.drain(..).collect();
        for action in actions {
            match action {
                attributes_enhanced::AttributeAction::SelectPath(path) => {
                    self.data_model.selection.switch_to_path(path);
                }
                attributes_enhanced::AttributeAction::CopyText(text) => {
                    ui.ctx().copy_text(text);
                }
                attributes_enhanced::AttributeAction::JumpToDefiningLayer { layer_id, .. } => {
                    log::info!("Jump to defining layer: {}", layer_id);
                    // TODO: switch to layer stack panel and highlight the layer
                }
                attributes_enhanced::AttributeAction::OpenInEditor { attr_name } => {
                    // Open attribute in the value editor dialog
                    if let Some(prim) = self.data_model.first_selected_prim() {
                        if let Some(attr) = prim.get_attribute(&attr_name) {
                            let tc = self.data_model.root.current_time;
                            self.attr_editor_state
                                .open_for(prim.path(), &attr_name, &attr, tc);
                        }
                    }
                }
                attributes_enhanced::AttributeAction::ViewSpline { attr_name } => {
                    // P2-11: Open SplineViewer for time-sampled attribute
                    if let Some(prim) = self.data_model.first_selected_prim() {
                        self.spline_state.set_attribute(&prim, &attr_name);
                        // Ensure SplineViewer tab is visible in dock
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
                }
            }
        }
    }

    #[allow(dead_code)]
    /// Editable or read-only attribute value widget (reference: attributeValueEditor.py).
    fn attr_value_edit(
        ui: &mut egui::Ui,
        attr: &usd_core::Attribute,
        tc: usd_sdf::TimeCode,
        type_str: &str,
        stage: Option<&std::sync::Arc<usd_core::Stage>>,
    ) {
        let can_edit = stage.and_then(|s| s.get_session_layer()).is_some();

        let (base_type, is_array) = {
            let s = type_str.trim();
            if s.ends_with("[]") {
                (s.strip_suffix("[]").unwrap_or(s), true)
            } else {
                (s, false)
            }
        };

        // Like usdview's dedicated array viewer: avoid eager full-array fetch in the summary table.
        if is_array {
            ui.label(format!("<{}: omitted>", type_str));
            return;
        }

        let val_opt = attr.get(tc);
        if can_edit {
            match base_type {
                "int" | "int64" => {
                    if let Some(ref v) = val_opt {
                        if let Some(&i) = v.get::<i32>() {
                            let mut x = i as f64;
                            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                                Self::set_attr_on_session(
                                    stage,
                                    attr,
                                    usd_vt::Value::from(i32::clamp(x as i32, i32::MIN, i32::MAX)),
                                    tc,
                                );
                            }
                            return;
                        }
                        if let Some(&i) = v.get::<i64>() {
                            let mut x = i as f64;
                            if ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed() {
                                Self::set_attr_on_session(
                                    stage,
                                    attr,
                                    usd_vt::Value::from(i64::clamp(x as i64, i64::MIN, i64::MAX)),
                                    tc,
                                );
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
                                Self::set_attr_on_session(
                                    stage,
                                    attr,
                                    usd_vt::Value::from(x as f32),
                                    tc,
                                );
                            }
                            return;
                        }
                        if let Some(&f) = v.get::<f64>() {
                            let mut x = f;
                            if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                                Self::set_attr_on_session(stage, attr, usd_vt::Value::from(x), tc);
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
                                Self::set_attr_on_session(stage, attr, usd_vt::Value::from(x), tc);
                            }
                            return;
                        }
                    }
                }
                "string" | "token" => {
                    let s = val_opt
                        .as_ref()
                        .and_then(|v| v.get::<String>().cloned())
                        .or_else(|| {
                            val_opt.as_ref().and_then(|v| {
                                v.get::<usd_tf::Token>().map(|t| t.get_text().to_string())
                            })
                        })
                        .unwrap_or_default();
                    let mut buf = s.clone();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .desired_width(120.0)
                            .hint_text("value"),
                    );
                    if resp.lost_focus() && buf != s {
                        if base_type == "token" {
                            Self::set_attr_on_session(
                                stage,
                                attr,
                                usd_vt::Value::from(usd_tf::Token::new(&buf)),
                                tc,
                            );
                        } else {
                            Self::set_attr_on_session(
                                stage,
                                attr,
                                usd_vt::Value::from(buf.clone()),
                                tc,
                            );
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        ui.label(Self::format_attr_value(val_opt));
    }

    /// Sets attribute value on session layer and restores edit target.
    fn set_attr_on_session(
        stage: Option<&std::sync::Arc<usd_core::Stage>>,
        attr: &usd_core::Attribute,
        value: usd_vt::Value,
        tc: usd_sdf::TimeCode,
    ) {
        let Some(stage) = stage else { return };
        let Some(session) = stage.get_session_layer() else {
            return;
        };
        let prev = stage.get_edit_target();
        stage.set_edit_target(usd_core::EditTarget::for_local_layer(session));
        let _ = attr.set(value, tc);
        stage.set_edit_target(prev);
    }

    /// Pretty-prints an attribute value (reference: scalarTypes.ToString).
    pub(crate) fn format_attr_value(val: Option<usd_vt::Value>) -> String {
        match val {
            None => "None".to_string(),
            Some(v) => Self::format_vt_value(&v),
        }
    }

    pub(crate) fn format_vt_value(val: &usd_vt::Value) -> String {
        if let Some(s) = val.get::<String>() {
            return s.clone();
        }
        if let Some(t) = val.get::<usd_tf::Token>() {
            return t.get_text().to_string();
        }
        if let Some(&i) = val.get::<i32>() {
            return i.to_string();
        }
        if let Some(&i) = val.get::<i64>() {
            return i.to_string();
        }
        if let Some(&f) = val.get::<f32>() {
            return format!("{:?}", f);
        }
        if let Some(&f) = val.get::<f64>() {
            return format!("{:?}", f);
        }
        if let Some(&b) = val.get::<bool>() {
            return b.to_string();
        }
        if let Some(p) = val.get::<usd_sdf::Path>() {
            return p.to_string();
        }
        // Array types (reference: arrayAttributeView.py, scalarTypes.ToString)
        if let Some(arr) = val.get::<Vec<i32>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |i| i.to_string()));
        }
        if let Some(arr) = val.get::<Vec<i64>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |i| i.to_string()));
        }
        if let Some(arr) = val.get::<Vec<f32>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |f| format!("{:?}", f)));
        }
        if let Some(arr) = val.get::<Vec<f64>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |f| format!("{:?}", f)));
        }
        if let Some(arr) = val.get::<Vec<String>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |s| format!("{:?}", s)));
        }
        if let Some(arr) = val.get::<Vec<usd_tf::Token>>() {
            return format!(
                "[{}]",
                Self::format_array_fmt(arr, |t| format!("{:?}", t.get_text()))
            );
        }
        if let Some(arr) = val.get::<Vec<usd_sdf::Path>>() {
            return format!("[{}]", Self::format_array_fmt(arr, |p| p.to_string()));
        }
        if let Some(arr) = val.get::<Vec<usd_gf::Vec3f>>() {
            return format!(
                "[{}]",
                Self::format_array_fmt(arr, |v| format!("({}, {}, {})", v.x, v.y, v.z))
            );
        }
        if let Some(arr) = val.get::<Vec<usd_gf::Vec3d>>() {
            return format!(
                "[{}]",
                Self::format_array_fmt(arr, |v| format!("({}, {}, {})", v.x, v.y, v.z))
            );
        }
        if val.is_array_valued() {
            return format!("[array: {} elements]", val.array_size());
        }
        format!("{:?}", val)
    }

    /// Formats array for display, truncating if too long (reference: arrayAttributeView lazy load).
    fn format_array_fmt<T, F>(arr: &[T], fmt: F) -> String
    where
        F: Fn(&T) -> String,
    {
        const MAX_SHOW: usize = 5;
        if arr.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = arr.iter().take(MAX_SHOW).map(|x| fmt(x)).collect();
        let s = parts.join(", ");
        if arr.len() > MAX_SHOW {
            format!("{}... ({} more)", s, arr.len() - MAX_SHOW)
        } else {
            s
        }
    }

    /// Layer stack panel (enhanced version).
    pub fn ui_layer_stack(&mut self, ui: &mut egui::Ui) {
        layer_stack_enhanced::ui_layer_stack_enhanced(
            ui,
            &self.data_model,
            &mut self.layer_stack_state,
        );
        let actions: Vec<_> = self.layer_stack_state.actions.drain(..).collect();
        for action in actions {
            match action {
                layer_stack_enhanced::LayerStackAction::CopyText(text) => {
                    ui.ctx().copy_text(text);
                }
                layer_stack_enhanced::LayerStackAction::MuteLayer(id) => {
                    if let Some(ref stage) = self.data_model.root.stage {
                        stage.mute_layer(&id);
                    }
                }
                layer_stack_enhanced::LayerStackAction::UnmuteLayer(id) => {
                    if let Some(ref stage) = self.data_model.root.stage {
                        stage.unmute_layer(&id);
                    }
                }
                layer_stack_enhanced::LayerStackAction::OpenInEditor(path) => {
                    if let Err(e) = Self::open_layer_in_editor(&path) {
                        tracing::warn!("Open layer in editor: {}", e);
                    }
                }
                layer_stack_enhanced::LayerStackAction::OpenInUsdview(path) => {
                    if let Err(e) = Self::open_layer_in_usdview(&path) {
                        tracing::warn!("Open layer in usdview: {}", e);
                    }
                }
            }
        }
    }

    /// Composition arcs panel.
    pub fn ui_composition(&mut self, ui: &mut egui::Ui) {
        composition::ui_composition(ui, &self.data_model, &mut self.composition_state);
        let actions: Vec<_> = self.composition_state.actions.drain(..).collect();
        for action in actions {
            match action {
                composition::CompositionAction::SelectPath(path) => {
                    self.data_model.selection.switch_to_path(path);
                }
                composition::CompositionAction::CopyText(text) => {
                    ui.ctx().copy_text(text);
                }
            }
        }
    }

    /// Settings panel.
    pub fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("View Settings");
        ui.separator();

        ui.horizontal(|ui| {
            use crate::data_model::RefinementComplexity;
            let level = RefinementComplexity::from_value(self.data_model.view.complexity);
            ui.label(format!("Complexity: {}", level.name()));
            ui.add(
                egui::Slider::new(
                    &mut self.data_model.view.complexity,
                    RefinementComplexity::MIN..=RefinementComplexity::MAX,
                )
                .step_by(0.1)
                .show_value(false),
            );
        });

        ui.checkbox(
            &mut self.data_model.view.display_guide,
            "Show camera guides",
        );
        ui.checkbox(&mut self.data_model.view.display_proxy, "Show proxy prims");
        ui.checkbox(
            &mut self.data_model.view.display_render,
            "Show render prims",
        );
        ui.checkbox(
            &mut self.data_model.view.show_bboxes,
            "BBox stand-in (unloaded prims)",
        );

        ui.separator();
        ui.label("Camera mask:");
        egui::ComboBox::from_id_salt("camera_mask")
            .width(140.0)
            .selected_text(self.data_model.view.camera_mask_mode.name())
            .show_ui(ui, |ui| {
                for mode in [
                    CameraMaskMode::None,
                    CameraMaskMode::Partial,
                    CameraMaskMode::Full,
                ] {
                    if ui
                        .selectable_label(
                            self.data_model.view.camera_mask_mode == mode,
                            mode.name(),
                        )
                        .clicked()
                    {
                        self.data_model.view.camera_mask_mode = mode;
                        ui.close();
                    }
                }
            });

        ui.label("Selection highlight:");
        egui::ComboBox::from_id_salt("selection_highlight")
            .width(140.0)
            .selected_text(self.data_model.view.sel_highlight_mode.name())
            .show_ui(ui, |ui| {
                for mode in [
                    SelectionHighlightMode::Never,
                    SelectionHighlightMode::OnlyWhenPaused,
                    SelectionHighlightMode::Always,
                ] {
                    if ui
                        .selectable_label(
                            self.data_model.view.sel_highlight_mode == mode,
                            mode.name(),
                        )
                        .clicked()
                    {
                        self.data_model.view.sel_highlight_mode = mode;
                        ui.close();
                    }
                }
            });

        ui.separator();
        ui.label("Clear color:");
        egui::ComboBox::from_id_salt("clear_color")
            .width(160.0)
            .selected_text(self.data_model.view.clear_color.name())
            .show_ui(ui, |ui| {
                for color in [
                    ClearColor::Black,
                    ClearColor::DarkGrey,
                    ClearColor::LightGrey,
                    ClearColor::White,
                ] {
                    if ui
                        .selectable_label(self.data_model.view.clear_color == color, color.name())
                        .clicked()
                    {
                        self.data_model.view.clear_color = color;
                        ui.close();
                    }
                }
            });

        ui.label("Highlight color:");
        egui::ComboBox::from_id_salt("highlight_color")
            .width(160.0)
            .selected_text(self.data_model.view.highlight_color.name())
            .show_ui(ui, |ui| {
                for color in [
                    HighlightColor::White,
                    HighlightColor::Yellow,
                    HighlightColor::Cyan,
                ] {
                    if ui
                        .selectable_label(
                            self.data_model.view.highlight_color == color,
                            color.name(),
                        )
                        .clicked()
                    {
                        self.data_model.view.highlight_color = color;
                        ui.close();
                    }
                }
            });
    }

    /// Spline / animation curve viewer panel.
    pub fn ui_spline_viewer(&mut self, ui: &mut egui::Ui) {
        spline_viewer::ui_spline_viewer(ui, &self.data_model, &mut self.spline_state);
    }
}
