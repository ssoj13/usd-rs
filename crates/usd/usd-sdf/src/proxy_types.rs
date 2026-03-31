//! Proxy type aliases - convenient type names for common proxies.
//!
//! This module provides type aliases for commonly-used proxy configurations,
//! making code more readable and consistent with USD API conventions.

use super::Path;
use super::children_proxy::ChildrenProxy;
use super::list_editor_proxy::ListEditorProxy;
use super::list_proxy::ListProxy;
use super::map_edit_proxy::MapEditProxy;
use usd_tf::Token;
use usd_vt::Value as VtValue;

// ============================================================================
// List Proxy Type Aliases
// ============================================================================

/// Proxy for a list of Paths.
pub type PathListProxy = ListProxy<super::proxy_policies::PathTypePolicy>;

/// Proxy for a list of Payloads.
pub type PayloadListProxy = ListProxy<super::proxy_policies::PayloadTypePolicy>;

/// Proxy for a list of References.
pub type ReferenceListProxy = ListProxy<super::proxy_policies::ReferenceTypePolicy>;

/// Proxy for a list of layer identifier strings (sublayers).
pub type SubLayerListProxy = ListProxy<super::proxy_policies::SubLayerTypePolicy>;

/// Proxy for a list of Tokens.
pub type TokenListProxy = ListProxy<super::proxy_policies::TokenTypePolicy>;

/// Proxy for a list of Strings.
pub type StringListProxy = ListProxy<super::proxy_policies::StringTypePolicy>;

// ============================================================================
// List Editor Proxy Type Aliases
// ============================================================================

/// Editor proxy for Path lists.
pub type PathListEditorProxy = ListEditorProxy<super::proxy_policies::PathTypePolicy>;

/// Editor proxy for Payload lists.
pub type PayloadListEditorProxy = ListEditorProxy<super::proxy_policies::PayloadTypePolicy>;

/// Editor proxy for Reference lists.
pub type ReferenceListEditorProxy = ListEditorProxy<super::proxy_policies::ReferenceTypePolicy>;

/// Editor proxy for sublayer lists.
pub type SubLayerListEditorProxy = ListEditorProxy<super::proxy_policies::SubLayerTypePolicy>;

/// Editor proxy for Token lists.
pub type TokenListEditorProxy = ListEditorProxy<super::proxy_policies::TokenTypePolicy>;

/// Editor proxy for String lists.
pub type StringListEditorProxy = ListEditorProxy<super::proxy_policies::StringTypePolicy>;

// ============================================================================
// Children Proxy Type Aliases
// ============================================================================

/// Proxy for prim children.
pub type PrimChildrenProxy = ChildrenProxy<super::children_policies::PrimChildPolicy>;

/// Proxy for property children (all properties).
pub type PropertyChildrenProxy = ChildrenProxy<super::children_policies::PropertyChildPolicy>;

/// Proxy for attribute children.
pub type AttributeChildrenProxy = ChildrenProxy<super::children_policies::AttributeChildPolicy>;

/// Proxy for relationship children.
pub type RelationshipChildrenProxy =
    ChildrenProxy<super::children_policies::RelationshipChildPolicy>;

/// Proxy for variant set children.
pub type VariantSetChildrenProxy = ChildrenProxy<super::children_policies::VariantSetChildPolicy>;

/// Proxy for variant children.
pub type VariantChildrenProxy = ChildrenProxy<super::children_policies::VariantChildPolicy>;

/// Proxy for connection/target path children.
pub type PathChildrenProxy = ChildrenProxy<super::children_policies::PathChildPolicy>;

// ============================================================================
// Map Edit Proxy Type Aliases
// ============================================================================

/// Proxy for dictionary fields (String -> VtValue).
///
/// Used for fields like customData, assetInfo, etc.
pub type DictionaryProxy = MapEditProxy<String, VtValue>;

/// Proxy for Token-keyed dictionaries.
pub type TokenDictionaryProxy = MapEditProxy<Token, VtValue>;

/// Proxy for String-keyed, String-valued maps.
pub type StringMapProxy = MapEditProxy<String, String>;

/// Proxy for relocates maps (Path -> Path).
pub type RelocatesMapProxy = MapEditProxy<Path, Path>;

// ============================================================================
// Specialized Proxies for Common USD Fields
// ============================================================================

/// Proxy for prim references list.
///
/// Equivalent to `SdfReferencesProxy` in C++.
pub type ReferencesProxy = ReferenceListEditorProxy;

/// Proxy for prim payloads list.
///
/// Equivalent to `SdfPayloadsProxy` in C++.
pub type PayloadsProxy = PayloadListEditorProxy;

/// Proxy for relationship targets.
///
/// Equivalent to `SdfTargetsProxy` in C++.
pub type TargetsProxy = PathListEditorProxy;

/// Proxy for attribute connections.
///
/// Equivalent to `SdfConnectionsProxy` in C++.
pub type ConnectionsProxy = PathListEditorProxy;

/// Proxy for sublayers list.
///
/// Equivalent to `SdfSubLayersProxy` in C++.
pub type SubLayersProxy = SubLayerListEditorProxy;

/// Proxy for name children order.
///
/// Used for reorder statements in prims.
pub type NameChildrenOrderProxy = TokenListProxy;

/// Proxy for property order.
///
/// Used for property reorder statements.
pub type PropertyOrderProxy = TokenListProxy;

/// Proxy for variant selections.
///
/// Maps variant set name to selected variant name.
pub type VariantSelectionsProxy = MapEditProxy<String, String>;

// ============================================================================
// Documentation
// ============================================================================

/// Re-export commonly-used proxy types for convenience.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::proxy_types::*;
///
/// // Use type aliases instead of full generic types
/// let references: ReferencesProxy = prim.get_references();
/// let payloads: PayloadsProxy = prim.get_payloads();
/// let children: PrimChildrenProxy = prim.get_children();
/// let custom_data: DictionaryProxy = prim.get_custom_data();
/// ```
pub mod prelude {
    pub use super::{
        AttributeChildrenProxy, ConnectionsProxy, DictionaryProxy, PathListProxy, PayloadListProxy,
        PayloadsProxy, PrimChildrenProxy, PropertyChildrenProxy, ReferenceListProxy,
        ReferencesProxy, RelationshipChildrenProxy, RelocatesMapProxy, StringMapProxy,
        SubLayerListProxy, SubLayersProxy, TargetsProxy, TokenDictionaryProxy, TokenListProxy,
        VariantChildrenProxy, VariantSelectionsProxy, VariantSetChildrenProxy,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_type_aliases_compile() {
        // Just ensure types compile correctly
        let _path_proxy: PathListProxy;
        let _payload_proxy: PayloadListProxy;
        let _reference_proxy: ReferenceListProxy;
        let _dict_proxy: DictionaryProxy;
    }

    #[test]
    fn test_prelude_exports() {
        use super::prelude::*;

        // Verify prelude exports the most common types
        let _refs: ReferencesProxy;
        let _pays: PayloadsProxy;
        let _children: PrimChildrenProxy;
        let _dict: DictionaryProxy;
    }
}
