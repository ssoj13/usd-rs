//! Command queue for Vulkan — manages command buffer allocation, submission,
//! and timeline semaphore synchronization.
//!
//! Port of pxr/imaging/hgiVulkan/commandQueue.h/.cpp

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use ash::vk;
use usd_hgi::enums::HgiSubmitWaitType;

use crate::command_buffer::{HgiVulkanCommandBuffer, InFlightUpdateResult};

// ---------------------------------------------------------------------------
// CommandPool — one per thread
// ---------------------------------------------------------------------------

/// One Vulkan command pool together with all command buffers allocated from it.
/// There is exactly one `HgiVulkanCommandPool` per thread that has ever called
/// `acquire_command_buffer`.
pub struct HgiVulkanCommandPool {
    pub vk_command_pool: vk::CommandPool,
    pub command_buffers: Vec<HgiVulkanCommandBuffer>,
}

// ---------------------------------------------------------------------------
// HgiVulkanCommandQueue
// ---------------------------------------------------------------------------

/// Manages command buffer allocation, per-thread pools, and GPU submission via
/// a Vulkan timeline semaphore.
///
/// Thread-safety contract (mirrors C++):
/// - `acquire_command_buffer` is thread-safe (pool map locked).
/// - `acquire_resource_command_buffer` must only be called from the main thread.
/// - `submit_to_queue`, `flush`, `reset_consumed_command_buffers`, and
///   `is_timeline_past_value` are NOT thread-safe; callers must synchronise.
pub struct HgiVulkanCommandQueue {
    /// Logical device — cloned from the creator so we can outlive the original.
    device: Arc<ash::Device>,

    /// The graphics queue obtained at construction time.
    vk_gfx_queue: vk::Queue,

    /// Per-thread command pools, keyed by `thread::ThreadId`.
    /// The `Mutex` allows concurrent `acquire_command_buffer` calls from
    /// different threads.
    command_pools: Mutex<HashMap<thread::ThreadId, HgiVulkanCommandPool>>,

    /// 64-bit bitmask: bit N is set while command buffer N is in-flight.
    inflight_bits: AtomicU64,

    /// Wrapping counter used to prefer higher-numbered (newer) bits, reducing
    /// immediate bit-reuse (same semantics as C++ `_inflightCounter`).
    inflight_counter: AtomicU8,

    /// The thread that constructed the queue — guards single-threaded access to
    /// the resource command buffer.
    owner_thread: thread::ThreadId,

    /// Index of the resource command buffer inside the owner thread's pool.
    /// `None` when no resource command buffer is currently active.
    resource_cmd_buf_index: Option<usize>,

    /// Pending command buffers (thread-id + pool index) that have been ended
    /// but not yet submitted to the GPU.  Flushed as a batch by `flush`.
    queued_buffers: VecDeque<(thread::ThreadId, usize)>,

    /// Timeline semaphore for lightweight GPU↔CPU sync without per-submission
    /// fences.
    timeline_semaphore: vk::Semaphore,

    /// The value signalled by the *next* call to `flush`.  Starts at 1.
    timeline_next_val: u64,

    /// Most recently queried completed value — cached to avoid repeated
    /// `vkGetSemaphoreCounterValue` syscalls.
    timeline_cached_val: u64,

    /// Queue family index used when creating per-thread command pools.
    gfx_queue_family_index: u32,
}

impl HgiVulkanCommandQueue {
    /// Create the queue: retrieve the graphics queue handle and create the
    /// timeline semaphore.  No command pool is created yet; pools are created
    /// lazily on first use per thread.
    pub fn new(device: Arc<ash::Device>, gfx_queue_family_index: u32) -> Result<Self, vk::Result> {
        // Queue index 0 within the graphics family (C++: `firstQueueInFamily = 0`).
        let vk_gfx_queue = unsafe { device.get_device_queue(gfx_queue_family_index, 0) };

        // Timeline semaphore — pNext chain: SemaphoreTypeCreateInfo inside
        // SemaphoreCreateInfo.
        let mut timeline_create_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);

        let semaphore_create_info =
            vk::SemaphoreCreateInfo::default().push_next(&mut timeline_create_info);

        let timeline_semaphore = unsafe { device.create_semaphore(&semaphore_create_info, None)? };

        Ok(Self {
            device,
            vk_gfx_queue,
            command_pools: Mutex::new(HashMap::new()),
            inflight_bits: AtomicU64::new(0),
            inflight_counter: AtomicU8::new(0),
            owner_thread: thread::current().id(),
            resource_cmd_buf_index: None,
            queued_buffers: VecDeque::new(),
            timeline_semaphore,
            timeline_next_val: 1,
            timeline_cached_val: 0,
            gfx_queue_family_index,
        })
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Enqueue `cmd_buf_index` (in `thread_id`'s pool) for GPU submission.
    ///
    /// If a resource command buffer is active it is enqueued first so that
    /// resource uploads precede work commands (mirrors C++ ordering).
    ///
    /// If `wait == WaitUntilCompleted` the call blocks until the GPU finishes.
    ///
    /// Externally synchronised — only one thread may call this at a time.
    pub fn submit_to_queue(
        &mut self,
        thread_id: thread::ThreadId,
        cmd_buf_index: usize,
        wait: HgiSubmitWaitType,
    ) {
        // Flush resource command buffer before work buffers.
        if let Some(res_index) = self.resource_cmd_buf_index.take() {
            let res_tid = self.owner_thread;
            {
                let mut pools = self.command_pools.lock().unwrap();
                let pool = pools.get_mut(&res_tid).expect("resource pool missing");
                pool.command_buffers[res_index].end_command_buffer();
                pool.command_buffers[res_index]
                    .set_completed_timeline_value(self.timeline_next_val);
            }
            self.queued_buffers.push_back((res_tid, res_index));
        }

        {
            let mut pools = self.command_pools.lock().unwrap();
            let pool = pools
                .get_mut(&thread_id)
                .expect("command buffer pool missing");
            pool.command_buffers[cmd_buf_index].end_command_buffer();
            pool.command_buffers[cmd_buf_index]
                .set_completed_timeline_value(self.timeline_next_val);
        }
        self.queued_buffers.push_back((thread_id, cmd_buf_index));

        if wait == HgiSubmitWaitType::WaitUntilCompleted {
            self.flush(wait, vk::Semaphore::null());
        }
    }

    /// Return `(thread_id, index)` of a command buffer that is reset and ready
    /// to record.  A new buffer is allocated from the thread's pool if none is
    /// free.
    ///
    /// Begins recording (`vkBeginCommandBuffer`) before returning so the caller
    /// has exclusive access.
    ///
    /// Thread-safe — multiple threads may call this concurrently.
    pub fn acquire_command_buffer(&mut self) -> (thread::ThreadId, usize) {
        let thread_id = thread::current().id();
        self.ensure_pool_exists(thread_id);

        // Find a reset (reusable) buffer, or push a new one.
        let index = {
            let mut pools = self.command_pools.lock().unwrap();
            let pool = pools.get_mut(&thread_id).unwrap();
            match pool.command_buffers.iter().position(|cb| cb.is_reset()) {
                Some(i) => i,
                None => {
                    let cb = HgiVulkanCommandBuffer::new(
                        &self.device,
                        pool.vk_command_pool,
                        self.timeline_semaphore,
                    )
                    .expect("failed to allocate VkCommandBuffer");
                    pool.command_buffers.push(cb);
                    pool.command_buffers.len() - 1
                }
            }
        };

        // Acquire a free inflight ID bit, spinning if all 64 are occupied.
        // When all bits are taken we release bits for any buffer the GPU
        // already finished (matching C++ AcquireCommandBuffer spin loop).
        let inflight_id = loop {
            if let Some(id) = self.acquire_inflight_id_bit() {
                break id;
            }

            // Sleep the minimum amount to avoid a hot loop (C++ uses 1µs).
            std::thread::sleep(std::time::Duration::from_micros(1));

            let mut pools = self.command_pools.lock().unwrap();
            let pool = pools.get_mut(&thread_id).unwrap();
            for cb in &mut pool.command_buffers {
                if cb.update_in_flight_status(HgiSubmitWaitType::NoWait)
                    == InFlightUpdateResult::FinishedFlight
                {
                    let id = cb.inflight_id();
                    // Release the bit without holding the pool lock (avoid
                    // re-entrant lock) — safe because `release_inflight_bit`
                    // only touches the atomic.
                    self.release_inflight_bit(id);
                }
            }
        };

        // Begin recording — caller now has exclusive access.
        {
            let mut pools = self.command_pools.lock().unwrap();
            pools.get_mut(&thread_id).unwrap().command_buffers[index]
                .begin_command_buffer(inflight_id);
        }

        (thread_id, index)
    }

    /// Return the resource command buffer (main thread only).
    ///
    /// Creates one via `acquire_command_buffer` on first call per frame.
    /// The buffer is automatically enqueued before work buffers in
    /// `submit_to_queue`.
    ///
    /// # Panics
    /// Panics if called from a thread other than the one that created the queue.
    pub fn acquire_resource_command_buffer(&mut self) -> (thread::ThreadId, usize) {
        assert_eq!(
            thread::current().id(),
            self.owner_thread,
            "acquire_resource_command_buffer must be called from the main thread"
        );

        if self.resource_cmd_buf_index.is_none() {
            let (_tid, idx) = self.acquire_command_buffer();
            self.resource_cmd_buf_index = Some(idx);
        }

        (self.owner_thread, self.resource_cmd_buf_index.unwrap())
    }

    /// 64-bit bitmask of all currently in-flight command buffers.
    ///
    /// Used by the garbage collector to delay destruction of GPU resources
    /// until the buffers referencing them have finished executing.
    ///
    /// Thread-safe.
    pub fn get_inflight_command_buffers_bits(&self) -> u64 {
        self.inflight_bits.load(Ordering::Relaxed)
    }

    /// The raw Vulkan graphics queue handle.
    pub fn vk_graphics_queue(&self) -> vk::Queue {
        self.vk_gfx_queue
    }

    /// Scan every pool and reset any command buffer the GPU has finished with,
    /// releasing its inflight ID bit.
    ///
    /// Single-threaded — must be called while no other threads are recording.
    pub fn reset_consumed_command_buffers(&mut self, wait: HgiSubmitWaitType) {
        let mut pools = self.command_pools.lock().unwrap();
        for pool in pools.values_mut() {
            for cb in &mut pool.command_buffers {
                if cb.reset_if_consumed(wait) {
                    let id = cb.inflight_id();
                    self.release_inflight_bit(id);
                }
            }
        }
    }

    /// Submit all queued command buffers to the GPU in one `vkQueueSubmit`.
    ///
    /// Signals `timeline_semaphore` with `timeline_next_val`, then increments
    /// it.  If `signal_semaphore` is non-null it is also signalled (value 0,
    /// binary semaphore) for swapchain / interop use.
    ///
    /// If `wait == WaitUntilCompleted` the call blocks until the GPU is done
    /// and runs all completion handlers.
    pub fn flush(&mut self, wait: HgiSubmitWaitType, signal_semaphore: vk::Semaphore) {
        // Collect raw VkCommandBuffer handles while holding the lock briefly.
        let raw_cmd_bufs: Vec<vk::CommandBuffer> = {
            let pools = self.command_pools.lock().unwrap();
            self.queued_buffers
                .iter()
                .map(|(tid, idx)| pools[tid].command_buffers[*idx].vk_command_buffer())
                .collect()
        };

        let has_extra = signal_semaphore != vk::Semaphore::null();

        // Build semaphore + value arrays depending on whether an interop
        // semaphore was provided.
        let semaphore_signals: Vec<vk::Semaphore> = if has_extra {
            vec![self.timeline_semaphore, signal_semaphore]
        } else {
            vec![self.timeline_semaphore]
        };
        let signal_values: Vec<u64> = if has_extra {
            // Binary semaphores use value 0 in the timeline submit info.
            vec![self.timeline_next_val, 0]
        } else {
            vec![self.timeline_next_val]
        };

        let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .wait_semaphore_values(&[])
            .signal_semaphore_values(&signal_values);

        let submit_info = vk::SubmitInfo::default()
            .push_next(&mut timeline_info)
            .command_buffers(&raw_cmd_bufs)
            .signal_semaphores(&semaphore_signals);

        unsafe {
            self.device
                .queue_submit(self.vk_gfx_queue, &[submit_info], vk::Fence::null())
                .expect("vkQueueSubmit failed");
        }

        if wait == HgiSubmitWaitType::WaitUntilCompleted {
            let wait_values = [self.timeline_next_val];
            let wait_semaphores = [self.timeline_semaphore];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&wait_semaphores)
                .values(&wait_values);

            unsafe {
                self.device
                    .wait_semaphores(&wait_info, u64::MAX)
                    .expect("vkWaitSemaphores failed");
            }

            // Run GPU→CPU completion callbacks (e.g. readbacks).
            let mut pools = self.command_pools.lock().unwrap();
            for (tid, idx) in &self.queued_buffers {
                pools.get_mut(tid).unwrap().command_buffers[*idx]
                    .run_and_clear_completed_handlers();
            }

            self.timeline_cached_val = self.timeline_next_val;
        }

        self.queued_buffers.clear();
        self.timeline_next_val += 1;
    }

    /// Check whether the timeline has advanced past `desired_value`.
    ///
    /// If `desired_value` equals `timeline_next_val` (the value that would be
    /// signalled by the next flush) a non-blocking flush is triggered first to
    /// put the value in flight.  If `wait` is true the function blocks until
    /// satisfied.
    pub fn is_timeline_past_value(&mut self, desired_value: u64, wait: bool) -> bool {
        if self.timeline_cached_val >= desired_value {
            return true;
        }

        // Ensure the value is actually in flight before we wait for it.
        if self.timeline_next_val == desired_value {
            self.flush(HgiSubmitWaitType::NoWait, vk::Semaphore::null());
        }

        let current = unsafe {
            self.device
                .get_semaphore_counter_value(self.timeline_semaphore)
                .expect("vkGetSemaphoreCounterValue failed")
        };
        self.timeline_cached_val = current;

        if self.timeline_cached_val >= desired_value {
            return true;
        }

        if wait {
            let values = [desired_value];
            let semaphores = [self.timeline_semaphore];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&semaphores)
                .values(&values);

            unsafe {
                self.device
                    .wait_semaphores(&wait_info, u64::MAX)
                    .expect("vkWaitSemaphores failed");
            }
            self.timeline_cached_val = desired_value;
            return true;
        }

        false
    }

    /// Graphics queue family index (used when creating command pools).
    pub fn gfx_queue_family_index(&self) -> u32 {
        self.gfx_queue_family_index
    }

    /// Immutable borrow of a command buffer by `(thread_id, index)`.
    ///
    /// Acquires the pool lock for the duration of the returned guard's life.
    pub fn command_buffer(
        &self,
        thread_id: thread::ThreadId,
        index: usize,
    ) -> CommandBufferRef<'_> {
        CommandBufferRef {
            guard: self.command_pools.lock().unwrap(),
            thread_id,
            index,
        }
    }

    /// Mutable borrow of a command buffer by `(thread_id, index)`.
    ///
    /// Acquires the pool lock for the duration of the returned guard's life.
    pub fn command_buffer_mut(
        &self,
        thread_id: thread::ThreadId,
        index: usize,
    ) -> CommandBufferMut<'_> {
        CommandBufferMut {
            guard: self.command_pools.lock().unwrap(),
            thread_id,
            index,
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Ensure `thread_id` has a command pool, creating one lazily if needed.
    fn ensure_pool_exists(&self, thread_id: thread::ThreadId) {
        let mut pools = self.command_pools.lock().unwrap();
        if !pools.contains_key(&thread_id) {
            let pool = create_command_pool(&self.device, self.gfx_queue_family_index)
                .expect("failed to create VkCommandPool");
            pools.insert(thread_id, pool);
        }
    }

    /// Atomically allocate the next free bit from `inflight_bits`.
    ///
    /// Mirrors C++ `_AcquireInflightIdBit`.  Prefers bits with higher indices
    /// by masking out `previous_bits` (all bits below the counter position),
    /// so recently-released bits are not immediately reused.  The counter
    /// wraps at 64.  Returns `None` when all 64 bits are occupied.
    fn acquire_inflight_id_bit(&self) -> Option<u8> {
        // Counter wraps at 64 (0x3F mask).
        let next_bit_index = 0x3F & self.inflight_counter.fetch_add(1, Ordering::Relaxed);
        // All bits below next_bit_index are considered "recently used".
        let previous_bits: u64 = (1u64 << next_bit_index).wrapping_sub(1);

        let mut expected = self.inflight_bits.load(Ordering::Relaxed);
        loop {
            let used_bits = expected | previous_bits;
            // Lowest free bit above the previous-bits mask.
            let free_bit = (!used_bits) & used_bits.wrapping_add(1);
            if free_bit == 0 {
                return None;
            }

            let desired = (expected & !free_bit) | free_bit;
            match self.inflight_bits.compare_exchange_weak(
                expected,
                desired,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(free_bit.trailing_zeros() as u8),
                Err(actual) => expected = actual,
            }
        }
    }

    /// Atomically clear bit `id` in `inflight_bits`.
    fn release_inflight_bit(&self, id: u8) {
        let mask = !(1u64 << id);
        let mut expected = self.inflight_bits.load(Ordering::Relaxed);
        loop {
            let desired = expected & mask;
            match self.inflight_bits.compare_exchange_weak(
                expected,
                desired,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => expected = actual,
            }
        }
    }
}

impl Drop for HgiVulkanCommandQueue {
    fn drop(&mut self) {
        // Wait for all in-flight work to finish before destroying Vulkan objects.
        unsafe {
            let _ = self.device.device_wait_idle();
        }

        // Drain all command pools. `HgiVulkanCommandBuffer::drop` frees the
        // individual VkCommandBuffer handles; we then destroy the pool itself.
        {
            let mut pools = self.command_pools.lock().unwrap();
            for (_, pool) in pools.drain() {
                // Drop command buffers first (they call vkFreeCommandBuffers).
                drop(pool.command_buffers);
                unsafe {
                    self.device.destroy_command_pool(pool.vk_command_pool, None);
                }
            }
        }

        unsafe {
            self.device.destroy_semaphore(self.timeline_semaphore, None);
        }
    }
}

// ---------------------------------------------------------------------------
// Command pool creation
// ---------------------------------------------------------------------------

/// Allocate a fresh `HgiVulkanCommandPool` for the given queue family.
fn create_command_pool(
    device: &ash::Device,
    queue_family_index: u32,
) -> Result<HgiVulkanCommandPool, vk::Result> {
    let create_info = vk::CommandPoolCreateInfo::default()
        // TRANSIENT: buffers are short-lived (reset / freed every frame).
        // RESET_COMMAND_BUFFER: individual buffers can be reset independently.
        .flags(
            vk::CommandPoolCreateFlags::TRANSIENT
                | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )
        .queue_family_index(queue_family_index);

    let vk_command_pool = unsafe { device.create_command_pool(&create_info, None)? };

    Ok(HgiVulkanCommandPool {
        vk_command_pool,
        command_buffers: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Ergonomic accessor guards
// ---------------------------------------------------------------------------

/// Immutable accessor that holds the pool `MutexGuard` and dereferences to
/// `&HgiVulkanCommandBuffer`.
pub struct CommandBufferRef<'a> {
    guard: std::sync::MutexGuard<'a, HashMap<thread::ThreadId, HgiVulkanCommandPool>>,
    thread_id: thread::ThreadId,
    index: usize,
}

impl std::ops::Deref for CommandBufferRef<'_> {
    type Target = HgiVulkanCommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.guard[&self.thread_id].command_buffers[self.index]
    }
}

/// Mutable accessor that holds the pool `MutexGuard` and dereferences to
/// `&mut HgiVulkanCommandBuffer`.
pub struct CommandBufferMut<'a> {
    guard: std::sync::MutexGuard<'a, HashMap<thread::ThreadId, HgiVulkanCommandPool>>,
    thread_id: thread::ThreadId,
    index: usize,
}

impl std::ops::Deref for CommandBufferMut<'_> {
    type Target = HgiVulkanCommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.guard[&self.thread_id].command_buffers[self.index]
    }
}

impl std::ops::DerefMut for CommandBufferMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard.get_mut(&self.thread_id).unwrap().command_buffers[self.index]
    }
}
