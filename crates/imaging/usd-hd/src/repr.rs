//! HdRepr - Single topological representation owning draw items.
//!
//! Corresponds to pxr/imaging/hd/repr.h (HdRepr class).
//! HdReprSelector composes multiple reprs; HdRepr holds the actual draw items.

use super::draw_item::HdDrawItem;
use std::cell::{Ref, RefCell};

/// Unique draw item pointer (owned).
pub type HdDrawItemUniquePtr = Box<HdDrawItem>;

/// Vector of draw items.
pub type HdDrawItemUniquePtrVector = Vec<HdDrawItemUniquePtr>;

/// A single topological representation of an rprim owning draw items.
///
/// Corresponds to C++ `HdRepr`.
/// Draw items are populated by the rprim. Main draw items first, then geom subset items.
#[derive(Debug, Default)]
pub struct HdRepr {
    /// Normal draw items first, then geom subset draw items.
    draw_items: RefCell<HdDrawItemUniquePtrVector>,
    /// Index where geom subset draw items begin.
    geom_subsets_start: RefCell<usize>,
}

impl HdRepr {
    /// Create new empty repr.
    pub fn new() -> Self {
        Self {
            draw_items: RefCell::new(Vec::new()),
            geom_subsets_start: RefCell::new(0),
        }
    }

    /// Get draw items (immutable).
    pub fn get_draw_items(&self) -> Ref<'_, HdDrawItemUniquePtrVector> {
        self.draw_items.borrow()
    }

    /// Add a draw item (main, not geom subset). Inserts before geom subset region.
    pub fn add_draw_item(&self, item: HdDrawItemUniquePtr) {
        let mut items = self.draw_items.borrow_mut();
        let start = *self.geom_subsets_start.borrow();
        items.insert(start, item);
        *self.geom_subsets_start.borrow_mut() = start + 1;
    }

    /// Get draw item at index.
    pub fn get_draw_item(&self, index: usize) -> Option<Ref<'_, HdDrawItem>> {
        let items = self.draw_items.borrow();
        if index >= items.len() {
            return None;
        }
        Some(Ref::map(items, |v| v[index].as_ref()))
    }

    /// Add geom subset draw item (appended at end).
    pub fn add_geom_subset_draw_item(&self, item: HdDrawItemUniquePtr) {
        self.draw_items.borrow_mut().push(item);
    }

    /// Get geom subset draw item by indices.
    pub fn get_draw_item_for_geom_subset(
        &self,
        repr_desc_index: usize,
        num_geom_subsets: usize,
        geom_subset_index: usize,
    ) -> Option<Ref<'_, HdDrawItem>> {
        let items = self.draw_items.borrow();
        let start = *self.geom_subsets_start.borrow();
        let idx = start + repr_desc_index * num_geom_subsets + geom_subset_index;
        if idx < items.len() {
            Some(Ref::map(items, |v| v[idx].as_ref()))
        } else {
            None
        }
    }

    /// Remove all geom subset draw items.
    pub fn clear_geom_subset_draw_items(&self) {
        let mut items = self.draw_items.borrow_mut();
        let start = *self.geom_subsets_start.borrow();
        items.truncate(start);
    }
}
