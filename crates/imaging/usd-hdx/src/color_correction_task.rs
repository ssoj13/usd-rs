
//! Color correction task - Apply color grading.
//!
//! Records deferred color-correction intent for the rendered AOV.
//!
//! The actual correction is replayed later by `usd_imaging::gl::Engine` after
//! backend rendering has produced the target AOV texture. Both `sRGB` and
//! `OpenColorIO` are executed on the engine-side post-FX path.
//! Port of pxr/imaging/hdx/colorCorrectionTask.h/cpp

use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Backend execution request emitted by `HdxColorCorrectionTask::execute()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdxColorCorrectionTaskRequest {
    /// The AOV that should be color-corrected.
    pub aov_name: Token,
    /// Color correction mode (`disabled`, `sRGB`, `openColorIO`).
    pub color_correction_mode: Token,
    /// OCIO display name.
    pub display_ocio: String,
    /// OCIO view name.
    pub view_ocio: String,
    /// OCIO source colorspace name.
    pub colorspace_ocio: String,
    /// OCIO looks string.
    pub looks_ocio: String,
    /// Requested LUT size from the HDX task params.
    pub lut3d_size_ocio: i32,
}

/// Standard color correction mode tokens.
pub mod color_correction_tokens {
    use usd_tf::Token;

    /// Disabled color correction.
    pub fn disabled() -> Token {
        Token::new("disabled")
    }

    /// sRGB gamma correction.
    pub fn srgb() -> Token {
        Token::new("sRGB")
    }

    /// OpenColorIO-based color correction.
    pub fn opencolorio() -> Token {
        Token::new("openColorIO")
    }
}

/// Color correction task parameters.
///
/// Port of HdxColorCorrectionTaskParams from pxr/imaging/hdx/colorCorrectionTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxColorCorrectionTaskParams {
    /// Color correction mode token.
    /// Use `color_correction_tokens::*` for standard values.
    pub color_correction_mode: Token,

    /// OCIO display name.
    pub display_ocio: String,

    /// OCIO view name.
    pub view_ocio: String,

    /// OCIO colorspace name.
    pub colorspace_ocio: String,

    /// OCIO looks (comma-separated).
    pub looks_ocio: String,

    /// 3D LUT size for OCIO (GPU lookup table dimensions).
    /// Default is 65.
    pub lut3d_size_ocio: i32,

    /// AOV buffer name to apply correction to.
    pub aov_name: Token,
}

impl Default for HdxColorCorrectionTaskParams {
    fn default() -> Self {
        Self {
            color_correction_mode: color_correction_tokens::disabled(),
            display_ocio: String::new(),
            view_ocio: String::new(),
            colorspace_ocio: String::new(),
            looks_ocio: String::new(),
            lut3d_size_ocio: 65,
            aov_name: Token::new("color"),
        }
    }
}

/// Color correction task.
///
/// A task for applying color correction to rendered images.
/// Supports sRGB gamma correction and OpenColorIO-based transformations.
///
/// Port of HdxColorCorrectionTask from pxr/imaging/hdx/colorCorrectionTask.h
pub struct HdxColorCorrectionTask {
    /// Task path.
    id: Path,

    /// Color correction parameters.
    params: HdxColorCorrectionTaskParams,

    /// Screen size for fullscreen pass.
    #[allow(dead_code)]
    screen_size: [f32; 2],

    /// Whether OCIO resources need updating.
    #[allow(dead_code)]
    ocio_dirty: bool,
}

impl HdxColorCorrectionTask {
    /// Create new color correction task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxColorCorrectionTaskParams::default(),
            screen_size: [0.0, 0.0],
            ocio_dirty: true,
        }
    }

    /// Set color correction parameters.
    pub fn set_params(&mut self, params: HdxColorCorrectionTaskParams) {
        // Mark OCIO as dirty if relevant params changed
        if self.params.color_correction_mode != params.color_correction_mode
            || self.params.display_ocio != params.display_ocio
            || self.params.view_ocio != params.view_ocio
            || self.params.colorspace_ocio != params.colorspace_ocio
            || self.params.looks_ocio != params.looks_ocio
            || self.params.lut3d_size_ocio != params.lut3d_size_ocio
        {
            self.ocio_dirty = true;
        }
        self.params = params;
    }

    /// Get color correction parameters.
    pub fn get_params(&self) -> &HdxColorCorrectionTaskParams {
        &self.params
    }

    /// Check if color correction is enabled.
    pub fn is_enabled(&self) -> bool {
        self.params.color_correction_mode != color_correction_tokens::disabled()
    }

    /// Check if using OCIO mode.
    pub fn is_ocio_mode(&self) -> bool {
        self.params.color_correction_mode == color_correction_tokens::opencolorio()
    }
}

impl HdTask for HdxColorCorrectionTask {
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
        // Pull params from scene delegate:
        // _params = delegate->Get<HdxColorCorrectionTaskParams>(id, HdTokens->params);

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // Skip if color correction is disabled
        if !self.is_enabled() {
            return;
        }

        // In full Storm implementation:
        // 1. Get viewport size from context
        // 2. If OCIO mode and dirty, rebuild LUT and shader
        // 3. Create/update fullscreen triangle resources
        // 4. Create graphics pipeline for color correction pass

        // Store color correction state in context
        ctx.insert(Token::new("colorCorrectionEnabled"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: Only color-correct the color AOV.
        // if (_params.aovName != HdAovTokens->color) return;
        if self.params.aov_name != Token::new("color") {
            return;
        }

        // Real source/intermediate textures are injected after backend draw,
        // so execution is deferred through an engine-side post-processing bridge.
        let request = HdxColorCorrectionTaskRequest {
            aov_name: self.params.aov_name.clone(),
            color_correction_mode: self.params.color_correction_mode.clone(),
            display_ocio: self.params.display_ocio.clone(),
            view_ocio: self.params.view_ocio.clone(),
            colorspace_ocio: self.params.colorspace_ocio.clone(),
            looks_ocio: self.params.looks_ocio.clone(),
            lut3d_size_ocio: self.params.lut3d_size_ocio,
        };
        let requests_token = Token::new("colorCorrectionTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxColorCorrectionTaskRequest>>())
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
            order.push(Token::new("colorCorrection"));
        } else {
            ctx.insert(order_token, Value::new(vec![Token::new("colorCorrection")]));
        }
    }

    fn get_render_tags(&self) -> &[Token] {
        &[]
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
    fn test_color_correction_tokens() {
        assert_ne!(
            color_correction_tokens::disabled(),
            color_correction_tokens::srgb()
        );
        assert_ne!(
            color_correction_tokens::srgb(),
            color_correction_tokens::opencolorio()
        );
    }

    #[test]
    fn test_color_correction_params_default() {
        let params = HdxColorCorrectionTaskParams::default();
        assert_eq!(
            params.color_correction_mode,
            color_correction_tokens::disabled()
        );
        assert_eq!(params.lut3d_size_ocio, 65);
        assert!(params.display_ocio.is_empty());
        assert!(params.view_ocio.is_empty());
    }

    #[test]
    fn test_color_correction_params_equality() {
        let params1 = HdxColorCorrectionTaskParams::default();
        let params2 = HdxColorCorrectionTaskParams::default();
        assert_eq!(params1, params2);

        let mut params3 = HdxColorCorrectionTaskParams::default();
        params3.color_correction_mode = color_correction_tokens::srgb();
        assert_ne!(params1, params3);
    }

    #[test]
    fn test_color_correction_task_creation() {
        let task = HdxColorCorrectionTask::new(Path::from_string("/colorCorrect").unwrap());
        assert!(!task.is_enabled());
        assert!(!task.is_ocio_mode());
    }

    #[test]
    fn test_color_correction_task_srgb() {
        let mut task = HdxColorCorrectionTask::new(Path::from_string("/colorCorrect").unwrap());

        let mut params = HdxColorCorrectionTaskParams::default();
        params.color_correction_mode = color_correction_tokens::srgb();

        task.set_params(params.clone());
        assert!(task.is_enabled());
        assert!(!task.is_ocio_mode());
    }

    #[test]
    fn test_color_correction_task_ocio() {
        let mut task = HdxColorCorrectionTask::new(Path::from_string("/colorCorrect").unwrap());

        let mut params = HdxColorCorrectionTaskParams::default();
        params.color_correction_mode = color_correction_tokens::opencolorio();
        params.display_ocio = "sRGB".to_string();
        params.view_ocio = "ACES 1.0 SDR-video".to_string();

        task.set_params(params.clone());
        assert!(task.is_enabled());
        assert!(task.is_ocio_mode());
        assert_eq!(task.get_params(), &params);
    }
}
