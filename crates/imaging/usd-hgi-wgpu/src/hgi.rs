//! Main wgpu implementation of Hydra Graphics Interface

use crate::blit_cmds::WgpuBlitCmds;
use crate::buffer::WgpuBuffer;
use crate::capabilities::WgpuCapabilities;
use crate::compute_cmds::WgpuComputeCmds;
use crate::compute_pipeline::WgpuComputePipeline;
use crate::graphics_cmds::WgpuGraphicsCmds;
use crate::graphics_pipeline::WgpuGraphicsPipeline;
use crate::mipmap::MipmapGenerator;
use crate::resource_bindings::WgpuResourceBindings;
use crate::sampler::WgpuSampler;
use crate::shader_function::WgpuShaderFunction;
use crate::shader_program::WgpuShaderProgram;
use crate::texture::WgpuTexture;
use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicU64, Ordering},
};
use usd_hgi::enums::HgiShaderStage;
use usd_hgi::shader_program::HgiShaderProgram;
use usd_hgi::*;

/// wgpu implementation of Hydra Graphics Interface.
///
/// Wraps wgpu Device/Queue/Adapter for cross-platform GPU access.
/// Covers Vulkan, Metal, DX12, and OpenGL under one backend --
/// replacing the separate HgiGL, HgiVulkan, HgiMetal C++ backends.
///
/// # Thread Safety
///
/// wgpu Device and Queue are Send + Sync. Command encoders can be
/// recorded on any thread; submission goes through Queue::submit().
///
/// # Initialization
///
/// Uses pollster::block_on for synchronous wgpu adapter/device creation.
/// Prefer `create_hgi_wgpu()` factory function.
///
/// # Surface Support
///
/// For window rendering, call `set_surface()` after construction.
/// Headless/offscreen rendering works without a surface.
pub struct HgiWgpu {
    /// wgpu instance (selects backend: Vulkan/Metal/DX12/GL)
    instance: wgpu::Instance,

    /// Physical GPU adapter
    adapter: Arc<wgpu::Adapter>,

    /// Logical device handle
    device: Arc<wgpu::Device>,

    /// Command submission queue
    queue: Arc<wgpu::Queue>,

    /// Queried device capabilities
    capabilities: HgiCapabilities,

    /// Monotonic ID counter for handles
    id_counter: AtomicU64,

    /// Frame nesting depth (for StartFrame/EndFrame). Thread-safe for nested engines.
    frame_depth: AtomicI32,

    /// Deferred resource deletions processed on garbage_collect()
    deferred_destroy_buffers: Vec<HgiBufferHandle>,
    deferred_destroy_textures: Vec<HgiTextureHandle>,
    deferred_destroy_samplers: Vec<HgiSamplerHandle>,
    deferred_destroy_shader_functions: Vec<HgiShaderFunctionHandle>,
    deferred_destroy_shader_programs: Vec<HgiShaderProgramHandle>,
    /// GPU mipmap generation pipeline
    mipmap_gen: Arc<MipmapGenerator>,
    deferred_destroy_resource_bindings: Vec<HgiResourceBindingsHandle>,
    deferred_destroy_graphics_pipelines: Vec<HgiGraphicsPipelineHandle>,
    deferred_destroy_compute_pipelines: Vec<HgiComputePipelineHandle>,

    /// Optional window surface for presentation (None = headless)
    surface: Option<wgpu::Surface<'static>>,
    /// Surface configuration (set when surface is configured)
    surface_config: Option<wgpu::SurfaceConfiguration>,
}

impl HgiWgpu {
    /// Create a new wgpu HGI instance.
    ///
    /// Blocks on async wgpu initialization via pollster.
    /// Requests a high-performance adapter with default backends.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_async())
    }

    /// Build the device descriptor used by both standalone and shared-device init.
    pub fn create_device_descriptor(
        adapter: &wgpu::Adapter,
        label: &'static str,
    ) -> wgpu::DeviceDescriptor<'static> {
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();

        if adapter_features.contains(wgpu::Features::PUSH_CONSTANTS) {
            required_features |= wgpu::Features::PUSH_CONSTANTS;
        } else {
            log::warn!("wgpu adapter lacks PUSH_CONSTANTS, per-draw uniforms won't work");
        }

        if adapter_features.contains(wgpu::Features::FLOAT32_FILTERABLE) {
            required_features |= wgpu::Features::FLOAT32_FILTERABLE;
        } else {
            log::warn!("wgpu adapter lacks FLOAT32_FILTERABLE, IBL may not render correctly");
        }

        if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
        } else {
            log::info!("wgpu adapter lacks TIMESTAMP_QUERY, GPU frame timing unavailable");
        }

        if adapter_features.contains(wgpu::Features::POLYGON_MODE_LINE) {
            required_features |= wgpu::Features::POLYGON_MODE_LINE;
        } else {
            log::warn!("wgpu adapter lacks POLYGON_MODE_LINE, wireframe modes won't work");
        }
        if adapter_features.contains(wgpu::Features::POLYGON_MODE_POINT) {
            required_features |= wgpu::Features::POLYGON_MODE_POINT;
        } else {
            log::warn!("wgpu adapter lacks POLYGON_MODE_POINT, points mode won't work");
        }

        let mut limits = adapter.limits();
        let adapter_max_pc = limits.max_push_constant_size;
        if adapter_features.contains(wgpu::Features::PUSH_CONSTANTS) && adapter_max_pc >= 128 {
            limits.max_push_constant_size = 128;
        }

        wgpu::DeviceDescriptor {
            label: Some(label),
            required_features,
            required_limits: limits,
            ..Default::default()
        }
    }

    /// Wrap an externally owned adapter/device/queue into an HGI backend.
    pub fn from_existing(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        device.on_uncaptured_error(std::sync::Arc::new(|err| {
            log::error!("wgpu uncaptured error: {err}");
        }));

        let adapter = Arc::new(adapter);
        let device = Arc::new(device);
        let queue = Arc::new(queue);
        let capabilities = WgpuCapabilities::new(&adapter).base;
        let mipmap_gen = Arc::new(MipmapGenerator::new(&device));

        Self {
            instance,
            adapter,
            device,
            queue,
            capabilities,
            id_counter: AtomicU64::new(1),
            frame_depth: AtomicI32::new(0),
            deferred_destroy_buffers: Vec::new(),
            deferred_destroy_textures: Vec::new(),
            deferred_destroy_samplers: Vec::new(),
            deferred_destroy_shader_functions: Vec::new(),
            deferred_destroy_shader_programs: Vec::new(),
            mipmap_gen,
            deferred_destroy_resource_bindings: Vec::new(),
            deferred_destroy_graphics_pipelines: Vec::new(),
            deferred_destroy_compute_pipelines: Vec::new(),
            surface: None,
            surface_config: None,
        }
    }

    /// Async initialization of wgpu device stack.
    async fn new_async() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            Ok(a) => a,
            Err(e) => {
                log::error!("wgpu request_adapter failed: {e}");
                return None;
            }
        };
        log::info!("wgpu adapter: {:?}", adapter.get_info());

        let (device, queue) = match adapter
            .request_device(&Self::create_device_descriptor(&adapter, "HgiWgpu"))
            .await
        {
            Ok(dq) => dq,
            Err(e) => {
                log::error!("wgpu request_device failed: {e}");
                return None;
            }
        };
        // Override wgpu's default fatal error handler with a non-fatal one.
        // This prevents panics during pipeline creation probes and similar operations.
        device.on_uncaptured_error(std::sync::Arc::new(|err| {
            log::error!("wgpu uncaptured error: {err}");
        }));

        log::info!(
            "wgpu device created OK (push_constants: {}, max_size: {})",
            adapter.features().contains(wgpu::Features::PUSH_CONSTANTS),
            device.limits().max_push_constant_size,
        );

        let capabilities = WgpuCapabilities::new(&adapter).base;
        let mipmap_gen = Arc::new(MipmapGenerator::new(&device));

        Some(Self {
            instance,
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            capabilities,
            id_counter: AtomicU64::new(1),
            frame_depth: AtomicI32::new(0),
            deferred_destroy_buffers: Vec::new(),
            deferred_destroy_textures: Vec::new(),
            deferred_destroy_samplers: Vec::new(),
            deferred_destroy_shader_functions: Vec::new(),
            deferred_destroy_shader_programs: Vec::new(),
            mipmap_gen,
            deferred_destroy_resource_bindings: Vec::new(),
            deferred_destroy_graphics_pipelines: Vec::new(),
            deferred_destroy_compute_pipelines: Vec::new(),
            surface: None,
            surface_config: None,
        })
    }

    /// Get shared reference to the wgpu device
    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    /// Get shared reference to the wgpu queue
    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }

    /// Get shared reference to the wgpu adapter
    pub fn adapter(&self) -> &Arc<wgpu::Adapter> {
        &self.adapter
    }

    /// Check whether a given HGI format + usage combination is supported by
    /// the physical adapter.  Uses `get_texture_format_features()` to query
    /// the actual driver/downlevel capabilities rather than assuming support.
    pub fn is_format_supported(
        &self,
        format: usd_hgi::HgiFormat,
        usage: usd_hgi::HgiTextureUsage,
    ) -> bool {
        use crate::conversions::{to_wgpu_texture_format, to_wgpu_texture_usages};
        #[allow(unused_imports)]
        use usd_hgi::{HgiFormat, HgiTextureUsage};
        let wgpu_fmt = to_wgpu_texture_format(format);
        let wgpu_usage = to_wgpu_texture_usages(usage);
        let caps = self.adapter.get_texture_format_features(wgpu_fmt);
        // Check that every requested usage bit is allowed by the adapter
        caps.allowed_usages.contains(wgpu_usage)
    }

    /// Get shared reference to the mipmap generator
    pub fn mipmap_gen(&self) -> &Arc<MipmapGenerator> {
        &self.mipmap_gen
    }

    /// Get shared reference to the wgpu instance (for surface creation)
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// Create and configure a surface from a window handle.
    ///
    /// The surface target must implement wgpu::WindowHandle (raw-window-handle).
    /// Call this after new() to enable window presentation.
    /// Headless/offscreen rendering works without calling this.
    pub fn set_surface<'w>(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
    ) -> Result<(), wgpu::CreateSurfaceError>
    where
        wgpu::SurfaceTarget<'w>: 'static,
    {
        let surface = self.instance.create_surface(target)?;

        let caps = surface.get_capabilities(&self.adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);
        Ok(())
    }

    /// Resize the surface (call when window resizes).
    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if let (Some(surface), Some(config)) = (&self.surface, &mut self.surface_config) {
            config.width = width.max(1);
            config.height = height.max(1);
            surface.configure(&self.device, config);
        }
    }

    /// Get the current surface texture for rendering.
    /// Returns None if no surface configured or acquisition fails.
    pub fn get_current_texture(&self) -> Option<wgpu::SurfaceTexture> {
        self.surface.as_ref()?.get_current_texture().ok()
    }

    /// Get the surface texture format (for render pipeline color target).
    pub fn surface_format(&self) -> Option<wgpu::TextureFormat> {
        self.surface_config.as_ref().map(|c| c.format)
    }

    /// Present the current frame to the window surface.
    /// Call after submitting all render commands for the frame.
    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        frame.present();
    }

    /// Check if a surface is configured for window rendering.
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    /// Check if wgpu backend can run on current hardware (static check).
    ///
    /// Attempts to create a temporary adapter to verify GPU availability.
    /// Unlike GL (which assumes context exists), wgpu may fail if no
    /// compatible GPU/driver is found.
    pub fn check_backend_support() -> bool {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .is_ok()
        })
    }

    /// Process deferred resource deletions
    fn process_deferred_deletions(&mut self) {
        // Dropping handles decrements Arc refcount, releasing GPU resources
        self.deferred_destroy_buffers.clear();
        self.deferred_destroy_textures.clear();
        self.deferred_destroy_samplers.clear();
        self.deferred_destroy_shader_functions.clear();
        self.deferred_destroy_shader_programs.clear();
        self.deferred_destroy_resource_bindings.clear();
        self.deferred_destroy_graphics_pipelines.clear();
        self.deferred_destroy_compute_pipelines.clear();
    }
}

impl Hgi for HgiWgpu {
    fn is_backend_supported(&self) -> bool {
        // If we got this far, adapter and device are valid
        true
    }

    fn capabilities(&self) -> &HgiCapabilities {
        &self.capabilities
    }

    // --- Resource Creation ---

    fn create_buffer(
        &mut self,
        desc: &HgiBufferDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiBufferHandle {
        let buffer = Arc::new(WgpuBuffer::new(&self.device, desc, initial_data));
        let id = self.unique_id();
        HgiBufferHandle::new(buffer, id)
    }

    fn create_texture(
        &mut self,
        desc: &HgiTextureDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiTextureHandle {
        let texture = Arc::new(WgpuTexture::new(
            &self.device,
            &self.queue,
            desc,
            initial_data,
        ));
        let id = self.unique_id();
        HgiTextureHandle::new(texture, id)
    }

    fn create_texture_view(&mut self, desc: &HgiTextureViewDesc) -> HgiTextureViewHandle {
        use crate::resolve::*;

        // Resolve source texture to WgpuTexture
        let source_tex = match resolve_wgpu_texture(&desc.source_texture) {
            Some(tex) => tex,
            None => {
                log::error!("create_texture_view: source texture is null or not WgpuTexture");
                return HgiTextureViewHandle::null();
            }
        };

        // Create a texture view with specific mip/layer range
        let view = source_tex
            .wgpu_texture()
            .create_view(&wgpu::TextureViewDescriptor {
                label: if desc.debug_name.is_empty() {
                    None
                } else {
                    Some(desc.debug_name.as_str())
                },
                format: Some(crate::conversions::to_wgpu_texture_format(desc.format)),
                dimension: None, // infer from texture
                usage: None,     // inherit from source texture
                aspect: wgpu::TextureAspect::All,
                base_mip_level: desc.source_first_mip as u32,
                mip_level_count: Some(desc.source_mip_count as u32),
                base_array_layer: desc.source_first_layer as u32,
                array_layer_count: Some(desc.source_layer_count as u32),
            });

        // Create a WgpuTexture that wraps just the view
        let texture = Arc::new(WgpuTexture::from_view(
            view,
            source_tex.descriptor().clone(),
        ));
        let id = self.unique_id();
        HgiTextureViewHandle::new(texture, id)
    }

    fn create_sampler(&mut self, desc: &HgiSamplerDesc) -> HgiSamplerHandle {
        let sampler = Arc::new(WgpuSampler::new(&self.device, desc));
        let id = self.unique_id();
        HgiSamplerHandle::new(sampler, id)
    }

    fn create_shader_function(&mut self, desc: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle {
        let function = Arc::new(WgpuShaderFunction::new(&self.device, desc));
        let id = self.unique_id();
        HgiShaderFunctionHandle::new(function, id)
    }

    fn create_shader_program(&mut self, desc: &HgiShaderProgramDesc) -> HgiShaderProgramHandle {
        let program = Arc::new(WgpuShaderProgram::new(desc));
        let id = self.unique_id();
        HgiShaderProgramHandle::new(program, id)
    }

    fn create_resource_bindings(
        &mut self,
        desc: &HgiResourceBindingsDesc,
    ) -> HgiResourceBindingsHandle {
        use crate::resolve::*;

        // Resolve buffer handles to WgpuBuffer instances
        let mut buffers: Vec<Vec<&WgpuBuffer>> = Vec::new();
        for buf_bind in &desc.buffer_bindings {
            let mut buf_list = Vec::new();
            for buf_handle in &buf_bind.buffers {
                if let Some(buf) = resolve_wgpu_buffer(buf_handle) {
                    buf_list.push(buf);
                }
            }
            buffers.push(buf_list);
        }

        // Resolve texture handles to WgpuTexture instances
        let mut textures: Vec<Vec<&WgpuTexture>> = Vec::new();
        let mut samplers: Vec<Vec<&WgpuSampler>> = Vec::new();
        for tex_bind in &desc.texture_bindings {
            let mut tex_list = Vec::new();
            for tex_handle in &tex_bind.textures {
                if let Some(tex) = resolve_wgpu_texture(tex_handle) {
                    tex_list.push(tex);
                }
            }
            textures.push(tex_list);

            let mut smp_list = Vec::new();
            for smp_handle in &tex_bind.samplers {
                if let Some(smp) = resolve_wgpu_sampler(smp_handle) {
                    smp_list.push(smp);
                }
            }
            samplers.push(smp_list);
        }

        // Create resource bindings with resolved wgpu resources
        let bindings = Arc::new(WgpuResourceBindings::new(
            &self.device,
            desc,
            &buffers,
            &textures,
            &samplers,
        ));
        let id = self.unique_id();
        HgiResourceBindingsHandle::new(bindings, id)
    }

    fn create_graphics_pipeline(
        &mut self,
        desc: &HgiGraphicsPipelineDesc,
    ) -> HgiGraphicsPipelineHandle {
        let id = self.unique_id();

        // Extract shader program from descriptor
        let shader_program = match desc.shader_program.get() {
            Some(prog) => prog,
            None => {
                log::warn!("create_graphics_pipeline: null shader program, creating stub");
                let pipeline = Arc::new(WgpuGraphicsPipeline::new_stub(desc));
                return HgiGraphicsPipelineHandle::new(pipeline, id);
            }
        };

        // Downcast to WgpuShaderProgram to access shader functions
        let wgpu_program = match shader_program.as_any().downcast_ref::<WgpuShaderProgram>() {
            Some(prog) => prog,
            None => {
                log::warn!(
                    "create_graphics_pipeline: shader program is not WgpuShaderProgram, creating stub"
                );
                let pipeline = Arc::new(WgpuGraphicsPipeline::new_stub(desc));
                return HgiGraphicsPipelineHandle::new(pipeline, id);
            }
        };

        // Extract vertex and fragment shader functions
        let shader_desc = wgpu_program.descriptor();
        let mut vertex_fn: Option<&WgpuShaderFunction> = None;
        let mut fragment_fn: Option<&WgpuShaderFunction> = None;

        for fn_handle in &shader_desc.shader_functions {
            if let Some(func) = fn_handle.get() {
                let func_desc = func.descriptor();

                if let Some(wgpu_func) = func.as_any().downcast_ref::<WgpuShaderFunction>() {
                    match func_desc.shader_stage {
                        HgiShaderStage::VERTEX => vertex_fn = Some(wgpu_func),
                        HgiShaderStage::FRAGMENT => fragment_fn = Some(wgpu_func),
                        _ => {}
                    }
                }
            }
        }

        // Verify we have at least a vertex shader
        let vertex_module = match vertex_fn {
            Some(v) => v,
            None => {
                log::warn!("create_graphics_pipeline: no vertex shader found, creating stub");
                let pipeline = Arc::new(WgpuGraphicsPipeline::new_stub(desc));
                return HgiGraphicsPipelineHandle::new(pipeline, id);
            }
        };

        // Create real pipeline with shader modules
        // wgpu auto-layout: pass empty slice and use layout: None in pipeline creation
        let pipeline = Arc::new(WgpuGraphicsPipeline::new(
            &self.device,
            desc,
            vertex_module,
            fragment_fn,
        ));

        HgiGraphicsPipelineHandle::new(pipeline, id)
    }

    fn create_compute_pipeline(
        &mut self,
        desc: &HgiComputePipelineDesc,
    ) -> HgiComputePipelineHandle {
        let id = self.unique_id();

        // Extract shader program from descriptor
        let shader_program = match desc.shader_program.get() {
            Some(prog) => prog,
            None => {
                log::warn!("create_compute_pipeline: null shader program, creating stub");
                let pipeline = Arc::new(WgpuComputePipeline::new_stub(desc));
                return HgiComputePipelineHandle::new(pipeline, id);
            }
        };

        // Downcast to WgpuShaderProgram to access shader functions
        let wgpu_program = match shader_program.as_any().downcast_ref::<WgpuShaderProgram>() {
            Some(prog) => prog,
            None => {
                log::warn!(
                    "create_compute_pipeline: shader program is not WgpuShaderProgram, creating stub"
                );
                let pipeline = Arc::new(WgpuComputePipeline::new_stub(desc));
                return HgiComputePipelineHandle::new(pipeline, id);
            }
        };

        // Extract compute shader function
        let shader_desc = wgpu_program.descriptor();
        let mut compute_fn: Option<&WgpuShaderFunction> = None;

        for fn_handle in &shader_desc.shader_functions {
            if let Some(func) = fn_handle.get() {
                let func_desc = func.descriptor();

                if func_desc.shader_stage == HgiShaderStage::COMPUTE {
                    if let Some(wgpu_func) = func.as_any().downcast_ref::<WgpuShaderFunction>() {
                        compute_fn = Some(wgpu_func);
                        break;
                    }
                }
            }
        }

        // Verify we have a compute shader
        let compute_module = match compute_fn {
            Some(c) => c,
            None => {
                log::warn!("create_compute_pipeline: no compute shader found, creating stub");
                let pipeline = Arc::new(WgpuComputePipeline::new_stub(desc));
                return HgiComputePipelineHandle::new(pipeline, id);
            }
        };

        // Create real pipeline with compute shader module
        // wgpu auto-layout: pass no layouts and use layout: None in pipeline creation
        let pipeline = Arc::new(WgpuComputePipeline::new(&self.device, desc, compute_module));

        HgiComputePipelineHandle::new(pipeline, id)
    }

    // --- Resource Destruction ---
    // Defer actual deletion to garbage_collect() to avoid destroying
    // resources that may still be in-flight on the GPU.

    fn destroy_buffer(&mut self, buffer: &HgiBufferHandle) {
        self.deferred_destroy_buffers.push(buffer.clone());
    }

    fn destroy_texture(&mut self, texture: &HgiTextureHandle) {
        self.deferred_destroy_textures.push(texture.clone());
    }

    fn destroy_sampler(&mut self, sampler: &HgiSamplerHandle) {
        self.deferred_destroy_samplers.push(sampler.clone());
    }

    fn destroy_shader_function(&mut self, function: &HgiShaderFunctionHandle) {
        self.deferred_destroy_shader_functions
            .push(function.clone());
    }

    fn destroy_shader_program(&mut self, program: &HgiShaderProgramHandle) {
        self.deferred_destroy_shader_programs.push(program.clone());
    }

    fn destroy_resource_bindings(&mut self, bindings: &HgiResourceBindingsHandle) {
        self.deferred_destroy_resource_bindings
            .push(bindings.clone());
    }

    fn destroy_graphics_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle) {
        self.deferred_destroy_graphics_pipelines
            .push(pipeline.clone());
    }

    fn destroy_compute_pipeline(&mut self, pipeline: &HgiComputePipelineHandle) {
        self.deferred_destroy_compute_pipelines
            .push(pipeline.clone());
    }

    // --- Command Buffer Creation ---

    fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
        Box::new(WgpuBlitCmds::new(
            self.device.clone(),
            self.queue.clone(),
            self.mipmap_gen.clone(),
        ))
    }

    fn create_graphics_cmds(&mut self, desc: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds> {
        Box::new(WgpuGraphicsCmds::new(
            self.device.clone(),
            self.queue.clone(),
            desc.clone(),
        ))
    }

    fn create_compute_cmds(
        &mut self,
        _desc: &usd_hgi::HgiComputeCmdsDesc,
    ) -> Box<dyn HgiComputeCmds> {
        Box::new(WgpuComputeCmds::new(
            self.device.clone(),
            self.queue.clone(),
        ))
    }

    // --- Command Submission ---

    fn submit_cmds(&mut self, mut cmds: Box<dyn HgiCmds>, wait: HgiSubmitWaitType) {
        // Execute any deferred GPU work recorded in the command buffer
        cmds.execute_submit();

        if wait == HgiSubmitWaitType::WaitUntilCompleted {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }

        // GC if not inside a frame
        if self.frame_depth.load(Ordering::Acquire) == 0 {
            self.process_deferred_deletions();
        }
    }

    // --- Utility ---

    fn unique_id(&mut self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }

    fn wait_for_idle(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    fn get_api_name(&self) -> &str {
        "wgpu"
    }

    fn start_frame(&mut self) {
        self.frame_depth.fetch_add(1, Ordering::AcqRel);
    }

    fn end_frame(&mut self) {
        let prev_depth = self.frame_depth.fetch_sub(1, Ordering::AcqRel);
        if prev_depth == 1 {
            self.garbage_collect();
        }
    }

    fn garbage_collect(&mut self) {
        self.process_deferred_deletions();
        // Poll device to release completed command buffer resources
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    fn device_identity(&self) -> u64 {
        Arc::as_ptr(&self.device) as u64
    }
}

/// Factory: create a new wgpu HGI backend instance.
///
/// Returns `None` if no suitable GPU adapter is found.
pub fn create_hgi_wgpu() -> Option<HgiWgpu> {
    HgiWgpu::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_hgi_wgpu() {
        // May fail in CI without GPU -- that's OK
        if let Some(hgi) = create_hgi_wgpu() {
            assert!(hgi.is_backend_supported());
            assert_eq!(hgi.get_api_name(), "wgpu");
        }
    }

    #[test]
    fn test_unique_id() {
        if let Some(mut hgi) = create_hgi_wgpu() {
            let id1 = hgi.unique_id();
            let id2 = hgi.unique_id();
            assert_ne!(id1, id2);
            assert!(id2 > id1);
        }
    }
}
