//! Vulkan compute command recording.
//!
//! Port of pxr/imaging/hgiVulkan/computeCmds.cpp/.h

#![allow(unsafe_code)]

use ash::vk;
use std::thread;
use usd_hgi::enums::{HgiComputeDispatch, HgiMemoryBarrier, HgiShaderStage, HgiSubmitWaitType};
use usd_hgi::{
    HgiCmds, HgiComputeCmds, HgiComputeCmdsDesc, HgiComputeDispatchOp, HgiComputePipelineHandle,
    HgiResourceBindingsHandle,
};

use crate::command_queue::HgiVulkanCommandQueue;
use crate::compute_pipeline::HgiVulkanComputePipeline;
use crate::diagnostic;
use crate::resource_bindings::HgiVulkanResourceBindings;

// Debug colors matching C++ `s_computeDebugColor` and `s_markerDebugColor`.
const COMPUTE_DEBUG_COLOR: [f32; 4] = [0.855, 0.161, 0.11, 1.0];
const MARKER_DEBUG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// Vulkan compute command buffer.
///
/// Mirrors C++ `HgiVulkanComputeCmds`. A real `VkCommandBuffer` is acquired
/// from the queue on first use (`create_command_buffer`), matching the C++
/// lazy-init pattern.
///
/// Resources and push constants are bound lazily, just before each
/// `vkCmdDispatch`, so the pipeline layout is always known at flush time.
///
/// `device` is `None` in stub mode (when no live Vulkan device exists).
/// All Vulkan calls are gated on both `device.is_some()` and
/// `command_buffer.is_some()`.
pub struct HgiVulkanComputeCmds {
    /// Logical device — `None` in stub/test mode.
    device: Option<ash::Device>,
    /// Optional debug-utils device extension loader (for labels/markers).
    debug_utils: Option<ash::ext::debug_utils::Device>,
    /// Raw command buffer handle; `None` until first command is recorded.
    command_buffer: Option<vk::CommandBuffer>,
    /// Pipeline layout captured from `bind_pipeline`.
    pipeline_layout: vk::PipelineLayout,
    /// Pending resource bindings to be applied before the next dispatch.
    resource_bindings: Option<HgiResourceBindingsHandle>,
    /// True when push-constant data has been updated but not yet flushed.
    push_constants_dirty: bool,
    /// Raw push-constant bytes, mirroring C++ `uint8_t* _pushConstants`.
    push_constants: Vec<u8>,
    /// Local work-group size extracted from the bound compute shader function.
    /// Defaults to `[1, 1, 1]`, matching C++ `GfVec3i(1, 1, 1)`.
    local_work_group_size: [i32; 3],
    /// Submission state — set after `execute_submit` completes.
    submitted: bool,
    /// Owning thread + pool index pair, set when `command_buffer` is acquired.
    cmd_buf_token: Option<(thread::ThreadId, usize)>,
    /// Back-pointer to the command queue. `None` in stub mode.
    command_queue: Option<*mut HgiVulkanCommandQueue>,
}

// SAFETY: HgiVulkanComputeCmds is used from a single thread at a time.
// The raw pointer to the command queue does not escape this struct.
unsafe impl Send for HgiVulkanComputeCmds {}
unsafe impl Sync for HgiVulkanComputeCmds {}

impl HgiVulkanComputeCmds {
    /// Creates a new compute command buffer connected to a live Vulkan device
    /// and command queue.
    ///
    /// The `_desc` parameter is accepted for API parity with C++ but carries
    /// no per-cmds Vulkan state.
    pub fn new_with_device(
        device: ash::Device,
        debug_utils: Option<ash::ext::debug_utils::Device>,
        command_queue: *mut HgiVulkanCommandQueue,
        _desc: &HgiComputeCmdsDesc,
    ) -> Self {
        Self {
            device: Some(device),
            debug_utils,
            command_buffer: None,
            pipeline_layout: vk::PipelineLayout::null(),
            resource_bindings: None,
            push_constants_dirty: false,
            push_constants: Vec::new(),
            local_work_group_size: [1, 1, 1],
            submitted: false,
            cmd_buf_token: None,
            command_queue: Some(command_queue),
        }
    }

    /// Stub constructor used when no live Vulkan device is available
    /// (e.g. the current `HgiVulkan` stub in `hgi.rs`).
    pub fn new() -> Self {
        Self {
            device: None,
            debug_utils: None,
            command_buffer: None,
            pipeline_layout: vk::PipelineLayout::null(),
            resource_bindings: None,
            push_constants_dirty: false,
            push_constants: Vec::new(),
            local_work_group_size: [1, 1, 1],
            submitted: false,
            cmd_buf_token: None,
            command_queue: None,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Acquire a command buffer from the queue on first use.
    ///
    /// Mirrors C++ `_CreateCommandBuffer()`.
    fn create_command_buffer(&mut self) {
        if self.command_buffer.is_some() {
            return;
        }
        let Some(queue_ptr) = self.command_queue else {
            return;
        };
        // SAFETY: `queue_ptr` is valid for the lifetime of this cmds object.
        let queue = unsafe { &mut *queue_ptr };
        let (thread_id, index) = queue.acquire_command_buffer();
        let raw_cb = queue.command_buffer(thread_id, index).vk_command_buffer();
        self.cmd_buf_token = Some((thread_id, index));
        self.command_buffer = Some(raw_cb);
    }

    /// Bind pending resources and flush push constants before a dispatch.
    ///
    /// Mirrors C++ `_BindResources()`.
    fn bind_resources_internal(&mut self) {
        let (Some(device), Some(cb)) = (self.device.as_ref(), self.command_buffer) else {
            return;
        };
        if self.pipeline_layout == vk::PipelineLayout::null() {
            return;
        }

        // Bind descriptor sets if pending resource bindings exist.
        if let Some(res) = self.resource_bindings.take() {
            if let Some(rb) = res
                .get()
                .and_then(|r| r.as_any().downcast_ref::<HgiVulkanResourceBindings>())
            {
                rb.bind_resources(cb, vk::PipelineBindPoint::COMPUTE, self.pipeline_layout);
            }
            // `resource_bindings` consumed — bind only once per dispatch.
        }

        // Flush dirty push constants.
        if self.push_constants_dirty && !self.push_constants.is_empty() {
            // SAFETY: cb is valid and recording; pipeline_layout was created
            // with a COMPUTE push-constant range of the appropriate size.
            unsafe {
                device.cmd_push_constants(
                    cb,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0, // offset always 0, matching C++
                    &self.push_constants,
                );
            }
            self.push_constants_dirty = false;
        }
    }
}

impl Default for HgiVulkanComputeCmds {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HgiCmds
// ---------------------------------------------------------------------------

impl HgiCmds for HgiVulkanComputeCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    /// Open a named debug group in GPU profilers (RenderDoc, NSight, etc.).
    fn push_debug_group(&mut self, label: &str) {
        self.create_command_buffer();
        if let Some(cb) = self.command_buffer {
            diagnostic::begin_label(self.debug_utils.as_ref(), cb, label, COMPUTE_DEBUG_COLOR);
        }
    }

    /// Close the current debug group.
    fn pop_debug_group(&mut self) {
        self.create_command_buffer();
        if let Some(cb) = self.command_buffer {
            diagnostic::end_label(self.debug_utils.as_ref(), cb);
        }
    }

    /// Insert a single-point debug marker.
    fn insert_debug_marker(&mut self, label: &str) {
        self.create_command_buffer();
        if let Some(cb) = self.command_buffer {
            diagnostic::insert_debug_marker(
                self.debug_utils.as_ref(),
                cb,
                label,
                MARKER_DEBUG_COLOR,
            );
        }
    }

    /// Submit the recorded commands to the GPU.
    ///
    /// Called by `Hgi::submit_cmds`. Returns early when no command buffer was
    /// ever recorded (mirrors C++ `if (!_commandBuffer) return false`).
    fn execute_submit(&mut self) {
        if self.command_buffer.is_none() {
            return;
        }
        let Some(queue_ptr) = self.command_queue else {
            return;
        };
        let Some((thread_id, index)) = self.cmd_buf_token else {
            return;
        };
        // SAFETY: queue_ptr is valid for the same reasons as in
        // `create_command_buffer`.
        let queue = unsafe { &mut *queue_ptr };
        queue.submit_to_queue(thread_id, index, HgiSubmitWaitType::NoWait);
        self.submitted = true;
    }
}

// ---------------------------------------------------------------------------
// HgiComputeCmds
// ---------------------------------------------------------------------------

impl HgiComputeCmds for HgiVulkanComputeCmds {
    /// Bind a compute pipeline.
    ///
    /// Records `vkCmdBindPipeline(COMPUTE)`, captures the `VkPipelineLayout`,
    /// and extracts the compute shader's local work-group size.
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::BindPipeline`.
    fn bind_pipeline(&mut self, pipeline: &HgiComputePipelineHandle) {
        self.create_command_buffer();
        let Some(cb) = self.command_buffer else {
            return;
        };

        let Some(pso) = pipeline
            .get()
            .and_then(|p| p.as_any().downcast_ref::<HgiVulkanComputePipeline>())
        else {
            log::warn!("HgiVulkanComputeCmds::bind_pipeline: not an HgiVulkanComputePipeline");
            return;
        };

        self.pipeline_layout = pso.vk_pipeline_layout();
        pso.bind_pipeline(cb);

        // Extract local work-group size from the bound compute shader function.
        // Mirrors C++ loop over shaderFunctionsHandles.
        let desc = pipeline.descriptor();
        if let Some(program) = desc.shader_program.get() {
            for fn_handle in &program.descriptor().shader_functions {
                if let Some(shader_fn) = fn_handle.get() {
                    let fn_desc = shader_fn.descriptor();
                    if fn_desc.shader_stage == HgiShaderStage::COMPUTE {
                        let ls = fn_desc.compute_descriptor.local_size;
                        if ls[0] > 0 && ls[1] > 0 && ls[2] > 0 {
                            self.local_work_group_size = ls;
                        }
                    }
                }
            }
        }
    }

    /// Store resource bindings for lazy application before the next dispatch.
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::BindResources`.
    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        self.create_command_buffer();
        self.resource_bindings = Some(resources.clone());
    }

    /// Store push-constant data for lazy upload before the next dispatch.
    ///
    /// `_bind_index` is unused on Vulkan (push-constant offset is always 0,
    /// matching C++).
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::SetConstantValues`.
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiComputePipelineHandle,
        _bind_index: u32,
        data: &[u8],
    ) {
        self.create_command_buffer();
        // Reallocate only when the byte size changes, mirroring the C++
        // `delete[] / new uint8_t[byteSize]` pattern.
        if self.push_constants.len() != data.len() {
            self.push_constants = vec![0u8; data.len()];
        }
        self.push_constants.copy_from_slice(data);
        self.push_constants_dirty = true;
    }

    /// Dispatch compute work groups.
    ///
    /// Binds pending resources / push constants, then emits `vkCmdDispatch`.
    /// The `dispatch` op carries pre-divided group counts; they are clamped
    /// to ≥1 (Vulkan requires group counts > 0).
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::Dispatch(dimX, dimY)`.
    fn dispatch(&mut self, dispatch: &HgiComputeDispatchOp) {
        self.create_command_buffer();
        if self.device.is_none() || self.command_buffer.is_none() {
            return;
        }

        // Bind resources before reading device/cb to satisfy the borrow checker:
        // `bind_resources_internal` takes `&mut self`, which conflicts with an
        // active `&self.device` borrow.
        self.bind_resources_internal();

        let device = self.device.as_ref().unwrap();
        let cb = self.command_buffer.unwrap();

        let groups_x = dispatch.work_group_count_x.max(1);
        let groups_y = dispatch.work_group_count_y.max(1);
        let groups_z = dispatch.work_group_count_z.max(1);

        // SAFETY: cb is valid and in recording state; group counts are > 0.
        unsafe {
            device.cmd_dispatch(cb, groups_x, groups_y, groups_z);
        }
    }

    /// Insert a pipeline memory barrier.
    ///
    /// Emits a full `READ|WRITE → READ|WRITE` barrier on `ALL_COMMANDS` —
    /// the "big hammer" approach used by C++ `InsertMemoryBarrier` via
    /// `HgiVulkanCommandBuffer::InsertMemoryBarrier`.
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::InsertMemoryBarrier`.
    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier) {
        self.create_command_buffer();
        let (Some(device), Some(cb)) = (self.device.as_ref(), self.command_buffer) else {
            return;
        };
        if !barrier.contains(HgiMemoryBarrier::ALL) {
            return;
        }

        let mem_barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE);

        // SAFETY: cb is valid and in recording state.
        unsafe {
            device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[mem_barrier],
                &[], // no buffer barriers
                &[], // no image barriers
            );
        }
    }

    /// Vulkan dispatches serially (single queue, no concurrent dispatch).
    ///
    /// Mirrors C++ `HgiVulkanComputeCmds::GetDispatchMethod`.
    fn get_dispatch_method(&self) -> HgiComputeDispatch {
        HgiComputeDispatch::Serial
    }
}

// ---------------------------------------------------------------------------
// Public utilities
// ---------------------------------------------------------------------------

/// Compute the number of work groups for `element_count` elements given
/// `threads_per_group` threads per group.
///
/// Implements C++ ceiling division:
/// `(dim + (threads_per_group - 1)) / threads_per_group`.
///
/// Useful for callers that start from raw element counts rather than
/// pre-divided group counts.
#[inline]
pub fn work_group_count(element_count: u32, threads_per_group: u32) -> u32 {
    let threads = threads_per_group.max(1);
    (element_count + threads - 1) / threads
}
