#![allow(dead_code)]

//! HdStAggregationStrategy - Base trait for buffer aggregation strategies.
//!
//! Defines the factory interface for creating buffer arrays and buffer array
//! ranges. Concrete strategies (VBO, VBO simple, interleaved UBO/SSBO)
//! implement this trait to provide different memory layouts.
//!
//! Port of pxr/imaging/hdSt/strategyBase.h

use usd_hd::resource::{
    HdBufferArrayHandle, HdBufferArrayRangeHandle, HdBufferArrayUsageHint, HdBufferSpecVector,
};
use usd_tf::Token;
use std::collections::HashMap;

/// Aggregation ID used to group compatible buffer specs into shared arrays.
pub type AggregationId = u64;

/// Base trait for buffer aggregation strategies.
///
/// Each strategy knows how to:
/// - Create buffer arrays (GPU-side storage)
/// - Create buffer array ranges (views into buffer arrays)
/// - Compute aggregation IDs for grouping compatible specs
/// - Query buffer specs and resource allocation from existing arrays
///
/// Concrete implementations:
/// - `HdStVBOMemoryManager` - Striped VBO (aggregated, multiple ranges per buffer)
/// - `HdStVBOSimpleMemoryManager` - Simple VBO (1:1 buffer:range)
/// - `HdStInterleavedUBOMemoryManager` - Interleaved UBO
/// - `HdStInterleavedSSBOMemoryManager` - Interleaved SSBO
///
/// Port of HdStAggregationStrategy from pxr/imaging/hdSt/strategyBase.h
pub trait HdStAggregationStrategy: Send + Sync {
    /// Create a buffer array with the given role, specs, and usage hint.
    fn create_buffer_array(
        &self,
        role: &Token,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> HdBufferArrayHandle;

    /// Create an empty buffer array range (not yet assigned to a buffer array).
    fn create_buffer_array_range(&self) -> HdBufferArrayRangeHandle;

    /// Compute an aggregation ID for the given specs and usage hint.
    ///
    /// Buffer arrays with the same aggregation ID can share storage.
    /// The ID is typically a hash of buffer spec names, types, and usage.
    fn compute_aggregation_id(
        &self,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> AggregationId;

    /// Extract buffer specs from an existing buffer array.
    fn get_buffer_specs(&self, buffer_array: &HdBufferArrayHandle) -> HdBufferSpecVector;

    /// Get GPU memory used by the given buffer array.
    ///
    /// Populates `result` with per-resource details and returns total bytes.
    fn get_resource_allocation(
        &self,
        buffer_array: &HdBufferArrayHandle,
        result: &mut HashMap<String, usize>,
    ) -> usize;

    /// Flush any consolidated/staging buffers to GPU.
    ///
    /// Default implementation is a no-op.
    fn flush(&self) {}
}

/// Shared pointer to an aggregation strategy.
pub type HdStAggregationStrategySharedPtr = Box<dyn HdStAggregationStrategy>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_id_type() {
        // AggregationId is just u64
        let id: AggregationId = 42;
        assert_eq!(id, 42u64);
    }
}
