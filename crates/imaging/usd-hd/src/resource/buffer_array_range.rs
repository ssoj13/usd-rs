//! Buffer array range for sub-allocation within buffer arrays.

use super::{
    buffer_array::{HdBufferArrayHandle, HdBufferArrayUsageHint},
    buffer_source::HdBufferSourceHandle,
    buffer_spec::HdBufferSpecVector,
};
use std::sync::{Arc, Mutex, Weak};
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Handle to a buffer array range.
pub type HdBufferArrayRangeHandle = Arc<dyn HdBufferArrayRange>;

/// Weak handle to a buffer array range.
pub type HdBufferArrayRangeWeakHandle = Weak<dyn HdBufferArrayRange>;

/// Range (subset) locator within a buffer array.
///
/// Represents a contiguous range of elements within a buffer array.
/// Each memory management strategy defines a specialized range implementation.
///
/// # Lifecycle
///
/// 1. Create range
/// 2. Check `is_valid()` and `is_assigned()`
/// 3. Resize if needed with `resize()`
/// 4. Copy data with `copy_data()`
/// 5. Access via offsets (`get_element_offset()`, `get_byte_offset()`)
///
/// # Aggregation
///
/// Ranges can be aggregated - multiple ranges sharing the same buffer array.
/// Use `is_aggregated_with()` to check if two ranges share storage.
pub trait HdBufferArrayRange: Send + Sync {
    /// Check if this range is valid.
    fn is_valid(&self) -> bool;

    /// Check if this range is assigned to a buffer.
    fn is_assigned(&self) -> bool;

    /// Check if this range is immutable.
    fn is_immutable(&self) -> bool;

    /// Check if this range requires CPU staging for GPU upload.
    fn requires_staging(&self) -> bool;

    /// Resize memory area for this range.
    ///
    /// Returns `true` if it causes container buffer reallocation.
    fn resize(&self, num_elements: usize) -> bool;

    /// Copy source data into buffer.
    fn copy_data(&self, buffer_source: HdBufferSourceHandle);

    /// Read back buffer content for a named resource.
    fn read_data(&self, name: &Token) -> Option<VtValue>;

    /// Get element offset within the underlying buffer array.
    fn get_element_offset(&self) -> usize;

    /// Get byte offset for a specific resource.
    fn get_byte_offset(&self, resource_name: &Token) -> usize;

    /// Get number of elements in this range.
    fn get_num_elements(&self) -> usize;

    /// Get version of the buffer array.
    fn get_version(&self) -> usize;

    /// Increment buffer array version.
    fn increment_version(&self);

    /// Get maximum number of elements capacity.
    fn get_max_num_elements(&self) -> usize;

    /// Get usage hint from underlying buffer array.
    fn get_usage_hint(&self) -> HdBufferArrayUsageHint;

    /// Set the buffer array associated with this range.
    fn set_buffer_array(&self, buffer_array: Option<HdBufferArrayHandle>);

    /// Get buffer specs for all resources in this range.
    fn get_buffer_specs(&self) -> HdBufferSpecVector;

    /// Check if aggregated with another range.
    ///
    /// Returns `true` if both ranges share the same underlying buffer array.
    fn is_aggregated_with(&self, other: &dyn HdBufferArrayRange) -> bool {
        match (self.get_aggregation_id(), other.get_aggregation_id()) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Get aggregation identifier (for internal use).
    ///
    /// Returns unique ID representing the underlying buffer array.
    fn get_aggregation_id(&self) -> Option<usize>;

    /// Debug output.
    fn debug_dump(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result;
}

/// Base implementation for buffer array ranges.
pub struct HdBufferArrayRangeBase {
    buffer_array: Mutex<Option<HdBufferArrayHandle>>,
    element_offset: Mutex<usize>,
    num_elements: Mutex<usize>,
}

impl HdBufferArrayRangeBase {
    /// Create a new buffer array range base.
    pub fn new() -> Self {
        Self {
            buffer_array: Mutex::new(None),
            element_offset: Mutex::new(0),
            num_elements: Mutex::new(0),
        }
    }

    /// Get element offset.
    pub fn element_offset(&self) -> usize {
        *self
            .element_offset
            .lock()
            .expect("element_offset mutex poisoned")
    }

    /// Set element offset.
    pub fn set_element_offset(&self, offset: usize) {
        *self
            .element_offset
            .lock()
            .expect("element_offset mutex poisoned") = offset;
    }

    /// Get number of elements.
    pub fn num_elements(&self) -> usize {
        *self
            .num_elements
            .lock()
            .expect("num_elements mutex poisoned")
    }

    /// Set number of elements.
    pub fn set_num_elements(&self, count: usize) {
        *self
            .num_elements
            .lock()
            .expect("num_elements mutex poisoned") = count;
    }

    /// Get buffer array handle.
    pub fn buffer_array(&self) -> Option<HdBufferArrayHandle> {
        self.buffer_array
            .lock()
            .expect("buffer_array mutex poisoned")
            .clone()
    }

    /// Set buffer array.
    pub fn set_buffer_array(&self, array: Option<HdBufferArrayHandle>) {
        *self
            .buffer_array
            .lock()
            .expect("buffer_array mutex poisoned") = array;
    }

    /// Check if assigned.
    pub fn is_assigned(&self) -> bool {
        self.buffer_array
            .lock()
            .expect("buffer_array mutex poisoned")
            .is_some()
    }

    /// Get version from buffer array.
    pub fn version(&self) -> usize {
        self.buffer_array()
            .map(|arr| arr.get_version())
            .unwrap_or(0)
    }

    /// Increment version.
    pub fn increment_version(&self) {
        if let Some(arr) = self.buffer_array() {
            arr.increment_version();
        }
    }

    /// Get usage hint.
    pub fn usage_hint(&self) -> HdBufferArrayUsageHint {
        self.buffer_array()
            .map(|arr| arr.get_usage_hint())
            .unwrap_or(0)
    }

    /// Check if immutable.
    pub fn is_immutable(&self) -> bool {
        self.buffer_array()
            .map(|arr| arr.is_immutable())
            .unwrap_or(false)
    }

    /// Get aggregation ID (pointer to buffer array).
    ///
    /// Uses the data pointer portion of the fat pointer for comparison.
    pub fn aggregation_id(&self) -> Option<usize> {
        self.buffer_array()
            .map(|arr| Arc::as_ptr(&arr) as *const () as usize)
    }
}

impl Default for HdBufferArrayRangeBase {
    fn default() -> Self {
        Self::new()
    }
}

/// Resizable container of buffer array ranges.
///
/// Provides indexed access to ranges with automatic resizing.
pub struct HdBufferArrayRangeContainer {
    ranges: Mutex<Vec<Option<HdBufferArrayRangeHandle>>>,
}

impl HdBufferArrayRangeContainer {
    /// Create a container with initial size.
    pub fn new(size: usize) -> Self {
        Self {
            ranges: Mutex::new(vec![None; size]),
        }
    }

    /// Set range at index, growing container if needed.
    pub fn set(&self, index: usize, range: HdBufferArrayRangeHandle) {
        let mut ranges = self.ranges.lock().expect("ranges mutex poisoned");

        if index >= ranges.len() {
            ranges.resize(index + 1, None);
        }

        ranges[index] = Some(range);
    }

    /// Get range at index.
    ///
    /// Returns `None` if index is out of range or not set.
    pub fn get(&self, index: usize) -> Option<HdBufferArrayRangeHandle> {
        let ranges = self.ranges.lock().expect("ranges mutex poisoned");
        ranges.get(index).and_then(|r| r.clone())
    }

    /// Resize the container.
    pub fn resize(&self, size: usize) {
        let mut ranges = self.ranges.lock().expect("ranges mutex poisoned");
        ranges.resize(size, None);
    }

    /// Get current size.
    pub fn len(&self) -> usize {
        self.ranges.lock().expect("ranges mutex poisoned").len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all ranges.
    pub fn clear(&self) {
        self.ranges.lock().expect("ranges mutex poisoned").clear();
    }
}

impl std::fmt::Display for dyn HdBufferArrayRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.debug_dump(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_array_range_base() {
        let base = HdBufferArrayRangeBase::new();

        assert_eq!(base.element_offset(), 0);
        assert_eq!(base.num_elements(), 0);
        assert!(!base.is_assigned());

        base.set_element_offset(10);
        base.set_num_elements(100);

        assert_eq!(base.element_offset(), 10);
        assert_eq!(base.num_elements(), 100);
    }

    #[test]
    fn test_range_container() {
        let container = HdBufferArrayRangeContainer::new(5);

        assert_eq!(container.len(), 5);
        assert!(!container.is_empty());

        assert!(container.get(0).is_none());
        assert!(container.get(10).is_none());

        container.resize(10);
        assert_eq!(container.len(), 10);

        container.clear();
        assert_eq!(container.len(), 0);
        assert!(container.is_empty());
    }
}
