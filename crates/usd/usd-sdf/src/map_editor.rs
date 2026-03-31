//! Sdf_MapEditor - interface for map editing operations.
//!
//! Port of pxr/usd/sdf/mapEditor.h
//!
//! Interface for private implementations used by SdfMapEditProxy.

use crate::{Allowed, Layer, Path, Spec};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Weak};
use usd_tf::Token;

/// Trait for map editors.
///
/// Interface for private implementations used by map edit proxies.
pub trait MapEditor<K, V>: Send + Sync
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    /// Returns a string describing the location of the map.
    fn get_location(&self) -> String;

    /// Returns the owner spec.
    fn get_owner(&self) -> Option<Spec>;

    /// Returns true if the map is expired.
    fn is_expired(&self) -> bool;

    /// Returns const reference to map data.
    fn get_data(&self) -> Option<&HashMap<K, V>>;

    /// Returns mutable reference to map data.
    fn get_data_mut(&mut self) -> Option<&mut HashMap<K, V>>;

    /// Copies from another map.
    fn copy(&mut self, other: &HashMap<K, V>);

    /// Sets a key-value pair.
    fn set(&mut self, key: K, value: V);

    /// Inserts a key-value pair, returns (success, was_new).
    fn insert(&mut self, key: K, value: V) -> (bool, bool);

    /// Erases a key.
    fn erase(&mut self, key: &K) -> bool;

    /// Validates a key.
    fn is_valid_key(&self, key: &K) -> Allowed;

    /// Validates a value.
    fn is_valid_value(&self, value: &V) -> Allowed;
}

/// Simple in-memory map editor implementation.
#[derive(Debug)]
pub struct SimpleMapEditor<K, V>
where
    K: Clone + Eq + Hash + std::fmt::Debug,
    V: Clone + std::fmt::Debug,
{
    /// Weak reference to layer.
    layer: Weak<Layer>,
    /// Path to owning spec.
    path: Path,
    /// Field name.
    field: Token,
    /// The map data.
    data: HashMap<K, V>,
}

impl<K, V> SimpleMapEditor<K, V>
where
    K: Clone + Eq + Hash + std::fmt::Debug,
    V: Clone + std::fmt::Debug,
{
    /// Creates a new simple map editor.
    pub fn new(layer: &Arc<Layer>, path: Path, field: Token) -> Self {
        Self {
            layer: Arc::downgrade(layer),
            path,
            field,
            data: HashMap::new(),
        }
    }

    /// Creates an empty detached map editor.
    pub fn empty() -> Self {
        Self {
            layer: Weak::new(),
            path: Path::empty(),
            field: Token::empty(),
            data: HashMap::new(),
        }
    }

    /// Returns the path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the field.
    pub fn field(&self) -> &Token {
        &self.field
    }

    /// Returns the layer.
    pub fn layer(&self) -> Option<Arc<Layer>> {
        self.layer.upgrade()
    }
}

impl<K, V> MapEditor<K, V> for SimpleMapEditor<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + std::fmt::Debug,
    V: Clone + Send + Sync + std::fmt::Debug,
{
    fn get_location(&self) -> String {
        format!("{}:{}", self.path.get_string(), self.field.as_str())
    }

    fn get_owner(&self) -> Option<Spec> {
        // Would need layer access to get spec
        None
    }

    fn is_expired(&self) -> bool {
        self.layer.upgrade().is_none() && !self.path.is_empty()
    }

    fn get_data(&self) -> Option<&HashMap<K, V>> {
        Some(&self.data)
    }

    fn get_data_mut(&mut self) -> Option<&mut HashMap<K, V>> {
        Some(&mut self.data)
    }

    fn copy(&mut self, other: &HashMap<K, V>) {
        self.data = other.clone();
    }

    fn set(&mut self, key: K, value: V) {
        self.data.insert(key, value);
    }

    fn insert(&mut self, key: K, value: V) -> (bool, bool) {
        use std::collections::hash_map::Entry;
        match self.data.entry(key) {
            Entry::Occupied(_) => (true, false),
            Entry::Vacant(e) => {
                e.insert(value);
                (true, true)
            }
        }
    }

    fn erase(&mut self, key: &K) -> bool {
        self.data.remove(key).is_some()
    }

    fn is_valid_key(&self, _key: &K) -> Allowed {
        Allowed::yes()
    }

    fn is_valid_value(&self, _value: &V) -> Allowed {
        Allowed::yes()
    }
}

/// String-to-string map editor.
pub type StringMapEditor = SimpleMapEditor<String, String>;

/// Token-to-token map editor.
pub type TokenMapEditor = SimpleMapEditor<Token, Token>;

/// String-to-value map editor (for custom data).
pub type CustomDataEditor = SimpleMapEditor<String, usd_vt::Value>;

/// Creates a map editor for a spec field.
pub fn create_map_editor<K, V>(
    layer: &Arc<Layer>,
    path: &Path,
    field: &Token,
) -> SimpleMapEditor<K, V>
where
    K: Clone + Eq + Hash + std::fmt::Debug,
    V: Clone + std::fmt::Debug,
{
    SimpleMapEditor::new(layer, path.clone(), field.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_map_editor() {
        let mut editor: SimpleMapEditor<String, i32> = SimpleMapEditor::empty();

        editor.set("a".to_string(), 1);
        editor.set("b".to_string(), 2);

        assert_eq!(editor.get_data().unwrap().get("a"), Some(&1));
        assert_eq!(editor.get_data().unwrap().get("b"), Some(&2));

        assert!(editor.erase(&"a".to_string()));
        assert!(!editor.erase(&"c".to_string()));

        assert_eq!(editor.get_data().unwrap().len(), 1);
    }

    #[test]
    fn test_insert() {
        let mut editor: SimpleMapEditor<String, i32> = SimpleMapEditor::empty();

        let (ok, was_new) = editor.insert("key".to_string(), 100);
        assert!(ok);
        assert!(was_new);

        let (ok, was_new) = editor.insert("key".to_string(), 200);
        assert!(ok);
        assert!(!was_new); // Already existed
    }
}
