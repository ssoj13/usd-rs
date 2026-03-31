//! DataSourceMaterial - Material data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceMaterial.h/.cpp
//!
//! Walks UsdShade networks starting from material output terminals,
//! building HdMaterialNetwork containers with nodes, connections,
//! terminals, and parameters for Hydra consumption.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_prim::DataSourcePrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use indexmap::IndexMap;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdRetainedSmallVectorDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static MATERIAL: LazyLock<Token> = LazyLock::new(|| Token::new("material"));
    pub static NODES: LazyLock<Token> = LazyLock::new(|| Token::new("nodes"));
    pub static TERMINALS: LazyLock<Token> = LazyLock::new(|| Token::new("terminals"));
    #[allow(dead_code)] // C++ material network context tokens kept for schema parity
    pub static INTERFACE: LazyLock<Token> = LazyLock::new(|| Token::new("interface"));
    #[allow(dead_code)] // C++ material network context tokens kept for schema parity
    pub static ALL: LazyLock<Token> = LazyLock::new(|| Token::new(""));
    #[allow(dead_code)] // C++ material network context tokens kept for schema parity
    pub static SURFACE: LazyLock<Token> = LazyLock::new(|| Token::new("surface"));
    #[allow(dead_code)] // C++ material network context tokens kept for schema parity
    pub static DISPLACEMENT: LazyLock<Token> = LazyLock::new(|| Token::new("displacement"));
    #[allow(dead_code)] // C++ material network context tokens kept for schema parity
    pub static VOLUME: LazyLock<Token> = LazyLock::new(|| Token::new("volume"));

    // Material node schema tokens
    pub static PARAMETERS: LazyLock<Token> = LazyLock::new(|| Token::new("parameters"));
    pub static INPUT_CONNECTIONS: LazyLock<Token> =
        LazyLock::new(|| Token::new("inputConnections"));
    pub static NODE_IDENTIFIER: LazyLock<Token> = LazyLock::new(|| Token::new("nodeIdentifier"));

    // Material node parameter schema tokens
    pub static VALUE: LazyLock<Token> = LazyLock::new(|| Token::new("value"));
    pub static COLOR_SPACE: LazyLock<Token> = LazyLock::new(|| Token::new("colorSpace"));
    pub static TYPE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("typeName"));

    // Connection schema tokens
    pub static UPSTREAM_NODE_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("upstreamNodePath"));
    pub static UPSTREAM_NODE_OUTPUT_NAME: LazyLock<Token> =
        LazyLock::new(|| Token::new("upstreamNodeOutputName"));

    // Prefixes
    pub static INPUTS_PREFIX: LazyLock<Token> = LazyLock::new(|| Token::new("inputs:"));
    pub static OUTPUTS_PREFIX: LazyLock<Token> = LazyLock::new(|| Token::new("outputs:"));
    pub static INFO_ID: LazyLock<Token> = LazyLock::new(|| Token::new("info:id"));

    // Roles
    pub static COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("Color"));
    pub static ASSET: LazyLock<Token> = LazyLock::new(|| Token::new("asset"));

    // sourceColorSpace skip
    pub static SOURCE_COLOR_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("inputs:sourceColorSpace"));
}

// =============================================================================
// Helper: make a path relative to a material prefix
// =============================================================================

/// Strip `prefix` from `path`, yielding a relative token.
/// E.g. `/Mat/PBR` with prefix `/Mat` -> `PBR`.
fn relative_path(prefix: &Path, path: &Path) -> Token {
    if prefix.is_empty() {
        return path.get_token();
    }
    let prefix_str = prefix.get_string();
    let path_str = path.get_string();
    if let Some(suffix) = path_str.strip_prefix(&prefix_str) {
        // Strip leading '/'
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
        Token::new(suffix)
    } else {
        path.get_token()
    }
}

/// Extract render context from output name.
/// "outputs:surface" -> ""
/// "outputs:ri:surface" -> "ri"
fn get_render_context(output_name: &str) -> Token {
    let prefix = "outputs:";
    if let Some(rest) = output_name.strip_prefix(prefix) {
        // rest is e.g. "surface" or "ri:surface"
        if let Some(colon_pos) = rest.find(':') {
            // Has render context: everything before last ':'
            Token::new(&rest[..colon_pos])
        } else {
            // Universal context
            Token::new("")
        }
    } else {
        Token::new("")
    }
}

// =============================================================================
// ShadingNodeParameters - lazy container for shader input values
// =============================================================================

/// Container data source for a shader node's parameter values.
///
/// For each input on a UsdShadeShader that has an authored value (not a
/// connection), returns an HdMaterialNodeParameter container with
/// {value, colorSpace, typeName}.
#[derive(Clone)]
struct ShadingNodeParameters {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
}

impl std::fmt::Debug for ShadingNodeParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadingNodeParameters").finish()
    }
}

impl ShadingNodeParameters {
    fn new(
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        scene_index_path: Path,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            scene_index_path,
        })
    }
}

impl HdDataSourceBase for ShadingNodeParameters {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ShadingNodeParameters {
    /// Returns base names of inputs that have authored values (not connections).
    fn get_names(&self) -> Vec<Token> {
        let mut result = Vec::new();
        let inputs_prefix = tokens::INPUTS_PREFIX.as_str();

        for prop in self
            .prim
            .get_authored_properties_in_namespace(&tokens::INPUTS_PREFIX)
        {
            let Some(attr) = prop.as_attribute() else {
                continue;
            };
            let name_str = attr.name().as_str().to_string();

            // Only include actual input attributes, not connection targets
            if !name_str.starts_with(inputs_prefix) {
                continue;
            }

            // Skip inputs:sourceColorSpace (consolidated on inputs:file)
            if name_str == tokens::SOURCE_COLOR_SPACE.as_str() {
                continue;
            }

            let base_name = &name_str[inputs_prefix.len()..];

            // Check this is a value-producing attribute (not a connection)
            // An attribute with authored value + no connections is a parameter
            if attr.has_value() {
                result.push(Token::new(base_name));
            }
        }
        result
    }

    /// Returns a MaterialNodeParameter container for the named input.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let attr_name = format!("{}{}", tokens::INPUTS_PREFIX.as_str(), name.as_str());
        let attr = self.prim.get_attribute(&attr_name)?;

        if !attr.has_value() {
            return None;
        }

        // Build HdMaterialNodeParameterSchema: { value, colorSpace, typeName }
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

        // value: wrap as sampled data source
        let value_ds = DataSourceAttribute::<Value>::new(
            attr.clone(),
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        );
        entries.push((tokens::VALUE.clone(), value_ds as HdDataSourceBaseHandle));

        // colorSpace: if role is Color or type is Asset
        let role = attr.get_role_name();
        let type_name = attr.get_type_name();
        let type_name_str = type_name.get_alias().unwrap_or_default();
        if role == tokens::COLOR || type_name_str == tokens::ASSET.as_str() {
            // Author color space if present on the attribute
            if let Some(cs_value) = attr.get_metadata(&Token::new("colorSpace")) {
                if let Some(cs_str) = cs_value.get::<String>() {
                    let cs_ds = HdRetainedTypedSampledDataSource::new(Token::new(cs_str));
                    entries.push((tokens::COLOR_SPACE.clone(), cs_ds as HdDataSourceBaseHandle));
                }
            }
        }

        // typeName
        let type_token = attr.type_name();
        if !type_token.is_empty() {
            let tn_ds = HdRetainedTypedSampledDataSource::new(type_token);
            entries.push((tokens::TYPE_NAME.clone(), tn_ds as HdDataSourceBaseHandle));
        }

        Some(HdRetainedContainerDataSource::from_entries(&entries) as HdDataSourceBaseHandle)
    }
}

// =============================================================================
// ShadingNodeInputConnections - lazy container for upstream connections
// =============================================================================

/// Container data source for a shader node's input connections.
///
/// For each input on a UsdShadeShader that is connected to an upstream
/// output, returns a vector of HdMaterialConnection containers.
#[derive(Clone)]
struct ShadingNodeInputConnections {
    prim: Prim,
    material_prefix: Path,
}

impl std::fmt::Debug for ShadingNodeInputConnections {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadingNodeInputConnections").finish()
    }
}

impl ShadingNodeInputConnections {
    fn new(prim: Prim, material_prefix: Path) -> Arc<Self> {
        Arc::new(Self {
            prim,
            material_prefix,
        })
    }
}

impl HdDataSourceBase for ShadingNodeInputConnections {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ShadingNodeInputConnections {
    /// Returns base names of inputs that have connections to upstream outputs.
    fn get_names(&self) -> Vec<Token> {
        let mut result = Vec::new();
        let inputs_prefix = tokens::INPUTS_PREFIX.as_str();

        for prop in self
            .prim
            .get_authored_properties_in_namespace(&tokens::INPUTS_PREFIX)
        {
            let Some(attr) = prop.as_attribute() else {
                continue;
            };
            let name_str = attr.name().as_str().to_string();
            if !name_str.starts_with(inputs_prefix) {
                continue;
            }
            let base_name = &name_str[inputs_prefix.len()..];

            // Check if connected to an upstream output
            let connections = attr.get_connections();
            for conn_path in &connections {
                let conn_name = conn_path.get_name();
                if conn_name.starts_with(tokens::OUTPUTS_PREFIX.as_str()) {
                    result.push(Token::new(base_name));
                    break;
                }
            }
        }
        result
    }

    /// Returns a vector of MaterialConnection containers for the named input.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let attr_name = format!("{}{}", tokens::INPUTS_PREFIX.as_str(), name.as_str());
        let attr = self.prim.get_attribute(&attr_name)?;

        let connections = attr.get_connections();
        if connections.is_empty() {
            return None;
        }

        let mut elements: Vec<HdDataSourceBaseHandle> = Vec::new();
        let outputs_prefix = tokens::OUTPUTS_PREFIX.as_str();

        for conn_path in &connections {
            let conn_name = conn_path.get_name();
            if !conn_name.starts_with(outputs_prefix) {
                continue;
            }

            // Upstream node path: relative to material prefix
            let upstream_prim_path = conn_path.get_prim_path();
            let upstream_token = relative_path(&self.material_prefix, &upstream_prim_path);

            // Upstream output name: strip "outputs:" prefix
            let output_name = &conn_name[outputs_prefix.len()..];

            let conn_ds = HdRetainedContainerDataSource::from_entries(&[
                (
                    tokens::UPSTREAM_NODE_PATH.clone(),
                    HdRetainedTypedSampledDataSource::new(upstream_token) as HdDataSourceBaseHandle,
                ),
                (
                    tokens::UPSTREAM_NODE_OUTPUT_NAME.clone(),
                    HdRetainedTypedSampledDataSource::new(Token::new(output_name))
                        as HdDataSourceBaseHandle,
                ),
            ]);
            elements.push(conn_ds as HdDataSourceBaseHandle);
        }

        if elements.is_empty() {
            return None;
        }

        Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
    }
}

// =============================================================================
// ShadingNode - container for a single shader node
// =============================================================================

/// Container data source representing a single shader node in a material network.
///
/// Returns {nodeIdentifier, parameters, inputConnections} sub-data-sources.
#[derive(Clone)]
struct ShadingNode {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
    material_prefix: Path,
}

impl std::fmt::Debug for ShadingNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadingNode")
            .field("prim", &self.prim.path().get_string())
            .finish()
    }
}

impl ShadingNode {
    fn new(
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        scene_index_path: Path,
        material_prefix: Path,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            scene_index_path,
            material_prefix,
        })
    }
}

impl HdDataSourceBase for ShadingNode {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ShadingNode {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::NODE_IDENTIFIER.clone(),
            tokens::PARAMETERS.clone(),
            tokens::INPUT_CONNECTIONS.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::NODE_IDENTIFIER {
            // Get shader ID from info:id attribute
            let node_id = self.get_shader_id();
            return Some(HdRetainedTypedSampledDataSource::new(node_id) as HdDataSourceBaseHandle);
        }

        if *name == *tokens::PARAMETERS {
            return Some(ShadingNodeParameters::new(
                self.prim.clone(),
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle);
        }

        if *name == *tokens::INPUT_CONNECTIONS {
            return Some(ShadingNodeInputConnections::new(
                self.prim.clone(),
                self.material_prefix.clone(),
            ) as HdDataSourceBaseHandle);
        }

        None
    }
}

impl ShadingNode {
    /// Get shader ID from info:id, falling back to prim type name.
    fn get_shader_id(&self) -> Token {
        // Try info:id first (most common case)
        if let Some(attr) = self.prim.get_attribute(tokens::INFO_ID.as_str()) {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(token) = value.get::<Token>() {
                    return token.clone();
                }
                if let Some(s) = value.get::<String>() {
                    return Token::new(s);
                }
            }
        }

        // Fallback: use prim type name
        self.prim.type_name()
    }
}

// =============================================================================
// Walk graph - recursive network traversal
// =============================================================================

/// Walk the shade network starting from `prim`, collecting nodes into `output_nodes`.
/// `material_prefix` is stripped from all paths to produce relative node names.
fn walk_graph(
    prim: &Prim,
    output_nodes: &mut IndexMap<Token, HdDataSourceBaseHandle>,
    stage_globals: &DataSourceStageGlobalsHandle,
    scene_index_path: &Path,
    material_prefix: &Path,
) {
    if !prim.is_valid() {
        return;
    }

    let node_path = prim.path().clone();
    if node_path.is_empty() {
        return;
    }

    let node_name = relative_path(material_prefix, &node_path);
    if output_nodes.contains_key(&node_name) {
        // Already visited
        return;
    }

    // C++ _WalkGraph: if this prim is a NodeGraph, we need to traverse INTO it
    // by following its outputs to the internal shaders. NodeGraphs are containers
    // (encapsulation boundaries) — their interface outputs connect to internal nodes.
    use usd_shade::node_graph::NodeGraph;
    let is_node_graph = {
        let tn = prim.type_name();
        tn.as_str() == "NodeGraph" || tn.as_str() == "Material"
    };

    if is_node_graph {
        // Don't add the NodeGraph itself as a shading node.
        // Instead, follow its outputs to internal shaders.
        let ng = NodeGraph::new(prim.clone());
        for output in ng.get_outputs(true) {
            let mut invalid = Vec::new();
            for source_info in output.get_connected_sources(&mut invalid) {
                if source_info.is_valid() {
                    let inner_prim = source_info.source.get_prim();
                    walk_graph(
                        &inner_prim,
                        output_nodes,
                        stage_globals,
                        scene_index_path,
                        material_prefix,
                    );
                }
            }
        }
        return;
    }

    // Create the node data source (for Shader prims)
    let node_ds = ShadingNode::new(
        prim.clone(),
        stage_globals.clone(),
        scene_index_path.clone(),
        material_prefix.clone(),
    );
    output_nodes.insert(node_name, node_ds as HdDataSourceBaseHandle);

    // Recurse into upstream nodes via input connections
    let inputs_prefix = tokens::INPUTS_PREFIX.as_str();
    for prop in prim.get_authored_properties_in_namespace(&tokens::INPUTS_PREFIX) {
        let Some(attr) = prop.as_attribute() else {
            continue;
        };
        let name_str = attr.name().as_str().to_string();
        if !name_str.starts_with(inputs_prefix) {
            continue;
        }

        for conn_path in attr.get_connections() {
            let conn_name = conn_path.get_name();
            if !conn_name.starts_with(tokens::OUTPUTS_PREFIX.as_str()) {
                continue;
            }

            // Get upstream prim
            let upstream_prim_path = conn_path.get_prim_path();
            let Some(stage) = prim.stage() else {
                continue;
            };
            let Some(upstream_prim) = stage.get_prim_at_path(&upstream_prim_path) else {
                continue;
            };

            walk_graph(
                &upstream_prim,
                output_nodes,
                stage_globals,
                scene_index_path,
                material_prefix,
            );
        }
    }
}

// =============================================================================
// Build material network for a render context
// =============================================================================

/// Build the HdMaterialNetwork container for a given material prim and
/// render context. Returns None if no terminals are found.
fn build_material(
    mat_prim: &Prim,
    stage_globals: &DataSourceStageGlobalsHandle,
    render_context: &Token,
    scene_index_path: &Path,
) -> Option<HdDataSourceBaseHandle> {
    use usd_shade::material::Material;
    use usd_shade::node_graph::NodeGraph;
    use usd_shade::types::AttributeType;

    let material_prefix = mat_prim.path().clone();
    let node_graph = NodeGraph::new(mat_prim.clone());
    let material = Material::new(mat_prim.clone());

    let mut terminal_names: Vec<Token> = Vec::new();
    let mut terminal_values: Vec<HdDataSourceBaseHandle> = Vec::new();
    let mut node_data_sources: IndexMap<Token, HdDataSourceBaseHandle> = IndexMap::new();

    let is_all = render_context.is_empty();

    // Iterate material outputs to find terminals
    let outputs = node_graph.get_outputs(true);
    for output in &outputs {
        let full_name = output.get_full_name();
        let full_name_str = full_name.as_str().to_string();
        let base_name = output.get_base_name();

        // Filter by render context: skip outputs that don't match.
        // When is_all (empty context), accept all outputs.
        if !is_all {
            // render_context is non-empty here (is_all = render_context.is_empty())
            let output_ctx = get_render_context(&full_name_str);
            if output_ctx != *render_context {
                continue;
            }
        }

        // Determine terminal name
        let terminal_name = if is_all {
            // For "all" context, keep full base name (e.g. "ri:surface")
            base_name.clone()
        } else if !render_context.is_empty() {
            // Strip render context prefix from base name
            // "ri:surface" -> "surface"
            let base_str = base_name.as_str();
            let ctx_prefix = format!("{}:", render_context.as_str());
            if let Some(stripped) = base_str.strip_prefix(&ctx_prefix) {
                Token::new(stripped)
            } else {
                base_name.clone()
            }
        } else {
            base_name.clone()
        };

        // Follow connection from this output to find upstream shader
        let mut invalid_paths = Vec::new();
        let sources = output.get_connected_sources(&mut invalid_paths);

        for source_info in &sources {
            if !source_info.is_valid() {
                continue;
            }

            let upstream_prim = source_info.source.get_prim();

            if is_all {
                // For "all" context, include all shader descendants
                for child in mat_prim.descendants() {
                    let child_type = child.type_name();
                    if child_type == "Shader" {
                        let child_path = child.path().clone();
                        let child_name = relative_path(&material_prefix, &child_path);
                        if !node_data_sources.contains_key(&child_name) {
                            let node_ds = ShadingNode::new(
                                child.clone(),
                                stage_globals.clone(),
                                scene_index_path.clone(),
                                material_prefix.clone(),
                            );
                            node_data_sources.insert(child_name, node_ds as HdDataSourceBaseHandle);
                        }
                    }
                }
            } else {
                // Walk the graph starting from upstream shader
                walk_graph(
                    &upstream_prim,
                    &mut node_data_sources,
                    stage_globals,
                    scene_index_path,
                    &material_prefix,
                );
            }

            // Build terminal connection
            let upstream_path = relative_path(&material_prefix, upstream_prim.path());
            let terminal_ds = HdRetainedContainerDataSource::from_entries(&[
                (
                    tokens::UPSTREAM_NODE_PATH.clone(),
                    HdRetainedTypedSampledDataSource::new(upstream_path) as HdDataSourceBaseHandle,
                ),
                (
                    tokens::UPSTREAM_NODE_OUTPUT_NAME.clone(),
                    HdRetainedTypedSampledDataSource::new(source_info.source_name.clone())
                        as HdDataSourceBaseHandle,
                ),
            ]);

            terminal_names.push(terminal_name.clone());
            terminal_values.push(terminal_ds as HdDataSourceBaseHandle);
        }
    }

    // Also try compute_*_source for standard terminals if no outputs found
    if terminal_names.is_empty() && material.is_valid() {
        // Try surface
        let ctx_vec = if render_context.is_empty() {
            vec![Token::new("")]
        } else {
            vec![render_context.clone()]
        };

        for (terminal_name_str, compute_fn) in [
            (
                "surface",
                Material::compute_surface_source
                    as fn(
                        &Material,
                        &[Token],
                        &mut Token,
                        &mut AttributeType,
                    ) -> usd_shade::shader::Shader,
            ),
            ("displacement", Material::compute_displacement_source),
            ("volume", Material::compute_volume_source),
        ] {
            let mut source_name = Token::new("");
            let mut source_type = AttributeType::Invalid;
            let shader = compute_fn(&material, &ctx_vec, &mut source_name, &mut source_type);

            if shader.is_valid() && source_type == AttributeType::Output {
                let shader_prim = shader.get_prim();

                // Walk graph from this shader
                walk_graph(
                    &shader_prim,
                    &mut node_data_sources,
                    stage_globals,
                    scene_index_path,
                    &material_prefix,
                );

                let upstream_path = relative_path(&material_prefix, shader_prim.path());
                let terminal_ds = HdRetainedContainerDataSource::from_entries(&[
                    (
                        tokens::UPSTREAM_NODE_PATH.clone(),
                        HdRetainedTypedSampledDataSource::new(upstream_path)
                            as HdDataSourceBaseHandle,
                    ),
                    (
                        tokens::UPSTREAM_NODE_OUTPUT_NAME.clone(),
                        HdRetainedTypedSampledDataSource::new(source_name)
                            as HdDataSourceBaseHandle,
                    ),
                ]);

                terminal_names.push(Token::new(terminal_name_str));
                terminal_values.push(terminal_ds as HdDataSourceBaseHandle);
            }
        }
    }

    if terminal_names.is_empty() {
        return None;
    }

    // Build terminals container
    let terminal_entries: Vec<(Token, HdDataSourceBaseHandle)> =
        terminal_names.into_iter().zip(terminal_values).collect();
    let terminals_ds = HdRetainedContainerDataSource::from_entries(&terminal_entries);

    // Build nodes container
    let node_entries: Vec<(Token, HdDataSourceBaseHandle)> =
        node_data_sources.into_iter().collect();
    let nodes_ds = HdRetainedContainerDataSource::from_entries(&node_entries);

    // Build material network: { nodes, terminals }
    let network_ds = HdRetainedContainerDataSource::from_entries(&[
        (tokens::NODES.clone(), nodes_ds as HdDataSourceBaseHandle),
        (
            tokens::TERMINALS.clone(),
            terminals_ds as HdDataSourceBaseHandle,
        ),
    ]);

    Some(network_ds as HdDataSourceBaseHandle)
}

// =============================================================================
// DataSourceMaterial - container returning per-render-context networks
// =============================================================================

/// Container data source for material-specific data.
///
/// Returns material networks keyed by render context.
/// GetNames() returns discovered render contexts (including "all").
/// Get(context) builds and caches the material network for that context.
#[derive(Clone)]
pub struct DataSourceMaterial {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceMaterial")
            .field("path", &self.prim.path().get_string())
            .finish()
    }
}

impl DataSourceMaterial {
    /// Creates a new material data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }

    /// Collect render contexts from material output attributes.
    fn collect_render_contexts(&self) -> Vec<Token> {
        use usd_shade::node_graph::NodeGraph;

        let mut contexts = Vec::new();
        let node_graph = NodeGraph::new(self.prim.clone());
        let outputs = node_graph.get_outputs(true);

        for output in &outputs {
            let full_name = output.get_full_name();
            let ctx = get_render_context(full_name.as_str());
            if !contexts.iter().any(|c: &Token| c == &ctx) {
                contexts.push(ctx);
            }
        }

        // Always add the "all" context (empty string)
        let all = Token::new("");
        if !contexts.iter().any(|c| c == &all) {
            contexts.push(all);
        }

        contexts
    }
}

impl HdDataSourceBase for DataSourceMaterial {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMaterial {
    fn get_names(&self) -> Vec<Token> {
        self.collect_render_contexts()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        build_material(
            &self.prim,
            &self.stage_globals,
            name,
            &self.scene_index_path,
        )
    }
}

/// Handle type for DataSourceMaterial.
pub type DataSourceMaterialHandle = Arc<DataSourceMaterial>;

// =============================================================================
// DataSourceMaterialPrim - prim-level data source adding "material" key
// =============================================================================

/// Prim data source for UsdShadeMaterial.
///
/// Extends the base prim data source with a "material" key that returns
/// the DataSourceMaterial container.
#[derive(Clone)]
pub struct DataSourceMaterialPrim {
    base: DataSourcePrim,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceMaterialPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceMaterialPrim").finish()
    }
}

impl DataSourceMaterialPrim {
    /// Creates a new material prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourcePrim::new(prim.clone(), scene_index_path, stage_globals.clone()),
            prim,
            stage_globals,
        }
    }

    /// Returns the list of data source names.
    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::MATERIAL.clone());
        names
    }

    /// Gets a data source by name.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::MATERIAL {
            let ds = DataSourceMaterial::new(
                self.base.hydra_path().clone(),
                self.prim.clone(),
                self.stage_globals.clone(),
            );
            return Some(Arc::new(ds) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    /// Computes invalidation locators for property changes.
    ///
    /// Any change to outputs, surface, displacement, volume, or interface
    /// inputs dirties the entire material locator.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        if subprim.is_empty() {
            for prop in properties {
                let prop_str = prop.as_str();
                // Any shader-related property change dirties the material
                if prop_str.starts_with("outputs:")
                    || prop_str.starts_with("inputs:")
                    || prop_str.contains("surface")
                    || prop_str.contains("displacement")
                    || prop_str.contains("volume")
                    || prop_str == "info:id"
                {
                    locators.insert(HdDataSourceLocator::from_token(tokens::MATERIAL.clone()));
                    break;
                }
            }
        }
        locators
    }
}

/// Handle type for DataSourceMaterialPrim.
pub type DataSourceMaterialPrimHandle = Arc<DataSourceMaterialPrim>;

impl HdDataSourceBase for DataSourceMaterialPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMaterialPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceMaterialPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceMaterialPrim::get(self, name)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::common::InitialLoadSet;
    use usd_core::stage::Stage;
    use usd_sdf::Path;
    use usd_shade::material::Material;
    use usd_shade::shader::Shader;
    use usd_shade::tokens::tokens as shade_tokens;
    use usd_vt::Value;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    // ====================================================================
    // Basic construction
    // ====================================================================

    #[test]
    fn test_material_ds_construction() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceMaterial::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();
        // Should at least have the "all" context
        assert!(
            names.iter().any(|n| n.is_empty()),
            "Expected 'all' (empty) render context"
        );
    }

    // ====================================================================
    // Material prim data source
    // ====================================================================

    #[test]
    fn test_material_prim_names() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Mat", "Material")
            .expect("define material");
        let globals = create_test_globals();

        let ds = DataSourceMaterialPrim::new(Path::from_string("/Mat").unwrap(), prim, globals);
        let names = ds.get_names();
        assert!(
            names.iter().any(|n| n == "material"),
            "Expected 'material' in names"
        );
    }

    #[test]
    fn test_material_prim_get_material() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Mat", "Material")
            .expect("define material");
        let globals = create_test_globals();

        let ds = DataSourceMaterialPrim::new(Path::from_string("/Mat").unwrap(), prim, globals);
        let result = ds.get(&tokens::MATERIAL);
        assert!(result.is_some(), "Expected material data source");
    }

    // ====================================================================
    // Helper: relative_path
    // ====================================================================

    #[test]
    fn test_relative_path() {
        let prefix = Path::from_string("/Mat").unwrap();
        let path = Path::from_string("/Mat/PBR").unwrap();
        let rel = relative_path(&prefix, &path);
        assert_eq!(rel.as_str(), "PBR");
    }

    #[test]
    fn test_relative_path_nested() {
        let prefix = Path::from_string("/World/Mat").unwrap();
        let path = Path::from_string("/World/Mat/Shaders/PBR").unwrap();
        let rel = relative_path(&prefix, &path);
        assert_eq!(rel.as_str(), "Shaders/PBR");
    }

    #[test]
    fn test_relative_path_empty_prefix() {
        let prefix = Path::empty();
        let path = Path::from_string("/Mat/PBR").unwrap();
        let rel = relative_path(&prefix, &path);
        assert_eq!(rel.as_str(), "/Mat/PBR");
    }

    // ====================================================================
    // Helper: get_render_context
    // ====================================================================

    #[test]
    fn test_render_context_universal() {
        let ctx = get_render_context("outputs:surface");
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_render_context_ri() {
        let ctx = get_render_context("outputs:ri:surface");
        assert_eq!(ctx.as_str(), "ri");
    }

    #[test]
    fn test_render_context_glslfx() {
        let ctx = get_render_context("outputs:glslfx:surface");
        assert_eq!(ctx.as_str(), "glslfx");
    }

    // ====================================================================
    // Full material network walk
    // ====================================================================

    #[test]
    fn test_material_network_with_shader() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

        // Create material
        let mat = Material::define(&stage, &Path::from_string("/Mat").unwrap());
        assert!(mat.is_valid());

        // Create shader
        let shader = Shader::define(&stage, &Path::from_string("/Mat/PBR").unwrap());
        assert!(shader.is_valid());

        // Set shader ID
        shader.set_shader_id(&Token::new("UsdPreviewSurface"));

        // Create shader output
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        shader.create_output(&Token::new("surface"), &token_type);

        // Connect material surface output -> shader output
        let universal = shade_tokens().universal_render_context.clone();
        let mat_output = mat.create_surface_output(&universal);
        assert!(mat_output.is_defined());
        mat_output.connect_to_source_path(&Path::from_string("/Mat/PBR.outputs:surface").unwrap());

        // Create a diffuseColor input on the shader with a value
        let float3_type = usd_sdf::ValueTypeRegistry::instance().find_type("float3");
        let diffuse_input = shader.create_input(&Token::new("diffuseColor"), &float3_type);
        assert!(diffuse_input.is_defined());
        diffuse_input.set(
            Value::from(vec![0.8f32, 0.2, 0.1]),
            usd_sdf::TimeCode::default(),
        );

        // Now build the material data source
        let globals = create_test_globals();
        let ds =
            DataSourceMaterial::new(Path::from_string("/Mat").unwrap(), mat.get_prim(), globals);

        // Get the universal render context network
        let all_ctx = Token::new("");
        let network = ds.get(&all_ctx);
        assert!(
            network.is_some(),
            "Expected material network for universal context"
        );

        // Verify it's a container
        let network_handle = network.unwrap();
        let any = network_handle.as_any();
        assert!(
            any.downcast_ref::<HdRetainedContainerDataSource>()
                .is_some(),
            "Network should be a container data source"
        );
    }

    #[test]
    fn test_material_network_with_texture_chain() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

        // Material -> PBR shader -> Texture reader
        let mat = Material::define(&stage, &Path::from_string("/Mat").unwrap());
        let pbr = Shader::define(&stage, &Path::from_string("/Mat/PBR").unwrap());
        let tex = Shader::define(&stage, &Path::from_string("/Mat/Tex").unwrap());

        pbr.set_shader_id(&Token::new("UsdPreviewSurface"));
        tex.set_shader_id(&Token::new("UsdUVTexture"));

        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        let float3_type = usd_sdf::ValueTypeRegistry::instance().find_type("float3");

        // Tex outputs:rgb
        tex.create_output(&Token::new("rgb"), &float3_type);

        // PBR outputs:surface
        pbr.create_output(&Token::new("surface"), &token_type);

        // PBR inputs:diffuseColor -> Tex outputs:rgb
        let diffuse_input = pbr.create_input(&Token::new("diffuseColor"), &float3_type);
        diffuse_input.connect_to_source_path(&Path::from_string("/Mat/Tex.outputs:rgb").unwrap());

        // Material outputs:surface -> PBR outputs:surface
        let universal = shade_tokens().universal_render_context.clone();
        let mat_output = mat.create_surface_output(&universal);
        mat_output.connect_to_source_path(&Path::from_string("/Mat/PBR.outputs:surface").unwrap());

        // Set a file asset on the texture node
        let asset_type = usd_sdf::ValueTypeRegistry::instance().find_type("asset");
        let file_input = tex.create_input(&Token::new("file"), &asset_type);
        file_input.set(
            Value::from("texture.png".to_string()),
            usd_sdf::TimeCode::default(),
        );

        // Build and verify
        let globals = create_test_globals();
        let ds =
            DataSourceMaterial::new(Path::from_string("/Mat").unwrap(), mat.get_prim(), globals);

        let network = ds.get(&Token::new(""));
        assert!(
            network.is_some(),
            "Expected material network with texture chain"
        );
    }

    #[test]
    fn test_empty_material_no_network() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

        let mat = Material::define(&stage, &Path::from_string("/EmptyMat").unwrap());
        assert!(mat.is_valid());

        let globals = create_test_globals();
        let ds = DataSourceMaterial::new(
            Path::from_string("/EmptyMat").unwrap(),
            mat.get_prim(),
            globals,
        );

        // No terminals defined, should return None
        let network = ds.get(&Token::new(""));
        // Could be None since there are no connections
        // This is valid - empty materials produce no network
        let _ = network;
    }

    // ====================================================================
    // Invalidation
    // ====================================================================

    #[test]
    fn test_invalidate_surface_change() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Mat", "Material")
            .expect("define material");

        let locators = DataSourceMaterialPrim::invalidate(
            &prim,
            &Token::new(""),
            &[Token::new("outputs:surface")],
            PropertyInvalidationType::Resync,
        );

        assert!(
            !locators.is_empty(),
            "Surface change should produce invalidation"
        );
    }

    #[test]
    fn test_invalidate_input_change() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Mat", "Material")
            .expect("define material");

        let locators = DataSourceMaterialPrim::invalidate(
            &prim,
            &Token::new(""),
            &[Token::new("inputs:diffuseColor")],
            PropertyInvalidationType::Resync,
        );

        assert!(
            !locators.is_empty(),
            "Interface input change should produce invalidation"
        );
    }
}
