//! PCP Layer Stack Registry.
//!
//! A registry of layer stacks that caches and manages layer stack instances.
//! This is an internal component used by PcpCache to avoid recomputing layer
//! stacks that have already been composed.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/layerStackRegistry.h` and `layerStackRegistry.cpp`.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, Weak};

use crate::{ErrorType, LayerStack, LayerStackIdentifier, LayerStackPtr, LayerStackRefPtr};
use usd_sdf::LayerHandle;

/// A registry of layer stacks.
///
/// The registry caches layer stacks by their identifier, allowing efficient
/// lookup and reuse of already-composed layer stacks.
pub struct LayerStackRegistry {
    /// The root layer stack identifier.
    root_layer_stack_id: LayerStackIdentifier,
    /// File format target for layer stacks in this registry.
    file_format_target: String,
    /// Whether this is in USD mode.
    is_usd: bool,
    /// Muted layers helper.
    muted_layers: MutedLayers,
    /// Internal data protected by RwLock.
    data: RwLock<LayerStackRegistryData>,
}

/// Internal data for the registry.
#[derive(Default)]
struct LayerStackRegistryData {
    /// Map from identifier to layer stack.
    identifier_to_layer_stack: HashMap<LayerStackIdentifier, LayerStackRefPtr>,
    /// Map from layer to layer stacks that use it.
    layer_to_layer_stacks: HashMap<String, Vec<LayerStackPtr>>,
    /// Map from muted layer id to layer stacks that would use it.
    muted_layer_to_layer_stacks: HashMap<String, Vec<LayerStackPtr>>,
}

/// Reference-counted pointer to a LayerStackRegistry.
pub type LayerStackRegistryRefPtr = Arc<LayerStackRegistry>;

/// Weak pointer to a LayerStackRegistry.
pub type LayerStackRegistryPtr = Weak<LayerStackRegistry>;

impl LayerStackRegistry {
    /// Creates a new layer stack registry.
    ///
    /// # Arguments
    ///
    /// * `root_layer_stack_id` - The identifier for the root layer stack
    /// * `file_format_target` - Target file format (e.g., "usd")
    /// * `is_usd` - Whether this is in USD mode
    pub fn new(
        root_layer_stack_id: LayerStackIdentifier,
        file_format_target: String,
        is_usd: bool,
    ) -> LayerStackRegistryRefPtr {
        Arc::new(Self {
            root_layer_stack_id,
            file_format_target: file_format_target.clone(),
            is_usd,
            muted_layers: MutedLayers::new(file_format_target),
            data: RwLock::new(LayerStackRegistryData::default()),
        })
    }

    /// Returns the root layer stack identifier.
    pub fn root_layer_stack_identifier(&self) -> &LayerStackIdentifier {
        &self.root_layer_stack_id
    }

    /// Returns the file format target.
    pub fn file_format_target(&self) -> &str {
        &self.file_format_target
    }

    /// Returns whether this registry is in USD mode.
    pub fn is_usd(&self) -> bool {
        self.is_usd
    }

    // ========================================================================
    // Layer Stack Lookup
    // ========================================================================

    /// Returns the layer stack for the given identifier if it exists,
    /// otherwise creates a new layer stack.
    ///
    /// Returns None if the identifier is invalid (null root layer).
    pub fn find_or_create(
        &self,
        identifier: &LayerStackIdentifier,
        errors: &mut Vec<ErrorType>,
    ) -> Option<LayerStackRefPtr> {
        // Check if we already have this layer stack
        {
            let data = self.data.read().expect("rwlock poisoned");
            if let Some(layer_stack) = data.identifier_to_layer_stack.get(identifier) {
                return Some(layer_stack.clone());
            }
        }

        // Create new layer stack
        self.create_layer_stack(identifier, errors)
    }

    /// Returns the layer stack for the given identifier if it exists.
    pub fn find(&self, identifier: &LayerStackIdentifier) -> Option<LayerStackRefPtr> {
        let data = self.data.read().expect("rwlock poisoned");
        data.identifier_to_layer_stack.get(identifier).cloned()
    }

    /// Returns true if this registry contains the given layer stack.
    pub fn contains(&self, layer_stack: &LayerStackPtr) -> bool {
        let data = self.data.read().expect("rwlock poisoned");
        if let Some(strong) = layer_stack.upgrade() {
            data.identifier_to_layer_stack
                .values()
                .any(|ls| Arc::ptr_eq(ls, &strong))
        } else {
            false
        }
    }

    /// Returns every layer stack that includes the given layer.
    pub fn find_all_using_layer(&self, layer: &LayerHandle) -> Vec<LayerStackPtr> {
        let data = self.data.read().expect("rwlock poisoned");
        // Get layer identifier
        let layer_id = if let Some(l) = layer.upgrade() {
            l.identifier().to_string()
        } else {
            return Vec::new();
        };
        data.layer_to_layer_stacks
            .get(&layer_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns every layer stack that uses the given muted layer.
    pub fn find_all_using_muted_layer(&self, layer_id: &str) -> Vec<LayerStackPtr> {
        let data = self.data.read().expect("rwlock poisoned");
        data.muted_layer_to_layer_stacks
            .get(layer_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns all layer stacks in this registry.
    pub fn get_all_layer_stacks(&self) -> Vec<LayerStackPtr> {
        let data = self.data.read().expect("rwlock poisoned");
        data.identifier_to_layer_stack
            .values()
            .map(Arc::downgrade)
            .collect()
    }

    /// Iterates over all layer stacks in this registry.
    pub fn for_each_layer_stack<F>(&self, mut f: F)
    where
        F: FnMut(&LayerStackRefPtr),
    {
        let data = self.data.read().expect("rwlock poisoned");
        for layer_stack in data.identifier_to_layer_stack.values() {
            f(layer_stack);
        }
    }

    // ========================================================================
    // Muted Layers
    // ========================================================================

    /// Mutes and unmutes layers.
    ///
    /// Relative paths will be anchored to the given anchor layer.
    /// On completion, the vectors will be filled with the canonical identifiers
    /// for layers that were actually added or removed.
    pub fn mute_and_unmute_layers(
        &self,
        anchor_layer: Option<&LayerHandle>,
        layers_to_mute: &mut Vec<String>,
        layers_to_unmute: &mut Vec<String>,
    ) {
        self.muted_layers
            .mute_and_unmute_layers(anchor_layer, layers_to_mute, layers_to_unmute);
    }

    /// Returns the list of canonical identifiers for muted layers.
    pub fn get_muted_layers(&self) -> Vec<String> {
        self.muted_layers.get_muted_layers()
    }

    /// Returns true if the layer is muted.
    ///
    /// If the layer is muted, `canonical_id` will be set to the canonical
    /// identifier for the muted layer.
    pub fn is_layer_muted(
        &self,
        anchor_layer: Option<&LayerHandle>,
        layer_identifier: &str,
        canonical_id: Option<&mut String>,
    ) -> bool {
        self.muted_layers
            .is_layer_muted(anchor_layer, layer_identifier, canonical_id)
    }

    // ========================================================================
    // Internal Methods
    // ========================================================================

    /// Creates a new layer stack for the given identifier.
    fn create_layer_stack(
        &self,
        identifier: &LayerStackIdentifier,
        _errors: &mut Vec<ErrorType>,
    ) -> Option<LayerStackRefPtr> {
        // Check if root layer path is valid
        if identifier.root_layer.get_authored_path().is_empty() {
            return None;
        }

        // Create the layer stack
        let layer_stack = LayerStack::new(identifier.clone());

        // Register the layer stack
        let mut data = self.data.write().expect("rwlock poisoned");
        data.identifier_to_layer_stack
            .insert(identifier.clone(), layer_stack.clone());

        // Update layer-to-layer-stack map
        self.update_layer_mappings(&mut data, &layer_stack);

        Some(layer_stack)
    }

    /// Updates the layer-to-layer-stack mappings for the given layer stack.
    fn update_layer_mappings(
        &self,
        data: &mut LayerStackRegistryData,
        layer_stack: &LayerStackRefPtr,
    ) {
        let weak = Arc::downgrade(layer_stack);

        for layer in layer_stack.get_layers() {
            let layer_id = layer.identifier().to_string();
            data.layer_to_layer_stacks
                .entry(layer_id)
                .or_default()
                .push(weak.clone());
        }
    }

    /// Removes a layer stack from the registry.
    #[allow(dead_code)] // Internal API - used by cache cleanup
    pub(crate) fn remove(&self, identifier: &LayerStackIdentifier) {
        let mut data = self.data.write().expect("rwlock poisoned");

        if let Some(layer_stack) = data.identifier_to_layer_stack.remove(identifier) {
            // Remove from layer mappings
            for layer in layer_stack.get_layers() {
                let layer_id = layer.identifier().to_string();
                if let Some(stacks) = data.layer_to_layer_stacks.get_mut(&layer_id) {
                    stacks.retain(|weak| {
                        if let Some(strong) = weak.upgrade() {
                            !Arc::ptr_eq(&strong, &layer_stack)
                        } else {
                            false
                        }
                    });
                }
            }
        }
    }
}

// ============================================================================
// Muted Layers Helper
// ============================================================================

/// Helper for maintaining and querying a collection of muted layers.
pub struct MutedLayers {
    /// File format target for layer resolution.
    file_format_target: String,
    /// Set of muted layer identifiers.
    layers: RwLock<HashSet<String>>,
}

impl MutedLayers {
    /// Creates a new muted layers helper.
    pub fn new(file_format_target: String) -> Self {
        Self {
            file_format_target,
            layers: RwLock::new(HashSet::new()),
        }
    }

    /// Returns the list of muted layer identifiers.
    pub fn get_muted_layers(&self) -> Vec<String> {
        self.layers
            .read()
            .expect("rwlock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Mutes and unmutes layers.
    pub fn mute_and_unmute_layers(
        &self,
        anchor_layer: Option<&LayerHandle>,
        layers_to_mute: &mut Vec<String>,
        layers_to_unmute: &mut Vec<String>,
    ) {
        let mut layers = self.layers.write().expect("rwlock poisoned");

        // Process muting
        let mut actually_muted = Vec::new();
        for layer_id in layers_to_mute.drain(..) {
            let canonical = self.get_canonical_layer_id(anchor_layer, &layer_id);
            if layers.insert(canonical.clone()) {
                actually_muted.push(canonical);
            }
        }
        *layers_to_mute = actually_muted;

        // Process unmuting
        let mut actually_unmuted = Vec::new();
        for layer_id in layers_to_unmute.drain(..) {
            let canonical = self.get_canonical_layer_id(anchor_layer, &layer_id);
            if layers.remove(&canonical) {
                actually_unmuted.push(canonical);
            }
        }
        *layers_to_unmute = actually_unmuted;
    }

    /// Returns true if the layer is muted.
    pub fn is_layer_muted(
        &self,
        anchor_layer: Option<&LayerHandle>,
        layer_identifier: &str,
        canonical_id: Option<&mut String>,
    ) -> bool {
        let canonical = self.get_canonical_layer_id(anchor_layer, layer_identifier);
        let layers = self.layers.read().expect("rwlock poisoned");

        if layers.contains(&canonical) {
            if let Some(out) = canonical_id {
                *out = canonical;
            }
            true
        } else {
            false
        }
    }

    /// Returns the file format target used for layer resolution.
    pub fn file_format_target(&self) -> &str {
        &self.file_format_target
    }

    /// Gets the canonical layer identifier.
    fn get_canonical_layer_id(&self, anchor_layer: Option<&LayerHandle>, layer_id: &str) -> String {
        // For now, just return the layer_id as-is
        // In a full implementation, this would resolve relative paths using the anchor layer
        if let Some(_anchor) = anchor_layer {
            // Could resolve relative paths here
            layer_id.to_string()
        } else {
            layer_id.to_string()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let id = LayerStackIdentifier::new("test.usda");
        let registry = LayerStackRegistry::new(id.clone(), "usd".to_string(), true);

        assert_eq!(registry.root_layer_stack_identifier(), &id);
        assert_eq!(registry.file_format_target(), "usd");
        assert!(registry.is_usd());
    }

    #[test]
    fn test_registry_find_not_found() {
        let id = LayerStackIdentifier::new("test.usda");
        let registry = LayerStackRegistry::new(id.clone(), "usd".to_string(), true);

        let other_id = LayerStackIdentifier::new("other.usda");
        assert!(registry.find(&other_id).is_none());
    }

    #[test]
    fn test_muted_layers() {
        let muted = MutedLayers::new("usd".to_string());

        // Initially empty
        assert!(muted.get_muted_layers().is_empty());

        // Mute a layer
        let mut to_mute = vec!["layer1.usda".to_string()];
        let mut to_unmute = Vec::new();
        muted.mute_and_unmute_layers(None, &mut to_mute, &mut to_unmute);

        assert_eq!(to_mute.len(), 1);
        assert!(muted.is_layer_muted(None, "layer1.usda", None));

        // Unmute it
        let mut to_mute = Vec::new();
        let mut to_unmute = vec!["layer1.usda".to_string()];
        muted.mute_and_unmute_layers(None, &mut to_mute, &mut to_unmute);

        assert!(!muted.is_layer_muted(None, "layer1.usda", None));
    }

    #[test]
    fn test_get_all_layer_stacks_empty() {
        let id = LayerStackIdentifier::new("test.usda");
        let registry = LayerStackRegistry::new(id, "usd".to_string(), true);

        let stacks = registry.get_all_layer_stacks();
        assert!(stacks.is_empty());
    }

    #[test]
    fn test_muted_layers_file_format_target() {
        let muted = MutedLayers::new("usd".to_string());
        assert_eq!(muted.file_format_target(), "usd");

        let muted2 = MutedLayers::new("usda".to_string());
        assert_eq!(muted2.file_format_target(), "usda");
    }
}
