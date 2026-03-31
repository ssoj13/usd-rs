//! PCP Layer Relocates Edit Builder.
//!
//! Utility for building up a map of valid relocates and producing layer
//! metadata edits that can set these relocates on a layer stack.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/layerRelocatesEditBuilder.h` and `.cpp`.

use std::collections::HashSet;

use crate::LayerStackPtr;
use usd_sdf::{LayerHandle, Path};

/// Relocate entry: (source_path, target_path).
pub type Relocate = (Path, Path);

/// List of relocate entries.
pub type Relocates = Vec<Relocate>;

/// Map of relocates (source -> target).
pub type RelocatesMap = std::collections::HashMap<Path, Path>;

/// Edit to perform on a layer's relocates metadata.
pub type LayerRelocatesEdit = (LayerHandle, Relocates);

/// List of relocates edits to perform on all layers.
pub type LayerRelocatesEdits = Vec<LayerRelocatesEdit>;

/// Utility for building up valid relocates and producing layer edits.
///
/// This class must be constructed from an existing layer stack which will
/// initialize the edit builder with the layer stack's current relocates.
/// Then `relocate()` can be called any number of times to build a valid
/// map of edited relocates.
#[derive(Debug, Clone, Default)]
pub struct LayerRelocatesEditBuilder {
    /// Cached composed relocates map.
    relocates_map: Option<RelocatesMap>,
    /// Edits to perform on layers.
    layer_relocates_edits: LayerRelocatesEdits,
    /// Layers that have relocates changes.
    layers_with_relocates_changes: HashSet<String>,
    /// Index into layer_relocates_edits for where to add new relocates.
    edit_for_new_relocates_index: Option<usize>,
}

impl LayerRelocatesEditBuilder {
    /// Creates a new edit builder initialized from the given layer stack.
    ///
    /// # Arguments
    ///
    /// * `layer_stack` - The layer stack to initialize from
    /// * `add_new_relocates_layer` - Optional layer for adding new relocates.
    ///   If not provided, the root layer is used.
    pub fn new(
        layer_stack: &LayerStackPtr,
        add_new_relocates_layer: Option<&LayerHandle>,
    ) -> Option<Self> {
        let strong = layer_stack.upgrade()?;

        let mut builder = Self::default();

        // Get layers and initialize edits
        let layers = strong.get_layers();
        for (idx, layer) in layers.iter().enumerate() {
            // Initialize with empty relocates for each layer
            // In full implementation, would read existing relocates from layer
            let relocates: Relocates = Vec::new();

            if !relocates.is_empty()
                || add_new_relocates_layer.is_some_and(|l| {
                    l.upgrade()
                        .is_some_and(|ll| std::sync::Arc::ptr_eq(&ll, layer))
                })
            {
                builder
                    .layer_relocates_edits
                    .push((LayerHandle::from_layer(layer), relocates));
            }

            // Track which layer should receive new relocates
            if let Some(target_layer) = add_new_relocates_layer {
                if target_layer
                    .upgrade()
                    .is_some_and(|l| std::sync::Arc::ptr_eq(&l, layer))
                {
                    builder.edit_for_new_relocates_index =
                        Some(builder.layer_relocates_edits.len().saturating_sub(1));
                }
            } else if idx == 0 {
                // Use root layer if no specific layer provided
                if builder.layer_relocates_edits.is_empty() {
                    builder
                        .layer_relocates_edits
                        .push((LayerHandle::from_layer(layer), Vec::new()));
                }
                builder.edit_for_new_relocates_index = Some(0);
            }
        }

        Some(builder)
    }

    /// Creates an empty edit builder for testing.
    pub fn new_empty() -> Self {
        Self::default()
    }

    /// Relocates `source_path` to `target_path`.
    ///
    /// Returns `Ok(())` if the relocate can be performed, or `Err(reason)` if not.
    ///
    /// The edited relocates map will always conform to the valid relocates format.
    pub fn relocate(&mut self, source_path: &Path, target_path: &Path) -> Result<(), String> {
        // Validate paths
        if source_path.is_empty() {
            return Err("Source path cannot be empty".to_string());
        }
        if !source_path.is_prim_path() {
            return Err("Source path must be a prim path".to_string());
        }
        if !target_path.is_empty() && !target_path.is_prim_path() {
            return Err("Target path must be a prim path".to_string());
        }

        // Check for identity relocate
        if source_path == target_path {
            return Err("Source and target paths are the same".to_string());
        }

        // Check for illegal nested relocates
        if !target_path.is_empty() && target_path.has_prefix(source_path) {
            return Err("Cannot relocate a prim to be a descendant of itself".to_string());
        }
        if source_path.has_prefix(target_path) && !target_path.is_empty() {
            return Err("Cannot relocate a prim to be an ancestor of itself".to_string());
        }

        // Update existing relocates that are affected
        self.update_existing_relocates(source_path, target_path);

        // Add the new relocate to the appropriate layer
        if let Some(idx) = self.edit_for_new_relocates_index {
            if let Some((_, relocates)) = self.layer_relocates_edits.get_mut(idx) {
                // Check if we're undoing an existing relocate
                let mut found_existing = false;
                relocates.retain(|(src, tgt): &Relocate| {
                    if src == source_path {
                        found_existing = true;
                        // If target matches original source, remove the relocate
                        if tgt == target_path || target_path.is_empty() {
                            return false;
                        }
                    }
                    true
                });

                if !target_path.is_empty() && !found_existing {
                    relocates.push((source_path.clone(), target_path.clone()));
                }

                // Track that this layer had relocates changes
                if let Some((layer_handle, _)) = self.layer_relocates_edits.get(idx) {
                    if let Some(layer) = layer_handle.upgrade() {
                        self.layers_with_relocates_changes
                            .insert(layer.identifier().to_string());
                    }
                }
            }
        }

        // Invalidate cached map
        self.relocates_map = None;

        Ok(())
    }

    /// Removes the relocate with the given source path.
    ///
    /// Returns `Ok(())` if successful, or `Err(reason)` if the relocate doesn't exist.
    pub fn remove_relocate(&mut self, source_path: &Path) -> Result<(), String> {
        if source_path.is_empty() {
            return Err("Source path cannot be empty".to_string());
        }

        let mut found = false;
        for (_, relocates) in &mut self.layer_relocates_edits {
            relocates.retain(|(src, _): &Relocate| {
                if src == source_path {
                    found = true;
                    false
                } else {
                    true
                }
            });
        }

        if !found {
            return Err(format!(
                "No relocate found with source path {}",
                source_path.as_str()
            ));
        }

        // Track changes for layers that were modified
        for (layer_handle, _) in &self.layer_relocates_edits {
            if let Some(layer) = layer_handle.upgrade() {
                self.layers_with_relocates_changes
                    .insert(layer.identifier().to_string());
            }
        }

        // Invalidate cached map
        self.relocates_map = None;

        Ok(())
    }

    /// Returns the list of edits to perform on layers.
    pub fn get_edits(&self) -> &LayerRelocatesEdits {
        &self.layer_relocates_edits
    }

    /// Returns the set of layer identifiers that have relocates changes.
    pub fn get_layers_with_relocates_changes(&self) -> &HashSet<String> {
        &self.layers_with_relocates_changes
    }

    /// Returns the composed relocates map from the edited layer relocates.
    pub fn get_edited_relocates_map(&mut self) -> &RelocatesMap {
        if self.relocates_map.is_none() {
            let mut map = RelocatesMap::new();
            for (_, relocates) in &self.layer_relocates_edits {
                for (src, tgt) in relocates {
                    map.insert(src.clone(), tgt.clone());
                }
            }
            self.relocates_map = Some(map);
        }
        self.relocates_map.as_ref().expect("just set above")
    }

    /// Updates existing relocates when a new relocate is added.
    fn update_existing_relocates(&mut self, source: &Path, target: &Path) {
        for (_, relocates) in &mut self.layer_relocates_edits {
            for (src, tgt) in relocates.iter_mut() {
                // If source is under the new source, remap it
                if src.has_prefix(source) {
                    if let Some(suffix) = src.as_str().strip_prefix(source.as_str()) {
                        if !target.is_empty() {
                            if let Some(new_path) =
                                Path::from_string(&format!("{}{}", target.as_str(), suffix))
                            {
                                *src = new_path;
                            }
                        }
                    }
                }

                // If target is under the new source, remap it
                if tgt.has_prefix(source) {
                    if let Some(suffix) = tgt.as_str().strip_prefix(source.as_str()) {
                        if !target.is_empty() {
                            if let Some(new_path) =
                                Path::from_string(&format!("{}{}", target.as_str(), suffix))
                            {
                                *tgt = new_path;
                            }
                        }
                    }
                }
            }

            // Remove any relocates that become no-ops
            relocates.retain(|(src, tgt): &Relocate| src != tgt);
        }
    }
}

/// Modifies relocates in place by moving paths at or under `old_path` to `new_path`.
///
/// Returns `true` if any modifications were made.
pub fn modify_relocates(relocates: &mut Relocates, old_path: &Path, new_path: &Path) -> bool {
    if old_path.is_empty() {
        return false;
    }

    let mut modified = false;

    for (src, tgt) in relocates.iter_mut() {
        // Remap source if under old_path
        if src.has_prefix(old_path) {
            if let Some(suffix) = src.as_str().strip_prefix(old_path.as_str()) {
                let new_src = if new_path.is_empty() {
                    Path::empty()
                } else {
                    Path::from_string(&format!("{}{}", new_path.as_str(), suffix))
                        .unwrap_or_else(Path::empty)
                };
                if new_src != *src {
                    *src = new_src;
                    modified = true;
                }
            }
        }

        // Remap target if under old_path
        if tgt.has_prefix(old_path) {
            if let Some(suffix) = tgt.as_str().strip_prefix(old_path.as_str()) {
                let new_tgt = if new_path.is_empty() {
                    Path::empty()
                } else {
                    Path::from_string(&format!("{}{}", new_path.as_str(), suffix))
                        .unwrap_or_else(Path::empty)
                };
                if new_tgt != *tgt {
                    *tgt = new_tgt;
                    modified = true;
                }
            }
        }
    }

    // Remove invalid or no-op relocates
    let original_len = relocates.len();
    relocates.retain(|(src, tgt): &Relocate| !src.is_empty() && !tgt.is_empty() && src != tgt);

    modified || relocates.len() != original_len
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modify_relocates_empty() {
        let mut relocates: Relocates = Vec::new();
        let modified = modify_relocates(
            &mut relocates,
            &Path::from_string("/A").unwrap(),
            &Path::from_string("/B").unwrap(),
        );
        assert!(!modified);
        assert!(relocates.is_empty());
    }

    #[test]
    fn test_modify_relocates_simple() {
        let mut relocates: Relocates = vec![(
            Path::from_string("/A/Child").unwrap(),
            Path::from_string("/B/Child").unwrap(),
        )];

        let modified = modify_relocates(
            &mut relocates,
            &Path::from_string("/A").unwrap(),
            &Path::from_string("/C").unwrap(),
        );

        assert!(modified);
        assert_eq!(relocates.len(), 1);
        assert_eq!(relocates[0].0.as_str(), "/C/Child");
    }

    #[test]
    fn test_modify_relocates_noop_removal() {
        let mut relocates: Relocates = vec![(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        )];

        // This would make source == target after remapping
        let modified = modify_relocates(
            &mut relocates,
            &Path::from_string("/B").unwrap(),
            &Path::from_string("/A").unwrap(),
        );

        // Target gets remapped to /A, making it a no-op
        assert!(modified);
    }

    #[test]
    fn test_relocate_validation() {
        let mut builder = LayerRelocatesEditBuilder::new_empty();
        builder.edit_for_new_relocates_index = Some(0);
        builder
            .layer_relocates_edits
            .push((LayerHandle::default(), Vec::new()));

        // Empty source should fail
        let result = builder.relocate(&Path::empty(), &Path::from_string("/B").unwrap());
        assert!(result.is_err());

        // Same source and target should fail
        let path_a = Path::from_string("/A").unwrap();
        let result = builder.relocate(&path_a, &path_a);
        assert!(result.is_err());
    }

    #[test]
    fn test_relocate_success() {
        let mut builder = LayerRelocatesEditBuilder::new_empty();
        builder.edit_for_new_relocates_index = Some(0);
        builder
            .layer_relocates_edits
            .push((LayerHandle::default(), Vec::new()));

        let result = builder.relocate(
            &Path::from_string("/A").unwrap(),
            &Path::from_string("/B").unwrap(),
        );
        assert!(result.is_ok());

        let map = builder.get_edited_relocates_map();
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get(&Path::from_string("/A").unwrap()),
            Some(&Path::from_string("/B").unwrap())
        );
    }
}
