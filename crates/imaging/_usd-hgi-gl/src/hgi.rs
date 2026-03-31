//! OpenGL implementation of HGI

use super::blit_cmds::HgiGLBlitCmds;
use super::buffer::HgiGLBuffer;
use super::capabilities::HgiGLCapabilities;
use super::compute_cmds::HgiGLComputeCmds;
use super::compute_pipeline::HgiGLComputePipeline;
use super::graphics_cmds::HgiGLGraphicsCmds;
use super::graphics_pipeline::HgiGLGraphicsPipeline;
use super::resource_bindings::HgiGLResourceBindings;
use super::sampler::HgiGLSampler;
use super::shader_function::HgiGLShaderFunction;
use super::shader_program::HgiGLShaderProgram;
use super::texture::HgiGLTexture;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use usd_hgi::*;

/// OpenGL implementation of Hydra Graphics Interface
///
/// # OpenGL Context Management
///
/// HgiGL expects an OpenGL 4.5+ context to be created and made current
/// before any operations. The context must remain valid for the lifetime
/// of the HgiGL instance.
///
/// # Thread Safety
///
/// HgiGL supports:
/// - Single-threaded command submission on the main thread
/// - Single-threaded resource creation/destruction on the main thread
/// - Multi-threaded command recording (commands recorded on worker threads,
///   submitted on main thread)
///
/// # Implementation Status
///
/// This is a STUB implementation. Actual OpenGL calls require:
/// - The `gl` crate for OpenGL bindings
/// - A valid OpenGL 4.5+ context
/// - Platform-specific context creation
pub struct HgiGL {
    /// Device capabilities
    capabilities: HgiGLCapabilities,

    /// Unique ID counter for handles
    id_counter: AtomicU64,

    /// Whether the backend is initialized
    initialized: bool,

    /// Frame depth counter for nested StartFrame/EndFrame calls.
    /// Protects against client calling StartFrame more than once (nested engines).
    frame_depth: std::sync::atomic::AtomicI32,

    // --- Deferred deletion trash lists ---
    // Resources are moved here on destroy_* calls, then deleted during garbage_collect()
    // when the GPU is guaranteed to be done with them.
    /// Buffer handles pending GL deletion
    trash_buffers: Vec<HgiBufferHandle>,
    /// Texture handles pending GL deletion
    trash_textures: Vec<HgiTextureHandle>,
    /// Sampler handles pending GL deletion
    trash_samplers: Vec<HgiSamplerHandle>,
    /// Shader function handles pending GL deletion
    trash_shader_functions: Vec<HgiShaderFunctionHandle>,
    /// Shader program handles pending GL deletion
    trash_shader_programs: Vec<HgiShaderProgramHandle>,
    /// Resource bindings pending GL deletion
    trash_resource_bindings: Vec<HgiResourceBindingsHandle>,
    /// Graphics pipeline handles pending GL deletion
    trash_graphics_pipelines: Vec<HgiGraphicsPipelineHandle>,
    /// Compute pipeline handles pending GL deletion
    trash_compute_pipelines: Vec<HgiComputePipelineHandle>,
}

impl HgiGL {
    /// Create a new OpenGL HGI instance
    ///
    /// # Requirements
    ///
    /// An OpenGL 4.5+ context must be current on the calling thread.
    ///
    /// # Stub
    ///
    /// Real implementation: verify GL context, query version,
    /// initialize capabilities, set up debug callbacks.
    pub fn new() -> Self {
        // Note: Would verify GL context here
        // let mut context_available = false;
        // unsafe {
        //     // Try to get GL version - if this fails, no context is current
        //     let version_ptr = gl::GetString(gl::VERSION);
        //     context_available = !version_ptr.is_null();
        // }

        let capabilities = HgiGLCapabilities::new();

        Self {
            capabilities,
            id_counter: AtomicU64::new(1),
            initialized: true,
            frame_depth: std::sync::atomic::AtomicI32::new(0),
            trash_buffers: Vec::new(),
            trash_textures: Vec::new(),
            trash_samplers: Vec::new(),
            trash_shader_functions: Vec::new(),
            trash_shader_programs: Vec::new(),
            trash_resource_bindings: Vec::new(),
            trash_graphics_pipelines: Vec::new(),
            trash_compute_pipelines: Vec::new(),
        }
    }

    /// Check if OpenGL backend can run on current hardware
    ///
    /// # Stub
    ///
    /// Real implementation: check GL context, version >= 4.5, extensions.
    fn check_backend_support() -> bool {
        // Note: Would check actual GL support
        // - Try to create temporary GL context
        // - Query GL_VERSION
        // - Parse version and check >= 4.5
        // - Check for required extensions
        true
    }
}

impl Default for HgiGL {
    fn default() -> Self {
        Self::new()
    }
}

impl Hgi for HgiGL {
    fn is_backend_supported(&self) -> bool {
        Self::check_backend_support() && self.initialized
    }

    fn capabilities(&self) -> &HgiCapabilities {
        self.capabilities.base_capabilities()
    }

    // --- Resource Creation ---

    fn create_buffer(
        &mut self,
        desc: &HgiBufferDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiBufferHandle {
        let buffer = Arc::new(HgiGLBuffer::new(desc, initial_data));
        let id = self.unique_id();
        HgiBufferHandle::new(buffer, id)
    }

    fn create_texture(
        &mut self,
        desc: &HgiTextureDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiTextureHandle {
        let texture = Arc::new(HgiGLTexture::new(desc, initial_data));
        let id = self.unique_id();
        HgiTextureHandle::new(texture, id)
    }

    /// Create a texture view (GL texture alias with different format/mip/layer range).
    ///
    /// Uses `glTextureView` to create a GL texture object that shares storage
    /// with the source texture but interprets it with a different format or range.
    /// Matches C++ `HgiGL::CreateTextureView()`.
    fn create_texture_view(&mut self, desc: &HgiTextureViewDesc) -> HgiTextureViewHandle {
        // We need the source texture's GL ID to call glTextureView.
        // The view texture gets a *new* GL name but shares the source's storage.
        let src_tex_id = desc
            .source_texture
            .get()
            .map(|t| t.raw_resource() as u32)
            .unwrap_or(0);

        if src_tex_id == 0 {
            log::error!("create_texture_view: null source texture");
            return HgiTextureViewHandle::null();
        }

        // Build a HgiTextureDesc for the view texture by copying source properties
        // then overriding format, first-mip and first-layer.
        let view_tex_desc = if let Some(src) = desc.source_texture.get() {
            let mut d = src.descriptor().clone();
            d.format = desc.format;
            d.mip_levels = desc.source_mip_count;
            d
        } else {
            return HgiTextureViewHandle::null();
        };

        // Create GL texture object for the view.
        #[cfg(feature = "opengl")]
        let view_tex = {
            use gl::types::GLuint;
            let mut view_id: GLuint = 0;
            let target =
                super::conversions::hgi_texture_type_to_gl_target(view_tex_desc.texture_type);
            let internal_format = hgi_format_to_gl_internal_format(desc.format);
            unsafe {
                gl::GenTextures(1, &mut view_id);
                gl::TextureView(
                    view_id,
                    target,
                    src_tex_id,
                    internal_format,
                    desc.source_first_mip as u32,
                    desc.source_mip_count as u32,
                    desc.source_first_layer as u32,
                    desc.source_layer_count as u32,
                );
            }
            // Wrap the raw view_id in an HgiGLTexture-like object.
            // We construct a fake texture that owns the view GL name.
            HgiGLTexture::from_raw(view_id, &view_tex_desc)
        };

        #[cfg(not(feature = "opengl"))]
        let view_tex = HgiGLTexture::new(&view_tex_desc, None);

        let arc_view = Arc::new(view_tex);
        let id = self.unique_id();
        HgiTextureViewHandle::new(arc_view, id)
    }

    fn destroy_texture_view(&mut self, view: &HgiTextureViewHandle) {
        // The HgiTextureViewHandle holds an Arc<dyn HgiTexture>.
        // Dropping the handle clone here decrements the refcount; when it hits 0
        // the Arc's drop calls glDeleteTextures on the view's GL name.
        // Clone + drop is idiomatic here because we can't get an owned handle.
        let _ = view.clone();
    }

    fn create_sampler(&mut self, desc: &HgiSamplerDesc) -> HgiSamplerHandle {
        let sampler = Arc::new(HgiGLSampler::new(desc));
        let id = self.unique_id();
        HgiSamplerHandle::new(sampler, id)
    }

    fn create_shader_function(&mut self, desc: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle {
        let function = Arc::new(HgiGLShaderFunction::new(desc));
        let id = self.unique_id();
        HgiShaderFunctionHandle::new(function, id)
    }

    fn create_shader_program(&mut self, desc: &HgiShaderProgramDesc) -> HgiShaderProgramHandle {
        let program = Arc::new(HgiGLShaderProgram::new(desc));
        let id = self.unique_id();
        HgiShaderProgramHandle::new(program, id)
    }

    fn create_resource_bindings(
        &mut self,
        desc: &HgiResourceBindingsDesc,
    ) -> HgiResourceBindingsHandle {
        let bindings = Arc::new(HgiGLResourceBindings::new(desc));
        let id = self.unique_id();
        HgiResourceBindingsHandle::new(bindings, id)
    }

    fn create_graphics_pipeline(
        &mut self,
        desc: &HgiGraphicsPipelineDesc,
    ) -> HgiGraphicsPipelineHandle {
        let pipeline = Arc::new(HgiGLGraphicsPipeline::new(desc));
        let id = self.unique_id();
        HgiGraphicsPipelineHandle::new(pipeline, id)
    }

    fn create_compute_pipeline(
        &mut self,
        desc: &HgiComputePipelineDesc,
    ) -> HgiComputePipelineHandle {
        let pipeline = Arc::new(HgiGLComputePipeline::new(desc));
        let id = self.unique_id();
        HgiComputePipelineHandle::new(pipeline, id)
    }

    // --- Resource Destruction ---

    fn destroy_buffer(&mut self, buffer: &HgiBufferHandle) {
        // Defer deletion until garbage_collect() when GPU is done with the resource
        self.trash_buffers.push(buffer.clone());
    }

    fn destroy_texture(&mut self, texture: &HgiTextureHandle) {
        self.trash_textures.push(texture.clone());
    }

    fn destroy_sampler(&mut self, sampler: &HgiSamplerHandle) {
        self.trash_samplers.push(sampler.clone());
    }

    fn destroy_shader_function(&mut self, function: &HgiShaderFunctionHandle) {
        self.trash_shader_functions.push(function.clone());
    }

    fn destroy_shader_program(&mut self, program: &HgiShaderProgramHandle) {
        self.trash_shader_programs.push(program.clone());
    }

    fn destroy_resource_bindings(&mut self, bindings: &HgiResourceBindingsHandle) {
        self.trash_resource_bindings.push(bindings.clone());
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle) {
        self.trash_graphics_pipelines.push(pipeline.clone());
    }

    fn destroy_compute_pipeline(&mut self, pipeline: &HgiComputePipelineHandle) {
        self.trash_compute_pipelines.push(pipeline.clone());
    }

    // --- Command Buffer Creation ---

    fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
        Box::new(HgiGLBlitCmds::new())
    }

    fn create_graphics_cmds(&mut self, desc: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds> {
        Box::new(HgiGLGraphicsCmds::new_with_desc(desc.clone()))
    }

    fn create_compute_cmds(
        &mut self,
        _desc: &usd_hgi::HgiComputeCmdsDesc,
    ) -> Box<dyn HgiComputeCmds> {
        Box::new(HgiGLComputeCmds::new())
    }

    // --- Command Submission ---

    #[cfg(feature = "opengl")]
    fn submit_cmds(&mut self, mut cmds: Box<dyn HgiCmds>, wait: HgiSubmitWaitType) {
        // Execute GL calls (OpenGL has immediate mode - no deferred command queues)
        cmds.execute_submit();

        // CPU-GPU synchronization by client request
        if wait == HgiSubmitWaitType::WaitUntilCompleted {
            const TIMEOUT_NS: u64 = 100_000_000_000; // 100 seconds

            unsafe {
                let fence = gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
                let status = gl::ClientWaitSync(fence, gl::SYNC_FLUSH_COMMANDS_BIT, TIMEOUT_NS);

                if status != gl::ALREADY_SIGNALED && status != gl::CONDITION_SATISFIED {
                    log::error!("Unexpected ClientWaitSync timeout");
                }

                gl::DeleteSync(fence);
            }
        }

        drop(cmds);

        // If the Hgi client does not call EndFrame we garbage collect here
        if self.frame_depth.load(Ordering::Acquire) == 0 {
            self.garbage_collect();
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn submit_cmds(&mut self, cmds: Box<dyn HgiCmds>, _wait: HgiSubmitWaitType) {
        // Note: Commands dropped when OpenGL not compiled in
        drop(cmds);
    }

    // --- Utility ---

    fn unique_id(&mut self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }

    #[cfg(feature = "opengl")]
    fn wait_for_idle(&mut self) {
        unsafe {
            gl::Finish();
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn wait_for_idle(&mut self) {
        // Note: No-op when OpenGL not compiled in
    }

    fn get_api_name(&self) -> &str {
        "OpenGL"
    }

    #[cfg(feature = "opengl")]
    fn start_frame(&mut self) {
        // Protect against client calling StartFrame more than once (nested engines)
        let prev_depth = self.frame_depth.fetch_add(1, Ordering::AcqRel);
        if prev_depth == 0 {
            // Start Full Frame debug label
            unsafe {
                if gl::PushDebugGroup::is_loaded() {
                    let label = b"Full Hydra Frame\0";
                    gl::PushDebugGroup(
                        gl::DEBUG_SOURCE_THIRD_PARTY,
                        0,
                        label.len() as i32 - 1,
                        label.as_ptr() as *const i8,
                    );
                }
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn start_frame(&mut self) {
        self.frame_depth.fetch_add(1, Ordering::AcqRel);
    }

    #[cfg(feature = "opengl")]
    fn end_frame(&mut self) {
        let prev_depth = self.frame_depth.fetch_sub(1, Ordering::AcqRel);
        if prev_depth == 1 {
            // End Full Frame debug label
            unsafe {
                if gl::PopDebugGroup::is_loaded() {
                    gl::PopDebugGroup();
                }
            }
            // Garbage collect
            self.garbage_collect();
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn end_frame(&mut self) {
        let prev_depth = self.frame_depth.fetch_sub(1, Ordering::AcqRel);
        if prev_depth == 1 {
            self.garbage_collect();
        }
    }

    #[cfg(feature = "opengl")]
    fn garbage_collect(&mut self) {
        unsafe {
            if gl::PushDebugGroup::is_loaded() {
                let label = b"Garbage Collection\0";
                gl::PushDebugGroup(
                    gl::DEBUG_SOURCE_THIRD_PARTY,
                    0,
                    label.len() as i32 - 1,
                    label.as_ptr() as *const i8,
                );
            }
        }

        // Drain all trash lists. Dropping the last Arc ref triggers
        // the resource's Drop impl which calls gl::Delete* internally.
        self.trash_buffers.clear();
        self.trash_textures.clear();
        self.trash_samplers.clear();
        self.trash_shader_functions.clear();
        self.trash_shader_programs.clear();
        self.trash_resource_bindings.clear();
        self.trash_graphics_pipelines.clear();
        self.trash_compute_pipelines.clear();

        unsafe {
            if gl::PopDebugGroup::is_loaded() {
                gl::PopDebugGroup();
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn garbage_collect(&mut self) {
        // Drain all trash lists (Drop impls are no-op without opengl)
        self.trash_buffers.clear();
        self.trash_textures.clear();
        self.trash_samplers.clear();
        self.trash_shader_functions.clear();
        self.trash_shader_programs.clear();
        self.trash_resource_bindings.clear();
        self.trash_graphics_pipelines.clear();
        self.trash_compute_pipelines.clear();
    }
}

/// Create a new HgiGL instance
///
/// # Returns
///
/// Returns `Some(HgiGL)` if OpenGL backend is supported, `None` otherwise.
///
/// # Requirements
///
/// An OpenGL 4.5+ context must be current on the calling thread.
pub fn create_hgi_gl() -> Option<HgiGL> {
    if HgiGL::check_backend_support() {
        Some(HgiGL::new())
    } else {
        None
    }
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;
    use usd_gf::Vec3i;

    let hgi = HgiGL::new();
    assert!(hgi.initialized);
    assert!(hgi.is_backend_supported());

    let mut hgi = HgiGL::new();
    let desc = HgiBufferDesc::new()
        .with_usage(HgiBufferUsage::VERTEX)
        .with_byte_size(1024);

    let buffer = hgi.create_buffer(&desc, None);
    assert!(!buffer.is_null());
    hgi.destroy_buffer(&buffer);

    let mut hgi = HgiGL::new();
    let desc = HgiTextureDesc::new()
        .with_format(HgiFormat::UNorm8Vec4)
        .with_dimensions(Vec3i::new(256, 256, 1))
        .with_texture_type(HgiTextureType::Texture2D)
        .with_usage(HgiTextureUsage::SHADER_READ);

    let texture = hgi.create_texture(&desc, None);
    assert!(!texture.is_null());
    hgi.destroy_texture(&texture);

    let mut hgi = HgiGL::new();

    let blit_cmds = hgi.create_blit_cmds();
    assert!(!blit_cmds.is_submitted());

    let gfx_desc = HgiGraphicsCmdsDesc::new();
    let graphics_cmds = hgi.create_graphics_cmds(&gfx_desc);
    assert!(!graphics_cmds.is_submitted());

    let compute_cmds = hgi.create_compute_cmds();
    assert!(!compute_cmds.is_submitted());

    let mut hgi = HgiGL::new();

    let id1 = hgi.unique_id();
    let id2 = hgi.unique_id();
    let id3 = hgi.unique_id();

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert!(id2 > id1);
    assert!(id3 > id2);
}
