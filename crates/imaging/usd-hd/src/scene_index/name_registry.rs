//! HdSceneIndexNameRegistry - Singleton registry for named scene index instances.
//!
//! Corresponds to C++ `HdSceneIndexNameRegistry` in pxr/imaging/hd/sceneIndex.h:265-303.
//! Scene indices are not automatically registered; the application must manually add them.

use super::base::{HdSceneIndexHandle, HdSceneIndexWeakHandle};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Singleton registry for named scene index instances.
///
/// Stores weak references to scene indices by name. Expired references are
/// cleaned up lazily on access (GetRegisteredNames, GetNamedSceneIndex).
///
/// Matches C++ `HdSceneIndexNameRegistry` behavior exactly:
/// - RegisterNamedSceneIndex stores a Weak handle
/// - GetRegisteredNames cleans up expired entries while collecting names
/// - GetNamedSceneIndex returns None and erases if the entry has expired
pub struct HdSceneIndexNameRegistry {
    named_instances: Mutex<HashMap<String, HdSceneIndexWeakHandle>>,
}

/// Global singleton instance.
static INSTANCE: OnceLock<HdSceneIndexNameRegistry> = OnceLock::new();

impl HdSceneIndexNameRegistry {
    /// Returns the singleton instance of this registry.
    pub fn get_instance() -> &'static HdSceneIndexNameRegistry {
        INSTANCE.get_or_init(|| HdSceneIndexNameRegistry {
            named_instances: Mutex::new(HashMap::new()),
        })
    }

    /// Register a scene index instance with the given name.
    ///
    /// Stores a weak reference. If the scene index is dropped elsewhere,
    /// the entry will be cleaned up lazily on next access.
    pub fn register_named_scene_index(&self, name: &str, instance: &HdSceneIndexHandle) {
        let weak = std::sync::Arc::downgrade(instance);
        let mut map = self.named_instances.lock().unwrap();
        map.insert(name.to_string(), weak);
    }

    /// Returns the names of all registered (still-alive) scene indices.
    ///
    /// Cleans up expired weak references during iteration (matches C++ behavior).
    pub fn get_registered_names(&self) -> Vec<String> {
        let mut map = self.named_instances.lock().unwrap();
        let mut result = Vec::with_capacity(map.len());

        // Collect live names, remove expired entries (matches C++ erase-while-iterating)
        map.retain(|name, weak| {
            if weak.strong_count() > 0 {
                result.push(name.clone());
                true
            } else {
                false
            }
        });

        result
    }

    /// Returns the scene index registered with the given name.
    ///
    /// Returns None if not found or if the weak reference has expired.
    /// Expired entries are removed on access (matches C++ behavior).
    pub fn get_named_scene_index(&self, name: &str) -> Option<HdSceneIndexHandle> {
        let mut map = self.named_instances.lock().unwrap();

        if let Some(weak) = map.get(name) {
            if let Some(strong) = weak.upgrade() {
                return Some(strong);
            }
            // Expired - erase it (matches C++ line 287)
            map.remove(name);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_index::base::scene_index_to_handle;
    use crate::scene_index::retained::HdRetainedSceneIndex;

    #[test]
    fn test_register_and_get() {
        let registry = HdSceneIndexNameRegistry::get_instance();
        let si: HdSceneIndexHandle = scene_index_to_handle(HdRetainedSceneIndex::new());

        registry.register_named_scene_index("test_si", &si);

        let names = registry.get_registered_names();
        assert!(names.contains(&"test_si".to_string()));

        let got = registry.get_named_scene_index("test_si");
        assert!(got.is_some());
    }

    #[test]
    fn test_expired_cleanup() {
        let registry = HdSceneIndexNameRegistry::get_instance();

        // Register, then drop the strong ref
        {
            let si: HdSceneIndexHandle = scene_index_to_handle(HdRetainedSceneIndex::new());
            registry.register_named_scene_index("ephemeral", &si);
        }
        // Now the scene index is dropped, weak ref is expired

        let got = registry.get_named_scene_index("ephemeral");
        assert!(got.is_none());
    }

    #[test]
    fn test_not_found() {
        let registry = HdSceneIndexNameRegistry::get_instance();
        assert!(registry.get_named_scene_index("nonexistent").is_none());
    }
}
