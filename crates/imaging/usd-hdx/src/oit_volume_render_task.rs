//! OIT volume render task - Volume rendering with transparency.
//!
//! Extends HdxOitRenderTask with a "volume" render tag filter so only
//! volumetric draw items are rendered in this pass.
//!
//! Port of pxr/imaging/hdx/oitVolumeRenderTask.h/cpp

use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::oit_render_task::HdxOitRenderTaskParams;

/// Parameters for OIT volume render task.
///
/// Identical to HdxOitRenderTaskParams — C++ uses the same params type
/// (HdxOitVolumeRenderTask inherits HdxRenderTask, not HdxOitRenderTask,
/// but shares the same params shape for AOV/camera/viewport configuration).
pub type HdxOitVolumeRenderTaskParams = HdxOitRenderTaskParams;

/// OIT volume render task.
///
/// Renders volumetric geometry (smoke, clouds, volumes) using OIT buffers.
/// Only draw items tagged with the "volume" render tag are processed.
/// The companion HdxOitResolveTask composites the accumulated samples.
///
/// C++ reference: `HdxOitVolumeRenderTask` extends `HdxRenderTask`, overrides
/// `_Sync` to set the volume render tag, delegates everything else to base.
///
/// Port of pxr/imaging/hdx/oitVolumeRenderTask.h
pub struct HdxOitVolumeRenderTask {
    /// Task path.
    id: Path,

    /// Render tags — always contains "volume" to filter volumetric prims.
    render_tags: TfTokenVector,

    /// Whether OIT is enabled (checked once at construction time).
    is_oit_enabled: bool,

    /// Convergence state.
    converged: bool,
}

impl HdxOitVolumeRenderTask {
    /// Create new OIT volume render task.
    ///
    /// `is_oit_enabled` should match `HdxOitBufferAccessor::IsOitEnabled()`.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            // Volume tag filters this task to volumetric draw items only.
            render_tags: vec![Token::new("volume")],
            is_oit_enabled: true,
            converged: false,
        }
    }

    /// Override OIT enabled flag (mirrors `_isOitEnabled` field in C++).
    pub fn set_oit_enabled(&mut self, enabled: bool) {
        self.is_oit_enabled = enabled;
    }

    /// Check if OIT is enabled for this task.
    pub fn is_oit_enabled(&self) -> bool {
        self.is_oit_enabled
    }
}

impl HdTask for HdxOitVolumeRenderTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // C++ _Sync: skip if OIT not enabled, otherwise call HdxRenderTask::_Sync.
        if !self.is_oit_enabled {
            *dirty_bits = 0;
            return;
        }
        // In full implementation: delegate to HdxRenderTask base _Sync.
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // C++: skip if OIT disabled or no draw items, then call base Prepare +
        // HdxOitBufferAccessor::RequestOitBuffers + update AOV input textures.
        if !self.is_oit_enabled {
            return;
        }

        // Signal that volume OIT pass needs OIT buffers.
        ctx.insert(Token::new("oitVolumeRenderRequested"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: skip if disabled or no draw items, set render pass state
        // (depth test off, cull back, no MSAA), bind OIT buffers, execute.
        if !self.is_oit_enabled {
            self.converged = true;
            return;
        }

        // In full implementation:
        // 1. Get render pass state from context
        // 2. Request and init OIT buffers via HdxOitBufferAccessor
        // 3. Bind _oitVolumeRenderPassShader with OIT buffer bindings
        // 4. Disable depth test, depth write, MSAA; set cull back
        // 5. Transition depth texture to shader-read layout
        // 6. HdxRenderTask::Execute(ctx) — render volumetric draw items
        // 7. Restore depth texture layout

        self.converged = true;
        ctx.insert(
            Token::new("oitVolumeRenderTaskExecuted"),
            Value::from(format!("HdxOitVolumeRenderTask@{}", self.id.get_string())),
        );
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn is_converged(&self) -> bool {
        self.converged
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
    fn test_oit_volume_task_creation() {
        let task = HdxOitVolumeRenderTask::new(Path::from_string("/oitVolume").unwrap());
        assert!(!task.is_converged());
        assert_eq!(task.render_tags.len(), 1);
        assert_eq!(task.render_tags[0].as_str(), "volume");
        assert!(task.is_oit_enabled());
    }

    #[test]
    fn test_oit_volume_task_render_tag() {
        let task = HdxOitVolumeRenderTask::new(Path::from_string("/oitVolume").unwrap());
        let tags = task.get_render_tags();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].as_str(), "volume");
    }

    #[test]
    fn test_oit_volume_task_oit_disabled() {
        let mut task = HdxOitVolumeRenderTask::new(Path::from_string("/oitVolume").unwrap());
        task.set_oit_enabled(false);
        assert!(!task.is_oit_enabled());

        let mut ctx = HdTaskContext::new();
        task.execute(&mut ctx);
        // When OIT disabled, task marks as converged and skips rendering.
        assert!(task.is_converged());
        assert!(!ctx.contains_key(&Token::new("oitVolumeRenderTaskExecuted")));
    }

    #[test]
    fn test_oit_volume_task_execute() {
        let mut task = HdxOitVolumeRenderTask::new(Path::from_string("/oitVolume").unwrap());
        let mut ctx = HdTaskContext::new();

        task.execute(&mut ctx);
        assert!(task.is_converged());
        assert!(ctx.contains_key(&Token::new("oitVolumeRenderTaskExecuted")));
    }

    #[test]
    fn test_oit_volume_params_type_alias() {
        // HdxOitVolumeRenderTaskParams is an alias for HdxOitRenderTaskParams.
        let params = HdxOitVolumeRenderTaskParams::default();
        assert!(params.enable_oit);
    }
}
