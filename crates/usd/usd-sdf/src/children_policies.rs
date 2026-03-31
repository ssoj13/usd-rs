//! Children policies - define behavior for different types of spec children.
//!
//! Policies control how children are stored, accessed, and validated for
//! different spec types (prims, properties, variants, etc).

use std::fmt::Debug;
use std::sync::OnceLock;

use usd_tf::Token;

use super::proxy_policies::{KeyPolicy, NameTokenKeyPolicy};

// Cached tokens for field names - these are called frequently during traversal
mod tokens {
    use super::*;

    macro_rules! cached_token {
        ($name:ident, $str:literal) => {
            pub fn $name() -> Token {
                static TOKEN: OnceLock<Token> = OnceLock::new();
                TOKEN.get_or_init(|| Token::new($str)).clone()
            }
        };
    }

    cached_token!(prim_children, "primChildren");
    cached_token!(properties, "properties");
    cached_token!(variant_sets, "variantSets");
    cached_token!(variants, "variants");
    cached_token!(target_paths, "targetPaths");
}
use super::{
    AttributeSpec, Path, PrimSpec, PropertySpec, RelationshipSpec, SpecType, VariantSetSpec,
    VariantSpec,
};

// ============================================================================
// Child Policy Trait
// ============================================================================

/// Policy trait for spec children.
///
/// Defines how children are identified, accessed, and validated for
/// a particular parent spec type.
pub trait ChildPolicy: Clone + Debug {
    /// The parent spec type.
    type Parent;

    /// The child spec type.
    type Child: Clone + Debug;

    /// The key type used to identify children.
    type Key: Clone + Debug + PartialEq + Eq + std::hash::Hash;

    /// Key policy for this child type.
    type KeyPolicy: KeyPolicy<Key = Self::Key>;

    /// Returns the field name where children are stored.
    fn get_children_field_name() -> Token;

    /// Returns the spec type of children.
    fn get_child_spec_type() -> SpecType;

    /// Validates that a child name is legal.
    fn is_valid_name(name: &str) -> bool;

    /// Creates a new child spec with the given name.
    fn create_child(parent: &Self::Parent, name: Self::Key) -> Option<Self::Child>;

    /// Gets a child by key.
    fn get_child(parent: &Self::Parent, key: &Self::Key) -> Option<Self::Child>;

    /// Returns true if the parent can have children.
    fn can_have_children(parent: &Self::Parent) -> bool;

    /// Converts a key to a path.
    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path;
}

// ============================================================================
// Token Child Policy - Base for name-based children
// ============================================================================

/// Base policy for children identified by Token names.
#[derive(Debug, Clone)]
pub struct TokenChildPolicy;

impl TokenChildPolicy {
    /// Checks if a name is valid for a child.
    pub fn is_valid_identifier(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Must start with letter or underscore
        let first = name.chars().next().expect("not empty");
        if !first.is_alphabetic() && first != '_' {
            return false;
        }

        // Rest must be alphanumeric or underscore
        name.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
    }
}

// ============================================================================
// Prim Child Policy
// ============================================================================

/// Policy for prim children (child prims of a prim).
#[derive(Debug, Clone)]
pub struct PrimChildPolicy;

impl ChildPolicy for PrimChildPolicy {
    type Parent = PrimSpec;
    type Child = PrimSpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::prim_children()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Prim
    }

    fn is_valid_name(name: &str) -> bool {
        TokenChildPolicy::is_valid_identifier(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        // Would create via layer API
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        // Would query via layer
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path {
        parent_path
            .append_child(key.as_str())
            .unwrap_or_else(Path::empty)
    }
}

// ============================================================================
// Property Child Policy
// ============================================================================

/// Policy for property children (properties of a prim).
#[derive(Debug, Clone)]
pub struct PropertyChildPolicy;

impl ChildPolicy for PropertyChildPolicy {
    type Parent = PrimSpec;
    type Child = PropertySpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::properties()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Attribute // Generic property - can be attribute or relationship
    }

    fn is_valid_name(name: &str) -> bool {
        TokenChildPolicy::is_valid_identifier(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path {
        parent_path
            .append_property(key.as_str())
            .unwrap_or_else(Path::empty)
    }
}

// ============================================================================
// Attribute Child Policy
// ============================================================================

/// Policy for attribute children (attributes of a prim).
#[derive(Debug, Clone)]
pub struct AttributeChildPolicy;

impl ChildPolicy for AttributeChildPolicy {
    type Parent = PrimSpec;
    type Child = AttributeSpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::properties()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Attribute
    }

    fn is_valid_name(name: &str) -> bool {
        PropertyChildPolicy::is_valid_name(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path {
        PropertyChildPolicy::key_to_path(parent_path, key)
    }
}

// ============================================================================
// Relationship Child Policy
// ============================================================================

/// Policy for relationship children (relationships of a prim).
#[derive(Debug, Clone)]
pub struct RelationshipChildPolicy;

impl ChildPolicy for RelationshipChildPolicy {
    type Parent = PrimSpec;
    type Child = RelationshipSpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::properties()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Relationship
    }

    fn is_valid_name(name: &str) -> bool {
        PropertyChildPolicy::is_valid_name(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path {
        PropertyChildPolicy::key_to_path(parent_path, key)
    }
}

// ============================================================================
// Variant Set Child Policy
// ============================================================================

/// Policy for variant set children (variant sets of a prim).
#[derive(Debug, Clone)]
pub struct VariantSetChildPolicy;

impl ChildPolicy for VariantSetChildPolicy {
    type Parent = PrimSpec;
    type Child = VariantSetSpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::variant_sets()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::VariantSet
    }

    fn is_valid_name(name: &str) -> bool {
        TokenChildPolicy::is_valid_identifier(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, key: &Self::Key) -> Path {
        parent_path
            .append_variant_selection(key.as_str(), "")
            .unwrap_or_else(Path::empty)
    }
}

// ============================================================================
// Variant Child Policy
// ============================================================================

/// Policy for variant children (variants in a variant set).
#[derive(Debug, Clone)]
pub struct VariantChildPolicy;

impl ChildPolicy for VariantChildPolicy {
    type Parent = VariantSetSpec;
    type Child = VariantSpec;
    type Key = Token;
    type KeyPolicy = NameTokenKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::variants()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Variant
    }

    fn is_valid_name(name: &str) -> bool {
        TokenChildPolicy::is_valid_identifier(name)
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, _key: &Self::Key) -> Option<Self::Child> {
        None
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(parent_path: &Path, _key: &Self::Key) -> Path {
        // Would need variant set name to construct proper path
        parent_path.clone()
    }
}

// ============================================================================
// Path Child Policy
// ============================================================================

/// Policy for Path-keyed children (e.g., connection targets).
#[derive(Debug, Clone)]
pub struct PathChildPolicy;

impl ChildPolicy for PathChildPolicy {
    type Parent = PropertySpec;
    type Child = Path;
    type Key = Path;
    type KeyPolicy = super::proxy_policies::PathKeyPolicy;

    fn get_children_field_name() -> Token {
        tokens::target_paths()
    }

    fn get_child_spec_type() -> SpecType {
        SpecType::Unknown // Path children don't have a spec type
    }

    fn is_valid_name(_name: &str) -> bool {
        true
    }

    fn create_child(_parent: &Self::Parent, _name: Self::Key) -> Option<Self::Child> {
        None
    }

    fn get_child(_parent: &Self::Parent, key: &Self::Key) -> Option<Self::Child> {
        Some(key.clone())
    }

    fn can_have_children(_parent: &Self::Parent) -> bool {
        true
    }

    fn key_to_path(_parent_path: &Path, key: &Self::Key) -> Path {
        key.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_child_policy_validation() {
        assert!(TokenChildPolicy::is_valid_identifier("validName"));
        assert!(TokenChildPolicy::is_valid_identifier("_underscore"));
        assert!(TokenChildPolicy::is_valid_identifier("name123"));
        assert!(TokenChildPolicy::is_valid_identifier("name:space"));

        assert!(!TokenChildPolicy::is_valid_identifier(""));
        assert!(!TokenChildPolicy::is_valid_identifier("123invalid"));
        assert!(!TokenChildPolicy::is_valid_identifier("-invalid"));
    }

    #[test]
    fn test_prim_child_policy() {
        assert_eq!(
            PrimChildPolicy::get_children_field_name().as_str(),
            "primChildren"
        );
        assert_eq!(PrimChildPolicy::get_child_spec_type(), SpecType::Prim);
        assert!(PrimChildPolicy::is_valid_name("Child"));
    }

    #[test]
    fn test_property_child_policy() {
        assert_eq!(
            PropertyChildPolicy::get_children_field_name().as_str(),
            "properties"
        );
        assert_eq!(
            PropertyChildPolicy::get_child_spec_type(),
            SpecType::Attribute
        );
    }
}
