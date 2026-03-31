
//! HdStRenderPassState - Storm render pass state management.
//!
//! Manages camera parameters, viewport, shader references, clip planes,
//! stencil/depth state, and other GPU pipeline state for render passes.

use crate::lighting::LightGpuData;
use crate::shadow::ShadowEntry;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use usd_gf::{Matrix4d, Vec4f};
use usd_hd::render::HdRenderPassState;
use usd_hgi::graphics_cmds_desc::HgiGraphicsCmdsDesc;
use usd_hgi::{
    HgiAttachmentDesc, HgiAttachmentLoadOp, HgiAttachmentStoreOp, HgiBlendFactor, HgiBlendOp,
    HgiBufferHandle, HgiCompareFunction, HgiCullMode, HgiDepthStencilState, HgiFormat,
    HgiGraphicsPipelineDesc, HgiMultiSampleState, HgiPolygonMode, HgiRasterizationState,
    HgiSamplerHandle, HgiStencilOp, HgiStencilState, HgiTextureHandle,
};
use usd_sdf::Path as SdfPath;

/// Polygon rasterization mode for glPolygonMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HdStPolygonRasterMode {
    /// Filled polygons (GL_FILL)
    #[default]
    Fill,
    /// Wireframe edges (GL_LINE)
    Line,
    /// Vertices as points (GL_POINT)
    Point,
}

/// Depth comparison function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepthFunc {
    Never,
    #[default]
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Stencil operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StencilOp {
    #[default]
    Keep,
    Zero,
    Replace,
    IncrClamp,
    DecrClamp,
    Invert,
    IncrWrap,
    DecrWrap,
}

/// AOV binding for render pass attachments.
///
/// Port of HdRenderPassAovBinding — describes a render target attachment
/// (color or depth) for a render pass.
#[derive(Debug, Clone)]
pub struct HdStAovBinding {
    /// AOV name token (e.g. "color", "depth", "primId")
    pub aov_name: usd_tf::Token,
    /// HGI texture handle for the render buffer backing this AOV
    pub texture: HgiTextureHandle,
    /// Format of the AOV
    pub format: HgiFormat,
    /// Clear value (color = RGBA, depth = r only)
    pub clear_value: Vec4f,
    /// Whether to clear on load
    pub clear_on_load: bool,
}

impl HdStAovBinding {
    /// Create a color AOV binding.
    pub fn new_color(texture: HgiTextureHandle, clear_color: Vec4f) -> Self {
        Self {
            aov_name: usd_tf::Token::new("color"),
            format: texture
                .get()
                .map(|texture| texture.descriptor().format)
                .unwrap_or(HgiFormat::UNorm8Vec4),
            texture,
            clear_value: clear_color,
            clear_on_load: true,
        }
    }

    /// Create a depth AOV binding.
    pub fn new_depth(texture: HgiTextureHandle) -> Self {
        Self {
            aov_name: usd_tf::Token::new("depth"),
            texture,
            format: HgiFormat::Float32,
            clear_value: Vec4f::new(1.0, 0.0, 0.0, 0.0),
            clear_on_load: true,
        }
    }
}

/// Storm render pass state.
///
/// Contains all state needed for a render pass including:
/// - Camera parameters (view/projection matrices)
/// - Viewport dimensions
/// - Clear colors
/// - Depth/stencil settings
/// - Culling settings
/// - Clip planes
/// - Shader references (lighting, render pass)
/// - AOV multisampling
/// - Line width
#[derive(Debug, Clone)]
pub struct HdStRenderPassState {
    /// Camera path
    camera: Option<SdfPath>,

    /// Viewport (x, y, width, height)
    viewport: (f32, f32, f32, f32),

    /// View matrix
    view_matrix: Matrix4d,

    /// Projection matrix
    proj_matrix: Matrix4d,

    /// Clear color (RGBA)
    clear_color: Vec4f,

    /// Enable depth testing
    depth_test_enabled: bool,

    /// Enable depth writing
    depth_write_enabled: bool,

    /// Enable blending
    blend_enabled: bool,

    /// Blend color operation (port of C++ _blendColorOp)
    blend_color_op: HgiBlendOp,
    /// Source color blend factor (port of C++ _blendColorSrcFactor)
    blend_color_src_factor: HgiBlendFactor,
    /// Destination color blend factor (port of C++ _blendColorDstFactor)
    blend_color_dst_factor: HgiBlendFactor,
    /// Blend alpha operation (port of C++ _blendAlphaOp)
    blend_alpha_op: HgiBlendOp,
    /// Source alpha blend factor (port of C++ _blendAlphaSrcFactor)
    blend_alpha_src_factor: HgiBlendFactor,
    /// Destination alpha blend factor (port of C++ _blendAlphaDstFactor)
    blend_alpha_dst_factor: HgiBlendFactor,

    /// Cull mode for rasterization.
    /// Port of C++ geometricShader->ResolveCullMode().
    /// Default: Back (back-face culling for single-sided opaque geometry).
    cull_mode: HgiCullMode,

    /// Enable culling
    cull_enabled: bool,

    /// Polygon rasterization mode (fill, wireframe, points)
    polygon_raster_mode: HdStPolygonRasterMode,

    /// Whether selection highlighting is enabled
    highlight_enabled: bool,

    /// Selection highlight color (RGBA)
    selection_color: Vec4f,

    /// Paths that are selected (prims under these are highlighted)
    selected_paths: HashSet<SdfPath>,

    /// Clip planes (each is [a, b, c, d] for ax + by + cz + d = 0)
    clip_planes: Vec<[f64; 4]>,

    /// Whether to resolve multi-sampled AOVs at end of render pass
    resolve_aov_multisample: bool,

    /// Cull matrix (view-projection for frustum culling)
    cull_matrix: Matrix4d,

    /// Camera framing state override (when no HdCamera is set)
    camera_framing_override: bool,
    /// Override world-to-view matrix
    override_world_to_view: Matrix4d,
    /// Override projection matrix
    override_projection: Matrix4d,
    /// Override viewport
    override_viewport: [f64; 4],
    /// Override clip planes
    override_clip_planes: Vec<[f64; 4]>,

    /// Depth comparison function
    depth_func: DepthFunc,

    /// Stencil test enabled
    stencil_enabled: bool,
    /// Stencil function
    stencil_func: DepthFunc,
    /// Stencil reference value
    stencil_ref: i32,
    /// Stencil mask
    stencil_mask: u32,
    /// Stencil fail operation
    stencil_fail_op: StencilOp,
    /// Stencil pass + depth fail operation
    stencil_z_fail_op: StencilOp,
    /// Stencil pass + depth pass operation
    stencil_z_pass_op: StencilOp,

    /// Line width for line/wireframe rendering
    line_width: f32,

    /// Color mask (RGBA)
    color_mask: [bool; 4],

    /// Alpha to coverage
    alpha_to_coverage: bool,

    /// Scene lights collected by SimpleLightTask (empty = use default 3-point)
    scene_lights: Vec<LightGpuData>,

    /// Shadow entries for shadow-casting lights.
    /// Matches C++ GlfSimpleLightingContext shadow data.
    shadow_entries: Vec<ShadowEntry>,

    /// Whether shadow mapping is active (any light casts shadows).
    /// Matches C++ GlfSimpleLightingContext::GetUseShadows().
    use_shadows: bool,

    /// Shadow atlas depth texture (texture_depth_2d_array, Depth32Float).
    /// Set by engine after rendering shadow depth passes.
    shadow_atlas: Option<usd_hgi::texture::HgiTextureHandle>,

    /// Shadow comparison sampler (sampler_comparison with LessEqual).
    shadow_sampler: Option<usd_hgi::sampler::HgiSamplerHandle>,

    /// AOV render target bindings (color + depth attachments).
    /// Port of HdRenderPassState::GetAovBindings().
    aov_bindings: Vec<HdStAovBinding>,
    /// Optional writable storage buffer used by pick/deep-resolve passes.
    pick_buffer: Option<HgiBufferHandle>,
    /// Color attachment format used when no explicit color AOV is bound.
    fallback_color_format: HgiFormat,
    /// Depth attachment format used when no explicit depth AOV is bound.
    fallback_depth_format: HgiFormat,

    /// When false, draw batches use default grey material instead of authored materials.
    /// Matches C++ HdRenderPassState::GetEnableSceneMaterials().
    enable_scene_materials: bool,

    /// Use face normals (dpdx/dpdy) instead of vertex normals for lit shading.
    /// Set when engine draw mode is ShadedFlat.
    flat_shading: bool,

    /// Default material ambient intensity [0..1] from UI slider.
    /// Scales scene.ambient_color in the shader.
    default_material_ambient: f32,

    /// Default material specular intensity [0..1] from UI slider.
    /// Controls specular highlight strength of the default material.
    default_material_specular: f32,

    /// Depth-only pass: write depth but suppress color output.
    /// Used for HiddenSurfaceWireframe depth prepass.
    depth_only: bool,

    /// Load op for the color attachment.
    /// Clear = start fresh (default). Load = preserve (after skydome).
    color_load_op: HgiAttachmentLoadOp,

    /// Load op for the depth attachment.
    /// Clear = start fresh (default, pass 1).
    /// Load = preserve previous depth (pass 2, wireframe over prepass).
    depth_load_op: HgiAttachmentLoadOp,

    /// IBL textures: (irradiance_tex, irradiance_smp, prefilter_tex, prefilter_smp, brdf_tex, brdf_smp)
    /// Set by engine when a DomeLight with HDRI is present.
    ibl_handles: Option<IblHandles>,

    /// wgpu device + queue for GPU frustum culling compute pass.
    /// Set by the engine after device creation; None until the engine sets it.
    #[cfg(feature = "gpu-culling")]
    wgpu_device: Option<std::sync::Arc<wgpu::Device>>,
    #[cfg(feature = "gpu-culling")]
    wgpu_queue: Option<std::sync::Arc<wgpu::Queue>>,
}

/// GPU handles for IBL textures (group 4).
#[derive(Debug, Clone)]
pub struct IblHandles {
    /// Raw environment cubemap (unfiltered, for skydome background).
    pub env_cubemap_tex: HgiTextureHandle,
    pub env_cubemap_smp: HgiSamplerHandle,
    pub irradiance_tex: HgiTextureHandle,
    pub irradiance_smp: HgiSamplerHandle,
    pub prefilter_tex: HgiTextureHandle,
    pub prefilter_smp: HgiSamplerHandle,
    pub brdf_lut_tex: HgiTextureHandle,
    pub brdf_lut_smp: HgiSamplerHandle,
}

impl HdStRenderPassState {
    /// Create a new render pass state with defaults.
    pub fn new() -> Self {
        Self {
            camera: None,
            viewport: (0.0, 0.0, 0.0, 0.0),
            view_matrix: Matrix4d::identity(),
            proj_matrix: Matrix4d::identity(),
            clear_color: Vec4f::new(0.0, 0.0, 0.0, 1.0),
            depth_test_enabled: true,
            depth_write_enabled: true,
            blend_enabled: false,
            blend_color_op: HgiBlendOp::Add,
            blend_color_src_factor: HgiBlendFactor::One,
            blend_color_dst_factor: HgiBlendFactor::Zero,
            blend_alpha_op: HgiBlendOp::Add,
            blend_alpha_src_factor: HgiBlendFactor::One,
            blend_alpha_dst_factor: HgiBlendFactor::Zero,
            cull_mode: HgiCullMode::Back,
            cull_enabled: true,
            polygon_raster_mode: HdStPolygonRasterMode::Fill,
            highlight_enabled: false,
            selection_color: Vec4f::new(1.0, 1.0, 0.0, 1.0),
            selected_paths: HashSet::new(),
            clip_planes: Vec::new(),
            resolve_aov_multisample: true,
            cull_matrix: Matrix4d::identity(),
            camera_framing_override: false,
            override_world_to_view: Matrix4d::identity(),
            override_projection: Matrix4d::identity(),
            override_viewport: [0.0, 0.0, 0.0, 0.0],
            override_clip_planes: Vec::new(),
            depth_func: DepthFunc::Less,
            stencil_enabled: false,
            stencil_func: DepthFunc::Always,
            stencil_ref: 0,
            stencil_mask: 0xFF,
            stencil_fail_op: StencilOp::Keep,
            stencil_z_fail_op: StencilOp::Keep,
            stencil_z_pass_op: StencilOp::Keep,
            line_width: 1.0,
            color_mask: [true, true, true, true],
            alpha_to_coverage: false,
            scene_lights: Vec::new(),
            shadow_entries: Vec::new(),
            use_shadows: false,
            shadow_atlas: None,
            shadow_sampler: None,
            aov_bindings: Vec::new(),
            pick_buffer: None,
            fallback_color_format: HgiFormat::UNorm8Vec4,
            fallback_depth_format: HgiFormat::Float32,
            enable_scene_materials: true,
            flat_shading: false,
            default_material_ambient: 0.2,
            default_material_specular: 0.1,
            depth_only: false,
            color_load_op: HgiAttachmentLoadOp::Clear,
            depth_load_op: HgiAttachmentLoadOp::Clear,
            ibl_handles: None,
            #[cfg(feature = "gpu-culling")]
            wgpu_device: None,
            #[cfg(feature = "gpu-culling")]
            wgpu_queue: None,
        }
    }

    /// Set selection highlighting parameters.
    pub fn set_selection_highlight(
        &mut self,
        enabled: bool,
        color: Vec4f,
        paths: impl IntoIterator<Item = SdfPath>,
    ) {
        self.highlight_enabled = enabled;
        self.selection_color = color;
        self.selected_paths = paths.into_iter().collect();
    }

    /// Check if selection highlighting is enabled.
    pub fn is_highlight_enabled(&self) -> bool {
        self.highlight_enabled
    }

    /// Get selection color.
    pub fn get_selection_color(&self) -> Vec4f {
        self.selection_color
    }

    /// Check if a prim path is selected (path equals or is under a selected path).
    pub fn is_path_selected(&self, path: &SdfPath) -> bool {
        if !self.highlight_enabled || self.selected_paths.is_empty() {
            return false;
        }
        self.selected_paths.iter().any(|sel| path.has_prefix(sel))
    }

    /// Set polygon rasterization mode.
    pub fn set_polygon_raster_mode(&mut self, mode: HdStPolygonRasterMode) {
        self.polygon_raster_mode = mode;
    }

    /// Get polygon rasterization mode.
    pub fn get_polygon_raster_mode(&self) -> HdStPolygonRasterMode {
        self.polygon_raster_mode
    }

    /// Enable flat shading (face normals via dpdx/dpdy instead of vertex normals).
    pub fn set_flat_shading(&mut self, flat: bool) {
        self.flat_shading = flat;
    }

    /// Whether flat shading is enabled (ShadedFlat draw mode).
    pub fn is_flat_shading(&self) -> bool {
        self.flat_shading
    }

    /// Set camera path.
    pub fn set_camera(&mut self, camera: SdfPath) {
        self.camera = Some(camera);
    }

    /// Set viewport.
    pub fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.viewport = (x, y, width, height);
    }

    /// Get viewport (x, y, width, height).
    pub fn get_viewport(&self) -> (f32, f32, f32, f32) {
        self.viewport
    }

    /// Set view matrix.
    pub fn set_view_matrix(&mut self, matrix: Matrix4d) {
        self.view_matrix = matrix;
    }

    /// Get view matrix.
    pub fn get_view_matrix(&self) -> &Matrix4d {
        &self.view_matrix
    }

    /// Set projection matrix.
    pub fn set_proj_matrix(&mut self, matrix: Matrix4d) {
        self.proj_matrix = matrix;
    }

    /// Get projection matrix.
    pub fn get_proj_matrix(&self) -> &Matrix4d {
        &self.proj_matrix
    }

    /// Set clear color.
    pub fn set_clear_color(&mut self, color: Vec4f) {
        self.clear_color = color;
    }

    /// Get clear color.
    pub fn get_clear_color(&self) -> Vec4f {
        self.clear_color
    }

    /// Enable/disable depth testing.
    pub fn set_depth_test_enabled(&mut self, enabled: bool) {
        self.depth_test_enabled = enabled;
    }

    /// Check if depth testing is enabled.
    pub fn is_depth_test_enabled(&self) -> bool {
        self.depth_test_enabled
    }

    /// Enable/disable depth writing.
    pub fn set_depth_write_enabled(&mut self, enabled: bool) {
        self.depth_write_enabled = enabled;
    }

    /// Check if depth writing is enabled.
    pub fn is_depth_write_enabled(&self) -> bool {
        self.depth_write_enabled
    }

    /// Enable/disable blending.
    pub fn set_blend_enabled(&mut self, enabled: bool) {
        self.blend_enabled = enabled;
    }

    /// Check if blending is enabled.
    pub fn is_blend_enabled(&self) -> bool {
        self.blend_enabled
    }

    /// Set blend factors.
    ///
    /// Port of C++ HdRenderPassState::SetBlend.
    /// Called by the task controller per material tag.
    pub fn set_blend(
        &mut self,
        color_op: HgiBlendOp,
        color_src: HgiBlendFactor,
        color_dst: HgiBlendFactor,
        alpha_op: HgiBlendOp,
        alpha_src: HgiBlendFactor,
        alpha_dst: HgiBlendFactor,
    ) {
        self.blend_color_op = color_op;
        self.blend_color_src_factor = color_src;
        self.blend_color_dst_factor = color_dst;
        self.blend_alpha_op = alpha_op;
        self.blend_alpha_src_factor = alpha_src;
        self.blend_alpha_dst_factor = alpha_dst;
    }

    /// Set blend state for a material tag.
    ///
    /// Port of C++ HdxTaskController::_SetBlendStateForMaterialTag.
    /// Configures blend factors, depth mask, and alpha-to-coverage
    /// based on the material tag string.
    pub fn set_blend_for_material_tag(&mut self, tag: &str) {
        match tag {
            "additive" => {
                // C++ additive: src=One dst=One for color, src=Zero dst=One for alpha
                // Expects pre-multiplied alpha: vec4(rgb*a, a)
                self.blend_enabled = true;
                self.set_blend(
                    HgiBlendOp::Add,
                    HgiBlendFactor::One,
                    HgiBlendFactor::One,
                    HgiBlendOp::Add,
                    HgiBlendFactor::Zero,
                    HgiBlendFactor::One,
                );
                self.depth_write_enabled = false;
                self.alpha_to_coverage = false;
            }
            "translucent" => {
                // C++ uses OIT for translucent. Without OIT, use standard alpha-over
                // blend as fallback: src=SrcAlpha, dst=OneMinusSrcAlpha.
                self.blend_enabled = true;
                self.set_blend(
                    HgiBlendOp::Add,
                    HgiBlendFactor::SrcAlpha,
                    HgiBlendFactor::OneMinusSrcAlpha,
                    HgiBlendOp::Add,
                    HgiBlendFactor::One,
                    HgiBlendFactor::OneMinusSrcAlpha,
                );
                self.depth_write_enabled = false;
                self.alpha_to_coverage = false;
            }
            "defaultMaterialTag" | "masked" | _ => {
                // C++: blend=false, depthMask=true, alphaToCoverage=true
                self.blend_enabled = false;
                self.set_blend(
                    HgiBlendOp::Add,
                    HgiBlendFactor::One,
                    HgiBlendFactor::Zero,
                    HgiBlendOp::Add,
                    HgiBlendFactor::One,
                    HgiBlendFactor::Zero,
                );
                self.depth_write_enabled = true;
                self.alpha_to_coverage = true;
            }
        }
    }

    /// Get blend color op.
    pub fn get_blend_color_op(&self) -> HgiBlendOp {
        self.blend_color_op
    }
    /// Get src color blend factor.
    pub fn get_blend_color_src_factor(&self) -> HgiBlendFactor {
        self.blend_color_src_factor
    }
    /// Get dst color blend factor.
    pub fn get_blend_color_dst_factor(&self) -> HgiBlendFactor {
        self.blend_color_dst_factor
    }
    /// Get blend alpha op.
    pub fn get_blend_alpha_op(&self) -> HgiBlendOp {
        self.blend_alpha_op
    }
    /// Get src alpha blend factor.
    pub fn get_blend_alpha_src_factor(&self) -> HgiBlendFactor {
        self.blend_alpha_src_factor
    }
    /// Get dst alpha blend factor.
    pub fn get_blend_alpha_dst_factor(&self) -> HgiBlendFactor {
        self.blend_alpha_dst_factor
    }

    /// Set cull mode.
    ///
    /// Port of C++ _InitRasterizationState cull_mode assignment.
    pub fn set_cull_mode(&mut self, mode: HgiCullMode) {
        self.cull_mode = mode;
    }

    /// Get cull mode.
    pub fn get_cull_mode(&self) -> HgiCullMode {
        self.cull_mode
    }

    /// Enable/disable culling.
    pub fn set_cull_enabled(&mut self, enabled: bool) {
        self.cull_enabled = enabled;
    }

    /// Check if culling is enabled.
    pub fn is_cull_enabled(&self) -> bool {
        self.cull_enabled
    }

    // --- Clip planes ---

    /// Set clip planes.
    pub fn set_clip_planes(&mut self, planes: Vec<[f64; 4]>) {
        self.clip_planes = planes;
    }

    /// Get clip planes.
    pub fn get_clip_planes(&self) -> &[[f64; 4]] {
        if self.camera_framing_override && !self.override_clip_planes.is_empty() {
            &self.override_clip_planes
        } else {
            &self.clip_planes
        }
    }

    // --- AOV multisample ---

    /// Set whether to resolve multi-sampled AOVs.
    pub fn set_resolve_aov_multisample(&mut self, state: bool) {
        self.resolve_aov_multisample = state;
    }

    /// Get resolve AOV multisample state.
    pub fn get_resolve_aov_multisample(&self) -> bool {
        self.resolve_aov_multisample
    }

    // --- Camera framing state (override when no HdCamera) ---

    /// Set camera framing state explicitly (for shadow passes, etc.).
    pub fn set_camera_framing_state(
        &mut self,
        world_to_view: Matrix4d,
        projection: Matrix4d,
        viewport: [f64; 4],
        clip_planes: Vec<[f64; 4]>,
    ) {
        self.camera_framing_override = true;
        self.override_world_to_view = world_to_view;
        self.override_projection = projection;
        self.override_viewport = viewport;
        self.override_clip_planes = clip_planes;
    }

    /// Get the effective world-to-view matrix (override or camera).
    pub fn get_world_to_view_matrix(&self) -> &Matrix4d {
        if self.camera_framing_override {
            &self.override_world_to_view
        } else {
            &self.view_matrix
        }
    }

    /// Get the effective projection matrix.
    pub fn get_projection_matrix(&self) -> &Matrix4d {
        if self.camera_framing_override {
            &self.override_projection
        } else {
            &self.proj_matrix
        }
    }

    /// Get cull matrix.
    pub fn get_cull_matrix(&self) -> &Matrix4d {
        &self.cull_matrix
    }

    /// Set cull matrix.
    pub fn set_cull_matrix(&mut self, matrix: Matrix4d) {
        self.cull_matrix = matrix;
    }

    /// Store wgpu device + queue for GPU frustum culling.
    ///
    /// Called by the engine after wgpu initialization.  The references are kept
    /// alive for the lifetime of the render pass state (via Arc).
    #[cfg(feature = "gpu-culling")]
    pub fn set_wgpu_device_queue(
        &mut self,
        device: std::sync::Arc<wgpu::Device>,
        queue: std::sync::Arc<wgpu::Queue>,
    ) {
        self.wgpu_device = Some(device);
        self.wgpu_queue = Some(queue);
    }

    /// Get wgpu device + queue for GPU frustum culling.
    ///
    /// Returns None when the engine has not yet called `set_wgpu_device_queue`.
    #[cfg(feature = "gpu-culling")]
    pub fn get_wgpu_device_queue(
        &self,
    ) -> Option<(&std::sync::Arc<wgpu::Device>, &std::sync::Arc<wgpu::Queue>)> {
        match (&self.wgpu_device, &self.wgpu_queue) {
            (Some(d), Some(q)) => Some((d, q)),
            _ => None,
        }
    }

    /// No-op when `gpu-culling` feature is disabled.
    #[cfg(not(feature = "gpu-culling"))]
    pub fn get_wgpu_device_queue(&self) -> Option<()> {
        None
    }

    /// Drop the wgpu device + queue references.
    ///
    /// Called by the engine during device reset (file reload) to release old-device
    /// Arcs before the new device is created. Without this the old wgpu::Device
    /// stays alive (Arc refcount > 0), causing epoch mismatches when new BGLs are
    /// validated against the old device's internal slab.
    #[cfg(feature = "gpu-culling")]
    pub fn clear_device_resources(&mut self) {
        self.wgpu_device = None;
        self.wgpu_queue = None;
    }

    /// No-op when `gpu-culling` feature is disabled.
    #[cfg(not(feature = "gpu-culling"))]
    pub fn clear_device_resources(&mut self) {}

    /// Compute the effective viewport as integer values.
    pub fn compute_viewport(&self) -> [i32; 4] {
        let (x, y, w, h) = if self.camera_framing_override {
            (
                self.override_viewport[0],
                self.override_viewport[1],
                self.override_viewport[2],
                self.override_viewport[3],
            )
        } else {
            (
                self.viewport.0 as f64,
                self.viewport.1 as f64,
                self.viewport.2 as f64,
                self.viewport.3 as f64,
            )
        };
        [x as i32, y as i32, w as i32, h as i32]
    }

    // --- Depth/stencil state ---

    /// Set depth comparison function.
    pub fn set_depth_func(&mut self, func: DepthFunc) {
        self.depth_func = func;
    }

    /// Get depth comparison function.
    pub fn get_depth_func(&self) -> DepthFunc {
        self.depth_func
    }

    /// Enable/disable stencil test.
    pub fn set_stencil_enabled(&mut self, enabled: bool) {
        self.stencil_enabled = enabled;
    }

    /// Check if stencil test is enabled.
    pub fn is_stencil_enabled(&self) -> bool {
        self.stencil_enabled
    }

    /// Set stencil function and reference.
    pub fn set_stencil_func(&mut self, func: DepthFunc, ref_val: i32, mask: u32) {
        self.stencil_func = func;
        self.stencil_ref = ref_val;
        self.stencil_mask = mask;
    }

    /// Set stencil operations.
    pub fn set_stencil_op(&mut self, fail: StencilOp, z_fail: StencilOp, z_pass: StencilOp) {
        self.stencil_fail_op = fail;
        self.stencil_z_fail_op = z_fail;
        self.stencil_z_pass_op = z_pass;
    }

    // --- Line width ---

    /// Set line width for wireframe rendering.
    pub fn set_line_width(&mut self, width: f32) {
        self.line_width = width;
    }

    /// Get line width.
    pub fn get_line_width(&self) -> f32 {
        self.line_width
    }

    // --- Color mask ---

    /// Set color write mask (R, G, B, A).
    pub fn set_color_mask(&mut self, r: bool, g: bool, b: bool, a: bool) {
        self.color_mask = [r, g, b, a];
    }

    /// Get color mask.
    pub fn get_color_mask(&self) -> [bool; 4] {
        self.color_mask
    }

    // --- Alpha to coverage ---

    /// Set alpha to coverage.
    pub fn set_alpha_to_coverage(&mut self, enabled: bool) {
        self.alpha_to_coverage = enabled;
    }

    /// Get alpha to coverage.
    pub fn get_alpha_to_coverage(&self) -> bool {
        self.alpha_to_coverage
    }

    // --- Scene lights ---

    /// Set scene lights for the current frame.
    ///
    /// Called by the engine after collecting lights from the render index.
    /// If empty, draw batches fall back to default 3-point lighting.
    pub fn set_scene_lights(&mut self, lights: Vec<LightGpuData>) {
        self.scene_lights = lights;
    }

    /// Get scene lights.
    ///
    /// Returns the lights set for the current frame.
    pub fn get_scene_lights(&self) -> &[LightGpuData] {
        &self.scene_lights
    }

    /// Whether any scene lights are configured.
    pub fn has_scene_lights(&self) -> bool {
        !self.scene_lights.is_empty()
    }

    // --- Shadow state ---

    /// Set shadow entries for shadow-casting lights.
    /// Automatically sets use_shadows = true if entries is non-empty.
    /// Matches C++ simpleLightTask.cpp shadow matrix computation flow.
    pub fn set_shadow_entries(&mut self, entries: Vec<ShadowEntry>) {
        self.use_shadows = !entries.is_empty();
        self.shadow_entries = entries;
    }

    /// Get shadow entries.
    pub fn get_shadow_entries(&self) -> &[ShadowEntry] {
        &self.shadow_entries
    }

    /// Whether shadow mapping is active.
    pub fn has_shadows(&self) -> bool {
        self.use_shadows && !self.shadow_entries.is_empty()
    }

    /// Whether shadow atlas texture is available for depth comparison.
    pub fn has_shadow_atlas(&self) -> bool {
        self.has_shadows() && self.shadow_atlas.is_some() && self.shadow_sampler.is_some()
    }

    /// Set shadow atlas texture + comparison sampler (from engine after depth pass).
    pub fn set_shadow_atlas(
        &mut self,
        atlas: usd_hgi::texture::HgiTextureHandle,
        sampler: usd_hgi::sampler::HgiSamplerHandle,
    ) {
        self.shadow_atlas = Some(atlas);
        self.shadow_sampler = Some(sampler);
    }

    /// Get shadow atlas texture handle.
    pub fn get_shadow_atlas(&self) -> Option<&usd_hgi::texture::HgiTextureHandle> {
        self.shadow_atlas.as_ref()
    }

    /// Get shadow comparison sampler handle.
    pub fn get_shadow_sampler(&self) -> Option<&usd_hgi::sampler::HgiSamplerHandle> {
        self.shadow_sampler.as_ref()
    }

    /// Clear shadow atlas (when shadows disabled or atlas destroyed).
    pub fn clear_shadow_atlas(&mut self) {
        self.shadow_atlas = None;
        self.shadow_sampler = None;
    }

    /// Set whether authored scene materials are used.
    /// When false, draw batches render with default grey material (GeomOnly mode).
    pub fn set_enable_scene_materials(&mut self, enable: bool) {
        self.enable_scene_materials = enable;
    }

    /// Whether authored scene materials should be applied.
    pub fn get_enable_scene_materials(&self) -> bool {
        self.enable_scene_materials
    }

    /// Set default material ambient intensity [0..1] from UI slider.
    pub fn set_default_material_ambient(&mut self, value: f32) {
        self.default_material_ambient = value;
    }

    /// Default material ambient intensity [0..1].
    pub fn get_default_material_ambient(&self) -> f32 {
        self.default_material_ambient
    }

    /// Set default material specular intensity [0..1] from UI slider.
    pub fn set_default_material_specular(&mut self, value: f32) {
        self.default_material_specular = value;
    }

    /// Default material specular intensity [0..1].
    pub fn get_default_material_specular(&self) -> f32 {
        self.default_material_specular
    }

    /// Set depth-only mode (no color output, only depth writes).
    pub fn set_depth_only(&mut self, depth_only: bool) {
        self.depth_only = depth_only;
    }

    /// Whether this is a depth-only pass (no color output).
    pub fn is_depth_only(&self) -> bool {
        self.depth_only
    }

    /// Set the load op for the depth attachment.
    ///
    /// Use `Clear` (default) to start with a fresh depth buffer.
    /// Set color attachment load op (Clear or Load).
    /// Use `Load` to preserve skydome background behind geometry.
    pub fn set_color_load_op(&mut self, op: HgiAttachmentLoadOp) {
        self.color_load_op = op;
    }

    /// Use `Load` to preserve depth written by a previous pass (e.g. hidden-surface wireframe).
    pub fn set_depth_load_op(&mut self, op: HgiAttachmentLoadOp) {
        self.depth_load_op = op;
    }

    /// Set IBL GPU texture handles from a loaded DomeLight HDRI.
    pub fn set_ibl_handles(&mut self, handles: IblHandles) {
        self.ibl_handles = Some(handles);
    }

    /// Clear IBL handles (no dome light in scene).
    pub fn clear_ibl_handles(&mut self) {
        self.ibl_handles = None;
    }

    /// Get IBL handles if available.
    pub fn get_ibl_handles(&self) -> Option<&IblHandles> {
        self.ibl_handles.as_ref()
    }

    /// Whether IBL textures are ready for binding.
    pub fn has_ibl(&self) -> bool {
        self.ibl_handles.is_some()
    }

    // --- AOV bindings ---

    /// Set AOV render target bindings.
    ///
    /// Port of HdRenderPassState::SetAovBindings (P0-10).
    pub fn set_aov_bindings(&mut self, bindings: Vec<HdStAovBinding>) {
        self.aov_bindings = bindings;
    }

    /// Get AOV render target bindings.
    pub fn get_aov_bindings(&self) -> &[HdStAovBinding] {
        &self.aov_bindings
    }

    /// Set the optional pick/deep-resolve storage buffer.
    pub fn set_pick_buffer(&mut self, buffer: Option<HgiBufferHandle>) {
        self.pick_buffer = buffer;
    }

    /// Get the optional pick/deep-resolve storage buffer.
    pub fn get_pick_buffer(&self) -> Option<&HgiBufferHandle> {
        self.pick_buffer.as_ref()
    }

    /// Set fallback attachment formats used when explicit AOV bindings are absent.
    pub fn set_fallback_attachment_formats(
        &mut self,
        color_format: HgiFormat,
        depth_format: HgiFormat,
    ) {
        self.fallback_color_format = color_format;
        self.fallback_depth_format = depth_format;
    }

    /// Get the active color attachment format for the current render pass.
    pub fn get_color_attachment_format(&self) -> HgiFormat {
        self.aov_bindings
            .iter()
            .find(|aov| {
                !(aov.aov_name == "depth"
                    || aov.aov_name.as_str().contains("depth")
                    || aov.aov_name == "depthStencil")
            })
            .map(|aov| aov.format)
            .unwrap_or(self.fallback_color_format)
    }

    /// Get the active depth attachment format for the current render pass.
    pub fn get_depth_attachment_format(&self) -> HgiFormat {
        self.aov_bindings
            .iter()
            .find(|aov| {
                aov.aov_name == "depth"
                    || aov.aov_name.as_str().contains("depth")
                    || aov.aov_name == "depthStencil"
            })
            .map(|aov| aov.format)
            .unwrap_or(self.fallback_depth_format)
    }

    /// Build HgiGraphicsCmdsDesc from AOV bindings.
    ///
    /// Port of HdStRenderPassState::MakeGraphicsCmdsDesc (P0-10).
    /// Iterates AOV bindings and sorts them into color or depth attachments.
    /// Falls back to provided texture handles when no AOVs are bound.
    pub fn make_graphics_cmds_desc(
        &self,
        color_texture: Option<&HgiTextureHandle>,
        depth_texture: Option<&HgiTextureHandle>,
    ) -> HgiGraphicsCmdsDesc {
        // If AOV bindings are set, build from them (Storm pipeline path).
        if !self.aov_bindings.is_empty() {
            let mut desc = HgiGraphicsCmdsDesc::default();
            for aov in &self.aov_bindings {
                let is_depth = aov.aov_name == "depth"
                    || aov.aov_name.as_str().contains("depth")
                    || aov.aov_name == "depthStencil";

                let load_op = if aov.clear_on_load {
                    HgiAttachmentLoadOp::Clear
                } else {
                    HgiAttachmentLoadOp::Load
                };

                let attach = HgiAttachmentDesc::new()
                    .with_format(aov.format)
                    .with_load_op(load_op)
                    .with_store_op(HgiAttachmentStoreOp::Store)
                    .with_clear_value(aov.clear_value);

                if is_depth {
                    desc.depth_attachment_desc = attach;
                    desc.depth_texture = aov.texture.clone();
                } else {
                    desc.color_attachment_descs.push(attach);
                    desc.color_textures.push(aov.texture.clone());
                }
            }
            return desc;
        }

        // Fallback: build from provided texture handles directly.
        let mut desc = HgiGraphicsCmdsDesc::default();

        if let Some(ct) = color_texture {
            let color_attach = HgiAttachmentDesc::new()
                .with_format(self.fallback_color_format)
                .with_load_op(self.color_load_op)
                .with_store_op(HgiAttachmentStoreOp::Store)
                .with_clear_value(self.clear_color)
                // Propagate render pass blend state to attachment descriptor.
                .with_blend_enabled(self.blend_enabled);
            // Alpha-over blend is controlled via HgiColorBlendState if needed.
            // HgiAttachmentDesc doesn't have a dedicated alpha-over method;
            // blend state is set at the pipeline level (see init_graphics_pipeline_desc).
            let color_attach = color_attach;
            desc.color_attachment_descs.push(color_attach);
            desc.color_textures.push(ct.clone());
        }

        if let Some(dt) = depth_texture {
            let depth_attach = HgiAttachmentDesc::new()
                .with_format(self.fallback_depth_format)
                .with_load_op(self.depth_load_op)
                .with_store_op(HgiAttachmentStoreOp::Store)
                // clear_value only used when load_op == Clear, ignored otherwise
                .with_clear_value(Vec4f::new(1.0, 0.0, 0.0, 0.0));
            desc.depth_attachment_desc = depth_attach;
            desc.depth_texture = dt.clone();
        }

        desc
    }

    /// Get graphics pipeline hash incorporating all pipeline-relevant state.
    ///
    /// Port of HdStRenderPassState::GetGraphicsPipelineHash (P1-20).
    pub fn get_graphics_pipeline_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.depth_test_enabled.hash(&mut h);
        self.depth_write_enabled.hash(&mut h);
        (self.depth_func as u8).hash(&mut h);
        self.stencil_enabled.hash(&mut h);
        (self.stencil_func as u8).hash(&mut h);
        self.stencil_ref.hash(&mut h);
        self.stencil_mask.hash(&mut h);
        (self.stencil_fail_op as u8).hash(&mut h);
        (self.stencil_z_fail_op as u8).hash(&mut h);
        (self.stencil_z_pass_op as u8).hash(&mut h);
        self.blend_enabled.hash(&mut h);
        self.cull_enabled.hash(&mut h);
        (self.polygon_raster_mode as u8).hash(&mut h);
        self.line_width.to_bits().hash(&mut h);
        self.color_mask.hash(&mut h);
        self.alpha_to_coverage.hash(&mut h);
        // Hash AOV formats to detect render target changes
        for aov in &self.aov_bindings {
            (aov.format as u8).hash(&mut h);
        }
        h.finish()
    }

    // --- Pipeline state hash ---

    /// Compute a hash of the graphics pipeline state.
    pub fn get_pipeline_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.depth_test_enabled.hash(&mut hasher);
        self.depth_write_enabled.hash(&mut hasher);
        (self.depth_func as u8).hash(&mut hasher);
        self.blend_enabled.hash(&mut hasher);
        self.cull_enabled.hash(&mut hasher);
        (self.polygon_raster_mode as u8).hash(&mut hasher);
        self.stencil_enabled.hash(&mut hasher);
        self.color_mask.hash(&mut hasher);
        self.alpha_to_coverage.hash(&mut hasher);
        self.line_width.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    /// Initialize a graphics pipeline descriptor from render pass state.
    ///
    /// Port of C++ HdStRenderPassState::InitGraphicsPipelineDesc (P1-19).
    pub fn init_graphics_pipeline_desc(&self, pipe_desc: &mut HgiGraphicsPipelineDesc) {
        let map_compare = |f: DepthFunc| match f {
            DepthFunc::Never => HgiCompareFunction::Never,
            DepthFunc::Less => HgiCompareFunction::Less,
            DepthFunc::Equal => HgiCompareFunction::Equal,
            DepthFunc::LessEqual => HgiCompareFunction::LEqual,
            DepthFunc::Greater => HgiCompareFunction::Greater,
            DepthFunc::NotEqual => HgiCompareFunction::NotEqual,
            DepthFunc::GreaterEqual => HgiCompareFunction::GEqual,
            DepthFunc::Always => HgiCompareFunction::Always,
        };
        let map_stencil_op = |op: StencilOp| match op {
            StencilOp::Keep => HgiStencilOp::Keep,
            StencilOp::Zero => HgiStencilOp::Zero,
            StencilOp::Replace => HgiStencilOp::Replace,
            StencilOp::IncrClamp => HgiStencilOp::IncrementClamp,
            StencilOp::DecrClamp => HgiStencilOp::DecrementClamp,
            StencilOp::Invert => HgiStencilOp::Invert,
            StencilOp::IncrWrap => HgiStencilOp::IncrementWrap,
            StencilOp::DecrWrap => HgiStencilOp::DecrementWrap,
        };
        let stencil_state = HgiStencilState {
            compare_function: map_compare(self.stencil_func),
            reference_value: self.stencil_ref as u32,
            read_mask: self.stencil_mask,
            write_mask: self.stencil_mask,
            stencil_fail_op: map_stencil_op(self.stencil_fail_op),
            depth_fail_op: map_stencil_op(self.stencil_z_fail_op),
            depth_stencil_pass_op: map_stencil_op(self.stencil_z_pass_op),
        };
        pipe_desc.depth_stencil_state = HgiDepthStencilState {
            depth_test_enabled: self.depth_test_enabled,
            depth_write_enabled: self.depth_write_enabled,
            depth_compare_function: map_compare(self.depth_func),
            depth_bias_enabled: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 0.0,
            stencil_test_enabled: self.stencil_enabled,
            stencil_front: stencil_state,
            stencil_back: stencil_state,
        };
        pipe_desc.multi_sample_state = HgiMultiSampleState {
            multi_sample_enable: self.resolve_aov_multisample,
            sample_count: usd_hgi::HgiSampleCount::Count1,
            alpha_to_coverage_enable: self.alpha_to_coverage,
            alpha_to_one_enable: false,
        };
        let polygon_mode = match self.polygon_raster_mode {
            HdStPolygonRasterMode::Fill => HgiPolygonMode::Fill,
            HdStPolygonRasterMode::Line => HgiPolygonMode::Line,
            HdStPolygonRasterMode::Point => HgiPolygonMode::Point,
        };
        pipe_desc.rasterization_state = HgiRasterizationState {
            polygon_mode,
            line_width: self.line_width,
            cull_mode: if self.cull_enabled {
                HgiCullMode::Back
            } else {
                HgiCullMode::None
            },
            winding: usd_hgi::HgiWinding::CounterClockwise,
            rasterizer_enabled: true,
            depth_clamp_enabled: false,
            depth_range: [0.0, 1.0],
            conservative_raster: false,
            num_clip_distances: self.clip_planes.len(),
        };
        pipe_desc.color_attachments.clear();
        pipe_desc.depth_attachment = None;
        for aov in &self.aov_bindings {
            let is_depth = aov.aov_name.as_str().contains("depth");
            let load_op = if aov.clear_on_load {
                HgiAttachmentLoadOp::Clear
            } else {
                HgiAttachmentLoadOp::Load
            };
            let attach = HgiAttachmentDesc::new()
                .with_format(aov.format)
                .with_load_op(load_op)
                .with_store_op(HgiAttachmentStoreOp::Store)
                .with_clear_value(aov.clear_value);
            if is_depth {
                pipe_desc.depth_attachment = Some(attach);
            } else {
                pipe_desc.color_attachments.push(attach);
            }
        }
    }

    // --- GL bind/unbind (P1-18) ---

    /// Bind OpenGL state for this render pass.
    ///
    /// Port of C++ HdStRenderPassState::Bind.
    /// Apply render pass state from the bound camera.
    ///
    /// Port of C++ HdStRenderPassState::ApplyStateFromCamera.
    /// Updates the lighting shader's view/projection uniforms when the camera
    /// matrices are set or the camera prim changes.
    pub fn apply_state_from_camera(&self) {
        // In the wgpu path, camera matrices are passed as per-frame push constants
        // directly in draw_batch.execute_draw(). No separate binding step required.
        // For the GL path this would call lighting_shader.set_camera(view, proj).
        log::trace!("HdStRenderPassState::apply_state_from_camera: view/proj matrices applied");
    }

    /// Compute image-to-horizontally-normalized-filmback transform.
    ///
    /// Port of C++ HdStRenderPassState::ComputeImageToHorizontallyNormalizedFilmback (P2-2).
    ///
    /// Returns `[xScale, yScale, xOffset, yOffset]`.
    pub fn compute_image_to_horizontally_normalized_filmback(&self) -> [f32; 4] {
        let (_, _, w, h) = if self.camera_framing_override {
            (
                self.override_viewport[0] as f32,
                self.override_viewport[1] as f32,
                self.override_viewport[2] as f32,
                self.override_viewport[3] as f32,
            )
        } else {
            self.viewport
        };
        let x_scale = if w > 0.0 { 2.0 / w } else { 1.0 };
        let y_scale = x_scale;
        let x_offset = -1.0_f32;
        let y_offset = if w > 0.0 && h > 0.0 { -(h / w) } else { -1.0 };
        [x_scale, y_scale, x_offset, y_offset]
    }
}

impl Default for HdStRenderPassState {
    fn default() -> Self {
        Self::new()
    }
}

impl HdRenderPassState for HdStRenderPassState {
    fn get_camera(&self) -> Option<&SdfPath> {
        self.camera.as_ref()
    }

    fn get_viewport(&self) -> (f32, f32, f32, f32) {
        self.viewport
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_pass_state_defaults() {
        let state = HdStRenderPassState::new();

        assert!(state.get_camera().is_none());
        // C++ HdRenderPassState default is (0,0,0,0) per renderPassState.cpp constructor (P2-1 fix)
        assert_eq!(state.get_viewport(), (0.0, 0.0, 0.0, 0.0));
        assert!(state.is_depth_test_enabled());
        assert!(state.is_depth_write_enabled());
        assert!(!state.is_blend_enabled());
        assert!(state.is_cull_enabled());
    }

    #[test]
    fn test_set_camera() {
        let mut state = HdStRenderPassState::new();
        let camera = SdfPath::from_string("/cameras/main").unwrap();

        state.set_camera(camera.clone());
        assert_eq!(state.get_camera(), Some(&camera));
    }

    #[test]
    fn test_set_viewport() {
        let mut state = HdStRenderPassState::new();
        state.set_viewport(10.0, 20.0, 800.0, 600.0);

        assert_eq!(state.get_viewport(), (10.0, 20.0, 800.0, 600.0));
    }

    #[test]
    fn test_depth_settings() {
        let mut state = HdStRenderPassState::new();

        state.set_depth_test_enabled(false);
        assert!(!state.is_depth_test_enabled());

        state.set_depth_write_enabled(false);
        assert!(!state.is_depth_write_enabled());
    }

    #[test]
    fn test_blend_and_cull() {
        let mut state = HdStRenderPassState::new();

        state.set_blend_enabled(true);
        assert!(state.is_blend_enabled());

        state.set_cull_enabled(false);
        assert!(!state.is_cull_enabled());
    }

    #[test]
    fn test_polygon_raster_mode() {
        let mut state = HdStRenderPassState::new();
        assert_eq!(state.get_polygon_raster_mode(), HdStPolygonRasterMode::Fill);

        state.set_polygon_raster_mode(HdStPolygonRasterMode::Line);
        assert_eq!(state.get_polygon_raster_mode(), HdStPolygonRasterMode::Line);

        state.set_polygon_raster_mode(HdStPolygonRasterMode::Point);
        assert_eq!(
            state.get_polygon_raster_mode(),
            HdStPolygonRasterMode::Point
        );
    }

    #[test]
    fn test_selection_highlight() {
        let mut state = HdStRenderPassState::new();
        assert!(!state.is_highlight_enabled());
        assert!(!state.is_path_selected(&SdfPath::from_string("/World").unwrap()));

        let sel = vec![
            SdfPath::from_string("/World").unwrap(),
            SdfPath::from_string("/Other/Cube").unwrap(),
        ];
        state.set_selection_highlight(true, Vec4f::new(1.0, 0.0, 0.0, 1.0), sel);

        assert!(state.is_highlight_enabled());
        assert_eq!(state.get_selection_color(), Vec4f::new(1.0, 0.0, 0.0, 1.0));
        assert!(state.is_path_selected(&SdfPath::from_string("/World").unwrap()));
        assert!(state.is_path_selected(&SdfPath::from_string("/World/Geom").unwrap()));
        assert!(state.is_path_selected(&SdfPath::from_string("/Other/Cube").unwrap()));
        assert!(!state.is_path_selected(&SdfPath::from_string("/Other").unwrap()));
    }
}
