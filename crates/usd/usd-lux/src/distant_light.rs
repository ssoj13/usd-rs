//! UsdLuxDistantLight - directional light from a distant source.
//!
//! This module provides [`DistantLight`], a light that emits parallel rays
//! from a distant source along the -Z axis.
//!
//! # Overview
//!
//! DistantLight simulates light from an infinitely distant source, producing
//! parallel rays. This is commonly used for simulating sunlight or moonlight,
//! where the light source is so far away that all rays are effectively parallel.
//!
//! # Direction
//!
//! The light emits along the -Z axis in its local coordinate space. The light's
//! orientation is controlled by transforming the prim.
//!
//! # Angular Size
//!
//! The `inputs:angle` attribute controls the angular diameter of the light
//! source in degrees:
//! - The Sun is approximately 0.53 degrees as seen from Earth
//! - Higher values broaden the light and soften shadow edges
//! - An angle of 0 produces perfectly sharp shadows (point-like source)
//! - Values > 180 degrees emit from more than a hemisphere
//!
//! # Attributes
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:angle` | float | 0.53 | Angular diameter in degrees |
//!
//! Plus all inherited [`LightAPI`](super::light_api::LightAPI) attributes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/distantLight.h`

use super::light_api::LightAPI;
use super::nonboundable_light_base::NonboundableLightBase;
use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use std::sync::Arc;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Light emitted from a distant source along the -Z axis.
///
/// Also known as a directional light. All rays are parallel, simulating
/// an infinitely distant light source like the sun.
///
/// # Schema Type
///
/// This is a **concrete typed schema** (`UsdSchemaKind::ConcreteTyped`).
///
/// # Inheritance
///
/// ```text
/// UsdTyped
///   -> UsdGeomImageable
///     -> UsdGeomXformable
///       -> UsdLuxNonboundableLightBase
///         -> UsdLuxDistantLight
/// ```
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:angle` | float | 0.53 |
///
/// Matches C++ `UsdLuxDistantLight`.
#[derive(Clone)]
pub struct DistantLight {
    prim: Prim,
}

impl DistantLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a DistantLight on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a DistantLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxDistantLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid DistantLight.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a DistantLight at the given path on the stage.
    ///
    /// If a prim already exists at the path, it will be returned if it
    /// adheres to this schema. Otherwise, a new prim is created with
    /// the DistantLight type.
    ///
    /// Matches C++ `UsdLuxDistantLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().distant_light.as_str())
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this schema is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    #[inline]
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns names of all pre-declared attributes for this schema.
    ///
    /// # Arguments
    /// * `include_inherited` - If true, includes attributes from parent schemas
    ///
    /// Matches C++ `UsdLuxDistantLight::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![tokens().inputs_angle.clone()];
        if include_inherited {
            names.extend(NonboundableLightBase::get_schema_attribute_names(true));
        }
        names
    }

    // =========================================================================
    // Base Class Accessors
    // =========================================================================

    /// Returns this light as a NonboundableLightBase.
    ///
    /// Provides access to all NonboundableLightBase methods.
    #[inline]
    pub fn as_nonboundable_light_base(&self) -> NonboundableLightBase {
        NonboundableLightBase::new(self.prim.clone())
    }

    /// Returns the LightAPI for this light.
    ///
    /// Provides access to all common light attributes.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // =========================================================================
    // ANGLE Attribute
    // =========================================================================

    /// Returns the angle attribute.
    ///
    /// Angular diameter of the light in degrees.
    /// As an example, the Sun is approximately 0.53 degrees as seen from Earth.
    /// Higher values broaden the light and therefore soften shadow edges.
    ///
    /// This value is clamped to the range `0 <= angle < 360`. Note that
    /// angles > 180 emit from more than a hemispherical area.
    /// If angle is 0, this represents a perfectly parallel light source
    /// (infinitely sharp shadows).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:angle = 0.53` |
    /// | C++ Type | float |
    /// | Default | 0.53 |
    ///
    /// Matches C++ `UsdLuxDistantLight::GetAngleAttr()`.
    #[inline]
    pub fn get_angle_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_angle.as_str())
    }

    /// Creates the angle attribute.
    ///
    /// See [`get_angle_attr`](Self::get_angle_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDistantLight::CreateAngleAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_angle_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_angle.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }
}

impl Default for DistantLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_distant_light() {
        let light = DistantLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = DistantLight::get_schema_attribute_names(false);
        assert_eq!(names.len(), 1);

        let names_inherited = DistantLight::get_schema_attribute_names(true);
        assert!(names_inherited.len() > 1);
    }
}
