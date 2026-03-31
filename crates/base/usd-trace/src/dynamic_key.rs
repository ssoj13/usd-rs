//! Dynamic trace keys.
//!
//! Port of pxr/base/trace/dynamicKey.h
//!
//! This module provides support for dynamic trace keys that can be created
//! at runtime, as opposed to static keys defined at compile time.

use std::hash::{Hash, Hasher};
use usd_tf::Token;

// ============================================================================
// Static Key Data
// ============================================================================

/// Static key data for trace events.
///
/// This structure holds the name of a trace key and is used for both
/// static (compile-time) and dynamic (runtime) keys.
#[derive(Debug, Clone)]
pub struct StaticKeyData {
    /// The name of the trace key.
    name: &'static str,
}

impl StaticKeyData {
    /// Creates a new static key data with the given name.
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// Returns the name of the key.
    pub fn name(&self) -> &str {
        self.name
    }
}

impl PartialEq for StaticKeyData {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for StaticKeyData {}

impl Hash for StaticKeyData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

// ============================================================================
// Dynamic Key
// ============================================================================

/// A dynamic trace key that can be created at runtime.
///
/// This class stores data used to create dynamic keys which can be referenced
/// in TraceEvent instances.
///
/// If a key is known at compile time, it is preferable to use a static
/// TraceStaticKeyData instance instead.
#[derive(Debug, Clone)]
pub struct DynamicKey {
    /// The token storing the key name.
    key: Token,
    /// The cached name pointer for the static key data interface.
    name: String,
}

impl DynamicKey {
    /// Creates a new dynamic key from a Token.
    pub fn from_token(name: Token) -> Self {
        let name_str = name.get_text().to_string();
        Self {
            key: name,
            name: name_str,
        }
    }

    /// Creates a new dynamic key from a string.
    pub fn from_str(name: &str) -> Self {
        let token = Token::from(name);
        Self {
            key: token,
            name: name.to_string(),
        }
    }

    /// Creates a new dynamic key from a String.
    pub fn from_string(name: String) -> Self {
        let token = Token::from(name.as_str());
        Self { key: token, name }
    }

    /// Returns a reference to the underlying token.
    pub fn token(&self) -> &Token {
        &self.key
    }

    /// Returns the name of the key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a cached hash code for this key.
    pub fn hash_value(&self) -> u64 {
        self.key.hash()
    }
}

impl PartialEq for DynamicKey {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for DynamicKey {}

impl Hash for DynamicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash().hash(state);
    }
}

impl From<Token> for DynamicKey {
    fn from(token: Token) -> Self {
        Self::from_token(token)
    }
}

impl From<&str> for DynamicKey {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for DynamicKey {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

// ============================================================================
// Hash Functor
// ============================================================================

/// A hash functor which uses the cached hash.
///
/// May be used to store keys in a HashMap.
pub struct DynamicKeyHasher;

impl DynamicKeyHasher {
    /// Computes the hash of a dynamic key.
    pub fn hash(key: &DynamicKey) -> u64 {
        key.hash_value()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_dynamic_key_from_str() {
        let key = DynamicKey::from_str("TestKey");
        assert_eq!(key.name(), "TestKey");
    }

    #[test]
    fn test_dynamic_key_from_string() {
        let key = DynamicKey::from_string("TestKey".to_string());
        assert_eq!(key.name(), "TestKey");
    }

    #[test]
    fn test_dynamic_key_equality() {
        let key1 = DynamicKey::from_str("TestKey");
        let key2 = DynamicKey::from_str("TestKey");
        let key3 = DynamicKey::from_str("OtherKey");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_dynamic_key_hash() {
        let key1 = DynamicKey::from_str("TestKey");
        let key2 = DynamicKey::from_str("TestKey");

        assert_eq!(key1.hash_value(), key2.hash_value());
    }

    #[test]
    fn test_dynamic_key_in_hashset() {
        let mut set = HashSet::new();

        let key1 = DynamicKey::from_str("Key1");
        let key2 = DynamicKey::from_str("Key2");
        let key3 = DynamicKey::from_str("Key1"); // duplicate

        set.insert(key1.clone());
        set.insert(key2.clone());
        set.insert(key3);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&key1));
        assert!(set.contains(&key2));
    }

    #[test]
    fn test_static_key_data() {
        let data = StaticKeyData::new("TestKey");
        assert_eq!(data.name(), "TestKey");
    }
}
