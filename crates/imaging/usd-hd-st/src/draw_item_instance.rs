
//! HdStDrawItemInstance - Per-instance state for a draw item.
//!
//! Stores visibility and batch association for a single draw item instance.
//! During culling, visibility is updated; if a batch is assigned, it receives
//! a DrawItemInstanceChanged callback. Ported from drawItemInstance.h.

use crate::draw_item::HdStDrawItemSharedPtr;

/// Per-instance state container for a draw item.
///
/// Tracks visibility for culling and maintains a reference to the owning batch.
/// When visibility changes and a batch is assigned, the batch is notified
/// so it can update its indirect draw command buffer.
#[derive(Debug)]
pub struct HdStDrawItemInstance {
    /// The draw item this instance references
    draw_item: HdStDrawItemSharedPtr,
    /// Index within the batch's draw item list
    batch_index: usize,
    /// Whether this instance is visible (survives culling)
    visible: bool,
}

impl HdStDrawItemInstance {
    /// Create a new draw item instance.
    pub fn new(draw_item: HdStDrawItemSharedPtr) -> Self {
        Self {
            draw_item,
            batch_index: 0,
            visible: true,
        }
    }

    /// Set visibility state.
    ///
    /// When a batch is assigned, it would receive a DrawItemInstanceChanged
    /// callback so it can update its GPU-side command buffer.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        // In full impl: notify batch via DrawItemInstanceChanged callback
    }

    /// Query visibility state.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set index into the batch's draw item list.
    ///
    /// Used by the batch during DrawItemInstanceChanged to locate
    /// this instance's entry in the indirect command buffer.
    pub fn set_batch_index(&mut self, index: usize) {
        self.batch_index = index;
    }

    /// Query batch index.
    pub fn get_batch_index(&self) -> usize {
        self.batch_index
    }

    /// Return a reference to the underlying draw item.
    pub fn get_draw_item(&self) -> &HdStDrawItemSharedPtr {
        &self.draw_item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_item::HdStDrawItem;
    use std::sync::Arc;

    #[test]
    fn test_draw_item_instance() {
        let item = Arc::new(HdStDrawItem::new(usd_sdf::Path::empty()));
        let mut inst = HdStDrawItemInstance::new(item);

        assert!(inst.is_visible());
        assert_eq!(inst.get_batch_index(), 0);

        inst.set_visible(false);
        assert!(!inst.is_visible());

        inst.set_batch_index(42);
        assert_eq!(inst.get_batch_index(), 42);
    }
}
