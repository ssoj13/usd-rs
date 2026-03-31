//! Metal implementation of HGI. Port of pxr/imaging/hgiMetal/hgi

use super::blit_cmds::HgiMetalBlitCmds;
use super::buffer::HgiMetalBuffer;
use super::capabilities::{HgiMetalCapabilities, MetalApiVersion};
use super::compute_cmds::HgiMetalComputeCmds;
use super::compute_pipeline::HgiMetalComputePipeline;
use super::graphics_cmds::HgiMetalGraphicsCmds;
use super::graphics_pipeline::HgiMetalGraphicsPipeline;
use super::indirect_command_encoder::HgiMetalIndirectCommandEncoder;
use super::resource_bindings::HgiMetalResourceBindings;
use super::sampler::HgiMetalSampler;
use super::shader_function::HgiMetalShaderFunction;
use super::shader_program::HgiMetalShaderProgram;
use super::texture::HgiMetalTexture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use usd_hgi::*;

/// Command buffer commit wait type.
/// Mirrors C++ HgiMetal::CommitCommandBufferWaitType.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitCommandBufferWaitType {
    NoWait = 0,
    WaitUntilScheduled = 1,
    WaitUntilCompleted = 2,
}

/// Metal backend implementation of HGI.
/// Mirrors C++ HgiMetal.
pub struct HgiMetal {
    capabilities: HgiMetalCapabilities,
    indirect_command_encoder: HgiMetalIndirectCommandEncoder,
    id_counter: AtomicU64,
    frame_depth: i32,
    work_to_flush: bool,
    // On real Metal:
    // device: id<MTLDevice>,
    // command_queue: id<MTLCommandQueue>,
    // command_buffer: id<MTLCommandBuffer>,
    // capture_scope: id<MTLCaptureScope>,
    // arg_encoder_buffer: id<MTLArgumentEncoder>,
    // arg_encoder_sampler: id<MTLArgumentEncoder>,
    // arg_encoder_texture: id<MTLArgumentEncoder>,
    // free_arg_buffers: Stack<id<MTLBuffer>>,
    // active_arg_buffers: Vec<id<MTLBuffer>>,
    // current_cmds: *mut HgiCmds,
}

impl HgiMetal {
    /// Creates a new Metal HGI instance.
    /// On real Metal, takes optional id<MTLDevice>. If nil, creates system default.
    pub fn new() -> Self {
        Self {
            capabilities: HgiMetalCapabilities::new(),
            indirect_command_encoder: HgiMetalIndirectCommandEncoder::new(),
            id_counter: AtomicU64::new(1),
            frame_depth: 0,
            work_to_flush: false,
        }
    }

    /// Returns the primary Metal device.
    /// Mirrors C++ GetPrimaryDevice().
    /// Stub: returns 0 (no real device).
    pub fn get_primary_device(&self) -> u64 {
        0
    }

    /// Returns the command queue.
    /// Mirrors C++ GetQueue().
    /// Stub: returns 0.
    pub fn get_queue(&self) -> u64 {
        0
    }

    /// Returns the primary command buffer.
    /// Mirrors C++ GetPrimaryCommandBuffer(HgiCmds*, bool).
    /// Stub: returns 0.
    pub fn get_primary_command_buffer(&self) -> u64 {
        0
    }

    /// Returns a secondary command buffer.
    /// Mirrors C++ GetSecondaryCommandBuffer().
    /// Stub: returns 0.
    pub fn get_secondary_command_buffer(&self) -> u64 {
        0
    }

    /// Mark that there is work to flush.
    /// Mirrors C++ SetHasWork().
    pub fn set_has_work(&mut self) {
        self.work_to_flush = true;
    }

    /// Returns the Metal API version.
    /// Mirrors C++ GetAPIVersion().
    pub fn get_api_version(&self) -> MetalApiVersion {
        self.capabilities.get_api_version()
    }

    /// Commit the primary command buffer.
    /// Mirrors C++ CommitPrimaryCommandBuffer().
    pub fn commit_primary_command_buffer(
        &mut self,
        _wait_type: CommitCommandBufferWaitType,
        force_new_buffer: bool,
    ) {
        if !self.work_to_flush && !force_new_buffer {
            return;
        }
        // Stub: on real Metal, commits and creates new command buffer
        self.work_to_flush = false;
    }

    /// Commit a secondary command buffer.
    /// Mirrors C++ CommitSecondaryCommandBuffer().
    pub fn commit_secondary_command_buffer(&mut self, _wait_type: CommitCommandBufferWaitType) {
        // Stub: on real Metal, commits the secondary command buffer
        // and moves active arg buffers to free pool on completion
    }

    /// Release a secondary command buffer.
    /// Mirrors C++ ReleaseSecondaryCommandBuffer().
    pub fn release_secondary_command_buffer(&mut self) {
        // Stub: on real Metal, releases the command buffer
    }

    /// Returns the buffer argument encoder.
    /// Mirrors C++ GetBufferArgumentEncoder().
    /// Stub: returns 0.
    pub fn get_buffer_argument_encoder(&self) -> u64 {
        0
    }

    /// Returns the sampler argument encoder.
    /// Mirrors C++ GetSamplerArgumentEncoder().
    /// Stub: returns 0.
    pub fn get_sampler_argument_encoder(&self) -> u64 {
        0
    }

    /// Returns the texture argument encoder.
    /// Mirrors C++ GetTextureArgumentEncoder().
    /// Stub: returns 0.
    pub fn get_texture_argument_encoder(&self) -> u64 {
        0
    }

    /// Returns an argument buffer from the pool.
    /// Mirrors C++ GetArgBuffer().
    /// Stub: returns 0.
    pub fn get_arg_buffer(&mut self) -> u64 {
        0
    }

    /// Returns the Metal capabilities.
    /// Mirrors C++ GetCapabilities().
    pub fn metal_capabilities(&self) -> &HgiMetalCapabilities {
        &self.capabilities
    }

    /// Returns the indirect command encoder.
    /// Mirrors C++ GetIndirectCommandEncoder().
    pub fn indirect_command_encoder(&self) -> &HgiMetalIndirectCommandEncoder {
        &self.indirect_command_encoder
    }

    /// Returns a mutable reference to the indirect command encoder.
    pub fn indirect_command_encoder_mut(&mut self) -> &mut HgiMetalIndirectCommandEncoder {
        &mut self.indirect_command_encoder
    }
}

impl Default for HgiMetal {
    fn default() -> Self {
        Self::new()
    }
}

impl Hgi for HgiMetal {
    fn is_backend_supported(&self) -> bool {
        // Metal requires macOS 10.15+ or iOS 13.0+
        // On non-Apple platforms, always false
        cfg!(target_os = "macos") || cfg!(target_os = "ios")
    }

    fn capabilities(&self) -> &HgiCapabilities {
        self.capabilities.base_capabilities()
    }

    fn create_buffer(
        &mut self,
        desc: &HgiBufferDesc,
        _initial_data: Option<&[u8]>,
    ) -> HgiBufferHandle {
        HgiBufferHandle::new(
            Arc::new(HgiMetalBuffer::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_texture(
        &mut self,
        desc: &HgiTextureDesc,
        _initial_data: Option<&[u8]>,
    ) -> HgiTextureHandle {
        HgiTextureHandle::new(
            Arc::new(HgiMetalTexture::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_texture_view(&mut self, desc: &HgiTextureViewDesc) -> HgiTextureViewHandle {
        let src_texture = Arc::new(HgiMetalTexture::new_view(desc));
        HgiTextureViewHandle::new(src_texture, self.unique_id())
    }

    fn create_sampler(&mut self, desc: &HgiSamplerDesc) -> HgiSamplerHandle {
        HgiSamplerHandle::new(
            Arc::new(HgiMetalSampler::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_shader_function(&mut self, desc: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle {
        HgiShaderFunctionHandle::new(
            Arc::new(HgiMetalShaderFunction::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_shader_program(&mut self, desc: &HgiShaderProgramDesc) -> HgiShaderProgramHandle {
        HgiShaderProgramHandle::new(
            Arc::new(HgiMetalShaderProgram::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_resource_bindings(
        &mut self,
        desc: &HgiResourceBindingsDesc,
    ) -> HgiResourceBindingsHandle {
        HgiResourceBindingsHandle::new(
            Arc::new(HgiMetalResourceBindings::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_graphics_pipeline(
        &mut self,
        desc: &HgiGraphicsPipelineDesc,
    ) -> HgiGraphicsPipelineHandle {
        HgiGraphicsPipelineHandle::new(
            Arc::new(HgiMetalGraphicsPipeline::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn create_compute_pipeline(
        &mut self,
        desc: &HgiComputePipelineDesc,
    ) -> HgiComputePipelineHandle {
        HgiComputePipelineHandle::new(
            Arc::new(HgiMetalComputePipeline::new(desc.clone())),
            self.unique_id(),
        )
    }

    fn destroy_buffer(&mut self, _handle: &HgiBufferHandle) {}
    fn destroy_texture(&mut self, _handle: &HgiTextureHandle) {}
    fn destroy_texture_view(&mut self, _handle: &HgiTextureViewHandle) {}
    fn destroy_sampler(&mut self, _handle: &HgiSamplerHandle) {}
    fn destroy_shader_function(&mut self, _handle: &HgiShaderFunctionHandle) {}
    fn destroy_shader_program(&mut self, _handle: &HgiShaderProgramHandle) {}
    fn destroy_resource_bindings(&mut self, _handle: &HgiResourceBindingsHandle) {}
    fn destroy_graphics_pipeline(&mut self, _handle: &HgiGraphicsPipelineHandle) {}
    fn destroy_compute_pipeline(&mut self, _handle: &HgiComputePipelineHandle) {}

    fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
        Box::new(HgiMetalBlitCmds::new())
    }

    fn create_graphics_cmds(&mut self, _desc: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds> {
        Box::new(HgiMetalGraphicsCmds::new())
    }

    fn create_compute_cmds(&mut self, _desc: &HgiComputeCmdsDesc) -> Box<dyn HgiComputeCmds> {
        Box::new(HgiMetalComputeCmds::new())
    }

    fn submit_cmds(&mut self, _cmds: Box<dyn HgiCmds>, _wait: HgiSubmitWaitType) {
        // Stub: on real Metal, calls _SubmitCmds which commits the command buffer
    }

    fn unique_id(&mut self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }

    fn wait_for_idle(&mut self) {
        // Stub: on real Metal, commits and waits for command buffer completion
    }

    fn get_api_name(&self) -> &str {
        "Metal"
    }

    fn start_frame(&mut self) {
        if self.frame_depth == 0 {
            // Stub: on real Metal, begins capture scope
        }
        self.frame_depth += 1;
    }

    fn end_frame(&mut self) {
        self.frame_depth -= 1;
        if self.frame_depth == 0 {
            // Stub: on real Metal, ends capture scope
        }
    }

    fn get_indirect_command_encoder(&self) -> Option<&dyn HgiIndirectCommandEncoder> {
        Some(&self.indirect_command_encoder)
    }

    fn garbage_collect(&mut self) {
        // Metal's internal garbage collection handles resource cleanup
    }
}
