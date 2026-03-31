//! Physics Joint schema.
//!
//! A joint constrains the movement of rigid bodies. Joints can be created
//! between two rigid bodies or between one rigid body and the world.
//! By default, a joint primitive defines a D6 joint where all degrees of
//! freedom are free (three linear and three angular).
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/joint.h` and `joint.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::Joint;
//!
//! let joint = Joint::define(&stage, &path)?;
//! joint.create_body0_rel()?.set_targets(&[body0_path])?;
//! joint.create_body1_rel()?.set_targets(&[body1_path])?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, Relationship, SchemaKind, Stage};
use usd_geom::Imageable;
use usd_gf::{Quatf, Vec3f};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics joint schema.
///
/// A joint constrains the movement of rigid bodies. Joint can be
/// created between two rigid bodies or between one rigid body and world.
/// By default joint primitive defines a D6 joint where all degrees of
/// freedom are free. Three linear and three angular degrees of freedom.
///
/// Note that default behavior is to disable collision between jointed bodies.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsJoint` class.
#[derive(Debug, Clone)]
pub struct Joint {
    prim: Prim,
}

impl Joint {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsJoint";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a Joint on the given prim.
    ///
    /// Equivalent to `Joint::get(prim.get_stage(), prim.get_path())`
    /// for a valid prim, but will not immediately throw an error for
    /// an invalid prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a Joint from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a Joint holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path` on `stage`, or if the prim at that
    /// path does not adhere to this schema, return an invalid schema object.
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
    ///
    /// If a prim adhering to this schema at `path` is already defined,
    /// return that prim. Otherwise author a prim spec with specifier == def
    /// and this schema's prim type name.
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

    // =========================================================================
    // LocalPos0 Attribute
    // =========================================================================

    /// Relative position of the joint frame to body0's frame.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `point3f physics:localPos0 = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_local_pos0_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_local_pos0.as_str())
    }

    /// Creates the localPos0 attribute.
    pub fn create_local_pos0_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("point3f"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_local_pos0.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // LocalRot0 Attribute
    // =========================================================================

    /// Relative orientation of the joint frame to body0's frame.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `quatf physics:localRot0 = (1, 0, 0, 0)` |
    /// | C++ Type | GfQuatf |
    pub fn get_local_rot0_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_local_rot0.as_str())
    }

    /// Creates the localRot0 attribute.
    pub fn create_local_rot0_attr(&self, default_value: Option<Quatf>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("quatf"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_local_rot0.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // LocalPos1 Attribute
    // =========================================================================

    /// Relative position of the joint frame to body1's frame.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `point3f physics:localPos1 = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_local_pos1_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_local_pos1.as_str())
    }

    /// Creates the localPos1 attribute.
    pub fn create_local_pos1_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("point3f"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_local_pos1.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // LocalRot1 Attribute
    // =========================================================================

    /// Relative orientation of the joint frame to body1's frame.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `quatf physics:localRot1 = (1, 0, 0, 0)` |
    /// | C++ Type | GfQuatf |
    pub fn get_local_rot1_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_local_rot1.as_str())
    }

    /// Creates the localRot1 attribute.
    pub fn create_local_rot1_attr(&self, default_value: Option<Quatf>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("quatf"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_local_rot1.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // JointEnabled Attribute
    // =========================================================================

    /// Determines if the joint is enabled.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool physics:jointEnabled = 1` |
    /// | C++ Type | bool |
    pub fn get_joint_enabled_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_joint_enabled.as_str())
    }

    /// Creates the jointEnabled attribute.
    pub fn create_joint_enabled_attr(&self, default_value: Option<bool>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_joint_enabled.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // CollisionEnabled Attribute
    // =========================================================================

    /// Determines if the jointed subtrees should collide or not.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool physics:collisionEnabled = 0` |
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
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // ExcludeFromArticulation Attribute
    // =========================================================================

    /// Determines if the joint can be included in an Articulation.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform bool physics:excludeFromArticulation = 0` |
    /// | C++ Type | bool |
    /// | Variability | Uniform |
    pub fn get_exclude_from_articulation_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(
            USD_PHYSICS_TOKENS
                .physics_exclude_from_articulation
                .as_str(),
        )
    }

    /// Creates the excludeFromArticulation attribute.
    pub fn create_exclude_from_articulation_attr(
        &self,
        default_value: Option<bool>,
    ) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("bool"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS
                .physics_exclude_from_articulation
                .as_str(),
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
    // BreakForce Attribute
    // =========================================================================

    /// Joint break force. If set, joint is to break when this force
    /// limit is reached. (Used for linear DOFs.)
    ///
    /// Units: mass * distance / second / second
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:breakForce = inf` |
    /// | C++ Type | float |
    pub fn get_break_force_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_break_force.as_str())
    }

    /// Creates the breakForce attribute.
    pub fn create_break_force_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_break_force.as_str(),
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
    // BreakTorque Attribute
    // =========================================================================

    /// Joint break torque. If set, joint is to break when this torque
    /// limit is reached. (Used for angular DOFs.)
    ///
    /// Units: mass * distance * distance / second / second
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:breakTorque = inf` |
    /// | C++ Type | float |
    pub fn get_break_torque_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_break_torque.as_str())
    }

    /// Creates the breakTorque attribute.
    pub fn create_break_torque_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_break_torque.as_str(),
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
    // Body0 Relationship
    // =========================================================================

    /// Relationship to body0 (any UsdGeomXformable).
    ///
    /// If empty, the joint is connected to the world.
    pub fn get_body0_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_PHYSICS_TOKENS.physics_body0.as_str())
    }

    /// Creates the body0 relationship.
    pub fn create_body0_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_PHYSICS_TOKENS.physics_body0.as_str(), false)
    }

    // =========================================================================
    // Body1 Relationship
    // =========================================================================

    /// Relationship to body1 (any UsdGeomXformable).
    ///
    /// If empty, the joint is connected to the world.
    pub fn get_body1_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_PHYSICS_TOKENS.physics_body1.as_str())
    }

    /// Creates the body1 relationship.
    pub fn create_body1_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_PHYSICS_TOKENS.physics_body1.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            Imageable::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_PHYSICS_TOKENS.physics_local_pos0.clone(),
            USD_PHYSICS_TOKENS.physics_local_rot0.clone(),
            USD_PHYSICS_TOKENS.physics_local_pos1.clone(),
            USD_PHYSICS_TOKENS.physics_local_rot1.clone(),
            USD_PHYSICS_TOKENS.physics_joint_enabled.clone(),
            USD_PHYSICS_TOKENS.physics_collision_enabled.clone(),
            USD_PHYSICS_TOKENS.physics_exclude_from_articulation.clone(),
            USD_PHYSICS_TOKENS.physics_break_force.clone(),
            USD_PHYSICS_TOKENS.physics_break_torque.clone(),
        ]);

        names
    }
}

impl Joint {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this joint is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Check if joint is enabled.
    ///
    /// Returns true if jointEnabled attribute is true or not authored (defaults to true).
    pub fn is_enabled(&self) -> bool {
        if let Some(attr) = self.get_joint_enabled_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(true)
        } else {
            true
        }
    }

    /// Check if collision is enabled between jointed bodies.
    ///
    /// Returns true if collisionEnabled attribute is true (default is false).
    pub fn is_collision_enabled(&self) -> bool {
        if let Some(attr) = self.get_collision_enabled_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if joint is excluded from articulation.
    pub fn is_excluded_from_articulation(&self) -> bool {
        if let Some(attr) = self.get_exclude_from_articulation_attr() {
            attr.get(TimeCode::default())
                .and_then(|v| v.get::<bool>().copied())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Get body0 prim.
    ///
    /// Returns the first body connected by this joint, or None if not set.
    pub fn get_body0(&self) -> Option<Prim> {
        let rel = self.get_body0_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            None
        } else {
            self.prim.stage()?.get_prim_at_path(&targets[0])
        }
    }

    /// Get body1 prim.
    ///
    /// Returns the second body connected by this joint, or None if not set.
    pub fn get_body1(&self) -> Option<Prim> {
        let rel = self.get_body1_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            None
        } else {
            self.prim.stage()?.get_prim_at_path(&targets[0])
        }
    }

    /// Get break force limit.
    pub fn get_break_force(&self) -> Option<f32> {
        self.get_break_force_attr()?
            .get(TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get break torque limit.
    pub fn get_break_torque(&self) -> Option<f32> {
        self.get_break_torque_attr()?
            .get(TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }
}

// ============================================================================
// From implementations for type conversions
// ============================================================================

impl From<Prim> for Joint {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Joint> for Prim {
    fn from(joint: Joint) -> Self {
        joint.prim
    }
}

impl AsRef<Prim> for Joint {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(Joint::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Joint::SCHEMA_TYPE_NAME, "PhysicsJoint");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = Joint::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:localPos0"));
        assert!(names.iter().any(|n| n.get_text() == "physics:jointEnabled"));
    }
}
