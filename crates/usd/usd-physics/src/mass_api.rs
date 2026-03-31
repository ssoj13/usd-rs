//! Physics Mass API schema.
//!
//! Defines explicit mass properties (mass, density, inertia, etc.).
//! MassAPI can be applied to any object that has a PhysicsCollisionAPI or
//! a PhysicsRigidBodyAPI.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/massAPI.h` and `massAPI.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::MassAPI;
//!
//! // Apply mass properties to a prim
//! let mass = MassAPI::apply(&prim)?;
//! mass.create_mass_attr(Some(10.0))?;
//! mass.create_density_attr(Some(1000.0))?;
//! ```

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_gf::{Quatf, Vec3f};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics mass API schema.
///
/// Defines explicit mass properties (mass, density, inertia etc.).
/// MassAPI can be applied to any object that has a PhysicsCollisionAPI or
/// a PhysicsRigidBodyAPI.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Mass Precedence
///
/// When mass is specified, it takes precedence over density. For parent/child
/// relationships, parent mass overrides child mass.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsMassAPI` class.
#[derive(Debug, Clone)]
pub struct MassAPI {
    prim: Prim,
}

impl MassAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsMassAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a MassAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a MassAPI from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a MassAPI holding the prim at `path` on `stage`.
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
    // Mass Attribute
    // =========================================================================

    /// If non-zero, directly specifies the mass of the object.
    ///
    /// Note that any child prim can also have a mass when they apply massAPI.
    /// In this case, the precedence rule is 'parent mass overrides the child's'.
    /// Note if mass is 0.0 it is ignored. Units: mass.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:mass = 0` |
    /// | C++ Type | float |
    pub fn get_mass_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_mass.as_str())
    }

    /// Creates the mass attribute.
    pub fn create_mass_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_mass.as_str(),
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

    /// If non-zero, specifies the density of the object.
    ///
    /// Density indirectly results in setting mass via (mass = density x volume).
    /// Mass has precedence over density. Unlike mass, child's density overrides
    /// parent's density. Note if density is 0.0 it is ignored.
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
    // CenterOfMass Attribute
    // =========================================================================

    /// Center of mass in the prim's local space. Units: distance.
    ///
    /// Default value (-inf, -inf, -inf) indicates the center should be computed.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `point3f physics:centerOfMass = (-inf, -inf, -inf)` |
    /// | C++ Type | GfVec3f |
    pub fn get_center_of_mass_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_center_of_mass.as_str())
    }

    /// Creates the centerOfMass attribute.
    pub fn create_center_of_mass_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("point3f"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_center_of_mass.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // DiagonalInertia Attribute
    // =========================================================================

    /// If non-zero, specifies diagonalized inertia tensor along the
    /// principal axes. Note if diagonalInertia is (0.0, 0.0, 0.0) it is ignored.
    /// Units: mass*distance*distance.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float3 physics:diagonalInertia = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_diagonal_inertia_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_diagonal_inertia.as_str())
    }

    /// Creates the diagonalInertia attribute.
    pub fn create_diagonal_inertia_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float3"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_diagonal_inertia.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // PrincipalAxes Attribute
    // =========================================================================

    /// Orientation of the inertia tensor's principal axes in the
    /// prim's local space.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `quatf physics:principalAxes = (0, 0, 0, 0)` |
    /// | C++ Type | GfQuatf |
    pub fn get_principal_axes_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_principal_axes.as_str())
    }

    /// Creates the principalAxes attribute.
    pub fn create_principal_axes_attr(&self, default_value: Option<Quatf>) -> Option<Attribute> {
        let type_name =
            usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("quatf"));
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_principal_axes.as_str(),
            &type_name,
            false,
            Some(usd_core::attribute::Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), usd_sdf::TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_PHYSICS_TOKENS.physics_mass.clone(),
            USD_PHYSICS_TOKENS.physics_density.clone(),
            USD_PHYSICS_TOKENS.physics_center_of_mass.clone(),
            USD_PHYSICS_TOKENS.physics_diagonal_inertia.clone(),
            USD_PHYSICS_TOKENS.physics_principal_axes.clone(),
        ]
    }
}

impl MassAPI {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this mass API is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get mass value.
    pub fn get_mass(&self) -> Option<f32> {
        self.get_mass_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get density value.
    pub fn get_density(&self) -> Option<f32> {
        self.get_density_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<f32>().copied())
    }

    /// Get center of mass.
    pub fn get_center_of_mass(&self) -> Option<Vec3f> {
        self.get_center_of_mass_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
    }

    /// Get diagonal inertia.
    pub fn get_diagonal_inertia(&self) -> Option<Vec3f> {
        self.get_diagonal_inertia_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<Vec3f>().copied())
    }

    /// Get principal axes.
    pub fn get_principal_axes(&self) -> Option<Quatf> {
        self.get_principal_axes_attr()?
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<Quatf>().copied())
    }

    /// Check if mass is explicitly authored.
    pub fn has_mass(&self) -> bool {
        if let Some(attr) = self.get_mass_attr() {
            attr.has_authored_value()
        } else {
            false
        }
    }

    /// Check if density is explicitly authored.
    pub fn has_density(&self) -> bool {
        if let Some(attr) = self.get_density_attr() {
            attr.has_authored_value()
        } else {
            false
        }
    }

    /// Check if center of mass is explicitly authored.
    pub fn has_center_of_mass(&self) -> bool {
        if let Some(attr) = self.get_center_of_mass_attr() {
            attr.has_authored_value()
        } else {
            false
        }
    }

    /// Check if diagonal inertia is explicitly authored.
    pub fn has_diagonal_inertia(&self) -> bool {
        if let Some(attr) = self.get_diagonal_inertia_attr() {
            attr.has_authored_value()
        } else {
            false
        }
    }

    /// Check if principal axes are explicitly authored.
    pub fn has_principal_axes(&self) -> bool {
        if let Some(attr) = self.get_principal_axes_attr() {
            attr.has_authored_value()
        } else {
            false
        }
    }
}

// ============================================================================
// From implementations
// ============================================================================

impl From<Prim> for MassAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<MassAPI> for Prim {
    fn from(api: MassAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for MassAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(MassAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(MassAPI::SCHEMA_TYPE_NAME, "PhysicsMassAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = MassAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "physics:mass"));
        assert!(names.iter().any(|n| n.get_text() == "physics:density"));
        assert!(names.iter().any(|n| n.get_text() == "physics:centerOfMass"));
    }
}
