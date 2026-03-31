//! Proxy policies - traits defining behavior for various proxy types.
//!
//! Policies define how keys and values are handled in proxies. Different
//! proxy types use different policies to control their behavior.

use std::fmt::Debug;
use std::hash::Hash;

use usd_tf::Token;
use usd_vt::Value as VtValue;

use super::{Path, Payload, Reference};

// ============================================================================
// Key Policy Traits
// ============================================================================

/// Policy trait for handling keys in proxies.
///
/// Defines how keys are compared, hashed, and converted between
/// different representations.
pub trait KeyPolicy: Clone {
    /// The key type used by this policy.
    type Key: Clone + Debug + PartialEq + Eq + Hash;

    /// Returns the key as a Token.
    fn as_token(key: &Self::Key) -> Token;

    /// Creates a key from a Token.
    fn from_token(token: &Token) -> Self::Key;

    /// Returns true if the key is valid.
    fn is_valid(key: &Self::Key) -> bool;
}

/// Policy for string-based name keys.
#[derive(Debug, Clone)]
pub struct NameKeyPolicy;

impl KeyPolicy for NameKeyPolicy {
    type Key = String;

    fn as_token(key: &Self::Key) -> Token {
        Token::new(key)
    }

    fn from_token(token: &Token) -> Self::Key {
        token.as_str().to_string()
    }

    fn is_valid(key: &Self::Key) -> bool {
        !key.is_empty()
    }
}

/// Policy for Token-based name keys.
#[derive(Debug, Clone)]
pub struct NameTokenKeyPolicy;

impl KeyPolicy for NameTokenKeyPolicy {
    type Key = Token;

    fn as_token(key: &Self::Key) -> Token {
        key.clone()
    }

    fn from_token(token: &Token) -> Self::Key {
        token.clone()
    }

    fn is_valid(key: &Self::Key) -> bool {
        !key.is_empty()
    }
}

/// Policy for Path-based keys.
#[derive(Debug, Clone)]
pub struct PathKeyPolicy;

impl KeyPolicy for PathKeyPolicy {
    type Key = Path;

    fn as_token(key: &Self::Key) -> Token {
        Token::new(key.as_str())
    }

    fn from_token(token: &Token) -> Self::Key {
        Path::from_token(token).unwrap_or_else(Path::empty)
    }

    fn is_valid(key: &Self::Key) -> bool {
        !key.is_empty()
    }
}

// ============================================================================
// Type Policy Traits
// ============================================================================

/// Policy trait for handling values in proxies.
///
/// Defines the value type and how values are converted to/from VtValue.
pub trait TypePolicy: Clone {
    /// The value type managed by this policy.
    type Value: Clone + Debug;

    /// Converts a value to VtValue.
    fn to_vtvalue(value: &Self::Value) -> VtValue;

    /// Converts from VtValue to value type.
    /// Returns None if conversion fails.
    fn from_vtvalue(value: &VtValue) -> Option<Self::Value>;

    /// Returns true if the value is valid.
    fn is_valid(value: &Self::Value) -> bool;
}

/// Policy for Payload values.
#[derive(Debug, Clone)]
pub struct PayloadTypePolicy;

impl TypePolicy for PayloadTypePolicy {
    type Value = Payload;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<Payload>().cloned()
    }

    fn is_valid(_value: &Self::Value) -> bool {
        true
    }
}

/// Policy for Reference values.
#[derive(Debug, Clone)]
pub struct ReferenceTypePolicy;

impl TypePolicy for ReferenceTypePolicy {
    type Value = Reference;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<Reference>().cloned()
    }

    fn is_valid(_value: &Self::Value) -> bool {
        true
    }
}

/// Policy for SubLayer values (strings representing layer identifiers).
#[derive(Debug, Clone)]
pub struct SubLayerTypePolicy;

impl TypePolicy for SubLayerTypePolicy {
    type Value = String;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<String>().cloned()
    }

    fn is_valid(value: &Self::Value) -> bool {
        !value.is_empty()
    }
}

/// Policy for Path values.
#[derive(Debug, Clone)]
pub struct PathTypePolicy;

impl TypePolicy for PathTypePolicy {
    type Value = Path;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<Path>().cloned()
    }

    fn is_valid(value: &Self::Value) -> bool {
        !value.is_empty()
    }
}

/// Policy for Token values.
#[derive(Debug, Clone)]
pub struct TokenTypePolicy;

impl TypePolicy for TokenTypePolicy {
    type Value = Token;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<Token>().cloned()
    }

    fn is_valid(value: &Self::Value) -> bool {
        !value.is_empty()
    }
}

/// Policy for String values.
#[derive(Debug, Clone)]
pub struct StringTypePolicy;

impl TypePolicy for StringTypePolicy {
    type Value = String;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        VtValue::new(value.clone())
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        value.get::<String>().cloned()
    }

    fn is_valid(_value: &Self::Value) -> bool {
        true
    }
}

// ============================================================================
// Generic Map Value Policy
// ============================================================================

/// Policy for generic VtValue in maps.
#[derive(Debug, Clone)]
pub struct VtValuePolicy;

impl TypePolicy for VtValuePolicy {
    type Value = VtValue;

    fn to_vtvalue(value: &Self::Value) -> VtValue {
        value.clone()
    }

    fn from_vtvalue(value: &VtValue) -> Option<Self::Value> {
        Some(value.clone())
    }

    fn is_valid(_value: &Self::Value) -> bool {
        true
    }
}

// ============================================================================
// Predicates for filtering children
// ============================================================================

/// Predicate trait for filtering spec children in views.
pub trait ChildPredicate<Spec> {
    /// Returns true if the spec should be included in the view.
    fn accept(&self, spec: &Spec) -> bool;
}

/// Trivial predicate that accepts all specs.
#[derive(Debug, Clone)]
pub struct TrivialPredicate;

impl<Spec> ChildPredicate<Spec> for TrivialPredicate {
    fn accept(&self, _spec: &Spec) -> bool {
        true
    }
}

/// Generic spec view predicate.
#[derive(Debug, Clone)]
pub struct GenericSpecViewPredicate<F> {
    predicate: F,
}

impl<F> GenericSpecViewPredicate<F> {
    /// Creates a new predicate with the given function.
    pub fn new(predicate: F) -> Self {
        Self { predicate }
    }
}

impl<Spec, F> ChildPredicate<Spec> for GenericSpecViewPredicate<F>
where
    F: Fn(&Spec) -> bool + Clone,
{
    fn accept(&self, spec: &Spec) -> bool {
        (self.predicate)(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_key_policy() {
        let key = "test".to_string();
        assert!(NameKeyPolicy::is_valid(&key));

        let token = NameKeyPolicy::as_token(&key);
        assert_eq!(token.as_str(), "test");

        let key2 = NameKeyPolicy::from_token(&token);
        assert_eq!(key, key2);
    }

    #[test]
    fn test_path_key_policy() {
        let path = Path::from_string("/World/Cube").unwrap();
        assert!(PathKeyPolicy::is_valid(&path));

        let token = PathKeyPolicy::as_token(&path);
        assert_eq!(token.as_str(), "/World/Cube");
    }

    #[test]
    fn test_string_type_policy() {
        let value = "test".to_string();
        assert!(StringTypePolicy::is_valid(&value));

        let vt = StringTypePolicy::to_vtvalue(&value);
        let value2 = StringTypePolicy::from_vtvalue(&vt).unwrap();
        assert_eq!(value, value2);
    }

    #[test]
    fn test_trivial_predicate() {
        let pred = TrivialPredicate;
        assert!(pred.accept(&"anything"));
        assert!(pred.accept(&42));
    }
}
