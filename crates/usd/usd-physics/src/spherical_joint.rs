//! Physics Spherical Joint schema.
//!
//! Predefined spherical joint type that removes linear degrees of freedom.
//! A cone limit may restrict the motion in a given range, allowing two limit
//! values that create circular (when equal) or elliptic cone constraints.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/sphericalJoint.h` and `sphericalJoint.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::SphericalJoint;
//!
//! let joint = SphericalJoint::define(&stage, &path)?;
//! joint.create_axis_attr(Some("X".into()))?;
//! joint.create_cone_angle0_limit_attr(Some(45.0))?;
//! joint.create_cone_angle1_limit_attr(Some(45.0))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::joint::Joint;
use super::tokens::USD_PHYSICS_TOKENS;

/// Physics spherical joint schema.
///
/// Predefined spherical joint type (Removes linear degrees of
/// freedom, cone limit may restrict the motion in a given range.)
/// It allows two limit values, which when equal create a circular,
/// else an elliptic cone limit around the limit axis.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `axis`: Cone limit axis (X, Y, or Z). Default: "X"
/// - `coneAngle0Limit`: Cone limit toward next axis. -1 means unlimited.
/// - `coneAngle1Limit`: Cone limit toward second-to-next axis. -1 means unlimited.
///
/// # Inheritance
///
/// Inherits from Joint - all base Joint attributes are available.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsSphericalJoint` class.
#[derive(Debug, Clone)]
pub struct SphericalJoint {
    /// The underlying joint.
    joint: Joint,
}

impl SphericalJoint {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsSphericalJoint";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a SphericalJoint on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            joint: Joint::new(prim),
        }
    }

    /// Construct a SphericalJoint from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a SphericalJoint holding the prim at `path` on `stage`.
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

    /// Cone limit axis.
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
    // ConeAngle0Limit Attribute
    // =========================================================================

    /// Cone limit from the primary joint axis in the local0 frame
    /// toward the next axis.
    ///
    /// (Next axis of X is Y, and of Z is X.)
    /// A negative value means not limited. Units: degrees.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:coneAngle0Limit = -1` |
    /// | C++ Type | float |
    pub fn get_cone_angle0_limit_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_cone_angle0_limit.as_str())
    }

    /// Creates the coneAngle0Limit attribute.
    pub fn create_cone_angle0_limit_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_cone_angle0_limit.as_str(),
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
    // ConeAngle1Limit Attribute
    // =========================================================================

    /// Cone limit from the primary joint axis in the local0 frame
    /// toward the second to next axis.
    ///
    /// A negative value means not limited. Units: degrees.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:coneAngle1Limit = -1` |
    /// | C++ Type | float |
    pub fn get_cone_angle1_limit_attr(&self) -> Option<Attribute> {
        self.joint
            .get_prim()
            .get_attribute(USD_PHYSICS_TOKENS.physics_cone_angle1_limit.as_str())
    }

    /// Creates the coneAngle1Limit attribute.
    pub fn create_cone_angle1_limit_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.joint.get_prim().create_attribute(
            USD_PHYSICS_TOKENS.physics_cone_angle1_limit.as_str(),
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
            USD_PHYSICS_TOKENS.physics_cone_angle0_limit.clone(),
            USD_PHYSICS_TOKENS.physics_cone_angle1_limit.clone(),
        ]);

        names
    }
}

// ============================================================================
// Delegate to Joint for common functionality
// ============================================================================

impl std::ops::Deref for SphericalJoint {
    type Target = Joint;

    fn deref(&self) -> &Self::Target {
        &self.joint
    }
}

impl SphericalJoint {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.joint.get_prim()
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for SphericalJoint {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<SphericalJoint> for Prim {
    fn from(joint: SphericalJoint) -> Self {
        joint.joint.into()
    }
}

impl AsRef<Prim> for SphericalJoint {
    fn as_ref(&self) -> &Prim {
        self.joint.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(SphericalJoint::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(SphericalJoint::SCHEMA_TYPE_NAME, "PhysicsSphericalJoint");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = SphericalJoint::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:axis"));
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:coneAngle0Limit")
        );
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:coneAngle1Limit")
        );
    }
}
