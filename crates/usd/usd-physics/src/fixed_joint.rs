//! Physics Fixed Joint schema.
//!
//! Predefined fixed joint type where all degrees of freedom are removed.
//! This effectively welds two bodies together.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/fixedJoint.h` and `fixedJoint.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::FixedJoint;
//!
//! let joint = FixedJoint::define(&stage, &path)?;
//! joint.create_body0_rel()?.set_targets(&[body0_path])?;
//! joint.create_body1_rel()?.set_targets(&[body1_path])?;
//! ```

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::joint::Joint;

/// Physics fixed joint schema.
///
/// Predefined fixed joint type (All degrees of freedom are removed.)
/// This effectively creates a rigid connection between two bodies.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Inheritance
///
/// Inherits from Joint - all base Joint attributes (localPos0, localRot0,
/// localPos1, localRot1, body0, body1, etc.) are available.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsFixedJoint` class.
#[derive(Debug, Clone)]
pub struct FixedJoint {
    /// The underlying joint.
    joint: Joint,
}

impl FixedJoint {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsFixedJoint";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a FixedJoint on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            joint: Joint::new(prim),
        }
    }

    /// Construct a FixedJoint from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a FixedJoint holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path` on `stage`, or if the prim at that
    /// path does not adhere to this schema, return None.
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
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// FixedJoint adds no new attributes beyond those from Joint.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        // FixedJoint has no additional attributes beyond Joint
        if include_inherited {
            Joint::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// Delegate to Joint for common functionality
// ============================================================================

impl std::ops::Deref for FixedJoint {
    type Target = Joint;

    fn deref(&self) -> &Self::Target {
        &self.joint
    }
}

impl FixedJoint {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.joint.get_prim()
    }
}

// ============================================================================
// From implementations for type conversions
// ============================================================================

impl From<Prim> for FixedJoint {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<FixedJoint> for Prim {
    fn from(joint: FixedJoint) -> Self {
        joint.joint.into()
    }
}

impl AsRef<Prim> for FixedJoint {
    fn as_ref(&self) -> &Prim {
        self.joint.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(FixedJoint::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(FixedJoint::SCHEMA_TYPE_NAME, "PhysicsFixedJoint");
    }
}
