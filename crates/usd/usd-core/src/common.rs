//! Common types and definitions for the USD module.

use std::fmt;
use usd_tf::Token;

// ============================================================================
// InitialLoadSet
// ============================================================================

/// Specifies the initial set of prims to load when opening a UsdStage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InitialLoadSet {
    /// Load all loadable prims.
    #[default]
    LoadAll,
    /// Load no loadable prims.
    LoadNone,
}

// ============================================================================
// ListPosition
// ============================================================================

/// Position for inserting items in list-valued metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ListPosition {
    /// Prepend to the front of the list.
    FrontOfPrependList,
    /// Append to the back of the prepend list.
    #[default]
    BackOfPrependList,
    /// Prepend to the front of the append list.
    FrontOfAppendList,
    /// Append to the back of the list.
    BackOfAppendList,
}

// ============================================================================
// LoadPolicy
// ============================================================================

/// Policy for loading payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LoadPolicy {
    /// Load payloads that match the stage's load rules.
    #[default]
    LoadWithDescendants,
    /// Load only the specified prim's payload.
    LoadWithoutDescendants,
}

// ============================================================================
// SchemaKind
// ============================================================================

/// An enum representing which kind of schema a given schema class belongs to.
///
/// Matches C++ `UsdSchemaKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SchemaKind {
    /// Invalid or unknown schema kind.
    #[default]
    Invalid,
    /// Represents abstract or base schema types that are interface-only
    /// and cannot be instantiated. These are reserved for core base classes
    /// known to the usdGenSchema system, so this should never be assigned to
    /// generated schema classes.
    AbstractBase,
    /// Represents a non-concrete typed schema
    AbstractTyped,
    /// Represents a concrete typed schema
    ConcreteTyped,
    /// Non-applied API schema
    NonAppliedAPI,
    /// Single Apply API schema
    SingleApplyAPI,
    /// Multiple Apply API Schema
    MultipleApplyAPI,
}

// ============================================================================
// SchemaVersion
// ============================================================================

/// Schema versions are specified as a single unsigned integer value.
///
/// Matches C++ `UsdSchemaVersion`.
pub type SchemaVersion = u32;

// ============================================================================
// VersionPolicy
// ============================================================================

/// A policy for filtering by schema version when querying for schemas in a
/// particular schema family.
///
/// Matches C++ `UsdSchemaRegistry::VersionPolicy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionPolicy {
    /// All versions
    All,
    /// Greater than the specified version
    GreaterThan,
    /// Greater than or equal to the specified version
    GreaterThanOrEqual,
    /// Less than the specified version
    LessThan,
    /// Less than or equal to the specified version
    LessThanOrEqual,
}

// ============================================================================
// Tokens
// ============================================================================

/// Common USD tokens.
pub fn fallback_prim_types() -> Token {
    Token::new("fallbackPrimTypes")
}

// ============================================================================
// Display impls
// ============================================================================

impl fmt::Display for SchemaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaKind::Invalid => write!(f, "Invalid"),
            SchemaKind::AbstractBase => write!(f, "AbstractBase"),
            SchemaKind::AbstractTyped => write!(f, "AbstractTyped"),
            SchemaKind::ConcreteTyped => write!(f, "ConcreteTyped"),
            SchemaKind::NonAppliedAPI => write!(f, "NonAppliedAPI"),
            SchemaKind::SingleApplyAPI => write!(f, "SingleApplyAPI"),
            SchemaKind::MultipleApplyAPI => write!(f, "MultipleApplyAPI"),
        }
    }
}

impl fmt::Display for ListPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListPosition::FrontOfPrependList => write!(f, "FrontOfPrependList"),
            ListPosition::BackOfPrependList => write!(f, "BackOfPrependList"),
            ListPosition::FrontOfAppendList => write!(f, "FrontOfAppendList"),
            ListPosition::BackOfAppendList => write!(f, "BackOfAppendList"),
        }
    }
}

impl fmt::Display for LoadPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadPolicy::LoadWithDescendants => write!(f, "LoadWithDescendants"),
            LoadPolicy::LoadWithoutDescendants => write!(f, "LoadWithoutDescendants"),
        }
    }
}

impl fmt::Display for InitialLoadSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitialLoadSet::LoadAll => write!(f, "LoadAll"),
            InitialLoadSet::LoadNone => write!(f, "LoadNone"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_load_set_default() {
        assert_eq!(InitialLoadSet::default(), InitialLoadSet::LoadAll);
    }

    #[test]
    fn test_list_position_default() {
        assert_eq!(ListPosition::default(), ListPosition::BackOfPrependList);
    }

    #[test]
    fn test_load_policy_default() {
        assert_eq!(LoadPolicy::default(), LoadPolicy::LoadWithDescendants);
    }

    #[test]
    fn test_schema_kind_display() {
        assert_eq!(SchemaKind::Invalid.to_string(), "Invalid");
        assert_eq!(SchemaKind::ConcreteTyped.to_string(), "ConcreteTyped");
        assert_eq!(SchemaKind::SingleApplyAPI.to_string(), "SingleApplyAPI");
    }

    #[test]
    fn test_list_position_display() {
        assert_eq!(
            ListPosition::FrontOfPrependList.to_string(),
            "FrontOfPrependList"
        );
        assert_eq!(
            ListPosition::BackOfAppendList.to_string(),
            "BackOfAppendList"
        );
    }

    #[test]
    fn test_load_policy_display() {
        assert_eq!(
            LoadPolicy::LoadWithDescendants.to_string(),
            "LoadWithDescendants"
        );
    }

    #[test]
    fn test_initial_load_set_display() {
        assert_eq!(InitialLoadSet::LoadAll.to_string(), "LoadAll");
        assert_eq!(InitialLoadSet::LoadNone.to_string(), "LoadNone");
    }
}
