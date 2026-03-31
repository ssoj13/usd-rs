//! Graphics pipeline state descriptors

use super::attachment_desc::HgiAttachmentDesc;
use super::enums::*;
use super::handle::HgiHandle;
use super::shader_program::HgiShaderProgramHandle;
use super::types::HgiFormat;
use usd_gf::Vec4f;

/// Describes a vertex attribute
#[derive(Debug, Clone, PartialEq)]
pub struct HgiVertexAttributeDesc {
    /// Format of the vertex attribute
    pub format: HgiFormat,

    /// Offset in bytes from the start of the vertex
    pub offset: u32,

    /// Shader input location (layout location in GLSL)
    pub shader_binding_location: u32,
}

impl HgiVertexAttributeDesc {
    /// Creates a new vertex attribute descriptor.
    ///
    /// # Arguments
    ///
    /// * `format` - Data format of the attribute (e.g., Float32Vec3)
    /// * `offset` - Byte offset from the start of the vertex structure
    /// * `location` - Shader binding location (corresponds to layout(location=N) in GLSL)
    ///
    /// # Reference
    ///
    /// Maps to USD's `HgiVertexAttributeDesc` constructor.
    pub fn new(format: HgiFormat, offset: u32, location: u32) -> Self {
        Self {
            format,
            offset,
            shader_binding_location: location,
        }
    }
}

/// Describes a vertex buffer binding
#[derive(Debug, Clone, PartialEq)]
pub struct HgiVertexBufferDesc {
    /// Binding index for this vertex buffer
    pub binding_index: u32,

    /// Stride in bytes between consecutive vertices
    pub vertex_stride: u32,

    /// Step function (per-vertex, per-instance, etc.)
    pub step_function: HgiVertexBufferStepFunction,

    /// Vertex attributes in this buffer
    pub vertex_attributes: Vec<HgiVertexAttributeDesc>,
}

impl HgiVertexBufferDesc {
    /// Creates a new vertex buffer descriptor with default values.
    ///
    /// Default values:
    /// - binding_index: 0
    /// - vertex_stride: 0
    /// - step_function: PerVertex
    /// - vertex_attributes: empty
    ///
    /// # Reference
    ///
    /// Maps to USD's `HgiVertexBufferDesc` constructor.
    pub fn new() -> Self {
        Self {
            binding_index: 0,
            vertex_stride: 0,
            step_function: HgiVertexBufferStepFunction::PerVertex,
            vertex_attributes: Vec::new(),
        }
    }

    /// Sets the binding index for this vertex buffer.
    ///
    /// # Arguments
    ///
    /// * `index` - Binding point index (corresponds to binding=N in GLSL)
    pub fn with_binding_index(mut self, index: u32) -> Self {
        self.binding_index = index;
        self
    }

    /// Sets the stride in bytes between consecutive vertices.
    ///
    /// # Arguments
    ///
    /// * `stride` - Number of bytes between vertex data (typically sizeof(Vertex))
    pub fn with_vertex_stride(mut self, stride: u32) -> Self {
        self.vertex_stride = stride;
        self
    }

    /// Sets the step function (per-vertex or per-instance).
    ///
    /// # Arguments
    ///
    /// * `step` - Controls whether data advances per vertex or per instance
    pub fn with_step_function(mut self, step: HgiVertexBufferStepFunction) -> Self {
        self.step_function = step;
        self
    }

    /// Adds a vertex attribute to this buffer.
    ///
    /// # Arguments
    ///
    /// * `attr` - Vertex attribute descriptor to add
    pub fn with_attribute(mut self, attr: HgiVertexAttributeDesc) -> Self {
        self.vertex_attributes.push(attr);
        self
    }
}

impl Default for HgiVertexBufferDesc {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes multi-sample anti-aliasing state
#[derive(Debug, Clone, PartialEq)]
pub struct HgiMultiSampleState {
    /// When enabled and sampleCount and attachments match, use multi-sampling.
    ///
    /// Mirrors C++ `HgiMultiSampleState::multiSampleEnable`.
    pub multi_sample_enable: bool,

    /// Sample count
    pub sample_count: HgiSampleCount,

    /// Alpha to coverage enabled.
    ///
    /// Fragment's color.a determines coverage (screen door transparency).
    pub alpha_to_coverage_enable: bool,

    /// Alpha to one enabled.
    ///
    /// Fragment's color.a is replaced by the maximum representable alpha value.
    pub alpha_to_one_enable: bool,
}

impl Default for HgiMultiSampleState {
    fn default() -> Self {
        // C++: multiSampleEnable(true), alphaToCoverageEnable(false),
        // alphaToOneEnable(false), sampleCount(HgiSampleCount1)
        Self {
            multi_sample_enable: true,
            sample_count: HgiSampleCount::Count1,
            alpha_to_coverage_enable: false,
            alpha_to_one_enable: false,
        }
    }
}

/// Describes rasterization state
#[derive(Debug, Clone, PartialEq)]
pub struct HgiRasterizationState {
    /// Polygon mode
    pub polygon_mode: HgiPolygonMode,

    /// Line width for line polygon mode
    pub line_width: f32,

    /// Cull mode
    pub cull_mode: HgiCullMode,

    /// Winding order for front faces
    pub winding: HgiWinding,

    /// Whether rasterization is enabled.
    ///
    /// When false, all primitives are discarded before the rasterization stage.
    /// Mirrors C++ `HgiRasterizationState::rasterizerEnabled`.
    pub rasterizer_enabled: bool,

    /// When enabled, clamps clip space depth to the view volume rather than
    /// clipping to near/far planes.
    pub depth_clamp_enabled: bool,

    /// Depth range mapping: NDC depth values to window depth values.
    ///
    /// `[min_depth, max_depth]` — typically `[0.0, 1.0]`.
    /// Mirrors C++ `HgiRasterizationState::depthRange`.
    pub depth_range: [f32; 2],

    /// Whether conservative rasterization is enabled.
    ///
    /// When enabled, any pixel at least partially covered is rasterized.
    pub conservative_raster: bool,

    /// Number of user-defined clip distances.
    pub num_clip_distances: usize,
}

impl Default for HgiRasterizationState {
    fn default() -> Self {
        // C++: polygonMode(Fill), lineWidth(1.0), cullMode(HgiCullModeBack),
        // winding(CounterClockwise), rasterizerEnabled(true), depthClampEnabled(false),
        // depthRange(0,1), conservativeRaster(false), numClipDistances(0)
        Self {
            polygon_mode: HgiPolygonMode::Fill,
            line_width: 1.0,
            cull_mode: HgiCullMode::Back,
            winding: HgiWinding::CounterClockwise,
            rasterizer_enabled: true,
            depth_clamp_enabled: false,
            depth_range: [0.0, 1.0],
            conservative_raster: false,
            num_clip_distances: 0,
        }
    }
}

/// Describes stencil operation state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HgiStencilState {
    /// Comparison function
    pub compare_function: HgiCompareFunction,

    /// Reference value
    pub reference_value: u32,

    /// Read mask
    pub read_mask: u32,

    /// Write mask
    pub write_mask: u32,

    /// Stencil operation when stencil test fails
    pub stencil_fail_op: HgiStencilOp,

    /// Stencil operation when depth test fails
    pub depth_fail_op: HgiStencilOp,

    /// Stencil operation when both tests pass
    pub depth_stencil_pass_op: HgiStencilOp,
}

impl Default for HgiStencilState {
    fn default() -> Self {
        // C++: compareFn(Always), referenceValue(0), stencilFailOp/depthFailOp/passOp(Keep),
        // readMask(0xffffffff), writeMask(0xffffffff)
        Self {
            compare_function: HgiCompareFunction::Always,
            reference_value: 0,
            read_mask: 0xffffffff,
            write_mask: 0xffffffff,
            stencil_fail_op: HgiStencilOp::Keep,
            depth_fail_op: HgiStencilOp::Keep,
            depth_stencil_pass_op: HgiStencilOp::Keep,
        }
    }
}

/// Describes depth and stencil state
#[derive(Debug, Clone, PartialEq)]
pub struct HgiDepthStencilState {
    /// Depth test enabled
    pub depth_test_enabled: bool,

    /// Depth write enabled
    pub depth_write_enabled: bool,

    /// Depth comparison function
    pub depth_compare_function: HgiCompareFunction,

    /// Whether a depth bias is applied to depth values before the depth test.
    ///
    /// Mirrors C++ `HgiDepthStencilState::depthBiasEnabled`.
    pub depth_bias_enabled: bool,

    /// Constant depth bias added to each fragment's depth value.
    ///
    /// Mirrors C++ `HgiDepthStencilState::depthBiasConstantFactor`.
    pub depth_bias_constant_factor: f32,

    /// Depth bias that scales with the gradient of the primitive.
    ///
    /// Mirrors C++ `HgiDepthStencilState::depthBiasSlopeFactor`.
    pub depth_bias_slope_factor: f32,

    /// Stencil test enabled
    pub stencil_test_enabled: bool,

    /// Stencil state for front faces
    pub stencil_front: HgiStencilState,

    /// Stencil state for back faces
    pub stencil_back: HgiStencilState,
}

impl Default for HgiDepthStencilState {
    fn default() -> Self {
        Self {
            depth_test_enabled: true,
            depth_write_enabled: true,
            depth_compare_function: HgiCompareFunction::Less,
            depth_bias_enabled: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 0.0,
            stencil_test_enabled: false,
            stencil_front: HgiStencilState::default(),
            stencil_back: HgiStencilState::default(),
        }
    }
}

/// Describes color blend state for an attachment
#[derive(Debug, Clone, PartialEq)]
pub struct HgiColorBlendState {
    /// Blending enabled
    pub blend_enabled: bool,

    /// Color blend operation
    pub color_blend_op: HgiBlendOp,

    /// Source color blend factor
    pub src_color_blend_factor: HgiBlendFactor,

    /// Destination color blend factor
    pub dst_color_blend_factor: HgiBlendFactor,

    /// Alpha blend operation
    pub alpha_blend_op: HgiBlendOp,

    /// Source alpha blend factor
    pub src_alpha_blend_factor: HgiBlendFactor,

    /// Destination alpha blend factor
    pub dst_alpha_blend_factor: HgiBlendFactor,

    /// Color write mask
    pub color_mask: HgiColorMask,
}

impl Default for HgiColorBlendState {
    fn default() -> Self {
        Self {
            blend_enabled: false,
            color_blend_op: HgiBlendOp::Add,
            src_color_blend_factor: HgiBlendFactor::One,
            dst_color_blend_factor: HgiBlendFactor::Zero,
            alpha_blend_op: HgiBlendOp::Add,
            src_alpha_blend_factor: HgiBlendFactor::One,
            dst_alpha_blend_factor: HgiBlendFactor::Zero,
            color_mask: HgiColorMask::RED
                | HgiColorMask::GREEN
                | HgiColorMask::BLUE
                | HgiColorMask::ALPHA,
        }
    }
}

/// Describes the shader push/function constants block for a graphics pipeline.
///
/// A small, fast uniform buffer accessible in shaders without binding overhead.
/// Mirrors C++ `HgiGraphicsShaderConstantsDesc`.
#[derive(Debug, Clone, PartialEq)]
pub struct HgiGraphicsShaderConstantsDesc {
    /// Size of the constants in bytes (max 256 bytes)
    pub byte_size: u32,
    /// Which shader stage(s) the constants are used in
    pub stage_usage: HgiShaderStage,
}

impl Default for HgiGraphicsShaderConstantsDesc {
    fn default() -> Self {
        // C++: byteSize(0), stageUsage(HgiShaderStageFragment)
        Self {
            byte_size: 0,
            stage_usage: HgiShaderStage::FRAGMENT,
        }
    }
}

/// Patch type for tessellation state in a graphics pipeline.
///
/// Mirrors C++ `HgiTessellationState::PatchType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiTessellationPatchType {
    Triangle,
    Quad,
    Isoline,
}

/// Tess factor source mode for Metal tessellation.
///
/// Mirrors C++ `HgiTessellationState::TessFactorMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HgiTessFactorMode {
    Constant,
    TessControl,
    TessVertex,
}

/// Fallback tessellation levels (inner and outer).
///
/// Mirrors C++ `HgiTessellationLevel`.
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTessellationLevel {
    /// Inner tessellation levels (2 values for triangle/quad)
    pub inner_tess_level: [f32; 2],
    /// Outer tessellation levels (4 values)
    pub outer_tess_level: [f32; 4],
}

impl Default for HgiTessellationLevel {
    fn default() -> Self {
        // C++: innerTessLevel{0, 0}, outerTessLevel{0, 0, 0, 0}
        Self {
            inner_tess_level: [0.0, 0.0],
            outer_tess_level: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Describes tessellation state for a graphics pipeline.
///
/// Mirrors C++ `HgiTessellationState`.
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTessellationState {
    /// Type of tessellation patch
    pub patch_type: HgiTessellationPatchType,
    /// Number of control indices per patch
    pub primitive_index_size: i32,
    /// How tessellation factors are provided (constant, from control shader, or vertex)
    pub tess_factor_mode: HgiTessFactorMode,
    /// Fallback tessellation levels when not driven by a shader
    pub tessellation_level: HgiTessellationLevel,
}

impl Default for HgiTessellationState {
    fn default() -> Self {
        // C++: patchType(Triangle), primitiveIndexSize(0), tessFactorMode(Constant)
        Self {
            patch_type: HgiTessellationPatchType::Triangle,
            primitive_index_size: 0,
            tess_factor_mode: HgiTessFactorMode::Constant,
            tessellation_level: HgiTessellationLevel::default(),
        }
    }
}

/// Describes a complete graphics pipeline state
#[derive(Debug, Clone)]
pub struct HgiGraphicsPipelineDesc {
    /// Debug label
    pub debug_name: String,

    /// Shader program
    pub shader_program: HgiShaderProgramHandle,

    /// Vertex buffer descriptors
    pub vertex_buffers: Vec<HgiVertexBufferDesc>,

    /// Color attachment descriptions
    pub color_attachments: Vec<HgiAttachmentDesc>,

    /// Depth attachment description
    pub depth_attachment: Option<HgiAttachmentDesc>,

    /// Multi-sample state
    pub multi_sample_state: HgiMultiSampleState,

    /// Rasterization state
    pub rasterization_state: HgiRasterizationState,

    /// Depth/stencil state
    pub depth_stencil_state: HgiDepthStencilState,

    /// Color blend state per attachment
    pub color_blend_states: Vec<HgiColorBlendState>,

    /// Blend constant color
    pub blend_constant_color: Vec4f,

    /// Primitive type
    pub primitive_type: HgiPrimitiveType,

    /// Shader push/function constants descriptor.
    /// Mirrors C++ `HgiGraphicsPipelineDesc::shaderConstantsDesc`.
    pub shader_constants_desc: HgiGraphicsShaderConstantsDesc,

    /// Whether to resolve MSAA color/depth attachments at end of pass.
    /// Mirrors C++ `HgiGraphicsPipelineDesc::resolveAttachments`.
    pub resolve_attachments: bool,

    /// Tessellation state (patch type, levels, factor mode).
    /// Mirrors C++ `HgiGraphicsPipelineDesc::tessellationState`.
    pub tessellation_state: HgiTessellationState,
}

impl Default for HgiGraphicsPipelineDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            shader_program: HgiHandle::null(),
            vertex_buffers: Vec::new(),
            color_attachments: Vec::new(),
            depth_attachment: None,
            multi_sample_state: HgiMultiSampleState::default(),
            rasterization_state: HgiRasterizationState::default(),
            depth_stencil_state: HgiDepthStencilState::default(),
            color_blend_states: Vec::new(),
            blend_constant_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            primitive_type: HgiPrimitiveType::TriangleList,
            shader_constants_desc: HgiGraphicsShaderConstantsDesc::default(),
            resolve_attachments: false,
            tessellation_state: HgiTessellationState::default(),
        }
    }
}

impl HgiGraphicsPipelineDesc {
    /// Creates a new graphics pipeline descriptor with default values.
    ///
    /// # Reference
    ///
    /// Maps to USD's `HgiGraphicsPipelineDesc` constructor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the debug label for this pipeline.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for debugging and profiling
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Sets the shader program for this pipeline.
    ///
    /// # Arguments
    ///
    /// * `program` - Handle to a compiled and linked shader program
    pub fn with_shader_program(mut self, program: HgiShaderProgramHandle) -> Self {
        self.shader_program = program;
        self
    }

    /// Sets the primitive topology type.
    ///
    /// # Arguments
    ///
    /// * `prim_type` - Primitive type (triangles, lines, points, etc.)
    pub fn with_primitive_type(mut self, prim_type: HgiPrimitiveType) -> Self {
        self.primitive_type = prim_type;
        self
    }
}

/// GPU graphics pipeline state object (abstract interface)
pub trait HgiGraphicsPipeline: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor
    fn descriptor(&self) -> &HgiGraphicsPipelineDesc;

    /// Returns the backend's raw GPU resource handle
    fn raw_resource(&self) -> u64;
}

/// Type alias for graphics pipeline handle
pub type HgiGraphicsPipelineHandle = HgiHandle<dyn HgiGraphicsPipeline>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_attribute() {
        let attr = HgiVertexAttributeDesc::new(HgiFormat::Float32Vec3, 0, 0);
        assert_eq!(attr.format, HgiFormat::Float32Vec3);
        assert_eq!(attr.offset, 0);
        assert_eq!(attr.shader_binding_location, 0);
    }

    #[test]
    fn test_vertex_buffer_desc() {
        let desc = HgiVertexBufferDesc::new()
            .with_binding_index(0)
            .with_vertex_stride(32)
            .with_step_function(HgiVertexBufferStepFunction::PerVertex)
            .with_attribute(HgiVertexAttributeDesc::new(HgiFormat::Float32Vec3, 0, 0));

        assert_eq!(desc.binding_index, 0);
        assert_eq!(desc.vertex_stride, 32);
        assert_eq!(desc.vertex_attributes.len(), 1);
    }

    #[test]
    fn test_pipeline_desc() {
        let desc = HgiGraphicsPipelineDesc::new()
            .with_debug_name("MyPipeline")
            .with_primitive_type(HgiPrimitiveType::TriangleList);

        assert_eq!(desc.debug_name, "MyPipeline");
        assert_eq!(desc.primitive_type, HgiPrimitiveType::TriangleList);
    }

    /// Verify defaults match C++ constructors exactly
    #[test]
    fn test_multi_sample_state_default() {
        let s = HgiMultiSampleState::default();
        // C++: multiSampleEnable(true)
        assert!(s.multi_sample_enable, "C++ default is true");
        assert!(!s.alpha_to_coverage_enable);
        assert!(!s.alpha_to_one_enable);
        assert_eq!(s.sample_count, HgiSampleCount::Count1);
    }

    #[test]
    fn test_rasterization_state_default() {
        let s = HgiRasterizationState::default();
        assert_eq!(s.polygon_mode, HgiPolygonMode::Fill);
        assert_eq!(s.line_width, 1.0);
        // C++: cullMode(HgiCullModeBack)
        assert_eq!(
            s.cull_mode,
            HgiCullMode::Back,
            "C++ default is Back, not None"
        );
        assert_eq!(s.winding, HgiWinding::CounterClockwise);
        assert!(s.rasterizer_enabled);
        assert!(!s.depth_clamp_enabled);
        assert_eq!(s.depth_range, [0.0, 1.0]);
        assert!(!s.conservative_raster);
        assert_eq!(s.num_clip_distances, 0);
    }

    #[test]
    fn test_stencil_state_default() {
        let s = HgiStencilState::default();
        assert_eq!(s.compare_function, HgiCompareFunction::Always);
        assert_eq!(s.reference_value, 0);
        // C++: 0xffffffff (not 0xFF)
        assert_eq!(s.read_mask, 0xffffffff, "C++ default is 0xffffffff");
        assert_eq!(s.write_mask, 0xffffffff, "C++ default is 0xffffffff");
        assert_eq!(s.stencil_fail_op, HgiStencilOp::Keep);
        assert_eq!(s.depth_fail_op, HgiStencilOp::Keep);
        assert_eq!(s.depth_stencil_pass_op, HgiStencilOp::Keep);
    }

    #[test]
    fn test_depth_stencil_state_default() {
        let s = HgiDepthStencilState::default();
        assert!(s.depth_test_enabled);
        assert!(s.depth_write_enabled);
        assert_eq!(s.depth_compare_function, HgiCompareFunction::Less);
        assert!(!s.depth_bias_enabled);
        assert_eq!(s.depth_bias_constant_factor, 0.0);
        assert_eq!(s.depth_bias_slope_factor, 0.0);
        assert!(!s.stencil_test_enabled);
    }

    #[test]
    fn test_shader_constants_desc_default() {
        let s = HgiGraphicsShaderConstantsDesc::default();
        assert_eq!(s.byte_size, 0);
        // C++: stageUsage(HgiShaderStageFragment)
        assert_eq!(
            s.stage_usage,
            HgiShaderStage::FRAGMENT,
            "C++ default is Fragment"
        );
    }

    #[test]
    fn test_tessellation_level_default() {
        let t = HgiTessellationLevel::default();
        // C++: {0, 0} / {0, 0, 0, 0}
        assert_eq!(t.inner_tess_level, [0.0, 0.0], "C++ default is 0.0");
        assert_eq!(
            t.outer_tess_level,
            [0.0, 0.0, 0.0, 0.0],
            "C++ default is 0.0"
        );
    }

    #[test]
    fn test_tessellation_state_default() {
        let t = HgiTessellationState::default();
        assert_eq!(t.patch_type, HgiTessellationPatchType::Triangle);
        // C++: primitiveIndexSize(0)
        assert_eq!(t.primitive_index_size, 0, "C++ default is 0, not 3");
        assert_eq!(t.tess_factor_mode, HgiTessFactorMode::Constant);
    }
}
