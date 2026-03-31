// Handles deferred destruction of Vulkan GPU resources.
//
// When the user calls Destroy* on HgiVulkan, objects are "trashed" here.
// On PerformGarbageCollection (called at EndFrame), objects whose in-flight
// command buffer bits have all been consumed by the GPU are actually deleted.
//
// C++ used thread_local vectors + static aggregators. In Rust we use a single
// Mutex per resource type — simpler and safe for our single-device, single-queue
// usage. The GC is called single-threaded at EndFrame so this is fine.

use std::sync::Mutex;

/// A trashed GPU resource waiting to be garbage collected.
///
/// `inflight_bits` records which command-buffer slots were in flight at the
/// moment the object was trashed. The destructor is only invoked once all of
/// those slots have been retired by the GPU.
pub struct GarbageItem {
    /// Bitmask of command-buffer slots that were in-flight when this object
    /// was trashed. Bit i == 1 means CB slot i may still reference the object.
    pub inflight_bits: u64,
    /// Closure that actually frees the Vulkan resource. Called exactly once.
    pub destructor: Box<dyn FnOnce() + Send>,
}

/// Garbage collector for deferred Vulkan resource destruction.
///
/// Port of `HgiVulkanGarbageCollector`. Keeps per-type dequeues of trashed
/// objects and destroys them when their in-flight bits no longer overlap with
/// the queue's current in-flight bits.
pub struct HgiVulkanGarbageCollector {
    buffers: Mutex<Vec<GarbageItem>>,
    textures: Mutex<Vec<GarbageItem>>,
    samplers: Mutex<Vec<GarbageItem>>,
    shader_functions: Mutex<Vec<GarbageItem>>,
    shader_programs: Mutex<Vec<GarbageItem>>,
    resource_bindings: Mutex<Vec<GarbageItem>>,
    graphics_pipelines: Mutex<Vec<GarbageItem>>,
    compute_pipelines: Mutex<Vec<GarbageItem>>,

    /// Guard against trashing objects while collection is in progress.
    /// Mirrors the `_isDestroying` flag in the C++ version.
    is_destroying: std::sync::atomic::AtomicBool,
}

// -------------------------------------------------------------------
// Construction
// -------------------------------------------------------------------

impl HgiVulkanGarbageCollector {
    pub fn new() -> Self {
        Self {
            buffers: Mutex::new(Vec::new()),
            textures: Mutex::new(Vec::new()),
            samplers: Mutex::new(Vec::new()),
            shader_functions: Mutex::new(Vec::new()),
            shader_programs: Mutex::new(Vec::new()),
            resource_bindings: Mutex::new(Vec::new()),
            graphics_pipelines: Mutex::new(Vec::new()),
            compute_pipelines: Mutex::new(Vec::new()),
            is_destroying: std::sync::atomic::AtomicBool::new(false),
        }
    }

    // -------------------------------------------------------------------
    // Trash (enqueue for deferred deletion)
    //
    // These are callable from any thread while GC is not running.
    // -------------------------------------------------------------------

    /// Enqueue a buffer for deferred destruction.
    pub fn trash_buffer(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.buffers.lock().unwrap().push(item);
    }

    /// Enqueue a texture for deferred destruction.
    pub fn trash_texture(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.textures.lock().unwrap().push(item);
    }

    /// Enqueue a sampler for deferred destruction.
    pub fn trash_sampler(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.samplers.lock().unwrap().push(item);
    }

    /// Enqueue a shader function for deferred destruction.
    pub fn trash_shader_function(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.shader_functions.lock().unwrap().push(item);
    }

    /// Enqueue a shader program for deferred destruction.
    pub fn trash_shader_program(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.shader_programs.lock().unwrap().push(item);
    }

    /// Enqueue resource bindings for deferred destruction.
    pub fn trash_resource_bindings(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.resource_bindings.lock().unwrap().push(item);
    }

    /// Enqueue a graphics pipeline for deferred destruction.
    pub fn trash_graphics_pipeline(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.graphics_pipelines.lock().unwrap().push(item);
    }

    /// Enqueue a compute pipeline for deferred destruction.
    pub fn trash_compute_pipeline(&self, item: GarbageItem) {
        self.assert_not_destroying();
        self.compute_pipelines.lock().unwrap().push(item);
    }

    // -------------------------------------------------------------------
    // Collection (single-threaded, called at EndFrame)
    // -------------------------------------------------------------------

    /// Destroy all trashed objects whose in-flight command-buffer bits have all
    /// been retired.
    ///
    /// `consumed_bits` is the complement of the queue's current in-flight mask:
    /// a bit is "consumed" when its command buffer has finished executing.
    ///
    /// An object is safe to destroy when `(item.inflight_bits & !consumed_bits) == 0`,
    /// i.e. every CB slot that referenced the object is now retired.
    ///
    /// This mirrors `_EmptyTrash` in the C++ implementation. Not thread-safe;
    /// call only from the render thread at EndFrame.
    pub fn perform_garbage_collection(&mut self, consumed_bits: u64) {
        self.is_destroying
            .store(true, std::sync::atomic::Ordering::SeqCst);

        Self::drain_ready(&self.buffers, consumed_bits);
        Self::drain_ready(&self.textures, consumed_bits);
        Self::drain_ready(&self.samplers, consumed_bits);
        Self::drain_ready(&self.shader_functions, consumed_bits);
        Self::drain_ready(&self.shader_programs, consumed_bits);
        Self::drain_ready(&self.resource_bindings, consumed_bits);
        Self::drain_ready(&self.graphics_pipelines, consumed_bits);
        Self::drain_ready(&self.compute_pipelines, consumed_bits);

        self.is_destroying
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Drain all items from `queue` that are safe to destroy.
    ///
    /// An item is safe when none of its in-flight bits are still live:
    ///   `(item.inflight_bits & !consumed_bits) == 0`
    ///
    /// We use swap-remove (O(1)) to avoid shifting, matching the C++ approach
    /// of `iter_swap + pop_back`.
    fn drain_ready(queue: &Mutex<Vec<GarbageItem>>, consumed_bits: u64) {
        let mut items = queue.lock().unwrap();
        let mut i = items.len();
        while i > 0 {
            i -= 1;
            // Any in-flight bit that is NOT consumed means the CB is still live.
            if (items[i].inflight_bits & !consumed_bits) == 0 {
                // Safe to destroy: swap-remove and invoke destructor.
                let item = items.swap_remove(i);
                (item.destructor)();
            }
        }
    }

    /// Panic-guard: trashing while collection is running is a coding error,
    /// equivalent to `TF_CODING_ERROR` + spin-wait in the C++ version.
    fn assert_not_destroying(&self) {
        if self.is_destroying.load(std::sync::atomic::Ordering::SeqCst) {
            // In C++ this was a TF_CODING_ERROR + spin. We panic here because
            // there is no safe way to recover; this indicates a threading bug.
            panic!("HgiVulkanGarbageCollector: cannot trash objects during collection");
        }
    }
}

// -------------------------------------------------------------------
// Default
// -------------------------------------------------------------------

impl Default for HgiVulkanGarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

// -------------------------------------------------------------------
// Drop: force-destroy everything that is still pending.
//
// In normal operation all items should have been collected before Drop.
// If any remain (e.g. device is being torn down) we destroy them
// unconditionally and emit a warning — matching the spirit of C++ RAII
// where all resources must be freed before the device is destroyed.
// -------------------------------------------------------------------

impl Drop for HgiVulkanGarbageCollector {
    fn drop(&mut self) {
        // consumed_bits = u64::MAX means every bit is consumed, so all items
        // satisfy the `(inflight_bits & !consumed_bits) == 0` condition.
        let all_consumed: u64 = u64::MAX;

        let mut warned = false;
        let warn_once = |count: usize, kind: &str| {
            log::warn!(
                "HgiVulkanGarbageCollector dropped with {} uncollected {} — force-destroying",
                count,
                kind
            );
        };

        macro_rules! force_drain {
            ($field:expr, $kind:expr) => {{
                let mut items = $field.lock().unwrap();
                if !items.is_empty() {
                    if !warned {
                        warned = true;
                    }
                    warn_once(items.len(), $kind);
                    // Drain unconditionally.
                    for item in items.drain(..) {
                        (item.destructor)();
                    }
                }
            }};
        }

        force_drain!(self.buffers, "buffers");
        force_drain!(self.textures, "textures");
        force_drain!(self.samplers, "samplers");
        force_drain!(self.shader_functions, "shader_functions");
        force_drain!(self.shader_programs, "shader_programs");
        force_drain!(self.resource_bindings, "resource_bindings");
        force_drain!(self.graphics_pipelines, "graphics_pipelines");
        force_drain!(self.compute_pipelines, "compute_pipelines");

        // Suppress unused-variable warning when all queues were empty.
        let _ = (all_consumed, warned);
    }
}
