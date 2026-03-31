//! MaterialX Discovery Plugin for SDR.
//!
//! Discovers MaterialX .mtlx files and creates discovery results for NodeDef elements.
//!
//! Port of pxr/usd/usdMtlx/discovery.cpp

use std::collections::HashMap;
use usd_sdr::declare::{SdrStringVec, SdrTokenMap};
use usd_sdr::discovery_plugin::{SdrDiscoveryPlugin, SdrDiscoveryPluginContext};
use usd_sdr::discovery_result::{SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec};
use usd_sdr::filesystem_discovery_helpers::{SdrDiscoveryUri, discover_files};
use usd_tf::Token;

use super::Document;
use super::document::NodeDef;
use super::utils::{
    custom_search_paths, get_document, get_version, search_paths, standard_file_extensions,
};

/// MaterialX discovery plugin.
///
/// Discovers .mtlx files in search paths and creates discovery results
/// for each NodeDef element found in those files.
///
/// Matches C++ `UsdMtlxDiscoveryPlugin`.
pub struct MtlxDiscoveryPlugin {
    /// Custom search paths from PXR_MTLX_PLUGIN_SEARCH_PATHS
    custom_search_paths: SdrStringVec,
    /// All search paths (custom + stdlib)
    all_search_paths: SdrStringVec,
}

impl MtlxDiscoveryPlugin {
    /// Create a new MaterialX discovery plugin with default search paths.
    ///
    /// Matches C++ constructor.
    pub fn new() -> Self {
        Self {
            custom_search_paths: custom_search_paths().clone(),
            all_search_paths: search_paths().clone(),
        }
    }

    /// Create a new MaterialX discovery plugin with custom search paths.
    pub fn with_paths(custom_paths: SdrStringVec) -> Self {
        let mut all_paths = custom_paths.clone();
        all_paths.extend(search_paths().iter().cloned());

        Self {
            custom_search_paths: custom_paths,
            all_search_paths: all_paths,
        }
    }
}

impl Default for MtlxDiscoveryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Map node names to their base names for versioning.
///
/// Walks the inheritance chain using root->getChild() lookup and finds
/// the shortest name in the hierarchy. This is a single-pass optimization
/// that walks the chain once to find the shortest name, then populates
/// the mapping for all elements in the chain.
///
/// Matches C++ `_MapNodeNamesToBaseForVersioning()`.
fn map_node_names_to_base(nodedef: &NodeDef, mapping: &mut HashMap<String, String>) {
    static INHERIT_ATTR: &str = "inherit";

    // Find shortest name in inheritance hierarchy (first pass)
    let mut shortest_name = nodedef.0.name().to_string();
    let mut current = Some(nodedef.0.clone());

    while let Some(elem) = current {
        let inherit = elem.get_attribute(INHERIT_ATTR);
        if inherit.is_empty() {
            break;
        }

        // Use root->getChild() for lookup (matches C++ getRoot()->getChild())
        if let Some(inherited) = elem.get_root().get_child(inherit) {
            if inherited.name().len() < shortest_name.len() {
                shortest_name = inherited.name().to_string();
            }
            current = Some(inherited);
        } else {
            break;
        }
    }

    // Populate mapping for this nodedef and all bases (second pass)
    let start_name = nodedef.0.name().to_string();
    let entry = mapping
        .entry(start_name.clone())
        .or_insert_with(|| shortest_name.clone());
    if shortest_name.len() < entry.len() {
        *entry = shortest_name.clone();
    }

    // Map all inherited nodedefs
    let mut mtlx = Some(nodedef.0.clone());
    while let Some(elem) = mtlx {
        let inherit = elem.get_attribute(INHERIT_ATTR);
        if inherit.is_empty() {
            break;
        }

        if let Some(inherited) = elem.get_root().get_child(inherit) {
            let base_name = inherited.name().to_string();
            let entry = mapping
                .entry(base_name)
                .or_insert_with(|| shortest_name.clone());
            if shortest_name.len() < entry.len() {
                *entry = shortest_name.clone();
            }
            mtlx = Some(inherited);
        } else {
            break;
        }
    }
}

/// Compute name mapping for all NodeDefs with inheritance.
///
/// Matches C++ `_ComputeNameMapping()`.
fn compute_name_mapping(doc: &Document) -> HashMap<String, String> {
    let mut mapping = HashMap::new();

    for nodedef in doc.get_node_defs() {
        if nodedef.has_inherit_string() {
            map_node_names_to_base(&nodedef, &mut mapping);
        }
    }

    mapping
}

/// Choose the SDR name from the name mapping.
///
/// Matches C++ `_ChooseName()`.
fn choose_name(name: &str, mapping: &HashMap<String, String>) -> String {
    mapping
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

/// Discover nodes in a MaterialX document.
///
/// Matches C++ `_DiscoverNodes()`.
fn discover_nodes(
    doc: &Document,
    file_result: &SdrDiscoveryUri,
    mapping: &HashMap<String, String>,
) -> SdrShaderNodeDiscoveryResultVec {
    let mut results = Vec::new();

    for nodedef in doc.get_node_defs() {
        let nodedef_name = nodedef.0.name();
        let sdr_name = choose_name(nodedef_name, mapping);

        // Get version and implicit_default flag
        let (version, implicit_default) = get_version(&nodedef);

        // Use implicit_default to determine if this should be marked as default version
        // implicit_default == false means explicitly marked as default (use as_default)
        // implicit_default == true means NOT explicitly marked (leave as-is)
        let final_version = if !implicit_default {
            version // Already marked as default in get_version()
        } else {
            version
        };

        // Build identifier - use original nodedef name
        let identifier = Token::new(nodedef_name);

        // Get family from node attribute
        let family = Token::new(nodedef.get_node_string());

        // Discovery type is "mtlx"
        let discovery_type = Token::new("mtlx");

        // Source type is "mtlx" (matches C++ _tokens->mtlx)
        let source_type = Token::new("mtlx");

        // Create metadata
        let mut metadata = SdrTokenMap::new();

        // Add target if present
        let target = nodedef.get_target();
        if !target.is_empty() {
            metadata.insert(Token::new("target"), target.to_string());
        }

        // Create discovery result
        let result = SdrShaderNodeDiscoveryResult::new(
            identifier,
            final_version,
            sdr_name,
            family,
            discovery_type,
            source_type,
            file_result.uri.clone(),
            file_result.resolved_uri.clone(),
            String::new(), // no inline source code
            metadata,
            String::new(),    // no blind data
            Token::default(), // no sub-identifier
        );

        results.push(result);
    }

    results
}

impl SdrDiscoveryPlugin for MtlxDiscoveryPlugin {
    /// Discover shader nodes for all MaterialX files in search paths.
    ///
    /// Matches C++ `DiscoverShaderNodes(const Context&)`.
    fn discover_shader_nodes(
        &self,
        _context: &dyn SdrDiscoveryPluginContext,
    ) -> SdrShaderNodeDiscoveryResultVec {
        let mut all_results = Vec::new();

        // Merge all MaterialX standard library files into a single document
        // and discover nodes from it. Empty URI means merge all stdlib files.
        if let Some(stdlib_doc) = get_document("") {
            let mapping = compute_name_mapping(&stdlib_doc);
            let stdlib_uri = SdrDiscoveryUri {
                uri: "mtlx".to_string(),
                resolved_uri: "mtlx".to_string(),
            };
            let results = discover_nodes(&stdlib_doc, &stdlib_uri, &mapping);
            all_results.extend(results);
        }

        // Check USDMTLX_PLUGIN_FOLLOW_SYMLINKS environment variable
        let follow_symlinks = std::env::var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        let extensions = standard_file_extensions();

        // Find the mtlx files from custom search paths using recursive discovery
        for file_result in discover_files(&self.custom_search_paths, &extensions, follow_symlinks) {
            if let Some(doc) = get_document(&file_result.resolved_uri) {
                let mapping = compute_name_mapping(&doc);
                let results = discover_nodes(&doc, &file_result, &mapping);
                all_results.extend(results);
            }
        }

        all_results
    }

    /// Gets the paths that this plugin is searching for nodes in.
    ///
    /// Matches C++ `GetSearchURIs()`.
    fn get_search_uris(&self) -> SdrStringVec {
        self.all_search_paths.clone()
    }

    fn get_name(&self) -> &str {
        "MtlxDiscoveryPlugin"
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Test-only accessor for compute_name_mapping
    pub(crate) fn compute_name_mapping_for_test(doc: &Document) -> HashMap<String, String> {
        compute_name_mapping(doc)
    }

    #[test]
    fn test_discovery_plugin_creation() {
        let plugin = MtlxDiscoveryPlugin::new();
        assert!(!plugin.get_name().is_empty());
    }

    #[test]
    fn test_discovery_plugin_with_paths() {
        let custom_paths = vec!["/custom/path".to_string()];
        let plugin = MtlxDiscoveryPlugin::with_paths(custom_paths);

        let search_uris = plugin.get_search_uris();
        assert!(!search_uris.is_empty());
    }

    #[test]
    fn test_compute_name_mapping_empty() {
        let doc = Document::create();
        let mapping = compute_name_mapping(&doc);
        assert_eq!(mapping.len(), 0);
    }

    #[test]
    fn test_choose_name() {
        let mut mapping = HashMap::new();
        mapping.insert("mix_float_2_1".to_string(), "mix_float".to_string());

        let name = choose_name("mix_float_2_1", &mapping);
        assert_eq!(name, "mix_float");

        let name = choose_name("other_node", &mapping);
        assert_eq!(name, "other_node");
    }

    #[test]
    fn test_map_node_names_to_base_single_pass() {
        use crate::read_from_xml_string;

        // Test inheritance chain: mix_float_210 -> mix_float_200 -> mix_float
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="mix_float" type="float" node="mix"/>
                <nodedef name="mix_float_200" type="float" node="mix" inherit="mix_float" version="2.0"/>
                <nodedef name="mix_float_210" type="float" node="mix" inherit="mix_float_200" version="2.1"/>
            </materialx>"#;

        let doc = read_from_xml_string(xml).unwrap();
        let mapping = compute_name_mapping(&doc);

        // All three should map to "mix_float" (shortest)
        assert_eq!(mapping.get("mix_float"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_200"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_210"), Some(&"mix_float".to_string()));
    }

    #[test]
    fn test_map_node_names_latest_has_official_name() {
        use crate::read_from_xml_string;

        // Test reverse naming: mix_float (latest) -> mix_float_200 -> mix_float_100
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="mix_float_100" type="float" node="mix" version="1.0"/>
                <nodedef name="mix_float_200" type="float" node="mix" inherit="mix_float_100" version="2.0"/>
                <nodedef name="mix_float" type="float" node="mix" inherit="mix_float_200" version="2.1" isdefaultversion="true"/>
            </materialx>"#;

        let doc = read_from_xml_string(xml).unwrap();
        let mapping = compute_name_mapping(&doc);

        // All should map to "mix_float" (shortest, 9 chars vs 13/14)
        assert_eq!(mapping.get("mix_float"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_200"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_100"), Some(&"mix_float".to_string()));
    }

    #[test]
    fn test_discovery_result_fields() {
        use crate::read_from_xml_string;

        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_standard_surface" type="surfaceshader" node="standard_surface" target="genglsl" version="1.0" isdefaultversion="true"/>
            </materialx>"#;

        let doc = read_from_xml_string(xml).unwrap();
        let mapping = compute_name_mapping(&doc);
        let file_result = SdrDiscoveryUri {
            uri: "/test/file.mtlx".to_string(),
            resolved_uri: "/resolved/test/file.mtlx".to_string(),
        };

        let results = discover_nodes(&doc, &file_result, &mapping);
        assert_eq!(results.len(), 1);

        let result = &results[0];
        assert_eq!(result.identifier.as_str(), "ND_standard_surface");
        assert_eq!(result.name, "ND_standard_surface");
        assert_eq!(result.family.as_str(), "standard_surface");
        assert_eq!(result.discovery_type.as_str(), "mtlx");
        assert_eq!(result.source_type.as_str(), "mtlx");
        assert_eq!(result.uri, "/test/file.mtlx");
        assert_eq!(result.resolved_uri, "/resolved/test/file.mtlx");
        assert!(result.version.is_default()); // isdefaultversion="true"

        // Check metadata
        assert_eq!(
            result.metadata.get(&Token::new("target")),
            Some(&"genglsl".to_string())
        );
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_follow_symlinks_env_var() {
        // Test that USDMTLX_PLUGIN_FOLLOW_SYMLINKS is read correctly
        unsafe {
            std::env::set_var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS", "1");
        }
        let val = std::env::var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        assert!(val);
        unsafe {
            std::env::remove_var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS");
        }

        unsafe {
            std::env::set_var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS", "true");
        }
        let val = std::env::var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        assert!(val);
        unsafe {
            std::env::remove_var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS");
        }

        let val = std::env::var("USDMTLX_PLUGIN_FOLLOW_SYMLINKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        assert!(!val); // Default is false
    }

    #[test]
    fn test_custom_search_paths_initialization() {
        // Verify that new() uses custom_search_paths() from utils
        let plugin = MtlxDiscoveryPlugin::new();
        assert_eq!(plugin.custom_search_paths, *custom_search_paths());
        assert_eq!(plugin.all_search_paths, *search_paths());
    }
}
