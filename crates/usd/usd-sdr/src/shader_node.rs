//! SDR Shader Node - Main shader node representation.
//!
//! Port of pxr/usd/sdr/shaderNode.h
//!
//! This module provides SdrShaderNode which represents a node that holds
//! shading information. It describes the name, inputs, outputs, and metadata
//! of a shader definition.
//!
//! Used by: SdrRegistry
//! Uses: SdrShaderProperty, SdrShaderNodeMetadata, SdrVersion

use super::declare::{SdrIdentifier, SdrTokenMap, SdrTokenVec, SdrVersion};
use super::shader_node_metadata::SdrShaderNodeMetadata;
use super::shader_property::{SdrShaderProperty, SdrShaderPropertyUniquePtrVec};
use super::tokens::tokens;
use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Represents a node that holds shading information.
///
/// Describes information like the name of the node, what its inputs and
/// outputs are, and any associated metadata. Shader nodes are created by
/// the registry from discovery results and parser plugins.
///
/// # Node Identification
///
/// Nodes are identified by:
/// - `identifier`: Unique ID (often includes version, e.g., "mix_float_2_1")
/// - `name`: Version-independent name (e.g., "mix_float")
/// - `family`: Optional grouping (e.g., "mix")
/// - `source_type`: Origin type (e.g., "OSL", "glslfx")
///
/// # Context
///
/// The context describes what role the shader plays in rendering:
/// - "pattern" - pattern evaluation shaders
/// - "surface" - surface BXDFs
/// - "volume" - volume shaders
/// - "light" - light shaders
/// - etc.
#[derive(Debug, Clone)]
pub struct SdrShaderNode {
    // Basic identification
    is_valid: bool,
    identifier: SdrIdentifier,
    version: SdrVersion,
    name: String,
    family: Token,
    context: Token,
    source_type: Token,
    definition_uri: String,
    implementation_uri: String,
    source_code: String,

    // Properties (owned)
    properties: SdrShaderPropertyUniquePtrVec,

    // Metadata
    legacy_metadata: SdrTokenMap,
    metadata: SdrShaderNodeMetadata,

    // Cached property maps for fast lookup
    inputs: HashMap<Token, usize>,
    input_names: SdrTokenVec,
    outputs: HashMap<Token, usize>,
    output_names: SdrTokenVec,

    // Cached metadata
    label: Token,
    category: Token,
    departments: SdrTokenVec,
    open_pages: SdrTokenVec,
    pages_shown_if: SdrTokenMap,

    // Processed primvar metadata
    primvars: SdrTokenVec,
    primvar_naming_properties: SdrTokenVec,

    // Aggregated pages from properties
    pages: SdrTokenVec,
}

impl SdrShaderNode {
    /// Creates a new shader node.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: SdrIdentifier,
        version: SdrVersion,
        name: String,
        family: Token,
        context: Token,
        source_type: Token,
        definition_uri: String,
        implementation_uri: String,
        properties: SdrShaderPropertyUniquePtrVec,
        metadata: SdrShaderNodeMetadata,
        source_code: String,
    ) -> Self {
        let mut node = Self {
            is_valid: true,
            identifier,
            version,
            name,
            family,
            context,
            source_type,
            definition_uri,
            implementation_uri,
            source_code,
            properties,
            legacy_metadata: metadata.encode_legacy_metadata(),
            metadata: metadata.clone(),
            inputs: HashMap::new(),
            input_names: Vec::new(),
            outputs: HashMap::new(),
            output_names: Vec::new(),
            label: metadata.get_label(),
            category: metadata.get_category(),
            departments: metadata.get_departments(),
            open_pages: metadata.get_open_pages(),
            pages_shown_if: metadata.get_pages_shown_if(),
            primvars: Vec::new(),
            primvar_naming_properties: Vec::new(),
            pages: Vec::new(),
        };

        node.post_process_properties();
        node
    }

    // ========================================================================
    // The Basics
    // ========================================================================

    /// Returns the identifier of the node.
    pub fn get_identifier(&self) -> &SdrIdentifier {
        &self.identifier
    }

    /// Returns the version of the node.
    pub fn get_shader_version(&self) -> SdrVersion {
        self.version
    }

    /// Gets the name of the node.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Gets the name of the family that the node belongs to.
    ///
    /// An empty token will be returned if the node does not belong to a family.
    pub fn get_family(&self) -> &Token {
        &self.family
    }

    /// Gets the context of the shader node.
    ///
    /// The context is the context that the node declares itself as having
    /// (e.g., 'pattern', 'surface', 'light').
    pub fn get_context(&self) -> &Token {
        &self.context
    }

    /// Gets the type of source that this shader node originated from.
    ///
    /// This is distinct from `get_context()`, which is the type that the node
    /// declares itself as having. The source type is unique to the parsing
    /// plugin (e.g., "OSL", "glslfx", "Args").
    pub fn get_source_type(&self) -> &Token {
        &self.source_type
    }

    /// Gets the URI to the resource that provided this node's definition.
    ///
    /// Could be a path to a file, or some other resource identifier.
    /// This URI should be fully resolved.
    pub fn get_resolved_definition_uri(&self) -> &str {
        &self.definition_uri
    }

    /// Gets the URI to the resource that provides this node's implementation.
    ///
    /// Could be a path to a file, or some other resource identifier.
    /// This URI should be fully resolved.
    pub fn get_resolved_implementation_uri(&self) -> &str {
        &self.implementation_uri
    }

    /// Returns the source code for this node.
    ///
    /// This will be empty for most nodes. It will be non-empty only for nodes
    /// constructed using SdrRegistry::GetShaderNodeFromSourceCode().
    pub fn get_source_code(&self) -> &str {
        &self.source_code
    }

    /// Whether or not this node is valid.
    ///
    /// A valid node indicates that the parser plugin was able to successfully
    /// parse the contents of this node.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Gets a string with basic information about this node.
    ///
    /// Matches C++ format: "IDENTIFIER (context: 'CTX', version: 'VER', family: 'FAM');
    /// definition URI: 'DEF'; implementation URI: 'IMPL'"
    pub fn get_info_string(&self) -> String {
        format!(
            "{} (context: '{}', version: '{}', family: '{}'); definition URI: '{}'; implementation URI: '{}'",
            super::declare::sdr_get_identifier_string(&self.identifier),
            self.context.as_str(),
            self.version.get_string(),
            self.family.as_str(),
            self.definition_uri,
            self.implementation_uri
        )
    }

    // ========================================================================
    // Inputs and Outputs
    // ========================================================================

    /// Get an ordered list of all the input names on this shader node.
    pub fn get_shader_input_names(&self) -> &SdrTokenVec {
        &self.input_names
    }

    /// Get an ordered list of all the output names on this shader node.
    pub fn get_shader_output_names(&self) -> &SdrTokenVec {
        &self.output_names
    }

    /// Get a shader input property by name.
    ///
    /// Returns None if an input with the given name does not exist.
    pub fn get_shader_input(&self, input_name: &Token) -> Option<&SdrShaderProperty> {
        self.inputs
            .get(input_name)
            .map(|&idx| self.properties[idx].as_ref())
    }

    /// Get a shader output property by name.
    ///
    /// Returns None if an output with the given name does not exist.
    pub fn get_shader_output(&self, output_name: &Token) -> Option<&SdrShaderProperty> {
        self.outputs
            .get(output_name)
            .map(|&idx| self.properties[idx].as_ref())
    }

    /// Returns the list of all inputs that are tagged as asset identifier inputs.
    pub fn get_asset_identifier_input_names(&self) -> SdrTokenVec {
        self.input_names
            .iter()
            .filter(|name| {
                self.get_shader_input(name)
                    .map(|p| p.is_asset_identifier())
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Returns the first shader input that is tagged as the default input.
    ///
    /// A default input and its value can be used to acquire a fallback value
    /// for a node when the node is considered 'disabled'.
    pub fn get_default_input(&self) -> Option<&SdrShaderProperty> {
        for name in &self.input_names {
            if let Some(prop) = self.get_shader_input(name) {
                if prop.is_default_input() {
                    return Some(prop);
                }
            }
        }
        None
    }

    /// Get all properties (inputs and outputs) of this node.
    pub fn get_properties(&self) -> impl Iterator<Item = &SdrShaderProperty> {
        self.properties.iter().map(|p| p.as_ref())
    }

    // ========================================================================
    // Metadata
    // ========================================================================

    /// All metadata that came from the parse process (legacy format).
    pub fn get_metadata(&self) -> &SdrTokenMap {
        &self.legacy_metadata
    }

    /// All metadata that came from the parse process (new format).
    pub fn get_metadata_object(&self) -> &SdrShaderNodeMetadata {
        &self.metadata
    }

    /// The label assigned to this node, if any.
    ///
    /// Distinct from the name returned from `get_name()`. In the context of a UI,
    /// the label value might be used as the display name.
    pub fn get_label(&self) -> &Token {
        &self.label
    }

    /// The category assigned to this node, if any.
    ///
    /// Distinct from the family returned from `get_family()`.
    pub fn get_category(&self) -> &Token {
        &self.category
    }

    /// Returns the role of this node.
    ///
    /// Matches C++: `HasRole() ? GetRole() : TfToken(GetName())`.
    /// Falls back to the node name when no role metadata is set.
    pub fn get_role(&self) -> Token {
        let role = self.metadata.get_role();
        if role.as_str().is_empty() {
            Token::new(&self.name)
        } else {
            role
        }
    }

    /// The help message assigned to this node, if any.
    pub fn get_help(&self) -> String {
        self.metadata.get_help()
    }

    /// The departments this node is associated with, if any.
    pub fn get_departments(&self) -> &SdrTokenVec {
        &self.departments
    }

    /// Gets the pages on which the node's properties reside.
    ///
    /// This is an aggregate of the unique `SdrShaderProperty::get_page()` values
    /// for all of the node's properties. Properties might be divided into pages
    /// like 'Simple' and 'Advanced'.
    pub fn get_pages(&self) -> &SdrTokenVec {
        &self.pages
    }

    /// Gets the pages which should be opened or expanded by default.
    pub fn get_open_pages(&self) -> &SdrTokenVec {
        &self.open_pages
    }

    /// Gets the `shownIf` expressions associated with each page.
    pub fn get_pages_shown_if(&self) -> &SdrTokenMap {
        &self.pages_shown_if
    }

    /// The list of primvars this node knows it requires / uses.
    ///
    /// Additional user-specified primvars may have been authored on the node;
    /// those can be queried via `get_additional_primvar_properties()`.
    pub fn get_primvars(&self) -> &SdrTokenVec {
        &self.primvars
    }

    /// The list of string input properties whose values provide the names of
    /// additional primvars consumed by this node.
    pub fn get_additional_primvar_properties(&self) -> &SdrTokenVec {
        &self.primvar_naming_properties
    }

    /// Returns the implementation name of this node.
    ///
    /// The name of the node is how to refer to the node in shader networks.
    /// The label is how to present this node to users. The implementation
    /// name is the name of the function this node represents in the implementation.
    pub fn get_implementation_name(&self) -> String {
        let impl_name = self.metadata.get_implementation_name();
        if impl_name.is_empty() {
            self.name.clone()
        } else {
            impl_name
        }
    }

    // ========================================================================
    // Aggregate Information
    // ========================================================================

    /// Gets the names of the properties on a certain page.
    ///
    /// To get properties that are not assigned to a page, an empty string
    /// can be used for `page_name`.
    pub fn get_property_names_for_page(&self, page_name: &str) -> SdrTokenVec {
        self.properties
            .iter()
            .filter(|p| p.get_page() == page_name)
            .map(|p| p.get_name().clone())
            .collect()
    }

    /// Gets all vstructs that are present in the shader.
    ///
    /// Matches C++ two-pass scan:
    /// - Pass 1: inputs map — Tag=="vstruct" in legacy metadata → head,
    ///           or IsVStructMember with head found in same map → head
    /// - Pass 2: outputs map — same logic
    pub fn get_all_vstruct_names(&self) -> SdrTokenVec {
        use std::collections::BTreeSet;

        let tag_key = &tokens().property_metadata.tag;

        // Returns true if the property has Tag metadata equal to "vstruct"
        let has_vstruct_tag = |prop: &super::shader_property::SdrShaderProperty| -> bool {
            prop.get_metadata()
                .get(tag_key)
                .map(|v| v == "vstruct")
                .unwrap_or(false)
        };

        let mut vstructs: BTreeSet<Token> = BTreeSet::new();

        // Pass 1: inputs
        for (name, &idx) in &self.inputs {
            let prop = self.properties[idx].as_ref();
            if has_vstruct_tag(prop) {
                vstructs.insert(name.clone());
                continue;
            }
            if !prop.is_vstruct_member() {
                continue;
            }
            let head = prop.get_vstruct_member_of();
            if self.inputs.contains_key(head) {
                vstructs.insert(head.clone());
            }
        }

        // Pass 2: outputs
        for (name, &idx) in &self.outputs {
            let prop = self.properties[idx].as_ref();
            if has_vstruct_tag(prop) {
                vstructs.insert(name.clone());
                continue;
            }
            if !prop.is_vstruct_member() {
                continue;
            }
            let head = prop.get_vstruct_member_of();
            if self.outputs.contains_key(head) {
                vstructs.insert(head.clone());
            }
        }

        vstructs.into_iter().collect()
    }

    /// Gets an item of data from this shader node according to the requested key.
    ///
    /// Special keys indicate class fields:
    /// - SdrNodeFieldKey->Identifier -> TfToken
    /// - SdrNodeFieldKey->Name -> std::string
    /// - SdrNodeFieldKey->Family -> TfToken
    /// - SdrNodeFieldKey->SourceType -> TfToken
    ///
    /// Any other requested key will be looked for in this shader node's metadata.
    /// Matches C++ `GetDataForKey()`: Identifier/Family/SourceType return
    /// TfToken values; Name returns std::string; others look up metadata.
    pub fn get_data_for_key(&self, key: &Token) -> Value {
        let field_keys = &tokens().node_field_key;

        if key == &field_keys.identifier {
            // C++: VtValue(GetIdentifier()) — TfToken
            Value::from(self.identifier.clone())
        } else if key == &field_keys.name {
            // C++: VtValue(GetName()) — std::string
            Value::from(self.name.clone())
        } else if key == &field_keys.family {
            // C++: VtValue(GetFamily()) — TfToken
            Value::from(self.family.clone())
        } else if key == &field_keys.source_type {
            // C++: VtValue(GetSourceType()) — TfToken
            Value::from(self.source_type.clone())
        } else {
            // C++: _metadata.GetItemValue(key) — returns VtValue directly
            self.metadata
                .get_item_value(key)
                .cloned()
                .unwrap_or_default()
        }
    }

    // ========================================================================
    // Post Processing (internal)
    // ========================================================================

    /// Performs post-processing on properties to determine information that can
    /// only be determined after parsing or in aggregate.
    fn post_process_properties(&mut self) {
        // If properties are not empty, the node was parsed successfully and is valid
        self.is_valid = !self.properties.is_empty();

        // Build input/output maps
        for (idx, prop) in self.properties.iter().enumerate() {
            let name = prop.get_name().clone();
            if prop.is_output() {
                self.outputs.insert(name.clone(), idx);
                self.output_names.push(name);
            } else {
                self.inputs.insert(name.clone(), idx);
                self.input_names.push(name);
            }
        }

        // Initialize primvars
        self.initialize_primvars();

        // Compute pages
        self.pages = self.compute_pages();

        // Get all vstruct names using the proper two-pass scan (C++ GetAllVstructNames).
        // This checks legacy Tag metadata AND IsVStructMember relationships, rather
        // than just checking if the type is already "vstruct".
        let vstruct_names = self.get_all_vstruct_names();

        // Set USD encoding version on all properties and perform conversions.
        // C++ uses DEFAULT_ENCODING = -1 to mean "not set"; we use the same.
        let has_encoding = self.metadata.has_sdr_usd_encoding_version();
        let encoding_version = if has_encoding {
            self.metadata.get_sdr_usd_encoding_version()
        } else {
            -1
        };

        // Collect definition URI for _ConvertExpressions context
        let def_uri = self.definition_uri.clone();

        // Phase 1: set encoding version and convert vstructs
        for prop in &mut self.properties {
            if encoding_version != -1 {
                prop.set_usd_encoding_version(encoding_version);
            }
            if vstruct_names.contains(prop.get_name()) {
                prop.convert_to_vstruct();
            }
        }

        // Phase 2: convert expressions (needs read access to all properties)
        // Collect expressions first, then apply, to avoid borrow conflicts
        let expressions: Vec<(usize, String)> = (0..self.properties.len())
            .filter(|&i| !self.properties[i].get_metadata_object().has_shown_if())
            .filter_map(|i| {
                let expr = super::shader_metadata_helpers::compute_shown_if_from_property_metadata(
                    &self.properties[i],
                    &self.properties,
                    &def_uri,
                );
                if expr.is_empty() {
                    None
                } else {
                    Some((i, expr))
                }
            })
            .collect();

        for (idx, expr) in expressions {
            self.properties[idx].set_shown_if(&expr);
        }

        // Phase 3: finalize all properties
        for prop in &mut self.properties {
            prop.finalize();
        }
    }

    /// Initializes `primvars` and `primvar_naming_properties`.
    ///
    /// Matches C++ `_InitializePrimvars()`:
    /// - Iterates the raw primvar list from node metadata.
    /// - Entries starting with "$" are property references: strip "$", look up the
    ///   input by name, add to `primvar_naming_properties` if it has type string.
    /// - All other entries are literal primvar names added to `primvars`.
    fn initialize_primvars(&mut self) {
        let raw_primvars = self.metadata.get_primvars();

        for pv in raw_primvars {
            if let Some(prop_name) = pv.strip_prefix('$') {
                // $-prefixed: resolve to input property of type string
                let tok = Token::new(prop_name);
                if let Some(&idx) = self.inputs.get(&tok) {
                    let prop = self.properties[idx].as_ref();
                    if prop.get_type() == &tokens().property_types.string {
                        self.primvar_naming_properties.push(tok);
                    }
                }
                // If the input doesn't exist or isn't a string, skip (log in C++)
            } else {
                self.primvars.push(Token::new(&pv));
            }
        }
    }

    /// Determines which pages are present on the node's properties.
    ///
    /// Matches C++ `_ComputePages()`: preserves insertion order, deduplicates,
    /// and includes the empty-string page (properties with no page assignment).
    fn compute_pages(&self) -> SdrTokenVec {
        let mut pages: SdrTokenVec = Vec::new();
        for prop in &self.properties {
            let page = prop.get_page().clone();
            if !pages.contains(&page) {
                pages.push(page);
            }
        }
        pages
    }
}

impl Default for SdrShaderNode {
    fn default() -> Self {
        Self {
            is_valid: false,
            identifier: SdrIdentifier::default(),
            version: SdrVersion::default(),
            name: String::new(),
            family: Token::default(),
            context: Token::default(),
            source_type: Token::default(),
            definition_uri: String::new(),
            implementation_uri: String::new(),
            source_code: String::new(),
            properties: Vec::new(),
            legacy_metadata: SdrTokenMap::new(),
            metadata: SdrShaderNodeMetadata::new(),
            inputs: HashMap::new(),
            input_names: Vec::new(),
            outputs: HashMap::new(),
            output_names: Vec::new(),
            label: Token::default(),
            category: Token::default(),
            departments: Vec::new(),
            open_pages: Vec::new(),
            pages_shown_if: SdrTokenMap::new(),
            primvars: Vec::new(),
            primvar_naming_properties: Vec::new(),
            pages: Vec::new(),
        }
    }
}

/// Pointer to a const shader node.
pub type SdrShaderNodeConstPtr<'a> = &'a SdrShaderNode;

/// Unique pointer to a shader node.
pub type SdrShaderNodeUniquePtr = Box<SdrShaderNode>;

/// Vector of const shader node pointers.
pub type SdrShaderNodeConstPtrVec<'a> = Vec<SdrShaderNodeConstPtr<'a>>;

/// Alias for SdrShaderNodeConstPtrVec.
pub type SdrShaderNodePtrVec<'a> = SdrShaderNodeConstPtrVec<'a>;

/// Vector of unique shader node pointers.
pub type SdrShaderNodeUniquePtrVec = Vec<SdrShaderNodeUniquePtr>;

/// Compliance check results for property name checking.
pub type ComplianceResults = HashMap<Token, Vec<SdrIdentifier>>;

/// Checks if same-named input properties of shader nodes are compatible.
///
/// Matches C++ `SdrShaderNode::CheckPropertyCompliance()`:
/// - Iterates INPUTS only (not outputs)
/// - Compares GetTypeAsSdfType, GetDefaultValue, and GetDefaultValueAsSdfType
/// - First node providing a property is authoritative; others that differ
///   are recorded as non-compliant
///
/// Returns a map of property names to their non-compliant shader node identifiers.
/// An empty map means no compliance issues.
pub fn check_property_compliance(shader_nodes: &[&SdrShaderNode]) -> ComplianceResults {
    let mut results = ComplianceResults::new();
    // Maps property name → the first-seen authoritative property
    let mut authoritative: HashMap<Token, (SdrIdentifier, usize)> = HashMap::new();

    for (node_idx, node) in shader_nodes.iter().enumerate() {
        for prop_name in node.get_shader_input_names() {
            if let Some(prop) = node.get_shader_input(prop_name) {
                if let Some((_, auth_node_idx)) = authoritative.get(prop_name) {
                    // Compare against the authoritative node's property
                    let auth_node = shader_nodes[*auth_node_idx];
                    if let Some(auth_prop) = auth_node.get_shader_input(prop_name) {
                        let sdf_mismatch =
                            prop.get_type_as_sdf_type() != auth_prop.get_type_as_sdf_type();
                        let default_mismatch =
                            prop.get_default_value() != auth_prop.get_default_value();
                        let sdf_default_mismatch = prop.get_default_value_as_sdf_type()
                            != auth_prop.get_default_value_as_sdf_type();
                        if sdf_mismatch || default_mismatch || sdf_default_mismatch {
                            results
                                .entry(prop_name.clone())
                                .or_default()
                                .push(node.get_identifier().clone());
                        }
                    }
                } else {
                    // Record which index in shader_nodes this authoritative property came from
                    authoritative
                        .insert(prop_name.clone(), (node.get_identifier().clone(), node_idx));
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::super::shader_property::SdrShaderProperty;
    use super::*;

    fn make_test_node() -> SdrShaderNode {
        let mut props = Vec::new();

        // Add an input
        props.push(Box::new(SdrShaderProperty::new(
            Token::new("diffuseColor"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        )));

        // Add an output
        props.push(Box::new(SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        )));

        SdrShaderNode::new(
            Token::new("test_shader"),
            SdrVersion::new(1, 0),
            "test_shader".to_string(),
            Token::new("test"),
            Token::new("surface"),
            Token::new("OSL"),
            "/path/to/shader.osl".to_string(),
            "/path/to/shader.osl".to_string(),
            props,
            SdrShaderNodeMetadata::new(),
            String::new(),
        )
    }

    #[test]
    fn test_basic_node() {
        let node = make_test_node();

        assert!(node.is_valid());
        assert_eq!(node.get_identifier().as_str(), "test_shader");
        assert_eq!(node.get_name(), "test_shader");
        assert_eq!(node.get_context().as_str(), "surface");
    }

    #[test]
    fn test_inputs_outputs() {
        let node = make_test_node();

        let input_names = node.get_shader_input_names();
        assert_eq!(input_names.len(), 1);
        assert_eq!(input_names[0].as_str(), "diffuseColor");

        let output_names = node.get_shader_output_names();
        assert_eq!(output_names.len(), 1);
        assert_eq!(output_names[0].as_str(), "out");
    }

    #[test]
    fn test_get_shader_input() {
        let node = make_test_node();

        let input = node.get_shader_input(&Token::new("diffuseColor"));
        assert!(input.is_some());
        assert_eq!(input.unwrap().get_name().as_str(), "diffuseColor");

        let missing = node.get_shader_input(&Token::new("nonexistent"));
        assert!(missing.is_none());
    }

    #[test]
    fn test_info_string() {
        let node = make_test_node();
        let info = node.get_info_string();

        // C++ format: "ID (context: 'CTX', version: 'VER', family: 'FAM'); ..."
        assert!(info.contains("test_shader"));
        assert!(info.contains("context: 'surface'"));
        assert!(info.contains("family: 'test'"));
        assert!(info.contains("definition URI:"));
        assert!(info.contains("/path/to/shader.osl"));
    }

    // =========================================================
    // check_property_compliance
    // =========================================================

    /// Two nodes with the same property but different types → non-compliant.
    #[test]
    fn test_compliance_type_mismatch() {
        let mut props_a = Vec::new();
        props_a.push(Box::new(SdrShaderProperty::new(
            Token::new("color"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        )));
        let node_a = SdrShaderNode::new(
            Token::new("A"),
            SdrVersion::new(1, 0),
            "A".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props_a,
            SdrShaderNodeMetadata::new(),
            String::new(),
        );

        let mut props_b = Vec::new();
        props_b.push(Box::new(SdrShaderProperty::new(
            Token::new("color"),
            tokens().property_types.float.clone(), // different type!
            Value::default(),
            false,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        )));
        let node_b = SdrShaderNode::new(
            Token::new("B"),
            SdrVersion::new(1, 0),
            "B".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props_b,
            SdrShaderNodeMetadata::new(),
            String::new(),
        );

        let results = check_property_compliance(&[&node_a, &node_b]);
        assert!(!results.is_empty(), "expected non-compliance");
        assert!(results.contains_key(&Token::new("color")));
        let violators = &results[&Token::new("color")];
        assert!(violators.contains(&Token::new("B")));
    }

    /// All nodes share identical property types → no compliance issues.
    #[test]
    fn test_compliance_identical_types() {
        let node = make_test_node();
        let results = check_property_compliance(&[&node, &node]);
        assert!(results.is_empty());
    }

    /// Empty node list → empty results.
    #[test]
    fn test_compliance_empty_list() {
        let results = check_property_compliance(&[]);
        assert!(results.is_empty());
    }

    // =========================================================
    // Pages
    // =========================================================

    /// Properties with page metadata → get_pages() aggregates them.
    #[test]
    fn test_compute_pages() {
        use super::super::shader_property_metadata::SdrShaderPropertyMetadata;

        let mut meta_a = SdrShaderPropertyMetadata::new();
        meta_a.set_page(&Token::new("Advanced"));
        let mut meta_b = SdrShaderPropertyMetadata::new();
        meta_b.set_page(&Token::new("Basic"));
        // second property also in "Advanced" — should not duplicate
        let mut meta_c = SdrShaderPropertyMetadata::new();
        meta_c.set_page(&Token::new("Advanced"));

        let props = vec![
            Box::new(SdrShaderProperty::new(
                Token::new("a"),
                tokens().property_types.float.clone(),
                Value::default(),
                false,
                0,
                meta_a,
                SdrTokenMap::new(),
                Vec::new(),
            )),
            Box::new(SdrShaderProperty::new(
                Token::new("b"),
                tokens().property_types.float.clone(),
                Value::default(),
                false,
                0,
                meta_b,
                SdrTokenMap::new(),
                Vec::new(),
            )),
            Box::new(SdrShaderProperty::new(
                Token::new("out"),
                tokens().property_types.float.clone(),
                Value::default(),
                true,
                0,
                meta_c,
                SdrTokenMap::new(),
                Vec::new(),
            )),
        ];

        let node = SdrShaderNode::new(
            Token::new("pages_node"),
            SdrVersion::new(1, 0),
            "pages_node".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props,
            SdrShaderNodeMetadata::new(),
            String::new(),
        );

        let pages = node.get_pages();
        assert_eq!(pages.len(), 2, "expected exactly 2 distinct pages");
        let page_strs: Vec<&str> = pages.iter().map(|p| p.as_str()).collect();
        assert!(page_strs.contains(&"Advanced"));
        assert!(page_strs.contains(&"Basic"));
    }

    // =========================================================
    // Primvars via initialize_primvars
    // =========================================================

    /// $-prefixed primvar entry in node metadata + string input → shows up in additional_primvar_properties.
    ///
    /// Matches C++ `_InitializePrimvars()` $-prefix resolution logic.
    #[test]
    fn test_primvar_naming_property() {
        // Node metadata sets primvars = ["$varname"] (dollar-prefix = property reference)
        let mut node_meta = SdrShaderNodeMetadata::new();
        node_meta.set_item(
            tokens().node_metadata.primvars.clone(),
            Value::from(vec!["$varname".to_string()]),
        );

        let props = vec![
            Box::new(SdrShaderProperty::new(
                Token::new("varname"),
                tokens().property_types.string.clone(), // must be string type
                Value::default(),
                false,
                0,
                super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
                SdrTokenMap::new(),
                Vec::new(),
            )),
            Box::new(SdrShaderProperty::new(
                Token::new("out"),
                tokens().property_types.float.clone(),
                Value::default(),
                true,
                0,
                super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
                SdrTokenMap::new(),
                Vec::new(),
            )),
        ];

        let node = SdrShaderNode::new(
            Token::new("primvar_node"),
            SdrVersion::new(1, 0),
            "primvar_node".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props,
            node_meta,
            String::new(),
        );

        assert!(
            node.get_additional_primvar_properties()
                .contains(&Token::new("varname")),
            "varname should be listed as a primvar-naming property"
        );
        assert!(
            node.get_primvars().is_empty(),
            "no literal primvars should be present"
        );
    }

    /// Literal primvar entry (no $) → shows up in get_primvars(), not additional_primvar_properties.
    #[test]
    fn test_literal_primvar() {
        let mut node_meta = SdrShaderNodeMetadata::new();
        node_meta.set_item(
            tokens().node_metadata.primvars.clone(),
            Value::from(vec!["st".to_string(), "N".to_string()]),
        );

        let props = vec![Box::new(SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.float.clone(),
            Value::default(),
            true,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        ))];

        let node = SdrShaderNode::new(
            Token::new("lit_pv_node"),
            SdrVersion::new(1, 0),
            "lit_pv_node".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props,
            node_meta,
            String::new(),
        );

        let pvs = node.get_primvars();
        assert!(pvs.contains(&Token::new("st")));
        assert!(pvs.contains(&Token::new("N")));
        assert!(node.get_additional_primvar_properties().is_empty());
    }

    // =========================================================
    // get_role
    // =========================================================

    #[test]
    fn test_get_role_from_metadata() {
        use super::super::shader_node_metadata::SdrShaderNodeMetadata;

        let mut meta = SdrShaderNodeMetadata::new();
        meta.set_item(
            tokens().node_metadata.role.clone(),
            Value::from("texture".to_string()),
        );

        let props = vec![Box::new(SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        ))];

        let node = SdrShaderNode::new(
            Token::new("role_node"),
            SdrVersion::new(1, 0),
            "role_node".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            props,
            meta,
            String::new(),
        );

        assert_eq!(node.get_role().as_str(), "texture");
    }

    #[test]
    fn test_get_role_empty_by_default() {
        let node = make_test_node();
        // C++: HasRole() ? GetRole() : TfToken(GetName())
        // No role set → falls back to node name
        assert_eq!(node.get_role().as_str(), node.get_name());
    }

    // =========================================================
    // VStruct
    // =========================================================

    /// Properties marked as vstruct members are detected by is_vstruct_member.
    #[test]
    fn test_vstruct_member_detection() {
        use super::super::shader_property_metadata::SdrShaderPropertyMetadata;

        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_item(
            tokens().property_metadata.vstruct_member_of.clone(),
            Value::from("myVstruct".to_string()),
        );
        meta.set_item(
            tokens().property_metadata.vstruct_member_name.clone(),
            Value::from("r".to_string()),
        );

        let prop = SdrShaderProperty::new(
            Token::new("vsChannel"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            meta,
            SdrTokenMap::new(),
            Vec::new(),
        );

        assert!(prop.is_vstruct_member());
        assert_eq!(prop.get_vstruct_member_of().as_str(), "myVstruct");
        assert_eq!(prop.get_vstruct_member_name().as_str(), "r");
        assert!(!prop.is_vstruct()); // head vstruct, not a member
    }

    /// A property with property_type == vstruct is a vstruct head.
    #[test]
    fn test_is_vstruct_head() {
        let mut prop = SdrShaderProperty::new(
            Token::new("myVs"),
            tokens().property_types.float.clone(), // start as float
            Value::default(),
            false,
            0,
            super::super::shader_property_metadata::SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        );
        // convert_to_vstruct sets the type
        prop.convert_to_vstruct();
        assert!(prop.is_vstruct());
        assert!(!prop.is_vstruct_member());
    }
}
