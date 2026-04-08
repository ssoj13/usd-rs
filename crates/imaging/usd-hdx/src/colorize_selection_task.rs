//! Colorize selection task - Apply highlight colors to selected objects.
//!
//! Emits a deferred post-process request for selection highlighting.
//!
//! The actual fullscreen composite is replayed later by
//! `usd_imaging::gl::Engine`, after backend rendering has produced real color
//! and ID AOV textures.
//!
//! The engine-side replay path now consumes the HDX selection task's
//! `selectionBuffer` contract directly, including `select`/`locate` modes and
//! hierarchical `prim -> instance -> element` decoding in the compositor.
//! Port of pxr/imaging/hdx/colorizeSelectionTask.h/cpp

use std::hash::{Hash, Hasher};

use usd_gf::Vec4f;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Backend execution request emitted by `HdxColorizeSelectionTask::execute()`.
#[derive(Debug, Clone, PartialEq)]
pub struct HdxColorizeSelectionTaskRequest {
    /// Whether selection data exists for this frame.
    pub has_selection: bool,
    /// Whether prim ID input is wired for the current viewport AOV.
    pub is_active: bool,
    /// Whether locate/rollover highlighting is enabled.
    pub enable_locate_highlight: bool,
    /// Whether outline mode is enabled.
    pub enable_outline: bool,
    /// Selection highlight color.
    pub selection_color: Vec4f,
    /// Locate/rollover highlight color.
    pub locate_color: Vec4f,
    /// Outline thickness in pixels.
    pub outline_radius: u32,
}

/// Hash via f32::to_bits so the struct can live inside vt::Value.
impl Hash for HdxColorizeSelectionTaskRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.has_selection.hash(state);
        self.is_active.hash(state);
        self.enable_locate_highlight.hash(state);
        self.enable_outline.hash(state);
        for c in [
            self.selection_color.x,
            self.selection_color.y,
            self.selection_color.z,
            self.selection_color.w,
        ] {
            c.to_bits().hash(state);
        }
        for c in [
            self.locate_color.x,
            self.locate_color.y,
            self.locate_color.z,
            self.locate_color.w,
        ] {
            c.to_bits().hash(state);
        }
        self.outline_radius.hash(state);
    }
}

/// Colorize selection task tokens.
pub mod colorize_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Primary selection color.
    pub static PRIMARY_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("primaryColor"));

    /// Secondary selection color (for multi-select).
    pub static SECONDARY_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("secondaryColor"));

    /// Selection outline width in pixels.
    pub static OUTLINE_WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("outlineWidth"));

    /// Selection glow intensity.
    pub static GLOW_INTENSITY: LazyLock<Token> = LazyLock::new(|| Token::new("glowIntensity"));
}

/// Selection colorization mode (extended, beyond C++ enableOutline bool).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorizeMode {
    Outline,
    Fill,
    Overlay,
    Glow,
}

impl Default for ColorizeMode {
    fn default() -> Self {
        Self::Outline
    }
}

/// Colorize selection task parameters.
///
/// Core fields match C++ HdxColorizeSelectionTaskParams (colorizeSelectionTask.h).
/// Additional fields extend functionality for the wgpu backend.
#[derive(Debug, Clone)]
pub struct HdxColorizeSelectionTaskParams {
    /// Enable primary selection highlight.
    pub enable_selection_highlight: bool,

    /// Enable locate (rollover) highlight.
    pub enable_locate_highlight: bool,

    /// Primary selection color.
    pub selection_color: Vec4f,

    /// Locate/rollover color.
    pub locate_color: Vec4f,

    /// Render selection as outline instead of solid fill.
    pub enable_outline: bool,

    /// Outline thickness in pixels.
    pub outline_radius: u32,

    // AOV buffer paths — set by SetViewportRenderOutput when color AOV is active.
    /// Path to primId render buffer for GPU-based selection resolve.
    pub prim_id_buffer_path: Path,

    /// Path to instanceId render buffer.
    pub instance_id_buffer_path: Path,

    /// Path to elementId render buffer.
    pub element_id_buffer_path: Path,

    // Aliases kept for compatibility with task_controller internal propagation.
    /// Alias for selection_color (GPU shader name).
    pub primary_color: Vec4f,

    /// Alias for locate_color (GPU shader name).
    pub secondary_color: Vec4f,

    /// Float alias for outline_radius.
    pub outline_width: f32,

    /// Masking mode (only render selected objects).
    pub enable_masking: bool,

    /// Extended colorization mode enum.
    pub mode: ColorizeMode,
}

impl Default for HdxColorizeSelectionTaskParams {
    fn default() -> Self {
        Self {
            enable_selection_highlight: true,
            enable_locate_highlight: true,
            selection_color: Vec4f::new(1.0, 1.0, 0.0, 1.0),
            locate_color: Vec4f::new(0.0, 0.0, 1.0, 1.0),
            enable_outline: true,
            outline_radius: 1,
            prim_id_buffer_path: Path::empty(),
            instance_id_buffer_path: Path::empty(),
            element_id_buffer_path: Path::empty(),
            primary_color: Vec4f::new(1.0, 1.0, 0.0, 1.0),
            secondary_color: Vec4f::new(0.0, 0.0, 1.0, 1.0),
            outline_width: 2.0,
            enable_masking: false,
            mode: ColorizeMode::Outline,
        }
    }
}

/// Selection colorization task.
///
/// Applies post-process colorization for selection highlights.
/// Reads primId/instanceId/elementId buffers to identify selected pixels,
/// then composites outline or fill effects onto the color target.
pub struct HdxColorizeSelectionTask {
    id: Path,
    params: HdxColorizeSelectionTaskParams,
    render_tags: TfTokenVector,
    has_selection: bool,
}

impl HdxColorizeSelectionTask {
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxColorizeSelectionTaskParams::default(),
            render_tags: Vec::new(),
            has_selection: false,
        }
    }

    pub fn set_params(&mut self, params: HdxColorizeSelectionTaskParams) {
        self.params = params;
    }

    pub fn get_params(&self) -> &HdxColorizeSelectionTaskParams {
        &self.params
    }

    pub fn has_selection(&self) -> bool {
        self.has_selection
    }

    /// True when id buffer paths are wired up (SetViewportRenderOutput called with color).
    pub fn is_active(&self) -> bool {
        !self.params.prim_id_buffer_path.is_empty()
    }
}

impl HdTask for HdxColorizeSelectionTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        self.has_selection = ctx.contains_key(&Token::new("selectionState"))
            || ctx.contains_key(&Token::new("selectionBuffer"));
        ctx.insert(Token::new("colorizePrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        if !self.params.enable_selection_highlight && !self.params.enable_locate_highlight {
            return;
        }
        if !self.has_selection || !self.is_active() {
            return;
        }
        let request = HdxColorizeSelectionTaskRequest {
            has_selection: self.has_selection,
            is_active: self.is_active(),
            enable_locate_highlight: self.params.enable_locate_highlight,
            enable_outline: self.params.enable_outline,
            selection_color: self.params.selection_color,
            locate_color: self.params.locate_color,
            outline_radius: self.params.outline_radius,
        };
        let requests_token = Token::new("colorizeSelectionTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxColorizeSelectionTaskRequest>>())
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
            order.push(Token::new("colorizeSelection"));
        } else {
            ctx.insert(
                order_token,
                Value::new(vec![Token::new("colorizeSelection")]),
            );
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
    fn test_colorize_tokens() {
        use colorize_tokens::*;
        assert_eq!(PRIMARY_COLOR.as_str(), "primaryColor");
        assert_eq!(SECONDARY_COLOR.as_str(), "secondaryColor");
        assert_eq!(OUTLINE_WIDTH.as_str(), "outlineWidth");
        assert_eq!(GLOW_INTENSITY.as_str(), "glowIntensity");
    }

    #[test]
    fn test_colorize_mode_default() {
        assert_eq!(ColorizeMode::default(), ColorizeMode::Outline);
    }

    #[test]
    fn test_params_default() {
        let p = HdxColorizeSelectionTaskParams::default();
        assert!(p.enable_selection_highlight);
        assert!(p.enable_locate_highlight);
        assert!(p.enable_outline);
        assert_eq!(p.outline_radius, 1);
        assert!(p.prim_id_buffer_path.is_empty());
    }

    #[test]
    fn test_task_creation() {
        let task = HdxColorizeSelectionTask::new(Path::from_string("/colorize").unwrap());
        assert!(!task.has_selection());
        assert!(!task.is_active());
    }

    #[test]
    fn test_task_active_when_id_buffer_set() {
        let mut task = HdxColorizeSelectionTask::new(Path::from_string("/colorize").unwrap());
        let mut params = HdxColorizeSelectionTaskParams::default();
        params.prim_id_buffer_path = Path::from_string("/TC/aov_primId").unwrap();
        task.set_params(params);
        assert!(task.is_active());
    }

    #[test]
    fn test_task_execute_when_active() {
        let mut task = HdxColorizeSelectionTask::new(Path::from_string("/colorize").unwrap());
        let mut params = HdxColorizeSelectionTaskParams::default();
        params.prim_id_buffer_path = Path::from_string("/aov_primId").unwrap();
        task.set_params(params);
        task.has_selection = true;

        let mut ctx = HdTaskContext::new();
        task.execute(&mut ctx);
        let requests = ctx
            .get(&Token::new("colorizeSelectionTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxColorizeSelectionTaskRequest>>())
            .cloned()
            .expect("active task must enqueue colorize selection request");
        assert_eq!(requests.len(), 1);
        assert!(requests[0].has_selection);
        assert!(requests[0].is_active);
        let order = ctx
            .get(&Token::new("postTaskOrder"))
            .and_then(|value| value.get::<Vec<Token>>())
            .cloned()
            .expect("active task must append post task order");
        assert_eq!(order, vec![Token::new("colorizeSelection")]);
    }

    #[test]
    fn test_task_execute_skips_when_highlight_disabled() {
        let mut task = HdxColorizeSelectionTask::new(Path::from_string("/colorize").unwrap());
        let mut params = HdxColorizeSelectionTaskParams::default();
        params.prim_id_buffer_path = Path::from_string("/aov_primId").unwrap();
        params.enable_selection_highlight = false;
        params.enable_locate_highlight = false;
        task.set_params(params);

        let mut ctx = HdTaskContext::new();
        task.execute(&mut ctx);
        assert!(!ctx.contains_key(&Token::new("colorizeSelectionTaskRequests")));
    }
}
