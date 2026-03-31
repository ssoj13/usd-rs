
//! AOV input task - AOV (Arbitrary Output Variable) input processing.
//!
//! Records which rendered AOV should become the canonical downstream input.
//!
//! In this port the task does not publish concrete GPU handles by itself,
//! because those handles only exist after backend rendering has completed. It
//! therefore emits a deferred request that `usd_imaging::gl::Engine` replays
//! once the frame AOVs are available.
//! Port of pxr/imaging/hdx/aovInputTask.h/cpp

use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Backend execution request emitted by `HdxAovInputTask::execute()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdxAovInputTaskRequest {
    /// The viewport AOV exposed downstream as the canonical "color" input.
    pub aov_name: Token,
    /// Whether the corresponding depth target should also be exposed.
    pub include_depth: bool,
}

/// AOV input task tokens.
pub mod aov_input_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// AOV input buffer resource.
    pub static AOV_BUFFER_INPUT: LazyLock<Token> = LazyLock::new(|| Token::new("aovBufferInput"));

    /// AOV name parameter.
    pub static AOV_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("aovName"));

    /// AOV read-only flag.
    pub static READ_ONLY: LazyLock<Token> = LazyLock::new(|| Token::new("readOnly"));
}

/// AOV input task parameters.
///
/// Matches C++ HdxAovInputTaskParams (pxr/imaging/hdx/aovInputTask.h).
/// The task reads the color AOV buffer and (optionally) the depth buffer,
/// resolves multisampled images and presents them to downstream tasks.
#[derive(Debug, Clone)]
pub struct HdxAovInputTaskParams {
    /// Path to the primary AOV render buffer (e.g. color).
    pub aov_buffer_path: Path,

    /// Path to the depth render buffer (empty if not used).
    pub depth_buffer_path: Path,

    // Legacy alias kept for compatibility.
    /// Path to the render buffer containing the AOV data (alias for aov_buffer_path).
    pub render_buffer_path: Path,

    /// Name of the AOV to read (e.g., "color", "depth", "normal").
    pub aov_name: Token,

    /// Whether buffer is read-only (no modifications allowed).
    pub read_only: bool,

    /// Whether to clear buffer after reading.
    pub clear_after_read: bool,
}

impl Default for HdxAovInputTaskParams {
    fn default() -> Self {
        Self {
            aov_buffer_path: Path::empty(),
            depth_buffer_path: Path::empty(),
            render_buffer_path: Path::empty(),
            aov_name: Token::new("color"),
            read_only: true,
            clear_after_read: false,
        }
    }
}

/// AOV input task.
///
/// A task for reading AOV data from render buffers and making it
/// available to downstream tasks in the rendering pipeline.
///
/// Typical use case: reading intermediate render results for
/// post-processing, compositing, or visualization.
///
/// Port of HdxAovInputTask from pxr/imaging/hdx/aovInputTask.h
pub struct HdxAovInputTask {
    /// Task path.
    id: Path,

    /// Task parameters.
    params: HdxAovInputTaskParams,

    /// Render tags for filtering.
    render_tags: TfTokenVector,

    /// Whether task has valid input.
    has_valid_input: bool,

    /// Whether AOV buffers have converged (C++: _converged).
    converged: bool,
}

impl HdxAovInputTask {
    /// Create new AOV input task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxAovInputTaskParams::default(),
            render_tags: Vec::new(),
            has_valid_input: false,
            converged: false,
        }
    }

    /// Set AOV input parameters.
    pub fn set_params(&mut self, params: HdxAovInputTaskParams) {
        self.params = params;
    }

    /// Get AOV input parameters.
    pub fn get_params(&self) -> &HdxAovInputTaskParams {
        &self.params
    }

    /// Check if task has valid AOV input data.
    pub fn has_valid_input(&self) -> bool {
        self.has_valid_input
    }
}

impl HdTask for HdxAovInputTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In full implementation: pull params from scene delegate
        // For now, params are set via set_params() directly

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // C++: Prepare() calls _GetHgi()->StartFrame() to wrap one HdEngine::Execute
        // frame with StartFrame/EndFrame for Hgi garbage collection.
        // In this port EndFrame is deferred to the engine's post-backend bridge.
        // Extract driver handle first to release the immutable borrow on ctx.
        let driver_handle = usd_hd::render::task::HdTaskBase::get_driver(ctx, &Token::new("renderDriver"))
            .and_then(|v| {
                use usd_hgi::HgiDriverHandle;
                v.get::<HgiDriverHandle>().map(|h| h.get().clone())
            });
        if let Some(hgi_arc) = driver_handle {
            hgi_arc.write().start_frame();
            ctx.insert(Token::new("hgiFrameStarted"), Value::from(true));
        }

        // Valid if either the new aov_buffer_path or legacy render_buffer_path is set.
        self.has_valid_input =
            !self.params.aov_buffer_path.is_empty() || !self.params.render_buffer_path.is_empty();
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: early-out if no aov buffer; task is immediately converged.
        if !self.has_valid_input {
            self.converged = true;
            return;
        }

        // C++: _converged = _aovBuffer->IsConverged() && _depthBuffer->IsConverged()
        // We don't own real render buffers in the wgpu path, so assume converged.
        self.converged = true;

        // Real AOV textures do not exist until the engine has executed the backend
        // draw passes, so this task only records what AOV exposure is required.
        let request = HdxAovInputTaskRequest {
            aov_name: self.params.aov_name.clone(),
            include_depth: !self.params.depth_buffer_path.is_empty(),
        };
        let requests_token = Token::new("aovInputTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxAovInputTaskRequest>>())
        {
            requests.push(request);
        } else {
            ctx.insert(requests_token, Value::new(vec![request]));
        }
        let order_token = Token::new("postTaskOrder");
        if let Some(order) = ctx
            .get_mut(&order_token)
            .and_then(|value| value.get_mut::<Vec<Token>>())
        {
            order.push(Token::new("aovInput"));
        } else {
            ctx.insert(order_token, Value::new(vec![Token::new("aovInput")]));
        }
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
    fn test_aov_input_tokens() {
        use aov_input_tokens::*;
        assert_eq!(AOV_BUFFER_INPUT.as_str(), "aovBufferInput");
        assert_eq!(AOV_NAME.as_str(), "aovName");
        assert_eq!(READ_ONLY.as_str(), "readOnly");
    }

    #[test]
    fn test_aov_input_params_default() {
        let params = HdxAovInputTaskParams::default();
        assert_eq!(params.aov_name.as_str(), "color");
        assert!(params.read_only);
        assert!(!params.clear_after_read);
        assert!(params.render_buffer_path.is_empty());
    }

    #[test]
    fn test_aov_input_task_creation() {
        let task = HdxAovInputTask::new(Path::from_string("/aovInput").unwrap());
        assert!(!task.has_valid_input());
        assert!(task.render_tags.is_empty());
    }

    #[test]
    fn test_aov_input_task_set_params() {
        let mut task = HdxAovInputTask::new(Path::from_string("/aovInput").unwrap());

        let mut params = HdxAovInputTaskParams::default();
        params.aov_name = Token::new("depth");
        params.render_buffer_path = Path::from_string("/RenderBuffers/depth").unwrap();
        params.read_only = false;

        task.set_params(params.clone());
        assert_eq!(task.get_params().aov_name.as_str(), "depth");
        assert!(!task.get_params().read_only);
    }

    #[test]
    fn test_aov_input_task_execute() {
        let mut task = HdxAovInputTask::new(Path::from_string("/aovInput").unwrap());
        let mut ctx = HdTaskContext::new();

        // Set valid params and simulate prepare
        let mut params = HdxAovInputTaskParams::default();
        params.render_buffer_path = Path::from_string("/RenderBuffers/color").unwrap();
        task.set_params(params);
        task.has_valid_input = true;

        // Execute should enqueue a backend request and execution order entry.
        task.execute(&mut ctx);
        let requests = ctx
            .get(&Token::new("aovInputTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxAovInputTaskRequest>>())
            .cloned()
            .expect("execute must enqueue AOV input request");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].aov_name.as_str(), "color");
        assert!(!requests[0].include_depth);
        let order = ctx
            .get(&Token::new("postTaskOrder"))
            .and_then(|value| value.get::<Vec<Token>>())
            .cloned()
            .expect("execute must append post task order");
        assert_eq!(order, vec![Token::new("aovInput")]);
        assert!(task.is_converged());
    }
}
