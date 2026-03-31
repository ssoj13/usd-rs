//! Physics Filtered Pairs API schema.
//!
//! Fine-grained collision filtering between specific object pairs.
//! When applied to a body, collision, or articulation, the filteredPairs
//! relationship defines objects that should not collide with it.
//!
//! Note: FilteredPairsAPI filtering takes precedence over CollisionGroup filtering.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/filteredPairsAPI.h` and `filteredPairsAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::FilteredPairsAPI;
//!
//! // Apply to a prim and add filtered objects
//! let filtered = FilteredPairsAPI::apply(&prim)?;
//! filtered.create_filtered_pairs_rel()?.add_target(&other_path)?;
//! ```

use std::sync::Arc;

use usd_core::{Prim, Relationship, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics filtered pairs API schema.
///
/// API to describe fine-grained filtering. If a collision between two
/// objects occurs, this pair might be filtered if the pair is defined
/// through this API. Can be applied to a body, collision, or articulation.
///
/// Note that FilteredPairsAPI filtering has precedence over CollisionGroup
/// filtering.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsFilteredPairsAPI` class.
#[derive(Debug, Clone)]
pub struct FilteredPairsAPI {
    prim: Prim,
}

impl FilteredPairsAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsFilteredPairsAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a FilteredPairsAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a FilteredPairsAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Check if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        // FilteredPairsAPI can be applied to any prim
        prim.is_valid()
    }

    /// Apply this API schema to the given prim.
    ///
    /// Adds "PhysicsFilteredPairsAPI" to the apiSchemas metadata.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !Self::can_apply(prim) {
            return None;
        }
        if prim.apply_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // FilteredPairs Relationship
    // =========================================================================

    /// Relationship to objects that should be filtered (no collision).
    pub fn get_filtered_pairs_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_PHYSICS_TOKENS.physics_filtered_pairs.as_str())
    }

    /// Creates the filteredPairs relationship.
    pub fn create_filtered_pairs_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_PHYSICS_TOKENS.physics_filtered_pairs.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // FilteredPairsAPI has no attributes, only a relationship
        Vec::new()
    }
}

impl FilteredPairsAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for FilteredPairsAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<FilteredPairsAPI> for Prim {
    fn from(api: FilteredPairsAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for FilteredPairsAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(FilteredPairsAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(
            FilteredPairsAPI::SCHEMA_TYPE_NAME,
            "PhysicsFilteredPairsAPI"
        );
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = FilteredPairsAPI::get_schema_attribute_names(false);
        // FilteredPairsAPI has no attributes
        assert!(names.is_empty());
    }
}
