//! Spec identity management.
//!
//! Port of pxr/usd/sdf/identity.h
//!
//! Manages the identity of specs across namespace edits.
//! When a spec is moved or renamed, its identity is updated
//! to track the new location.

use crate::{Layer, Path};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock, Weak};

/// Identifies the logical object behind an SdfSpec.
///
/// This is the layer the spec belongs to and the path to the spec.
/// Identities are tracked across namespace edits.
pub struct Identity {
    /// The path this identity refers to.
    path: RwLock<Path>,
    /// Weak reference to the registry (to avoid cycles).
    registry: Weak<IdentityRegistry>,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("path", &self.get_path())
            .finish()
    }
}

impl Identity {
    /// Creates a new identity.
    fn new(path: Path, registry: Weak<IdentityRegistry>) -> Self {
        Self {
            path: RwLock::new(path),
            registry,
        }
    }

    /// Returns the path this identity refers to.
    pub fn get_path(&self) -> Path {
        self.path
            .read()
            .expect("identity path lock poisoned")
            .clone()
    }

    /// Returns the layer this identity refers to.
    pub fn get_layer(&self) -> Option<Arc<Layer>> {
        self.registry.upgrade().map(|r| r.get_layer())
    }

    /// Updates the path (used during namespace edits).
    fn set_path(&self, new_path: Path) {
        *self.path.write().expect("identity path lock poisoned") = new_path;
    }
}

/// Handle to an identity.
pub type IdentityHandle = Arc<Identity>;

/// Registry that tracks identities for a layer.
///
/// The registry ensures that the same logical object always gets
/// the same identity, even across namespace edits.
pub struct IdentityRegistry {
    /// The layer that owns this registry.
    layer: Arc<Layer>,
    /// Map from path to identity.
    identities: RwLock<HashMap<Path, Weak<Identity>>>,
}

impl fmt::Debug for IdentityRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentityRegistry")
            .field("layer", &self.layer.identifier())
            .field("identity_count", &self.identity_count())
            .finish()
    }
}

impl IdentityRegistry {
    /// Creates a new identity registry for the given layer.
    pub fn new(layer: Arc<Layer>) -> Arc<Self> {
        Arc::new(Self {
            layer,
            identities: RwLock::new(HashMap::new()),
        })
    }

    /// Returns the layer that owns this registry.
    pub fn get_layer(&self) -> Arc<Layer> {
        self.layer.clone()
    }

    /// Returns the identity associated with the path, creating one if necessary.
    pub fn identify(self: &Arc<Self>, path: &Path) -> IdentityHandle {
        // First check if we already have an identity
        {
            let identities = self
                .identities
                .read()
                .expect("identity registry lock poisoned");
            if let Some(weak) = identities.get(path) {
                if let Some(strong) = weak.upgrade() {
                    return strong;
                }
            }
        }

        // Create a new identity
        let identity = Arc::new(Identity::new(path.clone(), Arc::downgrade(self)));

        // Store weak reference
        {
            let mut identities = self
                .identities
                .write()
                .expect("identity registry lock poisoned");
            identities.insert(path.clone(), Arc::downgrade(&identity));
        }

        identity
    }

    /// Updates identity in response to a namespace edit.
    pub fn move_identity(&self, old_path: &Path, new_path: &Path) {
        let mut identities = self
            .identities
            .write()
            .expect("identity registry lock poisoned");

        if let Some(weak) = identities.remove(old_path) {
            if let Some(strong) = weak.upgrade() {
                strong.set_path(new_path.clone());
                identities.insert(new_path.clone(), Arc::downgrade(&strong));
            }
        }

        // Also move any child identities
        let children_to_move: Vec<(Path, Weak<Identity>)> = identities
            .iter()
            .filter(|(p, _)| p.has_prefix(old_path))
            .map(|(p, w)| (p.clone(), w.clone()))
            .collect();

        for (child_path, weak) in children_to_move {
            if let Some(strong) = weak.upgrade() {
                // Calculate new child path
                if let Some(new_child_path) = child_path.replace_prefix(old_path, new_path) {
                    identities.remove(&child_path);
                    strong.set_path(new_child_path.clone());
                    identities.insert(new_child_path, Arc::downgrade(&strong));
                }
            }
        }
    }

    /// Removes expired identities from the registry.
    pub fn cleanup(&self) {
        let mut identities = self
            .identities
            .write()
            .expect("identity registry lock poisoned");
        identities.retain(|_, weak| weak.strong_count() > 0);
    }

    /// Returns the number of tracked identities.
    pub fn identity_count(&self) -> usize {
        self.identities.read().expect("rwlock poisoned").len()
    }

    /// Returns true if the registry has an identity for the given path.
    pub fn has_identity(&self, path: &Path) -> bool {
        let identities = self
            .identities
            .read()
            .expect("identity registry lock poisoned");
        if let Some(weak) = identities.get(path) {
            weak.strong_count() > 0
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_layer() -> Arc<Layer> {
        Layer::create_anonymous(Some("test"))
    }

    #[test]
    fn test_identify() {
        let layer = make_test_layer();
        let registry = IdentityRegistry::new(layer);

        let path = Path::from_string("/Prim").unwrap();
        let identity = registry.identify(&path);

        assert_eq!(identity.get_path(), path);
    }

    #[test]
    fn test_same_identity() {
        let layer = make_test_layer();
        let registry = IdentityRegistry::new(layer);

        let path = Path::from_string("/Prim").unwrap();
        let id1 = registry.identify(&path);
        let id2 = registry.identify(&path);

        assert!(Arc::ptr_eq(&id1, &id2));
    }

    #[test]
    fn test_move_identity() {
        let layer = make_test_layer();
        let registry = IdentityRegistry::new(layer);

        let old_path = Path::from_string("/OldPrim").unwrap();
        let new_path = Path::from_string("/NewPrim").unwrap();

        let identity = registry.identify(&old_path);
        registry.move_identity(&old_path, &new_path);

        assert_eq!(identity.get_path(), new_path);
        assert!(!registry.has_identity(&old_path));
        assert!(registry.has_identity(&new_path));
    }

    #[test]
    fn test_move_children() {
        let layer = make_test_layer();
        let registry = IdentityRegistry::new(layer);

        let parent = Path::from_string("/Parent").unwrap();
        let child = Path::from_string("/Parent/Child").unwrap();
        let new_parent = Path::from_string("/NewParent").unwrap();

        let _parent_id = registry.identify(&parent);
        let child_id = registry.identify(&child);

        registry.move_identity(&parent, &new_parent);

        assert_eq!(child_id.get_path().as_str(), "/NewParent/Child");
    }
}
