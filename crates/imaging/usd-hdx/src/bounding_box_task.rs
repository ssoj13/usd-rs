//! Bounding box task - Visualize bounding boxes for scene geometry.
//!
//! Renders axis-aligned or oriented bounding boxes for debugging and visualization.
//! Port of pxr/imaging/hdx/boundingBoxTask.h/cpp

use usd_gf::{Vec3f, Vec4f};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Bounding box task tokens.
pub mod bbox_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Bounding box color parameter.
    pub static BBOX_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("bboxColor"));

    /// Bounding box line width parameter.
    pub static BBOX_LINE_WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("bboxLineWidth"));

    /// Show bounding box for selected objects only.
    pub static BBOX_SELECTED_ONLY: LazyLock<Token> =
        LazyLock::new(|| Token::new("bboxSelectedOnly"));
}

/// Bounding box visualization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BBoxMode {
    /// Axis-aligned bounding boxes (AABB).
    AxisAligned,

    /// Oriented bounding boxes (OBB).
    Oriented,

    /// Both AABB and OBB.
    Both,
}

impl Default for BBoxMode {
    fn default() -> Self {
        Self::AxisAligned
    }
}

/// Bounding box task parameters.
///
/// Configures which objects to show bounding boxes for and how to render them.
/// Port of HdxBoundingBoxTaskParams from pxr/imaging/hdx/boundingBoxTask.h
#[derive(Debug, Clone)]
pub struct HdxBoundingBoxTaskParams {
    /// Color for bounding box lines.
    pub color: Vec4f,

    /// Line width in pixels.
    pub line_width: f32,

    /// Bounding box visualization mode.
    pub mode: BBoxMode,

    /// Show bounding boxes for selected objects only.
    pub selected_only: bool,

    /// Alpha blending enabled.
    pub enable_alpha: bool,

    /// AOV buffer name this task reads from / renders into.
    /// Set by task controller via SetViewportRenderOutput.
    pub aov_name: usd_tf::Token,

    // C++ HdxBoundingBoxTaskParams also has bboxes and dashSize;
    // simplified here to color/dashSize subset used by SetBBoxParams.
    /// Dash size for dashed-line bounding box rendering.
    pub dash_size: f32,
}

impl Default for HdxBoundingBoxTaskParams {
    fn default() -> Self {
        Self {
            color: Vec4f::new(1.0, 0.5, 0.0, 1.0),
            line_width: 1.0,
            mode: BBoxMode::AxisAligned,
            selected_only: false,
            enable_alpha: true,
            aov_name: usd_tf::Token::new("color"),
            dash_size: 0.0,
        }
    }
}

/// Bounding box visualization task.
///
/// A task for rendering bounding boxes around scene geometry.
/// Useful for debugging spatial relationships, culling, and selection visualization.
///
/// The task queries bounding box data from prims and renders wireframe
/// boxes in world space.
///
/// Port of HdxBoundingBoxTask from pxr/imaging/hdx/boundingBoxTask.h
pub struct HdxBoundingBoxTask {
    /// Task path.
    id: Path,

    /// Task parameters.
    params: HdxBoundingBoxTaskParams,

    /// Render tags for filtering.
    render_tags: TfTokenVector,

    /// Cached bounding box data for rendering.
    bbox_data: Vec<BoundingBoxData>,
}

/// Internal structure for cached bounding box rendering data.
#[derive(Debug, Clone)]
struct BoundingBoxData {
    /// Prim path.
    #[allow(dead_code)]
    prim_path: Path,

    /// Bounding box min corner.
    #[allow(dead_code)]
    min: Vec3f,

    /// Bounding box max corner.
    #[allow(dead_code)]
    max: Vec3f,
}

impl HdxBoundingBoxTask {
    /// Create new bounding box task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxBoundingBoxTaskParams::default(),
            render_tags: Vec::new(),
            bbox_data: Vec::new(),
        }
    }

    /// Set bounding box parameters.
    pub fn set_params(&mut self, params: HdxBoundingBoxTaskParams) {
        self.params = params;
    }

    /// Get bounding box parameters.
    pub fn get_params(&self) -> &HdxBoundingBoxTaskParams {
        &self.params
    }

    /// Get the AOV name this task is associated with.
    pub fn get_aov_name(&self) -> &usd_tf::Token {
        &self.params.aov_name
    }

    /// Set the AOV name (called by task controller SetViewportRenderOutput).
    pub fn set_aov_name(&mut self, name: usd_tf::Token) {
        self.params.aov_name = name;
    }

    /// Get number of bounding boxes to render.
    pub fn get_bbox_count(&self) -> usize {
        self.bbox_data.len()
    }
}

impl HdTask for HdxBoundingBoxTask {
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

    fn prepare(&mut self, ctx: &mut HdTaskContext, render_index: &dyn HdRenderIndexTrait) {
        // Clear previous bbox data
        self.bbox_data.clear();

        // In full implementation:
        // 1. Query selection tracker if selected_only is true
        // 2. Iterate through render index prims
        // 3. Get bounding box extent from each prim
        // 4. Build vertex/index buffers for wireframe boxes
        // 5. Create GPU resources for rendering

        let _ = render_index;

        ctx.insert(Token::new("bboxPrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        if self.bbox_data.is_empty() {
            return;
        }

        // In full implementation:
        // 1. Set up line rendering state (line width, depth test, blend)
        // 2. Bind bbox shader with color uniform
        // 3. Draw wireframe boxes for each bounding box
        // 4. Restore previous render state

        ctx.insert(
            Token::new("bboxTaskExecuted"),
            Value::from(format!("HdxBoundingBoxTask@{}", self.id.get_string())),
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
    fn test_bbox_tokens() {
        use bbox_tokens::*;
        assert_eq!(BBOX_COLOR.as_str(), "bboxColor");
        assert_eq!(BBOX_LINE_WIDTH.as_str(), "bboxLineWidth");
        assert_eq!(BBOX_SELECTED_ONLY.as_str(), "bboxSelectedOnly");
    }

    #[test]
    fn test_bbox_mode() {
        assert_eq!(BBoxMode::default(), BBoxMode::AxisAligned);
        assert_ne!(BBoxMode::AxisAligned, BBoxMode::Oriented);
    }

    #[test]
    fn test_bbox_params_default() {
        let params = HdxBoundingBoxTaskParams::default();
        assert_eq!(params.line_width, 1.0);
        assert_eq!(params.mode, BBoxMode::AxisAligned);
        assert!(!params.selected_only);
        assert!(params.enable_alpha);
    }

    #[test]
    fn test_bbox_task_creation() {
        let task = HdxBoundingBoxTask::new(Path::from_string("/bbox").unwrap());
        assert_eq!(task.get_bbox_count(), 0);
        assert!(task.render_tags.is_empty());
    }

    #[test]
    fn test_bbox_task_set_params() {
        let mut task = HdxBoundingBoxTask::new(Path::from_string("/bbox").unwrap());

        let mut params = HdxBoundingBoxTaskParams::default();
        params.color = Vec4f::new(0.0, 1.0, 0.0, 1.0); // Green
        params.line_width = 2.0;
        params.mode = BBoxMode::Oriented;
        params.selected_only = true;

        task.set_params(params.clone());
        assert_eq!(task.get_params().line_width, 2.0);
        assert_eq!(task.get_params().mode, BBoxMode::Oriented);
        assert!(task.get_params().selected_only);
    }

    #[test]
    fn test_bbox_task_execute() {
        let mut task = HdxBoundingBoxTask::new(Path::from_string("/bbox").unwrap());
        let mut ctx = HdTaskContext::new();

        // Execute should handle empty bbox data gracefully
        task.execute(&mut ctx);
        assert!(!ctx.contains_key(&Token::new("bboxTaskExecuted")));
    }
}
