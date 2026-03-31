//! MaterialX Parser Plugin for SDR.
//!
//! Parses MaterialX NodeDef elements and converts them to SdrShaderNode instances.
//! This plugin handles the "mtlx" discovery type.
//!
//! Full parity with C++ pxr/usd/usdMtlx/parser.cpp.

use std::collections::HashMap;
use std::sync::LazyLock;
use usd_sdr::declare::{SdrOptionVec, SdrTokenMap};
use usd_sdr::discovery_result::SdrShaderNodeDiscoveryResult;
use usd_sdr::parser_plugin::{SdrParserPlugin, get_invalid_shader_node};
use usd_sdr::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use usd_sdr::shader_node_metadata::SdrShaderNodeMetadata;
use usd_sdr::shader_property::{SdrShaderProperty, SdrShaderPropertyUniquePtrVec};
use usd_sdr::shader_property_metadata::SdrShaderPropertyMetadata;
use usd_sdr::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

use super::document::{NodeDef, NodeGraph};
use super::tokens::USD_MTLX_TOKENS;
use super::utils::{get_source_uri, get_usd_type, get_usd_value, split_string_array};

// ============================================================================
// Environment variable: USDMTLX_PRIMARY_UV_NAME
// ============================================================================

/// Get the primary UV set name for MaterialX.
///
/// Checks `USDMTLX_PRIMARY_UV_NAME` env var first;
/// falls back to `UsdUtilsGetPrimaryUVSetName()` (which defaults to "st").
fn get_primary_uv_set_name() -> &'static str {
    static NAME: LazyLock<String> = LazyLock::new(|| {
        if let Ok(env_val) = std::env::var("USDMTLX_PRIMARY_UV_NAME") {
            if !env_val.is_empty() {
                return env_val;
            }
        }
        // Fallback to USD pipeline default
        usd_utils::pipeline::get_primary_uv_set_name()
            .as_str()
            .to_string()
    });
    &NAME
}

// ============================================================================
// Standard property metadata keys (for hints filtering)
// ============================================================================

/// Returns the set of standard SDR property metadata keys.
/// Non-standard keys go into the hints dict. Matches SdrPropertyMetadata->allTokens.
fn standard_property_metadata_keys() -> &'static std::collections::HashSet<String> {
    static KEYS: LazyLock<std::collections::HashSet<String>> = LazyLock::new(|| {
        let t = &tokens().property_metadata;
        [
            t.label.as_str(),
            t.help.as_str(),
            t.page.as_str(),
            t.render_type.as_str(),
            t.role.as_str(),
            t.widget.as_str(),
            t.hints.as_str(),
            t.options.as_str(),
            t.is_dynamic_array.as_str(),
            t.tuple_size.as_str(),
            t.connectable.as_str(),
            t.tag.as_str(),
            t.shown_if.as_str(),
            t.valid_connection_types.as_str(),
            t.vstruct_member_of.as_str(),
            t.vstruct_member_name.as_str(),
            t.vstruct_conditional_expr.as_str(),
            t.is_asset_identifier.as_str(),
            t.implementation_name.as_str(),
            t.sdr_usd_definition_type.as_str(),
            t.default_input.as_str(),
            t.target.as_str(),
            t.colorspace.as_str(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    });
    &KEYS
}

// ============================================================================
// MaterialX parser plugin
// ============================================================================

/// MaterialX parser plugin.
///
/// Parses MaterialX .mtlx files containing NodeDef elements and creates
/// SdrShaderNode instances from them.
pub struct MtlxParserPlugin;

impl MtlxParserPlugin {
    /// Create a new MaterialX parser plugin.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MtlxParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ShaderBuilder
// ============================================================================

/// Builder for constructing SdrShaderNode from MaterialX NodeDef.
///
/// Accumulates properties, metadata, and context across multiple
/// parsing stages before final node construction.
struct ShaderBuilder<'a> {
    discovery_result: &'a SdrShaderNodeDiscoveryResult,
    valid: bool,
    definition_uri: String,
    implementation_uri: String,
    context: Token,
    properties: SdrShaderPropertyUniquePtrVec,
    metadata: SdrTokenMap,
    property_name_remapping: HashMap<String, String>,
}

impl<'a> ShaderBuilder<'a> {
    fn new(discovery_result: &'a SdrShaderNodeDiscoveryResult) -> Self {
        Self {
            discovery_result,
            valid: true,
            definition_uri: String::new(),
            implementation_uri: String::new(),
            context: Token::default(),
            properties: Vec::new(),
            metadata: discovery_result.metadata.clone(),
            property_name_remapping: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    fn set_invalid(&mut self) {
        self.valid = false;
    }

    #[cfg(test)]
    fn is_valid(&self) -> bool {
        self.valid
    }

    /// Register a property name remapping (from -> to).
    /// When `from != to`, the remapping is recorded and later emitted
    /// as `SdrPropertyMetadata->ImplementationName`.
    #[allow(dead_code)]
    fn add_property_name_remapping(&mut self, from: &str, to: &str) {
        if from != to {
            self.property_name_remapping
                .insert(from.to_string(), to.to_string());
        }
    }

    fn build(self) -> SdrShaderNodeUniquePtr {
        if !self.valid {
            return get_invalid_shader_node(self.discovery_result);
        }

        let node_metadata = SdrShaderNodeMetadata::from_token_map(&self.metadata);

        Box::new(SdrShaderNode::new(
            self.discovery_result.identifier.clone(),
            self.discovery_result.version,
            self.discovery_result.name.clone(),
            self.discovery_result.family.clone(),
            self.context,
            self.discovery_result.source_type.clone(),
            self.definition_uri,
            self.implementation_uri,
            self.properties,
            node_metadata,
            self.discovery_result.source_code.clone(),
        ))
    }

    /// Add a property (input or output) from a MaterialX typed element.
    ///
    /// Full C++ parity: handles type conversion, metadata, hints, options,
    /// primvar collection, name remapping, colorspace, etc.
    fn add_property(
        &mut self,
        element: &super::document::Element,
        is_output: bool,
        primvars: Option<&mut Vec<String>>,
        added_texcoord_primvar: bool,
    ) {
        let sdr_tokens = tokens();

        let prop_type: Token;
        let mut prop_metadata: SdrTokenMap = SdrTokenMap::new();
        let mut hints: SdrTokenMap = SdrTokenMap::new();
        let mut options: SdrOptionVec = SdrOptionVec::new();
        let default_value: Value;

        let mtlx_type = element.get_attribute("type");
        let converted = get_usd_type(mtlx_type);

        if converted.shader_property_type.is_empty() {
            // No shader property type found
            if converted.value_type_name.is_valid() {
                // Use the Sdf type token
                prop_type = converted.value_type_name.as_token();

                // Special handling for Bool and Matrix3d: set SdrUsdDefinitionType
                let bool_type = usd_sdf::ValueTypeRegistry::instance().find_type("bool");
                let matrix3d_type = usd_sdf::ValueTypeRegistry::instance().find_type("matrix3d");

                if converted.value_type_name == bool_type
                    || converted.value_type_name == matrix3d_type
                {
                    default_value = get_usd_value(element, is_output);
                    // Set SdrUsdDefinitionType from first alias
                    let aliases = converted.value_type_name.aliases();
                    if let Some(first_alias) = aliases.first() {
                        prop_metadata.insert(
                            sdr_tokens.property_metadata.sdr_usd_definition_type.clone(),
                            first_alias.as_str().to_string(),
                        );
                    }
                } else {
                    default_value = Value::default();
                }
            } else {
                // Unknown type - use raw mtlx type as token
                prop_type = Token::new(mtlx_type);

                // Custom type warning: check document for TypeDef
                let doc = element.get_document();
                if doc.get_type_def(mtlx_type).is_none() {
                    eprintln!(
                        "WARNING: MaterialX unrecognized type {} on {}",
                        mtlx_type,
                        element.get_name_path()
                    );
                }

                default_value = Value::default();
            }
        } else {
            // Known shader property type
            prop_type = converted.shader_property_type.clone();

            // IsDynamicArray: array type with array_size == 0
            if converted.value_type_name.is_array() && converted.array_size == 0 {
                prop_metadata.insert(
                    sdr_tokens.property_metadata.is_dynamic_array.clone(),
                    String::new(),
                );
            }

            // IsAssetIdentifier: asset type
            let asset_type = usd_sdf::ValueTypeRegistry::instance().find_type("asset");
            if converted.value_type_name == asset_type {
                prop_metadata.insert(
                    sdr_tokens.property_metadata.is_asset_identifier.clone(),
                    String::new(),
                );
            }

            // Get default value from element
            default_value = get_usd_value(element, is_output);
        }

        // DefaultInput metadata for outputs
        if is_output {
            let defaultinput = element.get_attribute("defaultinput");
            if !defaultinput.is_empty() {
                prop_metadata.insert(
                    sdr_tokens.property_metadata.default_input.clone(),
                    defaultinput.to_string(),
                );
            }
        }

        // Target metadata for inputs
        if !is_output {
            let target = element.get_attribute("target");
            if !target.is_empty() {
                prop_metadata.insert(
                    sdr_tokens.property_metadata.target.clone(),
                    target.to_string(),
                );
            }
        }

        // Colorspace metadata for inputs and outputs
        if is_output || element.is_a("input") {
            let colorspace = element.get_attribute("colorspace");
            if !colorspace.is_empty() {
                // Compare with parent's active colorspace
                let parent_colorspace = element
                    .get_parent()
                    .map(|p| {
                        // Walk up to find active colorspace
                        let cs = p.get_attribute("colorspace");
                        if !cs.is_empty() {
                            cs.to_string()
                        } else {
                            // Walk further up
                            let mut current = p.get_parent();
                            let mut result = String::new();
                            while let Some(elem) = current {
                                let cs = elem.get_attribute("colorspace");
                                if !cs.is_empty() {
                                    result = cs.to_string();
                                    break;
                                }
                                current = elem.get_parent();
                            }
                            result
                        }
                    })
                    .unwrap_or_default();

                if colorspace != parent_colorspace {
                    prop_metadata.insert(
                        sdr_tokens.property_metadata.colorspace.clone(),
                        colorspace.to_string(),
                    );
                }
            }
        }

        // Get property name
        let mut name = element.name().to_string();

        // Collect primvar references for inputs
        if !is_output {
            if let Some(primvars) = primvars {
                let defaultgeomprop = element.get_attribute("defaultgeomprop");
                if !defaultgeomprop.is_empty() {
                    // Replace MaterialX "UV0" with configured default
                    if defaultgeomprop == "UV0" {
                        if !added_texcoord_primvar {
                            primvars.push(get_primary_uv_set_name().to_string());
                        }
                    } else {
                        primvars.push(defaultgeomprop.to_string());
                    }
                }
            }
        }

        // Single unnamed output on NodeDef -> rename to DefaultOutputName ("out")
        if element.is_a("nodedef") {
            name = USD_MTLX_TOKENS.default_output_name.as_str().to_string();
        }

        // Remap property name if registered
        if let Some(remapped) = self.property_name_remapping.get(&name) {
            prop_metadata.insert(
                sdr_tokens.property_metadata.implementation_name.clone(),
                remapped.clone(),
            );
        }

        // Parse input-specific metadata
        if !is_output {
            // uiname -> Label
            parse_property_metadata(
                &mut prop_metadata,
                &sdr_tokens.property_metadata.label,
                element,
                "uiname",
            );
            // doc -> Help
            parse_property_metadata(
                &mut prop_metadata,
                &sdr_tokens.property_metadata.help,
                element,
                "doc",
            );
            // uifolder -> Page (C++ does NOT translate '/' to ':' at parse time)
            parse_property_metadata(
                &mut prop_metadata,
                &sdr_tokens.property_metadata.page,
                element,
                "uifolder",
            );

            // UI range/step metadata (using attribute name as key)
            for attr_name in &[
                "uimin",
                "uimax",
                "uisoftmin",
                "uisoftmax",
                "uistep",
                "unit",
                "unittype",
                "defaultgeomprop",
            ] {
                parse_property_metadata_same_key(&mut prop_metadata, element, attr_name);
            }

            // Auto-generate Help from unit if doc absent
            let help_key = sdr_tokens.property_metadata.help.clone();
            if !prop_metadata.contains_key(&help_key) {
                let unit_key = Token::new("unit");
                if let Some(unit_val) = prop_metadata.get(&unit_key) {
                    let help_text = format!("Unit is {}.", unit_val);
                    prop_metadata.insert(help_key, help_text);
                }
            }

            // Move non-standard metadata keys into hints
            let standard_keys = standard_property_metadata_keys();
            for (attr_name, attr_value) in &prop_metadata {
                if !standard_keys.contains(attr_name.as_str()) {
                    hints.insert(attr_name.clone(), attr_value.clone());
                }
            }

            // Parse enum options
            options = parse_options(element);
        }

        // Build property metadata object from the token map
        let metadata_obj = SdrShaderPropertyMetadata::from_token_map(&prop_metadata);

        // Create the property
        let property = Box::new(SdrShaderProperty::new(
            Token::new(&name),
            prop_type,
            default_value,
            is_output,
            converted.array_size as usize,
            metadata_obj,
            hints,
            options,
        ));

        self.properties.push(property);
    }
}

// ============================================================================
// Metadata parsing helpers
// ============================================================================

/// Parse a metadata value from a MaterialX element attribute into an SdrTokenMap.
/// Uses `key` as the metadata key and reads `attribute` from the element.
fn parse_property_metadata(
    metadata: &mut SdrTokenMap,
    key: &Token,
    element: &super::document::Element,
    attribute: &str,
) {
    let value = element.get_attribute(attribute);
    if !value.is_empty() {
        metadata
            .entry(key.clone())
            .or_insert_with(|| value.to_string());
    }
}

/// Parse metadata where the attribute name is also used as the key.
fn parse_property_metadata_same_key(
    metadata: &mut SdrTokenMap,
    element: &super::document::Element,
    attribute: &str,
) {
    let value = element.get_attribute(attribute);
    if !value.is_empty() {
        let key = Token::new(attribute);
        metadata.entry(key).or_insert_with(|| value.to_string());
    }
}

/// Parse node-level metadata from a MaterialX element attribute.
/// Normalizes texture2d/texture3d role to "texture".
fn parse_node_metadata(
    builder: &mut ShaderBuilder,
    key: &Token,
    element: &super::document::Element,
    attribute: &str,
) {
    let value = element.get_attribute(attribute);
    if !value.is_empty() {
        // Normalize texture2d/texture3d roles to "texture"
        if key == &tokens().node_metadata.role && (value == "texture2d" || value == "texture3d") {
            builder.metadata.insert(key.clone(), "texture".to_string());
        } else {
            builder.metadata.insert(key.clone(), value.to_string());
        }
    }
}

// ============================================================================
// Options parsing
// ============================================================================

/// Parse enum/enumvalues attributes into option pairs.
///
/// Handles stride regrouping for vector types where enumvalues
/// produces more elements than enum labels.
fn parse_options(element: &super::document::Element) -> SdrOptionVec {
    let enum_labels_str = element.get_attribute("enum");
    if enum_labels_str.is_empty() {
        return Vec::new();
    }

    let enum_values_str = element.get_attribute("enumvalues");
    let all_labels = split_string_array(enum_labels_str);
    let mut all_values = split_string_array(enum_values_str);

    // Reconcile label/value count mismatch (e.g., vector2 values have 2x elements)
    if !all_values.is_empty() && all_values.len() != all_labels.len() {
        if all_values.len() > all_labels.len() && all_values.len() % all_labels.len() == 0 {
            // Regroup values by stride
            let stride = all_values.len() / all_labels.len();
            let mut rebuilt = Vec::new();
            let mut current = String::new();
            for (i, val) in all_values.iter().enumerate() {
                if i % stride != 0 {
                    current.push_str(", ");
                }
                current.push_str(val);
                if (i + 1) % stride == 0 {
                    rebuilt.push(current.clone());
                    current.clear();
                }
            }
            all_values = rebuilt;
        } else {
            // Cannot reconcile
            all_values.clear();
        }
    }

    let mut result = Vec::new();
    let mut val_iter = all_values.iter();
    for label in &all_labels {
        let value = val_iter.next().map(|v| Token::new(v)).unwrap_or_default();
        result.push((Token::new(label), value));
    }
    result
}

// ============================================================================
// Context resolution
// ============================================================================

/// Get shader context from document TypeDef.
///
/// If the type has a TypeDef with `semantic="shader"`, return its `context` attribute.
/// Returns empty token if not found or no shader semantic.
fn get_context(doc: &super::document::Document, type_name: &str) -> Token {
    if let Some(typedef) = doc.get_type_def(type_name) {
        if typedef.get_semantic() == "shader" {
            let ctx = typedef.get_context();
            if !ctx.is_empty() {
                return Token::new(ctx);
            }
        }
    }
    Token::default()
}

// ============================================================================
// ParseElement - main NodeDef parsing
// ============================================================================

/// Parse a NodeDef element and populate the ShaderBuilder with full metadata,
/// properties, and primvar references.
fn parse_element(builder: &mut ShaderBuilder, nodedef: &NodeDef) {
    let sdr_tokens = tokens();
    let node_type = nodedef.get_type();

    // Resolve context from TypeDef
    let mut context = get_context(&nodedef.0.get_document(), node_type);
    if context.is_empty() {
        // Fallback to standard library typedefs
        if let Some(stdlib_doc) = super::utils::get_document("") {
            context = get_context(&stdlib_doc, node_type);
        }
    }
    if context.is_empty() {
        // Final fallback: Pattern context
        context = sdr_tokens.node_context.pattern.clone();
    }

    // Set builder fields
    builder.context = context;
    builder.definition_uri = get_source_uri(&nodedef.0);
    builder.implementation_uri = builder.definition_uri.clone();

    // Node metadata
    builder.metadata.insert(
        sdr_tokens.node_metadata.label.clone(),
        nodedef.get_node_string().to_string(),
    );
    builder.metadata.insert(
        sdr_tokens.node_metadata.category.clone(),
        node_type.to_string(),
    );
    parse_node_metadata(builder, &sdr_tokens.node_metadata.help, &nodedef.0, "doc");
    parse_node_metadata(
        builder,
        &sdr_tokens.node_metadata.target,
        &nodedef.0,
        "target",
    );
    // Role with texture2d/texture3d normalization
    parse_node_metadata(
        builder,
        &sdr_tokens.node_metadata.role,
        &nodedef.0,
        "nodegroup",
    );

    // Primvar collection
    let mut primvars: Vec<String> = Vec::new();

    // ND_geompropvalue -> $geomprop primvar
    if nodedef.0.name().starts_with("ND_geompropvalue") {
        primvars.push("$geomprop".to_string());
    }

    // ND_texcoord_vector2 -> default UV set
    if nodedef.0.name() == "ND_texcoord_vector2" {
        primvars.push(get_primary_uv_set_name().to_string());
    }

    // Scan implementation nodegraph for primvar-reading nodes
    let mut added_texcoord_primvar = false;
    if let Some(impl_elem) = nodedef.get_implementation() {
        if impl_elem.is_a("nodegraph") {
            let ng = NodeGraph(impl_elem);

            // geompropvalue nodes -> primvar from "geomprop" input
            for geomprop_node in ng.get_nodes("geompropvalue") {
                for input in geomprop_node.get_inputs() {
                    if input.0.name() == "geomprop" {
                        let val = input.get_value_string();
                        if !val.is_empty() {
                            primvars.push(val.to_string());
                            // Assume texcoord primvar if vector2 type
                            if geomprop_node.get_type() == "vector2" {
                                added_texcoord_primvar = true;
                            }
                        }
                    }
                }
            }

            // texcoord nodes -> default UV set
            if !ng.get_nodes("texcoord").is_empty() {
                primvars.push(get_primary_uv_set_name().to_string());
                added_texcoord_primvar = true;
            }

            // image/tiledimage nodes -> default UV set (if no texcoord primvar yet)
            if !added_texcoord_primvar
                && (!ng.get_nodes("tiledimage").is_empty() || !ng.get_nodes("image").is_empty())
            {
                primvars.push(get_primary_uv_set_name().to_string());
                added_texcoord_primvar = true;
            }
        }
    }

    // internalgeomprops attribute
    let internalgeomprops = nodedef.0.get_attribute("internalgeomprops");
    if !internalgeomprops.is_empty() {
        let mut split = split_string_array(internalgeomprops);
        // Replace "UV0" with configured default
        for name in &mut split {
            if name == "UV0" {
                *name = get_primary_uv_set_name().to_string();
            }
        }
        primvars.extend(split);
    }

    // Add input properties
    for mtlx_input in nodedef.get_active_inputs() {
        builder.add_property(
            &mtlx_input.0,
            false,
            Some(&mut primvars),
            added_texcoord_primvar,
        );
    }

    // Add output properties
    for mtlx_output in nodedef.get_active_outputs() {
        builder.add_property(&mtlx_output.0, true, None, false);
    }

    // Store collected primvars as pipe-separated string
    if !primvars.is_empty() {
        let primvars_str = primvars.join("|");
        builder
            .metadata
            .insert(sdr_tokens.node_metadata.primvars.clone(), primvars_str);
    }
}

// ============================================================================
// SdrParserPlugin implementation
// ============================================================================

impl SdrParserPlugin for MtlxParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        // Load MaterialX document
        let doc = if !discovery_result.resolved_uri.is_empty() {
            let uri = if discovery_result.resolved_uri == "mtlx" {
                ""
            } else {
                &discovery_result.resolved_uri
            };
            match super::utils::get_document(uri) {
                Some(doc) => doc,
                None => return Some(get_invalid_shader_node(discovery_result)),
            }
        } else if !discovery_result.source_code.is_empty() {
            match super::utils::get_document_from_string(&discovery_result.source_code) {
                Some(doc) => doc,
                None => {
                    eprintln!("WARNING: Invalid mtlx source code.");
                    return Some(get_invalid_shader_node(discovery_result));
                }
            }
        } else {
            eprintln!(
                "WARNING: Invalid SdrShaderNodeDiscoveryResult for identifier '{}': \
                 both resolvedUri and sourceCode fields are empty.",
                discovery_result.identifier.as_str()
            );
            return Some(get_invalid_shader_node(discovery_result));
        };

        // Create a potentially modified discovery result
        let mut new_discovery = discovery_result.clone();

        // Look up NodeDef by identifier
        let mut nodedef = doc.get_node_def(discovery_result.identifier.as_str());

        if nodedef.is_none() && discovery_result.sub_identifier.is_empty() {
            eprintln!(
                "WARNING: Invalid MaterialX NodeDef; unknown node name ' {} '",
                discovery_result.identifier.as_str()
            );
            return Some(get_invalid_shader_node(discovery_result));
        }

        // SubIdentifier handling: custom nodes specify nodeDef name as subIdentifier
        if !discovery_result.sub_identifier.is_empty() {
            nodedef = doc.get_node_def(discovery_result.sub_identifier.as_str());
            if nodedef.is_none() {
                eprintln!(
                    "WARNING: Invalid MaterialX NodeDef; unknown node name ' {} '",
                    discovery_result.sub_identifier.as_str()
                );
                return Some(get_invalid_shader_node(discovery_result));
            }
            // Pass nodeDef name (subIdentifier) through metadata as ImplementationName
            let mut metadata_map = SdrTokenMap::new();
            metadata_map.insert(
                tokens().node_metadata.implementation_name.clone(),
                discovery_result.sub_identifier.as_str().to_string(),
            );
            new_discovery.metadata = metadata_map;
        }

        let nodedef = nodedef.unwrap();

        // Build shader node
        let mut builder = ShaderBuilder::new(&new_discovery);
        parse_element(&mut builder, &nodedef);

        Some(builder.build())
    }

    fn get_discovery_types(&self) -> Vec<Token> {
        vec![Token::new("mtlx")]
    }

    fn get_source_type(&self) -> Token {
        Token::new("") // Empty source type = default
    }

    fn get_name(&self) -> &str {
        "MtlxParserPlugin"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdr::declare::SdrVersion;

    #[test]
    fn test_parser_discovery_types() {
        let parser = MtlxParserPlugin::new();
        let types = parser.get_discovery_types();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_str(), "mtlx");
    }

    #[test]
    fn test_parser_source_type() {
        let parser = MtlxParserPlugin::new();
        assert_eq!(parser.get_source_type().as_str(), "");
    }

    #[test]
    fn test_parse_invalid_uri() {
        let parser = MtlxParserPlugin::new();
        let result = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            "/nonexistent/file.mtlx".to_string(),
            "/nonexistent/file.mtlx".to_string(),
        );
        let node = parser.parse_shader_node(&result);
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.get_identifier().as_str(), "test");
    }

    #[test]
    fn test_primary_uv_set_name_default() {
        // Without USDMTLX_PRIMARY_UV_NAME env var, should return "st"
        let name = get_primary_uv_set_name();
        assert_eq!(name, "st");
    }

    #[test]
    fn test_parse_options_empty() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="float" node="test">
                    <input name="x" type="float" value="1.0"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();
        let inputs = nd.get_active_inputs();
        let opts = parse_options(&inputs[0].0);
        assert!(opts.is_empty());
    }

    #[test]
    fn test_parse_options_with_enum() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="float" node="test">
                    <input name="mode" type="string" enum="clamp,wrap,mirror" enumvalues="0,1,2"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();
        let inputs = nd.get_active_inputs();
        let opts = parse_options(&inputs[0].0);
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0].0.as_str(), "clamp");
        assert_eq!(opts[0].1.as_str(), "0");
        assert_eq!(opts[1].0.as_str(), "wrap");
        assert_eq!(opts[2].0.as_str(), "mirror");
    }

    #[test]
    fn test_parse_options_stride_regroup() {
        // vector2 enum: 2 labels, 4 values -> stride=2
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="vector2" node="test">
                    <input name="mode" type="vector2" enum="a,b" enumvalues="1.0,2.0,3.0,4.0"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();
        let inputs = nd.get_active_inputs();
        let opts = parse_options(&inputs[0].0);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].0.as_str(), "a");
        assert_eq!(opts[0].1.as_str(), "1.0, 2.0");
        assert_eq!(opts[1].0.as_str(), "b");
        assert_eq!(opts[1].1.as_str(), "3.0, 4.0");
    }

    #[test]
    fn test_get_context_shader_semantic() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <typedef name="surfaceshader" semantic="shader" context="surface"/>
                <nodedef name="ND_surf" type="surfaceshader" node="mysurf"/>
            </materialx>"#,
        )
        .unwrap();
        let ctx = get_context(&doc, "surfaceshader");
        assert_eq!(ctx.as_str(), "surface");
    }

    #[test]
    fn test_get_context_no_shader_semantic() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <typedef name="mytype" semantic="data"/>
                <nodedef name="ND_test" type="mytype" node="test"/>
            </materialx>"#,
        )
        .unwrap();
        let ctx = get_context(&doc, "mytype");
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_parse_element_basic() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_myfunc" type="color3" node="myfunc" nodegroup="texture">
                    <input name="base" type="float" value="0.5" uiname="Base Value" doc="The base"/>
                    <input name="color" type="color3" value="1.0, 0.0, 0.0"/>
                    <output name="out" type="color3"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_myfunc").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_myfunc"),
            SdrVersion::new(1, 0),
            "myfunc".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        assert!(builder.is_valid());
        assert_eq!(builder.context.as_str(), "pattern"); // No shader semantic -> fallback
        assert_eq!(builder.properties.len(), 3); // 2 inputs + 1 output

        // Check node metadata
        let role = builder
            .metadata
            .get(&tokens().node_metadata.role)
            .cloned()
            .unwrap_or_default();
        assert_eq!(role, "texture");

        let label = builder
            .metadata
            .get(&tokens().node_metadata.label)
            .cloned()
            .unwrap_or_default();
        assert_eq!(label, "myfunc");
    }

    #[test]
    fn test_parse_element_with_surface_context() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <typedef name="surfaceshader" semantic="shader" context="surface"/>
                <nodedef name="ND_mysurf" type="surfaceshader" node="mysurf">
                    <output name="out" type="surfaceshader"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_mysurf").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_mysurf"),
            SdrVersion::new(1, 0),
            "mysurf".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        assert_eq!(builder.context.as_str(), "surface");
    }

    #[test]
    fn test_parse_element_primvar_geompropvalue() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_geompropvalue_float" type="float" node="geompropvalue">
                    <input name="geomprop" type="string" value=""/>
                    <output name="out" type="float"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_geompropvalue_float").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_geompropvalue_float"),
            SdrVersion::new(1, 0),
            "geompropvalue".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        // Should have $geomprop primvar
        let primvars = builder
            .metadata
            .get(&tokens().node_metadata.primvars)
            .cloned()
            .unwrap_or_default();
        assert!(
            primvars.contains("$geomprop"),
            "Expected $geomprop in primvars: '{}'",
            primvars
        );
    }

    #[test]
    fn test_parse_element_primvar_texcoord() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_texcoord_vector2" type="vector2" node="texcoord">
                    <output name="out" type="vector2"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_texcoord_vector2").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_texcoord_vector2"),
            SdrVersion::new(1, 0),
            "texcoord".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        let primvars = builder
            .metadata
            .get(&tokens().node_metadata.primvars)
            .cloned()
            .unwrap_or_default();
        assert!(
            primvars.contains("st"),
            "Expected 'st' in primvars: '{}'",
            primvars
        );
    }

    #[test]
    fn test_parse_empty_source() {
        let parser = MtlxParserPlugin::new();
        let result = SdrShaderNodeDiscoveryResult::default();
        let node = parser.parse_shader_node(&result);
        assert!(node.is_some());
    }

    #[test]
    fn test_parse_from_source_code() {
        let parser = MtlxParserPlugin::new();
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_add_float" type="float" node="add">
                    <input name="in1" type="float" value="0.0"/>
                    <input name="in2" type="float" value="0.0"/>
                    <output name="out" type="float"/>
                </nodedef>
            </materialx>"#;

        let result = SdrShaderNodeDiscoveryResult::from_source_code(
            Token::new("ND_add_float"),
            SdrVersion::new(1, 0),
            "add".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            xml.to_string(),
        );

        let node = parser.parse_shader_node(&result);
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.get_identifier().as_str(), "ND_add_float");
    }

    #[test]
    fn test_sub_identifier_handling() {
        let parser = MtlxParserPlugin::new();
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_custom_impl" type="float" node="custom">
                    <input name="val" type="float" value="1.0"/>
                    <output name="out" type="float"/>
                </nodedef>
            </materialx>"#;

        let mut result = SdrShaderNodeDiscoveryResult::from_source_code(
            Token::new("custom_shader"),
            SdrVersion::new(1, 0),
            "custom".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            xml.to_string(),
        );
        result.sub_identifier = Token::new("ND_custom_impl");

        let node = parser.parse_shader_node(&result);
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.get_identifier().as_str(), "custom_shader");
    }

    #[test]
    fn test_metadata_auto_help_from_unit() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="float" node="test">
                    <input name="dist" type="float" value="1.0" unit="meter" unittype="distance"/>
                    <output name="out" type="float"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        // The "dist" input should have auto-generated help from unit
        let dist_prop = builder.properties.iter().find(|p| p.get_name() == "dist");
        assert!(dist_prop.is_some());
        let dist_prop = dist_prop.unwrap();
        let help = dist_prop.get_help();
        assert_eq!(help, "Unit is meter.");
    }

    #[test]
    fn test_texture_role_normalization() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="color3" node="test" nodegroup="texture2d">
                    <output name="out" type="color3"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        let role = builder
            .metadata
            .get(&tokens().node_metadata.role)
            .cloned()
            .unwrap_or_default();
        assert_eq!(role, "texture", "texture2d should be normalized to texture");
    }

    #[test]
    fn test_property_name_remapping() {
        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);

        // Same name -> no remapping
        builder.add_property_name_remapping("foo", "foo");
        assert!(builder.property_name_remapping.is_empty());

        // Different name -> remapping stored
        builder.add_property_name_remapping("bar", "baz");
        assert_eq!(builder.property_name_remapping.get("bar").unwrap(), "baz");
    }

    #[test]
    fn test_internalgeomprops_uv0_replacement() {
        let doc = crate::read_from_xml_string(
            r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" type="float" node="test" internalgeomprops="UV0, normal">
                    <output name="out" type="float"/>
                </nodedef>
            </materialx>"#,
        )
        .unwrap();
        let nd = doc.get_node_def("ND_test").unwrap();

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("ND_test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::new("mtlx"),
            Token::new(""),
            String::new(),
            "mtlx".to_string(),
        );
        let mut builder = ShaderBuilder::new(&dr);
        parse_element(&mut builder, &nd);

        let primvars = builder
            .metadata
            .get(&tokens().node_metadata.primvars)
            .cloned()
            .unwrap_or_default();
        // UV0 should be replaced with "st"
        assert!(
            primvars.contains("st"),
            "Expected 'st' in primvars: '{}'",
            primvars
        );
        assert!(
            primvars.contains("normal"),
            "Expected 'normal' in primvars: '{}'",
            primvars
        );
        assert!(!primvars.contains("UV0"), "UV0 should have been replaced");
    }
}
