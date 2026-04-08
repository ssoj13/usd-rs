//! Hydra utility functions.
//!
//! Port of pxr/imaging/hd/utils.h/cpp

use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdRetainedSampledDataSource, HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource,
    hd_debug_print_data_source,
};
use crate::material_network::HdMaterialNetworkMap;
use crate::scene_index::{HdSceneIndexHandle, HdSceneIndexPrimView};
use crate::schema::HdSceneGlobalsSchema;
use crate::schema::material::UNIVERSAL_RENDER_CONTEXT;
use crate::schema::material_network::HdMaterialNetworkSchema;
use crate::tokens;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::sync::{Arc, Weak};
use usd_camera_util::ConformWindowPolicy;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Retrieves the active render settings prim path from the scene index.
///
/// Returns `Some(path)` if the path points to a render settings prim
/// with a valid prim container, and `None` otherwise.
///
/// Port of HdUtils::HasActiveRenderSettingsPrim.
pub fn has_active_render_settings_prim(si: &HdSceneIndexHandle) -> Option<Path> {
    let guard = si.read();
    let si_ref = &*guard;

    let sg_schema = HdSceneGlobalsSchema::get_from_scene_index(si_ref);
    let path_handle = sg_schema.get_active_render_settings_prim()?;

    let rsp_path = path_handle.get_typed_value(0.0f32);

    // Validate prim
    let prim = si_ref.get_prim(&rsp_path);
    if prim.prim_type == *tokens::SPRIM_RENDER_SETTINGS && prim.data_source.is_some() {
        return Some(rsp_path);
    }

    None
}

/// Retrieves the active render pass prim path from the scene index.
///
/// Returns `Some(path)` if the path points to a render pass prim
/// with a valid prim container, and `None` otherwise.
///
/// Port of HdUtils::HasActiveRenderPassPrim.
pub fn has_active_render_pass_prim(si: &HdSceneIndexHandle) -> Option<Path> {
    let guard = si.read();
    let si_ref = &*guard;

    let sg_schema = HdSceneGlobalsSchema::get_from_scene_index(si_ref);
    let path_handle = sg_schema.get_active_render_pass_prim()?;

    let rp_path = path_handle.get_typed_value(0.0f32);

    // Validate prim
    let prim = si_ref.get_prim(&rp_path);
    if prim.prim_type == *tokens::SPRIM_RENDER_PASS && prim.data_source.is_some() {
        return Some(rp_path);
    }

    None
}

/// Retrieves the current frame number from the scene index.
///
/// Returns `Some(frame)` if a data source for currentFrame was found, `None` otherwise.
///
/// Port of HdUtils::GetCurrentFrame.
pub fn get_current_frame(si: &HdSceneIndexHandle) -> Option<f64> {
    let guard = si.read();
    let si_ref = &*guard;

    let sg_schema = HdSceneGlobalsSchema::get_from_scene_index(si_ref);
    let frame_handle = sg_schema.get_current_frame()?;

    let frame = frame_handle.get_typed_value(0.0f32);
    if frame.is_nan() {
        return None;
    }
    Some(frame)
}

/// Translates the aspect ratio conform policy token to ConformWindowPolicy.
///
/// Port of HdUtils::ToConformWindowPolicy.
pub fn to_conform_window_policy(token: &usd_tf::Token) -> ConformWindowPolicy {
    let s = token.as_str();
    if s == "adjustApertureWidth" {
        ConformWindowPolicy::MatchVertically
    } else if s == "adjustApertureHeight" {
        ConformWindowPolicy::MatchHorizontally
    } else if s == "expandAperture" {
        ConformWindowPolicy::Fit
    } else if s == "cropAperture" {
        ConformWindowPolicy::Crop
    } else if s == "adjustPixelAspectRatio" {
        ConformWindowPolicy::DontConform
    } else {
        ConformWindowPolicy::Fit // Fallback per C++
    }
}

/// Print the scene index contents to a string for debugging/testing.
///
/// Traverses the scene index from the given root, collects prim paths in
/// lexicographic order, and prints each prim's path, type, and data source.
///
/// Port of HdUtils::PrintSceneIndex.
pub fn print_scene_index(
    out: &mut impl Write,
    si: &HdSceneIndexHandle,
    root_path: &Path,
) -> std::fmt::Result {
    let guard = si.read();
    let si_ref = &*guard;

    let view = HdSceneIndexPrimView::with_root(Arc::clone(si), root_path.clone());
    let prim_path_set: BTreeSet<Path> = view.iter().collect();

    for prim_path in prim_path_set {
        let prim = si_ref.get_prim(&prim_path);
        if let Some(ref data_source) = prim.data_source {
            writeln!(out, "<{}> type = {}", prim_path, prim.prim_type.as_str())?;
            let base_handle = data_source.clone_box();
            hd_debug_print_data_source(out, Some(&base_handle), 1)?;
        }
    }

    Ok(())
}

/// Convert a VtDictionary to a container data source.
///
/// Each key-value pair becomes a named child; values are wrapped in
/// retained sampled data sources. Empty values are skipped.
///
/// Port of HdUtils::ConvertVtDictionaryToContainerDS.
pub fn convert_vt_dictionary_to_container_ds(
    dict: &HashMap<String, Value>,
) -> HdContainerDataSourceHandle {
    let mut children = HashMap::new();
    for (key, value) in dict {
        if value.is_empty() {
            continue;
        }
        let token = Token::new(key);
        let ds: HdDataSourceBaseHandle =
            HdRetainedSampledDataSource::new(value.clone()) as HdDataSourceBaseHandle;
        children.insert(token, ds);
    }
    HdRetainedContainerDataSource::new(children)
}

/// Convert an `HdMaterialNetworkMap` (v1 legacy) into an `HdContainerDataSourceHandle`
/// following the `HdMaterialSchema` / `HdMaterialNetworkSchema` structure.
///
/// The result is a container keyed by render context. The single context used
/// here is `universalRenderContext` (empty token), matching the C++ default.
/// Each context value is an `HdMaterialNetworkSchema` container holding:
/// - `nodes`     — one `HdMaterialNodeSchema` per node
/// - `terminals` — one `HdMaterialConnectionSchema` per network terminal
/// - `config`    — verbatim copy of `HdMaterialNetworkMap::config`
///
/// Port of `HdUtils::ConvertHdMaterialNetworkToHdMaterialSchema`.
pub fn convert_hd_material_network_to_hd_material_schema(
    hd_network_map: &HdMaterialNetworkMap,
) -> HdContainerDataSourceHandle {
    // Accumulate all node and terminal data sources across every terminal network.
    // Nodes may be shared between terminals; later writes win (matching C++).
    let mut node_map: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
    let mut terminal_entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

    // Metadata gathered per parameter name during node parameter processing.
    struct ParamData {
        value: Value,
        color_space: Token,
        type_name: Token,
    }

    for (terminal_name, hd_network) in &hd_network_map.map {
        if hd_network.nodes.is_empty() {
            continue;
        }

        // Build a data source for each node in this network.
        for node in &hd_network.nodes {
            // Group raw parameters by logical name, peeling off
            // "colorSpace:" and "typeName:" namespace prefixes.
            let mut params_info: BTreeMap<String, ParamData> = BTreeMap::new();
            for (param_token, param_value) in &node.parameters {
                let param_str = param_token.as_str();

                let (stripped, matched) = Path::strip_prefix_namespace(param_str, "colorSpace");
                if matched {
                    let entry = params_info.entry(stripped).or_insert_with(|| ParamData {
                        value: Value::default(),
                        color_space: Token::default(),
                        type_name: Token::default(),
                    });
                    if let Some(t) = param_value.get::<Token>() {
                        entry.color_space = t.clone();
                    }
                    continue;
                }

                let (stripped, matched) = Path::strip_prefix_namespace(param_str, "typeName");
                if matched {
                    let entry = params_info.entry(stripped).or_insert_with(|| ParamData {
                        value: Value::default(),
                        color_space: Token::default(),
                        type_name: Token::default(),
                    });
                    if let Some(t) = param_value.get::<Token>() {
                        entry.type_name = t.clone();
                    }
                    continue;
                }

                // Plain parameter value.
                let entry = params_info
                    .entry(param_str.to_string())
                    .or_insert_with(|| ParamData {
                        value: Value::default(),
                        color_space: Token::default(),
                        type_name: Token::default(),
                    });
                entry.value = param_value.clone();
            }

            // Build the parameters container: each entry is an
            // HdMaterialNodeParameterSchema (value, colorSpace?, typeName?).
            let mut params_children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
            for (name, data) in &params_info {
                let mut param_fields: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
                param_fields.push((
                    Token::new("value"),
                    HdRetainedSampledDataSource::new(data.value.clone()) as HdDataSourceBaseHandle,
                ));
                if !data.color_space.as_str().is_empty() {
                    param_fields.push((
                        Token::new("colorSpace"),
                        HdRetainedTypedSampledDataSource::new(data.color_space.clone())
                            as HdDataSourceBaseHandle,
                    ));
                }
                if !data.type_name.as_str().is_empty() {
                    param_fields.push((
                        Token::new("typeName"),
                        HdRetainedTypedSampledDataSource::new(data.type_name.clone())
                            as HdDataSourceBaseHandle,
                    ));
                }
                params_children.insert(
                    Token::new(name),
                    HdRetainedContainerDataSource::from_entries(&param_fields)
                        as HdDataSourceBaseHandle,
                );
            }
            let params_ds = HdRetainedContainerDataSource::new(params_children);

            // Build the inputConnections container.
            // Collect all relationships that target this node (rel.output_id == node.path),
            // grouped by input name (rel.output_name).
            let mut connections_map: HashMap<Token, Vec<HdDataSourceBaseHandle>> = HashMap::new();
            for rel in &hd_network.relationships {
                if rel.output_id != node.path {
                    continue;
                }
                // Upstream node path is stored as a Token (path string), matching C++ GetToken().
                let upstream_path_token = Token::new(rel.input_id.as_str());
                let upstream_output_token = Token::new(rel.input_name.as_str());
                let conn_ds = HdRetainedContainerDataSource::from_entries(&[
                    (
                        Token::new("upstreamNodePath"),
                        HdRetainedTypedSampledDataSource::new(upstream_path_token)
                            as HdDataSourceBaseHandle,
                    ),
                    (
                        Token::new("upstreamNodeOutputName"),
                        HdRetainedTypedSampledDataSource::new(upstream_output_token)
                            as HdDataSourceBaseHandle,
                    ),
                ]) as HdDataSourceBaseHandle;
                connections_map
                    .entry(rel.output_name.clone())
                    .or_default()
                    .push(conn_ds);
            }
            let conn_entries: Vec<(Token, HdDataSourceBaseHandle)> = connections_map
                .into_iter()
                .map(|(input_name, conn_list)| {
                    (
                        input_name,
                        HdRetainedSmallVectorDataSource::new(&conn_list) as HdDataSourceBaseHandle,
                    )
                })
                .collect();
            let connections_ds = HdRetainedContainerDataSource::from_entries(&conn_entries);

            // Assemble the HdMaterialNodeSchema container.
            let node_ds = HdRetainedContainerDataSource::from_entries(&[
                (
                    Token::new("parameters"),
                    params_ds as HdDataSourceBaseHandle,
                ),
                (
                    Token::new("inputConnections"),
                    connections_ds as HdDataSourceBaseHandle,
                ),
                (
                    Token::new("nodeIdentifier"),
                    HdRetainedTypedSampledDataSource::new(node.identifier.clone())
                        as HdDataSourceBaseHandle,
                ),
            ]);

            // Key the node by its path expressed as a Token (matches C++ SdfPath::GetToken()).
            let node_key = Token::new(node.path.as_str());
            node_map.insert(node_key, node_ds as HdDataSourceBaseHandle);
        }

        // Terminal connection points at the last node in the network.
        // The upstream output name is the terminal name itself (C++ behavior).
        let last_node = hd_network.nodes.last().expect("non-empty network");
        let terminal_conn_ds = HdRetainedContainerDataSource::from_entries(&[
            (
                Token::new("upstreamNodePath"),
                HdRetainedTypedSampledDataSource::new(Token::new(last_node.path.as_str()))
                    as HdDataSourceBaseHandle,
            ),
            (
                Token::new("upstreamNodeOutputName"),
                HdRetainedTypedSampledDataSource::new(terminal_name.clone())
                    as HdDataSourceBaseHandle,
            ),
        ]);
        terminal_entries.push((
            terminal_name.clone(),
            terminal_conn_ds as HdDataSourceBaseHandle,
        ));
    }

    let node_entries: Vec<(Token, HdDataSourceBaseHandle)> = node_map.into_iter().collect();
    let nodes_ds = HdRetainedContainerDataSource::from_entries(&node_entries);
    let terminals_ds = HdRetainedContainerDataSource::from_entries(&terminal_entries);

    // Convert the config dictionary (same logic as ConvertVtDictionaryToContainerDS in C++).
    let config_children: HashMap<Token, HdDataSourceBaseHandle> = hd_network_map
        .config
        .iter()
        .filter_map(|(k, v)| {
            if v.is_empty() {
                return None;
            }
            let ds = HdRetainedSampledDataSource::new(v.clone()) as HdDataSourceBaseHandle;
            Some((Token::new(k), ds))
        })
        .collect();
    let config_ds = HdRetainedContainerDataSource::new(config_children);

    // Assemble HdMaterialNetworkSchema container (nodes + terminals + config).
    let network_ds = HdMaterialNetworkSchema::build_retained(
        Some(nodes_ds),
        Some(terminals_ds),
        None, // no interface mapping in v1 networks
        Some(config_ds),
    );

    // Wrap in HdMaterialSchema under universalRenderContext (empty token).
    use crate::schema::material::HdMaterialSchema;
    HdMaterialSchema::build_retained(&[UNIVERSAL_RENDER_CONTEXT.clone()], &[network_ds])
}

/// Associates an application object with a render instance id.
///
/// Useful when using the scene index callback registration facility.
/// The callback is registered once but may be invoked each time the
/// scene index graph is created. An application may spawn several
/// render index instances; this maps callback back to the associated instance.
///
/// **Note**: Not thread-safe.
///
/// Port of HdUtils::RenderInstanceTracker.
pub struct RenderInstanceTracker<T> {
    id_to_instance: HashMap<String, Weak<T>>,
}

impl<T> Default for RenderInstanceTracker<T> {
    fn default() -> Self {
        Self {
            id_to_instance: HashMap::new(),
        }
    }
}

impl<T> RenderInstanceTracker<T> {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an instance for the given render instance id.
    pub fn register_instance(&mut self, render_instance_id: &str, instance: std::sync::Arc<T>) {
        if render_instance_id.is_empty() {
            return;
        }
        self.id_to_instance.insert(
            render_instance_id.to_string(),
            std::sync::Arc::downgrade(&instance),
        );
    }

    /// Unregister the instance for the given id.
    pub fn unregister_instance(&mut self, render_instance_id: &str) {
        self.id_to_instance.remove(render_instance_id);
    }

    /// Get the instance for the given id, if it exists and is still alive.
    pub fn get_instance(&self, id: &str) -> Option<std::sync::Arc<T>> {
        self.id_to_instance.get(id).and_then(|w| w.upgrade())
    }
}
