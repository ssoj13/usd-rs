//! UsdGeomVisibilityAPI - API schema for purpose-specific visibility.
//!
//! Port of pxr/usd/usdGeom/visibilityAPI.h/cpp
//!
//! UsdGeomVisibilityAPI introduces properties that can be used to author
//! visibility opinions for specific purposes (guide, proxy, render).

use super::schema_create_default::apply_optional_default;
use super::tokens::usd_geom_tokens;
use usd_core::{Attribute, Prim, SchemaBase, Stage};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// VisibilityAPI
// ============================================================================

/// API schema for purpose-specific visibility.
///
/// This schema introduces guideVisibility, proxyVisibility, and renderVisibility
/// attributes that control visibility for geometry with specific purposes.
///
/// Matches C++ `UsdGeomVisibilityAPI`.
#[derive(Debug, Clone)]
pub struct VisibilityAPI {
    /// Base schema.
    inner: SchemaBase,
}

impl VisibilityAPI {
    /// Creates a new VisibilityAPI from a prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: SchemaBase::new(prim),
        }
    }

    /// Creates an invalid VisibilityAPI.
    pub fn invalid() -> Self {
        Self {
            inner: SchemaBase::invalid(),
        }
    }

    /// Returns true if this VisibilityAPI is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the prim this schema is applied to.
    pub fn prim(&self) -> Prim {
        self.inner.prim().clone()
    }

    /// Returns the schema type name.
    ///
    /// Matches C++ `GetSchemaAttributeNames()` type identity.
    pub fn schema_type_name() -> Token {
        Token::new("VisibilityAPI")
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// Matches C++ `CanApply()`.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// Matches C++ `Apply()`.
    pub fn apply(prim: &Prim) -> Self {
        let schema_name = Self::schema_type_name();
        if prim.apply_api(&schema_name) {
            let api = Self::new(prim.clone());
            // Ensure purpose visibility attr specs exist in the layer so
            // get_*_visibility_attr() returns valid attributes and set() works.
            // C++ schema infrastructure creates these automatically.
            api.create_guide_visibility_attr(None, false);
            api.create_proxy_visibility_attr(None, false);
            api.create_render_visibility_attr(None, false);
            api
        } else {
            Self::invalid()
        }
    }

    /// Returns a VisibilityAPI wrapping the prim at `path` on `stage`.
    ///
    /// Matches C++ `Get(stage, path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Returns the schema attribute names.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        let t = usd_geom_tokens();
        vec![
            t.guide_visibility.clone(),
            t.proxy_visibility.clone(),
            t.render_visibility.clone(),
        ]
    }

    // ========================================================================
    // GuideVisibility
    // ========================================================================

    /// Returns the guideVisibility attribute.
    ///
    /// Matches C++ `GetGuideVisibilityAttr()`.
    pub fn get_guide_visibility_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().guide_visibility.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the guideVisibility attribute.
    ///
    /// Matches C++ `CreateGuideVisibilityAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_guide_visibility_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().guide_visibility.as_str()) {
            prim.get_attribute(usd_geom_tokens().guide_visibility.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = usd_sdf::ValueTypeRegistry::instance();
            let token_type = registry.find_type_by_token(&Token::new("token"));

            prim.create_attribute(
                usd_geom_tokens().guide_visibility.as_str(),
                &token_type,
                false, // not custom
                Some(usd_core::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
        };

        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // ProxyVisibility
    // ========================================================================

    /// Returns the proxyVisibility attribute.
    ///
    /// Matches C++ `GetProxyVisibilityAttr()`.
    pub fn get_proxy_visibility_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().proxy_visibility.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the proxyVisibility attribute.
    ///
    /// Matches C++ `CreateProxyVisibilityAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_proxy_visibility_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().proxy_visibility.as_str()) {
            prim.get_attribute(usd_geom_tokens().proxy_visibility.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = usd_sdf::ValueTypeRegistry::instance();
            let token_type = registry.find_type_by_token(&Token::new("token"));

            prim.create_attribute(
                usd_geom_tokens().proxy_visibility.as_str(),
                &token_type,
                false,
                Some(usd_core::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
        };

        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // RenderVisibility
    // ========================================================================

    /// Returns the renderVisibility attribute.
    ///
    /// Matches C++ `GetRenderVisibilityAttr()`.
    pub fn get_render_visibility_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().render_visibility.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the renderVisibility attribute.
    ///
    /// Matches C++ `CreateRenderVisibilityAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_render_visibility_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().render_visibility.as_str()) {
            prim.get_attribute(usd_geom_tokens().render_visibility.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = usd_sdf::ValueTypeRegistry::instance();
            let token_type = registry.find_type_by_token(&Token::new("token"));

            prim.create_attribute(
                usd_geom_tokens().render_visibility.as_str(),
                &token_type,
                false,
                Some(usd_core::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
        };

        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Purpose Visibility
    // ========================================================================

    /// Return the attribute that is used for expressing visibility opinions
    /// for the given purpose.
    ///
    /// Matches C++ `GetPurposeVisibilityAttr()`.
    pub fn get_purpose_visibility_attr(&self, purpose: &Token) -> Attribute {
        if *purpose == usd_geom_tokens().guide {
            return self.get_guide_visibility_attr();
        }
        if *purpose == usd_geom_tokens().proxy {
            return self.get_proxy_visibility_attr();
        }
        if *purpose == usd_geom_tokens().render {
            return self.get_render_visibility_attr();
        }

        // Error: unexpected purpose
        Attribute::invalid()
    }
}
