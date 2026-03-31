//! UsdLuxBoundableLightBase - abstract base class for boundable lights.
//!
//! This module provides [`BoundableLightBase`], the base class for all lights
//! that have intrinsic bounds (e.g., sphere, disk, rect, cylinder lights).
//!
//! # Purpose
//! BoundableLightBase serves two primary purposes:
//! 1. Inherits from `UsdGeomBoundable` to provide bounding box computation
//! 2. Provides convenience accessors to [`LightAPI`] attributes
//!
//! # Schema Hierarchy
//! ```text
//! UsdTyped
//!   └─ UsdGeomImageable
//!        └─ UsdGeomBoundable
//!             └─ UsdLuxBoundableLightBase  <-- This class
//!                  ├─ UsdLuxSphereLight
//!                  ├─ UsdLuxDiskLight
//!                  ├─ UsdLuxRectLight
//!                  ├─ UsdLuxCylinderLight
//!                  └─ ...
//! ```
//!
//! # C++ Reference
//! Port of `pxr/usd/usdLux/boundableLightBase.h`

use super::light_api::LightAPI;
use super::tokens::tokens;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_gf::Vec3f;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Base class for intrinsic lights that are boundable.
///
/// BoundableLightBase provides a direct API to the functions provided by
/// [`LightAPI`] for concrete derived light types. This allows accessing
/// light attributes directly without explicitly constructing a LightAPI.
///
/// # Abstract Schema
/// This is an abstract typed schema - you cannot directly define a
/// BoundableLightBase prim. Use concrete derived types like [`SphereLight`],
/// [`DiskLight`], etc.
///
/// # LightAPI Convenience Accessors
/// All Get/Create methods for intensity, color, exposure, etc. are
/// delegated to the underlying LightAPI.
///
/// Matches C++ `UsdLuxBoundableLightBase`.
#[derive(Clone)]
pub struct BoundableLightBase {
    prim: Prim,
}

impl BoundableLightBase {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a BoundableLightBase schema on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a BoundableLightBase holding the prim at `path` on `stage`.
    ///
    /// # Arguments
    /// * `stage` - The stage to query
    /// * `path` - Path to the prim
    ///
    /// Matches C++ `UsdLuxBoundableLightBase::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid BoundableLightBase schema object.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this schema object is valid.
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
    /// Matches C++ `UsdLuxBoundableLightBase::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        // BoundableLightBase itself doesn't add new attributes,
        // but includes all LightAPI attributes
        if include_inherited {
            LightAPI::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }

    // =========================================================================
    // LightAPI - Convenience Accessors
    // =========================================================================

    /// Returns the UsdLuxLightAPI for this light.
    ///
    /// Use this to access LightAPI-specific functionality.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // -------------------------------------------------------------------------
    // Shader ID
    // -------------------------------------------------------------------------

    /// Returns the shader ID attribute.
    ///
    /// See [`LightAPI::get_shader_id_attr`].
    #[inline]
    pub fn get_shader_id_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().light_shader_id.as_str())
    }

    /// Creates the shader ID attribute.
    pub fn create_shader_id_attr(&self) -> Attribute {
        self.get_shader_id_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Material Sync Mode
    // -------------------------------------------------------------------------

    /// Returns the material sync mode attribute.
    ///
    /// See [`LightAPI::get_material_sync_mode_attr`].
    #[inline]
    pub fn get_material_sync_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().light_material_sync_mode.as_str())
    }

    /// Creates the material sync mode attribute.
    pub fn create_material_sync_mode_attr(&self) -> Attribute {
        self.get_material_sync_mode_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Intensity
    // -------------------------------------------------------------------------

    /// Returns the intensity attribute.
    ///
    /// Scales the brightness of the light linearly.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:intensity = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_intensity_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_intensity.as_str())
    }

    /// Creates the intensity attribute.
    pub fn create_intensity_attr(&self) -> Attribute {
        self.get_intensity_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Exposure
    // -------------------------------------------------------------------------

    /// Returns the exposure attribute.
    ///
    /// Scales the brightness of the light exponentially as a power of 2.
    /// Typical range: -10 to 10.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:exposure = 0` |
    /// | Default | 0.0 |
    #[inline]
    pub fn get_exposure_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_exposure.as_str())
    }

    /// Creates the exposure attribute.
    pub fn create_exposure_attr(&self) -> Attribute {
        self.get_exposure_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Diffuse
    // -------------------------------------------------------------------------

    /// Returns the diffuse attribute.
    ///
    /// Multiplier for the effect on diffuse shading.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:diffuse = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_diffuse_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_diffuse.as_str())
    }

    /// Creates the diffuse attribute.
    pub fn create_diffuse_attr(&self) -> Attribute {
        self.get_diffuse_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Specular
    // -------------------------------------------------------------------------

    /// Returns the specular attribute.
    ///
    /// Multiplier for the effect on specular shading.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:specular = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_specular_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_specular.as_str())
    }

    /// Creates the specular attribute.
    pub fn create_specular_attr(&self) -> Attribute {
        self.get_specular_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Normalize
    // -------------------------------------------------------------------------

    /// Returns the normalize attribute.
    ///
    /// Normalizes power by the surface area of the light.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:normalize = 0` |
    /// | Default | false |
    #[inline]
    pub fn get_normalize_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_normalize.as_str())
    }

    /// Creates the normalize attribute.
    pub fn create_normalize_attr(&self) -> Attribute {
        self.get_normalize_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Color
    // -------------------------------------------------------------------------

    /// Returns the color attribute.
    ///
    /// The color of the emitted light, in energy-linear terms.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `color3f inputs:color = (1, 1, 1)` |
    /// | Default | (1, 1, 1) |
    #[inline]
    pub fn get_color_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_color.as_str())
    }

    /// Creates the color attribute.
    pub fn create_color_attr(&self) -> Attribute {
        self.get_color_attr().unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Enable Color Temperature
    // -------------------------------------------------------------------------

    /// Returns the enable color temperature attribute.
    ///
    /// If true, uses color temperature to compute the color.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:enableColorTemperature = 0` |
    /// | Default | false |
    #[inline]
    pub fn get_enable_color_temperature_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_enable_color_temperature.as_str())
    }

    /// Creates the enable color temperature attribute.
    pub fn create_enable_color_temperature_attr(&self) -> Attribute {
        self.get_enable_color_temperature_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Color Temperature
    // -------------------------------------------------------------------------

    /// Returns the color temperature attribute.
    ///
    /// Color temperature in Kelvin. Typical range: 1000-10000K.
    /// Only used when enableColorTemperature is true.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:colorTemperature = 6500` |
    /// | Default | 6500 |
    #[inline]
    pub fn get_color_temperature_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_color_temperature.as_str())
    }

    /// Creates the color temperature attribute.
    pub fn create_color_temperature_attr(&self) -> Attribute {
        self.get_color_temperature_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // -------------------------------------------------------------------------
    // Filters Relationship
    // -------------------------------------------------------------------------

    /// Returns the filters relationship.
    ///
    /// Ordered list of light filters that affect this light.
    #[inline]
    pub fn get_filters_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().light_filters.as_str())
    }

    /// Creates the filters relationship.
    pub fn create_filters_rel(&self) -> Option<Relationship> {
        self.get_filters_rel()
    }

    // =========================================================================
    // Convenience Value Getters (delegate to LightAPI)
    // =========================================================================

    /// Returns the intensity value at the given time, or the schema default (1.0).
    #[inline]
    pub fn get_intensity(&self, time: TimeCode) -> f32 {
        self.light_api().get_intensity(time)
    }

    /// Returns the exposure value at the given time, or the schema default (0.0).
    #[inline]
    pub fn get_exposure(&self, time: TimeCode) -> f32 {
        self.light_api().get_exposure(time)
    }

    /// Returns the color value at the given time, or the schema default (1, 1, 1).
    #[inline]
    pub fn get_color(&self, time: TimeCode) -> Vec3f {
        self.light_api().get_color(time)
    }

    /// Returns the diffuse multiplier at the given time, or the schema default (1.0).
    #[inline]
    pub fn get_diffuse(&self, time: TimeCode) -> f32 {
        self.light_api().get_diffuse(time)
    }

    /// Returns the specular multiplier at the given time, or the schema default (1.0).
    #[inline]
    pub fn get_specular(&self, time: TimeCode) -> f32 {
        self.light_api().get_specular(time)
    }

    /// Returns whether power normalization is enabled at the given time, or the schema default (false).
    #[inline]
    pub fn get_normalize_power(&self, time: TimeCode) -> bool {
        self.light_api().get_normalize_power(time)
    }

    /// Returns the color temperature in Kelvin at the given time, or the schema default (6500.0).
    #[inline]
    pub fn get_color_temperature(&self, time: TimeCode) -> f32 {
        self.light_api().get_color_temperature(time)
    }

    /// Returns whether color temperature is enabled at the given time, or the schema default (false).
    #[inline]
    pub fn get_enable_color_temperature(&self, time: TimeCode) -> bool {
        self.light_api().get_enable_color_temperature(time)
    }
}

impl Default for BoundableLightBase {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_boundable_light_base() {
        let light = BoundableLightBase::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = BoundableLightBase::get_schema_attribute_names(true);
        // Should include LightAPI attributes
        assert!(!names.is_empty());
    }
}
