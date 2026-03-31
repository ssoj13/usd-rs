//! SdfOpaqueValue - type-erased opaque value storage.
//!
//! Port of pxr/usd/sdf/opaqueValue.h
//!
//! Provides opaque storage for values that should be preserved but not
//! interpreted during layer operations.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// An opaque value that preserves data without interpretation.
///
/// Used for values that need to be stored and round-tripped but
/// not understood by the SDF layer system.
#[derive(Clone)]
pub struct OpaqueValue {
    /// The stored value.
    data: Option<Arc<dyn Any + Send + Sync>>,
    /// Type name for debugging.
    type_name: String,
}

impl Default for OpaqueValue {
    fn default() -> Self {
        Self::empty()
    }
}

impl OpaqueValue {
    /// Creates an empty opaque value.
    pub fn empty() -> Self {
        Self {
            data: None,
            type_name: String::new(),
        }
    }

    /// Creates an opaque value from any type.
    pub fn new<T: Any + Send + Sync + 'static>(value: T) -> Self {
        Self {
            data: Some(Arc::new(value)),
            type_name: std::any::type_name::<T>().to_string(),
        }
    }

    /// Creates an opaque value from bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            data: Some(Arc::new(bytes)),
            type_name: "bytes".to_string(),
        }
    }

    /// Creates an opaque value from a string.
    pub fn from_string(s: String) -> Self {
        Self {
            data: Some(Arc::new(s)),
            type_name: "string".to_string(),
        }
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// Returns the type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Tries to get the value as a specific type.
    pub fn get<T: Any + 'static>(&self) -> Option<&T> {
        self.data.as_ref().and_then(|d| d.downcast_ref::<T>())
    }

    /// Gets the value as bytes if stored as bytes.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        self.get::<Vec<u8>>().map(|v| v.as_slice())
    }

    /// Gets the value as string if stored as string.
    pub fn as_string(&self) -> Option<&str> {
        self.get::<String>().map(|s| s.as_str())
    }

    /// Clones the inner data.
    pub fn clone_data<T: Any + Clone + 'static>(&self) -> Option<T> {
        self.get::<T>().cloned()
    }
}

impl fmt::Debug for OpaqueValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "OpaqueValue(empty)")
        } else {
            write!(f, "OpaqueValue({})", self.type_name)
        }
    }
}

impl PartialEq for OpaqueValue {
    fn eq(&self, other: &Self) -> bool {
        // Opaque values are only equal if both empty or same Arc
        match (&self.data, &other.data) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for OpaqueValue {}

impl std::hash::Hash for OpaqueValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.data {
            None => 0u8.hash(state),
            Some(arc) => {
                1u8.hash(state);
                Arc::as_ptr(arc).hash(state);
            }
        }
    }
}

/// Holder for opaque values in dictionaries.
#[derive(Clone, Debug, Default)]
pub struct OpaqueValueHolder {
    /// The values by key.
    values: std::collections::HashMap<String, OpaqueValue>,
}

impl OpaqueValueHolder {
    /// Creates an empty holder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a value.
    pub fn set(&mut self, key: impl Into<String>, value: OpaqueValue) {
        self.values.insert(key.into(), value);
    }

    /// Gets a value.
    pub fn get(&self, key: &str) -> Option<&OpaqueValue> {
        self.values.get(key)
    }

    /// Removes a value.
    pub fn remove(&mut self, key: &str) -> Option<OpaqueValue> {
        self.values.remove(key)
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of values.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(|s| s.as_str())
    }

    /// Returns an iterator over values.
    pub fn values(&self) -> impl Iterator<Item = &OpaqueValue> {
        self.values.values()
    }

    /// Returns an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &OpaqueValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let v = OpaqueValue::empty();
        assert!(v.is_empty());
    }

    #[test]
    fn test_from_bytes() {
        let v = OpaqueValue::from_bytes(vec![1, 2, 3]);
        assert!(!v.is_empty());
        assert_eq!(v.as_bytes(), Some(&[1u8, 2, 3][..]));
    }

    #[test]
    fn test_from_string() {
        let v = OpaqueValue::from_string("hello".to_string());
        assert!(!v.is_empty());
        assert_eq!(v.as_string(), Some("hello"));
    }

    #[test]
    fn test_custom_type() {
        #[derive(Clone, Debug, PartialEq)]
        struct Custom(i32);

        let v = OpaqueValue::new(Custom(42));
        assert!(!v.is_empty());
        assert_eq!(v.get::<Custom>(), Some(&Custom(42)));
    }

    #[test]
    fn test_holder() {
        let mut holder = OpaqueValueHolder::new();
        holder.set("key1", OpaqueValue::from_string("value1".to_string()));
        holder.set("key2", OpaqueValue::from_bytes(vec![1, 2, 3]));

        assert_eq!(holder.len(), 2);
        assert!(holder.get("key1").is_some());
        assert!(holder.get("key2").is_some());
        assert!(holder.get("key3").is_none());
    }
}
