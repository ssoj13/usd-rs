//! Physics Limit API schema.
//!
//! Multiple-apply API schema for joint limits. When applied to a joint,
//! restricts movement along a specified axis.
//!
//! Instance names define the degree of freedom: "transX", "transY", "transZ",
//! "rotX", "rotY", "rotZ", or "distance".
//!
//! Note: If low > high, motion along that axis is considered locked.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/limitAPI.h` and `limitAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::LimitAPI;
//! use usd_tf::Token;
//!
//! // Apply rotation limit to a revolute joint
//! let limit = LimitAPI::apply(&joint_prim, &Token::new("rotX"))?;
//! limit.create_low_attr(Some(-45.0))?;   // -45 degrees
//! limit.create_high_attr(Some(45.0))?;   // +45 degrees
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Physics limit API schema (multiple-apply).
///
/// Restricts joint movement along an axis. Applied using instance names
/// like "rotX", "transY", "distance".
///
/// If low > high, the axis is considered locked.
///
/// # Schema Kind
///
/// This is a multiple-apply API schema (MultipleApplyAPI).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsLimitAPI` class.
#[derive(Debug, Clone)]
pub struct LimitAPI {
    prim: Prim,
    /// Instance name (axis): transX, transY, transZ, rotX, rotY, rotZ, distance
    instance_name: Token,
}

impl LimitAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::MultipleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsLimitAPI";

    /// Namespace prefix for limit properties.
    pub const NAMESPACE_PREFIX: &'static str = "limit";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a LimitAPI on the given prim with instance name.
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

    /// Return a LimitAPI holding the prim at `path` with instance `name`.
    pub fn get(stage: &Arc<Stage>, path: &Path, name: &Token) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Self::get_from_prim(&prim, name)
    }

    /// Return a LimitAPI for the given prim and instance name.
    pub fn get_from_prim(prim: &Prim, name: &Token) -> Option<Self> {
        // Check if this instance is applied
        let api_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, name.get_text());
        if prim.has_api(&Token::new(&api_name)) {
            Some(Self::new(prim.clone(), name.clone()))
        } else {
            None
        }
    }

    /// Return all LimitAPI instances on the given prim.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let mut result = Vec::new();
        for schema_name in prim.get_applied_schemas() {
            let name_str = schema_name.get_text();
            if let Some(instance) = name_str.strip_prefix("PhysicsLimitAPI:") {
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
    /// Adds "PhysicsLimitAPI:name" to the apiSchemas metadata.
    pub fn apply(prim: &Prim, name: &Token) -> Option<Self> {
        if !Self::can_apply(prim, name) {
            return None;
        }
        let api_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, name.get_text());
        prim.add_applied_schema(&Token::new(&api_name));
        Some(Self::new(prim.clone(), name.clone()))
    }

    /// Returns the instance name (axis) for this limit.
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
        // Format: limit:<instance>:physics:<attr>
        Token::new(&format!(
            "{}:{}:physics:{}",
            Self::NAMESPACE_PREFIX,
            self.instance_name.get_text(),
            base_name
        ))
    }

    // =========================================================================
    // Low Attribute
    // =========================================================================

    /// Lower limit.
    ///
    /// Units: degrees (rotational) or distance (translational).
    /// -inf means no lower limit.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:low = -inf` |
    pub fn get_low_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(self.make_attr_name("low").as_str())
    }

    /// Creates the low attribute.
    pub fn create_low_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("low").as_str(),
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
    // High Attribute
    // =========================================================================

    /// Upper limit.
    ///
    /// Units: degrees (rotational) or distance (translational).
    /// inf means no upper limit.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:high = inf` |
    pub fn get_high_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(self.make_attr_name("high").as_str())
    }

    /// Creates the high attribute.
    pub fn create_high_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            self.make_attr_name("high").as_str(),
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
        vec![Token::new("physics:low"), Token::new("physics:high")]
    }

    /// Returns attribute names for a specific instance.
    pub fn get_schema_attribute_names_for_instance(
        _include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        let prefix = format!("limit:{}:physics:", instance_name.get_text());
        vec![
            Token::new(&format!("{}low", prefix)),
            Token::new(&format!("{}high", prefix)),
        ]
    }

    /// Checks if the given path is of a PhysicsLimitAPI schema.
    /// If so, extracts the instance name and returns Some(instance_name).
    ///
    /// Matches C++ `UsdPhysicsLimitAPI::IsPhysicsLimitAPIPath(path, name)`.
    pub fn is_physics_limit_api_path(path: &Path) -> Option<Token> {
        // Path format: <prim_path>.limit:<instance_name>
        let path_str = path.as_str();
        // Look for property part containing "limit:"
        if let Some(dot_pos) = path_str.rfind('.') {
            let prop_name = &path_str[dot_pos + 1..];
            if let Some(instance) = prop_name.strip_prefix("limit:") {
                if !instance.is_empty() && !instance.contains(':') {
                    return Some(Token::new(instance));
                }
            }
        }
        None
    }

    /// Checks if a property base name belongs to LimitAPI.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        matches!(base_name.get_text(), "physics:low" | "physics:high")
    }
}

impl LimitAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl AsRef<Prim> for LimitAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(LimitAPI::SCHEMA_KIND, SchemaKind::MultipleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(LimitAPI::SCHEMA_TYPE_NAME, "PhysicsLimitAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = LimitAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:low"));
        assert!(names.iter().any(|n| n.get_text() == "physics:high"));
    }

    #[test]
    fn test_is_schema_property_base_name() {
        assert!(LimitAPI::is_schema_property_base_name(&Token::new(
            "physics:low"
        )));
        assert!(!LimitAPI::is_schema_property_base_name(&Token::new(
            "physics:mass"
        )));
    }

    #[test]
    fn test_schema_attribute_names_for_instance() {
        let names = LimitAPI::get_schema_attribute_names_for_instance(false, &Token::new("rotX"));
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "limit:rotX:physics:low")
        );
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "limit:rotX:physics:high")
        );
    }
}
