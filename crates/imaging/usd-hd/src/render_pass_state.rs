//! HdRenderPassState - Rendering parameters for render passes.
//!
//! Corresponds to pxr/imaging/hd/renderPassState.h.

use super::aov::HdRenderPassAovBindingVector;
use super::enums::{HdBlendFactor, HdBlendOp, HdCompareFunction, HdCullStyle, HdStencilOp};
use crate::prim::camera::{CameraUtilConformWindowPolicy, HdCamera};
use std::sync::Arc;
use usd_camera_util::ConformWindowPolicy as CamUtilPolicy;
use usd_gf::{Matrix4d, Range2f, Rect2i, Vec2f, Vec2i, Vec3d, Vec4f};
use usd_sdf::Path;

/// Resource registry shared ptr (placeholder for Prepare).
pub type HdResourceRegistrySharedPtr = Arc<dyn std::any::Any>;

/// Clip plane (plane equation coefficients).
pub type ClipPlanesVector = Vec<[f64; 4]>;

/// Color mask for render targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdColorMask {
    /// No color channels written.
    None,
    /// RGB channels only.
    Rgb,
    /// All RGBA channels.
    Rgba,
}

/// Framing placeholder - full CameraUtilFraming in camera_util.
#[derive(Debug, Clone, Default)]
pub struct CameraUtilFramingPlaceholder {
    /// Display window (x, y, width, height).
    pub display_window: (f32, f32, f32, f32),
    /// Data window in pixel coords (x_min, y_min, x_max, y_max).
    pub data_window: (i32, i32, i32, i32),
    /// Pixel aspect ratio (width/height of a pixel).
    pub pixel_aspect_ratio: f32,
}

// CameraUtilConformWindowPolicy: use the canonical definition from hd::prim::camera

/// Render pass state - camera, viewport, AOV bindings, depth/stencil/blend, etc.
///
/// Corresponds to C++ `HdRenderPassState`.
/// Implements the HdRenderPassState trait from render::render_pass.
#[derive(Debug)]
pub struct HdRenderPassStateBase {
    // Camera/framing
    /// Camera pointer (matches C++ `const HdCamera*`). Stored as Arc for shared ownership.
    pub camera: Option<Arc<HdCamera>>,
    /// Scene camera path for render index lookup.
    pub camera_path: Option<Path>,
    /// Viewport rect (x, y, width, height).
    pub viewport: Vec4f,
    /// Camera framing (display/data windows, pixel aspect ratio).
    pub framing: CameraUtilFramingPlaceholder,
    /// Override for camera's window conform policy.
    pub override_window_policy: Option<CameraUtilConformWindowPolicy>,

    // Application state
    /// Color override applied to all prims (alpha=0 disables).
    pub override_color: Vec4f,
    /// Wireframe display color.
    pub wireframe_color: Vec4f,
    /// Point primitive color.
    pub point_color: Vec4f,
    /// Point primitive size in pixels.
    pub point_size: f32,
    /// Whether scene lighting is enabled.
    pub lighting_enabled: bool,
    /// Whether user clip planes are enabled.
    pub clipping_enabled: bool,
    /// Whether exposure compensation is applied to final image.
    pub enable_exposure_compensation: bool,
    /// Selection mask overlay color.
    pub mask_color: Vec4f,
    /// Selection indicator overlay color.
    pub indicator_color: Vec4f,
    /// Point size for selected points.
    pub point_selected_size: f32,

    // Pipeline state
    /// Alpha cutoff threshold for transparency.
    pub alpha_threshold: f32,
    /// Tessellation level for subdivision surfaces.
    pub tess_level: f32,
    /// Drawing range in pixels (near, far).
    pub draw_range: Vec2f,
    /// Whether to use backend default depth bias.
    pub depth_bias_use_default: bool,
    /// Whether depth bias (polygon offset) is enabled.
    pub depth_bias_enabled: bool,
    /// Constant depth bias factor.
    pub depth_bias_constant_factor: f32,
    /// Slope-dependent depth bias factor.
    pub depth_bias_slope_factor: f32,
    /// Depth comparison function.
    pub depth_func: HdCompareFunction,
    /// Whether depth buffer writes are enabled.
    pub depth_mask_enabled: bool,
    /// Whether depth testing is enabled.
    pub depth_test_enabled: bool,
    /// Whether depth clamping is enabled.
    pub depth_clamp_enabled: bool,
    /// Depth range mapping (near, far) in [0,1].
    pub depth_range: Vec2f,
    /// Face culling style.
    pub cull_style: HdCullStyle,

    // Stencil
    /// Stencil comparison function.
    pub stencil_func: HdCompareFunction,
    /// Stencil reference value.
    pub stencil_ref: i32,
    /// Stencil read/write mask.
    pub stencil_mask: i32,
    /// Stencil op when stencil test fails.
    pub stencil_fail_op: HdStencilOp,
    /// Stencil op when stencil passes but depth fails.
    pub stencil_z_fail_op: HdStencilOp,
    /// Stencil op when both stencil and depth pass.
    pub stencil_z_pass_op: HdStencilOp,
    /// Whether stencil testing is enabled.
    pub stencil_enabled: bool,

    // Line/blend
    /// Line width in pixels.
    pub line_width: f32,
    /// Blend equation for color channels.
    pub blend_color_op: HdBlendOp,
    /// Source blend factor for color channels.
    pub blend_color_src_factor: HdBlendFactor,
    /// Destination blend factor for color channels.
    pub blend_color_dst_factor: HdBlendFactor,
    /// Blend equation for alpha channel.
    pub blend_alpha_op: HdBlendOp,
    /// Source blend factor for alpha channel.
    pub blend_alpha_src_factor: HdBlendFactor,
    /// Destination blend factor for alpha channel.
    pub blend_alpha_dst_factor: HdBlendFactor,
    /// Constant blend color used by ConstantColor/ConstantAlpha factors.
    pub blend_constant_color: Vec4f,
    /// Whether alpha blending is enabled.
    pub blend_enabled: bool,
    /// Whether alpha-to-coverage (MSAA transparency) is enabled.
    pub alpha_to_coverage_enabled: bool,
    /// Whether to use backend default color mask.
    pub color_mask_use_default: bool,
    /// Per-target color write masks.
    pub color_masks: Vec<HdColorMask>,

    // AOVs
    /// AOV (render target) bindings for this pass.
    pub aov_bindings: HdRenderPassAovBindingVector,
    /// AOV input bindings (read-back from previous pass).
    pub aov_input_bindings: HdRenderPassAovBindingVector,
    /// Whether to use multi-sample AOV resolve.
    pub use_multi_sample_aov: bool,
    /// Whether conservative rasterization is enabled.
    pub conservative_rasterization_enabled: bool,
    /// Volume ray-marching step size (0 = automatic).
    pub step_size: f32,
    /// Volume step size for lighting (0 = automatic).
    pub step_size_lighting: f32,
    /// Whether MSAA is enabled for rasterization.
    pub multi_sample_enabled: bool,
}

impl Default for HdRenderPassStateBase {
    /// Default values match C++ HdRenderPassState constructor (renderPassState.cpp:20-72).
    fn default() -> Self {
        Self {
            camera: None,
            camera_path: None,
            // C++: _viewport(0, 0, 1, 1)
            viewport: Vec4f::new(0.0, 0.0, 1.0, 1.0),
            framing: CameraUtilFramingPlaceholder::default(),
            override_window_policy: None,
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            // C++: _wireframeColor(0,0,0,0)
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            point_color: Vec4f::new(0.0, 0.0, 0.0, 1.0),
            // C++: _pointSize(3.0)
            point_size: 3.0,
            lighting_enabled: true,
            clipping_enabled: true,
            // C++: _enableExposureCompensation(true)
            enable_exposure_compensation: true,
            // C++: _maskColor(1,0,0,1)
            mask_color: Vec4f::new(1.0, 0.0, 0.0, 1.0),
            // C++: _indicatorColor(0,1,0,1)
            indicator_color: Vec4f::new(0.0, 1.0, 0.0, 1.0),
            point_selected_size: 3.0,
            alpha_threshold: 0.5,
            tess_level: 32.0,
            // C++: _drawRange(0.9, -1.0)
            draw_range: Vec2f::new(0.9, -1.0),
            depth_bias_use_default: true,
            depth_bias_enabled: false,
            depth_bias_constant_factor: 0.0,
            // C++: _depthBiasSlopeFactor(1.0)
            depth_bias_slope_factor: 1.0,
            // C++: _depthFunc(HdCmpFuncLEqual)
            depth_func: HdCompareFunction::LEqual,
            depth_mask_enabled: true,
            depth_test_enabled: true,
            depth_clamp_enabled: false,
            depth_range: Vec2f::new(0.0, 1.0),
            // C++: _cullStyle(HdCullStyleNothing)
            cull_style: HdCullStyle::Nothing,
            stencil_func: HdCompareFunction::Always,
            stencil_ref: 0,
            stencil_mask: !0,
            stencil_fail_op: HdStencilOp::Keep,
            stencil_z_fail_op: HdStencilOp::Keep,
            stencil_z_pass_op: HdStencilOp::Keep,
            stencil_enabled: false,
            line_width: 1.0,
            blend_color_op: HdBlendOp::Add,
            blend_color_src_factor: HdBlendFactor::One,
            blend_color_dst_factor: HdBlendFactor::Zero,
            blend_alpha_op: HdBlendOp::Add,
            blend_alpha_src_factor: HdBlendFactor::One,
            blend_alpha_dst_factor: HdBlendFactor::Zero,
            blend_constant_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            blend_enabled: false,
            alpha_to_coverage_enabled: false,
            color_mask_use_default: true,
            color_masks: Vec::new(),
            aov_bindings: Vec::new(),
            aov_input_bindings: Vec::new(),
            // C++: _useMultiSampleAov(true)
            use_multi_sample_aov: true,
            conservative_rasterization_enabled: false,
            // C++: _stepSize(0.0), _stepSizeLighting(0.0)
            step_size: 0.0,
            step_size_lighting: 0.0,
            multi_sample_enabled: true,
        }
    }
}

impl HdRenderPassStateBase {
    /// Create new default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule to update renderPassState parameters.
    ///
    /// Matches C++ `HdRenderPassState::Prepare(resourceRegistry)` (renderPassState.h:58).
    /// Called once per frame after sync phase, but prior to commit phase.
    /// Override in backend-specific render pass state to upload GPU resources.
    pub fn prepare(
        &mut self,
        _resource_registry: &std::sync::Arc<dyn crate::render::render_delegate::HdResourceRegistry>,
    ) {
        // Base implementation is a no-op. Backend subclasses (e.g. HdStRenderPassState)
        // override this to upload buffers, update uniform blocks, etc.
    }

    //--------------------------------------------------------------------------
    // Camera / Matrix Pipeline (C++ parity: lines 84-206)
    //--------------------------------------------------------------------------

    /// Set camera (matches C++ SetCamera(const HdCamera*)).
    pub fn set_camera(&mut self, camera: Option<Arc<HdCamera>>) {
        self.camera = camera;
    }

    /// Get camera reference.
    pub fn get_camera(&self) -> Option<&Arc<HdCamera>> {
        self.camera.as_ref()
    }

    /// Set camera path.
    pub fn set_camera_path(&mut self, path: Option<Path>) {
        self.camera_path = path;
    }

    /// Set override window policy (C++ SetOverrideWindowPolicy).
    pub fn set_override_window_policy(&mut self, policy: Option<CameraUtilConformWindowPolicy>) {
        self.override_window_policy = policy;
    }

    /// Resolve effective window policy: override > camera > Fit.
    ///
    /// Matches C++ HdRenderPassState::GetWindowPolicy().
    pub fn get_window_policy(&self) -> CameraUtilConformWindowPolicy {
        if let Some(policy) = self.override_window_policy {
            return policy;
        }
        if let Some(ref cam) = self.camera {
            return cam.get_window_policy();
        }
        CameraUtilConformWindowPolicy::Fit
    }

    /// World-to-view matrix: inverse of camera transform.
    ///
    /// Matches C++ HdRenderPassState::GetWorldToViewMatrix().
    pub fn get_world_to_view_matrix(&self) -> Matrix4d {
        match &self.camera {
            Some(cam) => cam
                .get_transform()
                .inverse()
                .unwrap_or_else(Matrix4d::identity),
            None => Matrix4d::identity(),
        }
    }

    /// Projection matrix from camera + framing/viewport + window policy.
    ///
    /// Matches C++ HdRenderPassState::GetProjectionMatrix().
    pub fn get_projection_matrix(&self) -> Matrix4d {
        let cam = match &self.camera {
            Some(c) => c,
            None => return Matrix4d::identity(),
        };

        // Compute raw projection from camera
        let proj = cam.compute_projection_matrix();

        // If framing is valid, delegate to CameraUtilFraming.ApplyToProjectionMatrix
        // (usd-camera-util), which handles display/data window offset + pixel aspect.
        // C++ reference: HdRenderPassState::GetProjectionMatrix, renderPassState.cpp.
        if self.framing.pixel_aspect_ratio > 0.0 {
            let dw = &self.framing.data_window;
            if dw.2 > dw.0 && dw.3 > dw.1 {
                // Convert CameraUtilFramingPlaceholder -> usd_camera_util::Framing.
                let dw_field = &self.framing.display_window;
                let display = Range2f::new(
                    usd_gf::Vec2f::new(dw_field.0, dw_field.1),
                    usd_gf::Vec2f::new(dw_field.2, dw_field.3),
                );
                let data = Rect2i::new(Vec2i::new(dw.0, dw.1), Vec2i::new(dw.2, dw.3));
                let framing =
                    usd_camera_util::Framing::new(display, data, self.framing.pixel_aspect_ratio);
                // Convert window policy enum (both enums are isomorphic).
                let policy = hd_policy_to_cam_util(self.get_window_policy());
                return framing.apply_to_projection_matrix(proj, policy);
            }
        }

        // Fallback: viewport-based aspect ratio conforming
        let aspect = if self.viewport.w != 0.0 {
            (self.viewport.z / self.viewport.w) as f64
        } else {
            1.0
        };
        conform_projection(&proj, self.get_window_policy(), aspect)
    }

    /// Full image-to-world matrix (viewport pixel -> world).
    ///
    /// Matches C++ HdRenderPassState::GetImageToWorldMatrix().
    /// Computes: inverse(worldToView * projection * viewportTransform).
    pub fn get_image_to_world_matrix(&self) -> Matrix4d {
        let vp_x = self.viewport.x as f64;
        let vp_y = self.viewport.y as f64;
        let vp_w = self.viewport.z as f64;
        let vp_h = self.viewport.w as f64;

        // NDC [-1,+1] -> viewport transform
        let viewport_scale = Vec3d::new(vp_w / 2.0, vp_h / 2.0, 0.5);
        let viewport_translate = Vec3d::new(vp_x + vp_w / 2.0, vp_y + vp_h / 2.0, 0.5);

        let viewport_xform = Matrix4d::from_scale_vec(&viewport_scale)
            * Matrix4d::from_translation(viewport_translate);

        let world_to_image =
            self.get_world_to_view_matrix() * self.get_projection_matrix() * viewport_xform;

        world_to_image.inverse().unwrap_or_else(Matrix4d::identity)
    }

    /// Get clip planes from camera (returns empty if clipping disabled or no camera).
    ///
    /// Matches C++ HdRenderPassState::GetClipPlanes().
    pub fn get_clip_planes(&self) -> ClipPlanesVector {
        if !self.clipping_enabled {
            return Vec::new();
        }
        match &self.camera {
            Some(cam) => cam
                .get_clip_planes()
                .iter()
                .map(|p| [p.x, p.y, p.z, p.w])
                .collect(),
            None => Vec::new(),
        }
    }

    //--------------------------------------------------------------------------
    // Setters missing from audit (P3 but easy)
    //--------------------------------------------------------------------------

    /// Set point color.
    pub fn set_point_color(&mut self, c: Vec4f) {
        self.point_color = c;
    }
    /// Get point color.
    pub fn get_point_color(&self) -> Vec4f {
        self.point_color
    }

    /// Set mask color.
    pub fn set_mask_color(&mut self, c: Vec4f) {
        self.mask_color = c;
    }
    /// Get mask color.
    pub fn get_mask_color(&self) -> Vec4f {
        self.mask_color
    }

    /// Set indicator color.
    pub fn set_indicator_color(&mut self, c: Vec4f) {
        self.indicator_color = c;
    }
    /// Get indicator color.
    pub fn get_indicator_color(&self) -> Vec4f {
        self.indicator_color
    }

    /// Get drawing range in pixels (matches C++ GetDrawingRange).
    pub fn get_drawing_range(&self) -> Vec2f {
        self.draw_range
    }

    /// Set drawing range.
    pub fn set_drawing_range(&mut self, range: Vec2f) {
        self.draw_range = range;
    }

    /// Get drawing range in NDC (normalized device coordinates).
    ///
    /// Matches C++ `HdRenderPassState::GetDrawingRangeNDC` (renderPassState.cpp:502-516).
    /// Converts pixel-space draw range to NDC by dividing by viewport/framing dimensions.
    pub fn get_drawing_range_ndc(&self) -> Vec2f {
        let (width, height) = if self.framing.pixel_aspect_ratio > 0.0 {
            // Valid framing: use data window dimensions
            let dw = &self.framing.data_window;
            ((dw.2 - dw.0) as f32, (dw.3 - dw.1) as f32)
        } else {
            // Fall back to viewport
            (self.viewport.z, self.viewport.w)
        };

        if width == 0.0 || height == 0.0 {
            return Vec2f::new(0.0, 0.0);
        }

        Vec2f::new(
            2.0 * self.draw_range.x / width,
            2.0 * self.draw_range.y / height,
        )
    }

    /// Set depth bias individually (C++ SetDepthBias).
    pub fn set_depth_bias(&mut self, constant_factor: f32, slope_factor: f32) {
        self.depth_bias_constant_factor = constant_factor;
        self.depth_bias_slope_factor = slope_factor;
    }

    /// Set exposure compensation enabled.
    pub fn set_enable_exposure_compensation(&mut self, e: bool) {
        self.enable_exposure_compensation = e;
    }
    /// Get exposure compensation enabled.
    pub fn get_enable_exposure_compensation(&self) -> bool {
        self.enable_exposure_compensation
    }

    /// Set viewport (x, y, w, h).
    pub fn set_viewport(&mut self, vp: Vec4f) {
        self.viewport = vp;
    }

    /// Get viewport.
    pub fn get_viewport(&self) -> Vec4f {
        self.viewport
    }

    /// Set override color.
    pub fn set_override_color(&mut self, c: Vec4f) {
        self.override_color = c;
    }
    /// Get override color.
    pub fn get_override_color(&self) -> Vec4f {
        self.override_color
    }

    /// Set wireframe color.
    pub fn set_wireframe_color(&mut self, c: Vec4f) {
        self.wireframe_color = c;
    }
    /// Get wireframe color.
    pub fn get_wireframe_color(&self) -> Vec4f {
        self.wireframe_color
    }

    /// Set point size.
    pub fn set_point_size(&mut self, s: f32) {
        self.point_size = s;
    }
    /// Get point size.
    pub fn get_point_size(&self) -> f32 {
        self.point_size
    }

    /// Set lighting enabled.
    pub fn set_lighting_enabled(&mut self, e: bool) {
        self.lighting_enabled = e;
    }
    /// Get lighting enabled.
    pub fn get_lighting_enabled(&self) -> bool {
        self.lighting_enabled
    }

    /// Set AOV bindings.
    pub fn set_aov_bindings(&mut self, b: HdRenderPassAovBindingVector) {
        self.aov_bindings = b;
    }
    /// Get AOV bindings.
    pub fn get_aov_bindings(&self) -> &HdRenderPassAovBindingVector {
        &self.aov_bindings
    }

    /// Set AOV input bindings.
    pub fn set_aov_input_bindings(&mut self, b: HdRenderPassAovBindingVector) {
        self.aov_input_bindings = b;
    }
    /// Get AOV input bindings.
    pub fn get_aov_input_bindings(&self) -> &HdRenderPassAovBindingVector {
        &self.aov_input_bindings
    }

    /// Set use AOV multi-sample.
    pub fn set_use_aov_multi_sample(&mut self, v: bool) {
        self.use_multi_sample_aov = v;
    }
    /// Get use AOV multi-sample.
    pub fn get_use_aov_multi_sample(&self) -> bool {
        self.use_multi_sample_aov
    }

    /// Set cull style.
    pub fn set_cull_style(&mut self, s: HdCullStyle) {
        self.cull_style = s;
    }
    /// Get cull style.
    pub fn get_cull_style(&self) -> HdCullStyle {
        self.cull_style
    }

    /// Set alpha threshold.
    pub fn set_alpha_threshold(&mut self, t: f32) {
        self.alpha_threshold = t;
    }
    /// Get alpha threshold.
    pub fn get_alpha_threshold(&self) -> f32 {
        self.alpha_threshold
    }

    /// Set depth func.
    pub fn set_depth_func(&mut self, f: HdCompareFunction) {
        self.depth_func = f;
    }
    /// Get depth comparison function.
    pub fn get_depth_func(&self) -> HdCompareFunction {
        self.depth_func
    }

    /// Set stencil.
    pub fn set_stencil(
        &mut self,
        func: HdCompareFunction,
        ref_: i32,
        mask: i32,
        fail: HdStencilOp,
        zfail: HdStencilOp,
        zpass: HdStencilOp,
    ) {
        self.stencil_func = func;
        self.stencil_ref = ref_;
        self.stencil_mask = mask;
        self.stencil_fail_op = fail;
        self.stencil_z_fail_op = zfail;
        self.stencil_z_pass_op = zpass;
    }
    /// Get stencil comparison function.
    pub fn get_stencil_func(&self) -> HdCompareFunction {
        self.stencil_func
    }
    /// Get stencil reference value.
    pub fn get_stencil_ref(&self) -> i32 {
        self.stencil_ref
    }
    /// Get stencil mask.
    pub fn get_stencil_mask(&self) -> i32 {
        self.stencil_mask
    }
    /// Get stencil-fail operation.
    pub fn get_stencil_fail_op(&self) -> HdStencilOp {
        self.stencil_fail_op
    }
    /// Get depth-fail stencil operation.
    pub fn get_stencil_depth_fail_op(&self) -> HdStencilOp {
        self.stencil_z_fail_op
    }
    /// Get depth-pass stencil operation.
    pub fn get_stencil_depth_pass_op(&self) -> HdStencilOp {
        self.stencil_z_pass_op
    }
    /// Set stencil enabled.
    pub fn set_stencil_enabled(&mut self, e: bool) {
        self.stencil_enabled = e;
    }
    /// Get stencil enabled.
    pub fn get_stencil_enabled(&self) -> bool {
        self.stencil_enabled
    }

    /// Set blend.
    pub fn set_blend(
        &mut self,
        color_op: HdBlendOp,
        color_src: HdBlendFactor,
        color_dst: HdBlendFactor,
        alpha_op: HdBlendOp,
        alpha_src: HdBlendFactor,
        alpha_dst: HdBlendFactor,
    ) {
        self.blend_color_op = color_op;
        self.blend_color_src_factor = color_src;
        self.blend_color_dst_factor = color_dst;
        self.blend_alpha_op = alpha_op;
        self.blend_alpha_src_factor = alpha_src;
        self.blend_alpha_dst_factor = alpha_dst;
    }
    /// Get blend equation for color channels.
    pub fn get_blend_color_op(&self) -> HdBlendOp {
        self.blend_color_op
    }
    /// Get source blend factor for color channels.
    pub fn get_blend_color_src_factor(&self) -> HdBlendFactor {
        self.blend_color_src_factor
    }
    /// Get destination blend factor for color channels.
    pub fn get_blend_color_dst_factor(&self) -> HdBlendFactor {
        self.blend_color_dst_factor
    }
}

/// Simplified CameraUtilConformedWindow for projection matrices.
///
/// Adjusts a projection matrix to conform to the given window policy and aspect ratio.
/// Matches the behavior of CameraUtilConformedWindow(matrix, policy, aspect).
fn conform_projection(
    proj: &Matrix4d,
    policy: CameraUtilConformWindowPolicy,
    target_aspect: f64,
) -> Matrix4d {
    if target_aspect <= 0.0 {
        return *proj;
    }
    // Extract current aspect from the projection matrix
    // For a symmetric perspective: proj[0][0] = 2n/(r-l), proj[1][1] = 2n/(t-b)
    // aspect_proj = proj[1][1] / proj[0][0]
    let sx = proj[0][0];
    let sy = proj[1][1];
    if sx.abs() < 1e-12 || sy.abs() < 1e-12 {
        return *proj;
    }
    let proj_aspect = sx / sy; // width/height ratio of the frustum

    let ratio = proj_aspect / target_aspect;
    if (ratio - 1.0).abs() < 1e-12 {
        return *proj;
    }

    let mut result = *proj;
    match policy {
        CameraUtilConformWindowPolicy::MatchVertically => {
            // Scale X to match target aspect
            result[0][0] /= ratio;
        }
        CameraUtilConformWindowPolicy::MatchHorizontally => {
            // Scale Y to match target aspect
            result[1][1] *= ratio;
        }
        CameraUtilConformWindowPolicy::Fit => {
            if ratio > 1.0 {
                result[1][1] *= ratio;
            } else {
                result[0][0] /= ratio;
            }
        }
        CameraUtilConformWindowPolicy::Crop => {
            if ratio > 1.0 {
                result[0][0] /= ratio;
            } else {
                result[1][1] *= ratio;
            }
        }
        CameraUtilConformWindowPolicy::DontConform => {
            // No adjustment
        }
    }
    result
}

/// Convert HdCamera's window policy enum to usd_camera_util's isomorphic enum.
///
/// Both enums mirror C++ CameraUtilConformWindowPolicy; they are kept separate
/// to avoid a circular dependency between usd-hd and usd-camera-util.
fn hd_policy_to_cam_util(p: CameraUtilConformWindowPolicy) -> CamUtilPolicy {
    match p {
        CameraUtilConformWindowPolicy::MatchVertically => CamUtilPolicy::MatchVertically,
        CameraUtilConformWindowPolicy::MatchHorizontally => CamUtilPolicy::MatchHorizontally,
        CameraUtilConformWindowPolicy::Fit => CamUtilPolicy::Fit,
        CameraUtilConformWindowPolicy::Crop => CamUtilPolicy::Crop,
        CameraUtilConformWindowPolicy::DontConform => CamUtilPolicy::DontConform,
    }
}

impl crate::render::render_pass::HdRenderPassState for HdRenderPassStateBase {
    fn get_camera(&self) -> Option<&Path> {
        self.camera_path.as_ref()
    }

    fn get_viewport(&self) -> (f32, f32, f32, f32) {
        (
            self.viewport.x,
            self.viewport.y,
            self.viewport.z,
            self.viewport.w,
        )
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn prepare(
        &mut self,
        _resource_registry: &std::sync::Arc<dyn crate::render::render_delegate::HdResourceRegistry>,
    ) {
        // Base implementation: no-op (same as the struct method).
        // Avoids infinite recursion by not calling self.prepare().
    }
}
