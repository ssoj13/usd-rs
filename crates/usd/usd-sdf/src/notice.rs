//! SDF notices - Notification system for layer changes.
//!
//! Port of pxr/usd/sdf/notice.h
//!
//! Provides notification types for various layer events like content changes,
//! identifier changes, save operations, and muting.

use crate::{ChangeList, Layer};
use std::sync::Arc;
use usd_tf::Token;
use usd_tf::notice::Notice;

/// A pair of layer and its change list.
pub type LayerChangeListPair = (Arc<Layer>, ChangeList);

/// Vector of layer-changelist pairs.
pub type LayerChangeListVec = Vec<LayerChangeListPair>;

/// Base notification class for SDF.
///
/// Only useful for type hierarchy purposes.
pub trait SdfNotice: Notice {}

/// Base class for LayersDidChange notices.
///
/// Contains the change list vector and serial number.
#[derive(Clone)]
pub struct BaseLayersDidChange {
    /// The change list vector.
    changes: LayerChangeListVec,
    /// Serial number for this round of change processing.
    serial_number: usize,
}

impl std::fmt::Debug for BaseLayersDidChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseLayersDidChange")
            .field("num_changes", &self.changes.len())
            .field("serial_number", &self.serial_number)
            .finish()
    }
}

impl BaseLayersDidChange {
    /// Creates a new BaseLayersDidChange.
    pub fn new(changes: LayerChangeListVec, serial_number: usize) -> Self {
        Self {
            changes,
            serial_number,
        }
    }

    /// Returns a list of layers that changed.
    pub fn get_layers(&self) -> Vec<Arc<Layer>> {
        self.changes
            .iter()
            .map(|(layer, _)| layer.clone())
            .collect()
    }

    /// Returns the change list vector.
    pub fn get_change_list_vec(&self) -> &LayerChangeListVec {
        &self.changes
    }

    /// Returns the serial number for this round of change processing.
    pub fn serial_number(&self) -> usize {
        self.serial_number
    }

    /// Returns an iterator over the changes.
    pub fn iter(&self) -> impl Iterator<Item = &LayerChangeListPair> {
        self.changes.iter()
    }

    /// Finds the change list for a specific layer.
    pub fn find(&self, layer: &Arc<Layer>) -> Option<&ChangeList> {
        self.changes
            .iter()
            .find(|(l, _)| Arc::ptr_eq(l, layer))
            .map(|(_, cl)| cl)
    }

    /// Returns true if the layer is in the change list.
    pub fn contains(&self, layer: &Arc<Layer>) -> bool {
        self.find(layer).is_some()
    }
}

/// Notice sent per-layer indicating all layers whose contents have changed.
///
/// If more than one layer changes in a single round of change processing,
/// this notice is sent once per layer with the same changeVec and serialNumber.
/// This allows clients to listen to notices from only specific layers.
#[derive(Clone, Debug)]
pub struct LayersDidChangeSentPerLayer {
    /// Base change data.
    base: BaseLayersDidChange,
}

impl LayersDidChangeSentPerLayer {
    /// Creates a new LayersDidChangeSentPerLayer notice.
    pub fn new(changes: LayerChangeListVec, serial_number: usize) -> Self {
        Self {
            base: BaseLayersDidChange::new(changes, serial_number),
        }
    }

    /// Returns the base change data.
    pub fn base(&self) -> &BaseLayersDidChange {
        &self.base
    }
}

impl Notice for LayersDidChangeSentPerLayer {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayersDidChangeSentPerLayer"
    }
}

impl SdfNotice for LayersDidChangeSentPerLayer {}

/// Global notice sent to indicate that layer contents have changed.
#[derive(Clone, Debug)]
pub struct LayersDidChange {
    /// Base change data.
    base: BaseLayersDidChange,
}

impl LayersDidChange {
    /// Creates a new LayersDidChange notice.
    pub fn new(changes: LayerChangeListVec, serial_number: usize) -> Self {
        Self {
            base: BaseLayersDidChange::new(changes, serial_number),
        }
    }

    /// Returns the base change data.
    pub fn base(&self) -> &BaseLayersDidChange {
        &self.base
    }
}

impl Notice for LayersDidChange {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayersDidChange"
    }
}

impl SdfNotice for LayersDidChange {}

/// Sent when the (scene spec) info of a layer has changed.
#[derive(Debug, Clone)]
pub struct LayerInfoDidChange {
    /// The key that changed.
    key: Token,
}

impl LayerInfoDidChange {
    /// Creates a new LayerInfoDidChange notice.
    pub fn new(key: Token) -> Self {
        Self { key }
    }

    /// Returns the key that was affected.
    pub fn key(&self) -> &Token {
        &self.key
    }
}

impl Notice for LayerInfoDidChange {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerInfoDidChange"
    }
}

impl SdfNotice for LayerInfoDidChange {}

/// Sent when the identifier of a layer has changed.
#[derive(Debug, Clone)]
pub struct LayerIdentifierDidChange {
    /// The old identifier.
    old_id: String,
    /// The new identifier.
    new_id: String,
}

impl LayerIdentifierDidChange {
    /// Creates a new LayerIdentifierDidChange notice.
    pub fn new(old_identifier: String, new_identifier: String) -> Self {
        Self {
            old_id: old_identifier,
            new_id: new_identifier,
        }
    }

    /// Returns the old identifier.
    pub fn old_identifier(&self) -> &str {
        &self.old_id
    }

    /// Returns the new identifier.
    pub fn new_identifier(&self) -> &str {
        &self.new_id
    }
}

impl Notice for LayerIdentifierDidChange {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerIdentifierDidChange"
    }
}

impl SdfNotice for LayerIdentifierDidChange {}

/// Sent after a layer has been loaded from a file.
#[derive(Debug, Clone, Default)]
pub struct LayerDidReplaceContent;

impl LayerDidReplaceContent {
    /// Creates a new LayerDidReplaceContent notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for LayerDidReplaceContent {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerDidReplaceContent"
    }
}

impl SdfNotice for LayerDidReplaceContent {}

/// Sent after a layer is reloaded.
#[derive(Debug, Clone, Default)]
pub struct LayerDidReloadContent;

impl LayerDidReloadContent {
    /// Creates a new LayerDidReloadContent notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for LayerDidReloadContent {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerDidReloadContent"
    }
}

impl SdfNotice for LayerDidReloadContent {}

/// Sent after a layer is saved to file.
#[derive(Debug, Clone, Default)]
pub struct LayerDidSaveLayerToFile;

impl LayerDidSaveLayerToFile {
    /// Creates a new LayerDidSaveLayerToFile notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for LayerDidSaveLayerToFile {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerDidSaveLayerToFile"
    }
}

impl SdfNotice for LayerDidSaveLayerToFile {}

/// Sent when the dirty status of a layer changes.
#[derive(Debug, Clone, Default)]
pub struct LayerDirtinessChanged;

impl LayerDirtinessChanged {
    /// Creates a new LayerDirtinessChanged notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for LayerDirtinessChanged {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerDirtinessChanged"
    }
}

impl SdfNotice for LayerDirtinessChanged {}

/// Sent after a layer has been muted or unmuted.
///
/// Note this does not necessarily mean the specified layer is currently loaded.
#[derive(Debug, Clone)]
pub struct LayerMutenessChanged {
    /// Path of the layer.
    layer_path: String,
    /// True if the layer was muted, false if unmuted.
    was_muted: bool,
}

impl LayerMutenessChanged {
    /// Creates a new LayerMutenessChanged notice.
    pub fn new(layer_path: String, was_muted: bool) -> Self {
        Self {
            layer_path,
            was_muted,
        }
    }

    /// Returns the path of the layer that was muted or unmuted.
    pub fn layer_path(&self) -> &str {
        &self.layer_path
    }

    /// Returns true if the layer was muted, false if unmuted.
    pub fn was_muted(&self) -> bool {
        self.was_muted
    }
}

impl Notice for LayerMutenessChanged {
    fn notice_type_name() -> &'static str {
        "SdfNotice::LayerMutenessChanged"
    }
}

impl SdfNotice for LayerMutenessChanged {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_identifier_did_change() {
        let notice =
            LayerIdentifierDidChange::new("old/path.usda".to_string(), "new/path.usda".to_string());
        assert_eq!(notice.old_identifier(), "old/path.usda");
        assert_eq!(notice.new_identifier(), "new/path.usda");
    }

    #[test]
    fn test_layer_muteness_changed() {
        let notice = LayerMutenessChanged::new("layer.usda".to_string(), true);
        assert_eq!(notice.layer_path(), "layer.usda");
        assert!(notice.was_muted());

        let notice2 = LayerMutenessChanged::new("layer.usda".to_string(), false);
        assert!(!notice2.was_muted());
    }

    #[test]
    fn test_layer_info_did_change() {
        let notice = LayerInfoDidChange::new(Token::new("documentation"));
        assert_eq!(notice.key().as_str(), "documentation");
    }
}
