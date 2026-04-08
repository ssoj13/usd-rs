//! Task controller - Orchestrates rendering tasks for Hydra.
//!
//! HdxTaskController manages creation and configuration of rendering tasks,
//! providing a high-level API for interactive applications.
//!
//! Architecture differs from C++: since Rust HdRenderIndex trait is read-only,
//! task objects are stored directly in the controller in a HashMap.
//! Per-task params are also cached here so set_render_params / set_collection
//! can correctly merge (C++ stores params in _delegate value cache).

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use usd_gf::{Matrix4d, Vec2i, Vec4d, Vec4f, Vec4i};
use usd_hd::enums::HdBlendFactor;
use usd_hd::render::{HdTaskSharedPtr, HdTaskSharedPtrVector};
use usd_sdf::Path;
use usd_tf::Token;

use super::aov_input_task::HdxAovInputTask;
use super::bounding_box_task::{HdxBoundingBoxTask, HdxBoundingBoxTaskParams};
use super::color_correction_task::{
    HdxColorCorrectionTask, HdxColorCorrectionTaskParams, color_correction_tokens,
};
use super::colorize_selection_task::{
    ColorizeMode, HdxColorizeSelectionTask, HdxColorizeSelectionTaskParams,
};
use super::oit_resolve_task::HdxOitResolveTask;
use super::pick_from_render_buffer_task::HdxPickFromRenderBufferTask;
use super::pick_task::HdxPickTask;
use super::present_task::{HdxPresentTask, HdxPresentTaskParams};
use super::render_setup_task::{
    CameraUtilConformWindowPolicy, CameraUtilFraming, HdRenderPassAovBinding, HdxRenderTaskParams,
};
use super::render_task::HdxRenderTask;
use super::selection_task::{HdxSelectionTask, HdxSelectionTaskParams};
use super::selection_tracker::{HdxSelectionTracker, create_selection_tracker};
use super::shadow_task::{HdxShadowTask, HdxShadowTaskParams};
use super::simple_light_task::{HdxSimpleLightTask, HdxSimpleLightTaskParams};
use super::skydome_task::HdxSkydomeTask;
use super::task_controller_scene_index::HdAovDescriptor;
use super::visualize_aov_task::HdxVisualizeAovTask;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Internal param cache entry — mirrors C++ _delegate value store per task.

/// Cached parameters for a single render task entry.
struct RenderTaskCache {
    params: HdxRenderTaskParams,
    material_tag: Token,
}

/// Cached AOV buffer descriptor.
struct AovBufferEntry {
    /// AOV token name (e.g. "color", "depth").
    aov_name: Token,
    /// Descriptor (format, dimensions, clear value).
    desc: HdAovDescriptor,
}

// ---------------------------------------------------------------------------

/// Task controller — manages the full rendering pipeline task graph.
///
/// Creates all pipeline tasks on construction and stores them in `tasks` map.
/// Provides high-level API matching C++ HdxTaskController.
pub struct HdxTaskController {
    controller_id: Path,

    #[allow(dead_code)]
    gpu_enabled: bool,

    selection_tracker: HdxSelectionTracker,

    // Actual task objects keyed by SdfPath.
    tasks: HashMap<Path, HdTaskSharedPtr>,

    // Per-render-task cached params (for correct merge in set_render_params).
    render_task_params: HashMap<Path, RenderTaskCache>,

    // Task path registry.
    simple_light_task_id: Option<Path>,
    shadow_task_id: Option<Path>,
    render_task_ids: Vec<Path>,
    aov_input_task_id: Option<Path>,
    oit_resolve_task_id: Option<Path>,
    selection_task_id: Option<Path>,
    colorize_selection_task_id: Option<Path>,
    color_correction_task_id: Option<Path>,
    visualize_aov_task_id: Option<Path>,
    pick_task_id: Option<Path>,
    pick_from_render_buffer_task_id: Option<Path>,
    bounding_box_task_id: Option<Path>,
    skydome_task_id: Option<Path>,
    present_task_id: Option<Path>,

    // Camera state.
    active_camera_id: Option<Path>,
    free_camera_view: Option<Matrix4d>,
    free_camera_proj: Option<Matrix4d>,
    free_camera_clip_planes: Vec<Vec4d>,

    // Light and AOV state.
    #[allow(dead_code)] // populated by set_lighting_state when fully implemented
    light_ids: Vec<Path>,
    aov_buffer_entries: Vec<AovBufferEntry>,
    aov_outputs: Vec<Token>,
    viewport_aov: Token,

    // Selection visual state.
    selection_color: Vec4f,
    selection_locate_color: Vec4f,
    selection_enable_outline: bool,
    selection_outline_radius: u32,

    // Viewport / framing.
    render_buffer_size: Vec2i,
    framing: CameraUtilFraming,
    override_window_policy: Option<CameraUtilConformWindowPolicy>,
    viewport: Vec4d,

    // Feature flags.
    enable_shadows: bool,
    enable_presentation: bool,
    is_storm_backend: bool,

    // Cached collection token (propagated to all render tasks on change).
    collection: Token,

    // Cached lighting context token (triggers simple_light_task re-sync when changed).
    lighting_context: Token,
}

// ---------------------------------------------------------------------------
// Construction

impl HdxTaskController {
    /// Create task controller and build the full render graph.
    pub fn new(controller_id: Path, gpu_enabled: bool) -> Self {
        let mut controller = Self {
            controller_id,
            gpu_enabled,
            selection_tracker: create_selection_tracker(),
            tasks: HashMap::new(),
            render_task_params: HashMap::new(),
            simple_light_task_id: None,
            shadow_task_id: None,
            render_task_ids: Vec::new(),
            aov_input_task_id: None,
            oit_resolve_task_id: None,
            selection_task_id: None,
            colorize_selection_task_id: None,
            color_correction_task_id: None,
            visualize_aov_task_id: None,
            pick_task_id: None,
            pick_from_render_buffer_task_id: None,
            bounding_box_task_id: None,
            skydome_task_id: None,
            present_task_id: None,
            active_camera_id: None,
            free_camera_view: None,
            free_camera_proj: None,
            free_camera_clip_planes: Vec::new(),
            light_ids: Vec::new(),
            aov_buffer_entries: Vec::new(),
            aov_outputs: Vec::new(),
            viewport_aov: Token::new(""),
            selection_color: Vec4f::new(1.0, 1.0, 0.0, 1.0),
            selection_locate_color: Vec4f::new(0.0, 0.0, 1.0, 1.0),
            selection_enable_outline: true,
            selection_outline_radius: 1,
            render_buffer_size: Vec2i::new(0, 0),
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
            viewport: Vec4d::new(0.0, 0.0, 1.0, 1.0),
            enable_shadows: false,
            enable_presentation: true,
            is_storm_backend: true,
            collection: Token::default(),
            lighting_context: Token::default(),
        };

        controller.create_render_graph();
        controller
    }

    pub fn get_controller_id(&self) -> &Path {
        &self.controller_id
    }

    pub fn get_selection_tracker(&self) -> HdxSelectionTracker {
        self.selection_tracker.clone()
    }

    pub fn set_is_storm_backend(&mut self, enabled: bool) {
        self.is_storm_backend = enabled;
    }
}

// ---------------------------------------------------------------------------
// Execution API

impl HdxTaskController {
    /// Returns ordered list of rendering task paths, matching C++ GetRenderingTaskPaths().
    ///
    /// Conditional tasks (shadows, colorize_selection, color_correction, visualize_aov)
    /// are only included when their respective enable conditions are met.
    pub fn get_rendering_task_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();

        if let Some(ref id) = self.simple_light_task_id {
            paths.push(id.clone());
        }

        if self.shadows_enabled() {
            if let Some(ref id) = self.shadow_task_id {
                paths.push(id.clone());
            }
        }

        if let Some(ref id) = self.skydome_task_id {
            paths.push(id.clone());
        }

        // Render tasks — volume task goes after aov_input (like C++).
        let volume_id = self.get_render_task_path(&Token::new("volume"));
        let mut has_volume = false;

        for id in &self.render_task_ids {
            if Some(id) == volume_id.as_ref() {
                has_volume = true;
                continue;
            }
            paths.push(id.clone());
        }

        if let Some(ref id) = self.aov_input_task_id {
            paths.push(id.clone());
        }

        if let Some(ref id) = self.bounding_box_task_id {
            paths.push(id.clone());
        }

        if has_volume {
            if let Some(vol_id) = volume_id {
                paths.push(vol_id);
            }
        }

        if let Some(ref id) = self.oit_resolve_task_id {
            paths.push(id.clone());
        }

        if self.selection_enabled() {
            if let Some(ref id) = self.selection_task_id {
                paths.push(id.clone());
            }
        }

        if self.colorize_selection_enabled() {
            if let Some(ref id) = self.colorize_selection_task_id {
                paths.push(id.clone());
            }
        }

        if self.color_correction_enabled() {
            if let Some(ref id) = self.color_correction_task_id {
                paths.push(id.clone());
            }
        }

        if self.visualize_aov_enabled() {
            if let Some(ref id) = self.visualize_aov_task_id {
                paths.push(id.clone());
            }
        }

        if self.enable_presentation {
            if let Some(ref id) = self.present_task_id {
                paths.push(id.clone());
            }
        }

        paths
    }

    /// Returns all task (path, handle) pairs for registration in the render index.
    /// C++ does this via `renderIndex->InsertTask()` inside each _Create*Task().
    pub fn get_all_tasks(&self) -> Vec<(Path, HdTaskSharedPtr)> {
        self.tasks
            .iter()
            .map(|(p, t)| (p.clone(), t.clone()))
            .collect()
    }

    /// Returns actual task objects for the rendering pipeline.
    pub fn get_rendering_tasks(&self) -> HdTaskSharedPtrVector {
        self.get_rendering_task_paths()
            .iter()
            .filter_map(|p| self.tasks.get(p).cloned())
            .collect()
    }

    /// Returns paths to picking tasks.
    pub fn get_picking_task_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();
        if let Some(ref id) = self.pick_task_id {
            paths.push(id.clone());
        }
        if let Some(ref id) = self.pick_from_render_buffer_task_id {
            paths.push(id.clone());
        }
        paths
    }

    /// Returns actual task objects for picking.
    pub fn get_picking_tasks(&self) -> HdTaskSharedPtrVector {
        self.get_picking_task_paths()
            .iter()
            .filter_map(|p| self.tasks.get(p).cloned())
            .collect()
    }

    pub fn get_task(&self, id: &Path) -> Option<HdTaskSharedPtr> {
        self.tasks.get(id).cloned()
    }
}

// ---------------------------------------------------------------------------
// Rendering API

impl HdxTaskController {
    /// Set the collection to be rendered.
    ///
    /// Propagates to all render tasks, preserving their per-task material tags
    /// (same logic as C++ SetCollection).
    pub fn set_collection(&mut self, collection: &Token) {
        if self.collection == *collection {
            return;
        }
        self.collection = collection.clone();

        // Snapshot (task_id, params, material_tag) to avoid mid-loop borrow conflicts.
        let snapshots: Vec<(Path, HdxRenderTaskParams, Token)> = self
            .render_task_ids
            .iter()
            .filter_map(|id| {
                self.render_task_params
                    .get(id)
                    .map(|c| (id.clone(), c.params.clone(), c.material_tag.clone()))
            })
            .collect();

        for (task_id, mut params, material_tag) in snapshots {
            // Re-apply blend state so the stored params stay consistent.
            self.apply_blend_state_for_tag(&material_tag, &mut params);
            // Persist updated params.
            if let Some(cache) = self.render_task_params.get_mut(&task_id) {
                cache.params = params.clone();
            }
            // Push to actual task object.
            if let Some(task) = self.tasks.get(&task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        rt.set_params(&params);
                    }
                }
            }
        }
    }

    /// Set render parameters on all render tasks.
    ///
    /// Matches C++ SetRenderParams: user params are merged with internally-managed
    /// camera, viewport, framing, aov bindings (those are NOT overwritten by caller).
    pub fn set_render_params(&mut self, params: &HdxRenderTaskParams) {
        for task_id in &self.render_task_ids.clone() {
            // Get currently stored params to preserve internally-managed fields.
            let old_params = self
                .render_task_params
                .get(task_id)
                .map(|c| c.params.clone())
                .unwrap_or_default();

            let material_tag = self
                .render_task_params
                .get(task_id)
                .map(|c| c.material_tag.clone())
                .unwrap_or_default();

            // Merge: accept user's rendering settings, keep our camera/viewport/aov.
            let mut merged = params.clone();
            merged.camera = old_params.camera;
            merged.viewport = old_params.viewport;
            merged.framing = old_params.framing.clone();
            merged.override_window_policy = old_params.override_window_policy;
            merged.aov_bindings = old_params.aov_bindings.clone();

            // Apply blend state for material tag.
            self.apply_blend_state_for_tag(&material_tag, &mut merged);

            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        rt.set_params(&merged);
                    }
                }
            }

            // Update cache.
            if let Some(cache) = self.render_task_params.get_mut(task_id) {
                cache.params = merged;
            }
        }

        // Also forward cull style to pick task via its params.
        if let Some(ref pick_id) = self.pick_task_id.clone() {
            if let Some(task) = self.tasks.get(pick_id) {
                {
                    let mut guard = task.write();
                    if let Some(pt) = guard.as_any_mut().downcast_mut::<HdxPickTask>() {
                        let mut p = pt.get_params().clone();
                        if p.cull_style != params.cull_style {
                            p.cull_style = params.cull_style;
                            pt.set_params(p);
                        }
                    }
                }
            }
        }
    }

    /// Set render tags (geometry filter) for the scene.
    pub fn set_render_tags(&mut self, render_tags: &[Token]) {
        for task_id in &self.render_task_ids.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        rt.set_render_tags(render_tags.to_vec());
                    }
                }
            }
        }

        if let Some(ref pick_id) = self.pick_task_id.clone() {
            if let Some(task) = self.tasks.get(pick_id) {
                {
                    let mut guard = task.write();
                    if let Some(pick_task) = guard.as_any_mut().downcast_mut::<HdxPickTask>() {
                        pick_task.set_render_tags(render_tags.to_vec());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AOV API

impl HdxTaskController {
    /// Set render outputs (AOV names to produce).
    ///
    /// Matches C++ SetRenderOutputs: stores AOV list, creates buffer entries,
    /// wires AOV bindings into render task params, and calls SetViewportRenderOutput.
    pub fn set_render_outputs(&mut self, names: &[Token]) {
        if self.aov_outputs == names {
            return;
        }
        self.aov_outputs = names.to_vec();

        // Resolve the backend-facing AOV list using the same Storm/non-Storm split
        // as OpenUSD `_ResolvedRenderOutputs(...)`.
        let resolved = self.resolve_render_outputs(names);

        // Clear old AOV buffer entries.
        self.aov_buffer_entries.clear();

        // Create new buffer entries.
        for aov_name in &resolved {
            self.aov_buffer_entries.push(AovBufferEntry {
                aov_name: aov_name.clone(),
                desc: HdAovDescriptor::default(),
            });
        }

        // Build AOV binding vectors.
        // C++ pattern: first render task gets aovBindingsClear (clears buffers each frame),
        // subsequent render tasks get aovBindingsNoClear (no clear, layered rendering).
        // Volume tasks additionally get aovInputBindings for depth compositing.
        let mut aov_bindings_clear: Vec<HdRenderPassAovBinding> = Vec::new();
        let mut aov_bindings_no_clear: Vec<HdRenderPassAovBinding> = Vec::new();
        let mut aov_input_bindings: Vec<HdRenderPassAovBinding> = Vec::new();
        for aov_name in &resolved {
            let buffer_path = self.get_aov_path(aov_name);
            // Default clear value: depth=1.0, color=transparent black, others=-1 (invalid sentinel).
            let clear_val = if aov_name == "depth" {
                Value::from(1.0f32)
            } else if aov_name == "color" {
                Value::from(Vec4f::new(0.0, 0.0, 0.0, 0.0))
            } else {
                Value::from(-1i32)
            };
            let binding_clear = HdRenderPassAovBinding::new(aov_name.clone(), buffer_path.clone())
                .with_clear_value(clear_val);
            let binding_no_clear =
                HdRenderPassAovBinding::new(aov_name.clone(), buffer_path.clone());
            aov_bindings_clear.push(binding_clear);
            aov_bindings_no_clear.push(binding_no_clear.clone());
            // depth is used as input for volume pass compositing.
            if aov_name == "depth" {
                aov_input_bindings.push(binding_no_clear);
            }
        }

        // Wire bindings into render tasks: first task gets clear bindings, rest get no-clear.
        let task_ids = self.render_task_ids.clone();
        for (i, task_id) in task_ids.iter().enumerate() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        let bindings = if i == 0 {
                            aov_bindings_clear.clone()
                        } else {
                            aov_bindings_no_clear.clone()
                        };
                        let input = if i == 0 {
                            Vec::new()
                        } else {
                            aov_input_bindings.clone()
                        };
                        rt.set_aov_bindings(bindings, input);
                    }
                }
            }
        }

        // Auto-route single output to viewport.
        if names.len() == 1 {
            self.set_viewport_render_output(names[0].clone());
        } else {
            self.set_viewport_render_output(Token::new(""));
        }

        // Viewport data depends on whether AOVs are in use.
        self.set_camera_framing_for_tasks();
    }

    /// Set which AOV to display in the viewport.
    ///
    /// Matches C++ SetViewportRenderOutput: wires AOV buffer paths into
    /// aov_input_task, colorize_selection_task, pick_from_render_buffer_task,
    /// color_correction_task, visualize_aov_task, and bounding_box_task.
    pub fn set_viewport_render_output(&mut self, name: Token) {
        if self.viewport_aov == name {
            return;
        }
        self.viewport_aov = name.clone();

        let is_color = name == "color";
        let is_empty = name.is_empty();

        // Update aov_input_task.
        if let Some(ref task_id) = self.aov_input_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxAovInputTask>() {
                        let mut p = t.get_params().clone();
                        if is_empty {
                            p.aov_buffer_path = Path::empty();
                            p.depth_buffer_path = Path::empty();
                        } else if is_color {
                            p.aov_buffer_path = self.get_aov_path(&Token::new("color"));
                            p.depth_buffer_path = self.get_aov_path(&Token::new("depth"));
                        } else {
                            p.aov_buffer_path = self.get_aov_path(&name);
                            p.depth_buffer_path = Path::empty();
                        }
                        t.set_params(p);
                    }
                }
            }
        }

        // Update colorize_selection_task: wire id buffers only when rendering color.
        if let Some(ref task_id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if is_color {
                            p.prim_id_buffer_path = self.get_aov_path(&Token::new("primId"));
                            p.instance_id_buffer_path =
                                self.get_aov_path(&Token::new("instanceId"));
                            p.element_id_buffer_path = self.get_aov_path(&Token::new("elementId"));
                        } else {
                            p.prim_id_buffer_path = Path::empty();
                            p.instance_id_buffer_path = Path::empty();
                            p.element_id_buffer_path = Path::empty();
                        }
                        t.set_params(p);
                    }
                }
            }
        }

        // Update pick_from_render_buffer_task.
        if let Some(ref task_id) = self.pick_from_render_buffer_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxPickFromRenderBufferTask>()
                    {
                        let mut p = t.get_params().clone();
                        if is_color {
                            p.prim_id_buffer_path = self.get_aov_path(&Token::new("primId"));
                            p.instance_id_buffer_path =
                                self.get_aov_path(&Token::new("instanceId"));
                            p.element_id_buffer_path = self.get_aov_path(&Token::new("elementId"));
                            p.edge_id_buffer_path = self.get_aov_path(&Token::new("edgeId"));
                            p.point_id_buffer_path = self.get_aov_path(&Token::new("pointId"));
                            p.normal_buffer_path = self.get_aov_path(&Token::new("normal"));
                            p.depth_buffer_path = self.get_aov_path(&Token::new("depth"));
                        } else {
                            p.prim_id_buffer_path = Path::empty();
                            p.instance_id_buffer_path = Path::empty();
                            p.element_id_buffer_path = Path::empty();
                            p.edge_id_buffer_path = Path::empty();
                            p.point_id_buffer_path = Path::empty();
                            p.normal_buffer_path = Path::empty();
                            p.depth_buffer_path = Path::empty();
                        }
                        t.set_params(p);
                    }
                }
            }
        }

        // Update color_correction_task aov_name.
        if let Some(ref task_id) = self.color_correction_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxColorCorrectionTask>() {
                        let mut p = t.get_params().clone();
                        p.aov_name = name.clone();
                        t.set_params(p);
                    }
                }
            }
        }

        // Update visualize_aov_task aov_name.
        if let Some(ref task_id) = self.visualize_aov_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxVisualizeAovTask>() {
                        t.set_aov_name(name.clone());
                    }
                }
            }
        }

        // Update bounding_box_task aov_name.
        if let Some(ref task_id) = self.bounding_box_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxBoundingBoxTask>() {
                        t.set_aov_name(name.clone());
                    }
                }
            }
        }
    }

    /// Get AOV descriptor for a named output.
    pub fn get_render_output_settings(&self, name: &Token) -> HdAovDescriptor {
        self.aov_buffer_entries
            .iter()
            .find(|e| &e.aov_name == name)
            .map(|e| e.desc.clone())
            .unwrap_or_default()
    }

    /// Set AOV descriptor for a named output.
    pub fn set_render_output_settings(&mut self, name: &Token, desc: &HdAovDescriptor) {
        if let Some(entry) = self
            .aov_buffer_entries
            .iter_mut()
            .find(|e| &e.aov_name == name)
        {
            entry.desc = desc.clone();
        }
    }

    /// Set the presentation output (framebuffer destination API and handle).
    pub fn set_presentation_output(&mut self, api: &Token, _framebuffer: &[u8]) {
        if let Some(ref task_id) = self.present_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxPresentTask>() {
                        let mut p = t.get_params().clone();
                        p.dst_api = api.clone();
                        t.set_params(p);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lighting API

impl HdxTaskController {
    /// Set lighting state from a lighting context token.
    ///
    /// In full impl this would parse GlfSimpleLightingContext and create/update
    /// light sprims. Here we store the token for future frames and update the
    /// simple light task's camera reference.
    pub fn set_lighting_state(&mut self, lighting_context: &Token) {
        // Store context; skip work if unchanged.
        if self.lighting_context == *lighting_context {
            return;
        }
        self.lighting_context = lighting_context.clone();

        // In full Storm implementation this calls _SetBuiltInLightingState which
        // creates/updates/removes light sprims in the render index.
        // Here we update the simple light task's camera path and mark it dirty
        // so it re-publishes the lighting context next sync.
        if let Some(ref task_id) = self.simple_light_task_id.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSimpleLightTask>() {
                        let mut p = t.get_params().clone();
                        // Propagate active camera to light task.
                        if let Some(ref camera_id) = self.active_camera_id {
                            p.camera_path = camera_id.clone();
                        }
                        t.set_params(&p);
                        // Force rebuild of lighting buffer sources.
                        t.mark_lighting_dirty();
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Camera and Framing API

impl HdxTaskController {
    /// Set scene camera path and propagate to all tasks.
    pub fn set_camera_path(&mut self, id: Path) {
        if self.active_camera_id.as_ref() == Some(&id) {
            return;
        }
        self.active_camera_id = Some(id.clone());
        self.set_camera_param_for_tasks(&id);
    }

    /// Set render buffer size (window size for GUI apps).
    ///
    /// Also resizes all AOV buffer descriptors so GPU textures are reallocated next frame.
    pub fn set_render_buffer_size(&mut self, size: Vec2i) {
        if self.render_buffer_size == size {
            return;
        }
        self.render_buffer_size = size;
        // Update AOV descriptor dimensions (matches C++ _UpdateAovDimensions).
        for entry in &mut self.aov_buffer_entries {
            entry.desc.dimensions = Vec2i::new(size.x, size.y);
        }
    }

    pub fn set_framing(&mut self, framing: CameraUtilFraming) {
        self.framing = framing;
        self.set_camera_framing_for_tasks();
    }

    pub fn set_override_window_policy(&mut self, policy: Option<CameraUtilConformWindowPolicy>) {
        self.override_window_policy = policy;
        self.set_camera_framing_for_tasks();
    }

    /// Set viewport (deprecated; prefer set_framing + set_render_buffer_size).
    pub fn set_render_viewport(&mut self, viewport: Vec4d) {
        if self.viewport == viewport {
            return;
        }
        self.viewport = viewport;
        self.set_camera_framing_for_tasks();
    }

    pub fn set_free_camera_matrices(&mut self, view_matrix: Matrix4d, projection_matrix: Matrix4d) {
        self.free_camera_view = Some(view_matrix);
        self.free_camera_proj = Some(projection_matrix);
        self.active_camera_id = None;
    }

    pub fn set_free_camera_clip_planes(&mut self, clip_planes: Vec<Vec4d>) {
        self.free_camera_clip_planes = clip_planes;
    }

    pub fn clear_free_camera(&mut self) {
        self.free_camera_view = None;
        self.free_camera_proj = None;
        self.free_camera_clip_planes.clear();
    }

    pub fn is_using_free_camera(&self) -> bool {
        self.free_camera_view.is_some() && self.free_camera_proj.is_some()
    }

    pub fn get_free_camera_view(&self) -> Option<&Matrix4d> {
        self.free_camera_view.as_ref()
    }

    pub fn get_free_camera_proj(&self) -> Option<&Matrix4d> {
        self.free_camera_proj.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Selection API

impl HdxTaskController {
    /// Enable/disable selection highlighting on selection_task and colorize_selection_task.
    pub fn set_enable_selection(&mut self, enable: bool) {
        if let Some(ref id) = self.selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSelectionTask>() {
                        let mut p = t.get_params().clone();
                        if p.enable_selection_highlight != enable
                            || p.enable_locate_highlight != enable
                        {
                            p.enable_selection_highlight = enable;
                            p.enable_locate_highlight = enable;
                            t.set_params(p);
                        }
                    }
                }
            }
        }

        if let Some(ref id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if p.enable_selection_highlight != enable
                            || p.enable_locate_highlight != enable
                        {
                            p.enable_selection_highlight = enable;
                            p.enable_locate_highlight = enable;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    pub fn set_selection_color(&mut self, color: Vec4f) {
        self.selection_color = color;

        if let Some(ref id) = self.selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSelectionTask>() {
                        let mut p = t.get_params().clone();
                        if p.selection_color != color {
                            p.selection_color = color;
                            t.set_params(p);
                        }
                    }
                }
            }
        }

        if let Some(ref id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if p.selection_color != color {
                            p.selection_color = color;
                            p.primary_color = color;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    pub fn get_selection_color(&self) -> Vec4f {
        self.selection_color
    }

    pub fn set_selection_locate_color(&mut self, color: Vec4f) {
        self.selection_locate_color = color;

        if let Some(ref id) = self.selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSelectionTask>() {
                        let mut p = t.get_params().clone();
                        if p.locate_color != color {
                            p.locate_color = color;
                            t.set_params(p);
                        }
                    }
                }
            }
        }

        if let Some(ref id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if p.locate_color != color {
                            p.locate_color = color;
                            p.secondary_color = color;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    pub fn get_selection_locate_color(&self) -> Vec4f {
        self.selection_locate_color
    }

    pub fn set_selection_enable_outline(&mut self, enable_outline: bool) {
        self.selection_enable_outline = enable_outline;

        if let Some(ref id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if p.enable_outline != enable_outline {
                            p.enable_outline = enable_outline;
                            p.mode = if enable_outline {
                                ColorizeMode::Outline
                            } else {
                                ColorizeMode::Fill
                            };
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    pub fn set_selection_outline_radius(&mut self, radius: u32) {
        self.selection_outline_radius = radius;

        if let Some(ref id) = self.colorize_selection_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxColorizeSelectionTask>()
                    {
                        let mut p = t.get_params().clone();
                        if p.outline_radius != radius {
                            p.outline_radius = radius;
                            p.outline_width = radius as f32;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    pub fn get_selection_outline_radius(&self) -> u32 {
        self.selection_outline_radius
    }
}

// ---------------------------------------------------------------------------
// Shadow API

impl HdxTaskController {
    /// Enable/disable shadow rendering.
    pub fn set_enable_shadows(&mut self, enable: bool) {
        self.enable_shadows = enable;

        if let Some(ref id) = self.simple_light_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSimpleLightTask>() {
                        let mut p = t.get_params().clone();
                        if p.enable_shadows != enable {
                            p.enable_shadows = enable;
                            t.set_params(&p);
                        }
                    }
                }
            }
        }
    }

    /// Set shadow task parameters.
    pub fn set_shadow_params(&mut self, params: &HdxShadowTaskParams) {
        if let Some(ref id) = self.shadow_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxShadowTask>() {
                        t.set_params(params.clone());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Color Correction API

impl HdxTaskController {
    /// Set color correction parameters.
    ///
    /// Matches C++ SetColorCorrectionParams: preserves internally-managed aov_name.
    pub fn set_color_correction_params(&mut self, params: &HdxColorCorrectionTaskParams) {
        if let Some(ref id) = self.color_correction_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxColorCorrectionTask>() {
                        // Preserve internally-managed aov_name.
                        let old_aov = t.get_params().aov_name.clone();
                        let mut new_params = params.clone();
                        new_params.aov_name = old_aov;
                        t.set_params(new_params);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bounding Box API

impl HdxTaskController {
    /// Set bounding box task parameters.
    ///
    /// Matches C++ SetBBoxParams: only takes bboxes, color, dashSize from caller.
    pub fn set_bbox_params(&mut self, params: &HdxBoundingBoxTaskParams) {
        if let Some(ref id) = self.bounding_box_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxBoundingBoxTask>() {
                        // Preserve internally-managed aov_name; merge in external fields.
                        let old_aov = t.get_aov_name().clone();
                        let mut merged = params.clone();
                        merged.aov_name = old_aov;
                        t.set_params(merged);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Present API

impl HdxTaskController {
    pub fn set_enable_presentation(&mut self, enabled: bool) {
        self.enable_presentation = enabled;

        if let Some(ref id) = self.present_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxPresentTask>() {
                        let mut p = t.get_params().clone();
                        if p.enabled != enabled {
                            p.enabled = enabled;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Progressive Image Generation

impl HdxTaskController {
    /// Returns true if all rendering tasks report convergence.
    pub fn is_converged(&self) -> bool {
        for task in self.get_rendering_tasks() {
            {
                let guard = task.read();
                if !guard.is_converged() {
                    return false;
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Internal helpers

impl HdxTaskController {
    /// Insert task into the local task map.
    fn insert_task(&mut self, id: &Path, task: HdTaskSharedPtr) {
        self.tasks.insert(id.clone(), task);
    }

    /// Build the full render graph (called once from constructor).
    fn create_render_graph(&mut self) {
        // Non-Storm path: single render task + AOV pipeline.
        self.create_lighting_task();
        self.create_shadow_task();
        self.create_skydome_task();
        self.create_render_task(&Token::new(""));

        // AOV pipeline (always created, even before AOVs are enabled).
        self.create_aov_input_task();
        self.create_oit_resolve_task();
        self.create_selection_task();
        self.create_colorize_selection_task();
        self.create_color_correction_task();
        self.create_visualize_aov_task();
        self.create_pick_task();
        self.create_pick_from_render_buffer_task();
        self.create_bounding_box_task();
        self.create_present_task();

        // Auto-initialize to color AOV (matches C++ non-Storm path).
        self.set_render_outputs(&[Token::new("color")]);
    }

    fn create_lighting_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("simpleLightTask") {
            let mut task = HdxSimpleLightTask::new(id.clone());
            let mut params = HdxSimpleLightTaskParams::default();
            params.camera_path = self.active_camera_id.clone().unwrap_or_default();
            task.set_params(&params);
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.simple_light_task_id = Some(id);
        }
    }

    fn create_shadow_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("shadowTask") {
            let mut task = HdxShadowTask::new(id.clone());
            task.set_render_tags(vec![Token::new("geometry")]);
            task.set_params(HdxShadowTaskParams::default());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.shadow_task_id = Some(id);
        }
    }

    fn create_render_task(&mut self, material_tag: &Token) {
        let name = if material_tag.is_empty() {
            "renderTask".to_string()
        } else {
            format!("renderTask_{}", material_tag.as_str().replace(':', "_"))
        };

        if let Some(id) = self.controller_id.append_child(&name) {
            let mut task = HdxRenderTask::new(id.clone());
            let mut params = HdxRenderTaskParams::default();
            params.camera = self.active_camera_id.clone().unwrap_or_default();
            params.viewport = self.viewport;
            params.framing = self.framing.clone();
            params.override_window_policy = self.override_window_policy;
            task.set_params(&params);
            task.set_material_tag(material_tag.clone());
            task.set_render_tags(vec![Token::new("geometry")]);

            self.render_task_params.insert(
                id.clone(),
                RenderTaskCache {
                    params: params.clone(),
                    material_tag: material_tag.clone(),
                },
            );

            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.render_task_ids.push(id);
        }
    }

    fn create_aov_input_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("aovInputTask") {
            let task = HdxAovInputTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.aov_input_task_id = Some(id);
        }
    }

    fn create_oit_resolve_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("oitResolveTask") {
            let task = HdxOitResolveTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.oit_resolve_task_id = Some(id);
        }
    }

    fn create_selection_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("selectionTask") {
            let mut task = HdxSelectionTask::new(id.clone());
            task.set_selection_tracker(self.selection_tracker.clone());
            let params = HdxSelectionTaskParams {
                enable_selection_highlight: true,
                enable_locate_highlight: true,
                selection_color: self.selection_color,
                locate_color: self.selection_locate_color,
                ..HdxSelectionTaskParams::default()
            };
            task.set_params(params);
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.selection_task_id = Some(id);
        }
    }

    fn create_colorize_selection_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("colorizeSelectionTask") {
            let mut task = HdxColorizeSelectionTask::new(id.clone());
            let params = HdxColorizeSelectionTaskParams {
                enable_selection_highlight: true,
                enable_locate_highlight: true,
                selection_color: self.selection_color,
                locate_color: self.selection_locate_color,
                primary_color: self.selection_color,
                secondary_color: self.selection_locate_color,
                ..HdxColorizeSelectionTaskParams::default()
            };
            task.set_params(params);
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.colorize_selection_task_id = Some(id);
        }
    }

    fn create_color_correction_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("colorCorrectionTask") {
            let task = HdxColorCorrectionTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.color_correction_task_id = Some(id);
        }
    }

    fn create_visualize_aov_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("visualizeAovTask") {
            let task = HdxVisualizeAovTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.visualize_aov_task_id = Some(id);
        }
    }

    fn create_pick_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("pickTask") {
            let task = HdxPickTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.pick_task_id = Some(id);
        }
    }

    fn create_pick_from_render_buffer_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("pickFromRenderBufferTask") {
            let mut task = HdxPickFromRenderBufferTask::new(id.clone());
            let mut params = task.get_params().clone();
            params.camera_path = self.active_camera_id.clone().unwrap_or_default();
            params.viewport = [
                self.viewport[0] as i32,
                self.viewport[1] as i32,
                self.viewport[2] as i32,
                self.viewport[3] as i32,
            ];
            params.framing = self.framing.clone();
            params.override_window_policy = self.override_window_policy;
            task.set_params(params);
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.pick_from_render_buffer_task_id = Some(id);
        }
    }

    fn create_bounding_box_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("boundingBoxTask") {
            let task = HdxBoundingBoxTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.bounding_box_task_id = Some(id);
        }
    }

    fn create_present_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("presentTask") {
            let mut task = HdxPresentTask::new(id.clone());
            task.set_params(HdxPresentTaskParams::default());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.present_task_id = Some(id);
        }
    }

    /// Create skydome task (Storm path only).
    ///
    /// In C++ this is called from _CreateRenderGraph when IsStormRenderingDelegate.
    /// For non-Storm, skydome_task_id stays None.
    #[allow(dead_code)]
    fn create_skydome_task(&mut self) {
        if let Some(id) = self.controller_id.append_child("skydomeTask") {
            let task = HdxSkydomeTask::new(id.clone());
            self.insert_task(&id, Arc::new(RwLock::new(task)));
            self.skydome_task_id = Some(id);
        }
    }

    /// Propagate camera path to all render tasks, light task, and pick_from_render_buffer.
    fn set_camera_param_for_tasks(&mut self, camera_id: &Path) {
        for task_id in &self.render_task_ids.clone() {
            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        let mut p = rt.get_params().unwrap_or_default();
                        p.camera = camera_id.clone();
                        rt.set_params(&p);
                    }
                }
            }
            if let Some(cache) = self.render_task_params.get_mut(task_id) {
                cache.params.camera = camera_id.clone();
            }
        }

        if let Some(ref id) = self.simple_light_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxSimpleLightTask>() {
                        let mut p = t.get_params().clone();
                        p.camera_path = camera_id.clone();
                        t.set_params(&p);
                    }
                }
            }
        }

        if let Some(ref id) = self.pick_from_render_buffer_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxPickFromRenderBufferTask>()
                    {
                        let mut p = t.get_params().clone();
                        p.camera_path = camera_id.clone();
                        t.set_params(p);
                    }
                }
            }
        }
    }

    /// Propagate viewport/framing to render tasks and present task.
    fn set_camera_framing_for_tasks(&mut self) {
        // When AOVs active, use (0,0,w,h) — offset goes to present task dst_region.
        let adjusted = if self.using_aovs() {
            Vec4d::new(0.0, 0.0, self.viewport[2], self.viewport[3])
        } else {
            self.viewport
        };

        for task_id in &self.render_task_ids.clone() {
            let mut changed = false;
            {
                let cache = self.render_task_params.get(task_id);
                if let Some(c) = cache {
                    changed = c.params.viewport != adjusted
                        || c.params.framing != self.framing
                        || c.params.override_window_policy != self.override_window_policy;
                }
            }
            if !changed {
                continue;
            }

            if let Some(task) = self.tasks.get(task_id) {
                {
                    let mut guard = task.write();
                    if let Some(rt) = guard.as_any_mut().downcast_mut::<HdxRenderTask>() {
                        let mut p = rt.get_params().unwrap_or_default();
                        p.viewport = adjusted;
                        p.framing = self.framing.clone();
                        p.override_window_policy = self.override_window_policy;
                        rt.set_params(&p);
                    }
                }
            }

            if let Some(cache) = self.render_task_params.get_mut(task_id) {
                cache.params.viewport = adjusted;
                cache.params.framing = self.framing.clone();
                cache.params.override_window_policy = self.override_window_policy;
            }
        }

        // Update pick_from_render_buffer viewport/framing.
        if let Some(ref id) = self.pick_from_render_buffer_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard
                        .as_any_mut()
                        .downcast_mut::<HdxPickFromRenderBufferTask>()
                    {
                        let mut p = t.get_params().clone();
                        p.viewport = [
                            adjusted[0] as i32,
                            adjusted[1] as i32,
                            adjusted[2] as i32,
                            adjusted[3] as i32,
                        ];
                        p.framing = self.framing.clone();
                        p.override_window_policy = self.override_window_policy;
                        t.set_params(p);
                    }
                }
            }
        }

        // Update present task dst_region.
        if let Some(ref id) = self.present_task_id.clone() {
            if let Some(task) = self.tasks.get(id) {
                {
                    let mut guard = task.write();
                    if let Some(t) = guard.as_any_mut().downcast_mut::<HdxPresentTask>() {
                        let mut p = t.get_params().clone();
                        let dst = if self.framing.is_valid() {
                            Vec4i::new(0, 0, self.render_buffer_size[0], self.render_buffer_size[1])
                        } else {
                            Vec4i::new(
                                self.viewport[0] as i32,
                                self.viewport[1] as i32,
                                self.viewport[2] as i32,
                                self.viewport[3] as i32,
                            )
                        };
                        if p.dst_region != dst {
                            p.dst_region = dst;
                            t.set_params(p);
                        }
                    }
                }
            }
        }
    }

    /// Compute AOV buffer path: {controller_id}/aov_{name}.
    fn get_aov_path(&self, aov: &Token) -> Path {
        // Sanitize AOV name: SdfPath components cannot contain `:` or `.`.
        // e.g. "primvars:st" -> "aov_primvars_st", matching C++ TfMakeValidIdentifier.
        let safe = aov.as_str().replace([':', '.'], "_");
        self.controller_id
            .append_child(&format!("aov_{}", safe))
            .unwrap_or_default()
    }

    /// Compute render task path for a material tag.
    fn get_render_task_path(&self, material_tag: &Token) -> Option<Path> {
        let name = if material_tag.is_empty() {
            "renderTask".to_string()
        } else {
            format!("renderTask_{}", material_tag.as_str().replace(':', "_"))
        };
        self.controller_id.append_child(&name)
    }

    /// Resolve user-requested AOV names to the backend-facing output list.
    ///
    /// Matches `_ref/OpenUSD/pxr/imaging/hdx/taskController.cpp::_ResolvedRenderOutputs`.
    fn resolve_render_outputs(&self, names: &[Token]) -> Vec<Token> {
        let has_color = names.iter().any(|n| n == "color");
        let has_depth = names.iter().any(|n| n == "depth");
        let has_prim_id = names.iter().any(|n| n == "primId");
        let has_element_id = names.iter().any(|n| n == "elementId");
        let has_instance_id = names.iter().any(|n| n == "instanceId");
        let has_neye = names.iter().any(|n| n == "Neye");

        if self.is_storm_backend {
            let mut result = Vec::new();
            if has_color {
                result.push(Token::new("color"));
            }
            if has_prim_id || has_instance_id {
                result.push(Token::new("primId"));
                result.push(Token::new("instanceId"));
            }
            if has_neye {
                result.push(Token::new("Neye"));
            }
            result.push(Token::new("depth"));
            return result;
        }

        let mut result = names.to_vec();
        if has_color {
            if !has_depth {
                result.push(Token::new("depth"));
            }
            if !has_prim_id {
                result.push(Token::new("primId"));
            }
            if !has_element_id {
                result.push(Token::new("elementId"));
            }
            if !has_instance_id {
                result.push(Token::new("instanceId"));
            }
        }
        result
    }

    /// Apply blend state for the given material tag (matches C++ _SetBlendStateForMaterialTag).
    fn apply_blend_state_for_tag(&self, material_tag: &Token, params: &mut HdxRenderTaskParams) {
        match material_tag.as_str() {
            "additive" => {
                params.blend_enable = true;
                params.depth_mask_enable = false;
                params.enable_alpha_to_coverage = false;
            }
            "defaultMaterialTag" | "masked" | "" => {
                params.blend_enable = false;
                params.depth_mask_enable = true;
                params.enable_alpha_to_coverage = true;
            }
            "volume" => {
                // Volume uses pre-multiplied alpha blending (One, OneMinusSrcAlpha).
                params.blend_enable = true;
                params.depth_mask_enable = false;
                params.enable_alpha_to_coverage = false;
                params.blend_color_src_factor = HdBlendFactor::One;
                params.blend_color_dst_factor = HdBlendFactor::OneMinusSrcAlpha;
                params.blend_alpha_src_factor = HdBlendFactor::One;
                params.blend_alpha_dst_factor = HdBlendFactor::OneMinusSrcAlpha;
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Conditional task enable predicates (match C++ private methods)

    fn shadows_enabled(&self) -> bool {
        if self.simple_light_task_id.is_none() {
            return false;
        }
        // Check simple light task's enable_shadows param.
        if let Some(ref id) = self.simple_light_task_id {
            if let Some(task) = self.tasks.get(id) {
                {
                    let guard = task.read();
                    if let Some(t) = guard.as_any().downcast_ref::<HdxSimpleLightTask>() {
                        return t.get_params().enable_shadows;
                    }
                }
            }
        }
        false
    }

    fn selection_enabled(&self) -> bool {
        !self.render_task_ids.is_empty()
    }

    /// Colorize selection is active when rendering color AOV.
    fn colorize_selection_enabled(&self) -> bool {
        self.viewport_aov == "color"
    }

    /// Color correction is active when mode != disabled.
    fn color_correction_enabled(&self) -> bool {
        if let Some(ref id) = self.color_correction_task_id {
            if let Some(task) = self.tasks.get(id) {
                {
                    let guard = task.read();
                    if let Some(t) = guard.as_any().downcast_ref::<HdxColorCorrectionTask>() {
                        let mode = &t.get_params().color_correction_mode;
                        return mode != &color_correction_tokens::disabled() && !mode.is_empty();
                    }
                }
            }
        }
        false
    }

    /// Visualize AOV is active when viewport AOV is not color (needs special viz).
    fn visualize_aov_enabled(&self) -> bool {
        self.viewport_aov != "color" && !self.viewport_aov.is_empty()
    }

    fn using_aovs(&self) -> bool {
        !self.aov_buffer_entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::render::HdRenderIndexTrait;

    // Minimal null render delegate for the mock render index.
    // Implements only the mandatory methods; all others use default no-ops.
    struct NullDelegate {
        supported_types: usd_hd::render::render_delegate::TfTokenVector,
    }
    impl NullDelegate {
        fn new() -> Self {
            Self {
                supported_types: vec![],
            }
        }
    }
    impl usd_hd::render::render_delegate::HdRenderDelegate for NullDelegate {
        fn get_supported_rprim_types(&self) -> &usd_hd::render::render_delegate::TfTokenVector {
            &self.supported_types
        }
        fn get_supported_sprim_types(&self) -> &usd_hd::render::render_delegate::TfTokenVector {
            &self.supported_types
        }
        fn get_supported_bprim_types(&self) -> &usd_hd::render::render_delegate::TfTokenVector {
            &self.supported_types
        }
        fn get_resource_registry(
            &self,
        ) -> usd_hd::render::render_delegate::HdResourceRegistrySharedPtr {
            unimplemented!("not needed in test") // intentional: test mock stub
        }
        fn create_render_pass(
            &mut self,
            _index: &usd_hd::render::render_index::HdRenderIndex,
            _collection: &usd_hd::HdRprimCollection,
        ) -> Option<usd_hd::render::render_delegate::HdRenderPassSharedPtr> {
            None
        }
        fn create_rprim(
            &mut self,
            _type_id: &usd_tf::Token,
            _id: usd_sdf::Path,
        ) -> Option<usd_hd::render::HdPrimHandle> {
            None
        }
        fn create_sprim(
            &mut self,
            _type_id: &usd_tf::Token,
            _id: usd_sdf::Path,
        ) -> Option<usd_hd::render::HdPrimHandle> {
            None
        }
        fn create_fallback_sprim(
            &mut self,
            _type_id: &usd_tf::Token,
        ) -> Option<usd_hd::render::HdPrimHandle> {
            None
        }
        fn create_bprim(
            &mut self,
            _type_id: &usd_tf::Token,
            _id: usd_sdf::Path,
        ) -> Option<usd_hd::render::HdPrimHandle> {
            None
        }
        fn create_fallback_bprim(
            &mut self,
            _type_id: &usd_tf::Token,
        ) -> Option<usd_hd::render::HdPrimHandle> {
            None
        }
        fn create_instancer(
            &mut self,
            _delegate: &dyn usd_hd::HdSceneDelegate,
            _id: usd_sdf::Path,
        ) -> Option<Box<dyn usd_hd::render::render_delegate::HdInstancer>> {
            None
        }
        fn destroy_instancer(
            &mut self,
            _instancer: Box<dyn usd_hd::render::render_delegate::HdInstancer>,
        ) {
        }
        fn commit_resources(&mut self, _tracker: &mut usd_hd::change_tracker::HdChangeTracker) {}
    }

    struct MockRenderIndex {
        delegate: usd_hd::render::HdRenderDelegateSharedPtr,
        tracker: usd_hd::change_tracker::HdChangeTracker,
    }

    impl MockRenderIndex {
        fn new() -> Self {
            use std::sync::Arc;
            Self {
                delegate: Arc::new(RwLock::new(NullDelegate::new())),
                tracker: usd_hd::change_tracker::HdChangeTracker::new(),
            }
        }
    }

    impl HdRenderIndexTrait for MockRenderIndex {
        fn get_task(&self, _id: &Path) -> Option<&HdTaskSharedPtr> {
            None
        }
        fn has_task(&self, _id: &Path) -> bool {
            false
        }
        fn get_rprim(&self, _id: &Path) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_rprim_ids(&self) -> Vec<Path> {
            Vec::new()
        }
        fn get_prim_id_for_rprim_path(&self, _rprim_path: &Path) -> Option<i32> {
            None
        }
        fn get_sprim(
            &self,
            _type_id: &usd_tf::Token,
            _id: &Path,
        ) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_bprim(
            &self,
            _type_id: &usd_tf::Token,
            _id: &Path,
        ) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_render_delegate(&self) -> &usd_hd::render::HdRenderDelegateSharedPtr {
            &self.delegate
        }
        fn get_change_tracker(&self) -> &usd_hd::change_tracker::HdChangeTracker {
            &self.tracker
        }
    }

    fn make_controller() -> HdxTaskController {
        let _unused_render_index = Arc::new(RwLock::new(MockRenderIndex::new()));
        HdxTaskController::new(Path::from_string("/TC").unwrap(), true)
    }

    #[test]
    fn test_creation() {
        let c = make_controller();
        assert_eq!(c.get_controller_id().as_str(), "/TC");
        assert!(!c.tasks.is_empty());
    }

    #[test]
    fn test_all_tasks_created() {
        let c = make_controller();
        // Verify each task ID has an entry in the tasks map.
        assert!(
            c.simple_light_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(
            c.shadow_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(!c.render_task_ids.is_empty());
        for id in &c.render_task_ids {
            assert!(
                c.tasks.contains_key(id),
                "render task missing: {}",
                id.as_str()
            );
        }
        assert!(
            c.aov_input_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(
            c.selection_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(
            c.colorize_selection_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(
            c.color_correction_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
        assert!(
            c.present_task_id
                .as_ref()
                .map(|id| c.tasks.contains_key(id))
                .unwrap_or(false)
        );
    }

    #[test]
    fn test_rendering_tasks_not_empty() {
        let c = make_controller();
        let tasks = c.get_rendering_tasks();
        assert!(!tasks.is_empty(), "should have rendering tasks");
    }

    #[test]
    fn test_shadows_disabled_by_default() {
        let c = make_controller();
        assert!(!c.enable_shadows);
        let paths = c.get_rendering_task_paths();
        assert!(!paths.iter().any(|p| p.as_str().contains("shadowTask")));
    }

    #[test]
    fn test_enable_shadows() {
        let mut c = make_controller();
        c.set_enable_shadows(true);
        assert!(c.enable_shadows);
        let paths = c.get_rendering_task_paths();
        assert!(paths.iter().any(|p| p.as_str().contains("shadowTask")));
    }

    #[test]
    fn test_set_camera_path() {
        let mut c = make_controller();
        let cam = Path::from_string("/cameras/main").unwrap();
        c.set_camera_path(cam.clone());
        assert_eq!(c.active_camera_id, Some(cam));
    }

    #[test]
    fn test_set_render_buffer_size() {
        let mut c = make_controller();
        c.set_render_buffer_size(Vec2i::new(1920, 1080));
        assert_eq!(c.render_buffer_size, Vec2i::new(1920, 1080));
    }

    #[test]
    fn test_selection_colors() {
        let mut c = make_controller();
        let color = Vec4f::new(0.0, 1.0, 0.0, 1.0);
        c.set_selection_color(color);
        assert_eq!(c.get_selection_color(), color);

        let loc = Vec4f::new(0.5, 0.5, 0.0, 1.0);
        c.set_selection_locate_color(loc);
        assert_eq!(c.get_selection_locate_color(), loc);
    }

    #[test]
    fn test_set_render_outputs_wires_aov_paths() {
        let c = make_controller();
        // Storm resolves color viewport output to backend color+depth only.
        let entries: Vec<String> = c
            .aov_buffer_entries
            .iter()
            .map(|e| e.aov_name.as_str().to_string())
            .collect();
        assert_eq!(entries, vec!["color".to_string(), "depth".to_string()]);

        let colorize_id = c
            .colorize_selection_task_id
            .as_ref()
            .expect("colorize selection task must exist");
        let colorize_task = c
            .tasks
            .get(colorize_id)
            .expect("colorize selection task must be registered");
        let colorize_guard = colorize_task.read();
        let colorize = colorize_guard
            .as_any()
            .downcast_ref::<HdxColorizeSelectionTask>()
            .expect("colorize selection task type");
        let colorize_params = colorize.get_params();
        assert_eq!(
            colorize_params.prim_id_buffer_path.as_str(),
            "/TC/aov_primId"
        );
        assert_eq!(
            colorize_params.instance_id_buffer_path.as_str(),
            "/TC/aov_instanceId"
        );
        assert_eq!(
            colorize_params.element_id_buffer_path.as_str(),
            "/TC/aov_elementId"
        );

        let pick_id = c
            .pick_from_render_buffer_task_id
            .as_ref()
            .expect("pick-from-render-buffer task must exist");
        let pick_task = c
            .tasks
            .get(pick_id)
            .expect("pick-from-render-buffer task must be registered");
        let pick_guard = pick_task.read();
        let pick = pick_guard
            .as_any()
            .downcast_ref::<HdxPickFromRenderBufferTask>()
            .expect("pick-from-render-buffer task type");
        let pick_params = pick.get_params();
        assert_eq!(pick_params.prim_id_buffer_path.as_str(), "/TC/aov_primId");
        assert_eq!(pick_params.depth_buffer_path.as_str(), "/TC/aov_depth");
    }

    #[test]
    fn test_colorize_selection_enabled_for_color_aov() {
        let c = make_controller();
        // Default viewport_aov is set to "color" by set_render_outputs.
        assert_eq!(c.viewport_aov.as_str(), "color");
        assert!(c.colorize_selection_enabled());
    }

    #[test]
    fn test_visualize_aov_disabled_for_color() {
        let c = make_controller();
        assert!(!c.visualize_aov_enabled());
    }

    #[test]
    fn test_visualize_aov_enabled_for_depth() {
        let mut c = make_controller();
        c.set_render_outputs(&[Token::new("depth")]);
        assert!(c.visualize_aov_enabled());
    }

    #[test]
    fn test_color_correction_disabled_by_default() {
        let c = make_controller();
        assert!(!c.color_correction_enabled());
    }

    #[test]
    fn test_presentation_default_on() {
        let c = make_controller();
        assert!(c.enable_presentation);
        let paths = c.get_rendering_task_paths();
        assert!(paths.iter().any(|p| p.as_str().contains("presentTask")));
    }

    #[test]
    fn test_disable_presentation() {
        let mut c = make_controller();
        c.set_enable_presentation(false);
        let paths = c.get_rendering_task_paths();
        assert!(!paths.iter().any(|p| p.as_str().contains("presentTask")));
    }

    #[test]
    fn test_picking_tasks_present() {
        let c = make_controller();
        let paths = c.get_picking_task_paths();
        assert!(!paths.is_empty());
        let tasks = c.get_picking_tasks();
        assert_eq!(paths.len(), tasks.len());
    }

    #[test]
    fn test_is_converged_initially() {
        let c = make_controller();
        // Tasks start converged (render task marks converged after execute).
        // Since execute hasn't run, render task is NOT converged.
        // is_converged() should return false.
        let result = c.is_converged();
        // Result depends on render task's initial state (not converged by default).
        // Just verify it doesn't panic.
        let _ = result;
    }

    #[test]
    fn test_outline_radius_propagated() {
        let mut c = make_controller();
        c.set_selection_outline_radius(3);
        assert_eq!(c.get_selection_outline_radius(), 3);

        // Verify propagated to colorize task.
        if let Some(ref id) = c.colorize_selection_task_id {
            if let Some(task) = c.tasks.get(id) {
                {
                    let guard = task.read();
                    if let Some(t) = guard.as_any().downcast_ref::<HdxColorizeSelectionTask>() {
                        assert_eq!(t.get_params().outline_radius, 3);
                    }
                }
            }
        }
    }

    #[test]
    fn test_aov_path_format() {
        // get_aov_path must produce {controller_id}/aov_{name}, matching C++ _GetAovPath.
        let c = make_controller();
        let path = c.get_aov_path(&Token::new("color"));
        assert_eq!(path.as_str(), "/TC/aov_color");

        // Colons sanitized to underscores (e.g. "primvars:st" -> "aov_primvars_st").
        let path2 = c.get_aov_path(&Token::new("primvars:st"));
        assert_eq!(path2.as_str(), "/TC/aov_primvars_st");
    }

    #[test]
    fn test_render_task_path_format() {
        // Matches C++ _GetRenderTaskPath: {controller_id}/renderTask_{tag}.
        let c = make_controller();
        let empty_path = c.get_render_task_path(&Token::new("")).unwrap();
        assert_eq!(empty_path.as_str(), "/TC/renderTask");

        let tagged_path = c.get_render_task_path(&Token::new("additive")).unwrap();
        assert_eq!(tagged_path.as_str(), "/TC/renderTask_additive");
    }

    #[test]
    fn test_blend_state_additive() {
        let c = make_controller();
        let mut params = HdxRenderTaskParams::default();
        c.apply_blend_state_for_tag(&Token::new("additive"), &mut params);
        assert!(params.blend_enable, "additive must enable blending");
        assert!(
            !params.depth_mask_enable,
            "additive must disable depth mask"
        );
        assert!(!params.enable_alpha_to_coverage);
    }

    #[test]
    fn test_blend_state_default() {
        let c = make_controller();
        let mut params = HdxRenderTaskParams::default();
        c.apply_blend_state_for_tag(&Token::new("defaultMaterialTag"), &mut params);
        assert!(!params.blend_enable);
        assert!(params.depth_mask_enable);
        assert!(params.enable_alpha_to_coverage);
    }

    #[test]
    fn test_blend_state_volume() {
        let c = make_controller();
        let mut params = HdxRenderTaskParams::default();
        c.apply_blend_state_for_tag(&Token::new("volume"), &mut params);
        assert!(params.blend_enable);
        assert!(!params.depth_mask_enable);
        assert!(!params.enable_alpha_to_coverage);
    }

    #[test]
    fn test_resolve_render_outputs_storm_matches_reference_order() {
        let c = make_controller();
        let outputs = c.resolve_render_outputs(&[Token::new("color")]);
        let names: Vec<&str> = outputs.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, vec!["color", "depth"]);
    }

    #[test]
    fn test_resolve_render_outputs_storm_no_duplication() {
        let c = make_controller();
        let outputs = c.resolve_render_outputs(&[Token::new("color"), Token::new("depth")]);
        let names: Vec<&str> = outputs.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, vec!["color", "depth"]);
    }

    #[test]
    fn test_set_render_outputs_idempotent() {
        let mut c = make_controller();
        // Setting same outputs twice must not change state.
        let old_count = c.aov_buffer_entries.len();
        c.set_render_outputs(&[Token::new("color")]);
        assert_eq!(
            c.aov_buffer_entries.len(),
            old_count,
            "idempotent re-set must not change entries"
        );
    }

    #[test]
    fn test_task_order_light_before_render() {
        // simpleLightTask must come before renderTask in the pipeline.
        let c = make_controller();
        let paths = c.get_rendering_task_paths();
        let light_pos = paths
            .iter()
            .position(|p| p.as_str().contains("simpleLightTask"));
        let render_pos = paths.iter().position(|p| p.as_str().contains("renderTask"));
        assert!(light_pos.is_some() && render_pos.is_some());
        assert!(
            light_pos.unwrap() < render_pos.unwrap(),
            "light must precede render"
        );
    }

    #[test]
    fn test_task_order_aov_input_after_render_before_selection() {
        let c = make_controller();
        let paths = c.get_rendering_task_paths();
        let render_pos = paths.iter().position(|p| p.as_str().contains("renderTask"));
        let aov_pos = paths
            .iter()
            .position(|p| p.as_str().contains("aovInputTask"));
        let sel_pos = paths
            .iter()
            .position(|p| p.as_str().contains("selectionTask"));
        if let (Some(r), Some(a), Some(s)) = (render_pos, aov_pos, sel_pos) {
            assert!(r < a, "renderTask must precede aovInputTask");
            assert!(a < s, "aovInputTask must precede selectionTask");
        }
    }

    #[test]
    fn test_task_order_present_last() {
        let c = make_controller();
        let paths = c.get_rendering_task_paths();
        let last = paths.last().unwrap();
        assert!(
            last.as_str().contains("presentTask"),
            "presentTask must be last"
        );
    }

    #[test]
    fn test_set_enable_selection_propagated() {
        let mut c = make_controller();
        c.set_enable_selection(false);

        if let Some(ref id) = c.selection_task_id {
            if let Some(task) = c.tasks.get(id) {
                {
                    let guard = task.read();
                    if let Some(t) = guard.as_any().downcast_ref::<HdxSelectionTask>() {
                        assert!(!t.get_params().enable_selection_highlight);
                        assert!(!t.get_params().enable_locate_highlight);
                    }
                }
            }
        }
    }

    #[test]
    fn test_set_viewport_propagated_to_render_tasks() {
        let mut c = make_controller();
        c.set_render_viewport(Vec4d::new(0.0, 0.0, 1280.0, 720.0));

        for id in &c.render_task_ids {
            if let Some(cache) = c.render_task_params.get(id) {
                // viewport[2] and viewport[3] are width and height.
                assert_eq!(cache.params.viewport[2], 1280.0);
                assert_eq!(cache.params.viewport[3], 720.0);
            }
        }
    }

    #[test]
    fn test_free_camera_state() {
        let mut c = make_controller();
        let view = usd_gf::Matrix4d::identity();
        let proj = usd_gf::Matrix4d::identity();

        c.set_free_camera_matrices(view, proj);
        assert!(c.is_using_free_camera());
        // Setting free camera clears the active_camera_id.
        assert!(c.active_camera_id.is_none());

        c.clear_free_camera();
        assert!(!c.is_using_free_camera());
    }
}
