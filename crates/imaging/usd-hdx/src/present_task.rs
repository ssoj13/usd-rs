
//! Present task - Blit final image to screen.
//!
//! Records deferred presentation intent for the application-facing engine.
//!
//! `usd_imaging::gl::Engine` consumes that request after geometry and deferred
//! post-FX replay, then performs the actual frame-finalization work (`Hgi`
//! frame end / app presentation policy).
//! Port of pxr/imaging/hdx/presentTask.h/cpp

use usd_gf::Vec4i;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_hgi::HgiFormat;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Backend execution request emitted by `HdxPresentTask::execute()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdxPresentTaskRequest {
    /// Whether present should composite to the app framebuffer this frame.
    pub enabled: bool,
    /// Destination API used by the application.
    pub dst_api: Token,
    /// Destination framebuffer region.
    pub dst_region: Vec4i,
}

/// Present task parameters.
///
/// Port of HdxPresentTaskParams from pxr/imaging/hdx/presentTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxPresentTaskParams {
    /// The graphics library used by the application/viewer.
    /// (The interopSrc is determined by checking Hgi->GetAPIName)
    pub dst_api: Token,

    /// The framebuffer that the AOVs are presented into.
    /// This is a VtValue encoding a framebuffer in a dst_api specific way.
    /// E.g., a uint32_t (aka GLuint) for framebuffer object for dst_api==OpenGL.
    /// For backwards compatibility, the currently bound framebuffer is used
    /// when the VtValue is empty.
    pub dst_framebuffer: Value,

    /// Subrectangular region of the framebuffer over which to composite AOV
    /// contents. Coordinates are (left, BOTTOM, width, height).
    pub dst_region: Vec4i,

    /// When not enabled, present task does not execute, but still calls
    /// Hgi::EndFrame.
    pub enabled: bool,
}

impl Default for HdxPresentTaskParams {
    fn default() -> Self {
        Self {
            dst_api: Token::new("OpenGL"),
            dst_framebuffer: Value::empty(),
            dst_region: Vec4i::new(0, 0, 0, 0),
            enabled: true,
        }
    }
}

/// Present to screen task.
///
/// A task for taking the final result of the AOVs and compositing it over
/// the currently bound framebuffer. This task uses the 'color' and optionally
/// 'depth' AOV's in the task context. The 'color' AOV is expected to use
/// non-integer (i.e., float or norm) types to keep the interop step simple.
///
/// Port of HdxPresentTask from pxr/imaging/hdx/presentTask.h
pub struct HdxPresentTask {
    /// Task path.
    id: Path,

    /// Present parameters.
    params: HdxPresentTaskParams,
}

impl HdxPresentTask {
    /// Create new present task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxPresentTaskParams::default(),
        }
    }

    /// Set present parameters.
    pub fn set_params(&mut self, params: HdxPresentTaskParams) {
        self.params = params;
    }

    /// Get present parameters.
    pub fn get_params(&self) -> &HdxPresentTaskParams {
        &self.params
    }

    /// Check if the format is supported for presentation.
    ///
    /// This is useful for upstream tasks to prepare the AOV data accordingly,
    /// and keeps the interop step simple.
    pub fn is_format_supported(aov_format: HgiFormat) -> bool {
        // Supported formats for presentation are floating point and normalized types
        matches!(
            aov_format,
            HgiFormat::Float16Vec4
                | HgiFormat::Float32Vec4
                | HgiFormat::UNorm8Vec4
                | HgiFormat::UNorm8Vec4srgb
                | HgiFormat::Float16
                | HgiFormat::Float32
                | HgiFormat::UNorm8
        )
    }
}

impl HdTask for HdxPresentTask {
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
        // Pull params from scene delegate
        // _params = delegate->Get<HdxPresentTaskParams>(id, HdTokens->params);

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // Skip if not enabled
        if !self.params.enabled {
            return;
        }

        // In full Storm implementation:
        // 1. Get color and depth AOV buffers from context
        // 2. Set up HgiInterop for compositing

        ctx.insert(Token::new("presentPrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // The actual presentation / EndFrame point must happen after backend
        // rendering has produced the final AOVs, so we defer this to the engine.
        let request = HdxPresentTaskRequest {
            enabled: self.params.enabled,
            dst_api: self.params.dst_api.clone(),
            dst_region: self.params.dst_region,
        };
        let requests_token = Token::new("presentTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxPresentTaskRequest>>())
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
            order.push(Token::new("present"));
        } else {
            ctx.insert(order_token, Value::new(vec![Token::new("present")]));
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
    fn test_present_task_params_default() {
        let params = HdxPresentTaskParams::default();
        assert_eq!(params.dst_api, Token::new("OpenGL"));
        assert!(params.enabled);
        assert_eq!(params.dst_region, Vec4i::new(0, 0, 0, 0));
    }

    #[test]
    fn test_present_task_params_equality() {
        let params1 = HdxPresentTaskParams::default();
        let params2 = HdxPresentTaskParams::default();
        assert_eq!(params1, params2);

        let mut params3 = HdxPresentTaskParams::default();
        params3.enabled = false;
        assert_ne!(params1, params3);
    }

    #[test]
    fn test_present_task_creation() {
        let task = HdxPresentTask::new(Path::from_string("/present").unwrap());
        assert!(task.get_params().enabled);
    }

    #[test]
    fn test_present_task_set_params() {
        let mut task = HdxPresentTask::new(Path::from_string("/present").unwrap());

        let mut params = HdxPresentTaskParams::default();
        params.dst_framebuffer = Value::from(42u32);
        params.dst_region = Vec4i::new(0, 0, 1920, 1080);

        task.set_params(params.clone());
        assert_eq!(task.get_params(), &params);
    }

    #[test]
    fn test_is_format_supported() {
        assert!(HdxPresentTask::is_format_supported(HgiFormat::Float32Vec4));
        assert!(HdxPresentTask::is_format_supported(HgiFormat::UNorm8Vec4));
        assert!(!HdxPresentTask::is_format_supported(HgiFormat::Int32));
    }
}
