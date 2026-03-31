//! Physics Distance Joint schema.
//!
//! Predefined distance joint type where the distance between rigid bodies
//! may be limited to a given minimum or maximum distance.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/distanceJoint.h` and `distanceJoint.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::DistanceJoint;
//!
//! let joint = DistanceJoint::define(&stage, &path)?;
//! joint.create_min_distance_attr(Some(0.0))?;
//! joint.create_max_distance_attr(Some(10.0))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::joint::Joint;
use super::tokens::USD_PHYSICS_TOKENS;

/// Physics distance joint schema.
///
/// Predefined distance joint type (Distance between rigid bodies
/// may be limited to given minimum or maximum distance.)
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `minDistance`: Minimum distance. -1 means not limited. Units: distance.
/// - `maxDistance`: Maximum distance. -1 means not limited. Units: distance.
///
/// # Inheritance
///
/// Inherits from Joint - all base Joint attributes are available.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsDistanceJoint` class.
#[derive(Debug, Clone)]
pub struct DistanceJoint {
    /// The underlying joint.
    joint: Joint,
}

impl DistanceJoint {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsDistanceJoint";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a DistanceJoint on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            joint: Joint::new(prim),
        }
    }

    /// Construct a DistanceJoint from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a DistanceJoint holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Attempt to ensure a prim adhering to this schema at `path`
    /// is defined on this stage.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Access the underlying Joint.
    pub fn joint(&self) -> &Joint {
        &self.joint
    }

    // =========================================================================
    // MinDistance Attribute
    // =========================================================================

    /// Minimum distance. If attribute is negative, the joint is not
    /// limited. Units: distance.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:minDistance = -1` |
    /// | C++ Type | float |
    pub fn get_min_distance_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_min_distance.as_str())
    }

    /// Creates the minDistance attribute.
    pub fn create_min_distance_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_min_distance.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_f32(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // MaxDistance Attribute
    // =========================================================================

    /// Maximum distance. If attribute is negative, the joint is not
    /// limited. Units: distance.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:maxDistance = -1` |
    /// | C++ Type | float |
    pub fn get_max_distance_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_max_distance.as_str())
    }

    /// Creates the maxDistance attribute.
    pub fn create_max_distance_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_max_distance.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_f32(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            Joint::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_PHYSICS_TOKENS.physics_min_distance.clone(),
            USD_PHYSICS_TOKENS.physics_max_distance.clone(),
        ]);

        names
    }
}

// ============================================================================
// Delegate to Joint for common functionality
// ============================================================================

impl std::ops::Deref for DistanceJoint {
    type Target = Joint;

    fn deref(&self) -> &Self::Target {
        &self.joint
    }
}

impl DistanceJoint {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.joint.get_prim()
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for DistanceJoint {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<DistanceJoint> for Prim {
    fn from(joint: DistanceJoint) -> Self {
        joint.joint.into()
    }
}

impl AsRef<Prim> for DistanceJoint {
    fn as_ref(&self) -> &Prim {
        self.joint.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(DistanceJoint::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(DistanceJoint::SCHEMA_TYPE_NAME, "PhysicsDistanceJoint");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = DistanceJoint::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:minDistance"));
        assert!(names.iter().any(|n| n.get_text() == "physics:maxDistance"));
    }
}
