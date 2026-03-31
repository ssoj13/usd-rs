//! Main Hydra Graphics Interface trait

use super::blit_cmds::HgiBlitCmds;
use super::buffer::{HgiBufferDesc, HgiBufferHandle};
use super::capabilities::HgiCapabilities;
use super::cmds::HgiCmds;
use super::compute_cmds::HgiComputeCmds;
use super::compute_cmds_desc::HgiComputeCmdsDesc;
use super::compute_pipeline::{HgiComputePipelineDesc, HgiComputePipelineHandle};
use super::enums::HgiSubmitWaitType;
use super::graphics_cmds::HgiGraphicsCmds;
use super::graphics_cmds_desc::HgiGraphicsCmdsDesc;
use super::graphics_pipeline::{HgiGraphicsPipelineDesc, HgiGraphicsPipelineHandle};
use super::indirect_command_encoder::HgiIndirectCommandEncoder;
use super::resource_bindings::{HgiResourceBindingsDesc, HgiResourceBindingsHandle};
use super::sampler::{HgiSamplerDesc, HgiSamplerHandle};
use super::shader_function::{HgiShaderFunctionDesc, HgiShaderFunctionHandle};
use super::shader_program::{HgiShaderProgramDesc, HgiShaderProgramHandle};
use super::texture::{HgiTextureDesc, HgiTextureHandle, HgiTextureViewDesc, HgiTextureViewHandle};

/// Hydra Graphics Interface
///
/// HGI is used to communicate with one or more physical GPU devices.
/// It provides an abstract API to create/destroy GPU resources and record commands.
///
/// # Thread Safety
///
/// Modern graphics APIs like Metal and Vulkan are designed with multi-threading in mind.
/// Each HGI backend should at minimum support:
///
/// - Single threaded `submit_cmds()` on main thread
/// - Single threaded resource `create_***` / `destroy_***` on main thread
/// - Multi-threaded recording of commands in `Hgi***Cmds` objects
/// - A `Hgi***Cmds` object should be creatable on the main thread, recorded into
///   with one secondary thread, and submitted via the main thread
///
/// Additional multi-threading support (e.g., multi-threaded resource creation) is encouraged
/// but not required for OpenGL compatibility.
pub trait Hgi: Send + Sync {
    /// Determine if HGI backend can run on current hardware
    fn is_backend_supported(&self) -> bool;

    /// Get the capabilities of this device
    fn capabilities(&self) -> &HgiCapabilities;

    // --- Resource Creation ---

    /// Create a buffer
    ///
    /// If `initial_data` is provided, it will be uploaded immediately during creation.
    fn create_buffer(
        &mut self,
        desc: &HgiBufferDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiBufferHandle;

    /// Create a texture
    ///
    /// If `initial_data` is provided, it will be uploaded immediately during creation.
    fn create_texture(
        &mut self,
        desc: &HgiTextureDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiTextureHandle;

    /// Create a texture view that aliases another texture's data
    ///
    /// A texture view allows accessing a texture with a different format or a subset
    /// of its layers/mips. The client must ensure that the source texture is not
    /// destroyed while the view is in use.
    fn create_texture_view(&mut self, _desc: &HgiTextureViewDesc) -> HgiTextureViewHandle {
        // Default: return null handle. Backends override for texture view support.
        HgiTextureViewHandle::null()
    }

    /// Create a sampler
    fn create_sampler(&mut self, desc: &HgiSamplerDesc) -> HgiSamplerHandle;

    /// Create a shader function
    fn create_shader_function(&mut self, desc: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle;

    /// Create a shader program by linking shader functions
    fn create_shader_program(&mut self, desc: &HgiShaderProgramDesc) -> HgiShaderProgramHandle;

    /// Create resource bindings
    fn create_resource_bindings(
        &mut self,
        desc: &HgiResourceBindingsDesc,
    ) -> HgiResourceBindingsHandle;

    /// Create a graphics pipeline
    fn create_graphics_pipeline(
        &mut self,
        desc: &HgiGraphicsPipelineDesc,
    ) -> HgiGraphicsPipelineHandle;

    /// Create a compute pipeline
    fn create_compute_pipeline(
        &mut self,
        desc: &HgiComputePipelineDesc,
    ) -> HgiComputePipelineHandle;

    // --- Resource Destruction ---

    /// Destroy a buffer
    fn destroy_buffer(&mut self, buffer: &HgiBufferHandle);

    /// Destroy a texture
    fn destroy_texture(&mut self, texture: &HgiTextureHandle);

    /// Destroy a texture view
    ///
    /// This destroys the view but not the source texture that was aliased.
    /// The source texture data remains unchanged.
    fn destroy_texture_view(&mut self, _view: &HgiTextureViewHandle) {
        // Default: no-op. Backends override for texture view support.
    }

    /// Destroy a sampler
    fn destroy_sampler(&mut self, sampler: &HgiSamplerHandle);

    /// Destroy a shader function
    fn destroy_shader_function(&mut self, function: &HgiShaderFunctionHandle);

    /// Destroy a shader program
    fn destroy_shader_program(&mut self, program: &HgiShaderProgramHandle);

    /// Destroy resource bindings
    fn destroy_resource_bindings(&mut self, bindings: &HgiResourceBindingsHandle);

    /// Destroy a graphics pipeline
    fn destroy_graphics_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle);

    /// Destroy a compute pipeline
    fn destroy_compute_pipeline(&mut self, pipeline: &HgiComputePipelineHandle);

    // --- Command Buffer Creation ---

    /// Create a blit command buffer for copy operations
    fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds>;

    /// Create a graphics command buffer for rendering with color/depth attachments
    fn create_graphics_cmds(&mut self, desc: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds>;

    /// Create a compute command buffer for compute shaders
    fn create_compute_cmds(&mut self, desc: &HgiComputeCmdsDesc) -> Box<dyn HgiComputeCmds>;

    /// Get the device-specific indirect command encoder, or None if unsupported
    fn get_indirect_command_encoder(&self) -> Option<&dyn HgiIndirectCommandEncoder> {
        None // Default: not supported. Metal overrides.
    }

    // --- Command Submission ---

    /// Submit a command buffer to the GPU
    ///
    /// Once submitted, the command buffer cannot be re-used to record commands.
    /// This call is not thread-safe and must happen on the main thread.
    fn submit_cmds(&mut self, cmds: Box<dyn HgiCmds>, wait: HgiSubmitWaitType);

    // --- Utility ---

    /// Get a unique resource ID for creating handles
    fn unique_id(&mut self) -> u64;

    /// Wait for all GPU work to complete (for debugging/profiling)
    fn wait_for_idle(&mut self);

    /// Return the name of the api (e.g. "OpenGL", "Vulkan", "Metal").
    /// Thread safety: This call is thread safe.
    fn get_api_name(&self) -> &str;

    /// Optionally called by client app at the start of a new rendering frame.
    /// We can't rely on StartFrame for anything important, because it is up to
    /// the external client to (optionally) call this and they may never do.
    /// This can be helpful to insert GPU frame debug markers.
    /// Thread safety: Not thread safe. Should be called on the main thread.
    fn start_frame(&mut self);

    /// Optionally called at the end of a rendering frame.
    /// Thread safety: Not thread safe. Should be called on the main thread.
    fn end_frame(&mut self);

    /// Perform any necessary garbage collection, if applicable.
    /// This can be used to flush pending deletes immediately after unloading
    /// assets, for example. Note that as some clients may not call this, Hgi
    /// implementations should find other opportunities to garbage collect as
    /// well (e.g. EndFrame).
    fn garbage_collect(&mut self);

    /// Unique identity for the underlying GPU device. Used to partition
    /// caches that store device-specific objects (pipelines, bind groups).
    /// Default returns 0 (single-device process).
    fn device_identity(&self) -> u64 {
        0
    }
}

// ============================================================================
// Platform factory functions
// ============================================================================

/// Registry entry for a named Hgi backend factory.
pub struct HgiBackendEntry {
    /// Backend name token (e.g. "wgpu", "OpenGL")
    pub name: &'static str,
    /// Factory function that constructs the backend
    pub factory: fn() -> Box<dyn Hgi>,
}

// Global registry populated via `register_hgi_backend!` macro.
static HGI_BACKEND_REGISTRY: std::sync::RwLock<Vec<HgiBackendEntry>> =
    std::sync::RwLock::new(Vec::new());

/// Register an Hgi backend factory under the given name.
///
/// This is the Rust-idiomatic replacement for C++ TfType plugin discovery.
/// Call this at startup (or via `register_hgi_backend!` macro) before
/// calling `create_platform_default_hgi()`.
pub fn register_hgi_backend(name: &'static str, factory: fn() -> Box<dyn Hgi>) {
    let mut reg = HGI_BACKEND_REGISTRY
        .write()
        .expect("HGI_BACKEND_REGISTRY poisoned");
    // Avoid duplicate registration
    if !reg.iter().any(|e| e.name == name) {
        reg.push(HgiBackendEntry { name, factory });
    }
}

/// Macro to register an Hgi backend at startup using `inventory` or manual call.
///
/// Usage: `register_hgi_backend!("wgpu", HgiWgpu::new_boxed);`
#[macro_export]
macro_rules! register_hgi_backend {
    ($name:expr, $factory:expr) => {
        $crate::hgi::register_hgi_backend($name, $factory);
    };
}

/// Return the name of the platform-default Hgi backend.
///
/// In this codebase wgpu is the primary backend (replaces HgiGL/HgiVulkan/HgiMetal).
/// Returns `"wgpu"` unless overridden via registered backends.
pub fn get_platform_default_hgi_name() -> &'static str {
    "wgpu"
}

/// Create the platform-default Hgi instance.
///
/// Mirrors C++ `Hgi::CreatePlatformDefaultHgi()`.
/// Looks up the backend named `get_platform_default_hgi_name()` in the registry.
/// Returns `None` if no backend with that name has been registered.
pub fn create_platform_default_hgi() -> Option<Box<dyn Hgi>> {
    let default_name = get_platform_default_hgi_name();
    create_named_hgi(default_name)
}

/// Create a named Hgi instance by backend name.
///
/// Mirrors C++ `Hgi::CreateNamedHgi(const TfToken&)`.
/// Looks up the backend in the static registry and invokes its factory.
/// Returns `None` if the name is not registered.
pub fn create_named_hgi(name: &str) -> Option<Box<dyn Hgi>> {
    let reg = HGI_BACKEND_REGISTRY
        .read()
        .expect("HGI_BACKEND_REGISTRY poisoned");
    let entry = reg.iter().find(|e| e.name == name)?;
    Some((entry.factory)())
}

/// Check if an Hgi backend is supported on the current hardware.
///
/// Creates a temporary instance and calls `is_backend_supported()`.
/// Mirrors C++ `Hgi::IsSupported(const TfToken&)`.
pub fn is_hgi_supported(name: &str) -> bool {
    create_named_hgi(name)
        .map(|hgi| hgi.is_backend_supported())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::blit_cmds::*;
    use super::super::compute_cmds::*;
    use super::super::graphics_cmds::*;
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Mock blit commands for testing
    struct MockBlitCmds {
        submitted: bool,
    }

    impl HgiCmds for MockBlitCmds {
        fn is_submitted(&self) -> bool {
            self.submitted
        }
        fn push_debug_group(&mut self, _label: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _label: &str) {}
    }

    impl HgiBlitCmds for MockBlitCmds {
        fn copy_buffer_cpu_to_gpu(&mut self, _op: &HgiBufferCpuToGpuOp) {}
        fn copy_buffer_gpu_to_gpu(&mut self, _op: &HgiBufferGpuToGpuOp) {}
        fn copy_texture_cpu_to_gpu(&mut self, _op: &HgiTextureCpuToGpuOp) {}
        fn copy_texture_gpu_to_gpu(&mut self, _op: &HgiTextureGpuToGpuOp) {}
        fn copy_texture_gpu_to_cpu(&mut self, _op: &HgiTextureGpuToCpuOp) {}
        fn copy_buffer_to_texture(&mut self, _op: &HgiBufferToTextureOp) {}
        fn copy_texture_to_buffer(&mut self, _op: &HgiTextureToBufferOp) {}
        fn generate_mipmap(&mut self, _texture: &HgiTextureHandle) {}
        fn fill_buffer(&mut self, _buffer: &HgiBufferHandle, _value: u8) {}
    }

    // Mock graphics commands for testing
    struct MockGraphicsCmds {
        submitted: bool,
    }

    impl HgiCmds for MockGraphicsCmds {
        fn is_submitted(&self) -> bool {
            self.submitted
        }
        fn push_debug_group(&mut self, _label: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _label: &str) {}
    }

    impl HgiGraphicsCmds for MockGraphicsCmds {
        fn bind_pipeline(&mut self, _pipeline: &HgiGraphicsPipelineHandle) {}
        fn bind_resources(&mut self, _resources: &HgiResourceBindingsHandle) {}
        fn bind_vertex_buffers(&mut self, _buffers: &[HgiBufferHandle], _offsets: &[u64]) {}
        fn set_viewport(&mut self, _viewport: &HgiViewport) {}
        fn set_scissor(&mut self, _scissor: &HgiScissor) {}
        fn set_blend_constant_color(&mut self, _color: &usd_gf::Vec4f) {}
        fn set_stencil_reference_value(&mut self, _value: u32) {}
        fn draw(&mut self, _op: &HgiDrawOp) {}
        fn draw_indexed(&mut self, _index_buffer: &HgiBufferHandle, _op: &HgiDrawIndexedOp) {}
        fn draw_indirect(&mut self, _op: &HgiDrawIndirectOp) {}
        fn draw_indexed_indirect(
            &mut self,
            _index_buffer: &HgiBufferHandle,
            _op: &HgiDrawIndirectOp,
        ) {
        }
        fn memory_barrier(&mut self, _barrier: super::super::enums::HgiMemoryBarrier) {}
    }

    // Mock compute commands for testing
    struct MockComputeCmds {
        submitted: bool,
    }

    impl HgiCmds for MockComputeCmds {
        fn is_submitted(&self) -> bool {
            self.submitted
        }
        fn push_debug_group(&mut self, _label: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _label: &str) {}
    }

    impl HgiComputeCmds for MockComputeCmds {
        fn bind_pipeline(&mut self, _pipeline: &HgiComputePipelineHandle) {}
        fn bind_resources(&mut self, _resources: &HgiResourceBindingsHandle) {}
        fn dispatch(&mut self, _dispatch: &crate::compute_cmds::HgiComputeDispatchOp) {}
        fn memory_barrier(&mut self, _barrier: super::super::enums::HgiMemoryBarrier) {}
    }

    // Mock HGI implementation for testing
    struct MockHgi {
        id_counter: AtomicU64,
        capabilities: HgiCapabilities,
    }

    impl MockHgi {
        fn new() -> Self {
            Self {
                id_counter: AtomicU64::new(1),
                capabilities: HgiCapabilities::default(),
            }
        }
    }

    impl Hgi for MockHgi {
        fn is_backend_supported(&self) -> bool {
            true
        }

        fn capabilities(&self) -> &HgiCapabilities {
            &self.capabilities
        }

        fn create_buffer(
            &mut self,
            _desc: &HgiBufferDesc,
            _initial_data: Option<&[u8]>,
        ) -> HgiBufferHandle {
            HgiBufferHandle::null()
        }

        fn create_texture(
            &mut self,
            _desc: &HgiTextureDesc,
            _initial_data: Option<&[u8]>,
        ) -> HgiTextureHandle {
            HgiTextureHandle::null()
        }

        fn create_sampler(&mut self, _desc: &HgiSamplerDesc) -> HgiSamplerHandle {
            HgiSamplerHandle::null()
        }

        fn create_shader_function(
            &mut self,
            _desc: &HgiShaderFunctionDesc,
        ) -> HgiShaderFunctionHandle {
            HgiShaderFunctionHandle::null()
        }

        fn create_shader_program(
            &mut self,
            _desc: &HgiShaderProgramDesc,
        ) -> HgiShaderProgramHandle {
            HgiShaderProgramHandle::null()
        }

        fn create_resource_bindings(
            &mut self,
            _desc: &HgiResourceBindingsDesc,
        ) -> HgiResourceBindingsHandle {
            HgiResourceBindingsHandle::null()
        }

        fn create_graphics_pipeline(
            &mut self,
            _desc: &HgiGraphicsPipelineDesc,
        ) -> HgiGraphicsPipelineHandle {
            HgiGraphicsPipelineHandle::null()
        }

        fn create_compute_pipeline(
            &mut self,
            _desc: &HgiComputePipelineDesc,
        ) -> HgiComputePipelineHandle {
            HgiComputePipelineHandle::null()
        }

        fn destroy_buffer(&mut self, _buffer: &HgiBufferHandle) {}
        fn destroy_texture(&mut self, _texture: &HgiTextureHandle) {}
        fn destroy_sampler(&mut self, _sampler: &HgiSamplerHandle) {}
        fn destroy_shader_function(&mut self, _function: &HgiShaderFunctionHandle) {}
        fn destroy_shader_program(&mut self, _program: &HgiShaderProgramHandle) {}
        fn destroy_resource_bindings(&mut self, _bindings: &HgiResourceBindingsHandle) {}
        fn destroy_graphics_pipeline(&mut self, _pipeline: &HgiGraphicsPipelineHandle) {}
        fn destroy_compute_pipeline(&mut self, _pipeline: &HgiComputePipelineHandle) {}

        fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
            Box::new(MockBlitCmds { submitted: false })
        }

        fn create_graphics_cmds(
            &mut self,
            _desc: &HgiGraphicsCmdsDesc,
        ) -> Box<dyn HgiGraphicsCmds> {
            Box::new(MockGraphicsCmds { submitted: false })
        }

        fn create_compute_cmds(&mut self, _desc: &HgiComputeCmdsDesc) -> Box<dyn HgiComputeCmds> {
            Box::new(MockComputeCmds { submitted: false })
        }

        fn submit_cmds(&mut self, _cmds: Box<dyn HgiCmds>, _wait: HgiSubmitWaitType) {}

        fn unique_id(&mut self) -> u64 {
            self.id_counter.fetch_add(1, Ordering::SeqCst)
        }

        fn wait_for_idle(&mut self) {}

        fn get_api_name(&self) -> &str {
            "Mock"
        }

        fn start_frame(&mut self) {}

        fn end_frame(&mut self) {}

        fn garbage_collect(&mut self) {}
    }

    #[test]
    fn test_hgi_trait() {
        let mut hgi = MockHgi::new();

        assert!(hgi.is_backend_supported());
        // Capabilities default: extended fields are 0 until backend populates them
        let _caps = hgi.capabilities();

        let id1 = hgi.unique_id();
        let id2 = hgi.unique_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_create_cmds() {
        let mut hgi = MockHgi::new();

        let blit = hgi.create_blit_cmds();
        assert!(!blit.is_submitted());

        let gfx_desc = HgiGraphicsCmdsDesc::new();
        let gfx = hgi.create_graphics_cmds(&gfx_desc);
        assert!(!gfx.is_submitted());

        let compute_desc = HgiComputeCmdsDesc::new();
        let compute = hgi.create_compute_cmds(&compute_desc);
        assert!(!compute.is_submitted());
    }
}
