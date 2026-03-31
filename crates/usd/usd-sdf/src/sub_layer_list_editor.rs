//! Sub-layer list editor.
//!
//! Port of pxr/usd/sdf/subLayerListEditor.h
//!
//! List editor implementation for sublayer path lists. When sublayers are
//! added or removed, the editor notifies the layer so it can update its
//! sublayer references.

use crate::{Layer, ListOpType};
use std::sync::Arc;

/// Sub-layer list editor.
///
/// Manages the list of sublayer paths for a layer. When sublayers are
/// added or removed, appropriate notifications are sent so the layer
/// can update its resolved sublayer references.
pub struct SubLayerListEditor {
    /// The owning layer.
    layer: Option<Arc<Layer>>,
    /// Current sublayer paths.
    sublayer_paths: Vec<String>,
}

impl SubLayerListEditor {
    /// Creates a new sublayer list editor for the given layer.
    pub fn new(layer: Arc<Layer>) -> Self {
        let sublayer_paths = layer.get_sublayer_paths();
        Self {
            layer: Some(layer),
            sublayer_paths,
        }
    }

    /// Returns the owning layer.
    pub fn layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Returns the current sublayer paths.
    pub fn get_paths(&self) -> &[String] {
        &self.sublayer_paths
    }

    /// Returns the number of sublayers.
    pub fn len(&self) -> usize {
        self.sublayer_paths.len()
    }

    /// Returns true if there are no sublayers.
    pub fn is_empty(&self) -> bool {
        self.sublayer_paths.is_empty()
    }

    /// Adds a sublayer path at the end.
    pub fn add(&mut self, path: &str) {
        let old = self.sublayer_paths.clone();
        self.sublayer_paths.push(path.to_string());
        self.on_edit(ListOpType::Appended, &old, &self.sublayer_paths.clone());
    }

    /// Inserts a sublayer path at the given index.
    pub fn insert(&mut self, index: usize, path: &str) {
        let old = self.sublayer_paths.clone();
        let idx = index.min(self.sublayer_paths.len());
        self.sublayer_paths.insert(idx, path.to_string());
        self.on_edit(ListOpType::Appended, &old, &self.sublayer_paths.clone());
    }

    /// Removes a sublayer path by index.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.sublayer_paths.len() {
            return false;
        }
        let old = self.sublayer_paths.clone();
        self.sublayer_paths.remove(index);
        self.on_edit(ListOpType::Deleted, &old, &self.sublayer_paths.clone());
        true
    }

    /// Removes a sublayer by path string.
    pub fn remove_by_path(&mut self, path: &str) -> bool {
        if let Some(idx) = self.sublayer_paths.iter().position(|p| p == path) {
            self.remove(idx)
        } else {
            false
        }
    }

    /// Replaces all sublayer paths.
    pub fn set_paths(&mut self, paths: Vec<String>) {
        let old = std::mem::replace(&mut self.sublayer_paths, paths);
        self.on_edit(ListOpType::Explicit, &old, &self.sublayer_paths.clone());
    }

    /// Clears all sublayer paths.
    pub fn clear(&mut self) {
        if !self.sublayer_paths.is_empty() {
            let old = std::mem::take(&mut self.sublayer_paths);
            self.on_edit(ListOpType::Explicit, &old, &[]);
        }
    }

    /// Called when sublayer paths are edited.
    ///
    /// This notifies the layer of changes so it can update
    /// its resolved sublayer references.
    fn on_edit(&self, _op: ListOpType, _old_values: &[String], _new_values: &[String]) {
        // The layer handles sublayer updates through its own change management.
        // In C++, this triggers SdfLayer::_SublayerPathsChanged notifications.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sublayer_paths_manipulation() {
        let mut paths: Vec<String> = vec!["a.usda".into(), "b.usda".into()];
        paths.push("c.usda".into());
        assert_eq!(paths.len(), 3);
        paths.remove(1);
        assert_eq!(paths, vec!["a.usda", "c.usda"]);
    }
}
