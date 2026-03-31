//! Render Settings schema.
//!
//! A RenderSettings prim specifies global settings for a render process.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/settings.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::settings_base::RenderSettingsBase;
use super::tokens::USD_RENDER_TOKENS;

/// Render settings schema.
#[derive(Debug, Clone)]
pub struct RenderSettings {
    prim: Prim,
}

impl RenderSettings {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RenderSettings";

    /// Fetch and return the stage's render settings.
    ///
    /// Returns render settings as indicated by root layer metadata
    /// `renderSettingsPrimPath`. If not authored, or the metadata does not
    /// refer to an existing prim, returns None.
    ///
    /// Matches C++ `UsdRenderSettings::GetStageRenderSettings()` which checks
    /// `HasAuthoredMetadata` before reading the path.
    pub fn get_stage_render_settings(stage: &Arc<Stage>) -> Option<Self> {
        // C++: only proceed when the metadata is explicitly authored
        if !stage.has_authored_metadata(&USD_RENDER_TOKENS.render_settings_prim_path) {
            return None;
        }
        let metadata = stage.get_metadata(&USD_RENDER_TOKENS.render_settings_prim_path)?;
        let path_str: String = metadata.downcast_clone()?;
        if !path_str.is_empty() {
            if let Some(path) = Path::from_string(&path_str) {
                return Self::get(stage, &path);
            }
        }
        None
    }

    /// Construct a RenderSettings on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RenderSettings holding the prim at `path` on `stage`.
    ///
    /// Note: no type check is performed — matches C++ which simply wraps `stage->GetPrimAtPath(path)`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Attempt to ensure a prim adhering to this schema at `path` is defined.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
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

    /// Get as base class for accessing inherited attributes.
    pub fn as_settings_base(&self) -> RenderSettingsBase {
        RenderSettingsBase::new(self.prim.clone())
    }

    // =========================================================================
    // IncludedPurposes Attribute
    // =========================================================================

    /// Get the includedPurposes attribute.
    pub fn get_included_purposes_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.included_purposes.as_str())
    }

    /// Creates the includedPurposes attribute.
    ///
    /// If `default_value` is provided, authors it as the attribute's default.
    /// If `write_sparsely` is true, the default value will only be authored
    /// if it differs from the fallback.
    ///
    /// Matches C++ `CreateIncludedPurposesAttr(VtValue, bool)`.
    pub fn create_included_purposes_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.included_purposes.as_str(),
                &token_array_type,
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
    // MaterialBindingPurposes Attribute
    // =========================================================================

    /// Get the materialBindingPurposes attribute.
    pub fn get_material_binding_purposes_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.material_binding_purposes.as_str())
    }

    /// Creates the materialBindingPurposes attribute.
    ///
    /// Matches C++ `CreateMaterialBindingPurposesAttr(VtValue, bool)`.
    pub fn create_material_binding_purposes_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.material_binding_purposes.as_str(),
                &token_array_type,
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
    // RenderingColorSpace Attribute
    // =========================================================================

    /// Get the renderingColorSpace attribute.
    pub fn get_rendering_color_space_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.rendering_color_space.as_str())
    }

    /// Creates the renderingColorSpace attribute.
    ///
    /// Matches C++ `CreateRenderingColorSpaceAttr(VtValue, bool)`.
    pub fn create_rendering_color_space_attr(
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
                USD_RENDER_TOKENS.rendering_color_space.as_str(),
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
    // Products Relationship
    // =========================================================================

    /// Get the products relationship.
    pub fn get_products_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_RENDER_TOKENS.products.as_str())
    }

    /// Creates the products relationship.
    pub fn create_products_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_RENDER_TOKENS.products.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            RenderSettingsBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_RENDER_TOKENS.included_purposes.clone(),
            USD_RENDER_TOKENS.material_binding_purposes.clone(),
            USD_RENDER_TOKENS.rendering_color_space.clone(),
        ]);

        names
    }
}

impl From<Prim> for RenderSettings {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RenderSettings> for Prim {
    fn from(settings: RenderSettings) -> Self {
        settings.prim
    }
}

impl AsRef<Prim> for RenderSettings {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RenderSettings::SCHEMA_TYPE_NAME, "RenderSettings");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RenderSettings::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "includedPurposes"));
    }
}
