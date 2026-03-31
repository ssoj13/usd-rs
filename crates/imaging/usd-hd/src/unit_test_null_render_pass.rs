
//! HdUnitTestNullRenderPass - Null render pass for core Hydra unit tests.
//!
//! Implements the sync part of the render pass, but not the draw part.
//! Corresponds to pxr/imaging/hd/unitTestNullRenderPass.h

use super::render::task::TfTokenVector;
use super::render::{
    HdRenderPass, HdRenderPassBase, HdRenderPassStateSharedPtr, HdRprimCollection,
};

/// Null render pass for unit tests - sync only, no draw.
///
/// Used by core Hydra tests that need a render pass to exercise sync
/// pipeline without requiring GPU draw implementation.
pub struct HdUnitTestNullRenderPass {
    base: HdRenderPassBase,
}

impl HdUnitTestNullRenderPass {
    /// Create a new null render pass with the given collection.
    pub fn new(collection: HdRprimCollection) -> Self {
        Self {
            base: HdRenderPassBase::new(collection),
        }
    }

    /// Get mutable reference to base for tests.
    pub fn base_mut(&mut self) -> &mut HdRenderPassBase {
        &mut self.base
    }
}

impl HdRenderPass for HdUnitTestNullRenderPass {
    fn get_rprim_collection(&self) -> &HdRprimCollection {
        self.base.get_rprim_collection()
    }

    fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        self.base.set_rprim_collection(collection);
    }

    fn sync(&mut self) {
        if self.base.is_collection_dirty() {
            self.base.mark_collection_clean();
        }
    }

    fn execute(&mut self, _state: &HdRenderPassStateSharedPtr, _render_tags: &TfTokenVector) {
        // No-op - null render pass does not draw
    }

    fn is_converged(&self) -> bool {
        true
    }
}
