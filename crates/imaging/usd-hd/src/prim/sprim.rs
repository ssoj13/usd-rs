
//! Sprim - State primitive base trait.
//!
//! Sprims represent rendering state objects in Hydra. This includes:
//! - Cameras (HdCamera)
//! - Lights (HdLight)
//! - Materials (HdMaterial)
//! - Integrators
//! - Sample filters
//!
//! # Responsibilities
//!
//! - Manage prim identity (SdfPath)
//! - Track dirty bits for change propagation
//! - Sync with scene data via HdSceneDelegate
//! - Provide rendering state to render delegate

use super::{HdRenderParam, HdSceneDelegate};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;

/// Base trait for state primitives.
///
/// All state objects (camera, light, material) implement this trait.
/// Matches C++ HdSprim from pxr/imaging/hd/sprim.h.
pub trait HdSprim {
    // =========================================================================
    // Dirty bit constants
    // =========================================================================

    /// Clean state - no changes.
    const CLEAN: HdDirtyBits = 0;

    /// Parameters changed.
    const DIRTY_PARAMS: HdDirtyBits = 1 << 0;

    /// Transform changed (for cameras and lights).
    const DIRTY_TRANSFORM: HdDirtyBits = 1 << 1;

    /// Visibility changed (for lights).
    const DIRTY_VISIBILITY: HdDirtyBits = 1 << 2;

    /// Collection changed (for lights).
    const DIRTY_COLLECTION: HdDirtyBits = 1 << 3;

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
    /// Matches C++ `HdSprim::Sync(HdSceneDelegate*, HdRenderParam*, HdDirtyBits*)`.
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

    struct MockSprim {
        id: SdfPath,
        dirty_bits: HdDirtyBits,
    }

    impl HdSprim for MockSprim {
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
    fn test_sprim_dirty_bits() {
        let mut sprim = MockSprim {
            id: SdfPath::from_string("/Camera").unwrap(),
            dirty_bits: MockSprim::CLEAN,
        };

        assert!(!sprim.is_dirty());

        sprim.mark_dirty(MockSprim::DIRTY_PARAMS | MockSprim::DIRTY_TRANSFORM);
        assert!(sprim.is_dirty());
        assert!(sprim.is_dirty_bits(MockSprim::DIRTY_PARAMS));
        assert!(sprim.is_dirty_bits(MockSprim::DIRTY_TRANSFORM));

        sprim.mark_clean(MockSprim::DIRTY_PARAMS);
        assert!(sprim.is_dirty_bits(MockSprim::DIRTY_TRANSFORM));
        assert!(!sprim.is_dirty_bits(MockSprim::DIRTY_PARAMS));
    }
}
