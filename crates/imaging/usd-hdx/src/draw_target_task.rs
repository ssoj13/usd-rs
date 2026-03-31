
//! Draw target task - Renders to offscreen draw targets.
//!
//! Manages rendering to offscreen FBO targets for effects like reflections,
//! render-to-texture, etc.
//! Port of pxr/imaging/hdx/drawTargetTask.h/cpp

use usd_gf::Vec4f;
use usd_hd::enums::{HdCompareFunction, HdCullStyle};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Draw target task parameters.
///
/// Port of HdxDrawTargetTaskParams from pxr/imaging/hdx/drawTargetTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxDrawTargetTaskParams {
    /// Override color for debugging.
    pub override_color: Vec4f,
    /// Wireframe color.
    pub wireframe_color: Vec4f,
    /// Enable lighting.
    pub enable_lighting: bool,
    /// Alpha threshold for transparency.
    pub alpha_threshold: f32,

    // Depth bias state
    /// Use default depth bias from GL state.
    pub depth_bias_use_default: bool,
    /// Enable depth bias.
    pub depth_bias_enable: bool,
    /// Depth bias constant factor.
    pub depth_bias_constant_factor: f32,
    /// Depth bias slope factor.
    pub depth_bias_slope_factor: f32,

    /// Depth comparison function.
    pub depth_func: HdCompareFunction,

    /// Enable alpha-to-coverage (required for draw targets until
    /// transparency pass is supported).
    pub enable_alpha_to_coverage: bool,

    /// Cull style for viewer.
    pub cull_style: HdCullStyle,
}

impl Default for HdxDrawTargetTaskParams {
    fn default() -> Self {
        Self {
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            enable_lighting: false,
            alpha_threshold: 0.0,
            depth_bias_use_default: true,
            depth_bias_enable: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 1.0,
            depth_func: HdCompareFunction::LEqual,
            enable_alpha_to_coverage: true,
            cull_style: HdCullStyle::BackUnlessDoubleSided,
        }
    }
}

/// Draw target rendering task.
///
/// Renders scene geometry into offscreen draw targets (FBOs).
/// Each draw target has its own camera, render pass state, and lighting.
///
/// Port of HdxDrawTargetTask from pxr/imaging/hdx/drawTargetTask.h
pub struct HdxDrawTargetTask {
    /// Task path.
    id: Path,

    /// Render tags for filtering.
    render_tags: TfTokenVector,

    /// Task parameters.
    params: HdxDrawTargetTaskParams,

    /// Current draw target set version for change tracking.
    #[allow(dead_code)]
    current_draw_target_set_version: u32,
}

impl HdxDrawTargetTask {
    /// Create new draw target task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            render_tags: Vec::new(),
            params: HdxDrawTargetTaskParams::default(),
            current_draw_target_set_version: 0,
        }
    }

    /// Set draw target task parameters.
    pub fn set_params(&mut self, params: HdxDrawTargetTaskParams) {
        self.params = params;
    }

    /// Get draw target task parameters.
    pub fn get_params(&self) -> &HdxDrawTargetTaskParams {
        &self.params
    }
}

impl HdTask for HdxDrawTargetTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In full implementation:
        // 1. Pull HdxDrawTargetTaskParams from delegate
        // 2. Update render pass info from draw targets in render index
        // 3. Track draw target set version changes
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // In full implementation:
        // 1. Compute render pass infos from draw targets
        // 2. Set up camera info for each draw target
        // 3. Update lighting context per draw target
        // 4. Configure render pass state (depth, stencil, blend, etc.)
        ctx.insert(
            Token::new("drawTargetPrepared"),
            Value::from(format!("HdxDrawTargetTask@{}", self.id.get_string())),
        );
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // In full implementation:
        // 1. For each draw target with valid render passes:
        //    a. Bind the draw target's FBO
        //    b. Set viewport to draw target resolution
        //    c. Apply render pass state
        //    d. Execute render pass for the draw target's collection
        //    e. Unbind FBO
        ctx.insert(
            Token::new("drawTargetTaskExecuted"),
            Value::from(format!("HdxDrawTargetTask@{}", self.id.get_string())),
        );
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn is_converged(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_target_task_params_default() {
        let params = HdxDrawTargetTaskParams::default();
        assert!(!params.enable_lighting);
        assert_eq!(params.alpha_threshold, 0.0);
        assert!(params.depth_bias_use_default);
        assert!(!params.depth_bias_enable);
        assert_eq!(params.depth_func, HdCompareFunction::LEqual);
        assert!(params.enable_alpha_to_coverage);
        assert_eq!(params.cull_style, HdCullStyle::BackUnlessDoubleSided);
    }

    #[test]
    fn test_draw_target_task_creation() {
        let task = HdxDrawTargetTask::new(Path::from_string("/drawTarget").unwrap());
        assert!(task.is_converged());
        assert!(task.render_tags.is_empty());
        assert_eq!(task.current_draw_target_set_version, 0);
    }

    #[test]
    fn test_draw_target_task_set_params() {
        let mut task = HdxDrawTargetTask::new(Path::from_string("/drawTarget").unwrap());
        let mut params = HdxDrawTargetTaskParams::default();
        params.enable_lighting = true;
        params.alpha_threshold = 0.5;

        task.set_params(params.clone());
        assert!(task.get_params().enable_lighting);
        assert_eq!(task.get_params().alpha_threshold, 0.5);
    }
}
