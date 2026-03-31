//! Layer registry for tracking all open USD layers.
//!
//! The `LayerRegistry` is a singleton that maintains references to all open
//! layers, allowing them to be found by identifier or real path. This enables
//! layer sharing and prevents duplicate loading of the same layer file.
//!
//! # Overview
//!
//! The registry uses weak references (`Weak<Layer>`) to track layers without
//! preventing them from being dropped. When the last strong reference to a layer
//! is dropped, the weak reference in the registry becomes invalid and is cleaned
//! up automatically on next access.
//!
//! # Thread Safety
//!
//! The registry is fully thread-safe using `RwLock` for concurrent access.
//! Multiple threads can read from the registry simultaneously, while write
//! operations (register/unregister) have exclusive access.
//!
//! # Anonymous Layers
//!
//! Anonymous layers are temporary layers that don't correspond to files.
//! They use special identifiers in the format `anon:<tag>:<counter>` and
//! are tracked separately for efficient lookup.
//!
//! # Examples
//!
//! ```ignore
//! use std::sync::Arc;
//! use usd_sdf::{Layer, LayerRegistry};
//!
//! // Get the singleton instance
//! let registry = LayerRegistry::instance();
//!
//! // Register a layer
//! let layer = Arc::new(Layer::new("path/to/layer.usd"));
//! registry.register(&layer);
//!
//! // Find a layer by identifier
//! if let Some(found) = registry.find("path/to/layer.usd") {
//!     println!("Found layer: {}", found.identifier());
//! }
//!
//! // Create an anonymous layer
//! let anon_id = LayerRegistry::generate_anonymous_identifier("temp");
//! let anon_layer = Arc::new(Layer::anonymous(&anon_id));
//! registry.register(&anon_layer);
//!
//! // List all registered layers
//! for layer in registry.all_layers() {
//!     println!("Layer: {}", layer.identifier());
//! }
//!
//! // Check how many layers are registered
//! let count = registry.layer_count();
//! println!("Total layers: {}", count);
//! ```

use std::collections::HashMap;
use std::path::Path as StdPath;
use std::sync::{Arc, RwLock, Weak};

use once_cell::sync::Lazy;

// ============================================================================
// Layer Type (placeholder until full implementation)
// ============================================================================

/// Placeholder for Layer type.
///
/// This will be replaced with the full Layer implementation.
#[derive(Debug)]
pub struct Layer {
    identifier: String,
    real_path: Option<String>,
    is_anonymous: bool,
}

impl Layer {
    /// Returns the layer identifier.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the real path if available.
    pub fn real_path(&self) -> Option<&str> {
        self.real_path.as_deref()
    }

    /// Returns true if this is an anonymous layer.
    pub fn is_anonymous(&self) -> bool {
        self.is_anonymous
    }
}

// ============================================================================
// Layer Registry
// ============================================================================

/// Global registry for tracking all open layers.
///
/// The `LayerRegistry` maintains weak references to all layers, allowing them
/// to be found by identifier or real path without preventing them from being
/// dropped when no longer in use.
///
/// # Thread Safety
///
/// The registry is thread-safe using `RwLock` for concurrent access.
/// Multiple readers can access the registry simultaneously, but writers
/// have exclusive access.
///
/// # Memory Management
///
/// Uses `Weak<Layer>` references to avoid keeping layers alive. When a layer
/// is dropped by all strong references, it is automatically cleaned up from
/// the registry on the next access.
pub struct LayerRegistry {
    /// Layers indexed by identifier.
    ///
    /// This includes both file paths and anonymous layer identifiers.
    layers: RwLock<HashMap<String, Weak<Layer>>>,

    /// Anonymous layers indexed by tag.
    ///
    /// Separate map for faster lookup of anonymous layers.
    anonymous_layers: RwLock<HashMap<String, Weak<Layer>>>,
}

impl LayerRegistry {
    /// Creates a new empty registry.
    fn new() -> Self {
        Self {
            layers: RwLock::new(HashMap::new()),
            anonymous_layers: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the global singleton instance.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// let registry = LayerRegistry::instance();
    /// ```
    #[must_use]
    pub fn instance() -> &'static Self {
        static INSTANCE: Lazy<LayerRegistry> = Lazy::new(LayerRegistry::new);
        &INSTANCE
    }

    /// Registers a layer in the registry.
    ///
    /// Creates weak references for both the identifier and real path (if available).
    /// If a layer with the same identifier already exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `layer` - The layer to register
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// use usd_sdf::{Layer, LayerRegistry};
    ///
    /// let layer = Arc::new(Layer::new("path/to/layer.usd"));
    /// LayerRegistry::instance().register(&layer);
    /// ```
    pub fn register(&self, layer: &Arc<Layer>) {
        let identifier = layer.identifier().to_string();
        let weak = Arc::downgrade(layer);

        // Register by identifier
        {
            let mut layers = self.layers.write().expect("rwlock poisoned");
            layers.insert(identifier.clone(), weak.clone());
        }

        // Register anonymous layers separately
        if layer.is_anonymous() {
            let mut anon_layers = self.anonymous_layers.write().expect("rwlock poisoned");
            anon_layers.insert(identifier.clone(), weak.clone());
        }

        // Register by real path if available
        if let Some(real_path) = layer.real_path() {
            let mut layers = self.layers.write().expect("rwlock poisoned");
            layers.insert(real_path.to_string(), weak);
        }
    }

    /// Unregisters a layer from the registry.
    ///
    /// Removes all references (identifier and real path) for the layer.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The identifier of the layer to unregister
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// LayerRegistry::instance().unregister("path/to/layer.usd");
    /// ```
    pub fn unregister(&self, identifier: &str) {
        // Find and remove the layer
        let layer = {
            let mut layers = self.layers.write().expect("rwlock poisoned");
            layers.remove(identifier)
        };

        // If found, also remove by real path
        if let Some(weak) = layer {
            if let Some(layer) = weak.upgrade() {
                // Remove from anonymous layers if applicable
                if layer.is_anonymous() {
                    let mut anon_layers = self.anonymous_layers.write().expect("rwlock poisoned");
                    anon_layers.remove(identifier);
                }

                // Remove by real path if available
                if let Some(real_path) = layer.real_path() {
                    let mut layers = self.layers.write().expect("rwlock poisoned");
                    layers.remove(real_path);
                }
            }
        }
    }

    /// Finds a layer by identifier.
    ///
    /// Returns `Some(Arc<Layer>)` if the layer is found and still alive,
    /// `None` otherwise. Automatically cleans up dead weak references.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The layer identifier to search for
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// if let Some(layer) = LayerRegistry::instance().find("path/to/layer.usd") {
    ///     println!("Found layer: {}", layer.identifier());
    /// }
    /// ```
    #[must_use]
    pub fn find(&self, identifier: &str) -> Option<Arc<Layer>> {
        // Try to upgrade the weak reference
        let result = {
            let layers = self.layers.read().expect("rwlock poisoned");
            layers.get(identifier).and_then(Weak::upgrade)
        };

        // Clean up if the reference is dead
        if result.is_none() {
            let mut layers = self.layers.write().expect("rwlock poisoned");
            if let Some(weak) = layers.get(identifier) {
                if weak.strong_count() == 0 {
                    layers.remove(identifier);
                }
            }
        }

        result
    }

    /// Finds a layer by real file path.
    ///
    /// Resolves the path to an absolute path before searching.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to search for
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::path::Path;
    /// use usd_sdf::LayerRegistry;
    ///
    /// let path = Path::new("path/to/layer.usd");
    /// if let Some(layer) = LayerRegistry::instance().find_by_real_path(path) {
    ///     println!("Found layer: {}", layer.identifier());
    /// }
    /// ```
    #[must_use]
    pub fn find_by_real_path(&self, path: &StdPath) -> Option<Arc<Layer>> {
        // Canonicalize the path if possible
        let resolved = path
            .canonicalize()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        self.find(&resolved)
    }

    /// Returns all currently registered layers.
    ///
    /// Only returns layers that are still alive (have strong references).
    /// Automatically cleans up dead weak references.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// let layers = LayerRegistry::instance().all_layers();
    /// for layer in layers {
    ///     println!("Layer: {}", layer.identifier());
    /// }
    /// ```
    #[must_use]
    pub fn all_layers(&self) -> Vec<Arc<Layer>> {
        let mut result = Vec::new();
        let mut dead_keys = Vec::new();

        {
            let layers = self.layers.read().expect("rwlock poisoned");
            for (key, weak) in layers.iter() {
                if let Some(layer) = weak.upgrade() {
                    // Only add unique layers (avoid duplicates from real path entries)
                    if !result.iter().any(|l: &Arc<Layer>| {
                        Arc::ptr_eq(l, &layer)
                    }) {
                        result.push(layer);
                    }
                } else {
                    dead_keys.push(key.clone());
                }
            }
        }

        // Clean up dead references
        if !dead_keys.is_empty() {
            let mut layers = self.layers.write().expect("rwlock poisoned");
            for key in dead_keys {
                layers.remove(&key);
            }
        }

        result
    }

    /// Returns the number of registered layers.
    ///
    /// Only counts layers that are still alive (have strong references).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// let count = LayerRegistry::instance().layer_count();
    /// println!("Registered layers: {}", count);
    /// ```
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.all_layers().len()
    }

    /// Generates a unique identifier for an anonymous layer.
    ///
    /// Anonymous layers are temporary layers that don't correspond to files.
    /// Their identifiers follow the pattern `anon:<tag>:<counter>`.
    ///
    /// # Arguments
    ///
    /// * `tag` - Optional tag to include in the identifier
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// let id1 = LayerRegistry::generate_anonymous_identifier("temp");
    /// let id2 = LayerRegistry::generate_anonymous_identifier("temp");
    /// assert_ne!(id1, id2); // Each call generates a unique ID
    /// ```
    #[must_use]
    pub fn generate_anonymous_identifier(tag: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        if tag.is_empty() {
            format!("anon:{counter:016x}")
        } else {
            format!("anon:{tag}:{counter:016x}")
        }
    }

    /// Checks if an identifier represents an anonymous layer.
    ///
    /// Anonymous layer identifiers start with "anon:".
    ///
    /// # Arguments
    ///
    /// * `identifier` - The identifier to check
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerRegistry;
    ///
    /// assert!(LayerRegistry::is_anonymous_identifier("anon:temp:0000000000000001"));
    /// assert!(!LayerRegistry::is_anonymous_identifier("path/to/layer.usd"));
    /// ```
    #[must_use]
    pub fn is_anonymous_identifier(identifier: &str) -> bool {
        identifier.starts_with("anon:")
    }
}

// ============================================================================
// Default Implementation
// ============================================================================

impl Default for LayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_layer(identifier: &str, real_path: Option<&str>) -> Arc<Layer> {
        Arc::new(Layer {
            identifier: identifier.to_string(),
            real_path: real_path.map(String::from),
            is_anonymous: LayerRegistry::is_anonymous_identifier(identifier),
        })
    }

    #[test]
    fn test_singleton() {
        let r1 = LayerRegistry::instance();
        let r2 = LayerRegistry::instance();
        assert!(std::ptr::eq(r1, r2));
    }

    #[test]
    fn test_register_and_find() {
        let registry = LayerRegistry::default();
        let layer = create_test_layer("test.usd", Some("/path/to/test.usd"));

        registry.register(&layer);

        // Find by identifier
        let found = registry.find("test.usd");
        assert!(found.is_some());
        assert_eq!(found.unwrap().identifier(), "test.usd");

        // Find by real path
        let found = registry.find("/path/to/test.usd");
        assert!(found.is_some());
    }

    #[test]
    fn test_unregister() {
        let registry = LayerRegistry::default();
        let layer = create_test_layer("test.usd", Some("/path/to/test.usd"));

        registry.register(&layer);
        assert!(registry.find("test.usd").is_some());

        registry.unregister("test.usd");
        assert!(registry.find("test.usd").is_none());
    }

    #[test]
    fn test_weak_references() {
        let registry = LayerRegistry::default();
        
        {
            let layer = create_test_layer("temp.usd", None);
            registry.register(&layer);
            assert!(registry.find("temp.usd").is_some());
            // layer is dropped here
        }

        // The weak reference should be cleaned up
        assert!(registry.find("temp.usd").is_none());
    }

    #[test]
    fn test_all_layers() {
        let registry = LayerRegistry::default();
        let layer1 = create_test_layer("layer1.usd", None);
        let layer2 = create_test_layer("layer2.usd", None);

        registry.register(&layer1);
        registry.register(&layer2);

        let layers = registry.all_layers();
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn test_layer_count() {
        let registry = LayerRegistry::default();
        assert_eq!(registry.layer_count(), 0);

        let layer1 = create_test_layer("layer1.usd", None);
        let layer2 = create_test_layer("layer2.usd", None);

        registry.register(&layer1);
        assert_eq!(registry.layer_count(), 1);

        registry.register(&layer2);
        assert_eq!(registry.layer_count(), 2);
    }

    #[test]
    fn test_anonymous_identifier() {
        let id1 = LayerRegistry::generate_anonymous_identifier("");
        let id2 = LayerRegistry::generate_anonymous_identifier("");
        assert_ne!(id1, id2);
        assert!(id1.starts_with("anon:"));
        assert!(id2.starts_with("anon:"));
    }

    #[test]
    fn test_anonymous_identifier_with_tag() {
        let id = LayerRegistry::generate_anonymous_identifier("temp");
        assert!(id.starts_with("anon:temp:"));
        assert!(LayerRegistry::is_anonymous_identifier(&id));
    }

    #[test]
    fn test_is_anonymous_identifier() {
        assert!(LayerRegistry::is_anonymous_identifier("anon:0000000000000001"));
        assert!(LayerRegistry::is_anonymous_identifier("anon:temp:0000000000000001"));
        assert!(!LayerRegistry::is_anonymous_identifier("path/to/layer.usd"));
        assert!(!LayerRegistry::is_anonymous_identifier(""));
    }

    #[test]
    fn test_anonymous_layer_registration() {
        let registry = LayerRegistry::default();
        let id = LayerRegistry::generate_anonymous_identifier("test");
        let layer = create_test_layer(&id, None);

        registry.register(&layer);

        // Should be findable
        let found = registry.find(&id);
        assert!(found.is_some());
        assert!(found.unwrap().is_anonymous());
    }

    #[test]
    fn test_duplicate_registration() {
        let registry = LayerRegistry::default();
        let layer1 = create_test_layer("test.usd", None);
        let layer2 = create_test_layer("test.usd", None);

        registry.register(&layer1);
        registry.register(&layer2);

        // Second registration should replace the first
        let found = registry.find("test.usd");
        assert!(found.is_some());
        assert!(Arc::ptr_eq(&found.unwrap(), &layer2));
    }

    #[test]
    fn test_find_by_real_path() {
        let registry = LayerRegistry::default();
        let layer = create_test_layer("test.usd", Some("/absolute/path/test.usd"));

        registry.register(&layer);

        // Find by real path string (not using file system resolution)
        let found = registry.find("/absolute/path/test.usd");
        assert!(found.is_some());
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let registry = LayerRegistry::instance();
        let layer = create_test_layer("concurrent.usd", None);
        registry.register(&layer);

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    let found = LayerRegistry::instance().find("concurrent.usd");
                    assert!(found.is_some());
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_all_layers_no_duplicates() {
        let registry = LayerRegistry::default();
        let layer = create_test_layer("test.usd", Some("/path/to/test.usd"));
        registry.register(&layer);

        let all = registry.all_layers();
        // Should only return one layer even though it's registered twice
        // (once by identifier, once by real path)
        assert_eq!(all.len(), 1);
    }
}
