//! OpenGL graphics pipeline implementation for HGI
//!
//! This module provides the OpenGL backend implementation of graphics pipelines.
//! Unlike modern APIs (Vulkan/Metal/DX12), OpenGL doesn't have monolithic pipeline
//! state objects. Instead, this implementation caches all pipeline state and applies
//! it via individual GL state calls.

use super::conversions::*;
use usd_hgi::*;

/// OpenGL graphics pipeline state
///
/// In OpenGL, graphics pipeline state is not a single object but rather
/// a collection of state that must be set before drawing. This struct
/// caches the state for efficient binding.
#[derive(Debug)]
pub struct HgiGLGraphicsPipeline {
    /// Pipeline descriptor
    desc: HgiGraphicsPipelineDesc,

    /// Vertex Array Object (VAO) for vertex input state
    vao: u32,

    /// Cached GL state for efficient binding
    cached_state: CachedGraphicsState,
}

/// Cached OpenGL graphics state for fast binding
#[derive(Debug, Clone)]
pub struct CachedGraphicsState {
    /// Cull mode (None if culling disabled)
    pub cull_mode: Option<u32>,
    /// Front face winding
    pub front_face: u32,
    /// Polygon mode
    pub polygon_mode: u32,
    /// Depth test enable
    pub depth_test_enabled: bool,
    /// Depth write enable
    pub depth_write_enabled: bool,
    /// Depth compare function
    pub depth_func: u32,
    /// Blend enable per attachment
    pub blend_enabled: Vec<bool>,
    /// Blend equations and factors
    pub blend_state: Vec<BlendState>,
}

/// Blend state for a single render target attachment
#[derive(Debug, Clone)]
pub struct BlendState {
    /// Color blend equation
    pub color_op: u32,
    /// Alpha blend equation
    pub alpha_op: u32,
    /// Source color blend factor
    pub src_color_factor: u32,
    /// Destination color blend factor
    pub dst_color_factor: u32,
    /// Source alpha blend factor
    pub src_alpha_factor: u32,
    /// Destination alpha blend factor
    pub dst_alpha_factor: u32,
}

impl HgiGLGraphicsPipeline {
    /// Create a new OpenGL graphics pipeline
    pub fn new(desc: &HgiGraphicsPipelineDesc) -> Self {
        let vao = Self::create_vao(desc);
        let cached_state = Self::build_cached_state(desc);

        Self {
            desc: desc.clone(),
            vao,
            cached_state,
        }
    }

    /// Create Vertex Array Object from vertex buffer layout
    #[cfg(feature = "opengl")]
    fn create_vao(desc: &HgiGraphicsPipelineDesc) -> u32 {
        use gl::types::*;

        unsafe {
            let mut vao: GLuint = 0;
            gl::CreateVertexArrays(1, &mut vao);

            if vao == 0 {
                log::error!("Failed to create VAO");
                return 0;
            }

            // Configure vertex attributes from descriptor
            for vb_desc in &desc.vertex_buffers {
                for attr in &vb_desc.vertex_attributes {
                    let location = attr.shader_binding_location as GLuint;

                    // Enable vertex attribute
                    gl::EnableVertexArrayAttrib(vao, location);

                    // Set attribute format based on component count and type
                    let (components, gl_type, normalized) = hgi_vertex_format_to_gl(attr.format);

                    gl::VertexArrayAttribFormat(
                        vao,
                        location,
                        components,
                        gl_type,
                        normalized,
                        attr.offset as GLuint,
                    );

                    // Bind attribute to vertex buffer binding
                    gl::VertexArrayAttribBinding(vao, location, vb_desc.binding_index as GLuint);
                }

                // Set binding divisor for instancing
                if vb_desc.step_function == HgiVertexBufferStepFunction::PerInstance {
                    gl::VertexArrayBindingDivisor(vao, vb_desc.binding_index as GLuint, 1);
                }
            }

            // Set debug label if provided
            if !desc.debug_name.is_empty() {
                gl::ObjectLabel(
                    gl::VERTEX_ARRAY,
                    vao,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }

            vao
        }
    }

    /// Create Vertex Array Object (stub when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    fn create_vao(_desc: &HgiGraphicsPipelineDesc) -> u32 {
        0
    }

    /// Build cached state from descriptor
    fn build_cached_state(desc: &HgiGraphicsPipelineDesc) -> CachedGraphicsState {
        let raster = &desc.rasterization_state;
        let depth = &desc.depth_stencil_state;

        // Convert rasterization state
        let cull_mode = hgi_cull_mode_to_gl(raster.cull_mode);
        let front_face = hgi_winding_to_gl(raster.winding);
        let polygon_mode = hgi_polygon_mode_to_gl(raster.polygon_mode);

        // Convert depth state
        let depth_func = hgi_compare_func_to_gl(depth.depth_compare_function);

        // Convert blend state per color attachment from desc.color_blend_states
        let mut blend_enabled = Vec::new();
        let mut blend_state = Vec::new();

        if desc.color_blend_states.is_empty() {
            // Fallback: single disabled blend attachment
            blend_enabled.push(false);
            blend_state.push(BlendState {
                color_op: hgi_blend_op_to_gl(HgiBlendOp::Add),
                alpha_op: hgi_blend_op_to_gl(HgiBlendOp::Add),
                src_color_factor: hgi_blend_factor_to_gl(HgiBlendFactor::One),
                dst_color_factor: hgi_blend_factor_to_gl(HgiBlendFactor::Zero),
                src_alpha_factor: hgi_blend_factor_to_gl(HgiBlendFactor::One),
                dst_alpha_factor: hgi_blend_factor_to_gl(HgiBlendFactor::Zero),
            });
        } else {
            for cbs in &desc.color_blend_states {
                blend_enabled.push(cbs.blend_enabled);
                blend_state.push(BlendState {
                    color_op: hgi_blend_op_to_gl(cbs.color_blend_op),
                    alpha_op: hgi_blend_op_to_gl(cbs.alpha_blend_op),
                    src_color_factor: hgi_blend_factor_to_gl(cbs.src_color_blend_factor),
                    dst_color_factor: hgi_blend_factor_to_gl(cbs.dst_color_blend_factor),
                    src_alpha_factor: hgi_blend_factor_to_gl(cbs.src_alpha_blend_factor),
                    dst_alpha_factor: hgi_blend_factor_to_gl(cbs.dst_alpha_blend_factor),
                });
            }
        }

        CachedGraphicsState {
            cull_mode,
            front_face,
            polygon_mode,
            depth_test_enabled: depth.depth_test_enabled,
            depth_write_enabled: depth.depth_write_enabled,
            depth_func,
            blend_enabled,
            blend_state,
        }
    }

    /// Get the OpenGL Vertex Array Object handle
    pub fn vao(&self) -> u32 {
        self.vao
    }

    /// Get the pipeline descriptor
    pub fn descriptor(&self) -> &HgiGraphicsPipelineDesc {
        &self.desc
    }

    /// Get cached graphics state
    pub fn cached_state(&self) -> &CachedGraphicsState {
        &self.cached_state
    }
}

impl HgiGraphicsPipeline for HgiGLGraphicsPipeline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiGraphicsPipelineDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        self.vao as u64
    }
}

impl Drop for HgiGLGraphicsPipeline {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.vao != 0 {
            unsafe {
                gl::DeleteVertexArrays(1, &self.vao);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// Bind graphics pipeline state to the current OpenGL context
#[cfg(feature = "opengl")]
pub fn bind_graphics_pipeline(pipeline: &HgiGLGraphicsPipeline) {
    let state = pipeline.cached_state();

    unsafe {
        // Bind VAO
        if pipeline.vao() != 0 {
            gl::BindVertexArray(pipeline.vao());
        }

        // Set rasterization state
        if let Some(cull) = state.cull_mode {
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(cull);
        } else {
            gl::Disable(gl::CULL_FACE);
        }
        gl::FrontFace(state.front_face);
        gl::PolygonMode(gl::FRONT_AND_BACK, state.polygon_mode);

        // Set depth state
        if state.depth_test_enabled {
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(state.depth_func);
        } else {
            gl::Disable(gl::DEPTH_TEST);
        }
        gl::DepthMask(state.depth_write_enabled as u8);

        // Set per-attachment blend state using indexed GL functions
        for (i, (enabled, blend)) in state
            .blend_enabled
            .iter()
            .zip(state.blend_state.iter())
            .enumerate()
        {
            let idx = i as u32;
            if *enabled {
                gl::Enablei(gl::BLEND, idx);
                gl::BlendEquationSeparatei(idx, blend.color_op, blend.alpha_op);
                gl::BlendFuncSeparatei(
                    idx,
                    blend.src_color_factor,
                    blend.dst_color_factor,
                    blend.src_alpha_factor,
                    blend.dst_alpha_factor,
                );
            } else {
                gl::Disablei(gl::BLEND, idx);
            }
        }
    }
}

/// Bind graphics pipeline state (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_graphics_pipeline(_pipeline: &HgiGLGraphicsPipeline) {}

/// Convert HGI vertex format to GL components, type, and normalized flag
#[cfg(feature = "opengl")]
fn hgi_vertex_format_to_gl(format: HgiFormat) -> (i32, u32, u8) {
    match format {
        HgiFormat::Float32 => (1, gl::FLOAT, gl::FALSE),
        HgiFormat::Float32Vec2 => (2, gl::FLOAT, gl::FALSE),
        HgiFormat::Float32Vec3 => (3, gl::FLOAT, gl::FALSE),
        HgiFormat::Float32Vec4 => (4, gl::FLOAT, gl::FALSE),
        HgiFormat::Int32 => (1, gl::INT, gl::FALSE),
        HgiFormat::Int32Vec2 => (2, gl::INT, gl::FALSE),
        HgiFormat::Int32Vec3 => (3, gl::INT, gl::FALSE),
        HgiFormat::Int32Vec4 => (4, gl::INT, gl::FALSE),
        HgiFormat::UNorm8Vec4 => (4, gl::UNSIGNED_BYTE, gl::TRUE),
        HgiFormat::SNorm8Vec4 => (4, gl::BYTE, gl::TRUE),
        _ => (4, gl::FLOAT, gl::FALSE), // fallback
    }
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiGraphicsPipelineDesc::new().with_debug_name("TestPipeline".to_string());

    let pipeline = HgiGLGraphicsPipeline::new(&desc);
    assert_eq!(pipeline.descriptor().debug_name, "TestPipeline");

    let mut desc = HgiGraphicsPipelineDesc::new();
    desc.rasterization_state.cull_mode = HgiCullMode::Back;
    desc.rasterization_state.winding = HgiWinding::CounterClockwise;
    desc.depth_stencil_state.depth_test_enabled = true;

    let pipeline = HgiGLGraphicsPipeline::new(&desc);
    let state = pipeline.cached_state();

    assert!(state.cull_mode.is_some());
    assert!(state.depth_test_enabled);
}
