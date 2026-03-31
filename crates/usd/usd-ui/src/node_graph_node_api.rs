//! Node Graph Node API schema.
//!
//! API schema for storing node positioning and display information
//! in node graph editors.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/nodeGraphNodeAPI.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_UI_TOKENS;

/// Expansion state for node graph nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionState {
    /// Node is fully expanded showing all ports.
    Open,
    /// Node is collapsed showing only the header.
    Closed,
    /// Node is minimized to a small icon.
    Minimized,
}

impl ExpansionState {
    /// Converts to token.
    pub fn to_token(&self) -> Token {
        match self {
            ExpansionState::Open => USD_UI_TOKENS.open.clone(),
            ExpansionState::Closed => USD_UI_TOKENS.closed.clone(),
            ExpansionState::Minimized => USD_UI_TOKENS.minimized.clone(),
        }
    }

    /// Parses from token.
    pub fn from_token(token: &Token) -> Option<Self> {
        match token.as_str() {
            "open" => Some(ExpansionState::Open),
            "closed" => Some(ExpansionState::Closed),
            "minimized" => Some(ExpansionState::Minimized),
            _ => None,
        }
    }
}

/// API schema for node graph node display information.
///
/// Provides attributes for positioning and styling nodes in visual
/// node graph editors.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Attributes
///
/// - `ui:nodegraph:node:pos` - Node position (float2)
/// - `ui:nodegraph:node:size` - Node size (float2)
/// - `ui:nodegraph:node:stackingOrder` - Z-order (int)
/// - `ui:nodegraph:node:displayColor` - Tint color (color3f)
/// - `ui:nodegraph:node:icon` - Icon asset path
/// - `ui:nodegraph:node:expansionState` - open/closed/minimized
/// - `ui:nodegraph:node:docURI` - Documentation URI
#[derive(Debug, Clone)]
pub struct NodeGraphNodeAPI {
    prim: Prim,
}

impl NodeGraphNodeAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "NodeGraphNodeAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a NodeGraphNodeAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a NodeGraphNodeAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_UI_TOKENS.node_graph_node_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&USD_UI_TOKENS.node_graph_node_api)
    }

    /// Applies this API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_UI_TOKENS.node_graph_node_api) {
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
    // Pos Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:pos attribute.
    ///
    /// Position relative to parent. X=horizontal, Y=vertical (Qt-style).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform float2 ui:nodegraph:node:pos` |
    /// | C++ Type | GfVec2f |
    pub fn get_pos_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_pos.as_str())
    }

    /// Creates the pos attribute.
    pub fn create_pos_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float2_type = registry.find_type_by_token(&Token::new("float2"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_pos.as_str(),
                &float2_type,
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
    // Size Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:size attribute.
    ///
    /// Optional size hint. X=width, Y=height.
    pub fn get_size_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_size.as_str())
    }

    /// Creates the size attribute.
    pub fn create_size_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float2_type = registry.find_type_by_token(&Token::new("float2"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_size.as_str(),
                &float2_type,
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
    // StackingOrder Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:stackingOrder attribute.
    ///
    /// Z-order hint. Lower values drawn below higher values.
    pub fn get_stacking_order_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_stacking_order.as_str())
    }

    /// Creates the stackingOrder attribute.
    pub fn create_stacking_order_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_stacking_order.as_str(),
                &int_type,
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
    // DisplayColor Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:displayColor attribute.
    ///
    /// Node tint color hint.
    pub fn get_display_color_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_display_color.as_str())
    }

    /// Creates the displayColor attribute.
    pub fn create_display_color_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let color3f_type = registry.find_type_by_token(&Token::new("color3f"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_display_color.as_str(),
                &color3f_type,
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
    // ExpansionState Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:expansionState attribute.
    ///
    /// Allowed values: open, closed, minimized.
    pub fn get_expansion_state_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_expansion_state.as_str())
    }

    /// Creates the expansionState attribute.
    pub fn create_expansion_state_attr(
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
                USD_UI_TOKENS.ui_nodegraph_node_expansion_state.as_str(),
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
    // Icon Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:icon attribute.
    ///
    /// Image to display on the node for visual classification.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform asset ui:nodegraph:node:icon` |
    /// | C++ Type | SdfAssetPath |
    pub fn get_icon_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_icon.as_str())
    }

    /// Creates the icon attribute.
    pub fn create_icon_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let asset_type = registry.find_type_by_token(&Token::new("asset"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_icon.as_str(),
                &asset_type,
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
    // DocURI Attribute
    // =========================================================================

    /// Get the ui:nodegraph:node:docURI attribute.
    ///
    /// URI pointing to detailed documentation for this node.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform string ui:nodegraph:node:docURI` |
    /// | C++ Type | std::string |
    pub fn get_doc_uri_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_nodegraph_node_doc_uri.as_str())
    }

    /// Creates the docURI attribute.
    pub fn create_doc_uri_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let string_type = registry.find_type_by_token(&Token::new("string"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_nodegraph_node_doc_uri.as_str(),
                &string_type,
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
            USD_UI_TOKENS.ui_nodegraph_node_pos.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_size.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_stacking_order.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_display_color.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_icon.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_expansion_state.clone(),
            USD_UI_TOKENS.ui_nodegraph_node_doc_uri.clone(),
        ]
    }
}

impl From<Prim> for NodeGraphNodeAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<NodeGraphNodeAPI> for Prim {
    fn from(api: NodeGraphNodeAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for NodeGraphNodeAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(NodeGraphNodeAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(NodeGraphNodeAPI::SCHEMA_TYPE_NAME, "NodeGraphNodeAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = NodeGraphNodeAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "ui:nodegraph:node:pos"));
        assert!(names.iter().any(|n| n == "ui:nodegraph:node:size"));
    }
}
