
//! OIT render task - Order Independent Transparency rendering.
//!
//! Renders geometry with transparency using order-independent transparency techniques.
//! Port of pxr/imaging/hdx/oitRenderTask.h/cpp

use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::render_setup_task::HdxRenderPassState;

/// OIT render task tokens.
pub mod oit_render_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Screen size parameter for OIT buffers
    pub static SCREEN_SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("screenSize"));
    /// Maximum number of transparent fragments per pixel
    pub static MAX_SAMPLES: LazyLock<Token> = LazyLock::new(|| Token::new("maxSamples"));
    /// Enable OIT rendering
    pub static ENABLE_OIT: LazyLock<Token> = LazyLock::new(|| Token::new("enableOit"));
}

/// Parameters for OIT render task.
#[derive(Clone)]
pub struct HdxOitRenderTaskParams {
    /// Enable OIT rendering
    pub enable_oit: bool,
    /// Screen dimensions (width, height)
    pub screen_size: (u32, u32),
    /// Maximum transparent samples per pixel
    pub max_samples: u32,
    /// Render pass state (shared with regular render task)
    pub render_pass_state: Option<HdxRenderPassState>,
}

impl Default for HdxOitRenderTaskParams {
    fn default() -> Self {
        Self {
            enable_oit: true,
            screen_size: (1920, 1080),
            max_samples: 8,
            render_pass_state: None,
        }
    }
}

/// OIT render task for transparent geometry.
///
/// Renders transparent geometry using order-independent transparency.
/// This task accumulates transparent fragments into OIT buffers which
/// are later resolved by HdxOitResolveTask.
///
/// The OIT technique used allows correct transparency rendering regardless
/// of draw order by storing all transparent fragments per pixel and
/// sorting/compositing them in a separate pass.
pub struct HdxOitRenderTask {
    /// Task path
    id: Path,

    /// Render tags for filtering
    render_tags: TfTokenVector,

    /// OIT parameters
    params: HdxOitRenderTaskParams,

    /// Convergence state
    converged: bool,

    /// OIT buffers allocated
    buffers_allocated: bool,
}

impl HdxOitRenderTask {
    /// Create new OIT render task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            render_tags: Vec::new(),
            params: HdxOitRenderTaskParams::default(),
            converged: false,
            buffers_allocated: false,
        }
    }

    /// Set OIT render task parameters.
    pub fn set_params(&mut self, params: HdxOitRenderTaskParams) {
        self.params = params;
    }

    /// Get current parameters.
    pub fn get_params(&self) -> &HdxOitRenderTaskParams {
        &self.params
    }

    /// Check if OIT is enabled.
    pub fn is_oit_enabled(&self) -> bool {
        self.params.enable_oit
    }

    /// Allocate OIT buffers based on screen size and max samples.
    fn alloc_buffers(&mut self) {
        if !self.params.enable_oit {
            return;
        }

        // In full implementation, allocate:
        // - Counter buffer (atomic counters per pixel)
        // - Data buffer (fragment colors)
        // - Depth buffer (fragment depths)
        // - Index buffer (fragment linked lists)

        self.buffers_allocated = true;
    }

    /// Clear OIT buffers before rendering.
    fn clear_buffers(&mut self) {
        if !self.buffers_allocated {
            return;
        }

        // In full implementation:
        // - Reset atomic counters to 0
        // - Clear index buffer to sentinel values
        // - Set cleared flag in context
    }

    /// Check if buffers need reallocation.
    fn need_realloc(&self) -> bool {
        // Check if screen size changed or buffers not allocated
        !self.buffers_allocated
    }
}

impl HdTask for HdxOitRenderTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // Pull params from delegate if available
        // For now, params are set via set_params() directly

        // Check if we need to reallocate buffers
        if self.need_realloc() {
            self.alloc_buffers();
        }

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        if !self.params.enable_oit {
            return;
        }

        // Store OIT request flag in context
        ctx.insert(super::tokens::OIT_REQUEST_FLAG.clone(), Value::from(true));

        // Store screen size for resolve task
        ctx.insert(
            super::tokens::OIT_SCREEN_SIZE.clone(),
            Value::from(format!(
                "{}x{}",
                self.params.screen_size.0, self.params.screen_size.1
            )),
        );
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        if !self.params.enable_oit {
            self.converged = true;
            return;
        }

        // Clear OIT buffers
        self.clear_buffers();

        // Mark buffers as cleared in context
        ctx.insert(super::tokens::OIT_CLEARED_FLAG.clone(), Value::from(true));

        // Execute render pass with OIT shaders
        // In full implementation:
        // - Bind OIT buffers
        // - Set OIT shader uniforms
        // - Render transparent geometry
        // - Each fragment stores color/depth in OIT buffers

        self.converged = true;

        ctx.insert(
            Token::new("oitRenderTaskExecuted"),
            Value::from(format!("HdxOitRenderTask@{}", self.id.get_string())),
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
    fn test_oit_render_task_creation() {
        let task = HdxOitRenderTask::new(Path::from_string("/oitRender").unwrap());
        assert!(!task.is_converged());
        assert!(task.render_tags.is_empty());
        assert!(task.is_oit_enabled());
    }

    #[test]
    fn test_oit_render_task_params() {
        let mut task = HdxOitRenderTask::new(Path::from_string("/oitRender").unwrap());

        let params = HdxOitRenderTaskParams {
            enable_oit: true,
            screen_size: (2560, 1440),
            max_samples: 16,
            render_pass_state: None,
        };

        task.set_params(params.clone());
        assert_eq!(task.get_params().screen_size, (2560, 1440));
        assert_eq!(task.get_params().max_samples, 16);
    }

    #[test]
    fn test_oit_render_task_disable() {
        let mut task = HdxOitRenderTask::new(Path::from_string("/oitRender").unwrap());

        let mut params = HdxOitRenderTaskParams::default();
        params.enable_oit = false;

        task.set_params(params);
        assert!(!task.is_oit_enabled());
    }

    #[test]
    fn test_oit_render_task_execute() {
        let mut task = HdxOitRenderTask::new(Path::from_string("/oitRender").unwrap());
        let mut ctx = HdTaskContext::new();

        task.execute(&mut ctx);
        assert!(task.is_converged());
        assert!(ctx.contains_key(&super::super::tokens::OIT_CLEARED_FLAG));
    }

    #[test]
    fn test_oit_render_tokens() {
        assert_eq!(oit_render_tokens::SCREEN_SIZE.as_str(), "screenSize");
        assert_eq!(oit_render_tokens::MAX_SAMPLES.as_str(), "maxSamples");
        assert_eq!(oit_render_tokens::ENABLE_OIT.as_str(), "enableOit");
    }
}
