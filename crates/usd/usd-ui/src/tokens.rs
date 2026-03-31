//! UsdUI tokens for UI schemas.
//!
//! These tokens are used for attribute names and allowed values
//! in the UsdUI schema module.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdUI schemas.
pub struct UsdUITokensType {
    // Expansion state values
    /// "closed" - Fully collapsed
    pub closed: Token,
    /// "minimized" - Least space possible
    pub minimized: Token,
    /// "open" - Fully expanded
    pub open: Token,

    // Priority values for AccessibilityAPI
    /// "low" - Low priority
    pub low: Token,
    /// "standard" - Standard priority (default)
    pub standard: Token,
    /// "high" - High priority
    pub high: Token,

    // AccessibilityAPI attribute base names
    /// "label" - Label attribute base name
    pub label: Token,
    /// "description" - Description attribute base name
    pub description: Token,
    /// "priority" - Priority attribute base name
    pub priority: Token,
    /// "accessibility" - Property namespace prefix
    pub accessibility: Token,
    /// "default" - Default instance name
    pub default_: Token,

    // Attribute names - Backdrop
    /// "ui:description" - Backdrop description
    pub ui_description: Token,

    // Attribute names - NodeGraphNodeAPI
    /// "ui:nodegraph:node:pos" - Node position
    pub ui_nodegraph_node_pos: Token,
    /// "ui:nodegraph:node:size" - Node size
    pub ui_nodegraph_node_size: Token,
    /// "ui:nodegraph:node:stackingOrder" - Z-order
    pub ui_nodegraph_node_stacking_order: Token,
    /// "ui:nodegraph:node:displayColor" - Node tint color
    pub ui_nodegraph_node_display_color: Token,
    /// "ui:nodegraph:node:icon" - Node icon
    pub ui_nodegraph_node_icon: Token,
    /// "ui:nodegraph:node:expansionState" - Expansion state
    pub ui_nodegraph_node_expansion_state: Token,
    /// "ui:nodegraph:node:docURI" - Documentation URI
    pub ui_nodegraph_node_doc_uri: Token,

    // Attribute names - SceneGraphPrimAPI
    /// "ui:displayName" - Display name
    pub ui_display_name: Token,
    /// "ui:displayGroup" - Display group
    pub ui_display_group: Token,

    // Template tokens for multiple-apply schema
    /// "accessibility:__INSTANCE_NAME__:description" - Template
    pub accessibility_template_description: Token,
    /// "accessibility:__INSTANCE_NAME__:label" - Template
    pub accessibility_template_label: Token,
    /// "accessibility:__INSTANCE_NAME__:priority" - Template
    pub accessibility_template_priority: Token,

    // Schema type names
    /// "Backdrop" - Schema identifier
    pub backdrop: Token,
    /// "NodeGraphNodeAPI" - Schema identifier
    pub node_graph_node_api: Token,
    /// "SceneGraphPrimAPI" - Schema identifier
    pub scene_graph_prim_api: Token,
    /// "AccessibilityAPI" - Schema identifier
    pub accessibility_api: Token,
}

impl UsdUITokensType {
    /// Returns all tokens as a vector.
    /// Matches C++ `UsdUITokensType::allTokens`.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.accessibility.clone(),
            self.accessibility_template_description.clone(),
            self.accessibility_template_label.clone(),
            self.accessibility_template_priority.clone(),
            self.closed.clone(),
            self.default_.clone(),
            self.description.clone(),
            self.high.clone(),
            self.label.clone(),
            self.low.clone(),
            self.minimized.clone(),
            self.open.clone(),
            self.priority.clone(),
            self.standard.clone(),
            self.ui_description.clone(),
            self.ui_display_group.clone(),
            self.ui_display_name.clone(),
            self.ui_nodegraph_node_display_color.clone(),
            self.ui_nodegraph_node_doc_uri.clone(),
            self.ui_nodegraph_node_expansion_state.clone(),
            self.ui_nodegraph_node_icon.clone(),
            self.ui_nodegraph_node_pos.clone(),
            self.ui_nodegraph_node_size.clone(),
            self.ui_nodegraph_node_stacking_order.clone(),
            self.accessibility_api.clone(),
            self.backdrop.clone(),
            self.node_graph_node_api.clone(),
            self.scene_graph_prim_api.clone(),
        ]
    }
}

impl UsdUITokensType {
    fn new() -> Self {
        Self {
            // Expansion states
            closed: Token::new("closed"),
            minimized: Token::new("minimized"),
            open: Token::new("open"),

            // Priority values
            low: Token::new("low"),
            standard: Token::new("standard"),
            high: Token::new("high"),

            // AccessibilityAPI base names
            label: Token::new("label"),
            description: Token::new("description"),
            priority: Token::new("priority"),
            accessibility: Token::new("accessibility"),
            default_: Token::new("default"),

            // Backdrop
            ui_description: Token::new("ui:description"),

            // NodeGraphNodeAPI
            ui_nodegraph_node_pos: Token::new("ui:nodegraph:node:pos"),
            ui_nodegraph_node_size: Token::new("ui:nodegraph:node:size"),
            ui_nodegraph_node_stacking_order: Token::new("ui:nodegraph:node:stackingOrder"),
            ui_nodegraph_node_display_color: Token::new("ui:nodegraph:node:displayColor"),
            ui_nodegraph_node_icon: Token::new("ui:nodegraph:node:icon"),
            ui_nodegraph_node_expansion_state: Token::new("ui:nodegraph:node:expansionState"),
            ui_nodegraph_node_doc_uri: Token::new("ui:nodegraph:node:docURI"),

            // SceneGraphPrimAPI
            ui_display_name: Token::new("ui:displayName"),
            ui_display_group: Token::new("ui:displayGroup"),

            // Template tokens
            accessibility_template_description: Token::new(
                "accessibility:__INSTANCE_NAME__:description",
            ),
            accessibility_template_label: Token::new("accessibility:__INSTANCE_NAME__:label"),
            accessibility_template_priority: Token::new("accessibility:__INSTANCE_NAME__:priority"),

            // Schema types
            backdrop: Token::new("Backdrop"),
            node_graph_node_api: Token::new("NodeGraphNodeAPI"),
            scene_graph_prim_api: Token::new("SceneGraphPrimAPI"),
            accessibility_api: Token::new("AccessibilityAPI"),
        }
    }
}

/// Global tokens instance for UsdUI schemas.
pub static USD_UI_TOKENS: LazyLock<UsdUITokensType> = LazyLock::new(UsdUITokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(USD_UI_TOKENS.closed.as_str(), "closed");
        assert_eq!(
            USD_UI_TOKENS.ui_nodegraph_node_pos.as_str(),
            "ui:nodegraph:node:pos"
        );
        assert_eq!(USD_UI_TOKENS.backdrop.as_str(), "Backdrop");
    }
}
