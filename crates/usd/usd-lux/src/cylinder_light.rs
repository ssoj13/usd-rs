//! UsdLuxCylinderLight - cylindrical area light source.
//!
//! This module provides [`CylinderLight`], a light that emits outward from
//! a cylinder.
//!
//! # Overview
//!
//! CylinderLight is a tubular area light, useful for simulating fluorescent
//! tubes, neon lights, or other elongated light sources. The cylinder does
//! not emit light from its flat end-caps.
//!
//! # Geometry
//!
//! - Shape: Cylinder (hollow tube, no end-caps)
//! - Centered at local origin
//! - Major axis along local X axis
//! - Emits outward from curved surface only
//! - Size controlled by `inputs:length` and `inputs:radius`
//!
//! # Line Light Optimization
//!
//! The `treatAsLine` attribute hints that renderers may treat this as a
//! zero-radius line light for performance. This is useful for renderers
//! that have optimized line light implementations.
//!
//! # Attributes
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:length` | float | 1.0 | Length along X axis |
//! | `inputs:radius` | float | 0.5 | Radius of the cylinder |
//! | `treatAsLine` | bool | false | Hint to treat as line light |
//!
//! Plus all inherited [`LightAPI`](super::light_api::LightAPI) attributes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/cylinderLight.h`

use super::boundable_light_base::BoundableLightBase;
use super::light_api::LightAPI;
use super::tokens::tokens;
use std::sync::Arc;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;

/// Light emitted outward from a cylinder.
///
/// The cylinder is centered at the origin with its major axis on the X axis.
/// It does not emit light from its flat end-caps.
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
///     -> UsdGeomBoundable
///       -> UsdLuxBoundableLightBase
///         -> UsdLuxCylinderLight
/// ```
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:length` | float | 1.0 |
/// | `inputs:radius` | float | 0.5 |
/// | `treatAsLine` | bool | false |
///
/// Matches C++ `UsdLuxCylinderLight`.
#[derive(Clone)]
pub struct CylinderLight {
    prim: Prim,
}

impl CylinderLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a CylinderLight on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a CylinderLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxCylinderLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid CylinderLight.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a CylinderLight at the given path on the stage.
    ///
    /// If a prim already exists at the path, it will be returned if it
    /// adheres to this schema. Otherwise, a new prim is created with
    /// the CylinderLight type.
    ///
    /// Matches C++ `UsdLuxCylinderLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().cylinder_light.as_str())
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
    /// Matches C++ `UsdLuxCylinderLight::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![
            tokens().inputs_length.clone(),
            tokens().inputs_radius.clone(),
            tokens().treat_as_line.clone(),
        ];
        if include_inherited {
            names.extend(BoundableLightBase::get_schema_attribute_names(true));
        }
        names
    }

    // =========================================================================
    // Base Class Accessors
    // =========================================================================

    /// Returns this light as a BoundableLightBase.
    ///
    /// Provides access to all BoundableLightBase methods.
    #[inline]
    pub fn as_boundable_light_base(&self) -> BoundableLightBase {
        BoundableLightBase::new(self.prim.clone())
    }

    /// Returns the LightAPI for this light.
    ///
    /// Provides access to all common light attributes.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // =========================================================================
    // LENGTH Attribute
    // =========================================================================

    /// Returns the length attribute.
    ///
    /// Length of the cylinder, in the local X axis.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:length = 1` |
    /// | C++ Type | float |
    /// | Default | 1.0 |
    ///
    /// Matches C++ `UsdLuxCylinderLight::GetLengthAttr()`.
    #[inline]
    pub fn get_length_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_length.as_str())
    }

    /// Creates the length attribute.
    ///
    /// See [`get_length_attr`](Self::get_length_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxCylinderLight::CreateLengthAttr()`.
    pub fn create_length_attr(&self) -> Attribute {
        self.get_length_attr().unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // RADIUS Attribute
    // =========================================================================

    /// Returns the radius attribute.
    ///
    /// Radius of the cylinder.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:radius = 0.5` |
    /// | C++ Type | float |
    /// | Default | 0.5 |
    ///
    /// Matches C++ `UsdLuxCylinderLight::GetRadiusAttr()`.
    #[inline]
    pub fn get_radius_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_radius.as_str())
    }

    /// Creates the radius attribute.
    ///
    /// See [`get_radius_attr`](Self::get_radius_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxCylinderLight::CreateRadiusAttr()`.
    pub fn create_radius_attr(&self) -> Attribute {
        self.get_radius_attr().unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // TREATASLINE Attribute
    // =========================================================================

    /// Returns the treatAsLine attribute.
    ///
    /// A hint that this light can be treated as a 'line' light (effectively,
    /// a zero-radius cylinder) by renderers that benefit from non-area lighting.
    /// Renderers that only support area lights can disregard this.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool treatAsLine = 0` |
    /// | C++ Type | bool |
    /// | Default | false |
    ///
    /// Matches C++ `UsdLuxCylinderLight::GetTreatAsLineAttr()`.
    #[inline]
    pub fn get_treat_as_line_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().treat_as_line.as_str())
    }

    /// Creates the treatAsLine attribute.
    ///
    /// See [`get_treat_as_line_attr`](Self::get_treat_as_line_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxCylinderLight::CreateTreatAsLineAttr()`.
    pub fn create_treat_as_line_attr(&self) -> Attribute {
        self.get_treat_as_line_attr()
            .unwrap_or_else(Attribute::invalid)
    }
}

impl Default for CylinderLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_cylinder_light() {
        let light = CylinderLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = CylinderLight::get_schema_attribute_names(false);
        assert_eq!(names.len(), 3);

        let names_inherited = CylinderLight::get_schema_attribute_names(true);
        assert!(names_inherited.len() > 3);
    }
}
