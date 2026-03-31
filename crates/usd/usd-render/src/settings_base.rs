//! Render Settings Base schema.
//!
//! Abstract base class that defines render settings that can be specified
//! on either a RenderSettings prim or a RenderProduct prim.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/settingsBase.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::typed::Typed;
use usd_core::{Attribute, Prim, Relationship, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_RENDER_TOKENS;

/// Abstract base class for render settings.
#[derive(Debug, Clone)]
pub struct RenderSettingsBase {
    prim: Prim,
}

impl RenderSettingsBase {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RenderSettingsBase";

    /// Compile-time constant for this schema's kind.
    /// C++: `static const UsdSchemaKind schemaKind = UsdSchemaKind::AbstractTyped;`
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::AbstractTyped;

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Construct a RenderSettingsBase on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RenderSettingsBase holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Resolution Attribute
    // =========================================================================

    /// Get the resolution attribute.
    pub fn get_resolution_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.resolution.as_str())
    }

    /// Creates the resolution attribute.
    ///
    /// Matches C++ `CreateResolutionAttr(VtValue, bool)`.
    pub fn create_resolution_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int2_type = registry.find_type_by_token(&Token::new("int2"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.resolution.as_str(),
                &int2_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // PixelAspectRatio Attribute
    // =========================================================================

    /// Get the pixelAspectRatio attribute.
    pub fn get_pixel_aspect_ratio_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.pixel_aspect_ratio.as_str())
    }

    /// Creates the pixelAspectRatio attribute.
    ///
    /// Matches C++ `CreatePixelAspectRatioAttr(VtValue, bool)`.
    pub fn create_pixel_aspect_ratio_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.pixel_aspect_ratio.as_str(),
                &float_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // AspectRatioConformPolicy Attribute
    // =========================================================================

    /// Get the aspectRatioConformPolicy attribute.
    pub fn get_aspect_ratio_conform_policy_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.aspect_ratio_conform_policy.as_str())
    }

    /// Creates the aspectRatioConformPolicy attribute.
    ///
    /// Matches C++ `CreateAspectRatioConformPolicyAttr(VtValue, bool)`.
    pub fn create_aspect_ratio_conform_policy_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.aspect_ratio_conform_policy.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // DataWindowNDC Attribute
    // =========================================================================

    /// Get the dataWindowNDC attribute.
    pub fn get_data_window_ndc_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.data_window_ndc.as_str())
    }

    /// Creates the dataWindowNDC attribute.
    ///
    /// Matches C++ `CreateDataWindowNDCAttr(VtValue, bool)`.
    pub fn create_data_window_ndc_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float4_type = registry.find_type_by_token(&Token::new("float4"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.data_window_ndc.as_str(),
                &float4_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // InstantaneousShutter Attribute (Deprecated)
    // =========================================================================

    /// Get the instantaneousShutter attribute (deprecated).
    pub fn get_instantaneous_shutter_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.instantaneous_shutter.as_str())
    }

    /// Creates the instantaneousShutter attribute.
    ///
    /// Matches C++ `CreateInstantaneousShutterAttr(VtValue, bool)`.
    pub fn create_instantaneous_shutter_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let bool_type = registry.find_type_by_token(&Token::new("bool"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.instantaneous_shutter.as_str(),
                &bool_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // DisableMotionBlur Attribute
    // =========================================================================

    /// Get the disableMotionBlur attribute.
    pub fn get_disable_motion_blur_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.disable_motion_blur.as_str())
    }

    /// Creates the disableMotionBlur attribute.
    ///
    /// Matches C++ `CreateDisableMotionBlurAttr(VtValue, bool)`.
    pub fn create_disable_motion_blur_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let bool_type = registry.find_type_by_token(&Token::new("bool"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.disable_motion_blur.as_str(),
                &bool_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // DisableDepthOfField Attribute
    // =========================================================================

    /// Get the disableDepthOfField attribute.
    pub fn get_disable_depth_of_field_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.disable_depth_of_field.as_str())
    }

    /// Creates the disableDepthOfField attribute.
    ///
    /// Matches C++ `CreateDisableDepthOfFieldAttr(VtValue, bool)`.
    pub fn create_disable_depth_of_field_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let bool_type = registry.find_type_by_token(&Token::new("bool"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.disable_depth_of_field.as_str(),
                &bool_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // Camera Relationship
    // =========================================================================

    /// Get the camera relationship.
    pub fn get_camera_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_RENDER_TOKENS.camera.as_str())
    }

    /// Creates the camera relationship.
    pub fn create_camera_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_RENDER_TOKENS.camera.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// When `include_inherited` is true, includes attributes from parent
    /// schema (UsdTyped). C++ concatenates with `UsdTyped::GetSchemaAttributeNames(true)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            USD_RENDER_TOKENS.resolution.clone(),
            USD_RENDER_TOKENS.pixel_aspect_ratio.clone(),
            USD_RENDER_TOKENS.aspect_ratio_conform_policy.clone(),
            USD_RENDER_TOKENS.data_window_ndc.clone(),
            USD_RENDER_TOKENS.instantaneous_shutter.clone(),
            USD_RENDER_TOKENS.disable_motion_blur.clone(),
            USD_RENDER_TOKENS.disable_depth_of_field.clone(),
        ];

        if include_inherited {
            let mut all_names = Typed::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl From<Prim> for RenderSettingsBase {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RenderSettingsBase> for Prim {
    fn from(settings: RenderSettingsBase) -> Self {
        settings.prim
    }
}

impl AsRef<Prim> for RenderSettingsBase {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RenderSettingsBase::SCHEMA_TYPE_NAME, "RenderSettingsBase");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(RenderSettingsBase::SCHEMA_KIND, SchemaKind::AbstractTyped);
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RenderSettingsBase::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "resolution"));
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn test_schema_attribute_names_inherited() {
        let local = RenderSettingsBase::get_schema_attribute_names(false);
        let inherited = RenderSettingsBase::get_schema_attribute_names(true);
        // Inherited should include at least all local names
        assert!(inherited.len() >= local.len());
        for name in &local {
            assert!(inherited.iter().any(|n| n.get_text() == name.get_text()));
        }
    }
}
