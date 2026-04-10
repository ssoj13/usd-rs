//! UsdLuxNonboundableLightBase - base class for non-boundable lights.
//!
//! This module provides [`NonboundableLightBase`], the abstract base class for
//! intrinsic lights that are not boundable (do not have a bounding box for
//! geometric extent computation).
//!
//! # Overview
//!
//! NonboundableLightBase serves as the parent class for lights like:
//! - [`DistantLight`](super::distant_light::DistantLight) - directional lights from infinity
//! - [`DomeLight`](super::dome_light::DomeLight) - environment/IBL lights
//!
//! Unlike [`BoundableLightBase`](super::boundable_light_base::BoundableLightBase),
//! these lights don't have finite geometric extent - they represent lighting
//! from infinitely distant or omnidirectional sources.
//!
//! # LightAPI Convenience Accessors
//!
//! This class provides direct accessors to all [`LightAPI`](super::light_api::LightAPI)
//! attributes, allowing derived light types to access light properties without
//! explicitly constructing a LightAPI object.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/nonboundableLightBase.h`

use super::light_api::LightAPI;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_gf::Vec3f;
use usd_sdf::Path;
use usd_vt::Value;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Base class for intrinsic lights that are not boundable.
///
/// The primary purpose of this class is to provide a direct API to the
/// functions provided by [`LightAPI`] for concrete derived light types
/// that don't have a finite geometric extent.
///
/// # Schema Type
///
/// This is an **abstract typed schema** (`UsdSchemaKind::AbstractTyped`).
/// It cannot be instantiated directly - use concrete derived types like
/// [`DistantLight`](super::distant_light::DistantLight) or
/// [`DomeLight`](super::dome_light::DomeLight).
///
/// # Inheritance
///
/// ```text
/// UsdTyped
///   -> UsdGeomImageable
///     -> UsdGeomXformable
///       -> UsdLuxNonboundableLightBase
///         -> UsdLuxDistantLight
///         -> UsdLuxDomeLight
/// ```
///
/// Matches C++ `UsdLuxNonboundableLightBase`.
#[derive(Clone)]
pub struct NonboundableLightBase {
    prim: Prim,
}

impl NonboundableLightBase {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a NonboundableLightBase on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a NonboundableLightBase holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxNonboundableLightBase::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid NonboundableLightBase schema object.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
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
    /// Matches C++ `UsdLuxNonboundableLightBase::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        // NonboundableLightBase has LightAPI as built-in, so return its attributes
        LightAPI::get_schema_attribute_names(include_inherited)
    }

    // =========================================================================
    // LightAPI Convenience Accessors
    // =========================================================================

    /// Constructs and returns a LightAPI object for this prim.
    ///
    /// Use this to access LightAPI-specific methods not exposed directly
    /// on this class.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // -------------------------------------------------------------------------
    // SHADERID
    // -------------------------------------------------------------------------

    /// Returns the shader ID attribute.
    ///
    /// See [`LightAPI::get_shader_id_attr`].
    #[inline]
    pub fn get_shader_id_attr(&self) -> Option<Attribute> {
        self.light_api().get_shader_id_attr()
    }

    /// Creates the shader ID attribute.
    ///
    /// See [`LightAPI::create_shader_id_attr`].
    pub fn create_shader_id_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_shader_id_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // MATERIALSYNCMODE
    // -------------------------------------------------------------------------

    /// Returns the material sync mode attribute.
    ///
    /// See [`LightAPI::get_material_sync_mode_attr`].
    #[inline]
    pub fn get_material_sync_mode_attr(&self) -> Option<Attribute> {
        self.light_api().get_material_sync_mode_attr()
    }

    /// Creates the material sync mode attribute.
    ///
    /// See [`LightAPI::create_material_sync_mode_attr`].
    pub fn create_material_sync_mode_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_material_sync_mode_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // INTENSITY
    // -------------------------------------------------------------------------

    /// Returns the intensity attribute.
    ///
    /// See [`LightAPI::get_intensity_attr`].
    #[inline]
    pub fn get_intensity_attr(&self) -> Option<Attribute> {
        self.light_api().get_intensity_attr()
    }

    /// Creates the intensity attribute.
    ///
    /// See [`LightAPI::create_intensity_attr`].
    pub fn create_intensity_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_intensity_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // EXPOSURE
    // -------------------------------------------------------------------------

    /// Returns the exposure attribute.
    ///
    /// See [`LightAPI::get_exposure_attr`].
    #[inline]
    pub fn get_exposure_attr(&self) -> Option<Attribute> {
        self.light_api().get_exposure_attr()
    }

    /// Creates the exposure attribute.
    ///
    /// See [`LightAPI::create_exposure_attr`].
    pub fn create_exposure_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_exposure_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // DIFFUSE
    // -------------------------------------------------------------------------

    /// Returns the diffuse attribute.
    ///
    /// See [`LightAPI::get_diffuse_attr`].
    #[inline]
    pub fn get_diffuse_attr(&self) -> Option<Attribute> {
        self.light_api().get_diffuse_attr()
    }

    /// Creates the diffuse attribute.
    ///
    /// See [`LightAPI::create_diffuse_attr`].
    pub fn create_diffuse_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_diffuse_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // SPECULAR
    // -------------------------------------------------------------------------

    /// Returns the specular attribute.
    ///
    /// See [`LightAPI::get_specular_attr`].
    #[inline]
    pub fn get_specular_attr(&self) -> Option<Attribute> {
        self.light_api().get_specular_attr()
    }

    /// Creates the specular attribute.
    ///
    /// See [`LightAPI::create_specular_attr`].
    pub fn create_specular_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_specular_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // NORMALIZE
    // -------------------------------------------------------------------------

    /// Returns the normalize attribute.
    ///
    /// See [`LightAPI::get_normalize_attr`].
    #[inline]
    pub fn get_normalize_attr(&self) -> Option<Attribute> {
        self.light_api().get_normalize_attr()
    }

    /// Creates the normalize attribute.
    ///
    /// See [`LightAPI::create_normalize_attr`].
    pub fn create_normalize_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_normalize_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // COLOR
    // -------------------------------------------------------------------------

    /// Returns the color attribute.
    ///
    /// See [`LightAPI::get_color_attr`].
    #[inline]
    pub fn get_color_attr(&self) -> Option<Attribute> {
        self.light_api().get_color_attr()
    }

    /// Creates the color attribute.
    ///
    /// See [`LightAPI::create_color_attr`].
    pub fn create_color_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_color_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // ENABLECOLORTEMPERATURE
    // -------------------------------------------------------------------------

    /// Returns the enable color temperature attribute.
    ///
    /// See [`LightAPI::get_enable_color_temperature_attr`].
    #[inline]
    pub fn get_enable_color_temperature_attr(&self) -> Option<Attribute> {
        self.light_api().get_enable_color_temperature_attr()
    }

    /// Creates the enable color temperature attribute.
    ///
    /// See [`LightAPI::create_enable_color_temperature_attr`].
    pub fn create_enable_color_temperature_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_enable_color_temperature_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // COLORTEMPERATURE
    // -------------------------------------------------------------------------

    /// Returns the color temperature attribute.
    ///
    /// See [`LightAPI::get_color_temperature_attr`].
    #[inline]
    pub fn get_color_temperature_attr(&self) -> Option<Attribute> {
        self.light_api().get_color_temperature_attr()
    }

    /// Creates the color temperature attribute.
    ///
    /// See [`LightAPI::create_color_temperature_attr`].
    pub fn create_color_temperature_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.light_api()
            .create_color_temperature_attr(default_value, write_sparsely)
    }

    // -------------------------------------------------------------------------
    // FILTERS
    // -------------------------------------------------------------------------

    /// Returns the filters relationship.
    ///
    /// See [`LightAPI::get_filters_rel`].
    #[inline]
    pub fn get_filters_rel(&self) -> Option<Relationship> {
        self.light_api().get_filters_rel()
    }

    /// Creates the filters relationship.
    ///
    /// See [`LightAPI::create_filters_rel`].
    pub fn create_filters_rel(&self) -> Option<Relationship> {
        self.light_api().create_filters_rel()
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

impl Default for NonboundableLightBase {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_nonboundable_light_base() {
        let light = NonboundableLightBase::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = NonboundableLightBase::get_schema_attribute_names(true);
        assert!(!names.is_empty());
    }
}
