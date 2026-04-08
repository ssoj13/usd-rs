//! Buffer array aggregation for coherent GPU buffers.

use super::buffer_array_range::{HdBufferArrayRangeHandle, HdBufferArrayRangeWeakHandle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use usd_tf::Token;

/// Handle to a buffer array.
pub type HdBufferArrayHandle = Arc<dyn HdBufferArray>;

/// Weak handle to a buffer array.
pub type HdBufferArrayWeakHandle = Weak<dyn HdBufferArray>;

/// Usage hint bits for buffer arrays.
///
/// Provides hints to the memory manager about buffer properties
/// for efficient organization and aggregation.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdBufferArrayUsageHintBits {
    /// Buffer data is immutable after creation
    Immutable = 1 << 0,

    /// Number of elements changes over time
    SizeVarying = 1 << 1,

    /// Can be used as uniform buffer
    Uniform = 1 << 2,

    /// Can be used as shader storage buffer
    Storage = 1 << 3,

    /// Can be used as vertex buffer
    Vertex = 1 << 4,

    /// Can be used as index buffer
    Index = 1 << 5,
}

/// Combined usage hints as bitfield.
pub type HdBufferArrayUsageHint = u32;

/// Collection of coherent GPU buffers with shared layout.
///
/// Similar to a VAO, `HdBufferArray` bundles multiple buffers together.
/// Supports aggregation where multiple primitives share the same buffer array.
///
/// # Thread Safety
///
/// Buffer arrays support multi-threaded range assignment and modification.
/// Internal synchronization ensures thread-safe operations.
///
/// # Lifecycle
///
/// 1. Create buffer array with role and usage hints
/// 2. Assign ranges via `try_assign_range()`
/// 3. Call `reallocate()` when `needs_reallocation()` is true
/// 4. Perform garbage collection with `garbage_collect()`
pub trait HdBufferArray: Send + Sync {
    /// Get the role of GPU data in this buffer array.
    ///
    /// Role describes the semantic purpose (e.g., "vertex", "index").
    fn get_role(&self) -> &Token;

    /// Get the version of this buffer array.
    ///
    /// Used to detect when to rebuild dependent structures (e.g., indirect dispatch).
    fn get_version(&self) -> usize;

    /// Increment version to invalidate dependent data.
    fn increment_version(&self);

    /// Attempt to assign a range to this buffer array.
    ///
    /// Multiple threads may call this simultaneously.
    /// Returns `true` if the range was assigned.
    /// Returns `false` if there's insufficient space.
    fn try_assign_range(&self, range: HdBufferArrayRangeHandle) -> bool;

    /// Perform compaction and garbage collection.
    ///
    /// Returns `true` if the buffer array becomes empty after collection.
    fn garbage_collect(&self) -> bool;

    /// Reallocate buffer to hold specified ranges.
    ///
    /// If ranges are currently held by `cur_range_owner`, their data
    /// will be copied during reallocation.
    fn reallocate(
        &self,
        ranges: &[HdBufferArrayRangeHandle],
        cur_range_owner: Option<HdBufferArrayHandle>,
    );

    /// Get maximum element capacity.
    fn get_max_num_elements(&self) -> usize {
        0
    }

    /// Get number of attached ranges.
    fn get_range_count(&self) -> usize;

    /// Get range at specified index.
    fn get_range(&self, idx: usize) -> Option<HdBufferArrayRangeWeakHandle>;

    /// Remove deallocated ranges from the range list.
    ///
    /// Returns the number of ranges after cleanup.
    fn remove_unused_ranges(&self) -> usize;

    /// Check if reallocation is needed.
    fn needs_reallocation(&self) -> bool;

    /// Check if this buffer array is immutable.
    fn is_immutable(&self) -> bool {
        (self.get_usage_hint() & HdBufferArrayUsageHintBits::Immutable as u32) != 0
    }

    /// Get usage hints for this buffer array.
    fn get_usage_hint(&self) -> HdBufferArrayUsageHint;

    /// Debug output.
    fn debug_dump(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result;
}

/// Base implementation for buffer arrays.
///
/// Provides common functionality for version tracking and range management.
pub struct HdBufferArrayBase {
    role: Token,
    version: AtomicUsize,
    usage_hint: HdBufferArrayUsageHint,
    needs_reallocation: Mutex<bool>,
    ranges: Mutex<Vec<HdBufferArrayRangeWeakHandle>>,
    range_count: AtomicUsize,
    max_num_ranges: AtomicUsize,
}

impl HdBufferArrayBase {
    /// Create a new buffer array base.
    pub fn new(role: Token, usage_hint: HdBufferArrayUsageHint) -> Self {
        Self {
            role,
            version: AtomicUsize::new(0),
            usage_hint,
            needs_reallocation: Mutex::new(false),
            ranges: Mutex::new(Vec::new()),
            range_count: AtomicUsize::new(0),
            max_num_ranges: AtomicUsize::new(usize::MAX),
        }
    }

    /// Get the role.
    pub fn role(&self) -> &Token {
        &self.role
    }

    /// Get current version.
    pub fn version(&self) -> usize {
        self.version.load(Ordering::Acquire)
    }

    /// Increment version.
    pub fn increment_version(&self) {
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    /// Get usage hint.
    pub fn usage_hint(&self) -> HdBufferArrayUsageHint {
        self.usage_hint
    }

    /// Check if reallocation is needed.
    pub fn needs_reallocation(&self) -> bool {
        *self.needs_reallocation.lock().expect("lock poisoned")
    }

    /// Set reallocation flag.
    pub fn set_needs_reallocation(&self, value: bool) {
        *self.needs_reallocation.lock().expect("lock poisoned") = value;
    }

    /// Get range count.
    pub fn range_count(&self) -> usize {
        self.range_count.load(Ordering::Acquire)
    }

    /// Get range at index.
    pub fn get_range(&self, idx: usize) -> Option<HdBufferArrayRangeWeakHandle> {
        let ranges = self.ranges.lock().expect("lock poisoned");
        if idx < ranges.len() {
            Some(ranges[idx].clone())
        } else {
            None
        }
    }

    /// Try to assign a range.
    pub fn try_assign_range_impl(&self, range: HdBufferArrayRangeHandle) -> bool {
        let mut ranges = self.ranges.lock().expect("lock poisoned");
        let max_ranges = self.max_num_ranges.load(Ordering::Acquire);

        if ranges.len() >= max_ranges {
            return false;
        }

        ranges.push(Arc::downgrade(&range));
        self.range_count.fetch_add(1, Ordering::AcqRel);
        self.set_needs_reallocation(true);
        true
    }

    /// Remove unused ranges.
    pub fn remove_unused_ranges_impl(&self) -> usize {
        let mut ranges = self.ranges.lock().expect("lock poisoned");
        ranges.retain(|weak| weak.strong_count() > 0);
        let count = ranges.len();
        self.range_count.store(count, Ordering::Release);
        count
    }

    /// Set maximum number of ranges.
    pub fn set_max_num_ranges(&self, max: usize) {
        self.max_num_ranges.store(max, Ordering::Release);
    }

    /// Replace range list.
    pub fn set_range_list(&self, new_ranges: &[HdBufferArrayRangeHandle]) {
        let mut ranges = self.ranges.lock().expect("lock poisoned");
        ranges.clear();
        ranges.extend(new_ranges.iter().map(Arc::downgrade));
        self.range_count.store(new_ranges.len(), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_hint_bits() {
        let hint = HdBufferArrayUsageHintBits::Immutable as u32
            | HdBufferArrayUsageHintBits::Vertex as u32;

        assert_ne!(hint & HdBufferArrayUsageHintBits::Immutable as u32, 0);
        assert_ne!(hint & HdBufferArrayUsageHintBits::Vertex as u32, 0);
        assert_eq!(hint & HdBufferArrayUsageHintBits::Storage as u32, 0);
    }

    #[test]
    fn test_buffer_array_base_creation() {
        let base = HdBufferArrayBase::new(
            Token::new("vertex"),
            HdBufferArrayUsageHintBits::Vertex as u32,
        );

        assert_eq!(base.role().as_str(), "vertex");
        assert_eq!(base.version(), 0);
        assert_eq!(base.usage_hint(), HdBufferArrayUsageHintBits::Vertex as u32);
        assert!(!base.needs_reallocation());
        assert_eq!(base.range_count(), 0);
    }

    #[test]
    fn test_version_increment() {
        let base = HdBufferArrayBase::new(
            Token::new("vertex"),
            HdBufferArrayUsageHintBits::Vertex as u32,
        );

        assert_eq!(base.version(), 0);
        base.increment_version();
        assert_eq!(base.version(), 1);
        base.increment_version();
        assert_eq!(base.version(), 2);
    }

    #[test]
    fn test_needs_reallocation() {
        let base = HdBufferArrayBase::new(
            Token::new("vertex"),
            HdBufferArrayUsageHintBits::Vertex as u32,
        );

        assert!(!base.needs_reallocation());
        base.set_needs_reallocation(true);
        assert!(base.needs_reallocation());
        base.set_needs_reallocation(false);
        assert!(!base.needs_reallocation());
    }
}
