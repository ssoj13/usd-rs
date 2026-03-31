//! Light Filter schema.
//!
//! A light filter modifies the effect of a light. Lights refer to filters
//! via relationships so that filters may be shared.
//!
//! # Linking
//!
//! Filters can be linked to geometry via UsdCollectionAPI. Linking controls
//! which geometry a light-filter affects, when considering the light filters
//! attached to a light illuminating the geometry.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/lightFilter.h` and `lightFilter.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::collection_api::CollectionAPI;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_geom::{XformQuery, Xformable};

use usd_sdf::{Path, TimeCode, ValueTypeName, ValueTypeRegistry};
use usd_shade::{ConnectableAPI, Input, Output};
use usd_tf::Token;

use super::tokens::tokens;

/// A light filter modifies the effect of a light.
///
/// Lights refer to filters via relationships so that filters may be shared.
/// Filters can be linked to geometry via `GetFilterLinkCollectionAPI()`.
///
/// Light filter parameters are encoded as inputs in the "inputs:" namespace.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema.
#[derive(Debug, Clone)]
pub struct LightFilter {
    xformable: Xformable,
}

impl LightFilter {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "LightFilter";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a LightFilter on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            xformable: Xformable::new(prim),
        }
    }

    /// Construct from Xformable.
    pub fn from_xformable(xformable: Xformable) -> Self {
        Self { xformable }
    }

    /// Create an invalid LightFilter.
    pub fn invalid() -> Self {
        Self {
            xformable: Xformable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.xformable.is_valid()
    }

    /// Get the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.xformable.prim()
    }

    /// Get as Xformable.
    pub fn xformable(&self) -> &Xformable {
        &self.xformable
    }

    /// Get XformQuery for efficient transform computation.
    pub fn xform_query(&self) -> XformQuery {
        XformQuery::new()
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Return a LightFilter holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxLightFilter::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a LightFilter at `path` on `stage`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // ShaderId Attribute
    // =========================================================================

    /// Get the lightFilter:shaderId attribute.
    ///
    /// Default ID for the light filter's shader. This defines the shader ID
    /// for this light filter when a render context specific shader ID is
    /// not available.
    pub fn get_shader_id_attr(&self) -> Option<Attribute> {
        self.prim()
            .get_attribute(tokens().light_filter_shader_id.as_str())
    }

    /// Create the lightFilter:shaderId attribute.
    pub fn create_shader_id_attr(&self, default_value: Option<Token>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self.prim().create_attribute(
            tokens().light_filter_shader_id.as_str(),
            &token_type,
            false,
            Some(Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(value, TimeCode::default());
        }

        Some(attr)
    }

    /// Returns the shader ID attribute for the given render context.
    ///
    /// If `render_context` is non-empty, returns an attribute named
    /// `{render_context}:lightFilter:shaderId`. Otherwise returns the
    /// default shader ID attribute.
    pub fn get_shader_id_attr_for_render_context(
        &self,
        render_context: &Token,
    ) -> Option<Attribute> {
        if render_context.as_str().is_empty() {
            return self.get_shader_id_attr();
        }

        let attr_name = format!(
            "{}:{}",
            render_context.as_str(),
            tokens().light_filter_shader_id.as_str()
        );
        self.prim().get_attribute(&attr_name)
    }

    /// Return the light filter's shader ID for the given list of render contexts.
    ///
    /// Contexts are expected in priority order. Returns the value from the first
    /// context with a non-empty shader ID, or the default shader ID if none found.
    pub fn get_shader_id(&self, render_contexts: &[Token]) -> Token {
        for context in render_contexts {
            if let Some(attr) = self.get_shader_id_attr_for_render_context(context) {
                if let Some(id) = attr.get_typed::<Token>(TimeCode::default()) {
                    if !id.as_str().is_empty() {
                        return id;
                    }
                }
            }
        }

        // Fall back to default shader ID
        if let Some(attr) = self.get_shader_id_attr() {
            attr.get_typed::<Token>(TimeCode::default())
                .unwrap_or_else(|| Token::new(""))
        } else {
            Token::new("")
        }
    }

    // =========================================================================
    // ConnectableAPI
    // =========================================================================

    /// Constructs a LightFilter from a ConnectableAPI.
    ///
    /// Matches C++ `UsdLuxLightFilter(const UsdShadeConnectableAPI &connectable)`.
    pub fn from_connectable(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim())
    }

    /// Returns a UsdShadeConnectableAPI for this light filter.
    ///
    /// Matches C++ `UsdLuxLightFilter::ConnectableAPI()`.
    pub fn connectable_api(&self) -> ConnectableAPI {
        ConnectableAPI::new(self.prim().clone())
    }

    // =========================================================================
    // Outputs API
    // =========================================================================

    /// Create an output which can either have a value or be connected.
    /// The attribute is created in the "outputs:" namespace.
    ///
    /// Matches C++ `UsdLuxLightFilter::CreateOutput()`.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Option<Output> {
        Output::new(self.prim(), name, type_name)
    }

    /// Return the requested output if it exists.
    ///
    /// Matches C++ `UsdLuxLightFilter::GetOutput()`.
    pub fn get_output(&self, name: &Token) -> Option<Output> {
        let attr_name = format!("outputs:{}", name.as_str());
        if self.prim().get_attribute(&attr_name).is_some() {
            Output::new(self.prim(), name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all outputs on this light filter.
    ///
    /// Matches C++ `UsdLuxLightFilter::GetOutputs()`.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    // =========================================================================
    // Inputs API
    // =========================================================================

    /// Create an input which can either have a value or be connected.
    /// The attribute is created in the "inputs:" namespace. Inputs on
    /// light filters are connectable.
    ///
    /// Matches C++ `UsdLuxLightFilter::CreateInput()`.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Option<Input> {
        Input::new(self.prim(), name, type_name)
    }

    /// Return the requested input if it exists.
    ///
    /// Matches C++ `UsdLuxLightFilter::GetInput()`.
    pub fn get_input(&self, name: &Token) -> Option<Input> {
        let attr_name = format!("inputs:{}", name.as_str());
        if self.prim().get_attribute(&attr_name).is_some() {
            Input::new(self.prim(), name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all inputs on this light filter.
    ///
    /// Matches C++ `UsdLuxLightFilter::GetInputs()`.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }

    // =========================================================================
    // Render Context Shader ID (create)
    // =========================================================================

    /// Creates the shader ID attribute for the given render context.
    ///
    /// Matches C++ `UsdLuxLightFilter::CreateShaderIdAttrForRenderContext()`.
    pub fn create_shader_id_attr_for_render_context(
        &self,
        render_context: &Token,
        default_value: Option<Token>,
        _write_sparsely: bool,
    ) -> Option<Attribute> {
        if render_context.as_str().is_empty() {
            return self.create_shader_id_attr(default_value);
        }

        let attr_name = format!(
            "{}:{}",
            render_context.as_str(),
            tokens().light_filter_shader_id.as_str()
        );

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self.prim().create_attribute(
            &attr_name,
            &token_type,
            false,
            Some(Variability::Uniform),
        )?;

        if let Some(value) = default_value {
            attr.set(value, TimeCode::default());
        }

        Some(attr)
    }

    // =========================================================================
    // Collection API
    // =========================================================================

    /// Collection name for filter linking.
    pub const FILTER_LINK_COLLECTION_NAME: &'static str = "filterLink";

    /// Get the filter link collection name token.
    pub fn get_filter_link_collection_name() -> Token {
        Token::new(Self::FILTER_LINK_COLLECTION_NAME)
    }

    /// Returns the filter link CollectionAPI for this light filter.
    ///
    /// This collection controls which geometry is affected by this filter.
    /// Default includes all geometry (`includeRoot=true`).
    ///
    /// Matches C++ `UsdLuxLightFilter::GetFilterLinkCollectionAPI()`.
    pub fn get_filter_link_collection_api(&self) -> CollectionAPI {
        CollectionAPI::new_with_include_root_fallback(
            self.prim().clone(),
            Token::new(Self::FILTER_LINK_COLLECTION_NAME),
            true,
        )
    }

    /// Returns the prim path.
    ///
    /// Matches C++ `UsdLuxLightFilter::GetPath()`.
    #[inline]
    pub fn get_path(&self) -> &Path {
        self.prim().path()
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            Xformable::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.push(tokens().light_filter_shader_id.clone());

        names
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for LightFilter {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Xformable> for LightFilter {
    fn from(xformable: Xformable) -> Self {
        Self::from_xformable(xformable)
    }
}

impl From<LightFilter> for Prim {
    fn from(filter: LightFilter) -> Self {
        filter.xformable.prim().clone()
    }
}

impl AsRef<Prim> for LightFilter {
    fn as_ref(&self) -> &Prim {
        self.prim()
    }
}

impl AsRef<Xformable> for LightFilter {
    fn as_ref(&self) -> &Xformable {
        &self.xformable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(LightFilter::SCHEMA_TYPE_NAME, "LightFilter");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(LightFilter::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_collection_name() {
        assert_eq!(LightFilter::FILTER_LINK_COLLECTION_NAME, "filterLink");
    }
}
