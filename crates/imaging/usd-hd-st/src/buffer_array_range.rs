#![allow(dead_code)]

//! HdStBufferArrayRange - Range interface for buffer array subsets.
//!
//! Provides an abstract interface for representing a subset (range) within
//! an HdBufferArray. Each memory management strategy defines a specialized
//! range that aggregates named buffer resources.
//!
//! Also provides HdStBufferArrayRangeContainer for resizable collections.
//!
//! Port of pxr/imaging/hdSt/bufferArrayRange.h

use super::buffer_resource::{HdStBufferResourceNamedList, HdStBufferResourceSharedPtr};
use std::sync::Arc;
use usd_hd::resource::HdBufferSpecVector;
use usd_tf::Token;

/// Shared pointer to buffer array range.
pub type HdStBufferArrayRangeSharedPtr = Arc<dyn HdStBufferArrayRangeTrait>;

/// Interface for a range (subset) locator within an HdBufferArray.
///
/// Different memory managers (VBO, interleaved, SSBO) implement this
/// trait to provide their own aggregation strategy while keeping
/// the draw item interface uniform.
///
/// Port of HdStBufferArrayRange from pxr/imaging/hdSt/bufferArrayRange.h
pub trait HdStBufferArrayRangeTrait: std::fmt::Debug + Send + Sync {
    /// Check if this range is valid and has GPU resources.
    fn is_valid(&self) -> bool;

    /// Check if this range is immutable (read-only).
    fn is_immutable(&self) -> bool {
        false
    }

    /// Check if this range requires GPU reallocation (resized, etc.).
    fn requires_staging(&self) -> bool {
        false
    }

    /// Get number of elements in this range.
    fn num_elements(&self) -> usize;

    /// Get version number (incremented on data change).
    fn version(&self) -> usize {
        0
    }

    /// Get the offset of this range within the parent buffer array.
    fn offset(&self) -> usize;

    /// Get the index of this range (for multi-draw).
    fn index(&self) -> usize {
        0
    }

    /// Get element stride (interleaved layouts only).
    fn element_stride(&self) -> usize {
        0
    }

    /// Get the single GPU resource (error if multiple resources exist).
    fn get_resource(&self) -> Option<HdStBufferResourceSharedPtr>;

    /// Get a named GPU resource.
    fn get_resource_by_name(&self, name: &Token) -> Option<HdStBufferResourceSharedPtr>;

    /// Get all named GPU resources for this range.
    fn get_resources(&self) -> &HdStBufferResourceNamedList;

    /// Collect buffer specs from all resources in this range.
    fn get_buffer_specs(&self, specs: &mut HdBufferSpecVector) {
        let _ = specs;
        // Default: subclasses override
    }

    /// Downcast to Any for type-specific access.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Resizable container of buffer array ranges.
///
/// Used by draw items to store ranges indexed by buffer type
/// (e.g. constant, vertex, topology, etc.).
///
/// Port of HdStBufferArrayRangeContainer
#[derive(Debug)]
pub struct HdStBufferArrayRangeContainer {
    ranges: Vec<Option<HdStBufferArrayRangeSharedPtr>>,
}

impl HdStBufferArrayRangeContainer {
    /// Create container with given capacity.
    pub fn new(size: usize) -> Self {
        Self {
            ranges: (0..size).map(|_| None).collect(),
        }
    }

    /// Set range at index. Grows container if needed.
    pub fn set(&mut self, index: usize, range: HdStBufferArrayRangeSharedPtr) {
        if index >= self.ranges.len() {
            self.ranges.resize_with(index + 1, || None);
        }
        self.ranges[index] = Some(range);
    }

    /// Get range at index. Returns None if out of bounds or not set.
    pub fn get(&self, index: usize) -> Option<&HdStBufferArrayRangeSharedPtr> {
        self.ranges.get(index).and_then(|r| r.as_ref())
    }

    /// Get number of slots (may include empty ones).
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Check if container is empty.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Resize the container.
    pub fn resize(&mut self, size: usize) {
        self.ranges.resize_with(size, || None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_basic() {
        let container = HdStBufferArrayRangeContainer::new(4);
        assert_eq!(container.len(), 4);
        assert!(container.get(0).is_none());
        assert!(container.get(5).is_none());
    }

    #[test]
    fn test_container_resize() {
        let mut container = HdStBufferArrayRangeContainer::new(2);
        assert_eq!(container.len(), 2);

        container.resize(8);
        assert_eq!(container.len(), 8);
        assert!(container.get(5).is_none());
    }
}
