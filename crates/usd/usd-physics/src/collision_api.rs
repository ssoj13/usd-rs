//! Physics Collision API schema.
//!
//! Applies collision attributes to a UsdGeomXformable prim. If a simulation
//! is running, this geometry will collide with other geometries that have
//! PhysicsCollisionAPI applied.
//!
//! If any prim in the parent hierarchy has the RigidBodyAPI applied, the
//! collider is considered a part of the closest ancestor body. If there is
//! no body in the parent hierarchy, this collider is considered to be static.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/collisionAPI.h` and `collisionAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::CollisionAPI;
//!
//! // Apply collision to a mesh
//! let collision = CollisionAPI::apply(&mesh_prim)?;
//! collision.create_collision_enabled_attr(Some(true))?;
//! ```

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, SchemaKind, Stage};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics collision API schema.
///
/// Applies collision attributes to a UsdGeomXformable prim. If a
/// simulation is running, this geometry will collide with other geometries
/// that have PhysicsCollisionAPI applied. If any prim in the parent hierarchy
/// has the RigidBodyAPI applied, the collider is considered a part of the
/// closest ancestor body. If there is no body in the parent hierarchy,
/// this collider is considered to be static.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsCollisionAPI` class.
#[derive(Debug, Clone)]
pub struct CollisionAPI {
    prim: Prim,
}

impl CollisionAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsCollisionAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a CollisionAPI on the given prim.
    ///
    /// Equivalent to `CollisionAPI::get(prim.get_stage(), prim.get_path())`
    /// for a valid prim, but will not immediately throw an error for
    /// an invalid prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a CollisionAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a CollisionAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path` on `stage`, or if the prim does not
    /// have this API schema applied, return None.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    ///
    /// If this schema cannot be applied to the prim, this returns false and,
    /// if provided, populates `why_not` with the reason it cannot be applied.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&Token::new(Self::SCHEMA_TYPE_NAME))
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// This information is stored by adding "PhysicsCollisionAPI" to the
    /// token-valued, listOp metadata `apiSchemas` on the prim.
    ///
    /// Returns a valid CollisionAPI object upon success, or None upon failure.
    pub fn apply(prim: &Prim) -> Option<Self> {
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
    // CollisionEnabled Attribute
    // =========================================================================

    /// Determines if the PhysicsCollisionAPI is enabled.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool physics:collisionEnabled = 1` |
    /// | C++ Type | bool |
    pub fn get_collision_enabled_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_collision_enabled.as_str())
    }

    /// Creates the collisionEnabled attribute.
    pub fn create_collision_enabled_attr(&self, default_value: Option<bool>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_collision_enabled.as_str(),
            &type_name,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // SimulationOwner Relationship
    // =========================================================================

    /// Single PhysicsScene that will simulate this collider.
    ///
    /// By default this object belongs to the first PhysicsScene.
    /// Note that if a RigidBodyAPI in the hierarchy above has a different
    /// simulationOwner then it has a precedence over this relationship.
    pub fn get_simulation_owner_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_PHYSICS_TOKENS.physics_simulation_owner.as_str())
    }

    /// Creates the simulationOwner relationship.
    pub fn create_simulation_owner_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_PHYSICS_TOKENS.physics_simulation_owner.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![USD_PHYSICS_TOKENS.physics_collision_enabled.clone()]
    }
}

impl CollisionAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this collision is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Check if collision is enabled.
    ///
    /// Returns true if collisionEnabled attribute is true or not authored (defaults to true).
    pub fn is_enabled(&self) -> bool {
        if let Some(attr) = self.get_collision_enabled_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(true)
        } else {
            true
        }
    }

    /// Get the simulation owner scene for this collider.
    ///
    /// Returns the PhysicsScene prim that will simulate this collider, or None.
    pub fn get_simulation_owner(&self) -> Option<Prim> {
        let rel = self.get_simulation_owner_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            None
        } else {
            self.prim.stage()?.get_prim_at_path(&targets[0])
        }
    }
}

// ============================================================================
// From implementations for type conversions
// ============================================================================

impl From<Prim> for CollisionAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<CollisionAPI> for Prim {
    fn from(api: CollisionAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for CollisionAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(CollisionAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(CollisionAPI::SCHEMA_TYPE_NAME, "PhysicsCollisionAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = CollisionAPI::get_schema_attribute_names(false);
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:collisionEnabled")
        );
    }
}
