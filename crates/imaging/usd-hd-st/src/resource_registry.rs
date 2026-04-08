//! HdStResourceRegistry - Central registry for Storm GPU resources.
//!
//! The resource registry manages all GPU resources (buffers, textures, etc.)
//! for Storm. It handles allocation, deallocation, and sharing of resources.
//!
//! Port of pxr/imaging/hdSt/resourceRegistry.h and .cpp
//!
//! Key concepts:
//! - **BufferArrayRange (BAR)**: A sub-range within a shared GPU buffer.
//!   Multiple draw items share one big buffer, each gets offset+size.
//! - **PendingSource**: Queued CPU data waiting to be uploaded to GPU.
//! - **Commit**: Resolves pending sources, resizes BARs, copies data to GPU.
//! - **GarbageCollect**: Frees unused BARs and buffer resources.
//!
//! When constructed with Hgi (via `new_with_hgi`), uses real GPU allocation.
//! Without Hgi (via `new`), uses mock handles for headless/testing.

use crate::buffer_resource::{HdStBufferResource, HdStBufferResourceSharedPtr};
use crate::mesh::HdStMeshTopology;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, Weak};
use usd_hd::render::HdResourceRegistry;
use usd_hgi::blit_cmds::{HgiBlitCmds, HgiBufferCpuToGpuOp, RawCpuBuffer};
use usd_hgi::compute_cmds::{HgiComputeCmds, HgiComputeDispatchOp};
use usd_hgi::compute_pipeline::HgiComputePipelineHandle;
use usd_hgi::enums::HgiSubmitWaitType;
use usd_hgi::resource_bindings::HgiResourceBindingsHandle;
use usd_hgi::{
    HgiBufferDesc, HgiBufferHandle, HgiBufferUsage, HgiComputeCmdsDesc, HgiDriverHandle,
    HgiGraphicsPipelineHandle, HgiShaderProgramHandle,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Buffer source for queuing CPU->GPU uploads
// ---------------------------------------------------------------------------

/// A buffer source: named CPU data to be uploaded to a BAR.
///
/// Port of HdBufferSource concept. Holds raw bytes + metadata.
#[derive(Debug, Clone)]
pub struct BufferSource {
    /// Name/role of the data (e.g. "points", "normals").
    pub name: Token,
    /// Raw CPU data bytes.
    pub data: Vec<u8>,
    /// Number of elements.
    pub num_elements: usize,
    /// Byte size per element.
    pub element_size: usize,
    /// Whether this source has been resolved.
    pub resolved: bool,
}

impl BufferSource {
    /// Create a new buffer source from raw data.
    pub fn new(name: Token, data: Vec<u8>, num_elements: usize, element_size: usize) -> Self {
        Self {
            name,
            data,
            num_elements,
            element_size,
            resolved: true, // pre-resolved by default
        }
    }

    /// Total byte size.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }

    /// Check if resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Check validity.
    pub fn is_valid(&self) -> bool {
        !self.data.is_empty() && self.num_elements > 0
    }
}

/// Shared pointer to buffer source.
pub type BufferSourceSharedPtr = Arc<BufferSource>;

// ---------------------------------------------------------------------------
// Buffer spec for describing required buffer layout
// ---------------------------------------------------------------------------

/// Description of a buffer's layout within a BAR.
///
/// Port of HdBufferSpec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferSpec {
    /// Name of the buffer (e.g. "points").
    pub name: Token,
    /// Number of elements.
    pub num_elements: usize,
    /// Byte size per element.
    pub element_size: usize,
}

/// Usage hint flags for buffer arrays.
///
/// Port of HdBufferArrayUsageHint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BufferArrayUsageHint {
    /// Immutable after initial upload.
    pub immutable: bool,
    /// Used as vertex data.
    pub vertex: bool,
    /// Used as index data.
    pub index: bool,
    /// Used as uniform data.
    pub uniform: bool,
    /// Used as storage data.
    pub storage: bool,
}

/// Convert BufferArrayUsageHint to HgiBufferUsage flags.
fn usage_hint_to_hgi(hint: BufferArrayUsageHint) -> HgiBufferUsage {
    let mut usage = HgiBufferUsage::empty();
    if hint.vertex {
        usage |= HgiBufferUsage::VERTEX;
    }
    if hint.index {
        usage |= HgiBufferUsage::INDEX32;
    }
    if hint.uniform {
        usage |= HgiBufferUsage::UNIFORM;
    }
    if hint.storage {
        usage |= HgiBufferUsage::STORAGE;
    }
    // Default: VERTEX | STORAGE if nothing specified
    if usage.is_empty() {
        usage = HgiBufferUsage::VERTEX | HgiBufferUsage::STORAGE;
    }
    usage
}

/// Compute queue for GPU computations.
///
/// Port of HdStComputeQueue. Synchronization barriers are inserted between
/// queues but not within a queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeQueue {
    Queue0 = 0,
    Queue1,
    Queue2,
    Queue3,
}

/// A pending GPU computation (port of HdStComputation).
///
/// Port of HdStComputationSharedPtr usage in AddComputation.
/// Holds an optional execute callback invoked during the commit phase.
pub struct GpuComputation {
    /// Debug name of the computation.
    pub name: String,
    /// Target BAR this computation writes to.
    pub target_bar: Option<ManagedBarSharedPtr>,
    /// Queue this computation should run in.
    pub queue: ComputeQueue,
    /// Execute callback: called during commit with a reference to the registry.
    ///
    /// When `Some`, the closure is consumed and called exactly once in queue order.
    /// When `None`, the computation is a no-op placeholder (useful for tests).
    pub execute_fn: Option<Box<dyn FnOnce(&HdStResourceRegistry) + Send>>,
}

impl std::fmt::Debug for GpuComputation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuComputation")
            .field("name", &self.name)
            .field("queue", &self.queue)
            .field("has_execute_fn", &self.execute_fn.is_some())
            .finish()
    }
}

/// Registered shader program entry for the shader registry.
///
/// Port of HdInstance<HdStGLSLProgramSharedPtr> concept.
#[derive(Debug)]
pub struct ShaderRegistryEntry {
    /// Hash ID used for deduplication.
    pub id: u64,
    /// Compiled shader program handle.
    pub program: Weak<HgiShaderProgramHandle>,
}

/// Registered pipeline entry for HGI pipeline registry.
///
/// Port of HdInstance<HgiGraphicsPipelineSharedPtr> concept.
#[derive(Debug)]
pub struct PipelineRegistryEntry {
    /// Hash ID for deduplication.
    pub id: u64,
    /// Graphics pipeline handle.
    pub pipeline: Arc<HgiGraphicsPipelineHandle>,
}

/// Dispatch buffer: an indirect draw/compute dispatch buffer.
///
/// Port of HdStDispatchBuffer. Holds `count * command_num_uints * 4` bytes.
#[derive(Debug)]
pub struct DispatchBuffer {
    /// Role/debug name.
    pub role: Token,
    /// Number of dispatch commands.
    pub count: usize,
    /// u32 per command entry.
    pub command_num_uints: usize,
    /// Backing GPU buffer.
    pub buffer: HdStBufferResourceSharedPtr,
}

impl DispatchBuffer {
    /// Total byte size of the dispatch buffer.
    pub fn byte_size(&self) -> usize {
        self.count * self.command_num_uints * std::mem::size_of::<u32>()
    }
}

/// Shared pointer to dispatch buffer.
pub type DispatchBufferSharedPtr = Arc<DispatchBuffer>;

// ---------------------------------------------------------------------------
// BufferArrayRange (BAR) - shared buffer sub-range
// ---------------------------------------------------------------------------

/// ID for tracking BARs in the registry.
type BarId = u64;

/// A managed BufferArrayRange: offset+size within a shared GPU buffer.
///
/// Port of HdBufferArrayRange concept. Multiple prims share one GPU buffer,
/// each tracked by a BAR with its own offset and element count.
#[derive(Debug)]
pub struct ManagedBar {
    /// Unique ID for this BAR.
    pub id: BarId,
    /// Parent buffer resource (the shared GPU buffer).
    pub buffer: HdStBufferResourceSharedPtr,
    /// Role of the data.
    pub role: Token,
    /// Byte offset within the parent buffer.
    pub offset: usize,
    /// Number of elements.
    pub num_elements: usize,
    /// Byte size per element.
    pub element_size: usize,
    /// Version counter (incremented on data change).
    pub version: u64,
    /// Whether this BAR needs reallocation.
    pub needs_realloc: bool,
}

impl ManagedBar {
    /// Total byte size of this range.
    pub fn byte_size(&self) -> usize {
        self.num_elements * self.element_size
    }

    /// Check if valid (has a buffer with allocation).
    pub fn is_valid(&self) -> bool {
        self.buffer.is_valid() && self.num_elements > 0
    }

    /// Resize the range (marks as needing reallocation).
    pub fn resize(&mut self, num_elements: usize) {
        if num_elements != self.num_elements {
            self.num_elements = num_elements;
            self.needs_realloc = true;
            self.version += 1;
        }
    }
}

/// Shared pointer to managed BAR.
pub type ManagedBarSharedPtr = Arc<Mutex<ManagedBar>>;

// ---------------------------------------------------------------------------
// Pending source: queued data for commit
// ---------------------------------------------------------------------------

/// A pending source: BAR + sources to upload during commit.
///
/// Port of HdStResourceRegistry::_PendingSource.
struct PendingSource {
    /// Target BAR (None for CPU-only computations).
    bar: Option<ManagedBarSharedPtr>,
    /// Buffer sources to upload.
    sources: Vec<BufferSourceSharedPtr>,
}

// ---------------------------------------------------------------------------
// Buffer array pool: manages shared buffers by role
// ---------------------------------------------------------------------------

/// Pool of shared buffers grouped by role.
///
/// Each role (e.g. "points", "normals") has one or more shared GPU buffers.
/// BARs are sub-allocated from these buffers.
struct BufferArrayPool {
    /// Shared buffers by role -> list of (buffer, current_fill_offset).
    pools: HashMap<Token, Vec<(HdStBufferResourceSharedPtr, usize)>>,
    /// Default capacity for new pool buffers.
    default_capacity: usize,
}

impl BufferArrayPool {
    fn new(default_capacity: usize) -> Self {
        Self {
            pools: HashMap::new(),
            default_capacity,
        }
    }

    /// Sub-allocate a range from the pool.
    /// Returns (buffer, offset) for the requested byte_size.
    fn allocate(
        &mut self,
        role: &Token,
        byte_size: usize,
        alloc_fn: &dyn Fn(usize) -> HdStBufferResourceSharedPtr,
    ) -> (HdStBufferResourceSharedPtr, usize) {
        let pool = self.pools.entry(role.clone()).or_default();

        // Try to fit in existing buffer
        for (buf, fill) in pool.iter_mut() {
            if *fill + byte_size <= buf.get_size() {
                let offset = *fill;
                *fill += byte_size;
                return (buf.clone(), offset);
            }
        }

        // Allocate new buffer (at least default_capacity or requested size)
        let new_size = self.default_capacity.max(byte_size * 2);
        let new_buf = alloc_fn(new_size);
        let offset = 0;
        pool.push((new_buf.clone(), byte_size));
        (new_buf, offset)
    }

    /// Drop all pooled buffers.
    fn clear(&mut self) {
        self.pools.clear();
    }

    /// Remove pools with only one reference (registry itself).
    fn garbage_collect(&mut self) {
        for pool in self.pools.values_mut() {
            pool.retain(|(buf, _)| Arc::strong_count(buf) > 1);
        }
        self.pools.retain(|_, pool| !pool.is_empty());
    }
}

// ---------------------------------------------------------------------------
// HdStResourceRegistry
// ---------------------------------------------------------------------------

/// Storm resource registry.
///
/// Manages all GPU resources for the Storm render delegate.
/// Handles buffer allocation, texture management, and resource sharing.
///
/// Port of pxr/imaging/hdSt/resourceRegistry.h
///
/// Key APIs:
/// - `allocate_non_uniform_bar()`: Sub-allocate a BAR from shared buffer pool
/// - `update_non_uniform_bar()`: Resize/migrate a BAR
/// - `add_sources()`: Queue buffer data for GPU upload
/// - `commit()`: Upload all queued data, reallocate as needed
/// - `garbage_collect()`: Free unused BARs and buffers
///
/// # Thread Safety
///
/// The registry is thread-safe and can be accessed from multiple threads
/// during parallel scene processing.
pub struct HdStResourceRegistry {
    /// HGI for real GPU allocation (None = mock handles)
    hgi: Option<HgiDriverHandle>,

    /// Global blit commands (created on demand, reset after submit)
    blit_cmds: Mutex<Option<Box<dyn HgiBlitCmds>>>,

    /// Allocated buffers (keyed by ID)
    buffers: Arc<Mutex<HashMap<u64, HdStBufferResourceSharedPtr>>>,

    /// Next buffer ID
    next_buffer_id: Arc<Mutex<u64>>,

    /// Total allocated memory in bytes
    allocated_memory: Arc<Mutex<usize>>,

    // --- BAR management (port of C++ buffer array registries) ---
    /// Non-uniform buffer array pool (vertex, varying, facevarying primvars)
    non_uniform_pool: Mutex<BufferArrayPool>,

    /// Uniform buffer pool (shader globals via UBO)
    uniform_pool: Mutex<BufferArrayPool>,

    /// All managed BARs keyed by ID
    managed_bars: Mutex<HashMap<BarId, ManagedBarSharedPtr>>,

    /// Next BAR ID
    next_bar_id: Mutex<BarId>,

    /// Pending sources queued for commit
    pending_sources: Mutex<Vec<PendingSource>>,

    /// Pending staging size (bytes to upload)
    pending_staging_size: Mutex<usize>,

    // --- Shader-storage BAR pool (SSBO) ---
    /// Shader storage buffer pool (large primvar arrays via SSBO)
    shader_storage_pool: Mutex<BufferArrayPool>,

    // --- Single buffer pool (for nested instancers) ---
    /// Single-item buffer pool.
    single_pool: Mutex<BufferArrayPool>,

    // --- GPU computation queue ---
    /// Pending GPU computations per queue (executed in order at commit).
    pending_computations: Mutex<Vec<GpuComputation>>,

    // --- Dispatch buffers ---
    /// Registered dispatch buffers (indirect draw/compute).
    dispatch_buffers: Mutex<Vec<DispatchBufferSharedPtr>>,

    // --- Shader / pipeline registries ---
    /// Shader program registry keyed by hash ID.
    shader_registry: Mutex<HashMap<u64, Arc<HgiShaderProgramHandle>>>,

    /// Graphics pipeline registry keyed by hash ID.
    graphics_pipeline_registry: Mutex<HashMap<u64, PipelineRegistryEntry>>,

    /// Whether the shader registry has been invalidated (triggers recompile).
    shader_registry_dirty: Mutex<bool>,

    /// Compute pipeline cache (hash-only sentinel for dedup).
    compute_pipeline_cache: Mutex<HashSet<u64>>,
    /// Resource bindings cache (hash-only sentinel for dedup).
    resource_bindings_cache: Mutex<HashSet<u64>>,
    /// ExtComputation data range dedup cache (hash -> already allocated).
    ext_computation_data_cache: Mutex<HashSet<u64>>,

    // --- Global compute command encoder (C++ GetGlobalComputeCmds) ---
    /// Shared compute command buffer for all ExtComputation dispatches in a frame.
    /// Created lazily on first use, submitted via `submit_compute_work()`.
    global_compute_cmds: Mutex<Option<Box<dyn HgiComputeCmds>>>,

    // --- ExtComputation sprim input BAR registry ---
    /// Maps computation prim path -> SSBO input range.
    ///
    /// Populated during HdStExtComputation::sync (sprim sync phase).
    /// Queried during rprim sync to wire GPU compute input BARs.
    /// Port of C++ `renderIndex.GetSprim() -> GetInputRange()` pattern.
    ext_comp_input_bars: Mutex<HashMap<SdfPath, ManagedBarSharedPtr>>,

    // --- Mesh topology deduplication registry ---
    /// Maps topology content hash -> shared HdStMeshTopology instance.
    ///
    /// Port of HdStResourceRegistry topology sharing:
    /// prims with identical topology share one GPU buffer set, avoiding
    /// redundant face_vertex_counts / face_vertex_indices uploads.
    topology_registry: Mutex<HashMap<u64, Arc<HdStMeshTopology>>>,
}

impl HdStResourceRegistry {
    /// Create a new resource registry without Hgi (mock handles for headless).
    pub fn new() -> Self {
        Self {
            hgi: None,
            blit_cmds: Mutex::new(None),
            buffers: Arc::new(Mutex::new(HashMap::new())),
            next_buffer_id: Arc::new(Mutex::new(1)),
            allocated_memory: Arc::new(Mutex::new(0)),
            // Pool sizes tuned to match Storm VBO/IMM memory manager defaults:
            // non-uniform (interleaved VBOs): 4MB per backing store
            // uniform (constant primvars, std140): 256KB per backing store
            // SSBO (shader storage / indirect buffers): 16MB per backing store
            // single (per-prim single-element buffers): 256KB per backing store
            non_uniform_pool: Mutex::new(BufferArrayPool::new(4 * 1024 * 1024)), // 4MB
            uniform_pool: Mutex::new(BufferArrayPool::new(256 * 1024)),          // 256KB
            shader_storage_pool: Mutex::new(BufferArrayPool::new(16 * 1024 * 1024)), // 16MB
            single_pool: Mutex::new(BufferArrayPool::new(256 * 1024)),           // 256KB
            managed_bars: Mutex::new(HashMap::new()),
            next_bar_id: Mutex::new(1),
            pending_sources: Mutex::new(Vec::new()),
            pending_staging_size: Mutex::new(0),
            pending_computations: Mutex::new(Vec::new()),
            dispatch_buffers: Mutex::new(Vec::new()),
            shader_registry: Mutex::new(HashMap::new()),
            graphics_pipeline_registry: Mutex::new(HashMap::new()),
            shader_registry_dirty: Mutex::new(false),
            compute_pipeline_cache: Mutex::new(HashSet::new()),
            resource_bindings_cache: Mutex::new(HashSet::new()),
            ext_computation_data_cache: Mutex::new(HashSet::new()),
            global_compute_cmds: Mutex::new(None),
            ext_comp_input_bars: Mutex::new(HashMap::new()),
            topology_registry: Mutex::new(HashMap::new()),
        }
    }

    /// Create resource registry with Hgi for real GPU allocation.
    pub fn new_with_hgi(hgi: HgiDriverHandle) -> Self {
        Self {
            hgi: Some(hgi),
            blit_cmds: Mutex::new(None),
            buffers: Arc::new(Mutex::new(HashMap::new())),
            next_buffer_id: Arc::new(Mutex::new(1)),
            allocated_memory: Arc::new(Mutex::new(0)),
            non_uniform_pool: Mutex::new(BufferArrayPool::new(4 * 1024 * 1024)), // 4MB
            uniform_pool: Mutex::new(BufferArrayPool::new(256 * 1024)),          // 256KB
            shader_storage_pool: Mutex::new(BufferArrayPool::new(16 * 1024 * 1024)), // 16MB
            single_pool: Mutex::new(BufferArrayPool::new(256 * 1024)),           // 256KB
            managed_bars: Mutex::new(HashMap::new()),
            next_bar_id: Mutex::new(1),
            pending_sources: Mutex::new(Vec::new()),
            pending_staging_size: Mutex::new(0),
            pending_computations: Mutex::new(Vec::new()),
            dispatch_buffers: Mutex::new(Vec::new()),
            shader_registry: Mutex::new(HashMap::new()),
            graphics_pipeline_registry: Mutex::new(HashMap::new()),
            shader_registry_dirty: Mutex::new(false),
            compute_pipeline_cache: Mutex::new(HashSet::new()),
            resource_bindings_cache: Mutex::new(HashSet::new()),
            ext_computation_data_cache: Mutex::new(HashSet::new()),
            global_compute_cmds: Mutex::new(None),
            ext_comp_input_bars: Mutex::new(HashMap::new()),
            topology_registry: Mutex::new(HashMap::new()),
        }
    }

    /// Get Hgi if available.
    pub fn get_hgi(&self) -> Option<&HgiDriverHandle> {
        self.hgi.as_ref()
    }

    /// Drop all GPU resources: buffers, pools, dispatch, shaders, pipelines, topology.
    ///
    /// Must be called BEFORE the wgpu device is dropped to ensure all wgpu::Buffer
    /// handles are freed while the device is still alive. Without this, stale buffer
    /// IDs survive device death and cause `Buffer does not exist` panics on reuse.
    pub fn clear_all_gpu_resources(&self) {
        self.buffers.lock().expect("buffers lock").clear();
        *self.next_buffer_id.lock().expect("next_buffer_id lock") = 1;
        *self.allocated_memory.lock().expect("allocated_memory lock") = 0;
        self.non_uniform_pool.lock().expect("pool lock").clear();
        self.uniform_pool.lock().expect("pool lock").clear();
        self.shader_storage_pool.lock().expect("pool lock").clear();
        self.single_pool.lock().expect("pool lock").clear();
        self.managed_bars.lock().expect("bars lock").clear();
        *self.next_bar_id.lock().expect("bar_id lock") = 1;
        self.pending_sources.lock().expect("pending lock").clear();
        *self.pending_staging_size.lock().expect("staging lock") = 0;
        self.pending_computations.lock().expect("comp lock").clear();
        self.dispatch_buffers.lock().expect("dispatch lock").clear();
        self.shader_registry.lock().expect("shader lock").clear();
        self.graphics_pipeline_registry
            .lock()
            .expect("pipeline lock")
            .clear();
        *self.shader_registry_dirty.lock().expect("dirty lock") = false;
        self.compute_pipeline_cache
            .lock()
            .expect("compute_pipe lock")
            .clear();
        self.resource_bindings_cache
            .lock()
            .expect("rb_cache lock")
            .clear();
        self.ext_computation_data_cache
            .lock()
            .expect("extcomp_data lock")
            .clear();
        self.ext_comp_input_bars
            .lock()
            .expect("ext_comp_bars lock")
            .clear();
        self.topology_registry.lock().expect("topo lock").clear();
        *self.blit_cmds.lock().expect("blit lock") = None;
    }

    // ------------------------------------------------------------------
    // Simple buffer allocation (existing API, unchanged)
    // ------------------------------------------------------------------

    /// Allocate a GPU buffer with default usage (STORAGE | UNIFORM).
    pub fn allocate_buffer(&self, size: usize) -> HdStBufferResourceSharedPtr {
        self.allocate_buffer_with_usage(HgiBufferUsage::STORAGE | HgiBufferUsage::UNIFORM, size)
    }

    /// Allocate a vertex buffer with stride info (VERTEX | STORAGE).
    pub fn allocate_vertex_buffer(
        &self,
        size: usize,
        vertex_stride: u32,
    ) -> HdStBufferResourceSharedPtr {
        let mut buffer = HdStBufferResource::with_size(size);
        let usage = HgiBufferUsage::VERTEX | HgiBufferUsage::STORAGE;

        let handle = if let Some(ref hgi) = self.hgi {
            let desc = HgiBufferDesc::new()
                .with_usage(usage)
                .with_byte_size(size)
                .with_vertex_stride(vertex_stride);
            hgi.with_write(|h| h.create_buffer(&desc, None))
        } else {
            let mut next_id = self
                .next_buffer_id
                .lock()
                .expect("Failed to lock next_buffer_id mutex");
            let id = *next_id;
            *next_id += 1;
            HgiBufferHandle::with_id(id)
        };

        let id = handle.id();
        buffer.set_allocation(handle, size);

        let buffer_ptr = Arc::new(buffer);
        self.buffers
            .lock()
            .expect("lock")
            .insert(id, buffer_ptr.clone());
        buffer_ptr
    }

    /// Allocate a GPU buffer with specified usage.
    pub fn allocate_buffer_with_usage(
        &self,
        usage: HgiBufferUsage,
        size: usize,
    ) -> HdStBufferResourceSharedPtr {
        let mut buffer = HdStBufferResource::with_size(size);

        let handle = if let Some(ref hgi) = self.hgi {
            let desc = HgiBufferDesc::new().with_usage(usage).with_byte_size(size);
            hgi.with_write(|h| h.create_buffer(&desc, None))
        } else {
            let mut next_id = self
                .next_buffer_id
                .lock()
                .expect("Failed to lock next_buffer_id mutex");
            let id = *next_id;
            *next_id += 1;
            HgiBufferHandle::with_id(id)
        };

        let id = handle.id();
        buffer.set_allocation(handle, size);

        let buffer_ptr = Arc::new(buffer);

        {
            let mut buffers = self.buffers.lock().expect("Failed to lock buffers mutex");
            buffers.insert(id, buffer_ptr.clone());
        }

        {
            let mut mem = self
                .allocated_memory
                .lock()
                .expect("Failed to lock allocated_memory mutex");
            *mem += size;
        }

        buffer_ptr
    }

    /// Free a GPU buffer by handle.
    pub fn free_buffer(&self, handle: &HgiBufferHandle) {
        let id = handle.id();
        let mut buffers = self.buffers.lock().expect("Failed to lock buffers mutex");

        if let Some(buffer) = buffers.remove(&id) {
            if let Some(ref hgi) = self.hgi {
                hgi.with_write(|h| h.destroy_buffer(handle));
            }
            let mut mem = self
                .allocated_memory
                .lock()
                .expect("Failed to lock allocated_memory mutex");
            *mem = mem.saturating_sub(buffer.get_size());
        }
    }

    /// Free a GPU buffer by ID.
    pub fn free_buffer_by_id(&self, id: u64) {
        let mut buffers = self.buffers.lock().expect("Failed to lock buffers mutex");

        if let Some(buffer) = buffers.remove(&id) {
            let mut mem = self
                .allocated_memory
                .lock()
                .expect("Failed to lock allocated_memory mutex");
            *mem = mem.saturating_sub(buffer.get_size());
        }
    }

    /// Get total allocated memory in bytes.
    pub fn get_allocated_memory(&self) -> usize {
        *self
            .allocated_memory
            .lock()
            .expect("Failed to lock allocated_memory mutex")
    }

    /// Get number of allocated buffers.
    pub fn get_buffer_count(&self) -> usize {
        self.buffers
            .lock()
            .expect("Failed to lock buffers mutex")
            .len()
    }

    /// Get resource allocation stats for render stats reporting.
    ///
    /// Returns a map of stat name -> byte count, matching C++
    /// `HdStResourceRegistry::GetResourceAllocation()`.
    pub fn get_resource_allocation(&self) -> std::collections::HashMap<String, u64> {
        let mut map = std::collections::HashMap::new();
        let gpu_mem = self.get_allocated_memory() as u64;
        map.insert("gpuMemoryUsed".to_string(), gpu_mem);
        map.insert("bufferCount".to_string(), self.get_buffer_count() as u64);
        // Texture memory tracked separately; start at 0 until texture registry is wired in
        map.insert("textureMemory".to_string(), 0u64);
        map
    }

    // ------------------------------------------------------------------
    // BAR allocation API (port of C++ AllocateNonUniform*, AllocateUniform*)
    // ------------------------------------------------------------------

    /// Allocate a non-uniform buffer array range.
    ///
    /// Port of HdStResourceRegistry::AllocateNonUniformBufferArrayRange.
    /// Sub-allocates a range within a shared vertex/primvar buffer pool.
    /// Multiple prims share the same GPU buffer, each with its own offset+size.
    pub fn allocate_non_uniform_bar(
        &self,
        role: &Token,
        specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        // Calculate total byte size from specs
        let byte_size: usize = specs.iter().map(|s| s.num_elements * s.element_size).sum();
        let element_size = if specs.is_empty() {
            1
        } else {
            specs[0].element_size
        };
        // Ceiling division: mixed-size primvars (e.g. pos=12B, st=8B) packed into
        // one buffer must not lose the tail bytes via floor division.
        let num_elements = if element_size > 0 {
            (byte_size + element_size - 1) / element_size
        } else {
            0
        };

        // Map usage hint to HGI buffer usage flags
        let gpu_usage = usage_hint_to_hgi(usage_hint);

        // Allocate from pool — use vertex_buffer path when VERTEX usage
        // so vertex_stride is set (avoids wgpu warning).
        let vertex_stride = if gpu_usage.contains(HgiBufferUsage::VERTEX) {
            element_size as u32
        } else {
            0
        };
        let alloc_fn = |size: usize| -> HdStBufferResourceSharedPtr {
            if vertex_stride > 0 {
                self.allocate_vertex_buffer(size, vertex_stride)
            } else {
                self.allocate_buffer_with_usage(gpu_usage, size)
            }
        };

        let (buffer, offset) = {
            let mut pool = self.non_uniform_pool.lock().expect("pool lock");
            pool.allocate(role, byte_size, &alloc_fn)
        };

        // Create managed BAR
        let bar_id = {
            let mut id = self.next_bar_id.lock().expect("bar_id lock");
            let current = *id;
            *id += 1;
            current
        };

        let bar = Arc::new(Mutex::new(ManagedBar {
            id: bar_id,
            buffer,
            role: role.clone(),
            offset,
            num_elements,
            element_size,
            version: 0,
            needs_realloc: false,
        }));

        self.managed_bars
            .lock()
            .expect("bars lock")
            .insert(bar_id, bar.clone());

        bar
    }

    /// Allocate a uniform buffer array range (for shader globals/UBOs).
    ///
    /// Port of HdStResourceRegistry::AllocateUniformBufferArrayRange.
    pub fn allocate_uniform_bar(
        &self,
        role: &Token,
        specs: &[BufferSpec],
        _usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        let byte_size: usize = specs.iter().map(|s| s.num_elements * s.element_size).sum();
        let element_size = if specs.is_empty() {
            1
        } else {
            specs[0].element_size
        };
        // Ceiling division: avoids truncation when packing mixed-size primvars.
        let num_elements = if element_size > 0 {
            (byte_size + element_size - 1) / element_size
        } else {
            0
        };

        let alloc_fn = |size: usize| -> HdStBufferResourceSharedPtr {
            self.allocate_buffer_with_usage(HgiBufferUsage::UNIFORM | HgiBufferUsage::STORAGE, size)
        };

        let (buffer, offset) = {
            let mut pool = self.uniform_pool.lock().expect("pool lock");
            pool.allocate(role, byte_size, &alloc_fn)
        };

        let bar_id = {
            let mut id = self.next_bar_id.lock().expect("bar_id lock");
            let current = *id;
            *id += 1;
            current
        };

        let bar = Arc::new(Mutex::new(ManagedBar {
            id: bar_id,
            buffer,
            role: role.clone(),
            offset,
            num_elements,
            element_size,
            version: 0,
            needs_realloc: false,
        }));

        self.managed_bars
            .lock()
            .expect("bars lock")
            .insert(bar_id, bar.clone());

        bar
    }

    // ------------------------------------------------------------------
    // BAR update/migration API (port of C++ UpdateNonUniform*)
    // ------------------------------------------------------------------

    /// Update a non-uniform BAR: resize or migrate if specs changed.
    ///
    /// Port of HdStResourceRegistry::UpdateNonUniformBufferArrayRange.
    /// If `cur_bar` is None, equivalent to allocate_non_uniform_bar.
    /// Otherwise, checks if the BAR needs migration (specs changed) and
    /// allocates a new range if necessary.
    pub fn update_non_uniform_bar(
        &self,
        role: &Token,
        cur_bar: Option<&ManagedBarSharedPtr>,
        updated_specs: &[BufferSpec],
        _removed_specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        // If no current range, just allocate fresh
        let cur = match cur_bar {
            Some(bar) => bar,
            None => return self.allocate_non_uniform_bar(role, updated_specs, usage_hint),
        };

        // Check if current range is still valid and big enough
        let needs_migrate = {
            let locked = cur.lock().expect("bar lock");
            let new_byte_size: usize = updated_specs
                .iter()
                .map(|s| s.num_elements * s.element_size)
                .sum();
            !locked.is_valid() || locked.byte_size() < new_byte_size || locked.needs_realloc
        };

        if needs_migrate {
            // Allocate a new BAR. Old GPU data is NOT copied here.
            // C++ HdStResourceRegistry::UpdateNonUniformBufferArrayRange uses
            // HdStCopyComputationGPU for a GPU-to-GPU blit after reallocation.
            // In our wgpu backend, HdStBufferResource holds only a GPU handle
            // with no CPU mirror, so we cannot read back the old contents here.
            // This is safe: callers (HdStMesh::Sync, HdStBasisCurves::Sync etc.)
            // always re-queue all sources via add_sources() immediately after
            // calling update_non_uniform_bar(), so data is fully re-uploaded on
            // the next commit() cycle.
            log::warn!(
                "ResourceRegistry::update_non_uniform_bar: reallocating BAR without GPU-to-GPU migration (data re-queued by caller)"
            );
            let new_bar = self.allocate_non_uniform_bar(role, updated_specs, usage_hint);
            new_bar
        } else {
            cur.clone()
        }
    }

    // ------------------------------------------------------------------
    // Resource update & source queuing (port of C++ AddSources/AddSource)
    // ------------------------------------------------------------------

    /// Queue buffer sources for a managed BAR to be committed later.
    ///
    /// Port of HdStResourceRegistry::AddSources.
    /// Sources are uploaded to GPU during the next commit() call.
    pub fn add_sources(&self, bar: &ManagedBarSharedPtr, sources: Vec<BufferSourceSharedPtr>) {
        if sources.is_empty() {
            log::warn!("add_sources: empty sources list");
            return;
        }

        // Validate sources
        let valid_sources: Vec<_> = sources
            .into_iter()
            .filter(|s| {
                if !s.is_valid() {
                    log::warn!("add_sources: invalid source '{}'", s.name.as_str());
                    false
                } else {
                    true
                }
            })
            .collect();

        if valid_sources.is_empty() {
            return;
        }

        // Track staging size
        let staging_size: usize = valid_sources.iter().map(|s| s.byte_size()).sum();
        {
            let mut pending_size = self.pending_staging_size.lock().expect("staging lock");
            *pending_size += staging_size;
        }

        // Queue for commit
        let mut pending = self.pending_sources.lock().expect("pending lock");
        pending.push(PendingSource {
            bar: Some(bar.clone()),
            sources: valid_sources,
        });
    }

    /// Queue a single buffer source for a managed BAR.
    ///
    /// Port of HdStResourceRegistry::AddSource (range + source variant).
    pub fn add_source(&self, bar: &ManagedBarSharedPtr, source: BufferSourceSharedPtr) {
        self.add_sources(bar, vec![source]);
    }

    /// Queue a standalone buffer source (CPU-only, no BAR target).
    ///
    /// Port of HdStResourceRegistry::AddSource (source-only variant).
    pub fn add_standalone_source(&self, source: BufferSourceSharedPtr) {
        if !source.is_valid() {
            log::warn!("add_standalone_source: invalid source");
            return;
        }

        let mut pending = self.pending_sources.lock().expect("pending lock");
        pending.push(PendingSource {
            bar: None,
            sources: vec![source],
        });
    }

    // ------------------------------------------------------------------
    // Commit (port of C++ _Commit)
    // ------------------------------------------------------------------

    /// Commit all pending sources to GPU.
    ///
    /// Port of HdStResourceRegistry::_Commit. Phases:
    /// 1. Resolve: resolve any unresolved buffer sources
    /// 2. Resize: resize BARs if source element count changed
    /// 3. Reallocate: grow shared buffers as needed
    /// 4. Copy: upload CPU data to GPU via blit commands
    /// 5. Flush: submit blit work
    pub fn commit(&self) {
        let mut pending = {
            let mut p = self.pending_sources.lock().expect("pending lock");
            std::mem::take(&mut *p)
        };

        if pending.is_empty() {
            return;
        }

        log::debug!(
            "[ResourceRegistry] commit: {} pending source batches",
            pending.len()
        );

        // Phase 1 trace: drain summary
        for (i, ps) in pending.iter().enumerate() {
            let bar_id = ps.bar.as_ref().map(|b| {
                let l = b.lock().expect("bar lock");
                (l.id, l.role.clone())
            });
            let total_bytes: usize = ps.sources.iter().map(|s| s.byte_size()).sum();
            log::trace!(
                "ResourceRegistry::commit phase1 batch[{}]: bar={:?}, sources={}, total_bytes={}",
                i,
                bar_id,
                ps.sources.len(),
                total_bytes
            );
            for (j, src) in ps.sources.iter().enumerate() {
                log::trace!(
                    "ResourceRegistry::commit phase1 batch[{}].src[{}]: name={}, data_len={}, num_elems={}, elem_size={}",
                    i,
                    j,
                    src.name,
                    src.data.len(),
                    src.num_elements,
                    src.element_size
                );
            }
        }

        // Phase 1: Resolve (all our sources are pre-resolved for now)
        //
        // Phase 2: Resize BARs to match the total data being uploaded.
        //
        // A single BAR may pack multiple primvar sources (e.g. positions +
        // normals) into one contiguous GPU buffer region. We must compute
        // the total byte size across ALL queued sources — not just the
        // first — and derive num_elements from that total.
        //
        // Previously this used `first_src.num_elements` which only counted
        // the first source (positions), shrinking the BAR to half its
        // intended size and leaving no room for normals. That caused wgpu
        // validation failures: the pipeline expected vertex buffer slot 1
        // (normals) but draw_batch couldn't bind it because the BAR was
        // too small.
        for ps in &pending {
            if let Some(ref bar) = ps.bar {
                if ps.sources.is_empty() {
                    continue;
                }
                let total_bytes: usize = ps.sources.iter().map(|s| s.byte_size()).sum();
                let mut locked = bar.lock().expect("bar lock");
                // Ceiling division: round up so the BAR fits all source bytes even
                // when total_bytes is not an exact multiple of element_size.
                let needed_elems = if locked.element_size > 0 {
                    (total_bytes + locked.element_size - 1) / locked.element_size
                } else {
                    total_bytes
                };
                log::trace!(
                    "ResourceRegistry::commit phase2: bar_id={}, cur_num_elements={}, element_size={}, cur_byte_size={}, total_bytes_from_sources={}, needed_elems={}",
                    locked.id,
                    locked.num_elements,
                    locked.element_size,
                    locked.byte_size(),
                    total_bytes,
                    needed_elems
                );
                if locked.num_elements != needed_elems {
                    log::trace!(
                        "ResourceRegistry::commit phase2: RESIZE bar_id={} from {} to {} elems (byte_size {} -> {})",
                        locked.id,
                        locked.num_elements,
                        needed_elems,
                        locked.byte_size(),
                        needed_elems * locked.element_size
                    );
                    locked.resize(needed_elems);
                }
            }
        }

        // Phase 3: Reallocate BARs whose byte size has grown beyond their current buffer.
        // C++ reference: HdStResourceRegistry::_GarbageCollectBufferArrays (resourceRegistry.cpp)
        // does a full repack: compacts all alive ranges into new tightly-packed buffers
        // and frees the old ones. Our simplified version gives each needing-realloc BAR
        // a fresh (possibly larger) buffer; existing GPU data is re-uploaded by the
        // source queue in the next commit() call (add_sources path).
        {
            let bars = self.managed_bars.lock().expect("bars lock");
            for bar in bars.values() {
                let mut locked = bar.lock().expect("bar lock");
                if locked.needs_realloc {
                    let new_size = locked.byte_size();
                    let old_buf_size = locked.buffer.get_size();
                    log::trace!(
                        "ResourceRegistry::commit phase3: REALLOC bar_id={}, old_buf_size={}, new_byte_size={}",
                        locked.id,
                        old_buf_size,
                        new_size
                    );
                    if new_size > 0 {
                        let realloc_size = new_size.max(old_buf_size);
                        let stride = locked.element_size as u32;
                        let new_buf = if stride > 0 {
                            self.allocate_vertex_buffer(realloc_size, stride)
                        } else {
                            self.allocate_buffer_with_usage(
                                HgiBufferUsage::VERTEX | HgiBufferUsage::STORAGE,
                                realloc_size,
                            )
                        };
                        log::trace!(
                            "ResourceRegistry::commit phase3: bar_id={} reallocated, final_size={}",
                            locked.id,
                            realloc_size
                        );
                        locked.buffer = new_buf;
                        locked.offset = 0;
                        locked.needs_realloc = false;
                    }
                }
            }
        }

        // Phase 4: Copy data to GPU
        for ps in &mut pending {
            let bar = match &ps.bar {
                Some(b) => b,
                None => continue,
            };

            let (handle, offset) = {
                let locked = bar.lock().expect("bar lock");
                if !locked.is_valid() || locked.num_elements == 0 {
                    continue;
                }
                (locked.buffer.get_handle().clone(), locked.offset)
            };

            if !handle.is_valid() {
                continue;
            }

            // Write sources at consecutive offsets within the BAR's region.
            // E.g. positions at offset, normals at offset+pos_bytes.
            let bar_byte_size = {
                let l = bar.lock().expect("bar lock");
                l.byte_size()
            };
            let mut write_offset = offset;
            for src in &ps.sources {
                if src.data.is_empty() {
                    continue;
                }
                // Validate: check for buffer overrun (write_offset is absolute pool pos)
                let local_off = write_offset - offset;
                if local_off + src.data.len() > bar_byte_size {
                    log::warn!(
                        "ResourceRegistry::commit phase4: BUFFER OVERRUN! src={}, local_off={} + data_len={} = {} > bar_byte_size={}",
                        src.name,
                        local_off,
                        src.data.len(),
                        local_off + src.data.len(),
                        bar_byte_size
                    );
                }
                log::trace!(
                    "ResourceRegistry::commit phase4: src={}, data_len={}, write_offset_before={}, write_offset_after={}, bar_byte_size={}",
                    src.name,
                    src.data.len(),
                    write_offset,
                    write_offset + src.data.len(),
                    bar_byte_size
                );
                #[allow(unsafe_code)]
                unsafe {
                    self.copy_buffer_cpu_to_gpu(
                        &handle,
                        src.data.as_ptr(),
                        src.data.len(),
                        write_offset,
                    );
                }
                write_offset += src.data.len();
            }
        }

        // Phase 5: Flush - submit blit work
        self.submit_blit_work(HgiSubmitWaitType::NoWait);

        // Phase 6: Execute GPU computations in queue order.
        //
        // Port of HdStResourceRegistry::_Commit compute phase:
        // Queues are executed in order (Queue0 -> Queue1 -> Queue2 -> Queue3).
        // A memory barrier is implied between queues (but within one queue,
        // computations may overlap).
        self.execute_computations();

        // Phase 7: Submit accumulated GPU compute work.
        self.submit_compute_work();

        // Reset staging size
        *self.pending_staging_size.lock().expect("staging lock") = 0;

        log::debug!("[ResourceRegistry] commit complete");
    }

    // ------------------------------------------------------------------
    // Garbage collection (enhanced with BAR support)
    // ------------------------------------------------------------------

    /// Garbage collect unused resources.
    ///
    /// Port of HdStResourceRegistry::_GarbageCollect.
    /// Frees:
    /// - Buffers only referenced by the registry
    /// - Managed BARs with no external references
    /// - Empty buffer array pools
    pub fn garbage_collect(&self) {
        // Clean up managed BARs with no external references
        {
            let mut bars = self.managed_bars.lock().expect("bars lock");
            bars.retain(|_, bar| Arc::strong_count(bar) > 1);
        }

        // Clean up buffer pools
        {
            let mut pool = self.non_uniform_pool.lock().expect("pool lock");
            pool.garbage_collect();
        }
        {
            let mut pool = self.uniform_pool.lock().expect("pool lock");
            pool.garbage_collect();
        }
        {
            let mut pool = self.shader_storage_pool.lock().expect("ssbo pool lock");
            pool.garbage_collect();
        }
        {
            let mut pool = self.single_pool.lock().expect("single pool lock");
            pool.garbage_collect();
        }
        // Clean up dispatch buffers and pipelines
        self.garbage_collect_dispatch_buffers();
        self.garbage_collect_pipelines();
        // GC shared topology entries with no external references
        self.garbage_collect_topologies();
        // Clear dirty flag after GC
        *self.shader_registry_dirty.lock().expect("dirty lock") = false;

        // Clean up standalone buffers
        let mut buffers = self.buffers.lock().expect("Failed to lock buffers mutex");
        let mut mem = self
            .allocated_memory
            .lock()
            .expect("Failed to lock allocated_memory mutex");

        buffers.retain(|_, buffer| {
            let is_used = Arc::strong_count(buffer) > 1;
            if !is_used {
                *mem = mem.saturating_sub(buffer.get_size());
            }
            is_used
        });
    }

    // ------------------------------------------------------------------
    // BAR query API
    // ------------------------------------------------------------------

    /// Get count of managed BARs.
    pub fn get_bar_count(&self) -> usize {
        self.managed_bars.lock().expect("bars lock").len()
    }

    /// Get total pending staging size in bytes.
    pub fn get_pending_staging_size(&self) -> usize {
        *self.pending_staging_size.lock().expect("staging lock")
    }

    // ------------------------------------------------------------------
    // Additional BAR allocation variants (port of C++ API surface)
    // ------------------------------------------------------------------

    /// Allocate an immutable non-uniform BAR.
    ///
    /// Port of HdStResourceRegistry::AllocateNonUniformImmutableBufferArrayRange.
    /// Immutable BARs cannot be resized after initial upload.
    pub fn allocate_non_uniform_immutable_bar(
        &self,
        role: &Token,
        specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        // Immutable uses same pool but marks buffer as immutable for optimization
        let mut hint = usage_hint;
        hint.immutable = true;
        self.allocate_non_uniform_bar(role, specs, hint)
    }

    /// Allocate a shader storage BAR (SSBO).
    ///
    /// Port of HdStResourceRegistry::AllocateShaderStorageBufferArrayRange.
    /// Used for large primvar arrays read via SSBO bindings in shaders.
    pub fn allocate_shader_storage_bar(
        &self,
        role: &Token,
        specs: &[BufferSpec],
        _usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        let byte_size: usize = specs.iter().map(|s| s.num_elements * s.element_size).sum();
        let element_size = if specs.is_empty() {
            1
        } else {
            specs[0].element_size
        };
        // Ceiling division: avoids truncation when packing mixed-size primvars.
        let num_elements = if element_size > 0 {
            (byte_size + element_size - 1) / element_size
        } else {
            0
        };

        let alloc_fn = |size: usize| -> HdStBufferResourceSharedPtr {
            self.allocate_buffer_with_usage(HgiBufferUsage::STORAGE, size)
        };

        let (buffer, offset) = {
            let mut pool = self.shader_storage_pool.lock().expect("ssbo pool lock");
            pool.allocate(role, byte_size, &alloc_fn)
        };

        let bar_id = {
            let mut id = self.next_bar_id.lock().expect("bar_id lock");
            let current = *id;
            *id += 1;
            current
        };

        let bar = Arc::new(Mutex::new(ManagedBar {
            id: bar_id,
            buffer,
            role: role.clone(),
            offset,
            num_elements,
            element_size,
            version: 0,
            needs_realloc: false,
        }));

        self.managed_bars
            .lock()
            .expect("bars lock")
            .insert(bar_id, bar.clone());
        bar
    }

    /// Allocate a single-item BAR (for nested instancers).
    ///
    /// Port of HdStResourceRegistry::AllocateSingleBufferArrayRange.
    pub fn allocate_single_bar(
        &self,
        role: &Token,
        specs: &[BufferSpec],
        _usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        let byte_size: usize = specs.iter().map(|s| s.num_elements * s.element_size).sum();
        let element_size = if specs.is_empty() {
            1
        } else {
            specs[0].element_size
        };
        // Ceiling division: avoids truncation when packing mixed-size primvars.
        let num_elements = if element_size > 0 {
            (byte_size + element_size - 1) / element_size
        } else {
            0
        };

        let alloc_fn = |size: usize| -> HdStBufferResourceSharedPtr {
            self.allocate_buffer_with_usage(HgiBufferUsage::UNIFORM | HgiBufferUsage::STORAGE, size)
        };

        let (buffer, offset) = {
            let mut pool = self.single_pool.lock().expect("single pool lock");
            pool.allocate(role, byte_size, &alloc_fn)
        };

        let bar_id = {
            let mut id = self.next_bar_id.lock().expect("bar_id lock");
            let current = *id;
            *id += 1;
            current
        };

        let bar = Arc::new(Mutex::new(ManagedBar {
            id: bar_id,
            buffer,
            role: role.clone(),
            offset,
            num_elements,
            element_size,
            version: 0,
            needs_realloc: false,
        }));

        self.managed_bars
            .lock()
            .expect("bars lock")
            .insert(bar_id, bar.clone());
        bar
    }

    // ------------------------------------------------------------------
    // BAR update variants for all pool types
    // ------------------------------------------------------------------

    /// Update an immutable non-uniform BAR.
    ///
    /// Port of HdStResourceRegistry::UpdateNonUniformImmutableBufferArrayRange.
    pub fn update_non_uniform_immutable_bar(
        &self,
        role: &Token,
        cur_bar: Option<&ManagedBarSharedPtr>,
        updated_specs: &[BufferSpec],
        removed_specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        // Immutable bars are never migrated — always reallocate fresh.
        // This matches C++ behavior: immutable = written once, read many.
        let _ = (cur_bar, removed_specs);
        self.allocate_non_uniform_immutable_bar(role, updated_specs, usage_hint)
    }

    /// Update a uniform BAR.
    ///
    /// Port of HdStResourceRegistry::UpdateUniformBufferArrayRange.
    pub fn update_uniform_bar(
        &self,
        role: &Token,
        cur_bar: Option<&ManagedBarSharedPtr>,
        updated_specs: &[BufferSpec],
        _removed_specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        let cur = match cur_bar {
            Some(bar) => bar,
            None => return self.allocate_uniform_bar(role, updated_specs, usage_hint),
        };
        let needs_migrate = {
            let locked = cur.lock().expect("bar lock");
            let new_size: usize = updated_specs
                .iter()
                .map(|s| s.num_elements * s.element_size)
                .sum();
            !locked.is_valid() || locked.byte_size() < new_size || locked.needs_realloc
        };
        if needs_migrate {
            self.allocate_uniform_bar(role, updated_specs, usage_hint)
        } else {
            cur.clone()
        }
    }

    /// Update a shader storage BAR.
    ///
    /// Port of HdStResourceRegistry::UpdateShaderStorageBufferArrayRange.
    pub fn update_shader_storage_bar(
        &self,
        role: &Token,
        cur_bar: Option<&ManagedBarSharedPtr>,
        updated_specs: &[BufferSpec],
        _removed_specs: &[BufferSpec],
        usage_hint: BufferArrayUsageHint,
    ) -> ManagedBarSharedPtr {
        let cur = match cur_bar {
            Some(bar) => bar,
            None => return self.allocate_shader_storage_bar(role, updated_specs, usage_hint),
        };
        let needs_migrate = {
            let locked = cur.lock().expect("bar lock");
            let new_size: usize = updated_specs
                .iter()
                .map(|s| s.num_elements * s.element_size)
                .sum();
            !locked.is_valid() || locked.byte_size() < new_size || locked.needs_realloc
        };
        if needs_migrate {
            self.allocate_shader_storage_bar(role, updated_specs, usage_hint)
        } else {
            cur.clone()
        }
    }

    // ------------------------------------------------------------------
    // GPU computation queue (port of C++ AddComputation)
    // ------------------------------------------------------------------

    /// Queue a GPU computation to run at commit time.
    ///
    /// Port of HdStResourceRegistry::AddComputation.
    /// Computations are executed in queue order (queue 0 first, then 1, etc.).
    /// Memory barriers separate queues.
    ///
    /// `execute_fn` is an optional closure called during the commit phase with
    /// a shared reference to this registry. Pass `None` for placeholder entries.
    pub fn add_computation(
        &self,
        target_bar: Option<ManagedBarSharedPtr>,
        name: impl Into<String>,
        queue: ComputeQueue,
        execute_fn: Option<Box<dyn FnOnce(&HdStResourceRegistry) + Send>>,
    ) {
        let computation = GpuComputation {
            name: name.into(),
            target_bar,
            queue,
            execute_fn,
        };
        self.pending_computations
            .lock()
            .expect("compute lock")
            .push(computation);
    }
    // ------------------------------------------------------------------
    // GPU computation execution (port of C++ _Commit compute phase)
    // ------------------------------------------------------------------

    /// Execute all pending GPU computations in queue order.
    ///
    /// Drains `pending_computations`, stable-sorts by queue ordinal, then
    /// calls each computation's `execute_fn` (if present) in order.
    /// A conceptual memory barrier is logged between queue transitions,
    /// mirroring C++ HdStResourceRegistry::_Commit compute phase.
    pub(crate) fn execute_computations(&self) {
        let mut computations = {
            let mut locked = self.pending_computations.lock().expect("compute lock");
            std::mem::take(&mut *locked)
        };

        if computations.is_empty() {
            return;
        }

        // Stable-sort so Queue0 always runs before Queue1, etc.
        computations.sort_by_key(|c| c.queue as u8);

        let mut current_queue = computations[0].queue;
        for comp in computations {
            if comp.queue != current_queue {
                log::debug!(
                    "[ResourceRegistry] compute barrier: {:?} -> {:?}",
                    current_queue,
                    comp.queue
                );
                current_queue = comp.queue;
            }
            log::debug!(
                "[ResourceRegistry] execute computation '{}' on {:?}",
                comp.name,
                comp.queue
            );
            if let Some(f) = comp.execute_fn {
                f(self);
            }
        }
    }

    // ------------------------------------------------------------------
    // Mesh topology deduplication registry
    // ------------------------------------------------------------------

    /// Register or retrieve a shared mesh topology by content hash.
    ///
    /// Port of HdStResourceRegistry topology sharing concept.
    /// Computes a 64-bit hash over the topology's face counts and indices,
    /// then returns an existing shared topology if the hash matches, or
    /// stores and returns a new one.
    ///
    /// This avoids uploading duplicate GPU buffers for meshes that share
    /// the same topology (e.g. many copies of the same asset).
    pub fn register_mesh_topology(&self, topology: HdStMeshTopology) -> Arc<HdStMeshTopology> {
        let hash = compute_topology_hash(&topology);
        let mut reg = self.topology_registry.lock().expect("topology lock");
        reg.entry(hash)
            .or_insert_with(|| Arc::new(topology))
            .clone()
    }

    /// Look up a registered topology by its content hash.
    ///
    /// Returns `None` if no topology with the given hash is registered.
    pub fn get_topology_by_hash(&self, hash: u64) -> Option<Arc<HdStMeshTopology>> {
        self.topology_registry
            .lock()
            .expect("topology lock")
            .get(&hash)
            .cloned()
    }

    /// Compute the 64-bit content hash for a mesh topology.
    ///
    /// Used as the key for the topology registry. Two topologies are
    /// considered identical if their face_vertex_counts, face_vertex_indices,
    /// hole_indices, and right_handed orientation all match.
    pub fn compute_mesh_topology_hash(topology: &HdStMeshTopology) -> u64 {
        compute_topology_hash(topology)
    }

    /// Number of unique topologies currently in the registry.
    pub fn topology_count(&self) -> usize {
        self.topology_registry.lock().expect("topology lock").len()
    }

    /// Garbage collect topology entries with no external references.
    ///
    /// Removes topologies held only by the registry itself.
    pub fn garbage_collect_topologies(&self) {
        let mut reg = self.topology_registry.lock().expect("topology lock");
        reg.retain(|_, topo| Arc::strong_count(topo) > 1);
    }

    // ------------------------------------------------------------------
    // Dispatch buffer registry (port of C++ RegisterDispatchBuffer)
    // ------------------------------------------------------------------

    /// Register an indirect dispatch buffer.
    ///
    /// Port of HdStResourceRegistry::RegisterDispatchBuffer.
    /// Allocates `count * command_num_uints * sizeof(u32)` bytes.
    pub fn register_dispatch_buffer(
        &self,
        role: &Token,
        count: usize,
        command_num_uints: usize,
    ) -> DispatchBufferSharedPtr {
        let byte_size = count * command_num_uints * std::mem::size_of::<u32>();
        let buffer = self.allocate_buffer_with_usage(
            HgiBufferUsage::STORAGE | HgiBufferUsage::UNIFORM,
            byte_size.max(4), // ensure non-zero
        );
        let dispatch = Arc::new(DispatchBuffer {
            role: role.clone(),
            count,
            command_num_uints,
            buffer,
        });
        self.dispatch_buffers
            .lock()
            .expect("dispatch lock")
            .push(dispatch.clone());
        dispatch
    }

    /// Garbage collect dispatch buffers with no external references.
    ///
    /// Port of HdStResourceRegistry::GarbageCollectDispatchBuffers.
    pub fn garbage_collect_dispatch_buffers(&self) {
        let mut bufs = self.dispatch_buffers.lock().expect("dispatch lock");
        bufs.retain(|b| Arc::strong_count(b) > 1);
    }

    // ------------------------------------------------------------------
    // Shader registry (port of C++ RegisterGLSLProgram / InvalidateShaderRegistry)
    // ------------------------------------------------------------------

    /// Register a shader program by hash ID for deduplication.
    ///
    /// Port of HdStResourceRegistry::RegisterGLSLProgram.
    /// Returns the existing program if one is already registered for this ID.
    pub fn register_shader(
        &self,
        id: u64,
        program: HgiShaderProgramHandle,
    ) -> Arc<HgiShaderProgramHandle> {
        let mut registry = self.shader_registry.lock().expect("shader_registry lock");
        registry
            .entry(id)
            .or_insert_with(|| Arc::new(program))
            .clone()
    }

    /// Look up a registered shader program by ID.
    pub fn get_shader(&self, id: u64) -> Option<Arc<HgiShaderProgramHandle>> {
        self.shader_registry
            .lock()
            .expect("shader_registry lock")
            .get(&id)
            .cloned()
    }

    /// Invalidate all shader programs, forcing recompile on next use.
    ///
    /// Port of HdStResourceRegistry::InvalidateShaderRegistry.
    /// Called when shader sources change (e.g., material edits).
    pub fn invalidate_shader_registry(&self) {
        let mut dirty = self.shader_registry_dirty.lock().expect("dirty lock");
        *dirty = true;
        // Clear cached programs so they get recompiled
        self.shader_registry
            .lock()
            .expect("shader_registry lock")
            .clear();
        log::debug!("[ResourceRegistry] shader registry invalidated");
    }

    /// Whether the shader registry has been invalidated since last commit.
    pub fn is_shader_registry_dirty(&self) -> bool {
        *self.shader_registry_dirty.lock().expect("dirty lock")
    }

    // ------------------------------------------------------------------
    // Pipeline registry (port of C++ RegisterGraphicsPipeline)
    // ------------------------------------------------------------------

    /// Register or retrieve a graphics pipeline by hash ID.
    ///
    /// Port of HdStResourceRegistry::RegisterGraphicsPipeline.
    pub fn register_graphics_pipeline(
        &self,
        id: u64,
        pipeline: HgiGraphicsPipelineHandle,
    ) -> Arc<HgiGraphicsPipelineHandle> {
        let mut registry = self
            .graphics_pipeline_registry
            .lock()
            .expect("pipeline lock");
        let entry = registry.entry(id).or_insert_with(|| PipelineRegistryEntry {
            id,
            pipeline: Arc::new(pipeline),
        });
        entry.pipeline.clone()
    }

    /// Look up a registered pipeline by ID.
    pub fn get_graphics_pipeline(&self, id: u64) -> Option<Arc<HgiGraphicsPipelineHandle>> {
        self.graphics_pipeline_registry
            .lock()
            .expect("pipeline lock")
            .get(&id)
            .map(|e| e.pipeline.clone())
    }

    /// Garbage collect pipelines with no external references.
    pub fn garbage_collect_pipelines(&self) {
        let mut registry = self
            .graphics_pipeline_registry
            .lock()
            .expect("pipeline lock");
        registry.retain(|_, e| Arc::strong_count(&e.pipeline) > 1);
    }

    // ------------------------------------------------------------------
    // Resource allocation stats extended (port of C++ GetResourceAllocation)
    // ------------------------------------------------------------------

    /// Get extended resource allocation statistics.
    pub fn get_resource_allocation_extended(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("buffer_count".into(), self.get_buffer_count());
        stats.insert("allocated_bytes".into(), self.get_allocated_memory());
        stats.insert("bar_count".into(), self.get_bar_count());
        stats.insert(
            "pending_staging_bytes".into(),
            self.get_pending_staging_size(),
        );
        stats.insert(
            "dispatch_buffers".into(),
            self.dispatch_buffers.lock().expect("dispatch lock").len(),
        );
        stats.insert(
            "shader_programs".into(),
            self.shader_registry.lock().expect("shader lock").len(),
        );
        stats.insert(
            "graphics_pipelines".into(),
            self.graphics_pipeline_registry
                .lock()
                .expect("pipeline lock")
                .len(),
        );
        stats
    }

    // ------------------------------------------------------------------
    // Blit commands (unchanged from original)
    // ------------------------------------------------------------------

    /// Get or create global blit commands, run closure, return result.
    ///
    /// Port of GetGlobalBlitCmds. Returns None if no Hgi.
    pub fn with_global_blit_cmds<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn HgiBlitCmds) -> R,
    {
        let mut blit = self.blit_cmds.lock().expect("blit_cmds lock");
        if blit.is_none() {
            if let Some(ref hgi) = self.hgi {
                let cmds = hgi.with_write(|h| h.create_blit_cmds());
                *blit = Some(cmds);
            }
        }
        blit.as_mut().map(|cmds| f(cmds.as_mut()))
    }

    /// Submit blit work to GPU. Resets global blit cmds after submit.
    ///
    /// Port of SubmitBlitWork.
    pub fn submit_blit_work(&self, wait: HgiSubmitWaitType) {
        let mut blit = self.blit_cmds.lock().expect("blit_cmds lock");
        if let (Some(ref hgi), Some(cmds)) = (self.hgi.as_ref(), blit.take()) {
            hgi.with_write(|h| h.submit_cmds(cmds, wait));
        }
    }

    /// Copy CPU data to GPU buffer via HGI blit. No-op if no Hgi.
    ///
    /// # Safety
    /// `cpu_data` must remain valid until `submit_blit_work` is called.
    #[allow(unsafe_code)]
    pub unsafe fn copy_buffer_cpu_to_gpu(
        &self,
        gpu_destination: &HgiBufferHandle,
        cpu_data: *const u8,
        byte_size: usize,
        destination_offset: usize,
    ) {
        if byte_size == 0 || cpu_data.is_null() {
            return;
        }
        self.with_global_blit_cmds(|blit| {
            let op = HgiBufferCpuToGpuOp {
                // cpu_data validated non-null above, lifetime guaranteed by caller
                cpu_source_buffer: RawCpuBuffer::new(cpu_data),
                source_byte_offset: 0,
                byte_size,
                gpu_destination_buffer: gpu_destination.clone(),
                destination_byte_offset: destination_offset,
            };
            blit.copy_buffer_cpu_to_gpu(&op);
        });
    }

    /// Upload CPU data to a GPU buffer resource (safe wrapper).
    ///
    /// Copies `data` into the buffer resource's backing GPU buffer at offset 0.
    /// Immediately submits the blit. Useful for dispatch / indirect command buffers
    /// that must be resident before the next draw.
    pub fn upload_to_buffer(&self, buffer: &HdStBufferResourceSharedPtr, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let handle = buffer.get_handle();
        if !handle.is_valid() {
            log::warn!("upload_to_buffer: invalid buffer handle");
            return;
        }
        // SAFETY: data slice is valid for the duration of the blit + submit.
        unsafe {
            self.copy_buffer_cpu_to_gpu(&handle, data.as_ptr(), data.len(), 0);
        }
        self.submit_blit_work(HgiSubmitWaitType::NoWait);
    }
    // ------------------------------------------------------------------
    // ExtComputation GPU dispatch support
    // ------------------------------------------------------------------

    /// Returns true if this registry has a real HGI backend (GPU available).
    ///
    /// Used by ExtCompGpuComputation::execute() to decide whether to
    /// perform actual GPU work or degrade gracefully in headless/test mode.
    pub fn has_hgi(&self) -> bool {
        self.hgi.is_some()
    }

    /// Register or retrieve a compute pipeline by hash.
    ///
    /// Port of C++ `HdStResourceRegistry::RegisterComputePipeline`.
    /// Keyed on `(program_handle, ubo_byte_size)` hash per the reference (§5.4).
    /// Returns `true` if this is the first registration (caller must create pipeline).
    pub fn register_compute_pipeline(&self, hash: u64) -> bool {
        let mut cache = self
            .compute_pipeline_cache
            .lock()
            .expect("compute_pipe lock");
        if cache.contains(&hash) {
            false // already cached
        } else {
            cache.insert(hash);
            log::debug!(
                "[ResourceRegistry] register_compute_pipeline hash={:#x} (first instance)",
                hash
            );
            true // first instance — caller should create the pipeline
        }
    }

    /// Register or retrieve resource bindings by hash.
    ///
    /// Port of C++ `HdStResourceRegistry::RegisterResourceBindings`.
    /// Keyed on XOR-combination of all bound buffer handle IDs (§5.3).
    /// Returns `true` if this is the first registration.
    pub fn register_resource_bindings(&self, hash: u64) -> bool {
        let mut cache = self.resource_bindings_cache.lock().expect("rb_cache lock");
        if cache.contains(&hash) {
            false // already cached
        } else {
            cache.insert(hash);
            log::debug!(
                "[ResourceRegistry] register_resource_bindings hash={:#x} (first instance)",
                hash
            );
            true // first instance
        }
    }

    /// Register ext computation data range for sharing/dedup.
    ///
    /// Port of C++ `HdStResourceRegistry::RegisterExtComputationDataRange`.
    /// Returns `true` if this is the first registration for this hash
    /// (caller should allocate the BAR).
    pub fn register_ext_computation_data_range(&self, hash: u64) -> bool {
        let mut cache = self
            .ext_computation_data_cache
            .lock()
            .expect("extcomp_data lock");
        if cache.contains(&hash) {
            false // already registered — reuse existing
        } else {
            cache.insert(hash);
            log::debug!(
                "[ResourceRegistry] register_ext_computation_data_range hash={:#x} (first)",
                hash
            );
            true // first instance — caller should allocate
        }
    }

    /// Register an ExtComputation sprim's input BAR after sync.
    ///
    /// Called from HdStExtComputation::sync_scene_inputs() to make the
    /// SSBO input range available for downstream GPU computations.
    /// Port of C++ `renderIndex.GetSprim() -> GetInputRange()` pattern.
    pub fn register_ext_comp_input_bar(&self, path: &SdfPath, bar: ManagedBarSharedPtr) {
        let mut map = self
            .ext_comp_input_bars
            .lock()
            .expect("ext_comp_input_bars lock");
        log::debug!("[ResourceRegistry] register_ext_comp_input_bar: {}", path);
        map.insert(path.clone(), bar);
    }

    /// Look up an ExtComputation sprim's input BAR by path.
    ///
    /// Returns the SSBO range containing the computation's scene inputs,
    /// or None if the sprim hasn't been synced yet.
    pub fn get_ext_comp_input_bar(&self, path: &SdfPath) -> Option<ManagedBarSharedPtr> {
        let map = self
            .ext_comp_input_bars
            .lock()
            .expect("ext_comp_input_bars lock");
        map.get(path).cloned()
    }

    /// Remove an ExtComputation sprim's input BAR (called during finalize).
    pub fn remove_ext_comp_input_bar(&self, path: &SdfPath) {
        let mut map = self
            .ext_comp_input_bars
            .lock()
            .expect("ext_comp_input_bars lock");
        map.remove(path);
    }

    /// Get or create the global compute command encoder for this frame.
    ///
    /// Port of C++ `HdStResourceRegistry::GetGlobalComputeCmds()`.
    /// Lazily creates a compute command buffer via HGI. All ExtComputation
    /// dispatches within a frame are recorded into this shared encoder,
    /// then submitted via `submit_compute_work()`.
    ///
    /// Returns `false` if HGI is unavailable (headless/test mode).
    fn ensure_compute_cmds(&self) -> bool {
        let mut slot = self.global_compute_cmds.lock().expect("compute_cmds lock");
        if slot.is_some() {
            return true;
        }
        let Some(hgi) = &self.hgi else {
            return false;
        };
        let cmds = hgi.with_write(|h| h.create_compute_cmds(&HgiComputeCmdsDesc::default()));
        *slot = Some(cmds);
        true
    }

    /// Execute a closure with the global compute command encoder.
    ///
    /// Port of C++ pattern: `GetGlobalComputeCmds()` → record → later `SubmitComputeWork()`.
    /// The closure receives `&mut dyn HgiComputeCmds` for recording bind/dispatch commands.
    /// Returns `false` if HGI is unavailable.
    pub fn with_compute_cmds<F>(&self, f: F) -> bool
    where
        F: FnOnce(&mut dyn HgiComputeCmds),
    {
        if !self.ensure_compute_cmds() {
            return false;
        }
        let mut slot = self.global_compute_cmds.lock().expect("compute_cmds lock");
        if let Some(cmds) = slot.as_mut() {
            f(cmds.as_mut());
            true
        } else {
            false
        }
    }

    /// Submit all accumulated compute work and reset the encoder.
    ///
    /// Port of C++ `HdStResourceRegistry::SubmitComputeWork()`. Called after
    /// `execute_computations()` to flush all recorded dispatches to the GPU.
    pub fn submit_compute_work(&self) {
        let cmds = {
            let mut slot = self.global_compute_cmds.lock().expect("compute_cmds lock");
            slot.take()
        };
        let Some(cmds) = cmds else {
            return; // nothing recorded
        };
        let Some(hgi) = &self.hgi else {
            return;
        };
        hgi.with_write(|h| {
            h.submit_cmds(cmds, HgiSubmitWaitType::NoWait);
        });
        log::debug!("[ResourceRegistry] submit_compute_work: flushed compute cmds");
    }

    /// Encode a 1D compute dispatch via the global compute command encoder.
    ///
    /// Mirrors C++ `HdStExtCompGpuComputation::Execute()` §5.6 command sequence:
    ///   BindPipeline → BindResources → SetConstantValues → Dispatch(count, 1)
    ///
    /// When HGI is present, records real GPU commands into the global compute encoder.
    /// In headless mode (no HGI), logs the dispatch without issuing GPU work.
    pub fn encode_compute_dispatch(
        &self,
        pipeline: &HgiComputePipelineHandle,
        resource_bindings: &HgiResourceBindingsHandle,
        uniforms: &[u8],
        dispatch_count: u32,
        debug_name: &str,
    ) {
        let pipeline_clone = pipeline.clone();
        let rb_clone = resource_bindings.clone();
        let uniform_data = uniforms.to_vec();
        let count = dispatch_count;

        let dispatched = self.with_compute_cmds(|cmds| {
            cmds.bind_pipeline(&pipeline_clone);
            cmds.bind_resources(&rb_clone);
            if !uniform_data.is_empty() {
                cmds.set_constant_values(&pipeline_clone, 0, &uniform_data);
            }
            cmds.dispatch(&HgiComputeDispatchOp::new_1d(count.max(1)));
        });

        if dispatched {
            log::debug!(
                "[ResourceRegistry] encode_compute_dispatch '{}' \
                 dispatch=({},1) uniforms={} bytes [HGI]",
                debug_name,
                dispatch_count,
                uniforms.len(),
            );
        } else {
            log::debug!(
                "[ResourceRegistry] encode_compute_dispatch [no HGI] '{}' \
                 dispatch=({},1) uniforms={} bytes",
                debug_name,
                dispatch_count,
                uniforms.len(),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Topology hash helper (module-level free function)
// ---------------------------------------------------------------------------

/// Compute a 64-bit content hash for `HdStMeshTopology`.
///
/// Hashes: face_vertex_counts, face_vertex_indices, hole_indices,
/// and orientation. Two topologies with the same hash
/// are treated as identical for GPU resource sharing.
fn compute_topology_hash(topo: &HdStMeshTopology) -> u64 {
    let mut h = DefaultHasher::new();
    topo.face_vertex_counts.hash(&mut h);
    topo.face_vertex_indices.hash(&mut h);
    topo.hole_indices.hash(&mut h);
    topo.right_handed.hash(&mut h);
    h.finish()
}
impl Default for HdStResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HdResourceRegistry for HdStResourceRegistry {
    // Implement base trait methods
}

/// Shared pointer to Storm resource registry.
pub type HdStResourceRegistrySharedPtr = Arc<HdStResourceRegistry>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResource};

    #[test]
    fn test_registry_creation() {
        let registry = HdStResourceRegistry::new();
        assert_eq!(registry.get_buffer_count(), 0);
        assert_eq!(registry.get_allocated_memory(), 0);
    }

    #[test]
    fn test_buffer_allocation() {
        let registry = HdStResourceRegistry::new();

        let buffer = registry.allocate_buffer(1024);
        assert!(buffer.is_valid());
        assert_eq!(buffer.get_size(), 1024);
        assert_eq!(registry.get_buffer_count(), 1);
        assert_eq!(registry.get_allocated_memory(), 1024);
    }

    #[test]
    fn test_buffer_deallocation() {
        let registry = HdStResourceRegistry::new();

        let buffer = registry.allocate_buffer(1024);
        let handle = buffer.get_handle();

        registry.free_buffer(handle);
        assert_eq!(registry.get_buffer_count(), 0);
        assert_eq!(registry.get_allocated_memory(), 0);
    }

    #[test]
    fn test_multiple_buffers() {
        let registry = HdStResourceRegistry::new();

        let _buf1 = registry.allocate_buffer(512);
        let _buf2 = registry.allocate_buffer(1024);
        let _buf3 = registry.allocate_buffer(256);

        assert_eq!(registry.get_buffer_count(), 3);
        assert_eq!(registry.get_allocated_memory(), 512 + 1024 + 256);
    }

    #[test]
    fn test_garbage_collection() {
        let registry = HdStResourceRegistry::new();

        {
            let _buf1 = registry.allocate_buffer(512);
            let _buf2 = registry.allocate_buffer(1024);
        } // Buffers dropped here

        assert_eq!(registry.get_buffer_count(), 2); // Still tracked

        registry.garbage_collect();

        assert_eq!(registry.get_buffer_count(), 0); // Cleaned up
        assert_eq!(registry.get_allocated_memory(), 0);
    }

    // --- BAR management tests ---

    #[test]
    fn test_allocate_non_uniform_bar() {
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 100,
            element_size: 12, // vec3f = 12 bytes
        }];

        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        let locked = bar.lock().unwrap();
        assert!(locked.is_valid());
        assert_eq!(locked.num_elements, 100);
        assert_eq!(locked.element_size, 12);
        assert_eq!(locked.byte_size(), 1200);
        assert_eq!(registry.get_bar_count(), 1);
    }

    #[test]
    fn test_bar_sub_allocation() {
        let registry = HdStResourceRegistry::new();

        // Allocate two BARs - they should share the same buffer
        let specs1 = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 100,
            element_size: 12,
        }];
        let specs2 = vec![BufferSpec {
            name: Token::new("normals"),
            num_elements: 100,
            element_size: 12,
        }];

        let bar1 = registry.allocate_non_uniform_bar(
            &Token::new("vertex"),
            &specs1,
            BufferArrayUsageHint::default(),
        );
        let bar2 = registry.allocate_non_uniform_bar(
            &Token::new("vertex"),
            &specs2,
            BufferArrayUsageHint::default(),
        );

        let locked1 = bar1.lock().unwrap();
        let locked2 = bar2.lock().unwrap();

        // Both should be valid
        assert!(locked1.is_valid());
        assert!(locked2.is_valid());

        // Second BAR should have offset after first
        assert_eq!(locked1.offset, 0);
        assert_eq!(locked2.offset, 1200); // 100 * 12

        assert_eq!(registry.get_bar_count(), 2);
    }

    #[test]
    fn test_add_sources_and_commit() {
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 4,
            element_size: 12,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        // Create source data (4 vertices * 12 bytes)
        let data = vec![0u8; 48];
        let source = Arc::new(BufferSource::new(Token::new("points"), data, 4, 12));

        // Queue source
        registry.add_source(&bar, source);
        assert!(registry.get_pending_staging_size() > 0);

        // Commit (no-op without real Hgi, but exercises the code path)
        registry.commit();
        assert_eq!(registry.get_pending_staging_size(), 0);
    }

    #[test]
    fn test_update_bar_allocates_if_none() {
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 50,
            element_size: 12,
        }];

        // Update with no current BAR = fresh allocation
        let bar = registry.update_non_uniform_bar(
            &Token::new("points"),
            None,
            &specs,
            &[],
            BufferArrayUsageHint::default(),
        );

        let locked = bar.lock().unwrap();
        assert!(locked.is_valid());
        assert_eq!(locked.num_elements, 50);
    }

    #[test]
    fn test_bar_garbage_collect() {
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 10,
            element_size: 12,
        }];

        {
            let _bar = registry.allocate_non_uniform_bar(
                &Token::new("points"),
                &specs,
                BufferArrayUsageHint::default(),
            );
        } // BAR dropped here

        assert_eq!(registry.get_bar_count(), 1); // Still tracked

        registry.garbage_collect();

        assert_eq!(registry.get_bar_count(), 0); // Cleaned up
    }

    #[test]
    fn test_buffer_source_creation() {
        let data = vec![1u8, 2, 3, 4];
        let src = BufferSource::new(Token::new("test"), data.clone(), 1, 4);

        assert!(src.is_valid());
        assert!(src.is_resolved());
        assert_eq!(src.byte_size(), 4);
        assert_eq!(src.num_elements, 1);
    }

    // --- Compute queue tests ---

    #[test]
    fn test_add_and_commit_computations_in_order() {
        let registry = HdStResourceRegistry::new();

        // Add computations in reverse queue order to verify sorting.
        // execute_fn = None for placeholder entries in this test.
        registry.add_computation(None, "skinning", ComputeQueue::Queue1, None);
        registry.add_computation(None, "normals", ComputeQueue::Queue0, None);
        registry.add_computation(None, "smooth", ComputeQueue::Queue2, None);

        // Commit should process computations sorted by queue (Queue0 first)
        // and clear the pending list. No panic = pass.
        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 1,
            element_size: 4,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );
        let src = Arc::new(BufferSource::new(Token::new("points"), vec![0u8; 4], 1, 4));
        registry.add_source(&bar, src);
        registry.commit(); // should drain computations too
    }

    #[test]
    fn test_execute_fn_called_in_queue_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let registry = HdStResourceRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        // Queue1 runs second — should capture value 1 (set by Queue0 fn)
        let c1 = counter.clone();
        registry.add_computation(
            None,
            "q1",
            ComputeQueue::Queue1,
            Some(Box::new(move |_reg| {
                // By the time this runs, Queue0 has already incremented to 1
                assert_eq!(c1.fetch_add(1, Ordering::SeqCst), 1);
            })),
        );

        // Queue0 runs first — should see counter == 0
        let c0 = counter.clone();
        registry.add_computation(
            None,
            "q0",
            ComputeQueue::Queue0,
            Some(Box::new(move |_reg| {
                assert_eq!(c0.fetch_add(1, Ordering::SeqCst), 0);
            })),
        );

        registry.execute_computations();
        assert_eq!(counter.load(Ordering::SeqCst), 2, "both fns must have run");
    }

    // --- Topology registry tests ---

    #[test]
    fn test_topology_registry_deduplication() {
        use crate::mesh::HdStMeshTopology;

        let registry = HdStResourceRegistry::new();
        assert_eq!(registry.topology_count(), 0);

        let mut topo = HdStMeshTopology::new();
        topo.face_vertex_counts = vec![4, 4];
        topo.face_vertex_indices = vec![0, 1, 2, 3, 4, 5, 6, 7];

        let shared1 = registry.register_mesh_topology(topo.clone());
        assert_eq!(registry.topology_count(), 1);

        // Same topology content -> should return the SAME Arc (deduplicated)
        let shared2 = registry.register_mesh_topology(topo.clone());
        assert_eq!(registry.topology_count(), 1); // still only 1 entry
        assert!(
            Arc::ptr_eq(&shared1, &shared2),
            "same topology must share Arc"
        );
    }

    #[test]
    fn test_topology_registry_different_topologies() {
        use crate::mesh::HdStMeshTopology;

        let registry = HdStResourceRegistry::new();

        let mut topo_a = HdStMeshTopology::new();
        topo_a.face_vertex_counts = vec![3, 3];
        topo_a.face_vertex_indices = vec![0, 1, 2, 0, 2, 3];

        let mut topo_b = HdStMeshTopology::new();
        topo_b.face_vertex_counts = vec![4];
        topo_b.face_vertex_indices = vec![0, 1, 2, 3];

        let _a = registry.register_mesh_topology(topo_a);
        let _b = registry.register_mesh_topology(topo_b);
        assert_eq!(registry.topology_count(), 2);
    }

    #[test]
    fn test_topology_gc() {
        use crate::mesh::HdStMeshTopology;

        let registry = HdStResourceRegistry::new();
        {
            let mut topo = HdStMeshTopology::new();
            topo.face_vertex_counts = vec![4];
            topo.face_vertex_indices = vec![0, 1, 2, 3];
            let _t = registry.register_mesh_topology(topo);
        } // _t dropped, external ref gone
        assert_eq!(registry.topology_count(), 1); // still in registry
        registry.garbage_collect_topologies();
        assert_eq!(registry.topology_count(), 0); // cleaned up
    }

    // ---------------------------------------------------------------
    // Commit Phase 2: BAR resize with multi-source packing
    // ---------------------------------------------------------------
    // These tests verify the fix for the bug where commit() Phase 2
    // resized a BAR using only the first source's num_elements,
    // shrinking packed buffers (positions+normals) to half size.

    #[test]
    fn test_commit_multi_source_bar_preserves_total_size() {
        // Simulate the real Storm pipeline: one BAR packs positions (44*12)
        // + normals (44*12) = 88 elements at 12 bytes each = 1056 bytes.
        // After commit, BAR must remain 88 elements, NOT shrink to 44.
        let registry = HdStResourceRegistry::new();

        let n_verts: usize = 44;
        let elem_size: usize = 12; // sizeof(vec3f)
        let total_elems = n_verts * 2; // positions + normals

        // Allocate BAR with both specs summed
        let specs = vec![
            BufferSpec {
                name: Token::new("points"),
                num_elements: n_verts,
                element_size: elem_size,
            },
            BufferSpec {
                name: Token::new("normals"),
                num_elements: n_verts,
                element_size: elem_size,
            },
        ];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        // Verify initial allocation is correct
        {
            let locked = bar.lock().unwrap();
            assert_eq!(
                locked.num_elements, total_elems,
                "initial BAR must cover both sources"
            );
            assert_eq!(locked.byte_size(), n_verts * 2 * elem_size);
        }

        // Queue TWO sources into the same BAR (as mesh.sync_vertices does)
        let pos_data = vec![0u8; n_verts * elem_size];
        let nrm_data = vec![0u8; n_verts * elem_size];
        let sources = vec![
            Arc::new(BufferSource::new(
                Token::new("points"),
                pos_data,
                n_verts,
                elem_size,
            )),
            Arc::new(BufferSource::new(
                Token::new("normals"),
                nrm_data,
                n_verts,
                elem_size,
            )),
        ];
        registry.add_sources(&bar, sources);

        // Commit — Phase 2 must sum both sources' bytes
        registry.commit();

        // BAR must still be 88 elements (not shrunk to 44)
        let locked = bar.lock().unwrap();
        assert_eq!(
            locked.num_elements, total_elems,
            "commit must not shrink BAR to first source only"
        );
        assert_eq!(
            locked.byte_size(),
            n_verts * 2 * elem_size,
            "BAR byte_size must equal positions + normals"
        );
    }

    #[test]
    fn test_commit_single_source_bar_unchanged() {
        // Single-source BAR (e.g. index buffer) must not be affected.
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("indices"),
            num_elements: 100,
            element_size: 4,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("indices"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        let data = vec![0u8; 400];
        let src = Arc::new(BufferSource::new(Token::new("indices"), data, 100, 4));
        registry.add_source(&bar, src);
        registry.commit();

        let locked = bar.lock().unwrap();
        assert_eq!(locked.num_elements, 100);
        assert_eq!(locked.byte_size(), 400);
    }

    #[test]
    fn test_commit_three_sources_sums_all() {
        // Edge case: 3 packed attributes (pos + normals + tangents)
        let registry = HdStResourceRegistry::new();
        let n: usize = 20;
        let es: usize = 12;

        let specs = vec![
            BufferSpec {
                name: Token::new("points"),
                num_elements: n,
                element_size: es,
            },
            BufferSpec {
                name: Token::new("normals"),
                num_elements: n,
                element_size: es,
            },
            BufferSpec {
                name: Token::new("tangents"),
                num_elements: n,
                element_size: es,
            },
        ];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        let sources: Vec<_> = ["points", "normals", "tangents"]
            .iter()
            .map(|name| {
                Arc::new(BufferSource::new(
                    Token::new(name),
                    vec![0u8; n * es],
                    n,
                    es,
                ))
            })
            .collect();
        registry.add_sources(&bar, sources);
        registry.commit();

        let locked = bar.lock().unwrap();
        assert_eq!(locked.num_elements, n * 3, "3-source BAR must be 3x");
        assert_eq!(locked.byte_size(), n * 3 * es);
    }

    #[test]
    fn test_commit_resize_grows_bar_when_sources_larger() {
        // BAR starts at 10 elems, but sources total 30 — must grow.
        let registry = HdStResourceRegistry::new();

        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 10,
            element_size: 12,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );
        assert_eq!(bar.lock().unwrap().num_elements, 10);

        // Queue 3 sources, each 10 elems = 30 total
        let sources: Vec<_> = (0..3)
            .map(|i| {
                Arc::new(BufferSource::new(
                    Token::new(&format!("attr{}", i)),
                    vec![0u8; 120],
                    10,
                    12,
                ))
            })
            .collect();
        registry.add_sources(&bar, sources);
        registry.commit();

        let locked = bar.lock().unwrap();
        assert_eq!(locked.num_elements, 30, "BAR must grow to fit all sources");
    }

    // ---------------------------------------------------------------
    // HdStBufferArrayRange: packed vertex buffer layout
    // ---------------------------------------------------------------

    #[test]
    fn test_buffer_array_range_positions_size() {
        // Packed buffer: 44 verts * 12 bytes = 528 positions, 1056 total
        let buf = Arc::new(HdStBufferResource::with_size(1056));
        let range = HdStBufferArrayRange::with_positions_size(buf, 0, 1056, 528);

        assert_eq!(range.get_positions_byte_size(), 528);
        assert_eq!(range.get_size(), 1056);

        // Normals start right after positions
        let normals_offset = range.get_positions_byte_size();
        assert_eq!(normals_offset, 528);
        // Remaining space for normals
        assert_eq!(range.get_size() - normals_offset, 528);
    }

    #[test]
    fn test_buffer_array_range_default_has_no_positions_size() {
        let buf = Arc::new(HdStBufferResource::with_size(512));
        let range = HdStBufferArrayRange::new(buf, 0, 512);
        assert_eq!(
            range.get_positions_byte_size(),
            0,
            "default range has no positions_byte_size"
        );
    }

    // ---------------------------------------------------------------
    // Multi-mesh concurrent: two meshes in same registry
    // ---------------------------------------------------------------

    #[test]
    fn test_two_bars_same_registry_no_cross_contamination() {
        // Two meshes allocate BARs in the same registry, queue sources,
        // commit. Each BAR must have correct size independently.
        let registry = HdStResourceRegistry::new();

        // Mesh A: 10 verts, pos + normals
        let specs_a = vec![
            BufferSpec {
                name: Token::new("points"),
                num_elements: 10,
                element_size: 12,
            },
            BufferSpec {
                name: Token::new("normals"),
                num_elements: 10,
                element_size: 12,
            },
        ];
        let bar_a = registry.allocate_non_uniform_bar(
            &Token::new("vertex_a"),
            &specs_a,
            BufferArrayUsageHint::default(),
        );

        // Mesh B: 50 verts, pos only
        let specs_b = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 50,
            element_size: 12,
        }];
        let bar_b = registry.allocate_non_uniform_bar(
            &Token::new("vertex_b"),
            &specs_b,
            BufferArrayUsageHint::default(),
        );

        // Queue sources for both
        let src_a = vec![
            Arc::new(BufferSource::new(
                Token::new("points"),
                vec![0u8; 120],
                10,
                12,
            )),
            Arc::new(BufferSource::new(
                Token::new("normals"),
                vec![0u8; 120],
                10,
                12,
            )),
        ];
        registry.add_sources(&bar_a, src_a);

        let src_b = vec![Arc::new(BufferSource::new(
            Token::new("points"),
            vec![0u8; 600],
            50,
            12,
        ))];
        registry.add_sources(&bar_b, src_b);

        registry.commit();

        // Verify each BAR independently
        let a = bar_a.lock().unwrap();
        assert_eq!(a.num_elements, 20, "mesh A = 10 pos + 10 nrm = 20");
        assert_eq!(a.byte_size(), 240);

        let b = bar_b.lock().unwrap();
        assert_eq!(b.num_elements, 50, "mesh B = 50 pos only");
        assert_eq!(b.byte_size(), 600);
    }

    #[test]
    fn test_commit_empty_sources_noop() {
        // BAR with no queued sources should not change after commit.
        let registry = HdStResourceRegistry::new();
        let specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: 10,
            element_size: 12,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("points"),
            &specs,
            BufferArrayUsageHint::default(),
        );
        let before = bar.lock().unwrap().num_elements;

        // Commit with nothing queued
        registry.commit();

        let after = bar.lock().unwrap().num_elements;
        assert_eq!(before, after, "empty commit must not change BAR");
    }

    #[test]
    fn test_managed_bar_byte_size_consistency() {
        // byte_size() must always equal num_elements * element_size
        let registry = HdStResourceRegistry::new();
        for (n, es) in [(1, 4), (44, 12), (100, 16), (0, 12), (10, 0)] {
            let specs = vec![BufferSpec {
                name: Token::new("test"),
                num_elements: n,
                element_size: es,
            }];
            let bar = registry.allocate_non_uniform_bar(
                &Token::new("test"),
                &specs,
                BufferArrayUsageHint::default(),
            );
            let locked = bar.lock().unwrap();
            assert_eq!(
                locked.byte_size(),
                locked.num_elements * locked.element_size,
                "byte_size invariant violated for n={} es={}",
                n,
                es
            );
        }
    }

    #[test]
    fn test_normals_offset_alignment() {
        // wgpu requires vertex buffer offsets aligned to 4 bytes.
        // Verify the alignment formula: (pos_size + 3) & !3
        for pos_size in [528usize, 529, 530, 531, 532, 1, 2, 3, 4, 0] {
            let aligned = (pos_size + 3) & !3;
            assert_eq!(
                aligned % 4,
                0,
                "offset {} must be 4-byte aligned (from pos_size={})",
                aligned,
                pos_size
            );
            assert!(aligned >= pos_size, "aligned offset must be >= original");
            assert!(aligned - pos_size < 4, "padding must be less than 4 bytes");
        }
    }

    #[test]
    fn test_topology_hash_lookup() {
        use crate::mesh::HdStMeshTopology;

        let registry = HdStResourceRegistry::new();
        let mut topo = HdStMeshTopology::new();
        topo.face_vertex_counts = vec![3];
        topo.face_vertex_indices = vec![0, 1, 2];

        let hash = HdStResourceRegistry::compute_mesh_topology_hash(&topo);
        let _registered = registry.register_mesh_topology(topo);
        assert!(registry.get_topology_by_hash(hash).is_some());
        assert!(registry.get_topology_by_hash(0xdeadbeef).is_none());
    }

    // ------------------------------------------------------------------
    // Computation round-trip tests (Level 1)
    // ------------------------------------------------------------------

    #[test]
    fn test_computation_roundtrip_counter() {
        // add_computation + execute_computations: closure must be called exactly once.
        use std::sync::atomic::{AtomicUsize, Ordering};

        let registry = HdStResourceRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        registry.add_computation(
            None,
            "counter",
            ComputeQueue::Queue0,
            Some(Box::new(move |_reg| {
                c.fetch_add(1, Ordering::SeqCst);
            })),
        );

        registry.execute_computations();
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "closure must run exactly once"
        );
    }

    #[test]
    fn test_computation_queue_ordering() {
        // Computations added in random queue order must execute in Queue0→1→2 order.
        use std::sync::Mutex as StdMutex;

        let registry = HdStResourceRegistry::new();
        let log: Arc<StdMutex<Vec<u8>>> = Arc::new(StdMutex::new(Vec::new()));

        // Add in reverse order to confirm sorting.
        let l2 = log.clone();
        registry.add_computation(
            None,
            "q2",
            ComputeQueue::Queue2,
            Some(Box::new(move |_| l2.lock().unwrap().push(2))),
        );
        let l0 = log.clone();
        registry.add_computation(
            None,
            "q0",
            ComputeQueue::Queue0,
            Some(Box::new(move |_| l0.lock().unwrap().push(0))),
        );
        let l1 = log.clone();
        registry.add_computation(
            None,
            "q1",
            ComputeQueue::Queue1,
            Some(Box::new(move |_| l1.lock().unwrap().push(1))),
        );

        registry.execute_computations();

        let result = log.lock().unwrap().clone();
        assert_eq!(
            result,
            vec![0u8, 1, 2],
            "must execute in queue-ascending order"
        );
    }

    #[test]
    fn test_computation_same_queue_all_run() {
        // Multiple computations in the same queue must all run, preserving insertion order.
        use std::sync::Mutex as StdMutex;

        let registry = HdStResourceRegistry::new();
        let log: Arc<StdMutex<Vec<u8>>> = Arc::new(StdMutex::new(Vec::new()));

        for i in 0u8..3 {
            let lc = log.clone();
            registry.add_computation(
                None,
                format!("comp{i}"),
                ComputeQueue::Queue0,
                Some(Box::new(move |_| lc.lock().unwrap().push(i))),
            );
        }

        registry.execute_computations();

        let result = log.lock().unwrap().clone();
        assert_eq!(result.len(), 3, "all 3 computations must run");
        // Stable sort preserves insertion order within the same queue.
        assert_eq!(result, vec![0u8, 1, 2]);
    }

    #[test]
    fn test_computation_empty_execute_no_panic() {
        // execute_computations on an empty queue must not panic.
        let registry = HdStResourceRegistry::new();
        registry.execute_computations(); // should return silently
    }

    #[test]
    fn test_computation_with_target_bar() {
        // Computation closure receives the registry and can inspect a target BAR.
        use std::sync::atomic::{AtomicU64, Ordering};

        let registry = HdStResourceRegistry::new();

        // Allocate a real BAR so we have a valid ManagedBarSharedPtr.
        let specs = vec![BufferSpec {
            name: Token::new("out"),
            num_elements: 16,
            element_size: 4,
        }];
        let bar = registry.allocate_non_uniform_bar(
            &Token::new("out"),
            &specs,
            BufferArrayUsageHint::default(),
        );

        let seen_elements = Arc::new(AtomicU64::new(0));
        let bar_ref = bar.clone();
        let seen = seen_elements.clone();

        registry.add_computation(
            Some(bar_ref),
            "bar_inspect",
            ComputeQueue::Queue0,
            Some(Box::new(move |_reg| {
                // Read the BAR element count that was set during allocation.
                let n = bar.lock().unwrap().num_elements as u64;
                seen.store(n, Ordering::SeqCst);
            })),
        );

        registry.execute_computations();

        assert_eq!(
            seen_elements.load(Ordering::SeqCst),
            16,
            "closure must see the BAR's num_elements"
        );
    }
}
