//! Physics Scene schema.
//!
//! Defines general physics simulation properties required for simulation.
//! A PhysicsScene provides gravity direction and magnitude settings.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/scene.h` and `scene.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::Scene;
//!
//! let scene = Scene::define(&stage, &path)?;
//! scene.create_gravity_direction_attr(Some(Vec3f::new(0.0, -1.0, 0.0)))?;
//! scene.create_gravity_magnitude_attr(Some(9.81))?;
//! ```

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage, Typed};
use usd_gf::Vec3f;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PHYSICS_TOKENS;

/// Physics scene schema.
///
/// General physics simulation properties, required for simulation.
/// Provides gravity settings and serves as the simulation context.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Gravity
///
/// - `gravityDirection`: Normalized direction vector in world space.
///   Zero vector means use negative upAxis.
/// - `gravityMagnitude`: Acceleration in distance/second/second.
///   Negative value means use earth gravity (9.81 m/s^2 adjusted for metersPerUnit).
///
/// # C++ Reference
///
/// Port of `UsdPhysicsScene` class.
#[derive(Debug, Clone)]
pub struct Scene {
    prim: Prim,
}

impl Scene {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PhysicsScene";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a Scene on the given prim.
    ///
    /// Equivalent to `Scene::get(prim.get_stage(), prim.get_path())`
    /// for a valid prim, but will not immediately throw an error for
    /// an invalid prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct a Scene from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a Scene holding the prim at `path` on `stage`.
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
            .define_prim(path.to_string(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // GravityDirection Attribute
    // =========================================================================

    /// Gravity direction vector in simulation world space.
    ///
    /// Will be normalized before use. A zero vector is a request to use
    /// the negative upAxis. Unitless.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `vector3f physics:gravityDirection = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    pub fn get_gravity_direction_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_gravity_direction.as_str())
    }

    /// Creates the gravityDirection attribute.
    ///
    /// See `get_gravity_direction_attr()` for attribute description.
    pub fn create_gravity_direction_attr(&self, default_value: Option<Vec3f>) -> Option<Attribute> {
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_gravity_direction.as_str(),
            &usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("vector3f")),
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from_no_hash(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // GravityMagnitude Attribute
    // =========================================================================

    /// Gravity acceleration magnitude in simulation world space.
    ///
    /// A negative value is a request to use a value equivalent to earth
    /// gravity regardless of the metersPerUnit scaling used by this scene.
    /// Units: distance/second/second.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float physics:gravityMagnitude = -inf` |
    /// | C++ Type | float |
    pub fn get_gravity_magnitude_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PHYSICS_TOKENS.physics_gravity_magnitude.as_str())
    }

    /// Creates the gravityMagnitude attribute.
    ///
    /// See `get_gravity_magnitude_attr()` for attribute description.
    pub fn create_gravity_magnitude_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let attr = self.prim.create_attribute(
            USD_PHYSICS_TOKENS.physics_gravity_magnitude.as_str(),
            &usd_sdf::ValueTypeRegistry::instance().find_type_by_token(&Token::new("float")),
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(Value::from(value), TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// If `include_inherited` is true, includes attributes from parent classes.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            Typed::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_PHYSICS_TOKENS.physics_gravity_direction.clone(),
            USD_PHYSICS_TOKENS.physics_gravity_magnitude.clone(),
        ]);

        names
    }
}

impl Scene {
    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Check if this scene is valid (has a valid prim).
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get gravity direction vector.
    ///
    /// Returns the normalized gravity direction in world space.
    /// Zero vector means use negative upAxis.
    pub fn get_gravity_direction(&self) -> Option<Vec3f> {
        let value = self
            .get_gravity_direction_attr()?
            .get(TimeCode::default())?;
        value.get::<Vec3f>().cloned()
    }

    /// Get gravity magnitude.
    ///
    /// Returns gravity acceleration in distance/second/second.
    /// Negative value means use earth gravity (9.81 m/s² adjusted for metersPerUnit).
    pub fn get_gravity_magnitude(&self) -> Option<f32> {
        let value = self
            .get_gravity_magnitude_attr()?
            .get(TimeCode::default())?;
        value.get::<f32>().copied()
    }

    /// Get computed gravity vector.
    ///
    /// Returns the full gravity vector (direction * magnitude) in world space.
    /// Handles default values: zero direction = negative upAxis, negative magnitude = earth gravity.
    pub fn compute_gravity(&self) -> Vec3f {
        use usd_geom::get_stage_up_axis;

        let mut direction = self.get_gravity_direction().unwrap_or_default();
        let mut magnitude = self.get_gravity_magnitude().unwrap_or(-f32::INFINITY);

        // Handle default direction (use negative upAxis)
        if direction.length() < 1e-6 {
            if let Some(stage) = self.prim.stage() {
                let up_axis = get_stage_up_axis(&stage);
                direction = match up_axis.as_str() {
                    "Y" => Vec3f::new(0.0, -1.0, 0.0),
                    "Z" => Vec3f::new(0.0, 0.0, -1.0),
                    _ => Vec3f::new(0.0, -1.0, 0.0),
                };
            } else {
                direction = Vec3f::new(0.0, -1.0, 0.0);
            }
        } else {
            direction = direction.normalized();
        }

        // Handle default magnitude (earth gravity)
        if magnitude.is_infinite() && magnitude.is_sign_negative() {
            if let Some(stage) = self.prim.stage() {
                use usd_geom::get_stage_meters_per_unit;
                let meters_per_unit = get_stage_meters_per_unit(&stage) as f32;
                // 9.81 m/s² converted to stage units
                magnitude = 9.81 / meters_per_unit;
            } else {
                magnitude = 9.81;
            }
        }

        direction * magnitude
    }
}

// ============================================================================
// From implementations for type conversions
// ============================================================================

impl From<Prim> for Scene {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Scene> for Prim {
    fn from(scene: Scene) -> Self {
        scene.prim
    }
}

impl AsRef<Prim> for Scene {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(Scene::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Scene::SCHEMA_TYPE_NAME, "PhysicsScene");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = Scene::get_schema_attribute_names(false);
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:gravityDirection")
        );
        assert!(
            names
                .iter()
                .any(|n| n.get_text() == "physics:gravityMagnitude")
        );
    }
}
