
//! Resource registry for GPU resource management.

use super::{
    buffer_array::HdBufferArrayUsageHint, buffer_array_range::HdBufferArrayRangeHandle,
    buffer_source::HdBufferSourceHandle, buffer_spec::HdBufferSpecVector,
};
use std::sync::Arc;
use usd_tf::Token;

/// Handle to a resource registry.
pub type HdResourceRegistryHandle = Arc<dyn HdResourceRegistry>;

/// Central registry for GPU resource allocation and management.
///
/// The resource registry is responsible for:
/// - Allocating and deallocating GPU buffers
/// - Buffer aggregation and suballocation
/// - Texture and sampler management
/// - Resource deduplication and sharing
/// - Garbage collection of unused resources
///
/// # Architecture
///
/// The registry acts as a factory and manager for GPU resources,
/// abstracting backend-specific allocation details.
///
/// # Thread Safety
///
/// All operations are thread-safe and can be called from multiple threads.
pub trait HdResourceRegistry: Send + Sync {
    /// Allocate a buffer array range.
    ///
    /// # Parameters
    ///
    /// - `role`: Semantic role of the buffer (e.g., "vertex", "index")
    /// - `buffer_specs`: Format specifications for each buffer
    /// - `usage_hint`: Usage hints for memory management
    ///
    /// # Returns
    ///
    /// Handle to the allocated range, or `None` if allocation fails.
    fn allocate_buffer_array_range(
        &self,
        role: &Token,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> Option<HdBufferArrayRangeHandle>;

    /// Update buffer array range with new data.
    ///
    /// Schedules buffer sources for resolution and GPU upload.
    fn update_buffer_array_range(
        &self,
        range: HdBufferArrayRangeHandle,
        sources: Vec<HdBufferSourceHandle>,
    );

    /// Commit staged resources to GPU.
    ///
    /// Resolves pending buffer sources and uploads data.
    /// Call this before rendering to ensure resources are ready.
    fn commit(&self);

    /// Perform garbage collection.
    ///
    /// Removes unused resources and performs compaction.
    /// Call periodically to reclaim memory.
    fn garbage_collect(&self);

    /// Invalidate shader cache.
    ///
    /// Forces recompilation of shaders on next use.
    fn invalidate_shader_cache(&self) {}

    /// Get total GPU memory usage in bytes.
    fn get_gpu_memory_usage(&self) -> usize {
        0
    }

    /// Get number of buffer arrays.
    fn get_buffer_array_count(&self) -> usize {
        0
    }

    /// Get number of allocated ranges.
    fn get_range_count(&self) -> usize {
        0
    }

    /// Check if registry is empty (no allocations).
    fn is_empty(&self) -> bool {
        self.get_range_count() == 0
    }

    /// Debug dump of registry state.
    fn debug_dump(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(out, "HdResourceRegistry")?;
        writeln!(out, "  Buffer arrays: {}", self.get_buffer_array_count())?;
        writeln!(out, "  Ranges: {}", self.get_range_count())?;
        writeln!(out, "  GPU memory: {} bytes", self.get_gpu_memory_usage())?;
        Ok(())
    }
}

/// Base implementation for resource registries.
pub struct HdResourceRegistryBase {
    // Implementations will add their backend-specific state
}

impl HdResourceRegistryBase {
    /// Create a new resource registry base.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for HdResourceRegistryBase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_tf::Token as TfToken;

    // Mock registry for testing
    struct MockResourceRegistry {
        buffer_array_count: std::sync::atomic::AtomicUsize,
        range_count: std::sync::atomic::AtomicUsize,
    }

    impl MockResourceRegistry {
        fn new() -> Self {
            Self {
                buffer_array_count: std::sync::atomic::AtomicUsize::new(0),
                range_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl HdResourceRegistry for MockResourceRegistry {
        fn allocate_buffer_array_range(
            &self,
            _role: &TfToken,
            _buffer_specs: &HdBufferSpecVector,
            _usage_hint: HdBufferArrayUsageHint,
        ) -> Option<HdBufferArrayRangeHandle> {
            self.range_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            None // Mock implementation
        }

        fn update_buffer_array_range(
            &self,
            _range: HdBufferArrayRangeHandle,
            _sources: Vec<HdBufferSourceHandle>,
        ) {
        }

        fn commit(&self) {}

        fn garbage_collect(&self) {}

        fn get_buffer_array_count(&self) -> usize {
            self.buffer_array_count
                .load(std::sync::atomic::Ordering::SeqCst)
        }

        fn get_range_count(&self) -> usize {
            self.range_count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[test]
    fn test_resource_registry_base() {
        let _base = HdResourceRegistryBase::new();
        let _default = HdResourceRegistryBase::default();
    }

    #[test]
    fn test_mock_registry() {
        let registry = MockResourceRegistry::new();

        assert_eq!(registry.get_buffer_array_count(), 0);
        assert_eq!(registry.get_range_count(), 0);
        assert!(registry.is_empty());

        let _ = registry.allocate_buffer_array_range(&Token::new("vertex"), &vec![], 0);

        assert_eq!(registry.get_range_count(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_debug_dump() {
        let registry = MockResourceRegistry::new();
        let mut output = String::new();

        registry.debug_dump(&mut output).unwrap();
        assert!(output.contains("HdResourceRegistry"));
        assert!(output.contains("Buffer arrays:"));
        assert!(output.contains("Ranges:"));
    }
}
