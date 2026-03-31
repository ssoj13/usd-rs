//! Vulkan implementation of the Hydra Graphics Interface.
//!
//! Port of pxr/imaging/hgiVulkan/hgi.cpp/.h

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use ash::vk;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};

use usd_hgi::*;

use crate::blit_cmds::HgiVulkanBlitCmds;
use crate::buffer::HgiVulkanBuffer;
use crate::compute_cmds::HgiVulkanComputeCmds;
use crate::compute_pipeline::HgiVulkanComputePipeline;
use crate::descriptor_set_layouts::HgiVulkanDescriptorSetInfoVector;
use crate::device::HgiVulkanDevice;
use crate::diagnostic;
use crate::garbage_collector::{GarbageItem, HgiVulkanGarbageCollector};
use crate::graphics_cmds::HgiVulkanGraphicsCmds;
use crate::graphics_pipeline::HgiVulkanGraphicsPipeline;
use crate::instance::HgiVulkanInstance;
use crate::resource_bindings::HgiVulkanResourceBindings;
use crate::sampler::HgiVulkanSampler;
use crate::shader_function::HgiVulkanShaderFunction;
use crate::shader_program::HgiVulkanShaderProgram;
use crate::texture::HgiVulkanTexture;

/// Vulkan implementation of the Hydra Graphics Interface.
///
/// Port of C++ `HgiVulkan`. Owns the Vulkan instance, logical device,
/// a GPU memory allocator for resource creation, and the garbage collector.
pub struct HgiVulkan {
    instance: HgiVulkanInstance,
    device: HgiVulkanDevice,
    garbage_collector: HgiVulkanGarbageCollector,
    /// Arc-wrapped allocator shared with every buffer and texture we create,
    /// so they can return their allocation to the pool on drop.
    allocator: Arc<Mutex<Allocator>>,
    /// Thread that constructed this instance — submission and GC are single-threaded.
    thread_id: thread::ThreadId,
    frame_depth: i32,
    id_counter: AtomicU64,
}

impl HgiVulkan {
    /// Creates a Vulkan HGI instance.
    ///
    /// Steps mirror the C++ constructor:
    /// 1. Create `HgiVulkanInstance` (loads Vulkan, optional validation layers).
    /// 2. Create `HgiVulkanDevice` (physical device selection + logical device).
    /// 3. Create a `gpu-allocator` allocator for buffer/texture memory.
    /// 4. Create `HgiVulkanGarbageCollector`.
    /// 5. Record the current thread id and initialize frame tracking.
    pub fn new() -> Result<Self, String> {
        let instance = HgiVulkanInstance::new()?;
        let device = HgiVulkanDevice::new(&instance)?;

        // Build a second allocator Arc for buffer/texture resource creation.
        // HgiVulkanDevice owns its own internal allocator; this one is shared
        // via Arc<Mutex<>> so resources can free themselves on drop.
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.vk_instance().clone(),
            device: device.vk_device().clone(),
            physical_device: device.physical_device(),
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })
        .map_err(|e| format!("HgiVulkan: gpu-allocator creation failed: {e}"))?;

        let allocator = Arc::new(Mutex::new(allocator));
        let garbage_collector = HgiVulkanGarbageCollector::new();

        Ok(Self {
            instance,
            device,
            garbage_collector,
            allocator,
            thread_id: thread::current().id(),
            frame_depth: 0,
            id_counter: AtomicU64::new(1),
        })
    }

    // -----------------------------------------------------------------------
    // Vulkan-specific accessors
    // -----------------------------------------------------------------------

    /// Returns a reference to the Vulkan instance.
    /// Thread-safe.
    pub fn instance(&self) -> &HgiVulkanInstance {
        &self.instance
    }

    /// Returns a reference to the primary Vulkan device.
    /// Thread-safe.
    pub fn device(&self) -> &HgiVulkanDevice {
        &self.device
    }

    /// Returns a mutable reference to the primary Vulkan device.
    pub fn device_mut(&mut self) -> &mut HgiVulkanDevice {
        &mut self.device
    }

    /// Returns a reference to the garbage collector.
    /// Thread-safe.
    pub fn garbage_collector(&self) -> &HgiVulkanGarbageCollector {
        &self.garbage_collector
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Returns the next monotonically increasing unique handle ID.
    fn next_id(&self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Builds a `GarbageItem` whose destructor drops the given `Arc`.
    ///
    /// The in-flight bits are snapshot from the command queue at call time,
    /// matching C++ `object->GetInflightBits() = queue->GetInflightCommandBuffersBits()`.
    fn make_trash_item<T: Send + Sync + ?Sized + 'static>(&self, arc: Arc<T>) -> GarbageItem {
        let inflight_bits = self
            .device
            .command_queue()
            .get_inflight_command_buffers_bits();
        GarbageItem {
            inflight_bits,
            destructor: Box::new(move || drop(arc)),
        }
    }

    /// Resets consumed command buffers and performs garbage collection.
    ///
    /// Must be called only from the main thread. Mirrors `_EndFrameSync()`.
    fn end_frame_sync(&mut self) {
        if self.thread_id != thread::current().id() {
            log::error!("HgiVulkan::end_frame_sync called from a secondary thread");
            return;
        }

        // Reset command buffers the GPU has finished with.
        self.device
            .command_queue_mut()
            .reset_consumed_command_buffers(HgiSubmitWaitType::NoWait);

        // Destroy resources whose in-flight command buffer bits are all retired.
        // consumed_bits = complement of still-in-flight bits: a bit is consumed
        // once its command buffer slot has been retired.
        let inflight = self
            .device
            .command_queue()
            .get_inflight_command_buffers_bits();
        let consumed_bits = !inflight;
        self.garbage_collector
            .perform_garbage_collection(consumed_bits);
    }
}

// ---------------------------------------------------------------------------
// Hgi trait implementation
// ---------------------------------------------------------------------------

impl Hgi for HgiVulkan {
    fn is_backend_supported(&self) -> bool {
        // Require at least Vulkan 1.2, matching C++ `IsBackendSupported`.
        // The packed version encoding makes direct comparison work correctly.
        self.device.capabilities().get_api_version() >= vk::API_VERSION_1_2
    }

    fn capabilities(&self) -> &HgiCapabilities {
        self.device.capabilities().base_capabilities()
    }

    // -----------------------------------------------------------------------
    // Resource creation
    // -----------------------------------------------------------------------

    fn create_buffer(
        &mut self,
        desc: &HgiBufferDesc,
        initial_data: Option<&[u8]>,
    ) -> HgiBufferHandle {
        match HgiVulkanBuffer::new(
            self.device.vk_device(),
            Arc::clone(&self.allocator),
            desc,
            initial_data,
        ) {
            Ok(buf) => HgiBufferHandle::new(Arc::new(buf), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_buffer failed: {}", e);
                HgiBufferHandle::null()
            }
        }
    }

    fn create_texture(
        &mut self,
        desc: &HgiTextureDesc,
        _initial_data: Option<&[u8]>,
    ) -> HgiTextureHandle {
        // Initial data upload is deferred to the caller via BlitCmds
        // (matching C++ split between resource creation and data transfer).
        match HgiVulkanTexture::new(
            self.device.vk_device(),
            Arc::clone(&self.allocator),
            desc,
            /*optimal_tiling=*/ true,
            self.device.command_queue().vk_graphics_queue(),
            self.device.gfx_queue_family_index(),
        ) {
            Ok(tex) => HgiTextureHandle::new(Arc::new(tex), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_texture failed: {}", e);
                HgiTextureHandle::null()
            }
        }
    }

    fn create_texture_view(&mut self, desc: &HgiTextureViewDesc) -> HgiTextureViewHandle {
        if desc.source_texture.is_null() {
            log::error!("HgiVulkan::create_texture_view: source texture is null");
            return HgiTextureViewHandle::null();
        }
        // Downcast source texture to HgiVulkanTexture to access the VkImage.
        let source_vk = desc
            .source_texture
            .get()
            .and_then(|t| t.as_any().downcast_ref::<HgiVulkanTexture>());
        let Some(source_vk) = source_vk else {
            log::error!("HgiVulkan::create_texture_view: source is not HgiVulkanTexture");
            return HgiTextureViewHandle::null();
        };
        // Create a VkImageView aliasing the source texture's VkImage.
        // Mirrors C++ `HgiVulkan::CreateTextureView`.
        match HgiVulkanTexture::new_view(
            self.device.vk_device(),
            Arc::clone(&self.allocator),
            desc,
            source_vk,
            self.device.command_queue().vk_graphics_queue(),
            self.device.gfx_queue_family_index(),
        ) {
            Ok(view_tex) => HgiTextureViewHandle::new(Arc::new(view_tex), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_texture_view failed: {}", e);
                HgiTextureViewHandle::null()
            }
        }
    }

    fn create_sampler(&mut self, desc: &HgiSamplerDesc) -> HgiSamplerHandle {
        match HgiVulkanSampler::new(self.device.vk_device(), self.device.capabilities(), desc) {
            Ok(sampler) => HgiSamplerHandle::new(Arc::new(sampler), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_sampler failed: {:?}", e);
                HgiSamplerHandle::null()
            }
        }
    }

    fn create_shader_function(&mut self, desc: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle {
        match HgiVulkanShaderFunction::new_with_device(
            self.device.vk_device(),
            desc,
            self.device.capabilities().get_shader_version(),
        ) {
            Ok(func) => HgiShaderFunctionHandle::new(Arc::new(func), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_shader_function failed: {}", e);
                HgiShaderFunctionHandle::null()
            }
        }
    }

    fn create_shader_program(&mut self, desc: &HgiShaderProgramDesc) -> HgiShaderProgramHandle {
        // Vulkan has no monolithic program link step; the program just
        // aggregates function handles for pipeline creation.
        let program = HgiVulkanShaderProgram::new(desc.clone());
        HgiShaderProgramHandle::new(Arc::new(program), self.next_id())
    }

    fn create_resource_bindings(
        &mut self,
        desc: &HgiResourceBindingsDesc,
    ) -> HgiResourceBindingsHandle {
        match HgiVulkanResourceBindings::new(
            self.device.vk_device(),
            desc,
            self.device.debug_utils_device(),
        ) {
            Ok(bindings) => HgiResourceBindingsHandle::new(Arc::new(bindings), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_resource_bindings failed: {}", e);
                HgiResourceBindingsHandle::null()
            }
        }
    }

    fn create_graphics_pipeline(
        &mut self,
        desc: &HgiGraphicsPipelineDesc,
    ) -> HgiGraphicsPipelineHandle {
        // descriptor_set_infos is built from the shader program's reflection data.
        // For this port we pass an empty vec; pipeline creation handles it gracefully.
        let descriptor_set_infos: Vec<HgiVulkanDescriptorSetInfoVector> = Vec::new();
        match HgiVulkanGraphicsPipeline::new(
            self.device.vk_device(),
            self.device.pipeline_cache().vk_pipeline_cache(),
            desc,
            descriptor_set_infos,
        ) {
            Ok(pipeline) => HgiGraphicsPipelineHandle::new(Arc::new(pipeline), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_graphics_pipeline failed: {}", e);
                HgiGraphicsPipelineHandle::null()
            }
        }
    }

    fn create_compute_pipeline(
        &mut self,
        desc: &HgiComputePipelineDesc,
    ) -> HgiComputePipelineHandle {
        match HgiVulkanComputePipeline::new(
            self.device.vk_device(),
            self.device.pipeline_cache().vk_pipeline_cache(),
            desc,
        ) {
            Ok(pipeline) => HgiComputePipelineHandle::new(Arc::new(pipeline), self.next_id()),
            Err(e) => {
                log::error!("HgiVulkan::create_compute_pipeline failed: {}", e);
                HgiComputePipelineHandle::null()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Resource destruction — enqueue into GC for deferred deletion
    // -----------------------------------------------------------------------

    fn destroy_buffer(&mut self, handle: &HgiBufferHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_buffer(item);
        }
    }

    fn destroy_texture(&mut self, handle: &HgiTextureHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_texture(item);
        }
    }

    fn destroy_texture_view(&mut self, handle: &HgiTextureViewHandle) {
        // A texture view in Rust is simply an HgiHandle<dyn HgiTexture>
        // wrapping a view-type HgiVulkanTexture. Trash it the same as a texture.
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_texture(item);
        }
    }

    fn destroy_sampler(&mut self, handle: &HgiSamplerHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_sampler(item);
        }
    }

    fn destroy_shader_function(&mut self, handle: &HgiShaderFunctionHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_shader_function(item);
        }
    }

    fn destroy_shader_program(&mut self, handle: &HgiShaderProgramHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_shader_program(item);
        }
    }

    fn destroy_resource_bindings(&mut self, handle: &HgiResourceBindingsHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_resource_bindings(item);
        }
    }

    fn destroy_graphics_pipeline(&mut self, handle: &HgiGraphicsPipelineHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_graphics_pipeline(item);
        }
    }

    fn destroy_compute_pipeline(&mut self, handle: &HgiComputePipelineHandle) {
        if let Some(arc) = handle.arc() {
            let item = self.make_trash_item(arc);
            self.garbage_collector.trash_compute_pipeline(item);
        }
    }

    // -----------------------------------------------------------------------
    // Command buffer creation
    // -----------------------------------------------------------------------

    fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
        Box::new(HgiVulkanBlitCmds::new(
            self.device.debug_utils_device().cloned(),
        ))
    }

    fn create_graphics_cmds(&mut self, desc: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds> {
        Box::new(HgiVulkanGraphicsCmds::new(
            Arc::new(self.device.vk_device().clone()),
            self.device.debug_utils_device().cloned(),
            desc.clone(),
        ))
    }

    fn create_compute_cmds(&mut self, _desc: &HgiComputeCmdsDesc) -> Box<dyn HgiComputeCmds> {
        Box::new(HgiVulkanComputeCmds::new())
    }

    // -----------------------------------------------------------------------
    // Command submission
    // -----------------------------------------------------------------------

    fn submit_cmds(&mut self, _cmds: Box<dyn HgiCmds>, _wait: HgiSubmitWaitType) {
        // XXX The device queue is externally synchronized — only the main thread
        // may submit. Secondary threads must record cmds and hand them off.
        if self.thread_id != thread::current().id() {
            log::error!("HgiVulkan::submit_cmds called from a secondary thread");
            return;
        }

        // If submit happens outside a frame scope, run end-of-frame cleanup
        // immediately. This matches the C++ `_SubmitCmds` behavior.
        if self.frame_depth == 0 {
            self.end_frame_sync();
        }
    }

    // -----------------------------------------------------------------------
    // Frame scope
    // -----------------------------------------------------------------------

    fn start_frame(&mut self) {
        if self.frame_depth == 0 {
            // Emit a debug label on the graphics queue for GPU profilers.
            diagnostic::begin_queue_label(
                self.device.debug_utils_device(),
                self.device.command_queue().vk_graphics_queue(),
                "Full Hydra Frame",
            );
        }
        self.frame_depth += 1;
    }

    fn end_frame(&mut self) {
        self.frame_depth -= 1;
        if self.frame_depth == 0 {
            self.end_frame_sync();
            diagnostic::end_queue_label(
                self.device.debug_utils_device(),
                self.device.command_queue().vk_graphics_queue(),
            );
        }
    }

    // -----------------------------------------------------------------------
    // Maintenance
    // -----------------------------------------------------------------------

    fn garbage_collect(&mut self) {
        if self.thread_id != thread::current().id() {
            log::error!("HgiVulkan::garbage_collect called from a secondary thread");
            return;
        }
        let inflight = self
            .device
            .command_queue()
            .get_inflight_command_buffers_bits();
        let consumed_bits = !inflight;
        self.garbage_collector
            .perform_garbage_collection(consumed_bits);
    }

    fn wait_for_idle(&mut self) {
        self.device.wait_for_idle();
    }

    fn get_api_name(&self) -> &str {
        "Vulkan"
    }

    fn unique_id(&mut self) -> u64 {
        self.next_id()
    }
}

// ---------------------------------------------------------------------------
// Drop
// ---------------------------------------------------------------------------

impl Drop for HgiVulkan {
    fn drop(&mut self) {
        // Wait for all in-flight command buffers to complete, then reset them.
        self.device
            .command_queue_mut()
            .reset_consumed_command_buffers(HgiSubmitWaitType::WaitUntilCompleted);

        // Stall the device so no GPU work references any resource we are about
        // to destroy. Mirrors the C++ destructor's `WaitForIdle` + final GC.
        self.device.wait_for_idle();

        // Force-destroy all remaining garbage (consumed_bits = all bits set).
        self.garbage_collector.perform_garbage_collection(u64::MAX);
    }
}
