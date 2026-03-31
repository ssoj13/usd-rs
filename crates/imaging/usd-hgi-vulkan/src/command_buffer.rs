//! Vulkan primary command buffer wrapper.
//!
//! Port of pxr/imaging/hgiVulkan/commandBuffer.h/.cpp
//!
//! Lifecycle (from C++ header):
//!   Initial:              Reset=T, InFlight=F, Submitted=F
//!   BeginCommandBuffer(): Reset=F, InFlight=T, Submitted=F
//!   EndCommandBuffer():   Reset=F, InFlight=T, Submitted=T
//!   UpdateInFlightStatus(): Reset=F, InFlight=F, Submitted=T  (if consumed)
//!   ResetIfConsumedByGPU(): Reset=T, InFlight=F, Submitted=F

use ash::vk;
use usd_hgi::enums::{HgiMemoryBarrier, HgiSubmitWaitType};

/// Callback executed once the GPU has finished consuming a command buffer.
/// Equivalent to C++ `HgiVulkanCompletedHandler = std::function<void(void)>`.
pub type HgiVulkanCompletedHandler = Box<dyn FnOnce() + Send>;

/// Result of [`HgiVulkanCommandBuffer::update_in_flight_status`].
///
/// Captures the state transition from in-flight to not-in-flight as
/// `FinishedFlight`, matching C++ `InFlightUpdateResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InFlightUpdateResult {
    /// The command buffer was not in flight (already reset or never started).
    NotInFlight,
    /// The command buffer is still being processed by the GPU.
    StillInFlight,
    /// The GPU just finished consuming the command buffer this call.
    FinishedFlight,
}

/// A Vulkan primary command buffer managed by the command queue.
///
/// Command buffers are allocated from a pool and follow the strict lifecycle
/// described in the module doc. They are not cloneable — ownership is
/// exclusive and managed by `HgiVulkanCommandQueue`.
pub struct HgiVulkanCommandBuffer {
    /// Logical device handle (cloned from the device, not owning).
    device: ash::Device,
    /// The pool this buffer was allocated from; needed for freeing.
    vk_command_pool: vk::CommandPool,
    /// The underlying Vulkan command buffer handle.
    vk_command_buffer: vk::CommandBuffer,
    /// Timeline semaphore from the command queue — used to check GPU completion
    /// without requiring a back-reference to the queue itself.
    timeline_semaphore: vk::Semaphore,
    /// Callbacks run once the GPU has consumed this buffer.
    completed_handlers: Vec<HgiVulkanCompletedHandler>,
    /// True when the buffer is ready to begin recording (initial / after reset).
    is_reset: bool,
    /// True while the buffer is recording or being consumed by the GPU.
    is_in_flight: bool,
    /// True after `end_command_buffer` — the buffer has been closed for submission.
    is_submitted: bool,
    /// Unique id among all currently in-flight command buffers (set by `begin_command_buffer`).
    inflight_id: u8,
    /// The timeline semaphore value the queue will reach when this buffer completes.
    completed_timeline_value: u64,
}

impl HgiVulkanCommandBuffer {
    /// Allocate a primary command buffer from `pool`.
    ///
    /// `timeline_semaphore` must be the queue's timeline semaphore — it is
    /// stored so that `update_in_flight_status` can query completion without
    /// holding a back-reference to the queue.  Matches C++ constructor shape.
    pub fn new(
        device: &ash::Device,
        pool: vk::CommandPool,
        timeline_semaphore: vk::Semaphore,
    ) -> Result<Self, vk::Result> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        // SAFETY: pool must be valid and created on the same device.
        let buffers = unsafe { device.allocate_command_buffers(&alloc_info)? };
        let vk_command_buffer = buffers[0];

        log::debug!(
            "HgiVulkan Command Buffer {:?} allocated from pool {:?}",
            vk_command_buffer,
            pool
        );

        Ok(Self {
            device: device.clone(),
            vk_command_pool: pool,
            vk_command_buffer,
            timeline_semaphore,
            completed_handlers: Vec::new(),
            is_reset: true,
            is_in_flight: false,
            is_submitted: false,
            inflight_id: 0,
            completed_timeline_value: 0,
        })
    }

    /// Begin recording commands into this buffer.
    ///
    /// Transitions: Reset=T → Reset=F, InFlight=T.
    /// Uses `ONE_TIME_SUBMIT` matching the C++ flag.
    ///
    /// # Panics
    /// Panics (debug) if the buffer is not in the initial Reset state.
    pub fn begin_command_buffer(&mut self, inflight_id: u8) {
        debug_assert!(
            self.is_reset,
            "begin_command_buffer: buffer must be in Reset state"
        );
        debug_assert!(
            !self.is_in_flight,
            "begin_command_buffer: buffer must not be in-flight"
        );
        debug_assert!(
            !self.is_submitted,
            "begin_command_buffer: buffer must not be submitted"
        );

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: vk_command_buffer is valid and in the initial/reset state.
        unsafe {
            self.device
                .begin_command_buffer(self.vk_command_buffer, &begin_info)
                .expect("vkBeginCommandBuffer failed");
        }

        self.inflight_id = inflight_id;
        self.is_in_flight = true;
        self.is_reset = false;
    }

    /// Stop recording commands. Must be called before submitting to the queue.
    ///
    /// Transitions: InFlight=T, Submitted=F → Submitted=T.
    ///
    /// # Panics
    /// Panics (debug) if not in the recording state.
    pub fn end_command_buffer(&mut self) {
        debug_assert!(
            !self.is_reset,
            "end_command_buffer: buffer must not be in Reset state"
        );
        debug_assert!(
            self.is_in_flight,
            "end_command_buffer: buffer must be in-flight"
        );
        debug_assert!(
            !self.is_submitted,
            "end_command_buffer: buffer must not be submitted yet"
        );

        // SAFETY: vk_command_buffer is valid and currently recording.
        unsafe {
            self.device
                .end_command_buffer(self.vk_command_buffer)
                .expect("vkEndCommandBuffer failed");
        }

        self.is_submitted = true;
    }

    /// Returns `true` if the buffer is ready to begin recording (initial or after GPU reset).
    pub fn is_reset(&self) -> bool {
        self.is_reset
    }

    /// Returns `true` while the buffer is recording or being consumed by the GPU.
    pub fn is_in_flight(&self) -> bool {
        self.is_in_flight
    }

    /// Returns the raw Vulkan command buffer handle.
    pub fn vk_command_buffer(&self) -> vk::CommandBuffer {
        self.vk_command_buffer
    }

    /// Returns the Vulkan command pool this buffer was allocated from.
    pub fn vk_command_pool(&self) -> vk::CommandPool {
        self.vk_command_pool
    }

    /// Update the in-flight status by querying the timeline semaphore.
    ///
    /// State machine (matches C++ `UpdateInFlightStatus`):
    /// - Not in flight at all → `NotInFlight`
    /// - In flight but not yet submitted (still recording) → `StillInFlight`
    /// - Submitted, NoWait: non-blocking `vkGetSemaphoreCounterValue` check;
    ///   returns `StillInFlight` if GPU has not reached `completed_timeline_value`.
    /// - Submitted, WaitUntilCompleted: blocks via `vkWaitSemaphores`.
    /// - GPU finished → set `is_in_flight = false`, return `FinishedFlight`.
    pub fn update_in_flight_status(&mut self, wait: HgiSubmitWaitType) -> InFlightUpdateResult {
        if !self.is_in_flight {
            return InFlightUpdateResult::NotInFlight;
        }

        // Vulkan requirement: cannot query completion before the buffer is closed.
        if !self.is_submitted {
            return InFlightUpdateResult::StillInFlight;
        }

        // Query the timeline semaphore counter non-blockingly first.
        // SAFETY: timeline_semaphore is valid for the lifetime of the queue,
        // which always outlives its command buffers.
        let current_value = unsafe {
            self.device
                .get_semaphore_counter_value(self.timeline_semaphore)
                .expect("vkGetSemaphoreCounterValue failed")
        };

        if current_value < self.completed_timeline_value {
            if wait != HgiSubmitWaitType::WaitUntilCompleted {
                return InFlightUpdateResult::StillInFlight;
            }

            // Block until the GPU signals the required value.
            let values = [self.completed_timeline_value];
            let semaphores = [self.timeline_semaphore];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&semaphores)
                .values(&values);

            // SAFETY: semaphore and value are valid; timeout is infinite.
            unsafe {
                self.device
                    .wait_semaphores(&wait_info, u64::MAX)
                    .expect("vkWaitSemaphores failed");
            }
        }

        self.is_in_flight = false;
        InFlightUpdateResult::FinishedFlight
    }

    /// Reset the command buffer if the GPU has finished consuming it.
    ///
    /// Returns `true` if the buffer was reset this call; `false` if it was
    /// already reset (no-op) or is still in flight.
    ///
    /// When reset, all `completed_handlers` are executed and then cleared,
    /// matching C++ `ResetIfConsumedByGPU`.
    pub fn reset_if_consumed(&mut self, wait: HgiSubmitWaitType) -> bool {
        // Already available — nothing to do.
        if self.is_reset {
            return false;
        }

        if self.update_in_flight_status(wait) == InFlightUpdateResult::StillInFlight {
            return false;
        }

        // GPU is done: run any registered callbacks (e.g. GPU→CPU readback).
        self.run_and_clear_completed_handlers();

        // Reset flags. We do NOT use VK_COMMAND_BUFFER_RESET_RELEASE_RESOURCES_BIT
        // to avoid the per-frame allocation cost (matches C++ comment).
        // SAFETY: vk_command_buffer is valid and no longer in use by the GPU.
        unsafe {
            self.device
                .reset_command_buffer(self.vk_command_buffer, vk::CommandBufferResetFlags::empty())
                .expect("vkResetCommandBuffer failed");
        }

        self.is_submitted = false;
        self.is_reset = true;
        true
    }

    /// Record a pipeline memory barrier into the command buffer.
    ///
    /// Uses a full `READ | WRITE` → `READ | WRITE` barrier on
    /// `ALL_COMMANDS` stages — the "big hammer" approach from C++ matching
    /// `HgiMemoryBarrierAll`.  Fine-grained barriers require more Hgi info
    /// than is currently available.
    pub fn insert_memory_barrier(&self, barrier: HgiMemoryBarrier) {
        if self.vk_command_buffer == vk::CommandBuffer::null() {
            return;
        }

        // Only HgiMemoryBarrier::ALL is currently supported.
        debug_assert!(
            barrier == HgiMemoryBarrier::ALL,
            "insert_memory_barrier: unsupported barrier flags {:?}",
            barrier
        );

        let memory_barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE);

        // SAFETY: command buffer is valid and currently recording.
        unsafe {
            self.device.cmd_pipeline_barrier(
                self.vk_command_buffer,
                vk::PipelineStageFlags::ALL_COMMANDS, // producer stages
                vk::PipelineStageFlags::ALL_COMMANDS, // consumer stages
                vk::DependencyFlags::empty(),
                &[memory_barrier],
                &[], // no buffer barriers
                &[], // no image barriers
            );
        }
    }

    /// Set the timeline semaphore value that signals completion of this buffer.
    ///
    /// Called by the command queue after submission so that
    /// `update_in_flight_status` knows what value to wait for.
    pub fn set_completed_timeline_value(&mut self, value: u64) {
        self.completed_timeline_value = value;
    }

    /// Returns the id that uniquely identifies this buffer among all currently
    /// in-flight command buffers (assigned by `begin_command_buffer`).
    pub fn inflight_id(&self) -> u8 {
        self.inflight_id
    }

    /// Returns a reference to the logical device used to create this buffer.
    pub fn device(&self) -> &ash::Device {
        &self.device
    }

    /// Register a callback to run once the GPU has consumed this command buffer.
    ///
    /// Handlers are executed in order during `reset_if_consumed` /
    /// `run_and_clear_completed_handlers`.
    pub fn add_completed_handler(&mut self, handler: HgiVulkanCompletedHandler) {
        self.completed_handlers.push(handler);
    }

    /// Execute every registered completed-handler in insertion order, then
    /// clear the list.
    pub fn run_and_clear_completed_handlers(&mut self) {
        for handler in self.completed_handlers.drain(..) {
            handler();
        }
    }
}

impl Drop for HgiVulkanCommandBuffer {
    fn drop(&mut self) {
        // SAFETY: vk_command_buffer was allocated from vk_command_pool on this device.
        unsafe {
            self.device
                .free_command_buffers(self.vk_command_pool, &[self.vk_command_buffer]);
        }
    }
}

// Command buffers are neither Clone nor Copy — ownership is exclusive.
// The C++ class explicitly deletes copy constructor and copy-assignment.
