//! Physics Revolute Joint schema.
//!
//! Predefined revolute joint type where rotation along the joint axis
//! is permitted. Used for hinges and similar rotational constraints.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/revoluteJoint.h` and `revoluteJoint.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::RevoluteJoint;
//!
//! let joint = RevoluteJoint::define(&stage, &path)?;
//! joint.create_axis_attr(Some("X".into()))?;
//! joint.create_lower_limit_attr(Some(-90.0))?;
//! joint.create_upper_limit_attr(Some(90.0))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::joint::Joint;
use super::tokens::USD_PHYSICS_TOKENS;

/// Physics revolute joint schema.
///
/// Predefined revolute joint type (rotation along revolute joint
/// axis is permitted.)
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `axis`: Joint axis (X, Y, or Z). Default: "X"
/// - `lowerLimit`: Lower angular limit in degrees. -inf means unlimited.
/// - `upperLimit`: Upper angular limit in degrees. inf means unlimited.
///
/// # Inheritance
///
/// Inherits from Joint - all base Joint attributes are available.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsRevoluteJoint` class.
#[derive(Debug, Clone)]
pub struct RevoluteJoint {
    /// The underlying joint.
    joint: Joint,
}

impl RevoluteJoint {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsRevoluteJoint";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a RevoluteJoint on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            joint: Joint::new(prim),
        }
    }

    /// Construct a RevoluteJoint from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RevoluteJoint holding the prim at `path` on `stage`.
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
    // Axis Attribute
    // =========================================================================

    /// Joint axis.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token physics:axis = "X"` |
    /// | C++ Type | TfToken |
    /// | Allowed Values | X, Y, Z |
    /// | Variability | Uniform |
    pub fn get_axis_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_axis.as_str())
    }

    /// Creates the axis attribute.
    pub fn create_axis_attr(&self, default_value: Option<Token>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("token"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_axis.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // LowerLimit Attribute
    // =========================================================================

    /// Lower limit. Units: degrees.
    ///
    /// -inf means not limited in negative direction.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:lowerLimit = -inf` |
    /// | C++ Type | float |
    pub fn get_lower_limit_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_lower_limit.as_str())
    }

    /// Creates the lowerLimit attribute.
    pub fn create_lower_limit_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_lower_limit.as_str(),
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
    // UpperLimit Attribute
    // =========================================================================

    /// Upper limit. Units: degrees.
    ///
    /// inf means not limited in positive direction.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:upperLimit = inf` |
    /// | C++ Type | float |
    pub fn get_upper_limit_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_upper_limit.as_str())
    }

    /// Creates the upperLimit attribute.
    pub fn create_upper_limit_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_upper_limit.as_str(),
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
            USD_PHYSICS_TOKENS.physics_axis.clone(),
            USD_PHYSICS_TOKENS.physics_lower_limit.clone(),
            USD_PHYSICS_TOKENS.physics_upper_limit.clone(),
        ]);

        names
    }
}

// ============================================================================
// Delegate to Joint for common functionality
// ============================================================================

impl std::ops::Deref for RevoluteJoint {
    type Target = Joint;

    fn deref(&self) -> &Self::Target {
        &self.joint
    }
}

impl RevoluteJoint {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.joint.get_prim()
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for RevoluteJoint {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RevoluteJoint> for Prim {
    fn from(joint: RevoluteJoint) -> Self {
        joint.joint.into()
    }
}

impl AsRef<Prim> for RevoluteJoint {
    fn as_ref(&self) -> &Prim {
        self.joint.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(RevoluteJoint::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RevoluteJoint::SCHEMA_TYPE_NAME, "PhysicsRevoluteJoint");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RevoluteJoint::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:axis"));
        assert!(names.iter().any(|n| n.get_text() == "physics:lowerLimit"));
        assert!(names.iter().any(|n| n.get_text() == "physics:upperLimit"));
    }
}
