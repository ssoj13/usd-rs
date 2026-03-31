//! SdfChangeManager - pathway for change notification.
//!
//! Port of pxr/usd/sdf/changeManager.h
//!
//! The change manager collects changes to layers during a change block
//! and sends notifications when the outermost change block closes.

use crate::notice::{
    LayerDidReloadContent, LayerDidReplaceContent, LayerIdentifierDidChange, LayersDidChange,
};
use crate::{ChangeList, Layer, Path};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use usd_tf::Token;
use usd_tf::notice::global_registry;
use usd_vt::Value;

/// Layer change list pair.
pub type LayerChangeListPair = (Weak<Layer>, ChangeList);

/// Vector of layer-changelist pairs.
pub type LayerChangeListVec = Vec<LayerChangeListPair>;

/// Thread-local data for change manager.
#[derive(Default)]
struct ChangeData {
    /// Accumulated changes by layer.
    changes: HashMap<usize, (Weak<Layer>, ChangeList)>,
    /// Specs to remove if inert after block closes.
    remove_if_inert: Vec<(Weak<Layer>, Path)>,
}

// Thread-local storage for change data.
thread_local! {
    static CHANGE_DATA: RefCell<ChangeData> = RefCell::new(ChangeData::default());
}

/// Singleton that manages layer change notifications.
///
/// The change manager collects changes to layers during a change block
/// and sends notifications when the outermost change block closes.
/// This batches notifications to avoid excessive notification traffic.
///
/// Uses thread-local storage for change lists, matching C++ implementation.
pub struct ChangeManager {
    /// Serial number for change notifications (shared across threads).
    serial_number: Mutex<usize>,
}

impl ChangeManager {
    /// Gets the singleton instance.
    pub fn instance() -> &'static ChangeManager {
        static INSTANCE: OnceLock<ChangeManager> = OnceLock::new();
        INSTANCE.get_or_init(ChangeManager::new)
    }

    /// Creates a new change manager.
    fn new() -> Self {
        Self {
            serial_number: Mutex::new(1),
        }
    }

    /// Opens a change block.
    ///
    /// Returns true if this is the outermost block.
    pub(crate) fn open_change_block(&self) -> bool {
        // Block depth is tracked by ChangeBlock using thread-local storage
        // This method is called by ChangeBlock, depth is already incremented
        crate::ChangeBlock::depth() == 1
    }

    /// Closes a change block.
    ///
    /// If this is the outermost block, sends all accumulated notifications.
    pub(crate) fn close_change_block(&self) {
        // Check if this is the outermost block closing
        if crate::ChangeBlock::depth() == 0 {
            self.send_notices();
        }
    }

    /// Returns true if we're currently in a change block.
    pub fn is_in_change_block(&self) -> bool {
        crate::ChangeBlock::is_open()
    }

    /// Extracts and returns the current changes for a layer.
    ///
    /// The internal collection of changes for the layer is cleared.
    pub fn extract_local_changes(&self, layer: &Arc<Layer>) -> ChangeList {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            data.changes
                .remove(&ptr)
                .map(|(_, cl)| cl)
                .unwrap_or_default()
        })
    }

    /// Records that a layer's content was replaced (e.g., loaded from file).
    pub fn did_replace_layer_content(&self, layer: &Arc<Layer>) {
        if !self.is_in_change_block() {
            // Send immediately if not in a block
            global_registry().send(&LayerDidReplaceContent::new());
            return;
        }

        // Record for later in thread-local storage
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_replace_layer_content();
        });
    }

    /// Records that a layer's content was reloaded.
    pub fn did_reload_layer_content(&self, layer: &Arc<Layer>) {
        if !self.is_in_change_block() {
            global_registry().send(&LayerDidReloadContent::new());
            return;
        }

        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_reload_layer_content();
        });
    }

    /// Records that a layer's identifier changed.
    pub fn did_change_layer_identifier(&self, layer: &Arc<Layer>, old_identifier: &str) {
        let notice = LayerIdentifierDidChange::new(
            old_identifier.to_string(),
            layer.identifier().to_string(),
        );

        if !self.is_in_change_block() {
            global_registry().send(&notice);
            return;
        }

        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_change_layer_identifier(old_identifier);
        });
    }

    /// Records a field change.
    pub fn did_change_field(
        &self,
        layer: &Arc<Layer>,
        path: &Path,
        field: &Token,
        old_value: Option<Value>,
        new_value: Option<Value>,
    ) {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_change_info(
                path,
                field.clone(),
                old_value.unwrap_or_default(),
                new_value.unwrap_or_default(),
            );
        });
    }

    /// Records that time samples changed.
    pub fn did_change_attribute_time_samples(&self, layer: &Arc<Layer>, attr_path: &Path) {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_change_attribute_time_samples(attr_path);
        });
    }

    /// Records that a spec was moved.
    pub fn did_move_spec(&self, layer: &Arc<Layer>, old_path: &Path, new_path: &Path) {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_move_prim(old_path, new_path);
        });
    }

    /// Records that a spec was added.
    pub fn did_add_spec(&self, layer: &Arc<Layer>, path: &Path, inert: bool) {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_add_prim(path, inert);
        });
    }

    /// Records that a spec was removed.
    pub fn did_remove_spec(&self, layer: &Arc<Layer>, path: &Path, inert: bool) {
        let ptr = Arc::as_ptr(layer) as usize;
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let entry = data
                .changes
                .entry(ptr)
                .or_insert_with(|| (Arc::downgrade(layer), ChangeList::new()));
            entry.1.did_remove_prim(path, inert);
        });
    }

    /// Marks a spec to be removed if it's inert when the block closes.
    pub fn remove_spec_if_inert(&self, layer: &Arc<Layer>, path: &Path) {
        CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            data.remove_if_inert
                .push((Arc::downgrade(layer), path.clone()));
        });
    }

    /// Sends all accumulated change notices.
    fn send_notices(&self) {
        // Extract changes from thread-local storage
        let (changes, remove_if_inert) = CHANGE_DATA.with(|data| {
            let mut data = data.borrow_mut();
            (
                std::mem::take(&mut data.changes),
                std::mem::take(&mut data.remove_if_inert),
            )
        });

        // Process remove-if-inert specs
        // Match C++ _ProcessRemoveIfInert: calls layer->_RemoveIfInert(spec)
        for (layer_weak, path) in remove_if_inert {
            if let Some(layer) = layer_weak.upgrade() {
                if layer.has_spec(&path) {
                    // Get spec at path and call remove_if_inert
                    if let Some(spec) = layer.get_object_at_path(&path) {
                        layer.remove_if_inert(&spec);
                    }
                }
            }
        }

        if changes.is_empty() {
            return;
        }

        // Get next serial number
        let serial = {
            let mut sn = self.serial_number.lock().expect("lock poisoned");
            let s = *sn;
            *sn += 1;
            s
        };

        // Build change list vec for the notice
        let mut change_vec = Vec::new();
        for (_, (layer_weak, change_list)) in changes {
            if let Some(layer) = layer_weak.upgrade() {
                change_vec.push((layer, change_list));
            }
        }

        if !change_vec.is_empty() {
            // Send global LayersDidChange notice
            let notice = LayersDidChange::new(change_vec, serial);
            global_registry().send(&notice);
        }
    }

    /// Returns the current change block depth.
    pub fn block_depth(&self) -> u32 {
        crate::ChangeBlock::depth()
    }
}

impl std::fmt::Debug for ChangeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let block_depth = self.block_depth();
        let pending_layers = CHANGE_DATA.with(|data| data.borrow().changes.len());
        f.debug_struct("ChangeManager")
            .field("block_depth", &block_depth)
            .field("pending_layers", &pending_layers)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChangeBlock;

    #[test]
    fn test_singleton() {
        let m1 = ChangeManager::instance();
        let m2 = ChangeManager::instance();
        assert!(std::ptr::eq(m1, m2));
    }

    #[test]
    fn test_change_block_depth() {
        let manager = ChangeManager::instance();
        let initial_depth = manager.block_depth();

        {
            let _block1 = ChangeBlock::new();
            assert_eq!(manager.block_depth(), initial_depth + 1);

            {
                let _block2 = ChangeBlock::new();
                assert_eq!(manager.block_depth(), initial_depth + 2);
            }

            assert_eq!(manager.block_depth(), initial_depth + 1);
        }

        assert_eq!(manager.block_depth(), initial_depth);
    }

    #[test]
    fn test_not_in_change_block() {
        let manager = ChangeManager::instance();
        // Clear any existing blocks
        while manager.block_depth() > 0 {
            manager.close_change_block();
        }

        // When not in a block, is_in_change_block should be false
        assert!(!manager.is_in_change_block());
    }
}
