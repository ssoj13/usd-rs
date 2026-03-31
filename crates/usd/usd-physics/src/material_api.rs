//! Physics Material API schema.
//!
//! Adds simulation material properties to a Material. All collisions that
//! have a relationship to this material will have their collision response
//! defined through this material.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/materialAPI.h` and `materialAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::MaterialAPI;
//!
//! // Apply physics material properties to a UsdShade material
//! let phys_mat = MaterialAPI::apply(&material_prim)?;
//! phys_mat.create_dynamic_friction_attr(Some(0.5))?;
//! phys_mat.create_static_friction_attr(Some(0.6))?;
//! phys_mat.create_restitution_attr(Some(0.3))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics material API schema.
///
/// Adds simulation material properties to a Material. All collisions
/// that have a relationship to this material will have their collision
/// response defined through this material.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Friction and Restitution
///
/// - `dynamicFriction`: Friction during sliding contact
/// - `staticFriction`: Friction preventing motion from rest
/// - `restitution`: Bounciness (0 = no bounce, 1 = perfect bounce)
///
/// # C++ Reference
///
/// Port of `UsdPhysicsMaterialAPI` class.
#[derive(Debug, Clone)]
pub struct MaterialAPI {
    prim: Prim,
}

impl MaterialAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsMaterialAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a MaterialAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a MaterialAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a MaterialAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&Token::new(Self::SCHEMA_TYPE_NAME)) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&Token::new(Self::SCHEMA_TYPE_NAME))
    }

    /// Applies this single-apply API schema to the given prim.
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
    // DynamicFriction Attribute
    // =========================================================================

    /// Dynamic friction coefficient. Unitless.
    ///
    /// Applied when surfaces are sliding against each other.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:dynamicFriction = 0` |
    /// | C++ Type | float |
    pub fn get_dynamic_friction_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_dynamic_friction.as_str())
    }

    /// Creates the dynamicFriction attribute.
    pub fn create_dynamic_friction_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_dynamic_friction.as_str(),
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
    // StaticFriction Attribute
    // =========================================================================

    /// Static friction coefficient. Unitless.
    ///
    /// Applied when surfaces are in contact but not moving.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:staticFriction = 0` |
    /// | C++ Type | float |
    pub fn get_static_friction_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_static_friction.as_str())
    }

    /// Creates the staticFriction attribute.
    pub fn create_static_friction_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_static_friction.as_str(),
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
    // Restitution Attribute
    // =========================================================================

    /// Restitution coefficient. Unitless.
    ///
    /// Determines how much kinetic energy is preserved on collision.
    /// 0 = perfectly inelastic, 1 = perfectly elastic.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:restitution = 0` |
    /// | C++ Type | float |
    pub fn get_restitution_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_restitution.as_str())
    }

    /// Creates the restitution attribute.
    pub fn create_restitution_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_restitution.as_str(),
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
    // Density Attribute
    // =========================================================================

    /// If non-zero, defines the density of the material.
    ///
    /// This can be used for body mass computation, see PhysicsMassAPI.
    /// Note that if the density is 0.0 it is ignored.
    /// Units: mass/distance/distance/distance.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:density = 0` |
    /// | C++ Type | float |
    pub fn get_density_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_density.as_str())
    }

    /// Creates the density attribute.
    pub fn create_density_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_density.as_str(),
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

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_PHYSICS_TOKENS.physics_dynamic_friction.clone(),
            USD_PHYSICS_TOKENS.physics_static_friction.clone(),
            USD_PHYSICS_TOKENS.physics_restitution.clone(),
            USD_PHYSICS_TOKENS.physics_density.clone(),
        ]
    }
}

impl MaterialAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this material API is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get dynamic friction coefficient.
    pub fn get_dynamic_friction(&self) -> Option<f32> {
        self.get_dynamic_friction_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get static friction coefficient.
    pub fn get_static_friction(&self) -> Option<f32> {
        self.get_static_friction_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get restitution (bounciness) coefficient.
    pub fn get_restitution(&self) -> Option<f32> {
        self.get_restitution_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get density.
    pub fn get_density(&self) -> Option<f32> {
        self.get_density_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for MaterialAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<MaterialAPI> for Prim {
    fn from(api: MaterialAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for MaterialAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(MaterialAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(MaterialAPI::SCHEMA_TYPE_NAME, "PhysicsMaterialAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = MaterialAPI::get_schema_attribute_names(false);
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:dynamicFriction")
        );
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:staticFriction")
        );
        assert!(names.iter().any(|n| n.get_text() == "physics:restitution"));
    }
}
