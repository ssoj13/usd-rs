//! UsdLuxSphereLight - spherical area light source.
//!
//! This module provides [`SphereLight`], a light that emits illumination outward
//! from a spherical surface. Sphere lights are commonly used for:
//! - Soft area lighting with natural falloff
//! - Simulating light bulbs and other spherical emitters
//! - Creating soft shadows with controllable softness via radius
//!
//! The light intensity follows inverse-square falloff from the sphere surface.
//! Use the `treatAsPoint` attribute for renderers that optimize point lights.
//!
//! # USD Schema
//! - Schema type: `SphereLight`
//! - Parent: [`BoundableLightBase`](super::BoundableLightBase)
//! - Inherits: [`LightAPI`](super::LightAPI)
//!
//! # C++ Reference
//! Port of `pxr/usd/usdLux/sphereLight.h`

use super::boundable_light_base::BoundableLightBase;
use super::light_api::LightAPI;
use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use std::sync::Arc;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Light emitted outward from a sphere.
///
/// A sphere light emits light uniformly from all points on the surface of a sphere.
/// The sphere is centered at the light's position with configurable radius.
///
/// # Attributes
/// - `inputs:radius` - Radius of the sphere (default: 0.5)
/// - `treatAsPoint` - Hint for renderers to treat as point light (default: false)
///
/// # Inheritance
/// Inherits all attributes from [`BoundableLightBase`] and [`LightAPI`], including:
/// - `inputs:intensity`, `inputs:exposure`, `inputs:color`
/// - `inputs:diffuse`, `inputs:specular`
/// - Transform and visibility from parent schemas
///
/// # Example
/// ```ignore
/// let light = SphereLight::define(&stage, &Path::from_string("/World/Lights/Key"));
/// if let Some(attr) = light.get_radius_attr() {
///     attr.set(&Value::from(1.0f32), TimeCode::default());
/// }
/// ```
///
/// Matches C++ `UsdLuxSphereLight`.
#[derive(Clone)]
pub struct SphereLight {
    prim: Prim,
}

impl SphereLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a SphereLight schema on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    ///
    /// # Note
    /// Does not validate that the prim is actually a SphereLight.
    /// Use [`is_valid`](Self::is_valid) to check validity.
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a SphereLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path`, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// # Arguments
    /// * `stage` - The stage to query
    /// * `path` - Path to the prim
    ///
    /// Matches C++ `UsdLuxSphereLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid SphereLight schema object.
    ///
    /// Useful as a sentinel value or for error cases.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a SphereLight at the given path on the stage.
    ///
    /// Creates the prim if it doesn't exist, or returns the existing prim
    /// if one is already defined at that path.
    ///
    /// # Arguments
    /// * `stage` - The stage to author on
    /// * `path` - Absolute prim path (must not contain variant selections)
    ///
    /// # Returns
    /// The defined SphereLight, or an invalid schema if definition failed.
    ///
    /// Matches C++ `UsdLuxSphereLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().sphere_light.as_str())
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this schema object is valid.
    ///
    /// A schema is valid if it wraps a valid prim.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    #[inline]
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns the prim path.
    pub fn get_path(&self) -> &Path {
        self.prim.path()
    }

    /// Returns names of all pre-declared attributes for this schema.
    ///
    /// # Arguments
    /// * `include_inherited` - If true, includes attributes from parent schemas
    ///
    /// Matches C++ `UsdLuxSphereLight::GetSchemaAttributeNames(includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![
            tokens().inputs_radius.clone(),
            tokens().treat_as_point.clone(),
        ];

        if include_inherited {
            // Add BoundableLightBase and LightAPI attributes
            names.extend(BoundableLightBase::get_schema_attribute_names(true));
        }

        names
    }

    // =========================================================================
    // Parent Schema Access
    // =========================================================================

    /// Returns this light as a BoundableLightBase schema.
    ///
    /// Provides access to the bounding box computation inherited from
    /// the boundable light base class.
    #[inline]
    pub fn as_boundable_light_base(&self) -> BoundableLightBase {
        BoundableLightBase::new(self.prim.clone())
    }

    /// Returns the LightAPI for this light.
    ///
    /// Provides access to common light attributes like intensity,
    /// color, exposure, etc.
    #[inline]
    pub fn get_light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // =========================================================================
    // RADIUS Attribute
    // =========================================================================

    /// Returns the radius attribute.
    ///
    /// The radius of the sphere in scene units. Larger radii produce
    /// softer shadows and more spread-out highlights.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:radius = 0.5` |
    /// | C++ Type | `float` |
    /// | Default | 0.5 |
    ///
    /// Matches C++ `UsdLuxSphereLight::GetRadiusAttr()`.
    pub fn get_radius_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_radius.as_str())
    }

    /// Creates the radius attribute if it doesn't exist.
    ///
    /// # Arguments
    /// * `default_value` - Optional default value to author
    ///
    /// # Returns
    /// The radius attribute (existing or newly created).
    ///
    /// Matches C++ `UsdLuxSphereLight::CreateRadiusAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_radius_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_radius.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // TREATASPOINT Attribute
    // =========================================================================

    /// Returns the treatAsPoint attribute.
    ///
    /// A hint that this light can be treated as a point light (zero-radius sphere)
    /// by renderers that benefit from non-area lighting optimizations.
    /// Renderers that only support area lights can disregard this.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool treatAsPoint = 0` |
    /// | C++ Type | `bool` |
    /// | Default | false |
    ///
    /// Matches C++ `UsdLuxSphereLight::GetTreatAsPointAttr()`.
    pub fn get_treat_as_point_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().treat_as_point.as_str())
    }

    /// Creates the treatAsPoint attribute if it doesn't exist.
    ///
    /// # Arguments
    /// * `default_value` - Optional default value to author
    ///
    /// # Returns
    /// The treatAsPoint attribute (existing or newly created).
    ///
    /// Matches C++ `UsdLuxSphereLight::CreateTreatAsPointAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_treat_as_point_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().treat_as_point.as_str(),
            "bool",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }
}

impl Default for SphereLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_sphere_light() {
        let light = SphereLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = SphereLight::get_schema_attribute_names(false);
        assert!(names.len() >= 2);
    }
}
