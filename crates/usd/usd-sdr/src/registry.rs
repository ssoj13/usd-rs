//! SDR Registry - Shader node registry for discovery and access.
//!
//! Port of pxr/usd/sdr/registry.h
//!
//! This module provides SdrRegistry, a singleton that provides access to
//! shader node information. Discovery plugins find nodes to include, and
//! parser plugins parse them on-demand when full node information is needed.
//!
//! Used by: Client code needing shader definitions
//! Uses: SdrShaderNode, SdrShaderNodeDiscoveryResult

use super::declare::{
    SdrIdentifier, SdrIdentifierVec, SdrStringVec, SdrTokenMap, SdrTokenVec, SdrVersion,
    SdrVersionFilter,
};
use super::discovery_result::SdrShaderNodeDiscoveryResult;
use super::parser_plugin::SdrParserPluginRef;
use super::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use super::shader_node_metadata::SdrShaderNodeMetadata;
use super::shader_node_query::{SdrShaderNodeArc, SdrShaderNodeQuery, SdrShaderNodeQueryResult};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{OnceLock, RwLock};
use usd_tf::Token;

/// Key for the node cache: (identifier, source_type).
type ShaderNodeMapKey = (SdrIdentifier, Token);

/// The shader node registry.
///
/// Provides access to shader node information. "Discovery Plugins" are responsible
/// for finding the nodes that should be included in the registry.
///
/// When the registry is first told about discovery plugins, they discover nodes
/// and generate `SdrShaderNodeDiscoveryResult` instances containing basic metadata.
/// Once the client asks for information requiring parsing (e.g., inputs/outputs),
/// the registry begins parsing on an as-needed basis.
///
/// # Thread Safety
///
/// The registry uses internal locking to allow concurrent access from multiple
/// threads.
///
/// # Example
///
/// ```ignore
/// let registry = SdrRegistry::get_instance();
///
/// // Get all shader identifiers
/// let ids = registry.get_shader_node_identifiers(None, SdrVersionFilter::DefaultOnly);
///
/// // Get a specific shader node
/// if let Some(node) = registry.get_shader_node_by_identifier(&id, &[]) {
///     println!("Found shader: {}", node.get_name());
/// }
/// ```
pub struct SdrRegistry {
    /// Discovery results keyed by identifier.
    discovery_results_by_identifier: RwLock<HashMap<Token, Vec<SdrShaderNodeDiscoveryResult>>>,

    /// Discovery results keyed by name (points to same data).
    discovery_results_by_name: RwLock<HashMap<String, Vec<usize>>>, // indices into by_identifier

    /// All source types discovered.
    all_source_types: RwLock<Vec<Token>>,

    /// Parsed node cache.
    node_map: RwLock<HashMap<ShaderNodeMapKey, SdrShaderNodeUniquePtr>>,

    /// Search URIs (where we look for shaders).
    search_uris: RwLock<SdrStringVec>,

    /// Parser plugins - maps discovery type to parser plugin.
    parser_plugin_map: RwLock<HashMap<Token, usize>>,

    /// Parser plugins storage (owns the plugins).
    parser_plugins: RwLock<Vec<SdrParserPluginRef>>,
}

impl SdrRegistry {
    /// Creates a new empty registry.
    fn new() -> Self {
        Self::new_isolated()
    }

    /// Creates a new isolated registry instance (not the singleton).
    ///
    /// Useful for testing without affecting the global singleton.
    pub fn new_isolated() -> Self {
        let registry = Self {
            discovery_results_by_identifier: RwLock::new(HashMap::new()),
            discovery_results_by_name: RwLock::new(HashMap::new()),
            all_source_types: RwLock::new(Vec::new()),
            node_map: RwLock::new(HashMap::new()),
            search_uris: RwLock::new(Vec::new()),
            parser_plugin_map: RwLock::new(HashMap::new()),
            parser_plugins: RwLock::new(Vec::new()),
        };

        // Register default parser plugins
        registry.register_default_parsers();

        registry
    }

    /// Gets the singleton registry instance.
    ///
    /// The registry is created on first access with default parsers registered.
    pub fn get_instance() -> &'static SdrRegistry {
        static REGISTRY: OnceLock<SdrRegistry> = OnceLock::new();
        REGISTRY.get_or_init(SdrRegistry::new)
    }

    /// Registers all built-in parser plugins.
    ///
    /// Called automatically during registry initialization.
    /// Registers:
    /// - `SdrArgsParserPlugin` for .args files (RenderMan)
    /// - `OslParserPlugin` (`osl_parser`) for compiled `.oso` OSL bytecode
    /// - `SdrOslParserPlugin` (`sdrosl_parser`) for JSON `.sdrOsl` definitions only
    /// - `UsdShadersParserPlugin` for built-in USD shaders
    fn register_default_parsers(&self) {
        use super::args_parser::SdrArgsParserPlugin;
        use super::osl_parser::OslParserPlugin;
        use super::sdrosl_parser::SdrOslParserPlugin;
        use super::usd_shaders::UsdShadersParserPlugin;

        // RenderMan Args parser (.args files)
        self.register_parser_plugin(Box::new(SdrArgsParserPlugin::new()));

        // Compiled .oso shaders (C++ `SdrOslParserPlugin` / oslParser.cpp) — register before JSON sdrOsl
        // so discovery type `oso` maps to bytecode parsing, not JSON.
        self.register_parser_plugin(Box::new(OslParserPlugin::new()));

        // JSON .sdrOsl shader definitions (no `oso` — see `sdrosl_parser`)
        self.register_parser_plugin(Box::new(SdrOslParserPlugin::new()));

        // Built-in USD shaders (UsdPreviewSurface, UsdUVTexture, etc.)
        self.register_parser_plugin(Box::new(UsdShadersParserPlugin::new()));

        log::debug!(
            "SDR: Registered {} default parser plugins",
            self.parser_plugins.read().expect("rwlock").len()
        );
    }

    /// Adds a discovery result to the registry.
    ///
    /// This method will not immediately spawn a parse call; parsing is deferred
    /// until a GetShaderNode*() method is called.
    pub fn add_discovery_result(&self, discovery_result: SdrShaderNodeDiscoveryResult) {
        let identifier = discovery_result.identifier.clone();
        let name = discovery_result.name.clone();
        let source_type = discovery_result.source_type.clone();

        // Add source type if new
        {
            let mut source_types = self.all_source_types.write().expect("rwlock poisoned");
            if !source_types.contains(&source_type) {
                source_types.push(source_type);
                source_types.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            }
        }

        // Add to by-identifier map
        {
            let mut by_id = self
                .discovery_results_by_identifier
                .write()
                .expect("rwlock poisoned");
            by_id.entry(identifier).or_default().push(discovery_result);
        }

        // Add to by-name index (simplified - just track that this name exists)
        {
            let mut by_name = self
                .discovery_results_by_name
                .write()
                .expect("rwlock poisoned");
            by_name.entry(name).or_default().push(0); // Placeholder index
        }
    }

    /// Get the locations where the registry is searching for nodes.
    pub fn get_search_uris(&self) -> SdrStringVec {
        self.search_uris.read().expect("rwlock poisoned").clone()
    }

    /// Adds a search URI to the registry.
    pub fn add_search_uri(&self, uri: String) {
        let mut uris = self.search_uris.write().expect("rwlock poisoned");
        if !uris.contains(&uri) {
            uris.push(uri);
        }
    }

    /// Registers a parser plugin.
    ///
    /// The plugin will be used to parse discovery results that have a
    /// `discovery_type` matching one of the types returned by the plugin's
    /// `get_discovery_types()` method.
    ///
    /// Note: This method cannot be called after nodes have been parsed.
    pub fn register_parser_plugin(&self, plugin: SdrParserPluginRef) {
        // Check if nodes have already been parsed
        {
            let node_map = self.node_map.read().expect("rwlock poisoned");
            if !node_map.is_empty() {
                // In C++ this would be TF_CODING_ERROR; we just log a warning
                eprintln!("Warning: register_parser_plugin() called after nodes parsed");
                return;
            }
        }

        let discovery_types = plugin.get_discovery_types();

        // Add plugin to storage
        let plugin_index = {
            let mut plugins = self.parser_plugins.write().expect("rwlock poisoned");
            let index = plugins.len();
            plugins.push(plugin);
            index
        };

        // Map each discovery type to this plugin
        let mut plugin_map = self.parser_plugin_map.write().expect("rwlock poisoned");
        for discovery_type in discovery_types {
            if plugin_map.contains_key(&discovery_type) {
                eprintln!(
                    "Warning: Discovery type '{}' already claimed by another parser",
                    discovery_type.as_str()
                );
                continue;
            }
            plugin_map.insert(discovery_type, plugin_index);
        }
    }

    /// Allows the client to set additional parser plugins that would
    /// otherwise NOT be found through the plugin system.
    ///
    /// Matches C++ `SdrRegistry::SetExtraParserPlugins`.
    ///
    /// Note: Cannot be called after any nodes have been parsed.
    pub fn set_extra_parser_plugins(&self, plugins: Vec<SdrParserPluginRef>) {
        {
            let node_map = self.node_map.read().expect("rwlock poisoned");
            if !node_map.is_empty() {
                eprintln!("Warning: set_extra_parser_plugins() called after nodes parsed");
                return;
            }
        }

        for plugin in plugins {
            self.register_parser_plugin(plugin);
        }
    }

    /// Allows the client to set additional discovery plugins that would
    /// otherwise NOT be found through the plugin system. Runs the discovery
    /// process for the specified plugins immediately.
    ///
    /// Note: Cannot be called after any nodes have been parsed.
    ///
    /// Matches C++ `SdrRegistry::SetExtraDiscoveryPlugins`.
    pub fn set_extra_discovery_plugins(
        &self,
        plugins: Vec<super::discovery_plugin::SdrDiscoveryPluginRef>,
    ) {
        // Check if nodes have already been parsed
        {
            let node_map = self.node_map.read().expect("rwlock poisoned");
            if !node_map.is_empty() {
                eprintln!("Warning: set_extra_discovery_plugins() called after nodes parsed");
                return;
            }
        }

        // Create a discovery context backed by this registry
        let context = RegistryDiscoveryContext { registry: self };

        // Run discovery on each plugin and add results
        for plugin in &plugins {
            // Add search URIs from the plugin
            for uri in plugin.get_search_uris() {
                self.add_search_uri(uri);
            }

            // Run discovery and add all results
            let results = plugin.discover_shader_nodes(&context);
            for result in results {
                self.add_discovery_result(result);
            }
        }
    }

    /// Returns the parser plugin for the given discovery type.
    fn get_parser_for_discovery_type(&self, discovery_type: &Token) -> Option<usize> {
        let plugin_map = self.parser_plugin_map.read().expect("rwlock poisoned");
        plugin_map.get(discovery_type).copied()
    }

    /// Returns the source type for a given discovery type.
    ///
    /// This is used by discovery plugins to determine the source type
    /// for discovered nodes.
    pub fn get_source_type_for_discovery_type(&self, discovery_type: &Token) -> Option<Token> {
        let plugin_index = self.get_parser_for_discovery_type(discovery_type)?;
        let plugins = self.parser_plugins.read().expect("rwlock poisoned");
        plugins.get(plugin_index).map(|p| p.get_source_type())
    }

    /// Get identifiers of all shader nodes that the registry is aware of.
    ///
    /// This will not run parsing plugins on discovered nodes, so this method
    /// is relatively quick. Optionally, a "family" name can be specified to
    /// only get identifiers of nodes that belong to that family.
    pub fn get_shader_node_identifiers(
        &self,
        family: Option<&Token>,
        filter: SdrVersionFilter,
    ) -> SdrIdentifierVec {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        let mut result = SdrIdentifierVec::new();

        for (id, results) in by_id.iter() {
            for dr in results {
                // Filter by family if specified
                if let Some(fam) = family {
                    if !fam.as_str().is_empty() && &dr.family != fam {
                        continue;
                    }
                }

                // Filter by version
                match filter {
                    SdrVersionFilter::DefaultOnly => {
                        if dr.version.is_default() || !dr.version.is_valid() {
                            result.push(id.clone());
                            break; // Only add once per identifier
                        }
                    }
                    SdrVersionFilter::AllVersions => {
                        result.push(id.clone());
                        break;
                    }
                }
            }
        }

        result
    }

    /// Get the names of all shader nodes that the registry is aware of.
    ///
    /// This will not run parsing plugins on discovered nodes.
    pub fn get_shader_node_names(&self, family: Option<&Token>) -> SdrStringVec {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        let mut names = std::collections::HashSet::new();

        for results in by_id.values() {
            for dr in results {
                // Filter by family if specified
                if let Some(fam) = family {
                    if !fam.as_str().is_empty() && &dr.family != fam {
                        continue;
                    }
                }
                names.insert(dr.name.clone());
            }
        }

        names.into_iter().collect()
    }

    /// Get the shader node with the specified identifier.
    ///
    /// If no `type_priority` is specified, the first encountered node with
    /// the specified identifier will be returned (first is arbitrary) if found.
    ///
    /// If a `type_priority` list is specified, this will iterate through each
    /// source type and try to find a matching node.
    ///
    /// Returns None if a node matching the arguments can't be found.
    pub fn get_shader_node_by_identifier(
        &self,
        identifier: &SdrIdentifier,
        type_priority: &SdrTokenVec,
    ) -> Option<&SdrShaderNode> {
        if type_priority.is_empty() {
            // Get any node with this identifier
            let by_id = self
                .discovery_results_by_identifier
                .read()
                .expect("rwlock poisoned");
            if let Some(results) = by_id.get(identifier) {
                if let Some(dr) = results.first() {
                    return self.find_or_parse_node(dr);
                }
            }
        } else {
            // Try each source type in priority order
            for source_type in type_priority {
                if let Some(node) =
                    self.get_shader_node_by_identifier_and_type(identifier, source_type)
                {
                    return Some(node);
                }
            }
        }
        None
    }

    /// Get the shader node with the specified identifier and source type.
    ///
    /// Returns None if there is no matching node.
    pub fn get_shader_node_by_identifier_and_type(
        &self,
        identifier: &SdrIdentifier,
        source_type: &Token,
    ) -> Option<&SdrShaderNode> {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        if let Some(results) = by_id.get(identifier) {
            for dr in results {
                if &dr.source_type == source_type {
                    return self.find_or_parse_node(dr);
                }
            }
        }
        None
    }

    /// Get the shader node with the specified name.
    ///
    /// An optional priority list specifies source types to search and in what order.
    pub fn get_shader_node_by_name(
        &self,
        name: &str,
        type_priority: &SdrTokenVec,
        filter: SdrVersionFilter,
    ) -> Option<&SdrShaderNode> {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");

        // Find all discovery results with this name
        let mut matching: Vec<&SdrShaderNodeDiscoveryResult> = Vec::new();
        for results in by_id.values() {
            for dr in results {
                if dr.name == name {
                    // Apply version filter
                    match filter {
                        SdrVersionFilter::DefaultOnly => {
                            if dr.version.is_default() || !dr.version.is_valid() {
                                matching.push(dr);
                            }
                        }
                        SdrVersionFilter::AllVersions => {
                            matching.push(dr);
                        }
                    }
                }
            }
        }

        if matching.is_empty() {
            return None;
        }

        if type_priority.is_empty() {
            // Return first match
            self.find_or_parse_node(matching[0])
        } else {
            // Try each source type in priority order
            for source_type in type_priority {
                for dr in &matching {
                    if &dr.source_type == source_type {
                        return self.find_or_parse_node(dr);
                    }
                }
            }
            None
        }
    }

    /// Get the shader node with the specified name and source type.
    pub fn get_shader_node_by_name_and_type(
        &self,
        name: &str,
        source_type: &Token,
        filter: SdrVersionFilter,
    ) -> Option<&SdrShaderNode> {
        self.get_shader_node_by_name(name, &vec![source_type.clone()], filter)
    }

    /// Get all shader nodes matching the given identifier.
    ///
    /// Multiple nodes of the same identifier but different source types may exist.
    pub fn get_shader_nodes_by_identifier(
        &self,
        identifier: &SdrIdentifier,
    ) -> Vec<&SdrShaderNode> {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        let mut result = Vec::new();

        if let Some(results) = by_id.get(identifier) {
            for dr in results {
                if let Some(node) = self.find_or_parse_node(dr) {
                    result.push(node);
                }
            }
        }

        result
    }

    /// Get all shader nodes matching the given name.
    pub fn get_shader_nodes_by_name(
        &self,
        name: &str,
        filter: SdrVersionFilter,
    ) -> Vec<&SdrShaderNode> {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        let mut result = Vec::new();

        for results in by_id.values() {
            for dr in results {
                if dr.name == name {
                    // Apply version filter
                    let include = match filter {
                        SdrVersionFilter::DefaultOnly => {
                            dr.version.is_default() || !dr.version.is_valid()
                        }
                        SdrVersionFilter::AllVersions => true,
                    };

                    if include {
                        if let Some(node) = self.find_or_parse_node(dr) {
                            result.push(node);
                        }
                    }
                }
            }
        }

        result
    }

    /// Get all shader nodes, optionally restricted to a family and/or default version.
    ///
    /// Note: This will parse ALL nodes that the registry is aware of (unless a
    /// family is specified), so this may take some time on first call.
    pub fn get_shader_nodes_by_family(
        &self,
        family: Option<&Token>,
        filter: SdrVersionFilter,
    ) -> Vec<&SdrShaderNode> {
        let by_id = self
            .discovery_results_by_identifier
            .read()
            .expect("rwlock poisoned");
        let mut result = Vec::new();

        for results in by_id.values() {
            for dr in results {
                // Filter by family
                if let Some(fam) = family {
                    if !fam.as_str().is_empty() && &dr.family != fam {
                        continue;
                    }
                }

                // Filter by version
                let include = match filter {
                    SdrVersionFilter::DefaultOnly => {
                        dr.version.is_default() || !dr.version.is_valid()
                    }
                    SdrVersionFilter::AllVersions => true,
                };

                if include {
                    if let Some(node) = self.find_or_parse_node(dr) {
                        result.push(node);
                    }
                }
            }
        }

        result
    }

    /// Parses all unparsed shader nodes and returns all shader nodes.
    ///
    /// First invocation is potentially expensive depending on parser plugins
    /// and number of nodes.
    pub fn get_all_shader_nodes(&self) -> Vec<&SdrShaderNode> {
        self.get_shader_nodes_by_family(None, SdrVersionFilter::AllVersions)
    }

    /// Get a sorted list of all shader node source types that may be present.
    ///
    /// Source types originate from the discovery process, but there's no guarantee
    /// that discovered source types will also have a registered parser plugin.
    pub fn get_all_shader_node_source_types(&self) -> SdrTokenVec {
        self.all_source_types
            .read()
            .expect("rwlock poisoned")
            .clone()
    }

    /// Parses all unparsed shader nodes.
    ///
    /// Front-loads node parsing so subsequent calls to node getters don't incur
    /// the parsing cost.
    pub fn parse_all(&self) {
        let _ = self.get_all_shader_nodes();
    }

    /// Run an SdrShaderNodeQuery.
    ///
    /// Note: This will cause all nodes in the registry to be parsed in order
    /// to examine data on these nodes in their final form.
    pub fn run_query(&self, query: &SdrShaderNodeQuery) -> SdrShaderNodeQueryResult {
        use std::collections::HashSet;
        use std::sync::Arc;

        // Get all nodes and filter by query constraints
        let all_nodes = self.get_all_shader_nodes();
        let mut matching_nodes: Vec<SdrShaderNodeArc> = Vec::new();

        for node in all_nodes {
            if query.matches(node) {
                // Clone node into Arc for query result
                // Note: In full implementation would need better ownership
                matching_nodes.push(Arc::new(node.clone()));
            }
        }

        // Sort by identifier, then source type
        matching_nodes.sort_by(|a, b| {
            let id_cmp = a.get_identifier().as_str().cmp(b.get_identifier().as_str());
            if id_cmp == std::cmp::Ordering::Equal {
                a.get_source_type()
                    .as_str()
                    .cmp(b.get_source_type().as_str())
            } else {
                id_cmp
            }
        });

        let select_keys = query.get_select_keys();

        if select_keys.is_empty() {
            // No SelectDistinct - return all matching nodes in single group
            return SdrShaderNodeQueryResult::with_data(
                Vec::new(),
                Vec::new(),
                vec![matching_nodes],
            );
        }

        // SelectDistinct specified - group by distinct value combinations
        let mut seen_combos: HashSet<Vec<String>> = HashSet::new();
        let mut values: Vec<Vec<usd_vt::Value>> = Vec::new();
        let mut nodes_by_values: Vec<Vec<SdrShaderNodeArc>> = Vec::new();

        for node in matching_nodes {
            // Extract values for select keys
            let value_row: Vec<usd_vt::Value> = select_keys
                .iter()
                .map(|key| node.get_data_for_key(key))
                .collect();

            // Create string key for deduplication
            let combo_key: Vec<String> = value_row.iter().map(|v| format!("{:?}", v)).collect();

            if seen_combos.contains(&combo_key) {
                // Add to existing group
                let idx = values
                    .iter()
                    .position(|row| {
                        row.iter()
                            .zip(&combo_key)
                            .all(|(v, k)| format!("{:?}", v) == *k)
                    })
                    .expect(
                        "combo_key must exist in values since it was just checked in seen_combos",
                    );
                nodes_by_values[idx].push(node);
            } else {
                // New combination
                seen_combos.insert(combo_key);
                values.push(value_row);
                nodes_by_values.push(vec![node]);
            }
        }

        SdrShaderNodeQueryResult::with_data(select_keys.clone(), values, nodes_by_values)
    }

    /// Registers a pre-parsed shader node directly.
    ///
    /// This is useful for programmatically created nodes that don't come from
    /// discovery or parsing.
    pub fn register_shader_node(&self, node: SdrShaderNodeUniquePtr) {
        let key = (
            node.get_identifier().clone(),
            node.get_source_type().clone(),
        );
        let mut node_map = self.node_map.write().expect("rwlock poisoned");
        node_map.insert(key, node);
    }

    /// Parses the given asset, constructs a SdrShaderNode from it and adds it to the registry.
    ///
    /// Nodes created from an asset using this API can be looked up by the unique identifier
    /// and source_type of the returned node, or by URI, which will be set to the unresolved
    /// asset path value.
    ///
    /// # Arguments
    /// * `asset_path` - Path to the shader asset file
    /// * `resolved_path` - Resolved path (can be same as asset_path if already resolved)
    /// * `metadata` - Additional metadata for parsing; supplements asset metadata
    /// * `sub_identifier` - Optional sub-identifier for multi-definition assets
    /// * `source_type` - Optional source type; if empty, uses parser's source type
    ///
    /// # Returns
    /// A reference to the parsed shader node, or None if parsing fails.
    pub fn get_shader_node_from_asset(
        &self,
        asset_path: &str,
        resolved_path: Option<&str>,
        metadata: &SdrTokenMap,
        sub_identifier: Option<&Token>,
        source_type: Option<&Token>,
    ) -> Option<&SdrShaderNode> {
        let resolved = resolved_path.unwrap_or(asset_path);

        // Get discovery type from file extension
        let discovery_type = get_file_extension(resolved);
        if discovery_type.is_empty() {
            eprintln!(
                "Warning: Cannot determine discovery type for asset: {}",
                asset_path
            );
            return None;
        }
        let discovery_type_token = Token::new(&discovery_type);

        // Find parser plugin for this discovery type
        let plugin_index = self.get_parser_for_discovery_type(&discovery_type_token)?;
        let plugins = self.parser_plugins.read().expect("rwlock poisoned");
        let parser = plugins.get(plugin_index)?;

        // Determine source type: use provided or get from parser
        let actual_source_type = source_type
            .filter(|t| !t.as_str().is_empty())
            .cloned()
            .unwrap_or_else(|| parser.get_source_type());

        // Generate identifier from hash
        let identifier = gen_id_for_asset(asset_path, metadata, sub_identifier, source_type);

        // Check if node already exists
        if let Some(node) =
            self.get_shader_node_by_identifier_and_type(&identifier, &actual_source_type)
        {
            return Some(node);
        }

        // Extract base name for node name
        let name = get_base_name(resolved);

        // Build discovery result
        let mut dr = SdrShaderNodeDiscoveryResult::new(
            identifier.clone(),
            SdrVersion::invalid(),
            name,
            Token::default(), // family
            discovery_type_token,
            actual_source_type.clone(),
            asset_path.to_string(),
            resolved.to_string(),
            String::new(), // source_code
            metadata.clone(),
            String::new(), // blind_data
            sub_identifier.cloned().unwrap_or_default(),
        );

        // Merge provided metadata
        for (k, v) in metadata {
            dr.metadata.insert(k.clone(), v.clone());
        }

        // Parse using plugin
        drop(plugins); // Release lock before parsing
        let plugins = self.parser_plugins.read().expect("rwlock poisoned");
        let parser = plugins.get(plugin_index)?;
        let node = parser.parse_shader_node(&dr)?;

        // Register node and discovery result
        let key = (identifier.clone(), actual_source_type.clone());

        // Add discovery result
        drop(plugins);
        self.add_discovery_result(dr);

        // Insert node into cache
        {
            let mut node_map = self.node_map.write().expect("rwlock poisoned");
            node_map.insert(key.clone(), node);
        }

        // Return reference to cached node
        self.get_node_from_cache(&key)
    }

    /// Parses shader node from source code string.
    ///
    /// Constructs a SdrShaderNode from the given source code and adds it to the registry.
    /// The parser to use is determined by the specified source_type.
    ///
    /// # Arguments
    /// * `source_code` - The shader source code string
    /// * `source_type` - The source type (must match a registered parser)
    /// * `metadata` - Additional metadata for parsing
    ///
    /// # Returns
    /// A reference to the parsed shader node, or None if parsing fails.
    pub fn get_shader_node_from_source_code(
        &self,
        source_code: &str,
        source_type: &Token,
        metadata: &SdrTokenMap,
    ) -> Option<&SdrShaderNode> {
        // Find parser for this source type
        let parser_index = {
            let plugins = self.parser_plugins.read().expect("rwlock poisoned");
            plugins
                .iter()
                .position(|p| &p.get_source_type() == source_type)
        }?;

        // Generate identifier from hash
        let identifier = gen_id_for_source_code(source_code, metadata);

        // Check if node already exists
        if let Some(node) = self.get_shader_node_by_identifier_and_type(&identifier, source_type) {
            return Some(node);
        }

        // Build discovery result
        let dr = SdrShaderNodeDiscoveryResult::new(
            identifier.clone(),
            SdrVersion::invalid(),
            identifier.as_str().to_string(), // use hash as name
            Token::default(),                // family
            source_type.clone(),             // discovery_type
            source_type.clone(),
            String::new(), // uri
            String::new(), // resolved_uri
            source_code.to_string(),
            metadata.clone(),
            String::new(),    // blind_data
            Token::default(), // sub_identifier
        );

        // Parse using plugin
        let node = {
            let plugins = self.parser_plugins.read().expect("rwlock poisoned");
            let parser = plugins.get(parser_index)?;
            parser.parse_shader_node(&dr)?
        };

        // Insert into cache
        let key = (identifier.clone(), source_type.clone());
        {
            let mut node_map = self.node_map.write().expect("rwlock poisoned");
            node_map.insert(key.clone(), node);
        }

        // Add discovery result for future lookups
        self.add_discovery_result(dr);

        // Return reference to cached node
        self.get_node_from_cache(&key)
    }

    // ========================================================================
    // Internal methods
    // ========================================================================

    /// Gets a node reference from the cache.
    ///
    /// # Safety
    /// This extends the lifetime of the returned reference beyond the lock guard.
    /// This is safe because:
    /// 1. The node_map is never cleared or shrunk
    /// 2. Entries are never removed once added
    /// 3. The registry lives for 'static
    #[allow(unsafe_code)]
    fn get_node_from_cache(&self, key: &ShaderNodeMapKey) -> Option<&SdrShaderNode> {
        let node_map = self.node_map.read().expect("rwlock poisoned");
        if node_map.contains_key(key) {
            // SAFETY: See function doc comment
            type NodeMap = HashMap<ShaderNodeMapKey, SdrShaderNodeUniquePtr>;
            let node_map: &NodeMap = unsafe { &*(&*node_map as *const NodeMap) };
            return node_map.get(key).map(|n| n.as_ref());
        }
        None
    }

    /// Finds an existing node in cache or parses a new one.
    fn find_or_parse_node(&self, dr: &SdrShaderNodeDiscoveryResult) -> Option<&SdrShaderNode> {
        let key = (dr.identifier.clone(), dr.source_type.clone());

        // Check cache first
        if let Some(node) = self.get_node_from_cache(&key) {
            return Some(node);
        }

        // Parse the node (simplified - in real implementation, this would use parser plugins)
        let node = self.parse_node(dr)?;

        // Insert into cache
        {
            let mut node_map = self.node_map.write().expect("rwlock poisoned");
            node_map.insert(key.clone(), node);
        }

        // Return reference to cached node
        self.get_node_from_cache(&key)
    }

    /// Parses a node from a discovery result.
    ///
    /// Delegates to the appropriate parser plugin based on the discovery type.
    /// If no parser is found, creates a basic node structure.
    fn parse_node(&self, dr: &SdrShaderNodeDiscoveryResult) -> Option<SdrShaderNodeUniquePtr> {
        // Try to find a parser plugin for this discovery type
        if let Some(plugin_index) = self.get_parser_for_discovery_type(&dr.discovery_type) {
            let plugins = self.parser_plugins.read().expect("rwlock poisoned");
            if let Some(plugin) = plugins.get(plugin_index) {
                return plugin.parse_shader_node(dr);
            }
        }

        // No parser found - create a basic node from discovery result metadata
        self.create_fallback_node(dr)
    }

    /// Creates a fallback node when no parser plugin is available.
    ///
    /// This creates a basic node with no properties, useful for representing
    /// discovered nodes that couldn't be fully parsed.
    fn create_fallback_node(
        &self,
        dr: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let metadata = SdrShaderNodeMetadata::from_token_map(&dr.metadata);

        let node = SdrShaderNode::new(
            dr.identifier.clone(),
            dr.version,
            dr.name.clone(),
            dr.family.clone(),
            Token::default(), // context would come from parser
            dr.source_type.clone(),
            dr.uri.clone(),
            dr.resolved_uri.clone(),
            Vec::new(), // no properties without parser
            metadata,
            dr.source_code.clone(),
        );

        Some(Box::new(node))
    }
}

/// Internal discovery context backed by the registry.
///
/// Implements SdrDiscoveryPluginContext to provide source type mapping
/// from discovery types via the registry's parser plugins.
struct RegistryDiscoveryContext<'a> {
    registry: &'a SdrRegistry,
}

impl<'a> super::discovery_plugin::SdrDiscoveryPluginContext for RegistryDiscoveryContext<'a> {
    fn get_source_type(&self, discovery_type: &Token) -> Token {
        self.registry
            .get_source_type_for_discovery_type(discovery_type)
            .unwrap_or_else(|| discovery_type.clone())
    }
}

// Ensure the registry is safe to share between threads
#[allow(unsafe_code)]
unsafe impl Send for SdrRegistry {}
#[allow(unsafe_code)]
unsafe impl Sync for SdrRegistry {}

// ============================================================================
// Helper functions for identifier generation
// ============================================================================

/// Generates a unique identifier for a shader asset.
///
/// The identifier is generated from a hash of the asset path, metadata,
/// sub-identifier, and source type.
fn gen_id_for_asset(
    asset_path: &str,
    metadata: &SdrTokenMap,
    sub_identifier: Option<&Token>,
    source_type: Option<&Token>,
) -> Token {
    let mut hasher = DefaultHasher::new();
    asset_path.hash(&mut hasher);

    // Hash metadata entries in sorted order for consistency
    let mut meta_entries: Vec<_> = metadata.iter().collect();
    meta_entries.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in meta_entries {
        k.as_str().hash(&mut hasher);
        v.hash(&mut hasher);
    }

    let hash = hasher.finish();
    let sub_id = sub_identifier.map(|t| t.as_str()).unwrap_or("");
    let src_type = source_type.map(|t| t.as_str()).unwrap_or("");

    Token::new(&format!("{}<{}><{}>", hash, sub_id, src_type))
}

/// Generates a unique identifier for shader source code.
///
/// The identifier is generated from a hash of the source code and metadata.
fn gen_id_for_source_code(source_code: &str, metadata: &SdrTokenMap) -> Token {
    let mut hasher = DefaultHasher::new();
    source_code.hash(&mut hasher);

    // Hash metadata entries in sorted order for consistency
    let mut meta_entries: Vec<_> = metadata.iter().collect();
    meta_entries.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in meta_entries {
        k.as_str().hash(&mut hasher);
        v.hash(&mut hasher);
    }

    Token::new(&hasher.finish().to_string())
}

/// Extracts file extension from a path.
fn get_file_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// Extracts base name (filename without extension) from a path.
fn get_base_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::declare::SdrVersion;
    use super::super::parser_plugin::SdrPassthroughParserPlugin;
    use super::*;

    #[test]
    fn test_add_and_get_discovery_result() {
        let registry = SdrRegistry::new();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test_shader"),
            SdrVersion::new(1, 0),
            "test_shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader.osl".to_string(),
            "/path/to/shader.osl".to_string(),
        );

        registry.add_discovery_result(dr);

        let ids = registry.get_shader_node_identifiers(None, SdrVersionFilter::AllVersions);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "test_shader");
    }

    #[test]
    fn test_get_shader_node_names() {
        let registry = SdrRegistry::new();

        let dr1 = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("shader_v1"),
            SdrVersion::new(1, 0),
            "shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader_v1.osl".to_string(),
            "/path/to/shader_v1.osl".to_string(),
        );

        let dr2 = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("shader_v2"),
            SdrVersion::new(2, 0),
            "shader".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader_v2.osl".to_string(),
            "/path/to/shader_v2.osl".to_string(),
        );

        registry.add_discovery_result(dr1);
        registry.add_discovery_result(dr2);

        let names = registry.get_shader_node_names(None);
        assert_eq!(names.len(), 1); // Same name, different versions
        assert!(names.contains(&"shader".to_string()));
    }

    #[test]
    fn test_source_types() {
        let registry = SdrRegistry::new();

        let dr1 = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("shader1"),
            SdrVersion::new(1, 0),
            "shader1".to_string(),
            Token::new("osl"),
            Token::new("OSL"),
            "/path/to/shader1.osl".to_string(),
            "/path/to/shader1.osl".to_string(),
        );

        let dr2 = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("shader2"),
            SdrVersion::new(1, 0),
            "shader2".to_string(),
            Token::new("glslfx"),
            Token::new("glslfx"),
            "/path/to/shader2.glslfx".to_string(),
            "/path/to/shader2.glslfx".to_string(),
        );

        registry.add_discovery_result(dr1);
        registry.add_discovery_result(dr2);

        let source_types = registry.get_all_shader_node_source_types();
        assert_eq!(source_types.len(), 2);
    }

    #[test]
    fn test_register_parser_plugin() {
        let registry = SdrRegistry::new();

        // Register a passthrough parser for OSL files
        let parser = SdrPassthroughParserPlugin::new(vec![Token::new("osl")], Token::new("OSL"));
        registry.register_parser_plugin(Box::new(parser));

        // Check that we can get the source type for the discovery type
        let source_type = registry.get_source_type_for_discovery_type(&Token::new("osl"));
        assert!(source_type.is_some());
        assert_eq!(source_type.unwrap().as_str(), "OSL");

        // Unknown discovery type should return None
        let unknown = registry.get_source_type_for_discovery_type(&Token::new("unknown"));
        assert!(unknown.is_none());
    }

    #[test]
    fn test_parser_plugin_parses_nodes() {
        let registry = SdrRegistry::new();

        // Register parser before adding discovery results
        let parser = SdrPassthroughParserPlugin::new(vec![Token::new("osl")], Token::new("OSL"));
        registry.register_parser_plugin(Box::new(parser));

        // Add a discovery result with matching discovery type
        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test_shader"),
            SdrVersion::new(1, 0),
            "test_shader".to_string(),
            Token::new("osl"), // discovery type matches registered parser
            Token::new("OSL"),
            "/path/to/shader.osl".to_string(),
            "/path/to/shader.osl".to_string(),
        );
        registry.add_discovery_result(dr);

        // Request the node - should trigger parsing via our plugin
        let node = registry.get_shader_node_by_identifier(&Token::new("test_shader"), &vec![]);
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.get_identifier().as_str(), "test_shader");
        assert_eq!(node.get_source_type().as_str(), "OSL");
    }

    #[test]
    fn test_multiple_parser_plugins() {
        let registry = SdrRegistry::new();

        // Register parsers for different types
        let osl_parser = SdrPassthroughParserPlugin::new(
            vec![Token::new("osl"), Token::new("oso")],
            Token::new("OSL"),
        );
        registry.register_parser_plugin(Box::new(osl_parser));

        let glslfx_parser =
            SdrPassthroughParserPlugin::new(vec![Token::new("glslfx")], Token::new("glslfx"));
        registry.register_parser_plugin(Box::new(glslfx_parser));

        // Verify both are registered
        assert!(
            registry
                .get_source_type_for_discovery_type(&Token::new("osl"))
                .is_some()
        );
        assert!(
            registry
                .get_source_type_for_discovery_type(&Token::new("oso"))
                .is_some()
        );
        assert!(
            registry
                .get_source_type_for_discovery_type(&Token::new("glslfx"))
                .is_some()
        );
    }

    #[test]
    fn test_set_extra_parser_plugins() {
        let registry = SdrRegistry::new();

        // Before any nodes are parsed, set_extra_parser_plugins should work
        let osl_parser =
            SdrPassthroughParserPlugin::new(vec![Token::new("osl")], Token::new("OSL"));
        let glslfx_parser =
            SdrPassthroughParserPlugin::new(vec![Token::new("glslfx")], Token::new("glslfx"));

        registry.set_extra_parser_plugins(vec![Box::new(osl_parser), Box::new(glslfx_parser)]);

        // Both plugins registered
        assert!(
            registry
                .get_source_type_for_discovery_type(&Token::new("osl"))
                .is_some()
        );
        assert!(
            registry
                .get_source_type_for_discovery_type(&Token::new("glslfx"))
                .is_some()
        );
    }
}
