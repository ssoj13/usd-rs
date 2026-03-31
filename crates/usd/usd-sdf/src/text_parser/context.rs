//! Parser context for the USDA text parser.
//!
//! This module provides the `TextParserContext` struct which maintains all
//! state during parsing. It is modeled after the C++ `Sdf_TextParserContext`
//! class in `textParserContext.h`.
//!
//! # Context Stack
//!
//! The parser maintains a stack of parsing contexts to handle nested structures
//! like prims, metadata blocks, and dictionaries. Each context level tracks
//! what kind of construct is being parsed.
//!
//! # State Categories
//!
//! - **Layer state**: Header info, sublayers, hints
//! - **Type state**: Current prim/attribute/dictionary types
//! - **Value state**: Current value being built, dictionaries, time samples
//! - **Spec state**: Specifier, path, variability
//! - **Hierarchy state**: Child/property name stacks, variants

use std::collections::HashMap;

use crate::{
    LayerHints, LayerOffset, ListOpType, Path, PathVector, PayloadVector, ReferenceVector,
    RelocatesMap, Specifier, Variability,
};
use usd_tf::Token;

use super::error::SourceLocation;

// ============================================================================
// Parsing Context Enum
// ============================================================================

/// The current parsing context.
///
/// Indicates what kind of construct the parser is currently processing.
/// Used to disambiguate values and provide appropriate error messages.
///
/// This enum mirrors `Sdf_TextParserCurrentParsingContext` from C++.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsingContext {
    /// Parsing layer-level content
    LayerSpec,
    /// Parsing a prim spec
    PrimSpec,
    /// Parsing an attribute spec
    AttributeSpec,
    /// Parsing a relationship spec
    RelationshipSpec,
    /// Parsing generic metadata block
    Metadata,
    /// Parsing key=value metadata
    KeyValueMetadata,
    /// Parsing list operation metadata
    ListOpMetadata,
    /// Parsing doc metadata
    DocMetadata,
    /// Parsing permission metadata
    PermissionMetadata,
    /// Parsing symmetryFunction metadata
    SymmetryFunctionMetadata,
    /// Parsing displayUnit metadata
    DisplayUnitMetadata,
    /// Parsing a dictionary value
    Dictionary,
    /// Parsing dictionary type name
    DictionaryTypeName,
    /// Parsing dictionary key
    DictionaryKey,
    /// Parsing attribute connection
    ConnectAttribute,
    /// Parsing reorder rootPrims
    ReorderRootPrims,
    /// Parsing reorder nameChildren
    ReorderNameChildren,
    /// Parsing reorder properties
    ReorderProperties,
    /// Parsing references list op
    ReferencesListOpMetadata,
    /// Parsing payload list op
    PayloadListOpMetadata,
    /// Parsing inherits list op
    InheritsListOpMetadata,
    /// Parsing specializes list op
    SpecializesListOpMetadata,
    /// Parsing variants metadata
    VariantsMetadata,
    /// Parsing variantSets metadata
    VariantSetsMetadata,
    /// Parsing relocates metadata
    RelocatesMetadata,
    /// Parsing kind metadata
    KindMetadata,
    /// Parsing relationship assignment
    RelationshipAssignment,
    /// Parsing relationship target
    RelationshipTarget,
    /// Parsing relationship default
    RelationshipDefault,
    /// Parsing time samples
    TimeSamples,
    /// Parsing spline values
    SplineValues,
    /// Parsing spline knot item
    SplineKnotItem,
    /// Parsing spline post extrapolation
    SplinePostExtrapItem,
    /// Parsing spline pre extrapolation
    SplinePreExtrapItem,
    /// Parsing sloped extrapolation
    SplineExtrapSloped,
    /// Parsing spline loop keyword
    SplineKeywordLoop,
    /// Parsing spline knot parameters
    SplineKnotParam,
    /// Parsing spline tangent
    SplineTangent,
    /// Parsing spline tangent with width
    SplineTangentWithWidth,
    /// Parsing spline interpolation mode
    SplineInterpMode,
    /// Parsing reference parameters
    ReferenceParameters,
    /// Parsing layer offset
    LayerOffsetContext,
    /// Parsing layer scale
    LayerScale,
    /// Parsing variant set statement
    VariantSetStatement,
    /// Parsing variant statement list
    VariantStatementList,
    /// Parsing prefix substitutions
    PrefixSubstitutionsMetadata,
    /// Parsing suffix substitutions
    SuffixSubstitutionsMetadata,
    /// Parsing sublayer metadata
    SubLayerMetadata,
}

// ============================================================================
// Parsed Value
// ============================================================================

/// A value parsed from the text file.
///
/// This is a temporary representation used during parsing before
/// conversion to the final VtValue type.
#[derive(Debug, Clone, Default)]
pub enum ParsedValue {
    /// No value (None keyword)
    #[default]
    None,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Unsigned integer value
    UInt(u64),
    /// Floating-point value
    Float(f64),
    /// String value
    String(String),
    /// Token value
    Token(Token),
    /// Asset path
    AssetPath(String),
    /// SDF path
    Path(Path),
    /// Tuple of values
    Tuple(Vec<ParsedValue>),
    /// List of values
    List(Vec<ParsedValue>),
    /// Dictionary of values
    Dictionary(HashMap<String, ParsedValue>),
    /// Time sample map
    TimeSamples(Vec<(f64, ParsedValue)>),
    /// Animation block marker
    AnimationBlock,
}

// ============================================================================
// Text Parser Context
// ============================================================================

/// Global state maintained during parsing of a USD text file.
///
/// This struct contains all the state needed to parse a complete USD layer,
/// including:
/// - Header information (magic identifier, version)
/// - Type names being built
/// - Parsing context stack
/// - Value building state
/// - Spec state (specifier, path, variability)
/// - Hierarchy state (children, properties, variants)
///
/// # C++ Parity
///
/// This struct mirrors `Sdf_TextParserContext` from the C++ USD library.
/// All fields have corresponding equivalents in the C++ implementation.
#[derive(Debug)]
pub struct TextParserContext {
    // ========================================================================
    // Header State
    // ========================================================================
    /// Magic identifier token (e.g., "usda" or "sdf")
    pub magic_identifier: String,

    /// Version string (e.g., "1.0")
    pub version_string: String,

    /// File context (path being parsed)
    pub file_context: String,

    // ========================================================================
    // Layer Reference State
    // ========================================================================
    /// Current layer reference path being built
    pub layer_ref_path: String,

    /// Current layer reference offset being built
    pub layer_ref_offset: LayerOffset,

    /// Sublayer paths
    pub sublayer_paths: Vec<String>,

    /// Sublayer offsets
    pub sublayer_offsets: Vec<LayerOffset>,

    /// Default prim name (from layer metadata)
    pub default_prim: Option<String>,

    // ========================================================================
    // Type Name State
    // ========================================================================
    /// Prim type name being built
    pub prim_type_name: String,

    /// Attribute type name being built
    pub attribute_type_name: String,

    /// Dictionary type name being built
    pub dictionary_type_name: String,

    /// Symmetry function name
    pub symmetry_function_name: String,

    // ========================================================================
    // Parsing Context Stack
    // ========================================================================
    /// Stack of parsing contexts
    pub parsing_context: Vec<ParsingContext>,

    // ========================================================================
    // Name/Token Building
    // ========================================================================
    /// String list being built (for name lists, etc.)
    pub name_vector: Vec<Token>,

    // ========================================================================
    // Time Samples State
    // ========================================================================
    /// Time samples being built
    pub time_samples: Vec<(f64, ParsedValue)>,

    /// Current time sample time
    pub time_sample_time: f64,

    // ========================================================================
    // Path State
    // ========================================================================
    /// Saved path for various uses
    pub saved_path: Path,

    // ========================================================================
    // Relationship Parsing State
    // ========================================================================
    /// Whether relationship target data is allowed
    pub rel_parsing_allow_target_data: bool,

    /// Relationship target paths being built
    pub rel_parsing_target_paths: Option<PathVector>,

    /// New relationship target children
    pub rel_parsing_new_target_children: PathVector,

    // ========================================================================
    // Connection Parsing State
    // ========================================================================
    /// Connection target paths being built
    pub conn_parsing_target_paths: PathVector,

    /// Whether connection data is allowed
    pub conn_parsing_allow_connection_data: bool,

    // ========================================================================
    // Inherit/Specialize Parsing State
    // ========================================================================
    /// Inherit target paths being built
    pub inherit_parsing_target_paths: PathVector,

    /// Specializes target paths being built
    pub specializes_parsing_target_paths: PathVector,

    // ========================================================================
    // Reference/Payload Parsing State
    // ========================================================================
    /// References being built
    pub reference_parsing_refs: ReferenceVector,

    /// Payloads being built
    pub payload_parsing_refs: PayloadVector,

    // ========================================================================
    // Relocates Parsing State
    // ========================================================================
    /// Relocates map being built
    pub relocates_parsing: RelocatesMap,

    /// Current relocates key path
    pub relocates_key: Path,

    /// Whether we've seen the first relocates path
    pub seen_first_relocates_path: bool,

    // ========================================================================
    // String Dictionary State
    // ========================================================================
    /// Current string dictionary key
    pub string_dictionary_key: String,

    /// Whether we've seen a string dictionary key
    pub seen_string_dictionary_key: bool,

    // ========================================================================
    // Generic Metadata State
    // ========================================================================
    /// Generic metadata key being built
    pub generic_metadata_key: Token,

    /// Current list operation type
    pub list_op_type: ListOpType,

    // ========================================================================
    // Value State
    // ========================================================================
    /// Last parsed value
    pub current_value: ParsedValue,

    /// Stack of dictionaries for nested parsing
    pub current_dictionaries: Vec<HashMap<String, ParsedValue>>,

    /// Stack of dictionary keys
    pub current_dictionary_key: Vec<String>,

    /// Stack of "expect dictionary value" flags
    pub expect_dictionary_value: Vec<bool>,

    // ========================================================================
    // Spec State
    // ========================================================================
    /// Is current property custom?
    pub custom: bool,

    /// Current prim specifier
    pub specifier: Specifier,

    /// Current path being built
    pub path: Path,

    /// Current variability
    pub variability: Variability,

    // ========================================================================
    // Layer Hints
    // ========================================================================
    /// Hints about layer contents
    pub layer_hints: LayerHints,

    // ========================================================================
    // Hierarchy State
    // ========================================================================
    /// Stack of child names per prim level
    pub name_children_stack: Vec<Vec<Token>>,

    /// Stack of property names per prim level
    pub properties_stack: Vec<Vec<Token>>,

    // ========================================================================
    // Variant State
    // ========================================================================
    /// Stack of variant set names
    pub current_variant_set_names: Vec<String>,

    /// Stack of variant names per variant set
    pub current_variant_names: Vec<Vec<String>>,

    // ========================================================================
    // Spline State (for animation curves)
    // ========================================================================
    /// Whether current spline is valid
    pub spline_valid: bool,

    /// Spline pre-extrapolation slope
    pub spline_pre_slope: f64,

    /// Spline post-extrapolation slope
    pub spline_post_slope: f64,

    /// Spline loop parameters: [proto_start, proto_end, num_pre_loops, num_post_loops, value_offset]
    pub spline_loop_params: [f64; 5],

    // ========================================================================
    // Array Edit State
    // ========================================================================
    /// Array edit size argument
    pub array_edit_size_arg: i64,

    /// Whether array edit has fill value
    pub array_edit_has_fill: bool,

    /// Array edit reference indexes [0] and [1]
    pub array_edit_reference_indexes: [i64; 2],

    /// Which reference indexes are present (bitmask)
    pub array_edit_reference_presence: u8,

    // ========================================================================
    // Error Recovery
    // ========================================================================
    /// Current error location for recovery
    pub error_location: SourceLocation,

    /// Whether we're in error recovery mode
    pub in_error_recovery: bool,
}

impl TextParserContext {
    /// Creates a new empty parser context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Header
            magic_identifier: String::new(),
            version_string: String::new(),
            file_context: String::new(),

            // Layer refs
            layer_ref_path: String::new(),
            layer_ref_offset: LayerOffset::identity(),
            sublayer_paths: Vec::new(),
            sublayer_offsets: Vec::new(),
            default_prim: None,

            // Type names
            prim_type_name: String::new(),
            attribute_type_name: String::new(),
            dictionary_type_name: String::new(),
            symmetry_function_name: String::new(),

            // Parsing context
            parsing_context: Vec::new(),

            // Names
            name_vector: Vec::new(),

            // Time samples
            time_samples: Vec::new(),
            time_sample_time: 0.0,

            // Paths
            saved_path: Path::empty(),

            // Relationships
            rel_parsing_allow_target_data: false,
            rel_parsing_target_paths: None,
            rel_parsing_new_target_children: PathVector::new(),

            // Connections
            conn_parsing_target_paths: PathVector::new(),
            conn_parsing_allow_connection_data: false,

            // Inherits/specializes
            inherit_parsing_target_paths: PathVector::new(),
            specializes_parsing_target_paths: PathVector::new(),

            // References/payloads
            reference_parsing_refs: ReferenceVector::new(),
            payload_parsing_refs: PayloadVector::new(),

            // Relocates
            relocates_parsing: RelocatesMap::new(),
            relocates_key: Path::empty(),
            seen_first_relocates_path: false,

            // String dictionary
            string_dictionary_key: String::new(),
            seen_string_dictionary_key: false,

            // Generic metadata
            generic_metadata_key: Token::empty(),
            list_op_type: ListOpType::Explicit,

            // Values
            current_value: ParsedValue::None,
            current_dictionaries: Vec::new(),
            current_dictionary_key: Vec::new(),
            expect_dictionary_value: Vec::new(),

            // Spec state
            custom: false,
            specifier: Specifier::Def,
            path: Path::absolute_root(),
            variability: Variability::Varying,

            // Hints
            layer_hints: LayerHints::default(),

            // Hierarchy
            name_children_stack: Vec::new(),
            properties_stack: Vec::new(),

            // Variants
            current_variant_set_names: Vec::new(),
            current_variant_names: Vec::new(),

            // Splines
            spline_valid: false,
            spline_pre_slope: 0.0,
            spline_post_slope: 0.0,
            spline_loop_params: [0.0; 5],

            // Array edits
            array_edit_size_arg: -1,
            array_edit_has_fill: false,
            array_edit_reference_indexes: [0, 0],
            array_edit_reference_presence: 0,

            // Error recovery
            error_location: SourceLocation::unknown(),
            in_error_recovery: false,
        }
    }

    /// Resets the context for a new parse.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Sets the file context (path being parsed).
    pub fn set_file_context(&mut self, path: impl Into<String>) {
        self.file_context = path.into();
    }

    // ========================================================================
    // Parsing Context Stack
    // ========================================================================

    /// Pushes a new parsing context onto the stack.
    pub fn push_context(&mut self, ctx: ParsingContext) {
        self.parsing_context.push(ctx);
    }

    /// Pops the current parsing context.
    pub fn pop_context(&mut self) -> Option<ParsingContext> {
        self.parsing_context.pop()
    }

    /// Returns the current parsing context.
    #[must_use]
    pub fn current_context(&self) -> Option<ParsingContext> {
        self.parsing_context.last().copied()
    }

    /// Returns true if currently parsing the given context.
    #[must_use]
    pub fn is_in_context(&self, ctx: ParsingContext) -> bool {
        self.parsing_context.last() == Some(&ctx)
    }

    /// Returns the context stack depth.
    #[must_use]
    pub fn context_depth(&self) -> usize {
        self.parsing_context.len()
    }

    // ========================================================================
    // Dictionary Stack
    // ========================================================================

    /// Starts a new dictionary.
    pub fn begin_dictionary(&mut self) {
        self.current_dictionaries.push(HashMap::new());
        self.current_dictionary_key.push(String::new());
        self.expect_dictionary_value.push(false);
    }

    /// Ends the current dictionary and returns it.
    pub fn end_dictionary(&mut self) -> Option<HashMap<String, ParsedValue>> {
        self.current_dictionary_key.pop();
        self.expect_dictionary_value.pop();
        self.current_dictionaries.pop()
    }

    /// Returns mutable reference to current dictionary.
    pub fn current_dictionary_mut(&mut self) -> Option<&mut HashMap<String, ParsedValue>> {
        self.current_dictionaries.last_mut()
    }

    // ========================================================================
    // Hierarchy Stack
    // ========================================================================

    /// Pushes a new prim level onto the hierarchy stack.
    pub fn push_prim_level(&mut self) {
        self.name_children_stack.push(Vec::new());
        self.properties_stack.push(Vec::new());
    }

    /// Pops the current prim level from the hierarchy stack.
    pub fn pop_prim_level(&mut self) {
        self.name_children_stack.pop();
        self.properties_stack.pop();
    }

    /// Adds a child name to the current prim level.
    pub fn add_child_name(&mut self, name: Token) {
        if let Some(children) = self.name_children_stack.last_mut() {
            children.push(name);
        }
    }

    /// Adds a property name to the current prim level.
    pub fn add_property_name(&mut self, name: Token) {
        if let Some(properties) = self.properties_stack.last_mut() {
            properties.push(name);
        }
    }

    // ========================================================================
    // Variant Stack
    // ========================================================================

    /// Begins a variant set.
    pub fn begin_variant_set(&mut self, name: impl Into<String>) {
        self.current_variant_set_names.push(name.into());
        self.current_variant_names.push(Vec::new());
    }

    /// Ends the current variant set.
    pub fn end_variant_set(&mut self) -> Option<(String, Vec<String>)> {
        let name = self.current_variant_set_names.pop()?;
        let variants = self.current_variant_names.pop()?;
        Some((name, variants))
    }

    /// Adds a variant name to the current variant set.
    pub fn add_variant_name(&mut self, name: impl Into<String>) {
        if let Some(variants) = self.current_variant_names.last_mut() {
            variants.push(name.into());
        }
    }

    // ========================================================================
    // Path Building
    // ========================================================================

    /// Appends a prim child to the current path.
    pub fn append_prim_child(&mut self, name: &str) -> bool {
        if let Some(new_path) = self.path.append_child(name) {
            self.path = new_path;
            true
        } else {
            false
        }
    }

    /// Appends a property to the current path.
    pub fn append_property(&mut self, name: &str) -> bool {
        if let Some(new_path) = self.path.append_property(name) {
            self.path = new_path;
            true
        } else {
            false
        }
    }

    /// Moves to the parent path.
    pub fn pop_path(&mut self) {
        self.path = self.path.get_parent_path();
    }

    /// Resets path to absolute root.
    pub fn reset_path(&mut self) {
        self.path = Path::absolute_root();
    }
}

impl Default for TextParserContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = TextParserContext::new();
        assert!(ctx.magic_identifier.is_empty());
        assert!(ctx.parsing_context.is_empty());
        assert_eq!(ctx.path, Path::absolute_root());
    }

    #[test]
    fn test_context_stack() {
        let mut ctx = TextParserContext::new();

        ctx.push_context(ParsingContext::LayerSpec);
        assert_eq!(ctx.current_context(), Some(ParsingContext::LayerSpec));

        ctx.push_context(ParsingContext::PrimSpec);
        assert_eq!(ctx.current_context(), Some(ParsingContext::PrimSpec));
        assert_eq!(ctx.context_depth(), 2);

        ctx.pop_context();
        assert_eq!(ctx.current_context(), Some(ParsingContext::LayerSpec));
    }

    #[test]
    fn test_dictionary_stack() {
        let mut ctx = TextParserContext::new();

        ctx.begin_dictionary();
        assert!(ctx.current_dictionary_mut().is_some());

        ctx.begin_dictionary();
        assert_eq!(ctx.current_dictionaries.len(), 2);

        let dict = ctx.end_dictionary();
        assert!(dict.is_some());
        assert_eq!(ctx.current_dictionaries.len(), 1);
    }

    #[test]
    fn test_hierarchy_stack() {
        let mut ctx = TextParserContext::new();

        ctx.push_prim_level();
        ctx.add_child_name(Token::new("child1"));
        ctx.add_property_name(Token::new("prop1"));

        assert_eq!(ctx.name_children_stack.len(), 1);
        assert_eq!(ctx.name_children_stack[0].len(), 1);

        ctx.pop_prim_level();
        assert!(ctx.name_children_stack.is_empty());
    }

    #[test]
    fn test_path_building() {
        let mut ctx = TextParserContext::new();

        assert!(ctx.append_prim_child("World"));
        assert_eq!(ctx.path.get_string(), "/World");

        assert!(ctx.append_prim_child("Cube"));
        assert_eq!(ctx.path.get_string(), "/World/Cube");

        ctx.pop_path();
        assert_eq!(ctx.path.get_string(), "/World");

        ctx.reset_path();
        assert_eq!(ctx.path, Path::absolute_root());
    }

    #[test]
    fn test_variant_stack() {
        let mut ctx = TextParserContext::new();

        ctx.begin_variant_set("modelingVariant");
        ctx.add_variant_name("default");
        ctx.add_variant_name("highRes");

        let (name, variants) = ctx.end_variant_set().unwrap();
        assert_eq!(name, "modelingVariant");
        assert_eq!(variants, vec!["default", "highRes"]);
    }
}
