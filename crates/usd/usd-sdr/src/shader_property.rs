//! SDR Shader Property - Shader property (input/output) representation.
//!
//! Port of pxr/usd/sdr/shaderProperty.h
//!
//! This module provides SdrShaderProperty which represents a property (input
//! or output) that is part of a shader node. Properties have names, types,
//! default values, and various metadata that control their behavior in
//! shader networks.
//!
//! Used by: SdrShaderNode
//! Uses: Token, VtValue, SdrSdfTypeIndicator, SdrShaderPropertyMetadata

use super::declare::{SdrOptionVec, SdrTokenMap, SdrTokenVec};
use super::sdf_type_indicator::SdrSdfTypeIndicator;
use super::shader_property_metadata::SdrShaderPropertyMetadata;
use super::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

/// Represents a property (input or output) that is part of a SdrShaderNode.
///
/// A property must have a name and type, but may also specify a host of
/// additional metadata. Instances can also be queried to determine if another
/// SdrShaderProperty instance can be connected to it.
///
/// # Property Types
///
/// SDR defines several standard property types:
/// - int, float, string - basic scalar types
/// - color, color4 - color types
/// - point, normal, vector - 3D vector types
/// - matrix - 4x4 transformation matrix
/// - struct, vstruct - composite types
/// - terminal - connection-only type
///
/// # Connection Rules
///
/// Properties can be connected if their types are compatible. The
/// `can_connect_to()` method checks type compatibility using the
/// `valid_connection_types` metadata or type matching.
#[derive(Debug, Clone)]
pub struct SdrShaderProperty {
    // Basic properties
    name: Token,
    property_type: Token,
    default_value: Value,
    is_output: bool,
    array_size: i32,
    tuple_size: i32,
    is_dynamic_array: bool,
    is_connectable: bool,

    // Metadata
    legacy_metadata: SdrTokenMap,
    metadata: SdrShaderPropertyMetadata,
    hints: SdrTokenMap,
    options: SdrOptionVec,

    // Tokenized metadata cached for fast access
    valid_connection_types: SdrTokenVec,
    label: Token,
    page: Token,
    widget: Token,
    vstruct_member_of: Token,
    vstruct_member_name: Token,
    vstruct_conditional_expr: Token,

    // SDF type conversion
    sdf_type_default_value: Value,

    // USD encoding version for type conversion behavior
    usd_encoding_version: i32,
}

impl SdrShaderProperty {
    /// Creates a new shader property.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: Token,
        property_type: Token,
        default_value: Value,
        is_output: bool,
        array_size: usize,
        metadata: SdrShaderPropertyMetadata,
        hints: SdrTokenMap,
        options: SdrOptionVec,
    ) -> Self {
        // Apply role-based type and array-size conversion (C++ _ConvertSdrPropertyTypeAndArraySize).
        // Certain SDR types (Color, Color4, Point, Normal, Vector) with role="none" are
        // converted to Float with the appropriate array size.
        let (property_type, array_size) =
            Self::convert_type_and_array_size(property_type, array_size, &metadata);

        // Extract cached metadata values
        let is_dynamic_array = metadata.get_is_dynamic_array();
        let tuple_size = metadata.get_tuple_size();

        // Outputs are ALWAYS connectable. If "connectable" metadata is found on
        // an output, ignore it (C++ constructor comment: "Note that outputs are always
        // connectable. If 'connectable' metadata is found on outputs, ignore it.")
        let is_connectable = if is_output {
            true
        } else if metadata.has_connectable() {
            metadata.get_connectable()
        } else {
            true // default is connectable
        };

        let valid_connection_types = metadata.get_valid_connection_types();
        let label = metadata.get_label();
        let page = metadata.get_page();

        // Ensure widget defaults to "default" when not specified (C++ constructor)
        let mut metadata = metadata;
        if !metadata.has_widget() {
            metadata.set_widget(&Token::new("default"));
        }
        let widget = metadata.get_widget();

        // VStruct metadata
        let vstruct_member_of = metadata
            .get_item_value_as::<String>(&tokens().property_metadata.vstruct_member_of)
            .map(|s| Token::new(&s))
            .unwrap_or_default();

        let vstruct_member_name = metadata
            .get_item_value_as::<String>(&tokens().property_metadata.vstruct_member_name)
            .map(|s| Token::new(&s))
            .unwrap_or_default();

        let vstruct_conditional_expr = metadata
            .get_item_value_as::<String>(&tokens().property_metadata.vstruct_conditional_expr)
            .map(|s| Token::new(&s))
            .unwrap_or_default();

        let legacy_metadata = metadata.encode_legacy_metadata();

        Self {
            name,
            property_type,
            default_value: default_value.clone(),
            is_output,
            array_size: array_size as i32,
            tuple_size,
            is_dynamic_array,
            is_connectable,
            legacy_metadata,
            metadata,
            hints,
            options,
            valid_connection_types,
            label,
            page,
            widget,
            vstruct_member_of,
            vstruct_member_name,
            vstruct_conditional_expr,
            sdf_type_default_value: default_value,
            // Matches C++ _UsdEncodingVersionsCurrent = _UsdEncodingVersions1 = 1
            usd_encoding_version: 1,
        }
    }

    /// Converts the SDR property type and array size based on role metadata.
    ///
    /// Matches C++ `_ConvertSdrPropertyTypeAndArraySize()`: certain semantic types
    /// (Color, Color4, Point, Normal, Vector) with role="none" are demoted to Float
    /// with the corresponding component count (3 or 4).
    fn convert_type_and_array_size(
        prop_type: Token,
        array_size: usize,
        metadata: &SdrShaderPropertyMetadata,
    ) -> (Token, usize) {
        let role = metadata.get_role();
        if prop_type.as_str().is_empty() || role.is_empty() {
            return (prop_type, array_size);
        }

        let role_tok = Token::new(&role);
        let types = &tokens().property_types;
        let roles = &tokens().property_role;

        // Only the "none" role triggers conversion (C++ _GetConvertedSdrTypes table)
        if role_tok != roles.none {
            return (prop_type, array_size);
        }

        // Color, Point, Normal, Vector → Float[3]; Color4 → Float[4]
        if prop_type == types.color
            || prop_type == types.point
            || prop_type == types.normal
            || prop_type == types.vector
        {
            return (types.float.clone(), 3);
        }
        if prop_type == types.color4 {
            return (types.float.clone(), 4);
        }

        (prop_type, array_size)
    }

    // ========================================================================
    // The Basics
    // ========================================================================

    /// Gets the name of the property.
    pub fn get_name(&self) -> &Token {
        &self.name
    }

    /// Gets the type of the property.
    pub fn get_type(&self) -> &Token {
        &self.property_type
    }

    /// Gets this property's default value associated with the type of the property.
    pub fn get_default_value(&self) -> &Value {
        &self.default_value
    }

    /// Whether this property is an output.
    pub fn is_output(&self) -> bool {
        self.is_output
    }

    /// Whether this property's type is an array type.
    pub fn is_array(&self) -> bool {
        self.array_size > 0 || self.is_dynamic_array
    }

    /// Whether this property's array type is dynamically-sized.
    pub fn is_dynamic_array(&self) -> bool {
        self.is_dynamic_array
    }

    /// Gets this property's array size.
    ///
    /// If this property is a fixed-size array type, the array size is returned.
    /// In the case of a dynamically-sized array, this method returns the array
    /// size that the parser reports, and should not be relied upon to be
    /// accurate. A parser may report -1 for the array size, for example, to
    /// indicate a dynamically-sized array. For types that are not a fixed-size
    /// array or dynamic array, this returns 0.
    pub fn get_array_size(&self) -> i32 {
        self.array_size
    }

    /// Gets this property's tuple size.
    ///
    /// The tuple size indicates an array's "column count", or how many elements
    /// it takes to form a logical row. For non-dynamic arrays, the array size
    /// should be a multiple of the tuple size.
    ///
    /// If no tuple size is specified, returns 0.
    pub fn get_tuple_size(&self) -> i32 {
        self.tuple_size
    }

    /// Gets a string with basic information about this property.
    ///
    /// Matches C++ format: "NAME (type: 'TYPE'); output|input"
    pub fn get_info_string(&self) -> String {
        let direction = if self.is_output { "output" } else { "input" };
        format!(
            "{} (type: '{}'); {}",
            self.name.as_str(),
            self.property_type.as_str(),
            direction
        )
    }

    // ========================================================================
    // Metadata
    // ========================================================================

    /// All of the metadata that came from the parse process (legacy format).
    pub fn get_metadata(&self) -> &SdrTokenMap {
        &self.legacy_metadata
    }

    /// All of the metadata that came from the parse process (new format).
    pub fn get_metadata_object(&self) -> &SdrShaderPropertyMetadata {
        &self.metadata
    }

    /// The label assigned to this property, if any.
    ///
    /// Distinct from the name returned from `get_name()`. In the context of a UI,
    /// the label value might be used as the display name for the property.
    pub fn get_label(&self) -> &Token {
        &self.label
    }

    /// The help message assigned to this property, if any.
    pub fn get_help(&self) -> String {
        self.metadata.get_help()
    }

    /// The page (group), eg "Advanced", this property appears on, if any.
    ///
    /// Note that the page for a shader property can be nested, delimited by ":",
    /// representing the hierarchy of sub-pages a property is defined in.
    pub fn get_page(&self) -> &Token {
        &self.page
    }

    /// The widget "hint" that indicates the widget that can best display the
    /// type of data contained in this property, if any.
    ///
    /// Examples include "number", "slider", etc.
    pub fn get_widget(&self) -> &Token {
        &self.widget
    }

    /// Any UI "hints" that are associated with this property.
    ///
    /// "Hints" are simple key/value pairs.
    pub fn get_hints(&self) -> &SdrTokenMap {
        &self.hints
    }

    /// If the property has a set of valid values that are pre-determined, this
    /// will return the valid option names and corresponding values.
    pub fn get_options(&self) -> &SdrOptionVec {
        &self.options
    }

    /// Returns the implementation name of this property.
    ///
    /// The name of the property is how to refer to the property in shader
    /// networks. The label is how to present this property to users. The
    /// implementation name is the name of the parameter this property
    /// represents in the implementation.
    pub fn get_implementation_name(&self) -> String {
        let impl_name = self.metadata.get_implementation_name();
        if impl_name.is_empty() {
            self.name.as_str().to_string()
        } else {
            impl_name
        }
    }

    /// A boolean expression that determines if the property should be shown
    /// in the UI based on the state of other properties.
    pub fn get_shown_if(&self) -> String {
        self.metadata.get_shown_if()
    }

    // ========================================================================
    // VStruct Information
    // ========================================================================

    /// If this field is part of a vstruct, this is the name of the struct.
    pub fn get_vstruct_member_of(&self) -> &Token {
        &self.vstruct_member_of
    }

    /// If this field is part of a vstruct, this is its name in the struct.
    pub fn get_vstruct_member_name(&self) -> &Token {
        &self.vstruct_member_name
    }

    /// Returns true if this field is part of a vstruct.
    ///
    /// Matches C++ `IsVStructMember()`: checks VstructMemberName presence,
    /// not VstructMemberOf (mirrors `_metadata.HasItem(VstructMemberName)`).
    pub fn is_vstruct_member(&self) -> bool {
        !self.vstruct_member_name.as_str().is_empty()
    }

    /// Returns true if the field is the head of a vstruct.
    pub fn is_vstruct(&self) -> bool {
        self.property_type == tokens().property_types.vstruct
    }

    /// If this field is part of a vstruct, this is the conditional expression.
    pub fn get_vstruct_conditional_expr(&self) -> &Token {
        &self.vstruct_conditional_expr
    }

    // ========================================================================
    // Connection Information
    // ========================================================================

    /// Whether this property can be connected to other properties.
    ///
    /// If this returns `true`, connectability to a specific property can be
    /// tested via `can_connect_to()`.
    pub fn is_connectable(&self) -> bool {
        self.is_connectable
    }

    /// Gets the list of valid connection types for this property.
    ///
    /// This value comes from shader metadata, and may not be specified. The
    /// value from `get_type()` can be used as a fallback, or you can use the
    /// connectability test in `can_connect_to()`.
    pub fn get_valid_connection_types(&self) -> &SdrTokenVec {
        &self.valid_connection_types
    }

    /// Determines if this property can be connected to the specified property.
    ///
    /// Matches C++ `SdrShaderProperty::CanConnectTo()` exactly:
    /// 1. Direction check: one input + one output required
    /// 2. Same type + same array size → true
    /// 3. Same type + output is scalar + input is dynamic array → true
    /// 4. Both float-3 family (color/point/normal/vector or Float3 SDF) → true
    /// 5. Both float-4 family (color4 or Float4 SDF) → true
    /// 6. vstruct output → float input → true
    ///
    /// Note: C++ `CanConnectTo` does NOT check `_isConnectable`; that flag is
    /// queried separately via `IsConnectable()`.
    pub fn can_connect_to(&self, other: &SdrShaderProperty) -> bool {
        // Cannot connect two outputs or two inputs
        if self.is_output == other.is_output {
            return false;
        }

        // Identify which side is input and which is output
        let (input, output) = if !self.is_output {
            (self, other)
        } else {
            (other, self)
        };

        let input_type = &input.property_type;
        let output_type = &output.property_type;

        // Rule 1: exact type + same array size
        if input_type == output_type && input.array_size == output.array_size {
            return true;
        }

        // Rule 2: same type, output is scalar, input is dynamic array
        if input_type == output_type && !output.is_array() && input.is_dynamic_array {
            return true;
        }

        let types = &tokens().property_types;

        // Convert input/output types to SDF types using the full type conversion
        // (C++ uses _GetTypeAsSdfType with metadata, not a simplified helper)
        let registry = usd_sdf::value_type_registry::ValueTypeRegistry::instance();
        let sdf_input_type = input.get_type_as_sdf_type();
        let sdf_output_type = output.get_type_as_sdf_type();
        let sdf_float3 = registry.find_type("float3");
        let sdf_float4 = registry.find_type("float4");

        let input_is_float3 = input_type == &types.color
            || input_type == &types.point
            || input_type == &types.normal
            || input_type == &types.vector
            || *sdf_input_type.get_sdf_type() == sdf_float3;

        let output_is_float3 = output_type == &types.color
            || output_type == &types.point
            || output_type == &types.normal
            || output_type == &types.vector
            || *sdf_output_type.get_sdf_type() == sdf_float3;

        // Rule 3: float-3 family <-> float-3 family
        if input_is_float3 && output_is_float3 {
            return true;
        }

        let input_is_float4 =
            input_type == &types.color4 || *sdf_input_type.get_sdf_type() == sdf_float4;
        let output_is_float4 =
            output_type == &types.color4 || *sdf_output_type.get_sdf_type() == sdf_float4;

        // Rule 4: float-4 family <-> float-4 family
        if input_is_float4 && output_is_float4 {
            return true;
        }

        // Rule 5: vstruct output -> float input (vstruct is output-only type)
        if output_type == &types.vstruct && input_type == &types.float {
            return true;
        }

        false
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /// Converts the property's type into a SdrSdfTypeIndicator.
    ///
    /// Dispatches to encoding v0 or v1 based on `usd_encoding_version`.
    /// Encoding v1 (current default) handles sdrUsdDefinitionType metadata,
    /// Asset types, Terminal/Struct/Vstruct → Token, and fixed-size int/float
    /// arrays (Int2/3/4, Float2/3/4).
    pub fn get_type_as_sdf_type(&self) -> SdrSdfTypeIndicator {
        match self.usd_encoding_version {
            0 => self.get_type_as_sdf_type_v0(),
            1 => self.get_type_as_sdf_type_v1(),
            _ => {
                // Invalid encoding version — fallback to Token (no mapping)
                let registry = usd_sdf::value_type_registry::ValueTypeRegistry::instance();
                SdrSdfTypeIndicator::with_types(
                    registry.find_type("token"),
                    self.property_type.clone(),
                    false,
                )
            }
        }
    }

    /// Encoding v0: original Pixar-internal mapping.
    ///
    /// Asset → String, Struct → String, Vstruct → Float, Terminal → Token (no mapping).
    fn get_type_as_sdf_type_v0(&self) -> SdrSdfTypeIndicator {
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let prop_type = &self.property_type;
        let types = &tokens().property_types;
        let registry = ValueTypeRegistry::instance();
        let is_array = self.is_array();

        // Asset identifier → String/StringArray (v0 behavior)
        if self.is_asset_identifier() {
            let sdf = if is_array {
                registry.find_type("string[]")
            } else {
                registry.find_type("string")
            };
            return SdrSdfTypeIndicator::with_types(sdf, prop_type.clone(), true);
        }

        // Terminal → Token (no clean mapping)
        if prop_type == &types.terminal {
            return SdrSdfTypeIndicator::with_types(
                registry.find_type("token"),
                prop_type.clone(),
                false,
            );
        }

        // Struct → String
        if prop_type == &types.struct_type {
            return SdrSdfTypeIndicator::with_types(
                registry.find_type("string"),
                prop_type.clone(),
                true,
            );
        }

        // Vstruct → Float/FloatArray
        if prop_type == &types.vstruct {
            let sdf = if is_array {
                registry.find_type("float[]")
            } else {
                registry.find_type("float")
            };
            return SdrSdfTypeIndicator::with_types(sdf, prop_type.clone(), true);
        }

        // Default mapping table
        Self::get_type_from_default_mapping(prop_type, is_array)
    }

    /// Encoding v1: current USD encoding mapping.
    ///
    /// Checks sdrUsdDefinitionType metadata first, then Asset → Asset,
    /// Terminal/Struct/Vstruct → Token, fixed-size Int/Float → Int2-4/Float2-4.
    fn get_type_as_sdf_type_v1(&self) -> SdrSdfTypeIndicator {
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let prop_type = &self.property_type;
        let types = &tokens().property_types;
        let registry = ValueTypeRegistry::instance();
        let is_array = self.is_array();

        // Check sdrUsdDefinitionType metadata first (explicit override)
        if self.metadata.has_sdr_usd_definition_type() {
            let def_type = self.metadata.get_sdr_usd_definition_type();
            if !def_type.is_empty() {
                let sdf = registry.find_type(def_type.as_str());
                return SdrSdfTypeIndicator::with_types(sdf, prop_type.clone(), true);
            }
        }

        // Asset identifier → Asset/AssetArray (v1 uses Asset, not String)
        if self.is_asset_identifier() {
            let sdf = if is_array {
                registry.find_type("asset[]")
            } else {
                registry.find_type("asset")
            };
            return SdrSdfTypeIndicator::with_types(sdf, prop_type.clone(), true);
        }

        // Terminal, Struct, Vstruct → Token/TokenArray
        if prop_type == &types.terminal
            || prop_type == &types.struct_type
            || prop_type == &types.vstruct
        {
            let sdf = if is_array {
                registry.find_type("token[]")
            } else {
                registry.find_type("token")
            };
            return SdrSdfTypeIndicator::with_types(sdf, prop_type.clone(), true);
        }

        // Fixed-size int arrays: Int + arraySize 2/3/4 → Int2/Int3/Int4
        if prop_type == &types.int {
            match self.array_size {
                2 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("int2"),
                        prop_type.clone(),
                        true,
                    );
                }
                3 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("int3"),
                        prop_type.clone(),
                        true,
                    );
                }
                4 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("int4"),
                        prop_type.clone(),
                        true,
                    );
                }
                _ => {}
            }
        }

        // Fixed-size float arrays: Float + arraySize 2/3/4 → Float2/Float3/Float4
        if prop_type == &types.float {
            match self.array_size {
                2 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("float2"),
                        prop_type.clone(),
                        true,
                    );
                }
                3 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("float3"),
                        prop_type.clone(),
                        true,
                    );
                }
                4 => {
                    return SdrSdfTypeIndicator::with_types(
                        registry.find_type("float4"),
                        prop_type.clone(),
                        true,
                    );
                }
                _ => {}
            }
        }

        // Default mapping table
        Self::get_type_from_default_mapping(prop_type, is_array)
    }

    /// Default token-to-SDF mapping table (shared between v0 and v1).
    ///
    /// Covers: Int, String, Float, Color, Color4, Point, Normal, Vector, Matrix.
    /// Falls back to Token (no clean mapping) for unknown types.
    fn get_type_from_default_mapping(prop_type: &Token, is_array: bool) -> SdrSdfTypeIndicator {
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let types = &tokens().property_types;
        let registry = ValueTypeRegistry::instance();

        // Scalar → Array suffix table
        let type_name = if prop_type == &types.int {
            if is_array { "int[]" } else { "int" }
        } else if prop_type == &types.string {
            if is_array { "string[]" } else { "string" }
        } else if prop_type == &types.float {
            if is_array { "float[]" } else { "float" }
        } else if prop_type == &types.color {
            if is_array { "color3f[]" } else { "color3f" }
        } else if prop_type == &types.color4 {
            if is_array { "color4f[]" } else { "color4f" }
        } else if prop_type == &types.point {
            if is_array { "point3f[]" } else { "point3f" }
        } else if prop_type == &types.normal {
            if is_array { "normal3f[]" } else { "normal3f" }
        } else if prop_type == &types.vector {
            if is_array { "vector3f[]" } else { "vector3f" }
        } else if prop_type == &types.matrix {
            if is_array { "matrix4d[]" } else { "matrix4d" }
        } else {
            // No clean mapping → Token
            return SdrSdfTypeIndicator::with_types(
                registry.find_type("token"),
                prop_type.clone(),
                false,
            );
        };

        SdrSdfTypeIndicator::with_types(registry.find_type(type_name), prop_type.clone(), true)
    }

    /// Accessor for default value corresponding to the SdfValueTypeName
    /// returned by get_type_as_sdf_type.
    ///
    /// Note that this is different than get_default_value which returns the
    /// default value associated with the SdrPropertyType and may differ from
    /// the SdfValueTypeName.
    pub fn get_default_value_as_sdf_type(&self) -> &Value {
        &self.sdf_type_default_value
    }

    /// Determines if the value held by this property is an asset identifier
    /// (e.g., a file path).
    pub fn is_asset_identifier(&self) -> bool {
        self.metadata.get_is_asset_identifier()
    }

    /// Determines if the value held by this property is the default input
    /// for this node.
    pub fn is_default_input(&self) -> bool {
        self.metadata.get_default_input()
    }

    // ========================================================================
    // Internal methods (for post-processing)
    // ========================================================================

    /// Sets the USD encoding version for type conversion behavior.
    pub(crate) fn set_usd_encoding_version(&mut self, version: i32) {
        self.usd_encoding_version = version;
    }

    /// Sets the shownIf expression on this property's metadata.
    pub(crate) fn set_shown_if(&mut self, expr: &str) {
        self.metadata.set_shown_if(expr);
    }

    /// Converts this property to a VStruct type.
    ///
    /// Called during post-processing for properties identified as vstruct heads.
    /// Matches C++ `_ConvertToVStruct()`: sets the default value to the SDF
    /// type's default (via GetTypeAsSdfType), not Value::default().
    pub fn convert_to_vstruct(&mut self) {
        self.property_type = tokens().property_types.vstruct.clone();

        // C++: _defaultValue = typeName.GetDefaultValue()
        let indicator = self.get_type_as_sdf_type();
        let sdf_type = indicator.get_sdf_type();
        self.default_value = sdf_type.default_value().cloned().unwrap_or_default();
    }

    /// Finalizes the property after all information is available.
    ///
    /// Conforms the default value to match the SDF type that this property
    /// maps to. Also validates the default value against the SDR type.
    ///
    /// Matches C++ `SdrShaderProperty::_FinalizeProperty()`.
    pub(crate) fn finalize(&mut self) {
        // Conform default value to the SDF type
        self.sdf_type_default_value = self.conform_sdf_type_default_value();

        // Conform and validate the SDR default value
        self.default_value = self.conform_sdr_default_value();
    }

    /// Conforms the default value to the SDF value type.
    ///
    /// Handles asset identifiers (String -> SdfAssetPath), fixed-size
    /// float/int arrays (VtFloatArray -> GfVec2f/3f/4f), and falls back
    /// to the SDF type's own default when no conversion is found.
    ///
    /// Matches C++ `_ConformSdfTypeDefaultValue()`.
    fn conform_sdf_type_default_value(&self) -> Value {
        if self.default_value.is_empty() {
            return self.default_value.clone();
        }

        // Get the SDF type indicator for this property
        let indicator = self.get_type_as_sdf_type();
        let sdf_type = indicator.get_sdf_type();

        // If types already match, no conformance needed
        if let Some(sdf_default) = sdf_type.default_value() {
            if sdf_default.type_name() == self.default_value.type_name() {
                return self.default_value.clone();
            }
        }

        // Special conformance when SdrUsdDefinitionType is provided (C++ lines 522-530):
        // The shader writer provides an explicit SdfValueTypeName, so we trust the
        // default value as-is (assuming it matches that type).
        if self.metadata.has_sdr_usd_definition_type() {
            // In C++ this tries VtValue::CastToTypeid; in Rust we return the
            // original value since our Value doesn't have runtime type casting.
            // The value was already parsed to match the definition type.
            return self.default_value.clone();
        }

        let types = &tokens().property_types;
        let is_array = self.is_array();

        // Asset identifier: String -> AssetPath
        if self.property_type == types.string && self.is_asset_identifier() {
            if is_array {
                if let Some(arr) = self.default_value.as_vec_clone::<String>() {
                    let assets: Vec<usd_sdf::AssetPath> =
                        arr.iter().map(|s| usd_sdf::AssetPath::new(s)).collect();
                    return Value::from_no_hash(assets);
                }
            } else if let Some(s) = self.default_value.downcast_clone::<String>() {
                return Value::from_no_hash(usd_sdf::AssetPath::new(&s));
            }
        }

        // Fixed-size float array -> GfVec
        if self.property_type == types.float && is_array {
            if let Some(arr) = self.default_value.as_vec_clone::<f32>() {
                if arr.len() == self.array_size as usize {
                    match self.array_size {
                        2 => return Value::from(usd_gf::Vec2f::new(arr[0], arr[1])),
                        3 => return Value::from(usd_gf::Vec3f::new(arr[0], arr[1], arr[2])),
                        4 => {
                            return Value::from(usd_gf::Vec4f::new(arr[0], arr[1], arr[2], arr[3]));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Fixed-size int array -> GfVec
        if self.property_type == types.int && is_array {
            if let Some(arr) = self.default_value.as_vec_clone::<i32>() {
                if arr.len() == self.array_size as usize {
                    match self.array_size {
                        2 => return Value::from(usd_gf::Vec2i::new(arr[0], arr[1])),
                        3 => return Value::from(usd_gf::Vec3i::new(arr[0], arr[1], arr[2])),
                        4 => {
                            return Value::from(usd_gf::Vec4i::new(arr[0], arr[1], arr[2], arr[3]));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Fallback: use the SDF type's own default value
        sdf_type.default_value().cloned().unwrap_or_default()
    }

    /// Validates the SDR default value matches the property's SDR type.
    ///
    /// For float arrays, also conforms VtFloatArray -> GfVec2f/3f/4f.
    ///
    /// Matches C++ `_ConformSdrDefaultValue()`.
    fn conform_sdr_default_value(&self) -> Value {
        if self.default_value.is_empty() {
            return self.default_value.clone();
        }

        let types = &tokens().property_types;
        let is_array = self.is_array();

        // Float array -> conform to GfVec if fixed size
        if self.property_type == types.float && is_array {
            if let Some(arr) = self.default_value.as_vec_clone::<f32>() {
                if arr.len() == self.array_size as usize {
                    match self.array_size {
                        2 => return Value::from(usd_gf::Vec2f::new(arr[0], arr[1])),
                        3 => return Value::from(usd_gf::Vec3f::new(arr[0], arr[1], arr[2])),
                        4 => {
                            return Value::from(usd_gf::Vec4f::new(arr[0], arr[1], arr[2], arr[3]));
                        }
                        _ => {}
                    }
                }
            }
        }

        // For all other types, return as-is (type mismatch is just logged in C++)
        self.default_value.clone()
    }
}

impl Default for SdrShaderProperty {
    fn default() -> Self {
        Self {
            name: Token::default(),
            property_type: tokens().property_types.unknown.clone(),
            default_value: Value::default(),
            is_output: false,
            array_size: 0,
            tuple_size: 0,
            is_dynamic_array: false,
            is_connectable: true,
            legacy_metadata: SdrTokenMap::new(),
            metadata: SdrShaderPropertyMetadata::new(),
            hints: SdrTokenMap::new(),
            options: SdrOptionVec::new(),
            valid_connection_types: SdrTokenVec::new(),
            label: Token::default(),
            page: Token::default(),
            widget: Token::default(),
            vstruct_member_of: Token::default(),
            vstruct_member_name: Token::default(),
            vstruct_conditional_expr: Token::default(),
            sdf_type_default_value: Value::default(),
            // Matches C++ _UsdEncodingVersionsCurrent = _UsdEncodingVersions1 = 1
            usd_encoding_version: 1,
        }
    }
}

/// Unique pointer to a shader property.
pub type SdrShaderPropertyUniquePtr = Box<SdrShaderProperty>;

/// Vector of unique shader property pointers.
pub type SdrShaderPropertyUniquePtrVec = Vec<SdrShaderPropertyUniquePtr>;

/// Map from token to property reference.
pub type SdrShaderPropertyMap<'a> = std::collections::HashMap<Token, &'a SdrShaderProperty>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_property() {
        let prop = SdrShaderProperty::new(
            Token::new("diffuseColor"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        assert_eq!(prop.get_name().as_str(), "diffuseColor");
        assert_eq!(prop.get_type(), &tokens().property_types.color);
        assert!(!prop.is_output());
        assert!(!prop.is_array());
        assert!(prop.is_connectable());
    }

    #[test]
    fn test_output_property() {
        let prop = SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        assert!(prop.is_output());
    }

    #[test]
    fn test_array_property() {
        let prop = SdrShaderProperty::new(
            Token::new("weights"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            4,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        assert!(prop.is_array());
        assert_eq!(prop.get_array_size(), 4);
        assert!(!prop.is_dynamic_array());
    }

    #[test]
    fn test_can_connect_to() {
        let input = SdrShaderProperty::new(
            Token::new("in"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        let output = SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        assert!(input.can_connect_to(&output));
        assert!(output.can_connect_to(&input));
    }

    #[test]
    fn test_cannot_connect_same_direction() {
        let input1 = SdrShaderProperty::new(
            Token::new("in1"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        let input2 = SdrShaderProperty::new(
            Token::new("in2"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        assert!(!input1.can_connect_to(&input2));
    }

    #[test]
    fn test_type_as_sdf_type() {
        let float_prop = SdrShaderProperty::new(
            Token::new("value"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );

        let indicator = float_prop.get_type_as_sdf_type();
        assert!(indicator.has_sdf_type());
    }

    // =========================================================
    // SdrVersion get_string / get_string_suffix
    // =========================================================

    #[test]
    fn test_version_get_string_format() {
        use super::super::declare::SdrVersion;
        let v = SdrVersion::new(3, 5);
        // C++ format: "N.M" for non-zero minor
        assert_eq!(v.get_string(), "3.5");
        // C++ suffix format: "_N.M" (dot separator, NOT underscore)
        assert_eq!(v.get_string_suffix(), "_3.5");
    }

    #[test]
    fn test_version_get_string_zero_minor() {
        use super::super::declare::SdrVersion;
        let v = SdrVersion::new(2, 0);
        // C++ format: "N" (no .0 suffix) when minor is 0
        assert_eq!(v.get_string(), "2");
        // Suffix for minor==0: "_N"
        assert_eq!(v.get_string_suffix(), "_2");
    }

    #[test]
    fn test_version_invalid_string() {
        use super::super::declare::SdrVersion;
        let v = SdrVersion::invalid();
        // C++: invalid version returns "<invalid version>"
        assert_eq!(v.get_string(), "<invalid version>");
        // Default/invalid versions return empty suffix
        assert_eq!(v.get_string_suffix(), "");
    }

    #[test]
    fn test_version_default_suffix_empty() {
        use super::super::declare::SdrVersion;
        // A default-marked version returns empty suffix
        let v = SdrVersion::new(1, 0).as_default();
        assert_eq!(v.get_string_suffix(), "");
        // But get_string still works
        assert_eq!(v.get_string(), "1");
    }

    // =========================================================
    // is_connectable: outputs always connectable
    // =========================================================

    #[test]
    fn test_outputs_always_connectable() {
        // Output with no explicit connectable metadata must still be connectable.
        let output = SdrShaderProperty::new(
            Token::new("result"),
            tokens().property_types.color.clone(),
            Value::default(),
            true, // is_output
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(output.is_connectable(), "output must always be connectable");
    }

    // =========================================================
    // can_connect_to: cross-type connection rules
    // =========================================================

    /// int -> float must NOT connect (C++ rule: no int<->float).
    #[test]
    fn test_no_int_float_connection() {
        let int_in = SdrShaderProperty::new(
            Token::new("i"),
            tokens().property_types.int.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let float_out = SdrShaderProperty::new(
            Token::new("f"),
            tokens().property_types.float.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        // C++ forbids int<->float cross-connection
        assert!(
            !int_in.can_connect_to(&float_out),
            "int<->float must not connect"
        );
        assert!(
            !float_out.can_connect_to(&int_in),
            "float<->int must not connect"
        );
    }

    /// color -> normal -> vector are inter-connectable (Float3 family).
    #[test]
    fn test_float3_family_connectable() {
        let color_in = SdrShaderProperty::new(
            Token::new("ci"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let normal_out = SdrShaderProperty::new(
            Token::new("n"),
            tokens().property_types.normal.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let vector_out = SdrShaderProperty::new(
            Token::new("v"),
            tokens().property_types.vector.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            color_in.can_connect_to(&normal_out),
            "color<->normal must connect"
        );
        assert!(
            color_in.can_connect_to(&vector_out),
            "color<->vector must connect"
        );
        assert!(
            normal_out.can_connect_to(&color_in),
            "normal<->color must connect"
        );
    }

    /// Mismatched types outside Float3/Float4 families must not connect.
    #[test]
    fn test_incompatible_types_no_connection() {
        let string_in = SdrShaderProperty::new(
            Token::new("s"),
            tokens().property_types.string.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let color_out = SdrShaderProperty::new(
            Token::new("c"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(!string_in.can_connect_to(&color_out));
    }

    // =========================================================
    // get_type_as_sdf_type: all standard mappings
    // =========================================================

    #[test]
    fn test_sdf_type_all_mappings() {
        let cases = [
            (tokens().property_types.int.clone(), "int"),
            (tokens().property_types.float.clone(), "float"),
            (tokens().property_types.color.clone(), "color3f"),
            (tokens().property_types.color4.clone(), "color4f"),
            (tokens().property_types.point.clone(), "point3f"),
            (tokens().property_types.normal.clone(), "normal3f"),
            (tokens().property_types.vector.clone(), "vector3f"),
            (tokens().property_types.matrix.clone(), "matrix4d"),
            (tokens().property_types.string.clone(), "string"),
        ];

        for (sdr_type, expected_sdf_name) in &cases {
            let prop = SdrShaderProperty::new(
                Token::new("p"),
                sdr_type.clone(),
                Value::default(),
                false,
                0,
                SdrShaderPropertyMetadata::new(),
                SdrTokenMap::new(),
                SdrOptionVec::new(),
            );
            let indicator = prop.get_type_as_sdf_type();
            assert!(
                indicator.has_sdf_type(),
                "type '{}' should have SDF mapping",
                sdr_type.as_str()
            );
            let sdf_type_name = indicator.get_sdf_type().name();
            assert_eq!(
                sdf_type_name,
                *expected_sdf_name,
                "SDF mapping for '{}' wrong",
                sdr_type.as_str()
            );
        }
    }

    /// Unknown SDR type maps to Token with has_sdf_type = false.
    #[test]
    fn test_sdf_type_unknown_type() {
        let prop = SdrShaderProperty::new(
            Token::new("p"),
            Token::new("mytexture"), // unknown type
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let indicator = prop.get_type_as_sdf_type();
        assert!(
            !indicator.has_sdf_type(),
            "unknown type should not have SDF mapping"
        );
    }

    // =========================================================
    // is_vstruct_member: edge cases
    // =========================================================

    #[test]
    fn test_is_vstruct_member_empty() {
        // No vstruct_member_of set -> not a vstruct member
        let prop = SdrShaderProperty::new(
            Token::new("p"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(!prop.is_vstruct_member());
    }

    // =========================================================
    // Edge case: empty version, version 0.0
    // =========================================================

    /// Version from empty string → invalid.
    #[test]
    fn test_version_empty_string() {
        use super::super::declare::SdrVersion;
        let v = SdrVersion::from_string("");
        assert!(!v.is_valid(), "empty string should produce invalid version");
        assert_eq!(v.get_string(), "<invalid version>");
        assert_eq!(v.get_string_suffix(), "");
    }

    /// Version 0.0 → invalid (both zero).
    #[test]
    fn test_version_zero_zero() {
        use super::super::declare::SdrVersion;
        let v = SdrVersion::new(0, 0);
        assert!(!v.is_valid(), "0.0 must be invalid");
        assert_eq!(v.get_string(), "<invalid version>");
        assert_eq!(v.get_string_suffix(), "");
        // Marking invalid as default still produces empty suffix
        assert_eq!(v.as_default().get_string_suffix(), "");
    }

    // =========================================================
    // Edge case: Color4 → Float4 connections
    // =========================================================

    /// color4 output → color4 input must connect.
    #[test]
    fn test_color4_to_color4() {
        let c4_in = SdrShaderProperty::new(
            Token::new("c4_in"),
            tokens().property_types.color4.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let c4_out = SdrShaderProperty::new(
            Token::new("c4_out"),
            tokens().property_types.color4.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            c4_in.can_connect_to(&c4_out),
            "color4<->color4 must connect"
        );
        assert!(
            c4_out.can_connect_to(&c4_in),
            "color4<->color4 (reversed) must connect"
        );
    }

    /// color4 and float[4] → both map to Float4 SDF family → must connect.
    #[test]
    fn test_color4_to_float4_array() {
        // float[4] SDF maps to Float4 in encoding v1
        let float4_in = SdrShaderProperty::new(
            Token::new("f4_in"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            4, // array_size=4 → Float4
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let c4_out = SdrShaderProperty::new(
            Token::new("c4_out"),
            tokens().property_types.color4.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            float4_in.can_connect_to(&c4_out),
            "float[4] input and color4 output should connect (both Float4 SDF family)"
        );
    }

    /// color4 must NOT connect to color (Float3 family).
    #[test]
    fn test_color4_not_color() {
        let c4_in = SdrShaderProperty::new(
            Token::new("c4_in"),
            tokens().property_types.color4.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let color_out = SdrShaderProperty::new(
            Token::new("c_out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            !c4_in.can_connect_to(&color_out),
            "color4 (Float4) must NOT connect to color (Float3)"
        );
    }

    // =========================================================
    // Edge case: vstruct cross-type connections
    // =========================================================

    /// vstruct output → float input must connect (C++ rule 5).
    #[test]
    fn test_vstruct_output_to_float_input() {
        let float_in = SdrShaderProperty::new(
            Token::new("f_in"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        // Create a vstruct output property via convert_to_vstruct
        let mut vs_out = SdrShaderProperty::new(
            Token::new("vs_out"),
            tokens().property_types.float.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        vs_out.convert_to_vstruct();
        assert_eq!(vs_out.get_type(), &tokens().property_types.vstruct);

        assert!(
            float_in.can_connect_to(&vs_out),
            "float input must accept vstruct output"
        );
        assert!(
            vs_out.can_connect_to(&float_in),
            "vstruct output must connect to float input"
        );
    }

    /// vstruct output → color input must NOT connect.
    #[test]
    fn test_vstruct_output_not_color_input() {
        let color_in = SdrShaderProperty::new(
            Token::new("c_in"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        let mut vs_out = SdrShaderProperty::new(
            Token::new("vs_out"),
            tokens().property_types.float.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        vs_out.convert_to_vstruct();

        assert!(
            !color_in.can_connect_to(&vs_out),
            "vstruct output must NOT connect to non-float input"
        );
    }

    // =========================================================
    // P0: _ConvertSdrPropertyTypeAndArraySize via role="none"
    // =========================================================

    /// Color property with role="none" must be converted to Float[3].
    #[test]
    fn test_role_none_color_to_float3() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_role("none");

        let prop = SdrShaderProperty::new(
            Token::new("c"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            meta,
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        // After role-based conversion: Color(role=none) → Float[3]
        assert_eq!(
            prop.get_type(),
            &tokens().property_types.float,
            "type must become float"
        );
        assert_eq!(prop.get_array_size(), 3, "array size must become 3");
    }

    /// Color4 property with role="none" must be converted to Float[4].
    #[test]
    fn test_role_none_color4_to_float4() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_role("none");

        let prop = SdrShaderProperty::new(
            Token::new("c4"),
            tokens().property_types.color4.clone(),
            Value::default(),
            false,
            0,
            meta,
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert_eq!(prop.get_type(), &tokens().property_types.float);
        assert_eq!(prop.get_array_size(), 4);
    }

    /// Color property with no role must NOT be converted (stays Color).
    #[test]
    fn test_no_role_color_unchanged() {
        let prop = SdrShaderProperty::new(
            Token::new("c"),
            tokens().property_types.color.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert_eq!(
            prop.get_type(),
            &tokens().property_types.color,
            "type must stay color"
        );
        assert_eq!(prop.get_array_size(), 0);
    }

    // =========================================================
    // P0: outputs always connectable (metadata override ignored)
    // =========================================================

    /// Output with explicit connectable=false metadata must still be connectable.
    #[test]
    fn test_output_ignores_connectable_false_metadata() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_connectable(false); // Should be ignored for outputs

        let output = SdrShaderProperty::new(
            Token::new("out"),
            tokens().property_types.color.clone(),
            Value::default(),
            true, // is_output
            0,
            meta,
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            output.is_connectable(),
            "output must always be connectable even with connectable=false metadata"
        );
    }

    /// Input with explicit connectable=false metadata must NOT be connectable.
    #[test]
    fn test_input_respects_connectable_false_metadata() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_connectable(false);

        let input = SdrShaderProperty::new(
            Token::new("in"),
            tokens().property_types.color.clone(),
            Value::default(),
            false, // is_input
            0,
            meta,
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert!(
            !input.is_connectable(),
            "input with connectable=false must not be connectable"
        );
    }

    // =========================================================
    // P1: widget defaults to "default" when not specified
    // =========================================================

    /// Widget should default to "default" when not specified in metadata.
    #[test]
    fn test_widget_default_when_not_specified() {
        let prop = SdrShaderProperty::new(
            Token::new("p"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(), // no widget set
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert_eq!(
            prop.get_widget().as_str(),
            "default",
            "widget must default to 'default' when not set"
        );
    }

    /// Widget should keep explicitly set value.
    #[test]
    fn test_widget_explicit_value_preserved() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_widget(&Token::new("slider"));

        let prop = SdrShaderProperty::new(
            Token::new("p"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            meta,
            SdrTokenMap::new(),
            SdrOptionVec::new(),
        );
        assert_eq!(prop.get_widget().as_str(), "slider");
    }
}
