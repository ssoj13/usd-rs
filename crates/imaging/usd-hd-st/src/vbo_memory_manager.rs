
//! HdStVboMemoryManager - VBO (Vertex Buffer Object) memory allocation manager.
//!
//! Manages GPU buffer allocation with aggregation (sub-allocation) strategy:
//! allocates large backing buffers and sub-allocates ranges from them.
//! Reduces per-draw-call GPU buffer overhead by packing many allocations
//! into a shared buffer.
//!
//! Port of pxr/imaging/hdSt/vboMemoryManager.h
//!
//! ## BAR Reallocation Strategy
//!
//! Matches C++ `_StripedBufferArray::Reallocate` approach:
//! - **Free-list**: freed ranges are tracked per backing buffer and coalesced
//!   with adjacent free ranges to reduce fragmentation.
//! - **Growth**: when no backing buffer has space, a new one is created with
//!   doubled capacity (capped at `MAX_BACKING_SIZE`).
//! - **Defragmentation / compaction**: when utilization drops below a threshold
//!   (configurable, default 50%), `compact()` tightly packs all live ranges
//!   into a fresh buffer and retires the old one.  This mirrors C++'s
//!   `GarbageCollect()` -> `Reallocate()` flow.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use usd_hgi::{HgiBufferDesc, HgiBufferHandle, HgiBufferUsage, HgiDriverHandle};

/// Default backing buffer capacity (4 MB).
const DEFAULT_BACKING_SIZE: usize = 4 * 1024 * 1024;

/// Maximum size of a single backing buffer (128 MB, matches HD_MAX_VBO_SIZE).
const MAX_BACKING_SIZE: usize = 128 * 1024 * 1024;

/// Alignment for all sub-allocations (256 bytes, required for UBOs).
const ALLOC_ALIGNMENT: usize = 256;

/// Utilization threshold below which `compact()` triggers defragmentation.
/// E.g. 0.5 means compact when < 50% of backing capacity is live.
const COMPACT_UTILIZATION_THRESHOLD: f64 = 0.5;

/// A sub-allocation within a backing VBO buffer.
///
/// Tracks offset+size within a shared `BackingBuffer`.
#[derive(Debug, Clone)]
pub struct VboAllocation {
    /// Shared backing buffer handle.
    buffer: HgiBufferHandle,
    /// Byte offset into the backing buffer.
    offset: usize,
    /// Requested allocation size (not rounded to alignment).
    size: usize,
    /// Aligned allocation size (rounded up to ALLOC_ALIGNMENT).
    /// Used by compact() to tightly repack allocations.
    #[allow(dead_code)]
    aligned_size: usize,
    /// Unique ID for this allocation.
    id: u64,
    /// Generation counter for validity tracking.
    generation: u64,
    /// Usage key (HgiBufferUsage bits) for the backing bucket.
    /// Used by free() to return the range to the correct backing buffer.
    #[allow(dead_code)]
    usage_key: u8,
}

impl VboAllocation {
    fn new(
        buffer: HgiBufferHandle,
        offset: usize,
        size: usize,
        aligned_size: usize,
        id: u64,
        generation: u64,
        usage_key: u8,
    ) -> Self {
        Self {
            buffer,
            offset,
            size,
            aligned_size,
            id,
            generation,
            usage_key,
        }
    }

    /// GPU buffer handle.
    pub fn buffer(&self) -> &HgiBufferHandle {
        &self.buffer
    }
    /// Byte offset into the backing buffer.
    pub fn offset(&self) -> usize {
        self.offset
    }
    /// Allocation size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
    /// Generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

// ---------------------------------------------------------------------------
// FreeList: sorted free ranges within a single backing buffer
// ---------------------------------------------------------------------------

/// Free-list for a single backing buffer.
///
/// Tracks free byte ranges keyed by offset (BTreeMap for ordered iteration).
/// Adjacent/overlapping free ranges are coalesced on insert.
#[derive(Debug, Clone, Default)]
struct FreeList {
    /// offset -> size of free range
    ranges: BTreeMap<usize, usize>,
}

impl FreeList {
    /// Insert a free range and coalesce with adjacent neighbors.
    fn insert(&mut self, offset: usize, size: usize) {
        if size == 0 {
            return;
        }
        let mut merged_off = offset;
        let mut merged_size = size;

        // Coalesce with preceding range (ends exactly at our start)
        if let Some((&prev_off, &prev_size)) = self.ranges.range(..=offset).next_back() {
            if prev_off + prev_size == offset {
                merged_off = prev_off;
                merged_size += prev_size;
                self.ranges.remove(&prev_off);
            }
        }

        // Coalesce with following range (starts exactly at our end)
        let end = merged_off + merged_size;
        if let Some((&next_off, &next_size)) = self.ranges.range(end..).next() {
            if next_off == end {
                merged_size += next_size;
                self.ranges.remove(&next_off);
            }
        }

        self.ranges.insert(merged_off, merged_size);
    }

    /// Try to allocate `size` bytes from the free list (best-fit).
    /// Returns the offset if found.
    fn try_alloc(&mut self, size: usize) -> Option<usize> {
        // Best-fit: find smallest range that fits
        let mut best: Option<(usize, usize)> = None;
        for (&off, &sz) in &self.ranges {
            if sz >= size {
                match best {
                    None => best = Some((off, sz)),
                    Some((_, best_sz)) if sz < best_sz => best = Some((off, sz)),
                    _ => {}
                }
            }
        }

        if let Some((off, sz)) = best {
            self.ranges.remove(&off);
            let remainder = sz - size;
            if remainder > 0 {
                self.ranges.insert(off + size, remainder);
            }
            Some(off)
        } else {
            None
        }
    }

    /// Total free bytes.
    fn total_free(&self) -> usize {
        self.ranges.values().sum()
    }

    /// Whether the free list is empty.
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

// ---------------------------------------------------------------------------
// BackingBuffer: one large GPU buffer with bump-pointer + free-list allocator
// ---------------------------------------------------------------------------

struct BackingBuffer {
    handle: HgiBufferHandle,
    capacity: usize,
    /// High-water mark for bump-pointer allocation.
    fill: usize,
    /// Free-list of returned ranges within [0..fill).
    free_list: FreeList,
}

impl BackingBuffer {
    fn new(handle: HgiBufferHandle, capacity: usize) -> Self {
        Self {
            handle,
            capacity,
            fill: 0,
            free_list: FreeList::default(),
        }
    }

    /// Try to sub-allocate `size` bytes (aligned).
    /// First checks the free-list, then falls back to bump pointer.
    fn try_alloc(&mut self, size: usize) -> Option<usize> {
        let aligned = align_up(size, ALLOC_ALIGNMENT);

        // Try free-list first (reclaim freed ranges)
        if let Some(offset) = self.free_list.try_alloc(aligned) {
            return Some(offset);
        }

        // Fall back to bump pointer
        if self.fill + aligned <= self.capacity {
            let offset = self.fill;
            self.fill += aligned;
            Some(offset)
        } else {
            None
        }
    }

    /// Return a range to the free-list.
    fn free_range(&mut self, offset: usize, aligned_size: usize) {
        self.free_list.insert(offset, aligned_size);
    }

    /// Live bytes = fill minus free ranges.
    fn live_bytes(&self) -> usize {
        self.fill.saturating_sub(self.free_list.total_free())
    }

    /// Utilization ratio (0.0 - 1.0). Returns 1.0 for empty buffers.
    fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            return 1.0;
        }
        self.live_bytes() as f64 / self.capacity as f64
    }
}

// ---------------------------------------------------------------------------
// VboMemoryManager
// ---------------------------------------------------------------------------

/// Extended allocation tracking: stores per-alloc metadata for free-list.
struct AllocMeta {
    aligned_size: usize,
    offset: usize,
    /// Buffer handle ID to identify which backing buffer it belongs to.
    buffer_id: u64,
    usage_key: u8,
}

struct State {
    /// Backing buffers grouped by HgiBufferUsage bits (u8 key).
    buckets: HashMap<u8, Vec<BackingBuffer>>,
    next_id: u64,
    generation: u64,
    /// Live allocation tracking: alloc_id -> metadata.
    live_allocs: HashMap<u64, AllocMeta>,
    live_bytes: usize,
    /// Last backing size created per bucket (for doubling growth).
    last_backing_size: HashMap<u8, usize>,
}

impl State {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            next_id: 1,
            generation: 1,
            live_allocs: HashMap::new(),
            live_bytes: 0,
            last_backing_size: HashMap::new(),
        }
    }
}

/// VBO memory manager.
///
/// Implements the aggregation strategy from C++ HdStVBOMemoryManager:
/// - Maintains pools of large backing GPU buffers per usage type
/// - Sub-allocates ranges using a bump pointer with free-list fallback
/// - Tracks live allocations for stats / GC
/// - Freed ranges are returned to a per-buffer free-list and coalesced
/// - Growth strategy: doubles backing size up to MAX_BACKING_SIZE
/// - `compact()`: defragments when utilization drops below threshold
///
/// # Thread Safety
/// All operations are protected by an internal mutex.
pub struct VboMemoryManager {
    /// Optional HGI handle for real GPU buffer creation.
    hgi: Option<HgiDriverHandle>,
    state: Arc<Mutex<State>>,
}

impl VboMemoryManager {
    /// Create a headless manager (mock handles, no real GPU allocation).
    pub fn new() -> Self {
        Self {
            hgi: None,
            state: Arc::new(Mutex::new(State::new())),
        }
    }

    /// Create a manager backed by real HGI GPU allocation.
    pub fn new_with_hgi(hgi: HgiDriverHandle) -> Self {
        Self {
            hgi: Some(hgi),
            state: Arc::new(Mutex::new(State::new())),
        }
    }

    /// Allocate `size` bytes with the given `usage` hint.
    ///
    /// Allocation order:
    /// 1. Try free-list in existing backing buffers
    /// 2. Try bump-pointer in existing backing buffers
    /// 3. Create new backing buffer (doubled capacity, capped at MAX_BACKING_SIZE)
    pub fn allocate(&self, size: usize, usage: HgiBufferUsage) -> Option<VboAllocation> {
        if size == 0 {
            return None;
        }

        let aligned = align_up(size, ALLOC_ALIGNMENT);
        let key = usage.bits() as u8;

        // Phase 1: try existing backing buffers (free-list + bump).
        let found = {
            let mut st = self.state.lock().unwrap();
            let buckets = st.buckets.entry(key).or_default();
            let mut result: Option<(HgiBufferHandle, usize)> = None;
            for backing in buckets.iter_mut() {
                if let Some(offset) = backing.try_alloc(size) {
                    result = Some((backing.handle.clone(), offset));
                    break;
                }
            }
            result
        };

        if let Some((handle, offset)) = found {
            let mut st = self.state.lock().unwrap();
            let id = st.next_id;
            st.next_id += 1;
            let generation_val = st.generation;
            st.generation += 1;
            st.live_allocs.insert(
                id,
                AllocMeta {
                    aligned_size: aligned,
                    offset,
                    buffer_id: handle.id(),
                    usage_key: key,
                },
            );
            st.live_bytes += aligned;
            return Some(VboAllocation::new(
                handle,
                offset,
                size,
                aligned,
                id,
                generation_val,
                key,
            ));
        }

        // Phase 2: new backing buffer with doubling growth strategy.
        // Start at DEFAULT_BACKING_SIZE, double on subsequent allocs, cap at MAX_BACKING_SIZE.
        let backing_capacity = {
            let st = self.state.lock().unwrap();
            let last = st.last_backing_size.get(&key).copied().unwrap_or(0);
            let grown = if last == 0 {
                DEFAULT_BACKING_SIZE
            } else {
                (last * 2).min(MAX_BACKING_SIZE)
            };
            // Ensure we can fit the allocation
            if aligned > grown {
                if aligned > MAX_BACKING_SIZE {
                    log::warn!(
                        "VboMemoryManager: allocation {} bytes exceeds MAX_BACKING_SIZE {} -- \
                         creating oversized backing buffer",
                        aligned,
                        MAX_BACKING_SIZE
                    );
                }
                aligned
            } else {
                grown
            }
        };

        let handle = self.create_gpu_buffer(usage, backing_capacity);

        // Phase 3: insert and sub-allocate.
        let mut st = self.state.lock().unwrap();
        let mut new_backing = BackingBuffer::new(handle, backing_capacity);
        let offset = new_backing.try_alloc(size)?;

        let id = st.next_id;
        st.next_id += 1;
        let generation_val = st.generation;
        st.generation += 1;
        st.live_allocs.insert(
            id,
            AllocMeta {
                aligned_size: aligned,
                offset,
                buffer_id: new_backing.handle.id(),
                usage_key: key,
            },
        );
        st.live_bytes += aligned;
        st.last_backing_size.insert(key, backing_capacity);

        let handle_clone = new_backing.handle.clone();
        st.buckets.entry(key).or_default().push(new_backing);

        Some(VboAllocation::new(
            handle_clone,
            offset,
            size,
            aligned,
            id,
            generation_val,
            key,
        ))
    }

    /// Convenience: allocate vertex buffer (VERTEX | STORAGE usage).
    pub fn allocate_vertex(&self, size: usize) -> Option<VboAllocation> {
        self.allocate(size, HgiBufferUsage::VERTEX | HgiBufferUsage::STORAGE)
    }

    /// Convenience: allocate uniform buffer (UNIFORM | STORAGE usage).
    pub fn allocate_uniform(&self, size: usize) -> Option<VboAllocation> {
        self.allocate(size, HgiBufferUsage::UNIFORM | HgiBufferUsage::STORAGE)
    }

    /// Free an allocation. Returns the range to the backing buffer's free-list
    /// for reuse, and coalesces adjacent free ranges.
    ///
    /// Port of C++ _StripedBufferArrayRange destructor behavior.
    pub fn free(&self, allocation: &VboAllocation) {
        let mut st = self.state.lock().unwrap();
        if let Some(meta) = st.live_allocs.remove(&allocation.id) {
            st.live_bytes = st.live_bytes.saturating_sub(meta.aligned_size);

            // Return range to the free-list of the correct backing buffer
            if let Some(buckets) = st.buckets.get_mut(&meta.usage_key) {
                for backing in buckets.iter_mut() {
                    if backing.handle.id() == meta.buffer_id {
                        backing.free_range(meta.offset, meta.aligned_size);
                        break;
                    }
                }
            }
        }
    }

    /// Defragment: drop backing buffers that are completely empty
    /// (all allocations freed and no bump-pointer fill remaining).
    pub fn defragment(&self) {
        let mut st = self.state.lock().unwrap();
        for buckets in st.buckets.values_mut() {
            buckets.retain(|b| b.live_bytes() > 0);
        }
        st.buckets.retain(|_, v| !v.is_empty());
    }

    /// Compact buffers with low utilization.
    ///
    /// Port of C++ `_StripedBufferArray::GarbageCollect` + `Reallocate`:
    /// When a backing buffer's utilization drops below the threshold,
    /// all its live ranges are tightly packed into a new buffer of
    /// exactly the needed capacity (rounded up to alignment).
    /// The old buffer is discarded.
    ///
    /// Returns the number of backing buffers compacted.
    pub fn compact(&self) -> usize {
        self.compact_with_threshold(COMPACT_UTILIZATION_THRESHOLD)
    }

    /// Compact with a custom utilization threshold.
    ///
    /// Port of C++ `_StripedBufferArray::GarbageCollect` + `Reallocate`:
    /// 1. Compute tight capacity from live allocations
    /// 2. Create new GPU buffer via HGI (or mock for headless)
    /// 3. Issue GPU-to-GPU blit copies for each live range (old -> new offsets)
    /// 4. Update allocation metadata + replace backing buffer
    /// 5. Increment generation to trigger dispatch buffer rebuilds
    ///
    /// Returns the number of backing buffers compacted.
    pub fn compact_with_threshold(&self, threshold: f64) -> usize {
        let mut st = self.state.lock().unwrap();

        // Phase 1: identify backing buffers that need compaction.
        let mut candidates: Vec<(u8, usize, u64, usize)> = Vec::new();
        for (&key, buckets) in &st.buckets {
            for (idx, backing) in buckets.iter().enumerate() {
                let live = backing.live_bytes();
                if live > 0 && backing.utilization() < threshold {
                    candidates.push((key, idx, backing.handle.id(), backing.capacity));
                }
            }
        }

        let mut compacted = 0usize;

        // Phase 2: for each candidate, gather allocs + compute tight capacity.
        for (key, bucket_idx, buf_id, old_cap) in candidates {
            // Gather alloc IDs, old offsets, and sizes for this backing buffer.
            let alloc_info: Vec<(u64, usize, usize)> = st
                .live_allocs
                .iter()
                .filter(|(_, m)| m.buffer_id == buf_id && m.usage_key == key)
                .map(|(&id, m)| (id, m.offset, m.aligned_size))
                .collect();

            let tight_cap: usize = alloc_info.iter().map(|(_, _, sz)| *sz).sum();
            let tight_cap = align_up(tight_cap, ALLOC_ALIGNMENT);
            if tight_cap == 0 || tight_cap >= old_cap {
                continue;
            }

            // Get old buffer handle for GPU copy source.
            let old_handle = st
                .buckets
                .get(&key)
                .and_then(|b| b.get(bucket_idx))
                .map(|b| b.handle.clone());

            // Create new backing buffer (real via HGI, mock for headless).
            let usage = HgiBufferUsage::from_bits_truncate(key as u32);
            let new_handle = self.create_gpu_buffer(usage, tight_cap);
            let mut new_backing = BackingBuffer::new(new_handle, tight_cap);
            let new_buf_id = new_backing.handle.id();

            // Collect GPU copy operations: (old_offset, new_offset, size).
            let mut copy_ops: Vec<(usize, usize, usize)> = Vec::new();

            // Reassign each live alloc to new tightly-packed offsets.
            for (alloc_id, old_offset, aligned_size) in &alloc_info {
                if let Some(new_off) = new_backing.try_alloc(*aligned_size) {
                    copy_ops.push((*old_offset, new_off, *aligned_size));
                    if let Some(meta) = st.live_allocs.get_mut(alloc_id) {
                        meta.offset = new_off;
                        meta.buffer_id = new_buf_id;
                    }
                } else {
                    log::error!(
                        "compact: failed to repack alloc {} (size {})",
                        alloc_id,
                        aligned_size
                    );
                }
            }

            // Phase 3: issue GPU-to-GPU blit copies (old buffer -> new buffer).
            // Port of C++ HdStBufferRelocator used in Reallocate().
            if let (Some(hgi), Some(old_buf)) = (&self.hgi, old_handle) {
                let mut blit = hgi.with_write(|h| h.create_blit_cmds());
                for (old_off, new_off, size) in &copy_ops {
                    let op = usd_hgi::blit_cmds::HgiBufferGpuToGpuOp {
                        gpu_source_buffer: old_buf.clone(),
                        gpu_destination_buffer: new_backing.handle.clone(),
                        source_byte_offset: *old_off,
                        destination_byte_offset: *new_off,
                        byte_size: *size,
                    };
                    blit.copy_buffer_gpu_to_gpu(&op);
                }
                hgi.with_write(|h| {
                    h.submit_cmds(blit, usd_hgi::enums::HgiSubmitWaitType::NoWait);
                });
                log::debug!(
                    "compact: GPU-copied {} ranges from buf {} to buf {} ({} -> {} bytes)",
                    copy_ops.len(),
                    buf_id,
                    new_buf_id,
                    old_cap,
                    tight_cap,
                );
            } else if !copy_ops.is_empty() {
                log::debug!(
                    "compact: headless mode, {} ranges metadata-only (no GPU copy)",
                    copy_ops.len(),
                );
            }

            // Increment generation so callers know buffers changed.
            st.generation += 1;

            // Replace old backing buffer in the bucket.
            if let Some(buckets) = st.buckets.get_mut(&key) {
                if bucket_idx < buckets.len() {
                    buckets[bucket_idx] = new_backing;
                    compacted += 1;
                }
            }
        }
        compacted
    }

    /// Total live bytes across all active allocations.
    pub fn live_bytes(&self) -> usize {
        self.state.lock().unwrap().live_bytes
    }

    /// Number of active (not yet freed) allocations.
    pub fn allocation_count(&self) -> usize {
        self.state.lock().unwrap().live_allocs.len()
    }

    /// Total backing capacity allocated from GPU.
    pub fn backing_capacity(&self) -> usize {
        let st = self.state.lock().unwrap();
        st.buckets
            .values()
            .flat_map(|v| v.iter())
            .map(|b| b.capacity)
            .sum()
    }

    /// Overall utilization ratio (live_bytes / backing_capacity).
    pub fn utilization(&self) -> f64 {
        let st = self.state.lock().unwrap();
        let cap: usize = st
            .buckets
            .values()
            .flat_map(|v| v.iter())
            .map(|b| b.capacity)
            .sum();
        if cap == 0 {
            return 1.0;
        }
        st.live_bytes as f64 / cap as f64
    }

    /// Number of backing buffers across all usage buckets.
    pub fn backing_buffer_count(&self) -> usize {
        let st = self.state.lock().unwrap();
        st.buckets.values().map(|v| v.len()).sum()
    }

    /// Total free bytes available in free-lists (reclaimable without new backing).
    pub fn free_list_bytes(&self) -> usize {
        let st = self.state.lock().unwrap();
        st.buckets
            .values()
            .flat_map(|v| v.iter())
            .map(|b| b.free_list.total_free())
            .sum()
    }

    // Create a GPU buffer handle -- real via HGI or sequential mock ID.
    fn create_gpu_buffer(&self, usage: HgiBufferUsage, size: usize) -> HgiBufferHandle {
        if let Some(ref hgi) = self.hgi {
            let desc = HgiBufferDesc::new().with_usage(usage).with_byte_size(size);
            return hgi.with_write(|h| h.create_buffer(&desc, None));
        }
        static MOCK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        HgiBufferHandle::with_id(MOCK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for VboMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared pointer to a VBO memory manager.
pub type VboMemoryManagerSharedPtr = Arc<VboMemoryManager>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Align `v` up to the next multiple of `align` (must be a power of 2).
#[inline]
fn align_up(v: usize, align: usize) -> usize {
    (v + align - 1) & !(align - 1)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 256), 0);
        assert_eq!(align_up(1, 256), 256);
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(257, 256), 512);
        assert_eq!(align_up(1000, 256), 1024);
    }

    #[test]
    fn test_allocate_basic() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(1024).expect("alloc failed");
        assert_eq!(a.size(), 1024);
        assert_eq!(a.offset(), 0);
        assert_eq!(mgr.allocation_count(), 1);
        assert_eq!(mgr.live_bytes(), align_up(1024, 256));
    }

    #[test]
    fn test_sub_allocation_from_same_backing() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(1024).unwrap();
        let b = mgr.allocate_vertex(2048).unwrap();
        // Second alloc starts after first (aligned)
        assert_eq!(b.offset(), align_up(1024, 256));
        // Same backing buffer
        assert_eq!(a.buffer().id(), b.buffer().id());
        assert_eq!(mgr.allocation_count(), 2);
    }

    #[test]
    fn test_free_updates_live_bytes() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(512).unwrap();
        let live_before = mgr.live_bytes();
        mgr.free(&a);
        assert!(mgr.live_bytes() < live_before);
        assert_eq!(mgr.allocation_count(), 0);
    }

    #[test]
    fn test_different_usage_separate_buckets() {
        let mgr = VboMemoryManager::new();
        let v = mgr.allocate_vertex(512).unwrap();
        let u = mgr.allocate_uniform(512).unwrap();
        // Different usage bits -> different backing buffers
        assert_ne!(v.buffer().id(), u.buffer().id());
    }

    #[test]
    fn test_large_alloc_forces_new_backing() {
        let mgr = VboMemoryManager::new();
        let big = DEFAULT_BACKING_SIZE - 256;
        let a = mgr.allocate_vertex(big).unwrap();
        let b = mgr.allocate_vertex(512).unwrap();
        assert_ne!(a.buffer().id(), b.buffer().id());
    }

    #[test]
    fn test_backing_capacity_tracked() {
        let mgr = VboMemoryManager::new();
        assert_eq!(mgr.backing_capacity(), 0);
        let _a = mgr.allocate_vertex(1024).unwrap();
        assert!(mgr.backing_capacity() >= DEFAULT_BACKING_SIZE);
    }

    #[test]
    fn test_zero_size_alloc_returns_none() {
        let mgr = VboMemoryManager::new();
        assert!(mgr.allocate_vertex(0).is_none());
    }

    // ---------------------------------------------------------------
    // Free-list tests
    // ---------------------------------------------------------------

    #[test]
    fn test_free_list_basic() {
        let mut fl = FreeList::default();
        fl.insert(0, 256);
        assert_eq!(fl.total_free(), 256);

        let off = fl.try_alloc(256);
        assert_eq!(off, Some(0));
        assert_eq!(fl.total_free(), 0);
    }

    #[test]
    fn test_free_list_coalesce_adjacent() {
        let mut fl = FreeList::default();
        fl.insert(0, 256);
        fl.insert(256, 256);
        // Should coalesce into one 512-byte range
        assert_eq!(fl.ranges.len(), 1);
        assert_eq!(fl.total_free(), 512);
    }

    #[test]
    fn test_free_list_coalesce_both_sides() {
        let mut fl = FreeList::default();
        fl.insert(0, 256);
        fl.insert(512, 256);
        assert_eq!(fl.ranges.len(), 2);
        // Insert middle gap -> coalesce all three
        fl.insert(256, 256);
        assert_eq!(fl.ranges.len(), 1);
        assert_eq!(fl.total_free(), 768);
    }

    #[test]
    fn test_free_list_best_fit() {
        let mut fl = FreeList::default();
        fl.insert(0, 1024); // large
        fl.insert(2048, 256); // small
        // Allocating 256 should pick the smaller range
        let off = fl.try_alloc(256);
        assert_eq!(off, Some(2048));
        assert_eq!(fl.total_free(), 1024);
    }

    // ---------------------------------------------------------------
    // Free-list reuse in VboMemoryManager
    // ---------------------------------------------------------------

    #[test]
    fn test_free_and_reuse() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(512).unwrap();
        let a_offset = a.offset();
        let a_buf_id = a.buffer().id();

        // Free the allocation
        mgr.free(&a);

        // New allocation of same size should reuse the freed range
        let b = mgr.allocate_vertex(512).unwrap();
        assert_eq!(b.offset(), a_offset, "should reuse freed range offset");
        assert_eq!(
            b.buffer().id(),
            a_buf_id,
            "should reuse same backing buffer"
        );
    }

    #[test]
    fn test_free_list_bytes_tracked() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(512).unwrap();
        assert_eq!(mgr.free_list_bytes(), 0);
        mgr.free(&a);
        assert!(
            mgr.free_list_bytes() > 0,
            "free-list should have bytes after free"
        );
    }

    // ---------------------------------------------------------------
    // Doubling growth strategy
    // ---------------------------------------------------------------

    #[test]
    fn test_doubling_growth() {
        let mgr = VboMemoryManager::new();
        // First alloc creates DEFAULT_BACKING_SIZE
        let a = mgr.allocate_vertex(DEFAULT_BACKING_SIZE - 256).unwrap();
        assert!(mgr.backing_capacity() >= DEFAULT_BACKING_SIZE);

        // Second alloc of same size should create a doubled backing
        let b = mgr.allocate_vertex(DEFAULT_BACKING_SIZE + 256).unwrap();
        assert_ne!(a.buffer().id(), b.buffer().id());
        // Total capacity should be at least DEFAULT + 2*DEFAULT
        assert!(
            mgr.backing_capacity() >= DEFAULT_BACKING_SIZE * 3,
            "capacity {} should be >= {} (doubling)",
            mgr.backing_capacity(),
            DEFAULT_BACKING_SIZE * 3,
        );
    }

    // ---------------------------------------------------------------
    // Compaction
    // ---------------------------------------------------------------

    #[test]
    fn test_compact_reduces_capacity() {
        let mgr = VboMemoryManager::new();
        // Allocate several ranges then free most of them
        let a = mgr.allocate_vertex(256).unwrap();
        let b = mgr.allocate_vertex(256).unwrap();
        let c = mgr.allocate_vertex(256).unwrap();
        let _d = mgr.allocate_vertex(256).unwrap(); // keep one alive

        mgr.free(&a);
        mgr.free(&b);
        mgr.free(&c);

        let cap_before = mgr.backing_capacity();

        // Force compact with aggressive threshold (keep d is only 256 of 4MB)
        let compacted = mgr.compact_with_threshold(0.99);
        assert!(compacted > 0, "should compact at least one buffer");

        let cap_after = mgr.backing_capacity();
        assert!(
            cap_after < cap_before,
            "capacity should shrink: {} < {}",
            cap_after,
            cap_before
        );
    }

    #[test]
    fn test_utilization() {
        let mgr = VboMemoryManager::new();
        let _a = mgr.allocate_vertex(1024).unwrap();
        let util = mgr.utilization();
        assert!(
            util > 0.0 && util <= 1.0,
            "utilization should be in (0,1]: {}",
            util
        );
    }

    #[test]
    fn test_defragment_drops_empty_backings() {
        let mgr = VboMemoryManager::new();
        let a = mgr.allocate_vertex(256).unwrap();
        assert_eq!(mgr.backing_buffer_count(), 1);
        mgr.free(&a);
        mgr.defragment();
        assert_eq!(
            mgr.backing_buffer_count(),
            0,
            "empty backing should be dropped"
        );
    }
}
