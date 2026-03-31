//! Scene Graph Prim API schema.
//!
//! API schema for storing display information for prims in scene
//! graph views.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/sceneGraphPrimAPI.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_UI_TOKENS;

/// API schema for scene graph prim display information.
///
/// Provides attributes for customizing how prims appear in scene
/// graph views (display name, grouping, etc.).
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Attributes
///
/// - `ui:displayName` - Human-readable name
/// - `ui:displayGroup` - Grouping category
#[derive(Debug, Clone)]
pub struct SceneGraphPrimAPI {
    prim: Prim,
}

impl SceneGraphPrimAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "SceneGraphPrimAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a SceneGraphPrimAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a SceneGraphPrimAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_UI_TOKENS.scene_graph_prim_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&USD_UI_TOKENS.scene_graph_prim_api)
    }

    /// Applies this API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_UI_TOKENS.scene_graph_prim_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
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
    // DisplayName Attribute
    // =========================================================================

    /// Get the ui:displayName attribute.
    ///
    /// Human-readable name for the prim.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token ui:displayName` |
    /// | C++ Type | TfToken |
    pub fn get_display_name_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_display_name.as_str())
    }

    /// Creates the ui:displayName attribute.
    pub fn create_display_name_attr(
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
                USD_UI_TOKENS.ui_display_name.as_str(),
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
    // DisplayGroup Attribute
    // =========================================================================

    /// Get the ui:displayGroup attribute.
    ///
    /// Grouping category for the prim.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token ui:displayGroup` |
    /// | C++ Type | TfToken |
    pub fn get_display_group_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_display_group.as_str())
    }

    /// Creates the ui:displayGroup attribute.
    pub fn create_display_group_attr(
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
                USD_UI_TOKENS.ui_display_group.as_str(),
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
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_UI_TOKENS.ui_display_name.clone(),
            USD_UI_TOKENS.ui_display_group.clone(),
        ]
    }
}

impl From<Prim> for SceneGraphPrimAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<SceneGraphPrimAPI> for Prim {
    fn from(api: SceneGraphPrimAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for SceneGraphPrimAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(SceneGraphPrimAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(SceneGraphPrimAPI::SCHEMA_TYPE_NAME, "SceneGraphPrimAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = SceneGraphPrimAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "ui:displayName"));
        assert!(names.iter().any(|n| n == "ui:displayGroup"));
    }
}
