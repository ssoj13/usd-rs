//! GPU computation interface for procedural data generation.

use super::buffer_spec::HdBufferSpecVector;
use std::sync::Arc;

/// Handle to a computation.
pub type HdComputationHandle = Arc<dyn HdComputation>;

/// GPU compute operation.
///
/// Represents a GPU-based computation that produces output buffers.
/// Used for procedural geometry generation, deformations, and
/// other GPU-accelerated data processing.
///
/// # Lifecycle
///
/// 1. Query output buffer specs with `get_output_specs()`
/// 2. Execute computation with `execute()`
/// 3. Results are written to output buffers
///
/// # Examples
///
/// - Tessellation and subdivision
/// - GPU-based deformers
/// - Procedural geometry generation
/// - Particle simulation
pub trait HdComputation: Send + Sync {
    /// Get buffer specifications for all outputs.
    ///
    /// Describes the format and layout of buffers produced by this computation.
    fn get_output_specs(&self) -> HdBufferSpecVector;

    /// Execute the GPU computation.
    ///
    /// Implementations should dispatch compute shaders or kernels
    /// to produce the output buffers.
    ///
    /// # Thread Safety
    ///
    /// This method may be called from multiple threads.
    /// Implementations should ensure proper synchronization.
    fn execute(&self);

    /// Get number of dispatch groups/invocations.
    ///
    /// For compute shaders, this represents the number of workgroups.
    fn get_dispatch_count(&self) -> usize {
        1
    }

    /// Check if this computation is valid and ready to execute.
    fn is_valid(&self) -> bool {
        true
    }

    /// Get unique identifier for this computation.
    ///
    /// Used for caching and deduplication.
    fn get_id(&self) -> u64 {
        0
    }
}

/// Base implementation for computations.
pub struct HdComputationBase {
    dispatch_count: usize,
}

impl HdComputationBase {
    /// Create a new computation base.
    pub fn new(dispatch_count: usize) -> Self {
        Self { dispatch_count }
    }

    /// Get dispatch count.
    pub fn dispatch_count(&self) -> usize {
        self.dispatch_count
    }

    /// Set dispatch count.
    pub fn set_dispatch_count(&mut self, count: usize) {
        self.dispatch_count = count;
    }
}

impl Default for HdComputationBase {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computation_base() {
        let mut base = HdComputationBase::new(10);
        assert_eq!(base.dispatch_count(), 10);

        base.set_dispatch_count(20);
        assert_eq!(base.dispatch_count(), 20);
    }

    #[test]
    fn test_computation_base_default() {
        let base = HdComputationBase::default();
        assert_eq!(base.dispatch_count(), 1);
    }
}
