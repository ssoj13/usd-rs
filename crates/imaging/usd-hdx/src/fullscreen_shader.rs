#![allow(dead_code)]

//! Fullscreen shader helper for Hydra extensions.
//!
//! Renders a single fullscreen triangle covering the entire viewport.
//! Used by color correction, present, and other post-process tasks.
//!
//! Triangle geometry (from C++ source):
//!   Vertex layout: [x, y, z, w, u, v]  (position vec4 + uv vec2)
//!   V0: (-1,  3, 0, 1) uv(0, 2)  -- top-left, outside NDC
//!   V1: (-1, -1, 0, 1) uv(0, 0)  -- bottom-left
//!   V2: ( 3, -1, 0, 1) uv(2, 0)  -- bottom-right, outside NDC
//!
//! This covers the entire [-1,1]^2 NDC square with a single triangle,
//! avoiding the diagonal overdraw of a quad approach.
//! The clipped portions (outside NDC) are discarded before rasterization.
//!
//! Port of pxr/imaging/hdx/fullscreenShader.h/cpp

use std::sync::{Arc, LazyLock};
use parking_lot::RwLock;
use usd_gf::{Vec4f, Vec4i};
use usd_hgi::Hgi;
use usd_hgi::{
    HgiAttachmentDesc, HgiAttachmentLoadOp, HgiAttachmentStoreOp, HgiBlendFactor, HgiBlendOp,
    HgiBufferHandle, HgiDepthStencilState, HgiSamplerHandle, HgiShaderFunctionDesc,
    HgiTextureHandle,
};
use usd_tf::Token;

use super::effects_shader::HdxEffectsShader;

/// Token for fullscreen vertex shader path.
pub static FULLSCREEN_VERTEX_SHADER: LazyLock<Token> =
    LazyLock::new(|| Token::new("hdx/shaders/fullscreen.glslfx"));

/// Token for fullscreen vertex shader technique.
pub static FULLSCREEN_VERTEX_TECHNIQUE: LazyLock<Token> =
    LazyLock::new(|| Token::new("FullScreenVertex"));

/// Fullscreen triangle vertex data (6 floats per vertex: xyzw + uv).
///
/// Matches C++ HdxFullscreenShader::_CreateBufferResources exactly.
/// Maps texture space [0,1]^2 to clip space XY [-1,1]^2.
pub const FULLSCREEN_TRIANGLE_VERTICES: [f32; 18] = [
    //  x    y   z   w     u    v
    -1.0, 3.0, 0.0, 1.0, 0.0, 2.0, // V0: top-left (clipped)
    -1.0, -1.0, 0.0, 1.0, 0.0, 0.0, // V1: bottom-left
    3.0, -1.0, 0.0, 1.0, 2.0, 0.0, // V2: bottom-right (clipped)
];

/// Index buffer for fullscreen triangle (trivial: 0, 1, 2).
pub const FULLSCREEN_TRIANGLE_INDICES: [u32; 3] = [0, 1, 2];

/// Vertex stride in bytes (6 floats * 4 bytes = 24).
pub const FULLSCREEN_VERTEX_STRIDE: u32 = 6 * std::mem::size_of::<f32>() as u32;

/// Blend state parameters for a color attachment (stored separately from HgiAttachmentDesc).
#[derive(Debug, Clone)]
pub struct HgiBlendState {
    pub enabled: bool,
    pub src_color: HgiBlendFactor,
    pub dst_color: HgiBlendFactor,
    pub color_op: HgiBlendOp,
    pub src_alpha: HgiBlendFactor,
    pub dst_alpha: HgiBlendFactor,
    pub alpha_op: HgiBlendOp,
}

impl Default for HgiBlendState {
    fn default() -> Self {
        Self {
            enabled: false,
            src_color: HgiBlendFactor::One,
            dst_color: HgiBlendFactor::Zero,
            color_op: HgiBlendOp::Add,
            src_alpha: HgiBlendFactor::One,
            dst_alpha: HgiBlendFactor::Zero,
            alpha_op: HgiBlendOp::Add,
        }
    }
}

/// Fullscreen shader utility.
///
/// Renders a fullscreen triangle with a customizable fragment shader.
/// The vertex shader always uses the hardcoded fullscreen triangle geometry;
/// only the fragment shader varies per effect.
///
/// Port of HdxFullscreenShader from pxr/imaging/hdx/fullscreenShader.h
pub struct HdxFullscreenShader {
    /// Base effects shader (manages pipeline, commands, resources)
    base: HdxEffectsShader,

    /// Bound textures (caller-managed lifetime)
    textures: Vec<HgiTextureHandle>,

    /// Texture samplers (one per texture; empty = use default)
    samplers: Vec<HgiSamplerHandle>,

    /// Bound buffers (caller-managed lifetime)
    buffers: Vec<HgiBufferHandle>,

    /// GLSLFX file path for fragment shader
    glslfx_path: Token,

    /// Fragment shader technique name
    shader_name: Token,

    /// Index buffer for the fullscreen triangle
    index_buffer: Option<HgiBufferHandle>,

    /// Vertex buffer containing fullscreen triangle geometry
    vertex_buffer: Option<HgiBufferHandle>,

    /// Compiled shader program
    shader_program: Option<usd_hgi::HgiShaderProgramHandle>,

    /// Default linear-clamp sampler (created on demand)
    default_sampler: Option<HgiSamplerHandle>,

    /// Depth stencil state
    depth_stencil_state: HgiDepthStencilState,

    /// Color attachment descriptor
    color_attachment: HgiAttachmentDesc,

    /// Depth attachment descriptor
    depth_attachment: HgiAttachmentDesc,

    /// Blend state (separate from attachment desc — stored here for pipeline setup)
    blend_state: HgiBlendState,

    /// Whether vertex/index buffers have been created
    buffers_created: bool,

    /// Stored fragment shader descriptor for deferred compilation.
    frag_desc: Option<HgiShaderFunctionDesc>,
}

impl HdxFullscreenShader {
    /// Create a new fullscreen shader object.
    ///
    /// * `hgi` - Hgi instance for GPU resource management
    /// * `debug_name` - Debug label shown in GPU debuggers
    pub fn new(hgi: Arc<RwLock<dyn Hgi>>, debug_name: String) -> Self {
        let base = HdxEffectsShader::new(hgi, debug_name);

        // C++ constructor defaults:
        //   depthTestEnabled = true, depthCompareFn = Always
        //   stencilTestEnabled = false
        //   colorAttachment: DontCare load, Store store
        //   depthAttachment: DontCare load, Store store
        let mut depth_stencil_state = HgiDepthStencilState::default();
        depth_stencil_state.depth_test_enabled = true;
        depth_stencil_state.stencil_test_enabled = false;

        let color_attachment = HgiAttachmentDesc {
            load_op: HgiAttachmentLoadOp::DontCare,
            store_op: HgiAttachmentStoreOp::Store,
            ..Default::default()
        };
        let depth_attachment = HgiAttachmentDesc {
            load_op: HgiAttachmentLoadOp::DontCare,
            store_op: HgiAttachmentStoreOp::Store,
            ..Default::default()
        };

        Self {
            base,
            textures: Vec::new(),
            samplers: Vec::new(),
            buffers: Vec::new(),
            glslfx_path: Token::new(""),
            shader_name: Token::new(""),
            index_buffer: None,
            vertex_buffer: None,
            shader_program: None,
            default_sampler: None,
            depth_stencil_state,
            color_attachment,
            depth_attachment,
            blend_state: HgiBlendState::default(),
            buffers_created: false,
            frag_desc: None,
        }
    }

    /// Set the fragment shader program (via GLSLFX file + technique name).
    ///
    /// Stores the fragment descriptor for deferred compilation when Hgi is available.
    /// Invalidates any previously compiled program so it is rebuilt on next draw.
    pub fn set_program(
        &mut self,
        glslfx_path: Token,
        shader_name: Token,
        frag_desc: &HgiShaderFunctionDesc,
    ) {
        if self.glslfx_path == glslfx_path && self.shader_name == shader_name {
            return;
        }
        self.glslfx_path = glslfx_path;
        self.shader_name = shader_name;
        // Store descriptor; invalidate compiled program so it is rebuilt next draw.
        self.frag_desc = Some(frag_desc.clone());
        self.shader_program = None;
    }

    /// Set the fragment shader program directly (bypasses GLSLFX cache).
    ///
    /// Stores the descriptor for deferred compilation; invalidates the compiled program.
    pub fn set_program_bypass(&mut self, frag_desc: &HgiShaderFunctionDesc) {
        self.frag_desc = Some(frag_desc.clone());
        self.shader_program = None;
    }

    /// Get the stored fragment shader descriptor (if any).
    pub fn get_frag_desc(&self) -> Option<&HgiShaderFunctionDesc> {
        self.frag_desc.as_ref()
    }

    /// Bind externally managed buffers (caller owns lifetime).
    pub fn bind_buffers(&mut self, buffers: Vec<HgiBufferHandle>) {
        self.buffers = buffers;
    }

    /// Bind externally managed textures (caller owns lifetime).
    pub fn bind_textures(
        &mut self,
        textures: Vec<HgiTextureHandle>,
        samplers: Option<Vec<HgiSamplerHandle>>,
    ) {
        self.textures = textures;
        self.samplers = match samplers {
            Some(s) => s,
            None => vec![HgiSamplerHandle::null(); self.textures.len()],
        };
    }

    /// Override depth/stencil state.
    ///
    /// Default: depth test with compare=Always (all pixels pass), no stencil.
    /// When non-trivial depth compare is active, loadOp is forced to Load.
    pub fn set_depth_state(&mut self, state: HgiDepthStencilState) {
        self.depth_stencil_state = state.clone();
        // Non-trivial depth test needs to read existing depth buffer
        if self.depth_stencil_state.depth_test_enabled {
            self.depth_attachment.load_op = HgiAttachmentLoadOp::Load;
        }
        self.base.set_depth_stencil_state(state);
    }

    /// Override blend state.
    ///
    /// Default: no blending (opaque). When blending is enabled, loadOp -> Load
    /// so the shader can blend with existing color buffer contents.
    pub fn set_blend_state(
        &mut self,
        enable_blending: bool,
        src_color_blend_factor: HgiBlendFactor,
        dst_color_blend_factor: HgiBlendFactor,
        color_blend_op: HgiBlendOp,
        src_alpha_blend_factor: HgiBlendFactor,
        dst_alpha_blend_factor: HgiBlendFactor,
        alpha_blend_op: HgiBlendOp,
    ) {
        self.blend_state = HgiBlendState {
            enabled: enable_blending,
            src_color: src_color_blend_factor,
            dst_color: dst_color_blend_factor,
            color_op: color_blend_op,
            src_alpha: src_alpha_blend_factor,
            dst_alpha: dst_alpha_blend_factor,
            alpha_op: alpha_blend_op,
        };
        self.color_attachment.blend_enabled = enable_blending;

        // Blending reads existing color -> must load
        if enable_blending {
            self.color_attachment.load_op = HgiAttachmentLoadOp::Load;
        }
    }

    /// Request color/depth clear on next draw.
    ///
    /// Sets loadOp to Clear; use for background/clear passes.
    pub fn set_clear_state(&mut self, clear_color: Vec4f, clear_depth: Vec4f) {
        self.color_attachment.clear_value = clear_color;
        self.color_attachment.load_op = HgiAttachmentLoadOp::Clear;

        self.depth_attachment.clear_value = clear_depth;
        self.depth_attachment.load_op = HgiAttachmentLoadOp::Clear;
    }

    /// Provide shader constant values (push constants / uniform data).
    pub fn set_shader_constants(&mut self, byte_size: u32, data: &[u8]) {
        self.base
            .set_shader_constants(byte_size, data, usd_hgi::HgiShaderStage::FRAGMENT);
    }

    /// Draw to destination textures.
    ///
    /// Convenience wrapper; pass `HgiTextureHandle::null()` for depth if unused.
    pub fn draw(&mut self, color_dst: &HgiTextureHandle, depth_dst: &HgiTextureHandle) {
        let viewport = Vec4i::new(0, 0, 0, 0); // Full impl: read from texture dimensions
        self.draw_full(
            color_dst,
            &HgiTextureHandle::null(),
            depth_dst,
            &HgiTextureHandle::null(),
            &viewport,
        );
    }

    /// Draw to destination textures with optional MSAA resolve and explicit viewport.
    ///
    /// This is the primary entry point for fullscreen passes.
    pub fn draw_full(
        &mut self,
        color_dst: &HgiTextureHandle,
        color_resolve_dst: &HgiTextureHandle,
        depth_dst: &HgiTextureHandle,
        depth_resolve_dst: &HgiTextureHandle,
        viewport: &Vec4i,
    ) {
        self.create_buffer_resources();
        self.set_resource_bindings();
        self.set_vertex_buffer_descriptor();

        let color_textures = vec![color_dst.clone()];
        let color_resolve = if !color_resolve_dst.is_null() {
            vec![color_resolve_dst.clone()]
        } else {
            Vec::new()
        };

        self.base.create_and_submit_graphics_cmds(
            &color_textures,
            &color_resolve,
            depth_dst,
            depth_resolve_dst,
            viewport,
        );
    }

    /// WGSL snippet for vertex-index-based fullscreen triangle (no VBO needed).
    ///
    /// WGSL backends can embed this directly instead of uploading a vertex buffer.
    pub fn wgsl_vertex_index_snippet() -> &'static str {
        r#"
// Fullscreen triangle via vertex_index (no VBO required)
var positions = array<vec2f, 3>(
    vec2f(-1.0,  3.0),
    vec2f(-1.0, -1.0),
    vec2f( 3.0, -1.0),
);
var uvs = array<vec2f, 3>(
    vec2f(0.0, 2.0),
    vec2f(0.0, 0.0),
    vec2f(2.0, 0.0),
);
let pos = positions[vertex_index];
let uv  = uvs[vertex_index];
"#
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Create vertex and index GPU buffers (idempotent).
    fn create_buffer_resources(&mut self) {
        if self.buffers_created {
            return;
        }
        // Full Storm: HgiBufferDesc for FULLSCREEN_TRIANGLE_VERTICES (vertex)
        //             and FULLSCREEN_TRIANGLE_INDICES (index32) -> Hgi::CreateBuffer
        self.buffers_created = true;
    }

    fn set_resource_bindings(&mut self) {
        // Full Storm: HgiResourceBindingsDesc with texture+sampler pairs and buffers
    }

    fn set_vertex_buffer_descriptor(&mut self) {
        // Full Storm: HgiVertexAttributeDesc:
        //   [0] "position" vec4f offset=0
        //   [1] "uvIn"     vec2f offset=16
        //   stride = FULLSCREEN_VERTEX_STRIDE
    }

    fn get_default_sampler(&mut self) -> HgiSamplerHandle {
        if let Some(ref s) = self.default_sampler {
            return s.clone();
        }
        // Full Storm: HgiSamplerDesc { magFilter=Linear, minFilter=Linear,
        //   addressModeU/V=ClampToEdge } -> Hgi::CreateSampler
        HgiSamplerHandle::null()
    }
}

impl Drop for HdxFullscreenShader {
    fn drop(&mut self) {
        // Full Storm: DestroyBuffer(vertex/index), DestroySampler, DestroyShaderProgram
        if let Some(ref program) = self.shader_program {
            self.base.destroy_shader_program(&mut Some(program.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fullscreen_tokens() {
        assert_eq!(
            FULLSCREEN_VERTEX_SHADER.as_str(),
            "hdx/shaders/fullscreen.glslfx"
        );
        assert_eq!(FULLSCREEN_VERTEX_TECHNIQUE.as_str(), "FullScreenVertex");
    }

    #[test]
    fn test_fullscreen_triangle_geometry() {
        // V0: top-left outside NDC
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[0], -1.0); // x
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[1], 3.0); // y
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[4], 0.0); // u
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[5], 2.0); // v

        // V1: bottom-left
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[6], -1.0); // x
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[7], -1.0); // y
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[10], 0.0); // u
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[11], 0.0); // v

        // V2: bottom-right outside NDC
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[12], 3.0); // x
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[13], -1.0); // y
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[16], 2.0); // u
        assert_eq!(FULLSCREEN_TRIANGLE_VERTICES[17], 0.0); // v
    }

    #[test]
    fn test_fullscreen_triangle_indices() {
        assert_eq!(FULLSCREEN_TRIANGLE_INDICES, [0, 1, 2]);
    }

    #[test]
    fn test_vertex_stride() {
        assert_eq!(FULLSCREEN_VERTEX_STRIDE, 24); // 6 floats * 4 bytes
    }

    #[test]
    fn test_blend_state_default() {
        let bs = HgiBlendState::default();
        assert!(!bs.enabled);
    }

    #[test]
    fn test_texture_handle_null() {
        assert!(HgiTextureHandle::null().is_null());
    }

    #[test]
    fn test_buffer_handle_null() {
        assert!(HgiBufferHandle::null().is_null());
    }

    #[test]
    fn test_sampler_handle_null() {
        assert!(HgiSamplerHandle::null().is_null());
    }

    #[test]
    fn test_depth_stencil_state_default() {
        let state = HgiDepthStencilState::default();
        assert!(state.depth_test_enabled);
        assert!(state.depth_write_enabled);
    }

    #[test]
    fn test_wgsl_snippet_not_empty() {
        let snippet = HdxFullscreenShader::wgsl_vertex_index_snippet();
        assert!(!snippet.is_empty());
        assert!(snippet.contains("vertex_index"));
        assert!(snippet.contains("positions"));
    }
}
