#![allow(dead_code)]

//! HdStRenderBufferPool - Pooled render buffer allocation.
//!
//! Re-uses render buffers between tasks in different render graphs
//! that regenerate data per-frame (e.g., shadow buffers).
//!
//! Port of pxr/imaging/hdSt/renderBufferPool.h

use super::render_buffer::HdFormat;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use usd_gf::Vec2i;
use usd_sdf::Path as SdfPath;

/// Descriptor for a pooled render buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PooledRenderBufferDesc {
    /// Pixel format
    pub fmt: HdFormat,
    /// Buffer dimensions (width, height)
    pub dims: Vec2i,
    /// Whether multisampled
    pub multi_sampled: bool,
    /// Whether this is a depth buffer
    pub depth: bool,
}

impl Hash for PooledRenderBufferDesc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash format as discriminant
        std::mem::discriminant(&self.fmt).hash(state);
        self.dims[0].hash(state);
        self.dims[1].hash(state);
        self.multi_sampled.hash(state);
        self.depth.hash(state);
    }
}

impl PooledRenderBufferDesc {
    /// Create a new pooled render buffer descriptor.
    pub fn new(fmt: HdFormat, dims: Vec2i, multi_sampled: bool, depth: bool) -> Self {
        Self {
            fmt,
            dims,
            multi_sampled,
            depth,
        }
    }
}

/// Allocation tracker for a single buffer configuration.
///
/// Tracks which indices are allocated and maintains a free list.
#[derive(Debug, Default)]
struct AllocationTracker {
    /// High-water mark for allocated indices
    max: u16,
    /// Free indices available for reuse
    free_list: Vec<u16>,
}

impl AllocationTracker {
    /// Allocate the next available index.
    fn allocate(&mut self) -> u16 {
        if let Some(idx) = self.free_list.pop() {
            idx
        } else {
            let idx = self.max;
            self.max += 1;
            idx
        }
    }

    /// Return an index to the free list.
    fn free(&mut self, index: u16) {
        self.free_list.push(index);
    }
}

/// Entry in the render buffer pool for a specific buffer configuration.
#[derive(Debug, Default)]
struct PoolEntry {
    /// Per-graph allocation trackers
    allocs: HashMap<SdfPath, AllocationTracker>,
}

/// Handle to a pooled render buffer.
///
/// Client has exclusive access during render graph execution.
/// No guarantees about contents before first or after last usage.
#[derive(Debug)]
pub struct PooledRenderBufferHandle {
    /// Buffer descriptor
    pub desc: PooledRenderBufferDesc,
    /// Owning graph path
    pub graph_path: SdfPath,
    /// Index within the pool entry
    pub idx: u16,
}

/// System for re-using HdStRenderBuffers between tasks.
///
/// Buffers are pooled by descriptor (format, dimensions, multisampled, depth)
/// and allocated per render graph. Freed buffers return to the pool for reuse.
///
/// Port of HdStRenderBufferPool
#[derive(Debug, Default)]
pub struct RenderBufferPool {
    /// Pooled entries keyed by buffer descriptor
    pool: HashMap<PooledRenderBufferDesc, PoolEntry>,
}

impl RenderBufferPool {
    /// Create a new render buffer pool.
    pub fn new() -> Self {
        Self {
            pool: HashMap::new(),
        }
    }

    /// Allocate a render buffer for the current render graph.
    pub fn allocate(
        &mut self,
        graph_path: SdfPath,
        fmt: HdFormat,
        dims: Vec2i,
        multi_sampled: bool,
        depth: bool,
    ) -> PooledRenderBufferHandle {
        let desc = PooledRenderBufferDesc::new(fmt, dims, multi_sampled, depth);

        let entry = self.pool.entry(desc.clone()).or_default();
        let tracker = entry.allocs.entry(graph_path.clone()).or_default();
        let idx = tracker.allocate();

        PooledRenderBufferHandle {
            desc,
            graph_path,
            idx,
        }
    }

    /// Free a previously allocated render buffer.
    pub fn free(&mut self, handle: &PooledRenderBufferHandle) {
        if let Some(entry) = self.pool.get_mut(&handle.desc) {
            if let Some(tracker) = entry.allocs.get_mut(&handle.graph_path) {
                tracker.free(handle.idx);
            }
        }
    }

    /// Commit: frees allocations no longer in use by any render graphs.
    pub fn commit(&mut self) {
        // Remove empty allocation trackers and pool entries
        self.pool.retain(|_desc, entry| {
            entry.allocs.retain(|_path, tracker| tracker.max > 0);
            !entry.allocs.is_empty()
        });
    }

    /// Number of distinct buffer configurations in the pool.
    pub fn num_configs(&self) -> usize {
        self.pool.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_allocate_free() {
        let mut pool = RenderBufferPool::new();
        let path = SdfPath::from_string("/shadows").unwrap();
        let dims = Vec2i::new(1024, 1024);

        let h1 = pool.allocate(path.clone(), HdFormat::Float32, dims, false, true);
        assert_eq!(h1.idx, 0);

        let h2 = pool.allocate(path.clone(), HdFormat::Float32, dims, false, true);
        assert_eq!(h2.idx, 1);

        // Free first, reallocate - should reuse index
        pool.free(&h1);
        let h3 = pool.allocate(path, HdFormat::Float32, dims, false, true);
        assert_eq!(h3.idx, 0);
    }

    #[test]
    fn test_pool_commit() {
        let mut pool = RenderBufferPool::new();
        let path = SdfPath::from_string("/test").unwrap();
        let dims = Vec2i::new(512, 512);

        let _h = pool.allocate(path, HdFormat::UNorm8Vec4, dims, false, false);
        assert_eq!(pool.num_configs(), 1);

        pool.commit();
        assert_eq!(pool.num_configs(), 1); // still has allocations
    }
}
