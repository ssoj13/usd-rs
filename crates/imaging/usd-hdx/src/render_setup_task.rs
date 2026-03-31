
//! Render setup task - Configure render pass state.
//!
//! Sets up camera, viewport, and rendering parameters for render tasks.
//! Port of pxr/imaging/hdx/renderSetupTask.h/cpp

use std::sync::Arc;
use usd_gf::{Matrix4d, Vec2f, Vec4d, Vec4f};
use usd_hd::enums::{HdBlendFactor, HdBlendOp, HdCompareFunction, HdCullStyle, HdStencilOp};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// AOV (Arbitrary Output Variable) binding for render targets.
#[derive(Debug, Clone, PartialEq)]
pub struct HdRenderPassAovBinding {
    /// AOV name (e.g., "color", "depth", "primId")
    pub aov_name: Token,
    /// Path to the render buffer prim
    pub render_buffer_id: Path,
    /// Clear value for this AOV (empty if no clear)
    pub clear_value: Value,
}

impl HdRenderPassAovBinding {
    /// Create a new AOV binding.
    pub fn new(aov_name: Token, render_buffer_id: Path) -> Self {
        Self {
            aov_name,
            render_buffer_id,
            clear_value: Value::default(),
        }
    }

    /// Create binding with clear value.
    pub fn with_clear_value(mut self, clear_value: Value) -> Self {
        self.clear_value = clear_value;
        self
    }
}

/// Vector of AOV bindings.
pub type HdRenderPassAovBindingVector = Vec<HdRenderPassAovBinding>;

/// Camera framing parameters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraUtilFraming {
    /// Display window (in NDC)
    pub display_window: (f64, f64, f64, f64),
    /// Data window (pixel coords)
    pub data_window: (i32, i32, i32, i32),
    /// Pixel aspect ratio
    pub pixel_aspect_ratio: f64,
}

impl CameraUtilFraming {
    /// Create new framing with default values.
    pub fn new() -> Self {
        Self {
            display_window: (0.0, 0.0, 1.0, 1.0),
            data_window: (0, 0, 0, 0),
            pixel_aspect_ratio: 1.0,
        }
    }

    /// Check if framing is valid.
    ///
    /// Both data window dimensions must be positive (C++ checks width>0 && height>0).
    pub fn is_valid(&self) -> bool {
        self.pixel_aspect_ratio > 0.0
            && self.data_window.2 > self.data_window.0
            && self.data_window.3 > self.data_window.1
    }
}

/// Window conform policy for camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraUtilConformWindowPolicy {
    /// Match vertically
    MatchVertically,
    /// Match horizontally
    MatchHorizontally,
    /// Fit inside
    Fit,
    /// Crop to fill
    CropToFill,
    /// Don't conform
    DontConform,
}

/// Render task parameters.
///
/// Complete set of parameters for configuring render pass state.
/// Port of HdxRenderTaskParams from pxr/imaging/hdx/renderSetupTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxRenderTaskParams {
    // ========================================================================
    // Global rendering parameters
    // ========================================================================
    /// Override color for debugging
    pub override_color: Vec4f,
    /// Wireframe color
    pub wireframe_color: Vec4f,
    /// Point color
    pub point_color: Vec4f,
    /// Point size in pixels
    pub point_size: f32,
    /// Enable lighting
    pub enable_lighting: bool,
    /// Alpha threshold for transparency
    pub alpha_threshold: f32,
    /// Enable scene lights
    pub enable_scene_lights: bool,
    /// Enable clipping planes
    pub enable_clipping: bool,

    // ========================================================================
    // Selection/Masking parameters
    // ========================================================================
    /// Mask color for selection
    pub mask_color: Vec4f,
    /// Indicator color for selection
    pub indicator_color: Vec4f,
    /// Selected point size
    pub point_selected_size: f32,

    // ========================================================================
    // AOV bindings
    // ========================================================================
    /// AOVs to render to
    pub aov_bindings: HdRenderPassAovBindingVector,
    /// AOV input bindings
    pub aov_input_bindings: HdRenderPassAovBindingVector,

    // ========================================================================
    // Depth bias settings
    // ========================================================================
    /// Use default depth bias from GL state
    pub depth_bias_use_default: bool,
    /// Enable depth bias
    pub depth_bias_enable: bool,
    /// Depth bias constant factor
    pub depth_bias_constant_factor: f32,
    /// Depth bias slope factor
    pub depth_bias_slope_factor: f32,

    // ========================================================================
    // Depth test settings
    // ========================================================================
    /// Depth comparison function
    pub depth_func: HdCompareFunction,
    /// Enable depth mask (write)
    pub depth_mask_enable: bool,

    // ========================================================================
    // Stencil settings
    // ========================================================================
    /// Stencil comparison function
    pub stencil_func: HdCompareFunction,
    /// Stencil reference value
    pub stencil_ref: i32,
    /// Stencil mask
    pub stencil_mask: i32,
    /// Stencil fail operation
    pub stencil_fail_op: HdStencilOp,
    /// Stencil z-fail operation
    pub stencil_z_fail_op: HdStencilOp,
    /// Stencil z-pass operation
    pub stencil_z_pass_op: HdStencilOp,
    /// Enable stencil test
    pub stencil_enable: bool,

    // ========================================================================
    // Blending settings
    // ========================================================================
    /// Blend operation for color
    pub blend_color_op: HdBlendOp,
    /// Blend source factor for color
    pub blend_color_src_factor: HdBlendFactor,
    /// Blend destination factor for color
    pub blend_color_dst_factor: HdBlendFactor,
    /// Blend operation for alpha
    pub blend_alpha_op: HdBlendOp,
    /// Blend source factor for alpha
    pub blend_alpha_src_factor: HdBlendFactor,
    /// Blend destination factor for alpha
    pub blend_alpha_dst_factor: HdBlendFactor,
    /// Blend constant color
    pub blend_constant_color: Vec4f,
    /// Enable blending
    pub blend_enable: bool,

    // ========================================================================
    // Multisampling settings
    // ========================================================================
    /// Enable alpha to coverage
    pub enable_alpha_to_coverage: bool,
    /// Use AOV multisampling
    pub use_aov_multi_sample: bool,
    /// Resolve AOV multisampling
    pub resolve_aov_multi_sample: bool,

    // ========================================================================
    // Camera and viewport
    // ========================================================================
    /// Camera path
    pub camera: Path,
    /// Camera framing
    pub framing: CameraUtilFraming,
    /// Viewport (x, y, width, height) - only used if framing is invalid
    pub viewport: Vec4d,
    /// Cull style
    pub cull_style: HdCullStyle,
    /// Override window policy
    pub override_window_policy: Option<CameraUtilConformWindowPolicy>,
}

impl Default for HdxRenderTaskParams {
    fn default() -> Self {
        Self {
            // Global params
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            point_color: Vec4f::new(0.0, 0.0, 0.0, 1.0),
            point_size: 3.0,
            enable_lighting: false,
            alpha_threshold: 0.0,
            enable_scene_lights: true,
            enable_clipping: true,

            // Selection/masking
            mask_color: Vec4f::new(1.0, 0.0, 0.0, 1.0),
            indicator_color: Vec4f::new(0.0, 1.0, 0.0, 1.0),
            point_selected_size: 3.0,

            // AOVs
            aov_bindings: Vec::new(),
            aov_input_bindings: Vec::new(),

            // Depth bias
            depth_bias_use_default: true,
            depth_bias_enable: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 1.0,

            // Depth test
            depth_func: HdCompareFunction::LEqual,
            depth_mask_enable: true,

            // Stencil
            stencil_func: HdCompareFunction::Always,
            stencil_ref: 0,
            stencil_mask: !0,
            stencil_fail_op: HdStencilOp::Keep,
            stencil_z_fail_op: HdStencilOp::Keep,
            stencil_z_pass_op: HdStencilOp::Keep,
            stencil_enable: false,

            // Blending
            blend_color_op: HdBlendOp::Add,
            blend_color_src_factor: HdBlendFactor::One,
            blend_color_dst_factor: HdBlendFactor::Zero,
            blend_alpha_op: HdBlendOp::Add,
            blend_alpha_src_factor: HdBlendFactor::One,
            blend_alpha_dst_factor: HdBlendFactor::Zero,
            blend_constant_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            blend_enable: false,

            // Multisampling
            enable_alpha_to_coverage: true,
            use_aov_multi_sample: true,
            resolve_aov_multi_sample: true,

            // Camera
            camera: Path::empty(),
            framing: CameraUtilFraming::default(),
            viewport: Vec4d::new(0.0, 0.0, 0.0, 0.0),
            cull_style: HdCullStyle::BackUnlessDoubleSided,
            override_window_policy: None,
        }
    }
}

/// Render pass state for Storm renderer.
///
/// Contains all state needed for a render pass.
#[derive(Clone)]
pub struct HdxRenderPassState {
    /// Camera path
    camera_id: Path,
    /// View matrix (for GPU picking and shadow passes)
    pub view_matrix: Option<Matrix4d>,
    /// Projection matrix (for GPU picking and shadow passes)
    pub proj_matrix: Option<Matrix4d>,
    /// Depth clamp enabled (shadow pass: prevent far-clip of shadow casters)
    pub depth_clamp_enabled: bool,
    /// Depth range [near, far] for shadow depth buffer
    pub depth_range: Vec2f,
    /// Framing
    framing: CameraUtilFraming,
    /// Override window policy
    override_window_policy: Option<CameraUtilConformWindowPolicy>,
    /// Viewport
    viewport: Vec4d,
    /// AOV bindings
    aov_bindings: HdRenderPassAovBindingVector,
    /// AOV input bindings
    aov_input_bindings: HdRenderPassAovBindingVector,
    /// Override color
    override_color: Vec4f,
    /// Wireframe color
    wireframe_color: Vec4f,
    /// Point color
    point_color: Vec4f,
    /// Point size
    point_size: f32,
    /// Lighting enabled
    lighting_enabled: bool,
    /// Clipping enabled
    clipping_enabled: bool,
    /// Alpha threshold
    alpha_threshold: f32,
    /// Cull style
    cull_style: HdCullStyle,
    /// Mask color
    mask_color: Vec4f,
    /// Indicator color
    indicator_color: Vec4f,
    /// Point selected size
    point_selected_size: f32,
    /// Depth bias use default
    depth_bias_use_default: bool,
    /// Depth bias enabled
    depth_bias_enabled: bool,
    /// Depth bias constant factor
    depth_bias_constant_factor: f32,
    /// Depth bias slope factor
    depth_bias_slope_factor: f32,
    /// Depth function
    depth_func: HdCompareFunction,
    /// Depth mask enabled
    depth_mask_enabled: bool,
    /// Stencil enabled
    stencil_enabled: bool,
    /// Stencil function
    stencil_func: HdCompareFunction,
    /// Stencil ref
    stencil_ref: i32,
    /// Stencil mask
    stencil_mask: i32,
    /// Stencil fail op
    stencil_fail_op: HdStencilOp,
    /// Stencil z-fail op
    stencil_z_fail_op: HdStencilOp,
    /// Stencil z-pass op
    stencil_z_pass_op: HdStencilOp,
    /// Blend enabled
    blend_enabled: bool,
    /// Blend color op
    blend_color_op: HdBlendOp,
    /// Blend color src factor
    blend_color_src_factor: HdBlendFactor,
    /// Blend color dst factor
    blend_color_dst_factor: HdBlendFactor,
    /// Blend alpha op
    blend_alpha_op: HdBlendOp,
    /// Blend alpha src factor
    blend_alpha_src_factor: HdBlendFactor,
    /// Blend alpha dst factor
    blend_alpha_dst_factor: HdBlendFactor,
    /// Blend constant color
    blend_constant_color: Vec4f,
    /// Alpha to coverage enabled
    alpha_to_coverage_enabled: bool,
    /// Use AOV multisample
    use_aov_multi_sample: bool,
    /// Resolve AOV multisample
    resolve_aov_multi_sample: bool,
}

impl Default for HdxRenderPassState {
    fn default() -> Self {
        Self::new()
    }
}

impl HdxRenderPassState {
    /// Create new render pass state.
    pub fn new() -> Self {
        Self {
            camera_id: Path::empty(),
            view_matrix: None,
            proj_matrix: None,
            depth_clamp_enabled: false,
            depth_range: Vec2f::new(0.0, 1.0),
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
            viewport: Vec4d::new(0.0, 0.0, 1920.0, 1080.0),
            aov_bindings: Vec::new(),
            aov_input_bindings: Vec::new(),
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            point_color: Vec4f::new(0.0, 0.0, 0.0, 1.0),
            point_size: 3.0,
            lighting_enabled: false,
            clipping_enabled: true,
            alpha_threshold: 0.0,
            cull_style: HdCullStyle::BackUnlessDoubleSided,
            mask_color: Vec4f::new(1.0, 0.0, 0.0, 1.0),
            indicator_color: Vec4f::new(0.0, 1.0, 0.0, 1.0),
            point_selected_size: 3.0,
            depth_bias_use_default: true,
            depth_bias_enabled: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 1.0,
            depth_func: HdCompareFunction::LEqual,
            depth_mask_enabled: true,
            stencil_enabled: false,
            stencil_func: HdCompareFunction::Always,
            stencil_ref: 0,
            stencil_mask: !0,
            stencil_fail_op: HdStencilOp::Keep,
            stencil_z_fail_op: HdStencilOp::Keep,
            stencil_z_pass_op: HdStencilOp::Keep,
            blend_enabled: false,
            blend_color_op: HdBlendOp::Add,
            blend_color_src_factor: HdBlendFactor::One,
            blend_color_dst_factor: HdBlendFactor::Zero,
            blend_alpha_op: HdBlendOp::Add,
            blend_alpha_src_factor: HdBlendFactor::One,
            blend_alpha_dst_factor: HdBlendFactor::Zero,
            blend_constant_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            alpha_to_coverage_enabled: true,
            use_aov_multi_sample: true,
            resolve_aov_multi_sample: true,
        }
    }

    // ========================================================================
    // Setters - match C++ API
    // ========================================================================

    /// Set override color.
    pub fn set_override_color(&mut self, color: Vec4f) {
        self.override_color = color;
    }

    /// Set wireframe color.
    pub fn set_wireframe_color(&mut self, color: Vec4f) {
        self.wireframe_color = color;
    }

    /// Set point color.
    pub fn set_point_color(&mut self, color: Vec4f) {
        self.point_color = color;
    }

    /// Set point size.
    pub fn set_point_size(&mut self, size: f32) {
        self.point_size = size;
    }

    /// Set lighting enabled.
    pub fn set_lighting_enabled(&mut self, enabled: bool) {
        self.lighting_enabled = enabled;
    }

    /// Set clipping enabled.
    pub fn set_clipping_enabled(&mut self, enabled: bool) {
        self.clipping_enabled = enabled;
    }

    /// Set alpha threshold.
    pub fn set_alpha_threshold(&mut self, threshold: f32) {
        self.alpha_threshold = threshold;
    }

    /// Set cull style.
    pub fn set_cull_style(&mut self, style: HdCullStyle) {
        self.cull_style = style;
    }

    /// Set mask color.
    pub fn set_mask_color(&mut self, color: Vec4f) {
        self.mask_color = color;
    }

    /// Set indicator color.
    pub fn set_indicator_color(&mut self, color: Vec4f) {
        self.indicator_color = color;
    }

    /// Set point selected size.
    pub fn set_point_selected_size(&mut self, size: f32) {
        self.point_selected_size = size;
    }

    /// Set depth bias use default.
    pub fn set_depth_bias_use_default(&mut self, use_default: bool) {
        self.depth_bias_use_default = use_default;
    }

    /// Set depth bias enabled.
    pub fn set_depth_bias_enabled(&mut self, enabled: bool) {
        self.depth_bias_enabled = enabled;
    }

    /// Set depth bias factors.
    pub fn set_depth_bias(&mut self, constant_factor: f32, slope_factor: f32) {
        self.depth_bias_constant_factor = constant_factor;
        self.depth_bias_slope_factor = slope_factor;
    }

    /// Set depth function.
    pub fn set_depth_func(&mut self, func: HdCompareFunction) {
        self.depth_func = func;
    }

    /// Set depth mask enabled.
    pub fn set_enable_depth_mask(&mut self, enabled: bool) {
        self.depth_mask_enabled = enabled;
    }

    /// Set stencil enabled.
    pub fn set_stencil_enabled(&mut self, enabled: bool) {
        self.stencil_enabled = enabled;
    }

    /// Set stencil parameters.
    pub fn set_stencil(
        &mut self,
        func: HdCompareFunction,
        ref_value: i32,
        mask: i32,
        fail_op: HdStencilOp,
        z_fail_op: HdStencilOp,
        z_pass_op: HdStencilOp,
    ) {
        self.stencil_func = func;
        self.stencil_ref = ref_value;
        self.stencil_mask = mask;
        self.stencil_fail_op = fail_op;
        self.stencil_z_fail_op = z_fail_op;
        self.stencil_z_pass_op = z_pass_op;
    }

    /// Set blend enabled.
    pub fn set_blend_enabled(&mut self, enabled: bool) {
        self.blend_enabled = enabled;
    }

    /// Set blend parameters.
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

    /// Set blend constant color.
    pub fn set_blend_constant_color(&mut self, color: Vec4f) {
        self.blend_constant_color = color;
    }

    /// Set alpha to coverage enabled.
    pub fn set_alpha_to_coverage_enabled(&mut self, enabled: bool) {
        self.alpha_to_coverage_enabled = enabled;
    }

    /// Set use AOV multisample.
    pub fn set_use_aov_multi_sample(&mut self, enabled: bool) {
        self.use_aov_multi_sample = enabled;
    }

    /// Set resolve AOV multisample.
    pub fn set_resolve_aov_multi_sample(&mut self, enabled: bool) {
        self.resolve_aov_multi_sample = enabled;
    }

    /// Set camera path.
    pub fn set_camera_id(&mut self, camera_id: Path) {
        self.camera_id = camera_id;
    }

    /// Set camera view and projection matrices directly (used by pick/shadow passes).
    pub fn set_camera(&mut self, view: Matrix4d, proj: Matrix4d) {
        self.view_matrix = Some(view);
        self.proj_matrix = Some(proj);
    }

    /// Get camera view matrix.
    pub fn get_view_matrix(&self) -> Option<&Matrix4d> {
        self.view_matrix.as_ref()
    }

    /// Get camera projection matrix.
    pub fn get_proj_matrix(&self) -> Option<&Matrix4d> {
        self.proj_matrix.as_ref()
    }

    /// Set depth clamp enabled (shadow pass: objects beyond far/near clamp depth instead of clipping).
    pub fn set_enable_depth_clamp(&mut self, enabled: bool) {
        self.depth_clamp_enabled = enabled;
    }

    /// Set depth range [near, far] for the depth buffer (shadow maps use [0, 0.99999]).
    pub fn set_depth_range(&mut self, range: Vec2f) {
        self.depth_range = range;
    }

    /// Set framing.
    pub fn set_framing(&mut self, framing: CameraUtilFraming) {
        self.framing = framing;
    }

    /// Set viewport.
    pub fn set_viewport(&mut self, viewport: Vec4d) {
        self.viewport = viewport;
    }

    /// Set override window policy.
    pub fn set_override_window_policy(&mut self, policy: Option<CameraUtilConformWindowPolicy>) {
        self.override_window_policy = policy;
    }

    /// Set AOV bindings.
    pub fn set_aov_bindings(&mut self, bindings: HdRenderPassAovBindingVector) {
        self.aov_bindings = bindings;
    }

    /// Set AOV input bindings.
    pub fn set_aov_input_bindings(&mut self, bindings: HdRenderPassAovBindingVector) {
        self.aov_input_bindings = bindings;
    }

    // ========================================================================
    // Getters
    // ========================================================================

    /// Get camera path.
    pub fn get_camera_id(&self) -> &Path {
        &self.camera_id
    }

    /// Get viewport.
    pub fn get_viewport(&self) -> Vec4d {
        self.viewport
    }

    /// Get AOV bindings.
    pub fn get_aov_bindings(&self) -> &HdRenderPassAovBindingVector {
        &self.aov_bindings
    }

    /// Get lighting enabled.
    pub fn get_lighting_enabled(&self) -> bool {
        self.lighting_enabled
    }

    /// Get cull style.
    pub fn get_cull_style(&self) -> HdCullStyle {
        self.cull_style
    }

    /// Get depth function.
    pub fn get_depth_func(&self) -> HdCompareFunction {
        self.depth_func
    }

    /// Get depth-mask state.
    pub fn get_depth_mask_enabled(&self) -> bool {
        self.depth_mask_enabled
    }

    /// Get blend-enable state.
    pub fn get_blend_enabled(&self) -> bool {
        self.blend_enabled
    }

    /// Get blend state tuple in C++ parameter order.
    pub fn get_blend_state(
        &self,
    ) -> (
        HdBlendOp,
        HdBlendFactor,
        HdBlendFactor,
        HdBlendOp,
        HdBlendFactor,
        HdBlendFactor,
    ) {
        (
            self.blend_color_op,
            self.blend_color_src_factor,
            self.blend_color_dst_factor,
            self.blend_alpha_op,
            self.blend_alpha_src_factor,
            self.blend_alpha_dst_factor,
        )
    }

    /// Get alpha-to-coverage state.
    pub fn get_alpha_to_coverage_enabled(&self) -> bool {
        self.alpha_to_coverage_enabled
    }

    /// Get override color.
    pub fn get_override_color(&self) -> Vec4f {
        self.override_color
    }
}

/// Shared pointer to render pass state.
pub type HdxRenderPassStateSharedPtr = Arc<HdxRenderPassState>;

/// Value-storable handle for HdxRenderPassState.
///
/// Wraps `Arc<HdxRenderPassState>` with ptr-based equality and hash so it can
/// be inserted into `HdTaskContext` (which stores `Value`).
/// Mirrors the `HgiDriverHandle` pattern from usd-hgi.
#[derive(Clone)]
pub struct HdxRenderPassStateHandle(pub Arc<HdxRenderPassState>);

impl HdxRenderPassStateHandle {
    /// Create a new handle from a shared render pass state.
    pub fn new(state: Arc<HdxRenderPassState>) -> Self {
        Self(state)
    }

    /// Get a reference to the underlying state.
    pub fn get(&self) -> &Arc<HdxRenderPassState> {
        &self.0
    }
}

impl std::fmt::Debug for HdxRenderPassStateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdxRenderPassStateHandle")
            .field("ptr", &Arc::as_ptr(&self.0))
            .finish()
    }
}

impl PartialEq for HdxRenderPassStateHandle {
    fn eq(&self, other: &Self) -> bool {
        // Identity comparison by pointer, matching C++ shared_ptr semantics.
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for HdxRenderPassStateHandle {}

impl std::hash::Hash for HdxRenderPassStateHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// Render setup task - prepares render state.
///
/// Port of HdxRenderSetupTask from pxr/imaging/hdx/renderSetupTask.h
pub struct HdxRenderSetupTask {
    /// Task path
    id: Path,
    /// Render pass state
    render_pass_state: HdxRenderPassState,
    /// Camera path
    camera_id: Path,
    /// Framing
    framing: CameraUtilFraming,
    /// Override window policy
    override_window_policy: Option<CameraUtilConformWindowPolicy>,
    /// Viewport
    viewport: Vec4d,
    /// AOV bindings
    aov_bindings: HdRenderPassAovBindingVector,
    /// AOV input bindings
    aov_input_bindings: HdRenderPassAovBindingVector,
}

impl HdxRenderSetupTask {
    /// Create new render setup task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            render_pass_state: HdxRenderPassState::new(),
            camera_id: Path::empty(),
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
            viewport: Vec4d::new(0.0, 0.0, 0.0, 0.0),
            aov_bindings: Vec::new(),
            aov_input_bindings: Vec::new(),
        }
    }

    /// Sync parameters from task params.
    ///
    /// Called from HdxRenderTask when it has HdxRenderTaskParams.
    pub fn sync_params(&mut self, params: &HdxRenderTaskParams) {
        self.viewport = params.viewport;
        self.framing = params.framing.clone();
        self.override_window_policy = params.override_window_policy;
        self.camera_id = params.camera.clone();
        self.aov_bindings = params.aov_bindings.clone();
        self.aov_input_bindings = params.aov_input_bindings.clone();

        // Apply all render state params
        self.render_pass_state
            .set_override_color(params.override_color);
        self.render_pass_state
            .set_wireframe_color(params.wireframe_color);
        self.render_pass_state.set_point_color(params.point_color);
        self.render_pass_state.set_point_size(params.point_size);
        self.render_pass_state
            .set_lighting_enabled(params.enable_lighting);
        self.render_pass_state
            .set_clipping_enabled(params.enable_clipping);
        self.render_pass_state
            .set_alpha_threshold(params.alpha_threshold);
        self.render_pass_state.set_cull_style(params.cull_style);

        self.render_pass_state.set_mask_color(params.mask_color);
        self.render_pass_state
            .set_indicator_color(params.indicator_color);
        self.render_pass_state
            .set_point_selected_size(params.point_selected_size);

        // Depth bias
        self.render_pass_state
            .set_depth_bias_use_default(params.depth_bias_use_default);
        self.render_pass_state
            .set_depth_bias_enabled(params.depth_bias_enable);
        self.render_pass_state.set_depth_bias(
            params.depth_bias_constant_factor,
            params.depth_bias_slope_factor,
        );
        self.render_pass_state.set_depth_func(params.depth_func);
        self.render_pass_state
            .set_enable_depth_mask(params.depth_mask_enable);

        // Stencil
        self.render_pass_state
            .set_stencil_enabled(params.stencil_enable);
        self.render_pass_state.set_stencil(
            params.stencil_func,
            params.stencil_ref,
            params.stencil_mask,
            params.stencil_fail_op,
            params.stencil_z_fail_op,
            params.stencil_z_pass_op,
        );

        // Blending
        self.render_pass_state
            .set_blend_enabled(params.blend_enable);
        self.render_pass_state.set_blend(
            params.blend_color_op,
            params.blend_color_src_factor,
            params.blend_color_dst_factor,
            params.blend_alpha_op,
            params.blend_alpha_src_factor,
            params.blend_alpha_dst_factor,
        );
        self.render_pass_state
            .set_blend_constant_color(params.blend_constant_color);

        // Multisampling
        self.render_pass_state
            .set_alpha_to_coverage_enabled(params.enable_alpha_to_coverage);
        self.render_pass_state
            .set_use_aov_multi_sample(params.use_aov_multi_sample);
        self.render_pass_state
            .set_resolve_aov_multi_sample(params.resolve_aov_multi_sample);
    }

    /// Prepare camera for rendering.
    pub fn prepare_camera(&mut self, _render_index: &dyn HdRenderIndexTrait) {
        self.render_pass_state.set_camera_id(self.camera_id.clone());
        self.render_pass_state
            .set_override_window_policy(self.override_window_policy);

        if self.framing.is_valid() {
            self.render_pass_state.set_framing(self.framing.clone());
        } else {
            self.render_pass_state.set_viewport(self.viewport);
        }
    }

    /// Get the render pass state.
    pub fn get_render_pass_state(&self) -> HdxRenderPassStateSharedPtr {
        Arc::new(self.render_pass_state.clone())
    }

    /// Get render parameters.
    pub fn get_params(&self) -> HdxRenderTaskParams {
        let mut params = HdxRenderTaskParams::default();
        params.camera = self.camera_id.clone();
        params.viewport = self.viewport;
        params.framing = self.framing.clone();
        params.override_window_policy = self.override_window_policy;
        params.aov_bindings = self.aov_bindings.clone();
        params.aov_input_bindings = self.aov_input_bindings.clone();
        params.enable_lighting = self.render_pass_state.lighting_enabled;
        params.override_color = self.render_pass_state.override_color;
        params
    }
}

// Implement RenderPassState trait for HdxRenderPassState
impl usd_hd::render::render_pass::HdRenderPassState for HdxRenderPassState {
    fn get_camera(&self) -> Option<&Path> {
        if self.camera_id.is_empty() {
            None
        } else {
            Some(&self.camera_id)
        }
    }

    fn get_viewport(&self) -> (f32, f32, f32, f32) {
        (
            self.viewport.x as f32,
            self.viewport.y as f32,
            self.viewport.z as f32,
            self.viewport.w as f32,
        )
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl HdTask for HdxRenderSetupTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In a full implementation, we would pull params from delegate
        // For now, params are set via sync_params() directly
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, render_index: &dyn HdRenderIndexTrait) {
        // Prepare AOV bindings
        self.render_pass_state
            .set_aov_bindings(self.aov_bindings.clone());
        self.render_pass_state
            .set_aov_input_bindings(self.aov_input_bindings.clone());

        // Prepare camera
        self.prepare_camera(render_index);

        // In a full Storm implementation, Prepare() would also:
        //   - Call renderPassState->SetVolumeRenderingConstants(stepSize, stepSizeLighting)
        //   - Call renderPassState->SetEnableExposureCompensation(enabled)
        //   - Call renderPassState->Prepare(resourceRegistry)
        // These require access to the render delegate settings and resource registry.

        // Store render pass state in context so downstream tasks can retrieve it.
        // C++: (*ctx)[HdxTokens->renderPassState] = VtValue(_renderPassState);
        let handle = HdxRenderPassStateHandle::new(Arc::new(self.render_pass_state.clone()));
        ctx.insert(Token::new("renderPassState"), Value::new(handle));
        let needs_clear = self
            .aov_bindings
            .iter()
            .any(|binding| !binding.clear_value.is_empty());
        ctx.insert(Token::new("aovNeedsClear"), Value::from(needs_clear));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // Store the actual render pass state in the context so that downstream
        // HdxRenderTask can retrieve it.  This mirrors C++:
        //   (*ctx)[HdxTokens->renderPassState] = VtValue(_renderPassState);
        // We wrap the Arc in a Value-storable handle (ptr-equality/hash).
        let handle = HdxRenderPassStateHandle::new(Arc::new(self.render_pass_state.clone()));
        ctx.insert(Token::new("renderPassState"), Value::new(handle));
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
    fn test_render_task_params_default() {
        let params = HdxRenderTaskParams::default();
        assert_eq!(params.point_size, 3.0);
        assert!(!params.enable_lighting);
        assert!(params.enable_scene_lights);
        assert!(params.enable_clipping);
        assert!(!params.depth_bias_enable);
        assert!(params.depth_bias_use_default);
        assert_eq!(params.depth_func, HdCompareFunction::LEqual);
        assert!(!params.blend_enable);
        assert_eq!(params.cull_style, HdCullStyle::BackUnlessDoubleSided);
    }

    #[test]
    fn test_render_pass_state() {
        let mut state = HdxRenderPassState::new();

        state.set_lighting_enabled(true);
        state.set_point_size(5.0);
        state.set_cull_style(HdCullStyle::Nothing);

        assert!(state.lighting_enabled);
        assert_eq!(state.point_size, 5.0);
        assert_eq!(state.cull_style, HdCullStyle::Nothing);
    }

    #[test]
    fn test_render_setup_task() {
        let mut task = HdxRenderSetupTask::new(Path::from_string("/setup").unwrap());

        let mut params = HdxRenderTaskParams::default();
        params.enable_lighting = true;
        params.point_size = 5.0;
        params.viewport = Vec4d::new(0.0, 0.0, 1280.0, 720.0);

        task.sync_params(&params);

        assert!(task.render_pass_state.lighting_enabled);
        assert_eq!(task.render_pass_state.point_size, 5.0);
    }

    #[test]
    fn test_camera_framing() {
        let framing = CameraUtilFraming::new();
        assert!(!framing.is_valid()); // Default framing is invalid (data_window is 0,0,0,0)

        let mut valid_framing = CameraUtilFraming::new();
        valid_framing.data_window = (0, 0, 1920, 1080);
        assert!(valid_framing.is_valid());
    }

    #[test]
    fn test_camera_framing_is_valid_requires_both_dims() {
        // P1-10: is_valid() requires BOTH width AND height to be positive (&&, not ||)
        let mut f = CameraUtilFraming::new();
        f.pixel_aspect_ratio = 1.0;
        // Width > 0 but height == 0: should be INVALID
        f.data_window = (0, 0, 100, 0);
        assert!(!f.is_valid(), "only width positive should be invalid");
        // Height > 0 but width == 0: should be INVALID
        f.data_window = (0, 0, 0, 100);
        assert!(!f.is_valid(), "only height positive should be invalid");
        // Both positive: should be VALID
        f.data_window = (0, 0, 100, 100);
        assert!(f.is_valid(), "both positive should be valid");
    }

    #[test]
    fn test_render_pass_state_camera_matrices() {
        use usd_gf::Matrix4d;
        let mut state = HdxRenderPassState::new();
        assert!(state.get_view_matrix().is_none());
        assert!(state.get_proj_matrix().is_none());

        let view = Matrix4d::identity();
        let proj = Matrix4d::identity();
        state.set_camera(view, proj);
        assert!(state.get_view_matrix().is_some());
        assert!(state.get_proj_matrix().is_some());
    }

    #[test]
    fn test_render_pass_state_depth_shadow() {
        let mut state = HdxRenderPassState::new();
        assert!(!state.depth_clamp_enabled);
        assert_eq!(state.depth_range, Vec2f::new(0.0, 1.0));

        state.set_enable_depth_clamp(true);
        assert!(state.depth_clamp_enabled);

        state.set_depth_range(Vec2f::new(0.0, 0.99999));
        assert!((state.depth_range.y - 0.99999).abs() < 1e-6);
    }

    #[test]
    fn test_aov_binding() {
        let binding = HdRenderPassAovBinding::new(
            Token::new("color"),
            Path::from_string("/renderBuffer").unwrap(),
        )
        .with_clear_value(Value::from(Vec4f::new(0.0, 0.0, 0.0, 1.0)));

        assert_eq!(binding.aov_name.as_str(), "color");
        assert!(!binding.clear_value.is_empty());
    }
}
