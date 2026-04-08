//! Visualize AOV task - Visualize AOV (Arbitrary Output Variable) buffers.
//!
//! Records deferred visualization requests for AOV debugging and inspection.
//!
//! The actual visualization pass is replayed later by
//! `usd_imaging::gl::Engine`, once the requested AOV texture has been produced
//! by backend rendering. The live engine-side replay path currently covers raw
//! fallback display, depth renormalization, ID hashing, and normal
//! visualization.
//! Port of pxr/imaging/hdx/visualizeAovTask.h/cpp

use usd_gf::{Vec2f, Vec4f};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Backend execution request emitted by `HdxVisualizeAovTask::execute()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdxVisualizeAovTaskRequest {
    /// AOV selected for visualization.
    pub aov_name: Token,
    /// Visualization mode.
    pub mode: AovVisMode,
    /// Selected channel (`-1` means all).
    pub channel: i32,
}

/// Visualize AOV task tokens.
pub mod visualize_aov_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// AOV name to visualize.
    pub static AOV_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("aovName"));

    /// Visualization color ramp.
    pub static COLOR_RAMP: LazyLock<Token> = LazyLock::new(|| Token::new("colorRamp"));

    /// Value range for visualization.
    pub static VALUE_RANGE: LazyLock<Token> = LazyLock::new(|| Token::new("valueRange"));
}

/// AOV visualization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AovVisMode {
    /// Display raw values (no processing).
    Raw,

    /// Display as grayscale (luminance).
    Grayscale,

    /// Apply false color mapping (heat map).
    FalseColor,

    /// Display channel split (R/G/B/A separately).
    ChannelSplit,

    /// Display normalized depth.
    Depth,

    /// Display normals as colors.
    Normal,
}

impl Default for AovVisMode {
    fn default() -> Self {
        Self::Raw
    }
}

/// Visualize AOV task parameters.
///
/// Configures how to visualize AOV buffer data.
/// Port of HdxVisualizeAovTaskParams from pxr/imaging/hdx/visualizeAovTask.h
#[derive(Debug, Clone)]
pub struct HdxVisualizeAovTaskParams {
    /// Name of AOV to visualize (e.g., "color", "depth", "normal", "primId").
    pub aov_name: Token,

    /// Visualization mode.
    pub mode: AovVisMode,

    /// Value range for remapping [min, max].
    pub value_range: Vec2f,

    /// Whether to auto-compute value range.
    pub auto_range: bool,

    /// False color gradient (for heat map mode).
    pub color_ramp: Vec<Vec4f>,

    /// Channel to display (0=R, 1=G, 2=B, 3=A, -1=all).
    pub channel: i32,

    /// Enable visualization (false = pass through).
    pub enable: bool,
}

impl Default for HdxVisualizeAovTaskParams {
    fn default() -> Self {
        Self {
            aov_name: Token::new("color"),
            mode: AovVisMode::Raw,
            value_range: Vec2f::new(0.0, 1.0),
            auto_range: false,
            color_ramp: vec![
                Vec4f::new(0.0, 0.0, 1.0, 1.0), // Blue (cold)
                Vec4f::new(0.0, 1.0, 0.0, 1.0), // Green
                Vec4f::new(1.0, 1.0, 0.0, 1.0), // Yellow
                Vec4f::new(1.0, 0.0, 0.0, 1.0), // Red (hot)
            ],
            channel: -1,
            enable: true,
        }
    }
}

/// AOV visualization task.
///
/// A task for visualizing AOV (Arbitrary Output Variable) buffers.
/// Useful for debugging render passes, inspecting intermediate results,
/// and analyzing render data like depth, normals, IDs, etc.
///
/// The task reads an AOV buffer from the render target and applies
/// various visualization modes to make the data human-readable.
///
/// Port of HdxVisualizeAovTask from pxr/imaging/hdx/visualizeAovTask.h
pub struct HdxVisualizeAovTask {
    /// Task path.
    id: Path,

    /// Task parameters.
    params: HdxVisualizeAovTaskParams,

    /// Render tags for filtering.
    render_tags: TfTokenVector,

    /// Whether AOV buffer is available.
    aov_available: bool,

    /// Cached min/max values for auto-range.
    cached_range: Vec2f,
}

impl HdxVisualizeAovTask {
    /// Create new visualize AOV task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxVisualizeAovTaskParams::default(),
            render_tags: Vec::new(),
            aov_available: false,
            cached_range: Vec2f::new(0.0, 1.0),
        }
    }

    /// Set visualization parameters.
    pub fn set_params(&mut self, params: HdxVisualizeAovTaskParams) {
        self.params = params;
    }

    /// Get visualization parameters.
    pub fn get_params(&self) -> &HdxVisualizeAovTaskParams {
        &self.params
    }

    /// Set the AOV name to visualize (called by task controller SetViewportRenderOutput).
    pub fn set_aov_name(&mut self, name: usd_tf::Token) {
        self.params.aov_name = name;
    }

    /// Check if AOV data is available.
    pub fn is_aov_available(&self) -> bool {
        self.aov_available
    }

    /// Get cached value range (for auto-range mode).
    pub fn get_cached_range(&self) -> Vec2f {
        self.cached_range
    }
}

impl HdTask for HdxVisualizeAovTask {
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
        if !self.params.enable {
            return;
        }

        // Real AOV textures are only bridged after backend rendering, so prepare
        // can only record high-level intent here.
        self.aov_available = !self.params.aov_name.is_empty();

        // In full implementation:
        // 1. Resolve AOV binding from render target
        // 2. Validate AOV format is supported
        // 3. Prepare visualization shader based on mode
        // 4. If auto_range: compute min/max from AOV data
        // 5. Upload color ramp texture (for false color mode)
        // 6. Set up fullscreen quad for post-processing

        if self.params.auto_range && self.aov_available {
            // Simulate auto-range computation
            self.cached_range = Vec2f::new(0.0, 10.0);
        }

        ctx.insert(Token::new("visualizePrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        if !self.params.enable || !self.aov_available {
            return;
        }
        let request = HdxVisualizeAovTaskRequest {
            aov_name: self.params.aov_name.clone(),
            mode: self.params.mode,
            channel: self.params.channel,
        };
        let requests_token = Token::new("visualizeAovTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxVisualizeAovTaskRequest>>())
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
            order.push(Token::new("visualizeAov"));
        } else {
            ctx.insert(order_token, Value::new(vec![Token::new("visualizeAov")]));
        }
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
    fn test_visualize_aov_tokens() {
        use visualize_aov_tokens::*;
        assert_eq!(AOV_NAME.as_str(), "aovName");
        assert_eq!(COLOR_RAMP.as_str(), "colorRamp");
        assert_eq!(VALUE_RANGE.as_str(), "valueRange");
    }

    #[test]
    fn test_aov_vis_mode() {
        assert_eq!(AovVisMode::default(), AovVisMode::Raw);
        assert_ne!(AovVisMode::Raw, AovVisMode::FalseColor);
        assert_ne!(AovVisMode::Depth, AovVisMode::Normal);
    }

    #[test]
    fn test_visualize_aov_params_default() {
        let params = HdxVisualizeAovTaskParams::default();
        assert_eq!(params.aov_name.as_str(), "color");
        assert_eq!(params.mode, AovVisMode::Raw);
        assert_eq!(params.value_range, Vec2f::new(0.0, 1.0));
        assert!(!params.auto_range);
        assert_eq!(params.channel, -1);
        assert!(params.enable);
        assert_eq!(params.color_ramp.len(), 4);
    }

    #[test]
    fn test_visualize_aov_task_creation() {
        let task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());
        assert!(!task.is_aov_available());
        assert!(task.render_tags.is_empty());
        assert_eq!(task.get_cached_range(), Vec2f::new(0.0, 1.0));
    }

    #[test]
    fn test_visualize_aov_task_set_params() {
        let mut task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());

        let mut params = HdxVisualizeAovTaskParams::default();
        params.aov_name = Token::new("depth");
        params.mode = AovVisMode::FalseColor;
        params.value_range = Vec2f::new(0.0, 100.0);
        params.auto_range = true;
        params.channel = 0; // Red channel only

        task.set_params(params.clone());
        assert_eq!(task.get_params().aov_name.as_str(), "depth");
        assert_eq!(task.get_params().mode, AovVisMode::FalseColor);
        assert!(task.get_params().auto_range);
        assert_eq!(task.get_params().channel, 0);
    }

    #[test]
    fn test_visualize_aov_task_availability() {
        let mut task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());

        // Initially no AOV
        assert!(!task.is_aov_available());

        // Simulate AOV availability
        task.aov_available = true;
        assert!(task.is_aov_available());
    }

    #[test]
    fn test_visualize_aov_task_auto_range() {
        let mut task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());

        let mut params = HdxVisualizeAovTaskParams::default();
        params.auto_range = true;
        task.set_params(params);

        // Initially default range
        assert_eq!(task.get_cached_range(), Vec2f::new(0.0, 1.0));

        // Simulate auto-range computation
        task.cached_range = Vec2f::new(0.0, 10.0);
        assert_eq!(task.get_cached_range(), Vec2f::new(0.0, 10.0));
    }

    #[test]
    fn test_visualize_aov_task_execute() {
        let mut task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());
        let mut ctx = HdTaskContext::new();

        // Without AOV should not execute
        task.execute(&mut ctx);
        assert!(!ctx.contains_key(&Token::new("visualizeAovTaskRequests")));

        // With AOV should execute
        task.aov_available = true;
        task.execute(&mut ctx);
        let requests = ctx
            .get(&Token::new("visualizeAovTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxVisualizeAovTaskRequest>>())
            .cloned()
            .expect("available AOV must enqueue visualize request");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].aov_name, task.get_params().aov_name);
        let order = ctx
            .get(&Token::new("postTaskOrder"))
            .and_then(|value| value.get::<Vec<Token>>())
            .cloned()
            .expect("available AOV must append post task order");
        assert_eq!(order, vec![Token::new("visualizeAov")]);
    }
}
