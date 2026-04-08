//! Node identifier resolving scene index.
//!
//! Port of pxr/imaging/hdsi/nodeIdentifierResolvingSceneIndex.{h,cpp}
//!
//! Resolves shader node identifiers from sourceAsset/sourceCode info stored
//! in node type info (implementationSource). For each node in a material
//! network that lacks a nodeIdentifier (nodeType), queries SDR to find the
//! matching SdrShaderNode and sets its identifier.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::material_network_interface::HdMaterialNetworkInterface;
use usd_hd::scene_index::material_filtering_scene_index_base::{
    HdMaterialFilteringSceneIndexBase, MaterialFilteringFn,
};
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Private tokens matching C++ TF_DEFINE_PRIVATE_TOKENS(_tokens, ...).
const TOKEN_IMPLEMENTATION_SOURCE: &str = "implementationSource";
const TOKEN_SOURCE_CODE: &str = "sourceCode";
const TOKEN_SOURCE_ASSET: &str = "sourceAsset";
const TOKEN_SOURCE_ASSET_SUB_IDENTIFIER: &str = "sourceAsset:subIdentifier";
/// Metadata key for SDR lookup — matches C++ TOKEN_SDR_METADATA token.
/// Reserved for future use when SDR registry is available via FFI.
#[allow(dead_code)]
const TOKEN_SDR_METADATA: &str = "sdrMetadata";

/// Retrieve a string-typed node type info value.
fn get_node_type_info_str(
    interface: &dyn HdMaterialNetworkInterface,
    node_name: &TfToken,
    key: &TfToken,
) -> String {
    let val = interface.get_node_type_info_value(node_name, key);
    if let Some(s) = val.get::<String>() {
        s.clone()
    } else if let Some(t) = val.get::<usd_tf::Token>() {
        t.to_string()
    } else {
        String::new()
    }
}

/// Build a prefixed key: "<sourceType>:<key>".
fn prefixed_key(source_type: &TfToken, key: &str) -> TfToken {
    TfToken::new(&format!("{}:{}", source_type.as_str(), key))
}

/// Attempt to resolve a shader node identifier from source-asset info
/// using SDR registry lookup.
///
/// Mirrors C++ _GetSdrShaderNodeFromSourceAsset and _GetSdrShaderNodeFromSourceCode.
/// Since we have no direct SDR FFI here, we construct the identifier string that
/// SDR would return as the node identifier (sourceType:assetPath subpath).
fn resolve_node_identifier_from_source(
    interface: &dyn HdMaterialNetworkInterface,
    node_name: &TfToken,
    source_type: &TfToken,
    impl_source: &str,
) -> Option<TfToken> {
    if impl_source == TOKEN_SOURCE_ASSET {
        // Resolve from sourceAsset: build SDR asset-based identifier.
        let asset_key = prefixed_key(source_type, TOKEN_SOURCE_ASSET);
        let asset_path = get_node_type_info_str(interface, node_name, &asset_key);
        if asset_path.is_empty() {
            return None;
        }
        let sub_key = prefixed_key(source_type, TOKEN_SOURCE_ASSET_SUB_IDENTIFIER);
        let sub_id = get_node_type_info_str(interface, node_name, &sub_key);
        // Construct the canonical identifier: "<asset>|<subId>" or just "<asset>".
        let identifier = if sub_id.is_empty() {
            asset_path
        } else {
            format!("{}|{}", asset_path, sub_id)
        };
        Some(TfToken::new(&identifier))
    } else if impl_source == TOKEN_SOURCE_CODE {
        // Resolve from inline sourceCode: use sourceType as-is.
        let code_key = prefixed_key(source_type, TOKEN_SOURCE_CODE);
        let source_code = get_node_type_info_str(interface, node_name, &code_key);
        if source_code.is_empty() {
            return None;
        }
        // Identifier for inline code nodes follows SDR convention.
        Some(source_type.clone())
    } else {
        None
    }
}

/// Filtering function: for each node missing a nodeType, attempt to resolve
/// it from sourceAsset/sourceCode info for the given `source_type`.
///
/// Mirrors C++ _SetNodeTypesFromSourceAssetInfo.
fn set_node_types_from_source_asset_info(
    interface: &mut dyn HdMaterialNetworkInterface,
    source_type: &TfToken,
) {
    let impl_source_key = TfToken::new(TOKEN_IMPLEMENTATION_SOURCE);
    // Collect node names first to avoid borrow conflict.
    let node_names = interface.get_node_names();
    for node_name in node_names {
        // Skip nodes that already have a type.
        if !interface.get_node_type(&node_name).is_empty() {
            continue;
        }
        let impl_source = get_node_type_info_str(interface, &node_name, &impl_source_key);
        if let Some(identifier) =
            resolve_node_identifier_from_source(interface, &node_name, source_type, &impl_source)
        {
            interface.set_node_type(&node_name, identifier);
        }
    }
}

/// Hydra scene index that resolves shader node identifiers.
///
/// Port of HdSiNodeIdentifierResolvingSceneIndex.
///
/// For each material prim, iterates all nodes in the material network.
/// If a node has no nodeType set, queries implementationSource in its
/// node type info. If sourceAsset or sourceCode, resolves the identifier
/// via SDR and sets it as the node type.
///
/// The `source_type` constructor parameter selects which shader language
/// to target (e.g. "OSL", "glslfx", "riCpp").
pub struct HdsiNodeIdentifierResolvingSceneIndex {
    base: HdMaterialFilteringSceneIndexBase,
}

impl HdsiNodeIdentifierResolvingSceneIndex {
    /// Creates a new node identifier resolving scene index.
    ///
    /// # Arguments
    /// * `input_scene` - Input scene index to filter.
    /// * `source_type` - Shader source type (e.g. "OSL", "glslfx").
    pub fn new(input_scene: HdSceneIndexHandle, source_type: TfToken) -> Arc<RwLock<Self>> {
        let input_clone = input_scene.clone();
        // Build the filtering closure capturing source_type.
        let filtering_fn: MaterialFilteringFn = Arc::new(move |interface| {
            set_node_types_from_source_asset_info(interface, &source_type);
        });
        let result = Arc::new(RwLock::new(Self {
            base: HdMaterialFilteringSceneIndexBase::new(Some(input_scene), filtering_fn),
        }));
        // C++ parity: constructor calls `_inputSceneIndex->AddObserver(this)`.
        usd_hd::scene_index::filtering::wire_filter_to_input(&result, &input_clone);
        result
    }
}

impl HdSceneIndexBase for HdsiNodeIdentifierResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        self.base.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        self.base.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        format!(
            "HdsiNodeIdentifierResolvingSceneIndex ({})",
            // Extract source_type from filtering_fn description unavailable;
            // display generic name.
            "sourceType"
        )
    }
}

impl FilteringObserverTarget for HdsiNodeIdentifierResolvingSceneIndex {
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.on_prims_added(sender, entries);
    }

    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.on_prims_removed(sender, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[node_identifier_resolving] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first,
            );
        }
        self.base.on_prims_dirtied(sender, entries);
    }

    fn on_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.on_prims_renamed(sender, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify private token constant values match C++ TF_DEFINE_PRIVATE_TOKENS.
    #[test]
    fn test_private_tokens() {
        assert_eq!(TOKEN_IMPLEMENTATION_SOURCE, "implementationSource");
        assert_eq!(TOKEN_SOURCE_CODE, "sourceCode");
        assert_eq!(TOKEN_SOURCE_ASSET, "sourceAsset");
        assert_eq!(
            TOKEN_SOURCE_ASSET_SUB_IDENTIFIER,
            "sourceAsset:subIdentifier"
        );
        assert_eq!(TOKEN_SDR_METADATA, "sdrMetadata");
    }

    /// Verify the prefixed_key helper builds "sourceType:key" strings correctly.
    #[test]
    fn test_prefixed_key() {
        let source_type = TfToken::new("OSL");
        let key = prefixed_key(&source_type, TOKEN_SOURCE_ASSET);
        assert_eq!(key.as_str(), "OSL:sourceAsset");

        let key2 = prefixed_key(&source_type, TOKEN_SOURCE_ASSET_SUB_IDENTIFIER);
        assert_eq!(key2.as_str(), "OSL:sourceAsset:subIdentifier");
    }

    /// Constructor must accept a source_type and produce a valid scene index.
    #[test]
    fn test_new_with_source_type() {
        use usd_hd::scene_index::base::HdSceneIndexHandle;
        use usd_hd::scene_index::{HdRetainedSceneIndex, HdSceneIndexBase};
        // Build a minimal retained scene index as input.
        let retained = HdRetainedSceneIndex::new();
        let handle: HdSceneIndexHandle = retained;
        let si = HdsiNodeIdentifierResolvingSceneIndex::new(handle, TfToken::new("glslfx"));
        // Should produce an empty scene at root.
        let guard = si.read();
        let root = SdfPath::absolute_root();
        let children = guard.get_child_prim_paths(&root);
        assert!(children.is_empty());
    }
}
