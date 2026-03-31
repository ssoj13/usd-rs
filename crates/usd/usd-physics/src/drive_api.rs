//! Physics Drive API schema.
//!
//! Multiple-apply API schema for joint drives. When applied to a joint,
//! drives the joint towards target position/velocity using a damped spring.
//!
//! Instance names define the degree of freedom: "transX", "transY", "transZ",
//! "rotX", "rotY", "rotZ", or "linear" (prismatic) / "angular" (revolute).
//!
//! Force formula: F = stiffness * (targetPos - pos) + damping * (targetVel - vel)
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/driveAPI.h` and `driveAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::DriveAPI;
//! use usd_tf::Token;
//!
//! // Apply rotational drive to a revolute joint
//! let drive = DriveAPI::apply(&joint_prim, &Token::new("angular"))?;
//! drive.create_target_position_attr(Some(45.0))?;  // 45 degrees
//! drive.create_stiffness_attr(Some(1000.0))?;
//! drive.create_damping_attr(Some(100.0))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Physics drive API schema (multiple-apply).
///
/// Drives a joint towards a target position/velocity. Applied per axis
/// using instance names like "rotX", "transY", "angular", "linear".
///
/// Each drive is an implicit force-limited damped spring.
///
/// # Schema Kind
///
/// This is a multiple-apply API schema (MultipleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsDriveAPI` class.
#[derive(Debug, Clone)]
pub struct DriveAPI {
    prim: Prim,
    /// Instance name (axis): transX, transY, transZ, rotX, rotY, rotZ, linear, angular
    instance_name: Token,
}

impl DriveAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::MultipleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsDriveAPI";

    /// Namespace prefix for drive properties.
    pub const NAMESPACE_PREFIX: &'static str = "drive";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a DriveAPI on the given prim with instance name.
    pub fn new(prim: Prim, instance_name: Token) -> Self {
        Self {
            prim,
            instance_name,
        }
    }

    /// Construct from another prim with instance name.
    pub fn from_prim(prim: &Prim, instance_name: Token) -> Self {
        Self::new(prim.clone(), instance_name)
    }

    /// Return a DriveAPI holding the prim at `path` with instance `name`.
    pub fn get(stage: &Arc<Stage>, path: &Path, name: &Token) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Self::get_from_prim(&prim, name)
    }

    /// Return a DriveAPI for the given prim and instance name.
    pub fn get_from_prim(prim: &Prim, name: &Token) -> Option<Self> {
        // Check if this instance is applied
        let api_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, name.get_text());
        if prim.has_api(&Token::new(&api_name)) {
            Some(Self::new(prim.clone(), name.clone()))
        } else {
            None
        }
    }

    /// Return all DriveAPI instances on the given prim.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let mut result = Vec::new();
        for schema_name in prim.get_applied_schemas() {
            let name_str = schema_name.get_text();
            if let Some(instance) = name_str.strip_prefix("PhysicsDriveAPI:") {
                result.push(Self::new(prim.clone(), Token::new(instance)));
            }
        }
        result
    }

    /// Check if this API schema can be applied with the given instance name.
    pub fn can_apply(prim: &Prim, _name: &Token) -> bool {
        prim.is_valid()
    }

    /// Apply this API schema with the given instance name.
    ///
    /// Adds "PhysicsDriveAPI:name" to the apiSchemas metadata.
    pub fn apply(prim: &Prim, name: &Token) -> Option<Self> {
        if !Self::can_apply(prim, name) {
            return None;
        }
        let api_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, name.get_text());
        prim.add_applied_schema(&Token::new(&api_name));
        Some(Self::new(prim.clone(), name.clone()))
    }

    /// Returns the instance name (axis) for this drive.
    pub fn get_name(&self) -> &Token {
        &self.instance_name
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // Helper: Build namespaced attribute name
    // =========================================================================

    fn make_attr_name(&self, base_name: &str) -> Token {
        // Format: drive:<instance>:physics:<attr>
        Token::new(&format!(
            "{}:{}:physics:{}",
            Self::NAMESPACE_PREFIX,
            self.instance_name.get_text(),
            base_name
        ))
    }

    // =========================================================================
    // Type Attribute
    // =========================================================================

    /// Drive type: "force" or "acceleration".
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token physics:type = "force"` |
    /// | Allowed Values | force, acceleration |
    pub fn get_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("type").as_str())
    }

    /// Creates the type attribute.
    pub fn create_type_attr(&self, default_value: Option<Token>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("token"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("type").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // MaxForce Attribute
    // =========================================================================

    /// Maximum force that can be applied.
    ///
    /// Units: mass * distance / second^2 (linear) or mass * dist^2 / second^2 (angular)
    /// inf means unlimited. Must be non-negative.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:maxForce = inf` |
    pub fn get_max_force_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("maxForce").as_str())
    }

    /// Creates the maxForce attribute.
    pub fn create_max_force_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("maxForce").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // TargetPosition Attribute
    // =========================================================================

    /// Target position. Units: distance (linear) or degrees (angular).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:targetPosition = 0` |
    pub fn get_target_position_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("targetPosition").as_str())
    }

    /// Creates the targetPosition attribute.
    pub fn create_target_position_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("targetPosition").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // TargetVelocity Attribute
    // =========================================================================

    /// Target velocity. Units: distance/second (linear) or degrees/second (angular).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:targetVelocity = 0` |
    pub fn get_target_velocity_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("targetVelocity").as_str())
    }

    /// Creates the targetVelocity attribute.
    pub fn create_target_velocity_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("targetVelocity").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Damping Attribute
    // =========================================================================

    /// Damping coefficient.
    ///
    /// Units: mass/second (linear) or mass*dist^2/second/degrees (angular)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:damping = 0` |
    pub fn get_damping_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("damping").as_str())
    }

    /// Creates the damping attribute.
    pub fn create_damping_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("damping").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Stiffness Attribute
    // =========================================================================

    /// Spring stiffness.
    ///
    /// Units: mass/second^2 (linear) or mass*dist^2/degrees/second^2 (angular)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:stiffness = 0` |
    pub fn get_stiffness_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("stiffness").as_str())
    }

    /// Creates the stiffness attribute.
    pub fn create_stiffness_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("stiffness").as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns base attribute names (without namespace prefix).
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            Token::new("physics:type"),
            Token::new("physics:maxForce"),
            Token::new("physics:targetPosition"),
            Token::new("physics:targetVelocity"),
            Token::new("physics:damping"),
            Token::new("physics:stiffness"),
        ]
    }

    /// Returns attribute names for a specific instance.
    pub fn get_schema_attribute_names_for_instance(
        _include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        let prefix = format!("drive:{}:physics:", instance_name.get_text());
        vec![
            Token::new(&format!("{}type", prefix)),
            Token::new(&format!("{}maxForce", prefix)),
            Token::new(&format!("{}targetPosition", prefix)),
            Token::new(&format!("{}targetVelocity", prefix)),
            Token::new(&format!("{}damping", prefix)),
            Token::new(&format!("{}stiffness", prefix)),
        ]
    }

    /// Checks if the given path is of a PhysicsDriveAPI schema.
    /// If so, extracts the instance name and returns Some(instance_name).
    ///
    /// Matches C++ `UsdPhysicsDriveAPI::IsPhysicsDriveAPIPath(path, name)`.
    pub fn is_physics_drive_api_path(path: &Path) -> Option<Token> {
        // Path format: <prim_path>.drive:<instance_name>
        let path_str = path.as_str();
        // Look for property part containing "drive:"
        if let Some(dot_pos) = path_str.rfind('.') {
            let prop_name = &path_str[dot_pos + 1..];
            if let Some(instance) = prop_name.strip_prefix("drive:") {
                if !instance.is_empty() && !instance.contains(':') {
                    return Some(Token::new(instance));
                }
            }
        }
        None
    }

    /// Checks if a property base name belongs to DriveAPI.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        matches!(
            base_name.get_text(),
            "physics:type"
                | "physics:maxForce"
                | "physics:targetPosition"
                | "physics:targetVelocity"
                | "physics:damping"
                | "physics:stiffness"
        )
    }
}

impl DriveAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl AsRef<Prim> for DriveAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(DriveAPI::SCHEMA_KIND, SchemaKind::MultipleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(DriveAPI::SCHEMA_TYPE_NAME, "PhysicsDriveAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = DriveAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:stiffness"));
        assert!(names.iter().any(|n| n.get_text() == "physics:damping"));
    }

    #[test]
    fn test_is_schema_property_base_name() {
        assert!(DriveAPI::is_schema_property_base_name(&Token::new(
            "physics:stiffness"
        )));
        assert!(!DriveAPI::is_schema_property_base_name(&Token::new(
            "physics:mass"
        )));
    }
}
