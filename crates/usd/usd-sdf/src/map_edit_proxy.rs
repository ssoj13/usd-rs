//! Map edit proxy - modifiable view of map fields.
//!
//! `MapEditProxy` provides a mutable dictionary-like interface to map fields
//! stored in specs, such as customData, assetInfo, etc.
//!
//! Simplified version that stores the map directly.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Error type for map edit proxy operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapEditProxyError {
    /// Proxy has expired.
    Expired,
    /// Invalid key.
    InvalidKey(String),
    /// Invalid value.
    InvalidValue(String),
    /// Permission denied.
    PermissionDenied(String),
    /// Other error.
    Other(String),
}

impl fmt::Display for MapEditProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(f, "Map edit proxy has expired"),
            Self::InvalidKey(msg) => write!(f, "Invalid key: {}", msg),
            Self::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for MapEditProxyError {}

/// Result type for map edit proxy operations.
pub type MapEditProxyResult<T> = Result<T, MapEditProxyError>;

// ============================================================================
// MapEditProxy - Simplified version
// ============================================================================

/// Mutable proxy to a map field in a spec.
///
/// Simplified version that stores the map directly.
pub struct MapEditProxy<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    /// The map data.
    data: HashMap<K, V>,
}

impl<K, V> MapEditProxy<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    /// Creates a new empty map edit proxy.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Creates from an existing HashMap.
    pub fn from_map(data: HashMap<K, V>) -> Self {
        Self { data }
    }

    /// Creates an empty detached proxy.
    pub fn empty() -> Self {
        Self::new()
    }

    /// Returns true if the proxy has expired.
    pub fn is_expired(&self) -> bool {
        false // Simplified
    }

    /// Returns the number of key-value pairs.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns true if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.data.contains_key(key)
    }

    /// Gets the value for a key.
    pub fn get(&self, key: &K) -> Option<V> {
        self.data.get(key).cloned()
    }

    /// Sets a key-value pair.
    pub fn set(&mut self, key: K, value: V) -> MapEditProxyResult<()> {
        self.data.insert(key, value);
        Ok(())
    }

    /// Inserts a key-value pair if the key doesn't exist.
    pub fn insert(&mut self, key: K, value: V) -> MapEditProxyResult<bool> {
        let was_new = !self.data.contains_key(&key);
        self.data.insert(key, value);
        Ok(was_new)
    }

    /// Removes a key-value pair.
    pub fn erase(&mut self, key: &K) -> MapEditProxyResult<bool> {
        Ok(self.data.remove(key).is_some())
    }

    /// Removes a key and returns its value.
    pub fn remove(&mut self, key: &K) -> MapEditProxyResult<Option<V>> {
        Ok(self.data.remove(key))
    }

    /// Clears all key-value pairs.
    pub fn clear(&mut self) -> MapEditProxyResult<()> {
        self.data.clear();
        Ok(())
    }

    /// Returns an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.data.keys().cloned()
    }

    /// Returns an iterator over values.
    pub fn values(&self) -> impl Iterator<Item = V> + '_ {
        self.data.values().cloned()
    }

    /// Returns an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (K, V)> + '_ {
        self.data.iter().map(|(k, v)| (k.clone(), v.clone()))
    }

    /// Copies all key-value pairs from another map.
    pub fn copy_from(&mut self, other: &HashMap<K, V>) -> MapEditProxyResult<()> {
        self.data = other.clone();
        Ok(())
    }

    /// Returns a snapshot of the map as a HashMap.
    pub fn to_map(&self) -> HashMap<K, V> {
        self.data.clone()
    }

    /// Returns a reference to the underlying map.
    pub fn as_map(&self) -> &HashMap<K, V> {
        &self.data
    }
}

impl<K, V> Default for MapEditProxy<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Clone for MapEditProxy<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<K, V> fmt::Debug for MapEditProxy<K, V>
where
    K: Clone + Eq + Hash + fmt::Debug,
    V: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapEditProxy")
            .field("size", &self.size())
            .finish()
    }
}

// ============================================================================
// Common map proxy types
// ============================================================================

/// Proxy for dictionary fields (String -> VtValue).
pub type DictionaryProxy = MapEditProxy<String, VtValue>;

/// Proxy for Token-keyed maps.
pub type TokenDictionaryProxy = MapEditProxy<Token, VtValue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_edit_proxy_error_display() {
        let err = MapEditProxyError::Expired;
        assert_eq!(err.to_string(), "Map edit proxy has expired");

        let err = MapEditProxyError::InvalidKey("bad".to_string());
        assert!(err.to_string().contains("bad"));
    }

    #[test]
    fn test_empty_proxy() {
        let proxy: MapEditProxy<String, String> = MapEditProxy::empty();
        assert!(proxy.is_empty());
        assert_eq!(proxy.size(), 0);
    }

    #[test]
    fn test_basic_operations() {
        let mut proxy: MapEditProxy<String, String> = MapEditProxy::new();

        assert!(
            proxy
                .insert("key1".to_string(), "value1".to_string())
                .unwrap()
        );
        assert!(
            !proxy
                .insert("key1".to_string(), "value2".to_string())
                .unwrap()
        );

        assert_eq!(proxy.get(&"key1".to_string()), Some("value2".to_string()));
        assert_eq!(proxy.size(), 1);

        assert!(proxy.erase(&"key1".to_string()).unwrap());
        assert!(proxy.is_empty());
    }
}
