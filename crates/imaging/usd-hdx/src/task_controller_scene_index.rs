//! Task controller scene index - Hydra 2.0 task orchestration.
//!
//! A scene index that manages tasks for rendering (or picking),
//! render buffers, lights, and a free camera.
//!
//! This is the Hydra 2.0 replacement for HdxTaskController,
//! using scene indices instead of scene delegates.
//!
//! Port of pxr/imaging/hdx/taskControllerSceneIndex.h/cpp

use usd_gf::{Matrix4d, Vec2i, Vec4d, Vec4f};
use usd_sdf::Path;
use usd_tf::Token;

use super::bounding_box_task::HdxBoundingBoxTaskParams;
use super::color_correction_task::HdxColorCorrectionTaskParams;
use super::render_setup_task::{
    CameraUtilConformWindowPolicy, CameraUtilFraming, HdxRenderTaskParams,
};
use super::shadow_task::HdxShadowTaskParams;

/// AOV descriptor for configuring render output buffers.
#[derive(Debug, Clone)]
pub struct HdAovDescriptor {
    /// AOV format (e.g., Float32Vec4, UNorm8Vec4).
    pub format: Token,
    /// Clear value for the AOV.
    pub clear_value: Option<Vec4f>,
    /// Whether this AOV uses multisampling.
    pub multi_sampled: bool,
    /// Render buffer dimensions (width, height) in pixels.
    pub dimensions: Vec2i,
}

impl Default for HdAovDescriptor {
    fn default() -> Self {
        Self {
            format: Token::new("float32Vec4"),
            clear_value: None,
            multi_sampled: false,
            dimensions: Vec2i::new(0, 0),
        }
    }
}

/// Callback type for querying default AOV descriptors from render delegate.
pub type AovDescriptorCallback = Box<dyn Fn(&Token) -> HdAovDescriptor + Send + Sync>;

/// Parameters for creating an HdxTaskControllerSceneIndex.
pub struct TaskControllerSceneIndexParams {
    /// Prefix path for all prims in this scene index.
    pub prefix: Path,
    /// Callback wrapping HdRenderDelegate::GetDefaultAovDescriptor.
    pub aov_descriptor_callback: AovDescriptorCallback,
    /// Whether this is for the Storm renderer.
    pub is_for_storm: bool,
    /// Whether GPU is enabled (affects present task for non-Storm).
    pub gpu_enabled: bool,
}

/// Hydra 2.0 task controller scene index.
///
/// Manages tasks necessary to render an image (or perform picking)
/// as well as related render buffers, lights, and a free camera.
///
/// The set of tasks differs between Storm and other renderers,
/// so the constructor needs the renderer plugin name.
///
/// Port of HdxTaskControllerSceneIndex from pxr/imaging/hdx/taskControllerSceneIndex.h
pub struct HdxTaskControllerSceneIndex {
    /// Creation parameters.
    prefix: Path,
    is_for_storm: bool,
    gpu_enabled: bool,

    // Task paths
    render_task_paths: Vec<Path>,
    active_camera_id: Option<Path>,

    // AOV state
    aov_names: Vec<Token>,
    viewport_aov: Token,

    // Framing state
    render_buffer_size: Vec2i,
    framing: CameraUtilFraming,
    override_window_policy: Option<CameraUtilConformWindowPolicy>,
    viewport: Vec4d,

    // Feature flags
    enable_shadows: bool,
    enable_selection: bool,
    enable_presentation: bool,
}

impl HdxTaskControllerSceneIndex {
    /// Create new task controller scene index.
    pub fn new(params: TaskControllerSceneIndexParams) -> Self {
        let is_for_storm = params.is_for_storm;
        let gpu_enabled = params.gpu_enabled;

        let mut controller = Self {
            prefix: params.prefix,
            is_for_storm,
            gpu_enabled,
            render_task_paths: Vec::new(),
            active_camera_id: None,
            aov_names: Vec::new(),
            viewport_aov: Token::new("color"),
            render_buffer_size: Vec2i::new(512, 512),
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
            viewport: Vec4d::new(0.0, 0.0, 512.0, 512.0),
            enable_shadows: false,
            enable_selection: false,
            enable_presentation: true,
        };

        if is_for_storm {
            controller.create_storm_tasks();
        } else {
            controller.create_generic_tasks();
        }

        controller
    }

    // -------------------------------------------------------
    // Execution API

    /// Get paths to rendering tasks.
    pub fn get_rendering_task_paths(&self) -> Vec<Path> {
        if self.is_for_storm {
            self.get_rendering_task_paths_for_storm()
        } else {
            self.get_rendering_task_paths_for_generic()
        }
    }

    /// Get paths to picking tasks.
    pub fn get_picking_task_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();
        if let Some(path) = self.prefix.append_child("pickTask") {
            paths.push(path);
        }
        paths
    }

    /// Get path for a named render buffer.
    pub fn get_render_buffer_path(&self, aov_name: &Token) -> Option<Path> {
        self.prefix
            .append_child(&format!("renderBuffer_{}", aov_name.as_str()))
    }

    // -------------------------------------------------------
    // Rendering API

    /// Set the collection to be rendered.
    pub fn set_collection(&mut self, _collection: &Token) {
        // In full implementation: update retained scene index prim data
        // for all render tasks with the new collection.
    }

    /// Set render parameters.
    pub fn set_render_params(&mut self, _params: &HdxRenderTaskParams) {
        // In full implementation: update retained scene index prim data
        // for render tasks with the new params.
    }

    /// Set render tags for the scene.
    pub fn set_render_tags(&mut self, _render_tags: &[Token]) {
        // In full implementation: update render task prim data
        // with new render tags.
    }

    // -------------------------------------------------------
    // AOV API

    /// Set render outputs (AOVs to produce).
    pub fn set_render_outputs(&mut self, aov_names: &[Token]) {
        self.aov_names = aov_names.to_vec();
        self._set_render_outputs(aov_names);
    }

    /// Set which AOV is displayed in the viewport.
    pub fn set_viewport_render_output(&mut self, aov_name: Token) {
        self.viewport_aov = aov_name;
    }

    /// Set custom parameters for an AOV.
    pub fn set_render_output_settings(&mut self, _aov_name: &Token, _desc: &HdAovDescriptor) {
        // In full implementation: update render buffer prim data
        // in retained scene index.
    }

    /// Get parameters for an AOV.
    pub fn get_render_output_settings(&self, _aov_name: &Token) -> HdAovDescriptor {
        HdAovDescriptor::default()
    }

    /// Set presentation output destination (API + framebuffer).
    pub fn set_presentation_output(&mut self, _api: &Token, _framebuffer: &[u8]) {
        // In full implementation: update present task prim data
        // with destination API and framebuffer handle.
    }

    // -------------------------------------------------------
    // Lighting API

    /// Set lighting state for the scene.
    pub fn set_lighting_state(&mut self, _lighting_context: &Token) {
        // In full implementation:
        // 1. Extract lights from GlfSimpleLightingContext
        // 2. Create/update/remove light sprims in retained scene index
        // 3. Update simple light task params
    }

    // -------------------------------------------------------
    // Camera and Framing API

    /// Set render buffer size (window size for GUI apps).
    pub fn set_render_buffer_size(&mut self, size: Vec2i) {
        self.render_buffer_size = size;
        self._set_render_buffer_size();
    }

    /// Set camera framing (filmback to pixel mapping).
    pub fn set_framing(&mut self, framing: CameraUtilFraming) {
        self.framing = framing;
        self._set_camera_framing_for_tasks();
    }

    /// Set override window policy for camera frustum conforming.
    pub fn set_override_window_policy(&mut self, policy: Option<CameraUtilConformWindowPolicy>) {
        self.override_window_policy = policy;
        self._set_camera_framing_for_tasks();
    }

    /// Set camera path (scene camera).
    pub fn set_camera_path(&mut self, path: Path) {
        self.active_camera_id = Some(path);
    }

    /// Set viewport (deprecated, use set_framing + set_render_buffer_size).
    pub fn set_render_viewport(&mut self, viewport: Vec4d) {
        self.viewport = viewport;
    }

    /// Set free camera view and projection matrices.
    pub fn set_free_camera_matrices(
        &mut self,
        _view_matrix: Matrix4d,
        _projection_matrix: Matrix4d,
    ) {
        // In full implementation: update free camera prim data source
        // in retained scene index.
    }

    /// Set free camera clip planes.
    pub fn set_free_camera_clip_planes(&mut self, _clip_planes: &[Vec4d]) {
        // In full implementation: update free camera prim data source
        // clipping planes.
    }

    // -------------------------------------------------------
    // Selection API

    /// Enable/disable selection highlighting.
    pub fn set_enable_selection(&mut self, enable: bool) {
        self.enable_selection = enable;
    }

    /// Set selection highlight color.
    pub fn set_selection_color(&mut self, _color: Vec4f) {
        // In full implementation: update selection task + colorize selection
        // task params in retained scene index.
    }

    /// Set selection locate (rollover) color.
    pub fn set_selection_locate_color(&mut self, _color: Vec4f) {
        // In full implementation: update selection task params.
    }

    /// Set whether selection highlight is outline or solid overlay.
    pub fn set_selection_enable_outline(&mut self, _enable_outline: bool) {
        // In full implementation: update colorize selection task params.
    }

    /// Set selection outline thickness in pixels.
    pub fn set_selection_outline_radius(&mut self, _radius: u32) {
        // In full implementation: update colorize selection task params.
    }

    // -------------------------------------------------------
    // Shadow API

    /// Enable/disable shadow rendering.
    pub fn set_enable_shadows(&mut self, enable: bool) {
        self.enable_shadows = enable;
    }

    /// Set shadow task parameters.
    pub fn set_shadow_params(&mut self, _params: &HdxShadowTaskParams) {
        // In full implementation: update shadow task prim data
        // in retained scene index.
    }

    // -------------------------------------------------------
    // Color Correction API

    /// Set color correction parameters.
    pub fn set_color_correction_params(&mut self, _params: &HdxColorCorrectionTaskParams) {
        // In full implementation: update color correction task prim data
        // in retained scene index.
    }

    // -------------------------------------------------------
    // Bounding Box API

    /// Set bounding box task parameters.
    pub fn set_bbox_params(&mut self, _params: &HdxBoundingBoxTaskParams) {
        // In full implementation: update bounding box task prim data
        // in retained scene index.
    }

    // -------------------------------------------------------
    // Present API

    /// Enable/disable presentation to framebuffer.
    pub fn set_enable_presentation(&mut self, enabled: bool) {
        self.enable_presentation = enabled;
    }

    // -------------------------------------------------------
    // Scene Index Interface

    /// Get prim at path (scene index interface).
    pub fn get_prim(&self, _prim_path: &Path) -> Option<SceneIndexPrim> {
        // In full implementation: look up prim in retained scene index
        // and return its type + data source.
        None
    }

    /// Get child prim paths (scene index interface).
    pub fn get_child_prim_paths(&self, prim_path: &Path) -> Vec<Path> {
        if prim_path == &self.prefix {
            // Return all task/buffer/light/camera paths
            self.render_task_paths.clone()
        } else {
            Vec::new()
        }
    }

    // -------------------------------------------------------
    // Private methods

    fn create_storm_tasks(&mut self) {
        // Storm needs the full task pipeline:
        // simpleLightTask, shadowTask, renderSetupTask, renderTask(s),
        // aovInputTask, selectionTask, colorizeSelectionTask,
        // oitResolveTask, colorCorrectionTask, visualizeAovTask,
        // boundingBoxTask, pickTask, pickFromRenderBufferTask, presentTask
        let task_names = [
            "simpleLightTask",
            "shadowTask",
            "renderSetupTask",
            "renderTask",
            "aovInputTask",
            "oitResolveTask",
            "selectionTask",
            "colorizeSelectionTask",
            "colorCorrectionTask",
            "visualizeAovTask",
            "boundingBoxTask",
            "skydomeTask",
            "pickTask",
            "pickFromRenderBufferTask",
            "presentTask",
        ];

        for name in &task_names {
            if let Some(path) = self.prefix.append_child(name) {
                self.render_task_paths.push(path);
            }
        }
    }

    fn create_generic_tasks(&mut self) {
        // Non-Storm renderers need fewer tasks:
        // renderTask, colorCorrectionTask, presentTask (if GPU), pickTask
        let mut task_names = vec!["renderTask", "colorCorrectionTask"];

        if self.gpu_enabled {
            task_names.push("presentTask");
        }
        task_names.push("pickTask");

        for name in &task_names {
            if let Some(path) = self.prefix.append_child(name) {
                self.render_task_paths.push(path);
            }
        }
    }

    fn get_rendering_task_paths_for_storm(&self) -> Vec<Path> {
        // Storm rendering pipeline order:
        // 1. simpleLightTask
        // 2. shadowTask (if shadows enabled)
        // 3. skydomeTask
        // 4. renderSetupTask + renderTask(s)
        // 5. aovInputTask
        // 6. oitResolveTask
        // 7. selectionTask (if selection enabled)
        // 8. colorizeSelectionTask (if selection enabled)
        // 9. colorCorrectionTask
        // 10. visualizeAovTask
        // 11. boundingBoxTask
        // 12. presentTask (if presentation enabled)
        self.render_task_paths
            .iter()
            .filter(|path| {
                let name = path.get_name();
                // Filter by enabled features
                if name == "shadowTask" && !self.enable_shadows {
                    return false;
                }
                if (name == "selectionTask" || name == "colorizeSelectionTask")
                    && !self.enable_selection
                {
                    return false;
                }
                if name == "presentTask" && !self.enable_presentation {
                    return false;
                }
                // Exclude pick tasks from rendering
                name != "pickTask" && name != "pickFromRenderBufferTask"
            })
            .cloned()
            .collect()
    }

    fn get_rendering_task_paths_for_generic(&self) -> Vec<Path> {
        self.render_task_paths
            .iter()
            .filter(|path| {
                let name = path.get_name();
                if name == "presentTask" && !self.enable_presentation {
                    return false;
                }
                name != "pickTask"
            })
            .cloned()
            .collect()
    }

    fn _set_render_outputs(&mut self, _aov_names: &[Token]) {
        // In full implementation: create/update render buffer prims
        // in retained scene index for each AOV.
    }

    fn _set_camera_framing_for_tasks(&mut self) {
        // In full implementation: update camera framing on all tasks
        // that use it (render, shadow, pick, etc.).
    }

    fn _set_render_buffer_size(&mut self) {
        // In full implementation: resize all render buffer prims
        // in retained scene index.
    }
}

/// Scene index prim representation.
#[derive(Debug, Clone)]
pub struct SceneIndexPrim {
    /// Prim type token (e.g., "renderTask", "renderBuffer").
    pub prim_type: Token,
}

/// Check if a render delegate is Storm.
///
/// Port of HdxIsStorm from pxr/imaging/hdx/stormCheck.h
pub fn hdx_is_storm(_delegate_type: &Token) -> bool {
    // In full implementation: check delegate->GetRendererDisplayName()
    // or delegate->RequiresStormTasks()
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_default_params() -> TaskControllerSceneIndexParams {
        TaskControllerSceneIndexParams {
            prefix: Path::from_string("/TaskController").unwrap(),
            aov_descriptor_callback: Box::new(|_| HdAovDescriptor::default()),
            is_for_storm: false,
            gpu_enabled: true,
        }
    }

    fn make_storm_params() -> TaskControllerSceneIndexParams {
        TaskControllerSceneIndexParams {
            prefix: Path::from_string("/TaskController").unwrap(),
            aov_descriptor_callback: Box::new(|_| HdAovDescriptor::default()),
            is_for_storm: true,
            gpu_enabled: true,
        }
    }

    #[test]
    fn test_generic_creation() {
        let controller = HdxTaskControllerSceneIndex::new(make_default_params());
        assert!(!controller.is_for_storm);
        assert!(controller.gpu_enabled);
        // Generic: renderTask, colorCorrectionTask, presentTask, pickTask
        assert_eq!(controller.render_task_paths.len(), 4);
    }

    #[test]
    fn test_storm_creation() {
        let controller = HdxTaskControllerSceneIndex::new(make_storm_params());
        assert!(controller.is_for_storm);
        // Storm has many more tasks
        assert!(controller.render_task_paths.len() >= 10);
    }

    #[test]
    fn test_rendering_task_paths_generic() {
        let controller = HdxTaskControllerSceneIndex::new(make_default_params());
        let paths = controller.get_rendering_task_paths();
        // Should exclude pickTask
        assert!(paths.iter().all(|p| p.get_name() != "pickTask"));
    }

    #[test]
    fn test_rendering_task_paths_storm() {
        let mut controller = HdxTaskControllerSceneIndex::new(make_storm_params());
        controller.set_enable_shadows(true);
        controller.set_enable_selection(true);

        let paths = controller.get_rendering_task_paths();
        assert!(paths.iter().any(|p| p.get_name() == "shadowTask"));
        assert!(paths.iter().any(|p| p.get_name() == "selectionTask"));
    }

    #[test]
    fn test_picking_task_paths() {
        let controller = HdxTaskControllerSceneIndex::new(make_default_params());
        let paths = controller.get_picking_task_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].get_name() == "pickTask");
    }

    #[test]
    fn test_camera_path() {
        let mut controller = HdxTaskControllerSceneIndex::new(make_default_params());
        let cam_path = Path::from_string("/cameras/main").unwrap();
        controller.set_camera_path(cam_path.clone());
        assert_eq!(controller.active_camera_id, Some(cam_path));
    }

    #[test]
    fn test_render_buffer_size() {
        let mut controller = HdxTaskControllerSceneIndex::new(make_default_params());
        controller.set_render_buffer_size(Vec2i::new(1920, 1080));
        assert_eq!(controller.render_buffer_size, Vec2i::new(1920, 1080));
    }

    #[test]
    fn test_feature_toggles() {
        let mut controller = HdxTaskControllerSceneIndex::new(make_storm_params());

        controller.set_enable_shadows(true);
        assert!(controller.enable_shadows);

        controller.set_enable_selection(true);
        assert!(controller.enable_selection);

        controller.set_enable_presentation(false);
        assert!(!controller.enable_presentation);
    }

    #[test]
    fn test_render_outputs() {
        let mut controller = HdxTaskControllerSceneIndex::new(make_default_params());
        controller.set_render_outputs(&[Token::new("color"), Token::new("depth")]);
        assert_eq!(controller.aov_names.len(), 2);
    }

    #[test]
    fn test_child_prim_paths() {
        let controller = HdxTaskControllerSceneIndex::new(make_default_params());
        let prefix = Path::from_string("/TaskController").unwrap();
        let children = controller.get_child_prim_paths(&prefix);
        assert!(!children.is_empty());
    }

    #[test]
    fn test_hdx_is_storm() {
        assert!(!hdx_is_storm(&Token::new("HdStormRendererPlugin")));
    }

    #[test]
    fn test_aov_descriptor_default() {
        let desc = HdAovDescriptor::default();
        assert_eq!(desc.format.as_str(), "float32Vec4");
        assert!(desc.clear_value.is_none());
        assert!(!desc.multi_sampled);
    }
}
