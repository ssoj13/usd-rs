
//! Bprim - Buffer primitive base trait.
//!
//! Bprims represent buffer objects in Hydra. This includes:
//! - Render buffers (HdRenderBuffer)
//! - Textures
//! - Buffer arrays
//!
//! # Responsibilities
//!
//! - Manage buffer identity (SdfPath)
//! - Track dirty bits for change propagation
//! - Sync with scene data via HdSceneDelegate
//! - Allocate and manage GPU buffer resources

use super::{HdRenderParam, HdSceneDelegate};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;

/// Base trait for buffer primitives.
///
/// All buffer objects (render buffers, textures) implement this trait.
/// Matches C++ HdBprim from pxr/imaging/hd/bprim.h.
pub trait HdBprim {
    // =========================================================================
    // Dirty bit constants
    // =========================================================================

    /// Clean state - no changes.
    const CLEAN: HdDirtyBits = 0;

    /// Buffer parameters changed (size, format, etc).
    const DIRTY_PARAMS: HdDirtyBits = 1 << 0;

    /// Buffer data changed.
    const DIRTY_DATA: HdDirtyBits = 1 << 1;

    /// All bits set.
    const ALL_DIRTY: HdDirtyBits = !0;

    // =========================================================================
    // Required methods
    // =========================================================================

    /// Get prim identifier.
    fn get_id(&self) -> &SdfPath;

    /// Get current dirty bits.
    fn get_dirty_bits(&self) -> HdDirtyBits;

    /// Set dirty bits.
    fn set_dirty_bits(&mut self, bits: HdDirtyBits);

    /// Sync prim data from scene delegate.
    ///
    /// Matches C++ `HdBprim::Sync(HdSceneDelegate*, HdRenderParam*, HdDirtyBits*)`.
    ///
    /// # Arguments
    ///
    /// * `delegate` - Scene delegate to query data from
    /// * `render_param` - Render delegate parameters (thread-safe)
    /// * `dirty_bits` - Which aspects need syncing (in/out)
    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    );

    // =========================================================================
    // Provided methods with default implementations
    // =========================================================================

    /// Get initial dirty bits mask for new prims.
    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        Self::ALL_DIRTY
    }

    /// Finalize before destruction. Matches C++ `Finalize(HdRenderParam*)`.
    fn finalize(&mut self, _render_param: Option<&dyn HdRenderParam>) {}

    /// Mark bits as clean.
    fn mark_clean(&mut self, bits: HdDirtyBits) {
        let current = self.get_dirty_bits();
        self.set_dirty_bits(current & !bits);
    }

    /// Mark bits as dirty.
    fn mark_dirty(&mut self, bits: HdDirtyBits) {
        let current = self.get_dirty_bits();
        self.set_dirty_bits(current | bits);
    }

    /// Check if any bits are dirty.
    fn is_dirty(&self) -> bool {
        self.get_dirty_bits() != Self::CLEAN
    }

    /// Check if specific bits are dirty.
    fn is_dirty_bits(&self, bits: HdDirtyBits) -> bool {
        (self.get_dirty_bits() & bits) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBprim {
        id: SdfPath,
        dirty_bits: HdDirtyBits,
    }

    impl HdBprim for MockBprim {
        fn get_id(&self) -> &SdfPath {
            &self.id
        }

        fn get_dirty_bits(&self) -> HdDirtyBits {
            self.dirty_bits
        }

        fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
            self.dirty_bits = bits;
        }

        fn sync(
            &mut self,
            _delegate: &dyn HdSceneDelegate,
            _render_param: Option<&dyn HdRenderParam>,
            dirty_bits: &mut HdDirtyBits,
        ) {
            *dirty_bits = Self::CLEAN;
            self.dirty_bits = Self::CLEAN;
        }
    }

    #[test]
    fn test_bprim_dirty_bits() {
        let mut bprim = MockBprim {
            id: SdfPath::from_string("/Buffer").unwrap(),
            dirty_bits: MockBprim::CLEAN,
        };

        assert!(!bprim.is_dirty());

        bprim.mark_dirty(MockBprim::DIRTY_PARAMS);
        assert!(bprim.is_dirty());
        assert!(bprim.is_dirty_bits(MockBprim::DIRTY_PARAMS));

        bprim.mark_clean(MockBprim::DIRTY_PARAMS);
        assert!(!bprim.is_dirty());
    }
}
