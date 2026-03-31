
//! Buffer sources for transient data upload to GPU.

use super::buffer_spec::HdBufferSpecVector;
use crate::types::HdTupleType;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Weak};
use usd_tf::Token;

/// Handle to a buffer source.
pub type HdBufferSourceHandle = Arc<dyn HdBufferSource>;

/// Weak handle to a buffer source.
pub type HdBufferSourceWeakHandle = Weak<dyn HdBufferSource>;

/// Resolution state of a buffer source.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdBufferSourceState {
    /// Source has not been resolved yet
    Unresolved = 0,

    /// Source is currently being resolved (locked)
    BeingResolved = 1,

    /// Source has been successfully resolved
    Resolved = 2,

    /// Resolution failed with an error
    ResolveError = 3,
}

impl From<u8> for HdBufferSourceState {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Unresolved,
            1 => Self::BeingResolved,
            2 => Self::Resolved,
            3 => Self::ResolveError,
            _ => Self::Unresolved,
        }
    }
}

/// Transient buffer of data pending GPU upload.
///
/// `HdBufferSource` represents data that has not yet been committed to GPU memory.
/// It provides an interface for preparing data (resolution) before transfer.
///
/// # Resolution
///
/// Resolution is the process of preparing data for GPU upload, which may include:
/// - CPU computations (e.g., smooth normals)
/// - Data format conversion
/// - Decompression or unpacking
///
/// Resolution uses atomic state management to support parallel processing
/// across multiple threads.
///
/// # Chaining
///
/// Buffer sources can be chained together:
/// - **Pre-chained**: Input dependencies that must be resolved first
/// - **Post-chained**: Additional outputs produced during resolution
pub trait HdBufferSource: Send + Sync {
    /// Get the name of this buffer source.
    fn get_name(&self) -> &Token;

    /// Add buffer specs to the provided vector.
    ///
    /// Buffer specs describe the format and must be determined before resolution.
    fn add_buffer_specs(&self, specs: &mut HdBufferSpecVector);

    /// Compute hash value for the underlying data.
    fn compute_hash(&self) -> u64 {
        // Default implementation based on name
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Use Token's hash() method which returns u64, then hash that value
        self.get_name().hash().hash(&mut hasher);
        hasher.finish()
    }

    /// Prepare data for access via `get_data()`.
    ///
    /// This may include computations or waiting for dependencies.
    /// Returns `true` if resolution succeeded or is complete.
    /// Returns `false` if dependencies are not ready or resolution is in progress.
    ///
    /// # Thread Safety
    ///
    /// This method may be called in parallel from multiple threads.
    /// Implementations should use atomic state management.
    fn resolve(&self) -> bool;

    /// Get raw pointer to the underlying data.
    ///
    /// Only valid after successful `resolve()`.
    fn get_data(&self) -> Option<*const u8>;

    /// Get the data type and array size.
    fn get_tuple_type(&self) -> HdTupleType;

    /// Get the number of elements in the source array.
    fn get_num_elements(&self) -> usize;

    /// Check if resolution is complete.
    fn is_resolved(&self) -> bool {
        matches!(
            self.get_state(),
            HdBufferSourceState::Resolved | HdBufferSourceState::ResolveError
        )
    }

    /// Check if resolution failed with an error.
    fn has_resolve_error(&self) -> bool {
        self.get_state() == HdBufferSourceState::ResolveError
    }

    /// Get current resolution state.
    fn get_state(&self) -> HdBufferSourceState;

    /// Set resolution state (for implementers).
    fn set_state(&self, state: HdBufferSourceState);

    /// Check if this source has a pre-chained dependency.
    fn has_pre_chained_buffer(&self) -> bool {
        false
    }

    /// Get the pre-chained buffer source.
    fn get_pre_chained_buffer(&self) -> Option<HdBufferSourceHandle> {
        None
    }

    /// Check if this source has post-chained outputs.
    fn has_chained_buffers(&self) -> bool {
        false
    }

    /// Get all post-chained buffer sources.
    fn get_chained_buffers(&self) -> Vec<HdBufferSourceHandle> {
        Vec::new()
    }

    /// Validate the buffer source.
    ///
    /// Returns `false` if the source would produce invalid specs
    /// or has invalid dependencies.
    fn is_valid(&self) -> bool {
        self.check_valid()
    }

    /// Internal validation implementation.
    ///
    /// Implementers should override this to perform actual validation.
    fn check_valid(&self) -> bool;

    /// Attempt to acquire resolution lock.
    ///
    /// Returns `true` if lock was acquired and caller should resolve.
    /// Returns `false` if already being resolved by another thread.
    fn try_lock(&self) -> bool {
        let state_ptr = self.get_state_atomic();
        let current = state_ptr.load(Ordering::Acquire);

        if current != HdBufferSourceState::Unresolved as u8 {
            return false;
        }

        state_ptr
            .compare_exchange(
                HdBufferSourceState::Unresolved as u8,
                HdBufferSourceState::BeingResolved as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Mark resolution as complete (for implementers).
    fn set_resolved(&self) {
        debug_assert_eq!(
            self.get_state(),
            HdBufferSourceState::BeingResolved,
            "Can only set resolved from being_resolved state"
        );
        self.set_state(HdBufferSourceState::Resolved);
    }

    /// Mark resolution as failed (for implementers).
    fn set_resolve_error(&self) {
        debug_assert_eq!(
            self.get_state(),
            HdBufferSourceState::BeingResolved,
            "Can only set error from being_resolved state"
        );
        self.set_state(HdBufferSourceState::ResolveError);
    }

    /// Get atomic state reference (for implementers).
    fn get_state_atomic(&self) -> &AtomicU8;
}

/// Base implementation for buffer sources with atomic state.
pub struct HdBufferSourceBase {
    state: AtomicU8,
}

impl HdBufferSourceBase {
    /// Create a new buffer source in unresolved state.
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(HdBufferSourceState::Unresolved as u8),
        }
    }

    /// Create a buffer source in a specific state.
    pub fn with_state(state: HdBufferSourceState) -> Self {
        Self {
            state: AtomicU8::new(state as u8),
        }
    }

    /// Get current state.
    pub fn get_state(&self) -> HdBufferSourceState {
        HdBufferSourceState::from(self.state.load(Ordering::Acquire))
    }

    /// Set state.
    pub fn set_state(&self, state: HdBufferSourceState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Get atomic state reference.
    pub fn state_atomic(&self) -> &AtomicU8 {
        &self.state
    }
}

impl Default for HdBufferSourceBase {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer source that is already resolved.
///
/// Use this for data that doesn't require preprocessing.
pub trait HdResolvedBufferSource: HdBufferSource {
    // Already resolved, no-op resolve
}

/// Buffer source that performs CPU computation.
///
/// Computation results are stored in an internal buffer source
/// that can be retrieved after resolution.
pub trait HdComputedBufferSource: HdBufferSource {
    /// Get the computed result buffer source.
    fn get_result(&self) -> Option<HdBufferSourceHandle>;

    /// Set the computed result (for implementers).
    fn set_result(&self, result: HdBufferSourceHandle);
}

/// Buffer source for pure CPU computation without GPU transfer.
///
/// The computation results are not uploaded to GPU memory.
pub trait HdNullBufferSource: HdBufferSource {
    // No GPU transfer, just CPU computation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_source_state() {
        assert_eq!(
            HdBufferSourceState::from(0),
            HdBufferSourceState::Unresolved
        );
        assert_eq!(
            HdBufferSourceState::from(1),
            HdBufferSourceState::BeingResolved
        );
        assert_eq!(HdBufferSourceState::from(2), HdBufferSourceState::Resolved);
        assert_eq!(
            HdBufferSourceState::from(3),
            HdBufferSourceState::ResolveError
        );
    }

    #[test]
    fn test_buffer_source_base() {
        let base = HdBufferSourceBase::new();
        assert_eq!(base.get_state(), HdBufferSourceState::Unresolved);

        base.set_state(HdBufferSourceState::BeingResolved);
        assert_eq!(base.get_state(), HdBufferSourceState::BeingResolved);

        base.set_state(HdBufferSourceState::Resolved);
        assert_eq!(base.get_state(), HdBufferSourceState::Resolved);
    }

    #[test]
    fn test_buffer_source_base_with_state() {
        let base = HdBufferSourceBase::with_state(HdBufferSourceState::Resolved);
        assert_eq!(base.get_state(), HdBufferSourceState::Resolved);
    }
}
