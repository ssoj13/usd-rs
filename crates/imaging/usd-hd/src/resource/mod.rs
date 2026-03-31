
//! GPU resource management for Hydra.
//!
//! This module provides abstractions for managing GPU resources including:
//! - Buffer specifications and sources
//! - Buffer arrays and ranges for aggregation
//! - Computation primitives for GPU processing
//! - Resource registry for allocation tracking

pub mod buffer_array;
pub mod buffer_array_range;
pub mod buffer_source;
pub mod buffer_spec;
pub mod computation;
pub mod ext_computation;
pub mod resource_registry;

// Re-export types
pub use buffer_array::{
    HdBufferArray, HdBufferArrayHandle, HdBufferArrayUsageHint, HdBufferArrayUsageHintBits,
    HdBufferArrayWeakHandle,
};
pub use buffer_array_range::{
    HdBufferArrayRange, HdBufferArrayRangeContainer, HdBufferArrayRangeHandle,
    HdBufferArrayRangeWeakHandle,
};
pub use buffer_source::{
    HdBufferSource, HdBufferSourceHandle, HdBufferSourceState, HdBufferSourceWeakHandle,
    HdComputedBufferSource, HdNullBufferSource, HdResolvedBufferSource,
};
pub use buffer_spec::{HdBufferSpec, HdBufferSpecVector};
pub use computation::{HdComputation, HdComputationHandle};
pub use ext_computation::{HdExtComputation, HdExtComputationDirtyBits};
pub use resource_registry::{HdResourceRegistry, HdResourceRegistryHandle};
