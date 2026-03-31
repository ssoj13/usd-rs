//! Syntax — base type syntax handling for shader generators.

use std::collections::{HashMap, HashSet};

use crate::core::Value;
use crate::core::util::split_string;
use crate::gen_shader::{TypeDesc, type_desc_types};

/// Identifier map: base name -> count for unique naming
pub type IdentifierMap = HashMap<String, usize>;

/// Mode for remapping string enumerations to target-language types (e.g. GLSL: string→int).
#[derive(Clone, Copy, Default)]
pub enum EnumRemapMode {
    /// No remapping; use original type/value.
    #[default]
    None,
    /// Remap string enum to integer (index into enum list). Used by GLSL, MSL, Slang.
    StringToInteger,
}

/// Base syntax for a shader language
pub struct Syntax {
    pub type_system: crate::gen_shader::TypeSystem,
    type_syntax: HashMap<String, TypeSyntax>,
    reserved_words: HashSet<String>,
    invalid_tokens: HashMap<String, String>,
    pub enum_remap_mode: EnumRemapMode,
}

impl Syntax {
    pub fn new(type_system: crate::gen_shader::TypeSystem) -> Self {
        Self {
            type_system,
            type_syntax: HashMap::new(),
            reserved_words: HashSet::new(),
            invalid_tokens: HashMap::new(),
            enum_remap_mode: EnumRemapMode::None,
        }
    }

    pub fn register_type_syntax(&mut self, type_desc: TypeDesc, syntax: TypeSyntax) {
        let name = type_desc.get_name().to_string();
        self.reserved_words.insert(syntax.name.clone());
        self.type_syntax.insert(name, syntax);
    }

    pub fn register_reserved_words(&mut self, names: impl IntoIterator<Item = String>) {
        self.reserved_words.extend(names);
    }

    pub fn register_invalid_tokens(&mut self, tokens: impl IntoIterator<Item = (String, String)>) {
        self.invalid_tokens.extend(tokens);
    }

    pub fn get_type_syntax(&self, type_desc: &TypeDesc) -> Option<&TypeSyntax> {
        self.type_syntax.get(type_desc.get_name())
    }

    /// Returns true if the given type has a registered TypeSyntax.
    /// C++ ref: Syntax::typeSupported (deprecated variant checked type registry).
    pub fn type_supported(&self, type_desc: &TypeDesc) -> bool {
        self.type_syntax.contains_key(type_desc.get_name())
    }

    /// Iterate over all type syntaxes (for emitTypeDefinitions).
    pub fn iter_type_syntax(&self) -> impl Iterator<Item = (&String, &TypeSyntax)> {
        self.type_syntax.iter()
    }

    /// Mutable access to the type syntax map (for language-specific overrides like GlslStructTypeSyntax).
    pub fn type_syntax_mut(&mut self) -> &mut HashMap<String, TypeSyntax> {
        &mut self.type_syntax
    }

    pub fn get_type(&self, name: &str) -> TypeDesc {
        self.type_system.get_type(name)
    }

    pub fn get_type_name(&self, type_desc: &TypeDesc) -> Option<&str> {
        self.type_syntax
            .get(type_desc.get_name())
            .map(|s| s.name.as_str())
    }

    pub fn get_default_value(&self, type_desc: &TypeDesc, uniform: bool) -> String {
        self.type_syntax
            .get(type_desc.get_name())
            .map(|s| {
                if uniform {
                    &s.uniform_default_value
                } else {
                    &s.default_value
                }
            })
            .cloned()
            .unwrap_or_else(|| "0".to_string())
    }

    pub fn get_value(&self, type_desc: &TypeDesc, value: &Value, uniform: bool) -> String {
        let syntax = match self.type_syntax.get(type_desc.get_name()) {
            Some(s) => s,
            None => return value.get_value_string(),
        };
        syntax.get_value_uniform(value, uniform)
    }

    /// Like get_value but for OSL network param format: vector/color values space-separated.
    /// По рефу OslNetworkSyntax OslNetworkVectorTypeSyntax::getValue.
    pub fn get_value_network(&self, type_desc: &TypeDesc, value: &Value, uniform: bool) -> String {
        let base = self.get_value(type_desc, value, uniform);
        let type_name = type_desc.get_name();
        if matches!(
            type_name,
            "vector2" | "vector3" | "vector4" | "color3" | "color4"
        ) {
            base.replace(',', " ")
        } else {
            base
        }
    }

    pub fn get_reserved_words(&self) -> &HashSet<String> {
        &self.reserved_words
    }

    /// Make name valid for shader code (matches C++ Syntax::makeValidName).
    /// Replaces invalid chars with _, applies invalid_tokens map, appends "1" if reserved.
    pub fn make_valid_name(&self, name: &mut String) {
        // Replace non-alnum and non-underscore with _
        let mut s: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        for (token, replacement) in &self.invalid_tokens {
            if !token.is_empty() {
                s = s.replace(token, replacement);
            }
        }
        *name = s;
        if self.reserved_words.contains(name) {
            name.push('1');
        }
    }

    /// Functional variant of make_valid_name: takes &str, returns owned valid name.
    pub fn valid_name(&self, name: &str) -> String {
        let mut s = name.to_string();
        self.make_valid_name(&mut s);
        s
    }

    /// Make unique identifier: append counter if collision, loop until unique.
    /// Matches C++ Syntax::makeIdentifier with do-while collision avoidance.
    pub fn make_identifier(&self, name: &mut String, identifiers: &mut IdentifierMap) {
        self.make_valid_name(name);
        let base = name.clone();
        if let Some(&current) = identifiers.get(&base) {
            // Name collision: keep incrementing until we find a unique name.
            let mut count = current;
            let mut name2;
            loop {
                name2 = format!("{}{}", base, count);
                count += 1;
                if !identifiers.contains_key(&name2) {
                    break;
                }
            }
            // Update the counter for the base name.
            identifiers.insert(base, count);
            *name = name2;
        }
        // Register the final name with count=1.
        identifiers.insert(name.clone(), 1);
    }

    pub fn get_variable_name(
        &self,
        name: &str,
        _type_desc: &TypeDesc,
        identifiers: &mut IdentifierMap,
    ) -> String {
        let mut n = name.to_string();
        self.make_identifier(&mut n, identifiers);
        n
    }

    /// Remap string enumeration to target type (e.g. GLSL: string→integer index).
    /// Returns Some((type_desc, value)) if remap succeeded, None otherwise.
    /// По рефу Syntax::remapEnumeration.
    pub fn remap_enumeration(
        &self,
        value: &str,
        type_desc: &TypeDesc,
        enum_names: &str,
    ) -> Option<(TypeDesc, Value)> {
        if enum_names.is_empty() {
            return None;
        }
        if type_desc.get_name() != "string" {
            return None;
        }
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        match self.enum_remap_mode {
            EnumRemapMode::None => None,
            EnumRemapMode::StringToInteger => {
                let enums_vec: Vec<String> = split_string(enum_names, ",");
                let pos = enums_vec.iter().position(|e| e.as_str() == value);
                match pos {
                    Some(idx) => Some((type_desc_types::integer(), Value::Integer(idx as i32))),
                    None => None,
                }
            }
        }
    }

    /// Return qualifier for input variable declarations (C++ getInputQualifier).
    /// Base returns empty string; language-specific syntax overrides (e.g. GLSL: "in").
    pub fn get_input_qualifier(&self) -> &str {
        ""
    }

    /// Return qualifier for output variable declarations (C++ getOutputQualifier).
    /// Base returns empty string; language-specific syntax overrides (e.g. GLSL: "out").
    pub fn get_output_qualifier(&self) -> &str {
        ""
    }

    /// Return type name for output context (C++ getOutputTypeName).
    /// Prepends output qualifier if non-empty (e.g. GLSL: "out float").
    pub fn get_output_type_name(&self, type_desc: &TypeDesc) -> String {
        let type_name = self
            .get_type_name(type_desc)
            .unwrap_or(type_desc.get_name());
        let qualifier = self.get_output_qualifier();
        if qualifier.is_empty() {
            type_name.to_string()
        } else {
            format!("{} {}", qualifier, type_name)
        }
    }

    /// Create and register a TypeSyntax for a struct type (C++ createStructSyntax).
    /// Returns a cloned TypeSyntax for inspection; the original is stored in the registry.
    pub fn create_struct_syntax(
        &mut self,
        struct_type_name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default_value: impl Into<String>,
        type_alias: impl Into<String>,
        type_definition: impl Into<String>,
    ) -> TypeSyntax {
        self.create_struct_syntax_with_format(
            struct_type_name,
            default_value,
            uniform_default_value,
            type_alias,
            type_definition,
            OslValueFormat::None,
        )
    }

    /// Create struct syntax with an OSL-specific value format override.
    /// Used by OslSyntax::createStructSyntax to return OslStructTypeSyntax.
    pub fn create_struct_syntax_with_format(
        &mut self,
        struct_type_name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default_value: impl Into<String>,
        type_alias: impl Into<String>,
        type_definition: impl Into<String>,
        osl_value_format: OslValueFormat,
    ) -> TypeSyntax {
        let ts = TypeSyntax {
            name: struct_type_name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default_value.into(),
            type_alias: type_alias.into(),
            type_definition: type_definition.into(),
            members: Vec::new(),
            kind: TypeSyntaxKind::Struct,
            osl_value_format,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        };
        // Register under the struct name as the key (structs use name-keyed lookup)
        self.reserved_words.insert(ts.name.clone());
        let key = ts.name.clone();
        self.type_syntax.insert(key, ts.clone());
        ts
    }

    /// Create struct syntax with Slang-specific value format (ref: SlangSyntax::createStructSyntax).
    pub fn create_struct_syntax_slang(
        &mut self,
        struct_type_name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default_value: impl Into<String>,
        type_alias: impl Into<String>,
        type_definition: impl Into<String>,
    ) -> TypeSyntax {
        let ts = TypeSyntax {
            name: struct_type_name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default_value.into(),
            type_alias: type_alias.into(),
            type_definition: type_definition.into(),
            members: Vec::new(),
            kind: TypeSyntaxKind::Struct,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::SlangStruct,
        };
        self.reserved_words.insert(ts.name.clone());
        let key = ts.name.clone();
        self.type_syntax.insert(key, ts.clone());
        ts
    }

    pub fn get_constant_qualifier(&self) -> &str {
        "const"
    }

    pub fn get_newline(&self) -> &str {
        "\n"
    }

    pub fn get_indentation(&self) -> &str {
        "    "
    }

    /// Return a type alias for the given data type (C++ getTypeAlias).
    pub fn get_type_alias(&self, type_desc: &TypeDesc) -> &str {
        self.type_syntax
            .get(type_desc.get_name())
            .map(|s| s.type_alias.as_str())
            .unwrap_or("")
    }

    /// Return a custom type definition for the given data type (C++ getTypeDefinition).
    pub fn get_type_definition(&self, type_desc: &TypeDesc) -> &str {
        self.type_syntax
            .get(type_desc.get_name())
            .map(|s| s.type_definition.as_str())
            .unwrap_or("")
    }

    /// Return the string quote character (C++ getStringQuote).
    pub fn get_string_quote(&self) -> &str {
        STRING_QUOTE
    }

    /// Return the include statement pattern (C++ getIncludeStatement).
    pub fn get_include_statement(&self) -> &str {
        INCLUDE_STATEMENT
    }

    /// Return the single line comment prefix (C++ getSingleLineComment).
    pub fn get_single_line_comment(&self) -> &str {
        SINGLE_LINE_COMMENT
    }

    /// Return the begin multi-line comment string (C++ getBeginMultiLineComment).
    pub fn get_begin_multi_line_comment(&self) -> &str {
        BEGIN_MULTI_LINE_COMMENT
    }

    /// Return the end multi-line comment string (C++ getEndMultiLineComment).
    pub fn get_end_multi_line_comment(&self) -> &str {
        END_MULTI_LINE_COMMENT
    }

    /// Return the file extension for source code files (C++ getSourceFileExtension).
    /// Base returns empty; override in language-specific syntax.
    pub fn get_source_file_extension(&self) -> &str {
        ""
    }

    /// Return the array suffix for declaring an array type (C++ getArrayTypeSuffix).
    pub fn get_array_type_suffix(&self, _type_desc: &TypeDesc, _value: &Value) -> String {
        String::new()
    }

    /// Return the array suffix for declaring an array variable (C++ getArrayVariableSuffix).
    /// C++ checks for vector<float> or vector<int>; Rust checks FloatArray/IntegerArray.
    pub fn get_array_variable_suffix(&self, type_desc: &TypeDesc, value: &Value) -> String {
        if !type_desc.is_array() {
            return String::new();
        }
        match value {
            Value::FloatArray(arr) => format!("[{}]", arr.len()),
            Value::IntegerArray(arr) => format!("[{}]", arr.len()),
            _ => {
                // Fallback: count comma-separated elements in value string
                let s = value.get_value_string();
                if s.is_empty() {
                    String::new()
                } else {
                    let count = s.split(',').count();
                    format!("[{}]", count)
                }
            }
        }
    }
}

// Static string constants matching C++ Syntax class
pub const SEMICOLON: &str = ";";
pub const COMMA: &str = ",";

/// Swizzle channel mapping: character -> component index.
/// Matches C++ Syntax::CHANNELS_MAPPING.
pub fn channels_mapping(ch: char) -> Option<usize> {
    match ch {
        'r' | 'x' => Some(0),
        'g' | 'y' => Some(1),
        'b' | 'z' => Some(2),
        'a' | 'w' => Some(3),
        _ => None,
    }
}
const STRING_QUOTE: &str = "\"";
const INCLUDE_STATEMENT: &str = "#include";
const SINGLE_LINE_COMMENT: &str = "// ";
const BEGIN_MULTI_LINE_COMMENT: &str = "/* ";
const END_MULTI_LINE_COMMENT: &str = " */";

/// Kind of TypeSyntax value formatting (matches C++ class hierarchy).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TypeSyntaxKind {
    /// Scalar: getValue returns raw value string (float, int, bool).
    #[default]
    Scalar,
    /// String: getValue wraps in double-quotes.
    StringLiteral,
    /// Aggregate: getValue wraps as `TypeName(values)` constructor call (vec2, mat4, etc.).
    Aggregate,
    /// Struct: getValue emits `{member0;member1;...}` brace initializer.
    Struct,
}

/// OSL-specific value formatting overrides (C++ anonymous inner classes in OslSyntax).
/// When set on a TypeSyntax, overrides the default get_value behavior.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum OslValueFormat {
    /// No override — use TypeSyntaxKind default behavior.
    #[default]
    None,
    /// OslBooleanTypeSyntax: "true"/"false" -> "1"/"0".
    Boolean,
    /// OslFloatArrayTypeSyntax: wraps in {}; errors on empty uniform.
    FloatArray,
    /// OslIntegerArrayTypeSyntax: wraps in {}; errors on empty uniform.
    IntegerArray,
    /// OslVecTypeSyntax: uniform -> "{vals}", non-uniform -> "type(vals)".
    OslVec,
    /// OslColor4TypeSyntax: uniform -> "{color(r,g,b), a}", non-uniform -> "color4(color(r,g,b), a)".
    OslColor4,
    /// OSLMatrix3TypeSyntax: expand 3x3 to 4x4 by inserting zero cols/rows.
    OslMatrix3,
    /// OSLFilenameTypeSyntax: textureresource("file","cs") or {"file","cs"} for uniform.
    OslFilename,
    /// OslStructTypeSyntax: recursive member formatting (Phase 3).
    OslStruct,
    /// OslNetworkVectorTypeSyntax: space-separated component values.
    NetworkVector,
    /// Network OSLFilenameTypeSyntax: returns just the filename string.
    NetworkFilename,
}

/// GLSL-specific value formatting overrides (C++ anonymous inner classes in GlslSyntax.cpp).
/// When set on a TypeSyntax, overrides the default get_value behavior for GLSL targets.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum GlslValueFormat {
    /// No override -- use TypeSyntaxKind default behavior.
    #[default]
    None,
    /// GlslFloatArrayTypeSyntax: produces `float[N](values)` array constructor.
    GlslFloatArray,
    /// GlslIntegerArrayTypeSyntax: produces `int[N](values)` array constructor.
    GlslIntegerArray,
    /// GlslStructTypeSyntax: recursive member formatting as `TypeName(member0,member1,...)`.
    GlslStruct,
}

/// Slang-specific value formatting overrides (C++ anonymous inner classes in SlangSyntax).
/// When set on a TypeSyntax, overrides the default get_value behavior for Slang targets.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SlangValueFormat {
    /// No override -- use TypeSyntaxKind default behavior.
    #[default]
    None,
    /// SlangStringTypeSyntax: always returns "0" (Slang has no string type).
    SlangString,
    /// SlangAggregateTypeSyntax: uniform -> raw value, non-uniform -> TypeName(value).
    SlangAggregate,
    /// SlangFloatArrayTypeSyntax: uniform -> raw value, non-uniform -> {value} if non-empty.
    SlangFloatArray,
    /// SlangIntegerArrayTypeSyntax: uniform -> raw value, non-uniform -> {value} if non-empty.
    SlangIntegerArray,
    /// SlangStructTypeSyntax: recursive struct value emission via AggregateValue members.
    SlangStruct,
}

/// Type-specific syntax (name, defaults, value formatting).
/// Corresponds to C++ TypeSyntax / ScalarTypeSyntax / AggregateTypeSyntax / StructTypeSyntax.
/// `Default` is derived so existing code using struct init can add `..Default::default()`.
#[derive(Clone, Debug, Default)]
pub struct TypeSyntax {
    pub name: String,
    pub default_value: String,
    pub uniform_default_value: String,
    pub type_alias: String,
    pub type_definition: String,
    /// Member type names for struct/aggregate types (optional).
    pub members: Vec<String>,
    /// How values are formatted when emitting literals.
    pub kind: TypeSyntaxKind,
    /// OSL-specific value formatting override (default: None = use kind-based formatting).
    pub osl_value_format: OslValueFormat,
    /// GLSL-specific value formatting override (default: None = use kind-based formatting).
    pub glsl_value_format: GlslValueFormat,
    /// Slang-specific value formatting override (default: None = use kind-based formatting).
    pub slang_value_format: SlangValueFormat,
}

impl TypeSyntax {
    /// Scalar type syntax: getValue returns raw value string (float, int, etc.).
    pub fn scalar(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: String::new(),
            type_definition: String::new(),
            members: Vec::new(),
            kind: TypeSyntaxKind::Scalar,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// Scalar with alias/definition strings.
    pub fn scalar_full(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
        type_alias: impl Into<String>,
        type_definition: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: type_alias.into(),
            type_definition: type_definition.into(),
            members: Vec::new(),
            kind: TypeSyntaxKind::Scalar,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// String literal type syntax: getValue wraps value in double-quotes.
    pub fn string_literal(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: String::new(),
            type_definition: String::new(),
            members: Vec::new(),
            kind: TypeSyntaxKind::StringLiteral,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// Aggregate type syntax: getValue returns `TypeName(value_string)` constructor.
    pub fn aggregate(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: String::new(),
            type_definition: String::new(),
            members: Vec::new(),
            kind: TypeSyntaxKind::Aggregate,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// Aggregate with alias/definition/members.
    pub fn aggregate_full(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
        type_alias: impl Into<String>,
        type_definition: impl Into<String>,
        members: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: type_alias.into(),
            type_definition: type_definition.into(),
            members,
            kind: TypeSyntaxKind::Aggregate,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// Struct type syntax: getValue returns `{member0;member1;...}` brace initializer.
    pub fn struct_type(
        name: impl Into<String>,
        default_value: impl Into<String>,
        uniform_default: impl Into<String>,
        type_definition: impl Into<String>,
        members: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            default_value: default_value.into(),
            uniform_default_value: uniform_default.into(),
            type_alias: String::new(),
            type_definition: type_definition.into(),
            members,
            kind: TypeSyntaxKind::Struct,
            osl_value_format: OslValueFormat::None,
            glsl_value_format: GlslValueFormat::None,
            slang_value_format: SlangValueFormat::None,
        }
    }

    /// Format a value according to this syntax kind.
    pub fn get_value(&self, value: &Value) -> String {
        self.get_value_uniform(value, false)
    }

    /// Format a value with uniform flag (OSL/Slang-specific overrides may depend on this).
    pub fn get_value_uniform(&self, value: &Value, uniform: bool) -> String {
        // Check Slang-specific override first
        match &self.slang_value_format {
            SlangValueFormat::None => {}
            fmt => return slang_format_value(fmt, &self.name, self, value, uniform),
        }
        // Check GLSL-specific override
        match &self.glsl_value_format {
            GlslValueFormat::None => {}
            fmt => return glsl_format_value(fmt, &self.name, value, uniform),
        }
        // Check OSL-specific override
        match &self.osl_value_format {
            OslValueFormat::None => {}
            fmt => return osl_format_value(fmt, &self.name, value, uniform),
        }
        // Default kind-based formatting
        match self.kind {
            TypeSyntaxKind::Scalar => value.get_value_string(),
            TypeSyntaxKind::StringLiteral => format!("\"{}\"", value.get_value_string()),
            TypeSyntaxKind::Aggregate => {
                let s = value.get_value_string();
                if s.is_empty() {
                    s
                } else {
                    format!("{}({})", self.name, s)
                }
            }
            TypeSyntaxKind::Struct => {
                let s = value.get_value_string();
                if s.is_empty() {
                    self.default_value.clone()
                } else {
                    format!("{{{}}}", s)
                }
            }
        }
    }
}

/// Apply GLSL-specific value formatting (matches C++ GlslArrayTypeSyntax, GlslStructTypeSyntax).
fn glsl_format_value(
    fmt: &GlslValueFormat,
    type_name: &str,
    value: &Value,
    _uniform: bool,
) -> String {
    match fmt {
        GlslValueFormat::None => unreachable!(),
        GlslValueFormat::GlslFloatArray => {
            // C++ GlslFloatArrayTypeSyntax::getValue: float[N](values)
            let size = match value {
                Value::FloatArray(arr) => arr.len(),
                _ => 0,
            };
            if size > 0 {
                format!("{}[{}]({})", type_name, size, value.get_value_string())
            } else {
                String::new()
            }
        }
        GlslValueFormat::GlslIntegerArray => {
            // C++ GlslIntegerArrayTypeSyntax::getValue: int[N](values)
            let size = match value {
                Value::IntegerArray(arr) => arr.len(),
                _ => 0,
            };
            if size > 0 {
                format!("{}[{}]({})", type_name, size, value.get_value_string())
            } else {
                String::new()
            }
        }
        GlslValueFormat::GlslStruct => {
            // C++ GlslStructTypeSyntax::getValue: TypeName(recursive_member_values)
            if let Value::Aggregate(agg) = value {
                let mut result = format!("{}(", agg.type_name);
                let mut sep = "";
                for member in &agg.members {
                    result.push_str(sep);
                    sep = ",";
                    // Recursively format the member value
                    result.push_str(&member.get_value_string());
                }
                result.push(')');
                result
            } else {
                value.get_value_string()
            }
        }
    }
}

/// Apply Slang-specific value formatting (matches C++ anonymous inner classes in SlangSyntax.cpp).
fn slang_format_value(
    fmt: &SlangValueFormat,
    type_name: &str,
    _ts: &TypeSyntax,
    value: &Value,
    uniform: bool,
) -> String {
    match fmt {
        SlangValueFormat::None => unreachable!(),
        SlangValueFormat::SlangString => {
            // C++ SlangStringTypeSyntax: always returns "0"
            "0".to_string()
        }
        SlangValueFormat::SlangAggregate => {
            // C++ SlangAggregateTypeSyntax: uniform -> raw value, non-uniform -> TypeName(value)
            let s = value.get_value_string();
            if uniform {
                s
            } else if s.is_empty() {
                s
            } else {
                format!("{}({})", type_name, s)
            }
        }
        SlangValueFormat::SlangFloatArray => {
            // C++ SlangFloatArrayTypeSyntax: uniform -> raw, non-uniform -> {value} if non-empty
            if uniform {
                return value.get_value_string();
            }
            let size = match value {
                Value::FloatArray(arr) => arr.len(),
                _ => 0,
            };
            if size > 0 {
                format!("{{{}}}", value.get_value_string())
            } else {
                String::new()
            }
        }
        SlangValueFormat::SlangIntegerArray => {
            // C++ SlangIntegerArrayTypeSyntax: uniform -> raw, non-uniform -> {value} if non-empty
            if uniform {
                return value.get_value_string();
            }
            let size = match value {
                Value::IntegerArray(arr) => arr.len(),
                _ => 0,
            };
            if size > 0 {
                format!("{{{}}}", value.get_value_string())
            } else {
                String::new()
            }
        }
        SlangValueFormat::SlangStruct => {
            // C++ SlangStructTypeSyntax: recursive struct value emission
            // Uses AggregateValue::getMembers() and recursively formats each member
            if let Value::Aggregate(agg) = value {
                let mut result = format!("{}(", agg.type_name);
                let mut sep = "";
                for member in &agg.members {
                    result.push_str(sep);
                    sep = ",";
                    // Recursively format the member value
                    result.push_str(&member.get_value_string());
                }
                result.push(')');
                result
            } else {
                value.get_value_string()
            }
        }
    }
}

/// Apply OSL-specific value formatting (matches C++ anonymous inner classes).
fn osl_format_value(fmt: &OslValueFormat, type_name: &str, value: &Value, uniform: bool) -> String {
    match fmt {
        OslValueFormat::None => unreachable!(),
        OslValueFormat::Boolean => {
            // C++: value.asA<bool>() ? "1" : "0"
            let s = value.get_value_string();
            if s == "true" || s == "1" {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        OslValueFormat::FloatArray | OslValueFormat::IntegerArray => {
            // C++: non-empty -> "{vals}"; empty+uniform -> error; empty -> ""
            let s = value.get_value_string();
            let is_empty = s.is_empty() || s.trim().is_empty();
            if !is_empty {
                format!("{{{}}}", s)
            } else if uniform {
                // C++ throws ExceptionShaderGenError
                "/* ERROR: Uniform array cannot initialize to empty value */".to_string()
            } else {
                String::new()
            }
        }
        OslValueFormat::OslVec => {
            // C++: uniform -> "{vals}", non-uniform -> "type(vals)"
            let s = value.get_value_string();
            if uniform {
                format!("{{{}}}", s)
            } else {
                format!("{}({})", type_name, s)
            }
        }
        OslValueFormat::OslColor4 => {
            // C++: parse 4 floats from Color4; uniform -> "{color(r,g,b), a}"
            let s = value.get_value_string();
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            if parts.len() >= 4 {
                if uniform {
                    format!(
                        "{{color({}, {}, {}), {}}}",
                        parts[0], parts[1], parts[2], parts[3]
                    )
                } else {
                    format!(
                        "color4(color({}, {}, {}), {})",
                        parts[0], parts[1], parts[2], parts[3]
                    )
                }
            } else {
                s
            }
        }
        OslValueFormat::OslMatrix3 => {
            // C++: expand 3x3 (9 values) to 4x4 by inserting zero col and identity last row
            let s = value.get_value_string();
            let values: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            if values.is_empty() {
                return "/* ERROR: No values given to construct a value */".to_string();
            }
            let mut result = format!("{}(", type_name);
            for (i, v) in values.iter().enumerate() {
                result.push_str(v);
                result.push_str(", ");
                if (i + 1) % 3 == 0 {
                    result.push_str("0.000, ");
                }
            }
            result.push_str("0.000, 0.000, 0.000, 1.000)");
            result
        }
        OslValueFormat::OslFilename => {
            // C++ getValue(Value&, uniform): prefix+"filename"+", \"\""+suffix
            let s = value.get_value_string();
            let prefix = if uniform {
                "{"
            } else {
                &format!("{}(", type_name.trim())
            };
            let suffix = if uniform { "}" } else { ")" };
            format!("{}\"{}\"{}\"\"{}", prefix, s, ", ", suffix)
        }
        OslValueFormat::OslStruct => {
            // Recursive struct formatting (simplified: raw value string)
            let s = value.get_value_string();
            if s.is_empty() {
                format!("{}()", type_name)
            } else {
                format!("{}({})", type_name, s)
            }
        }
        OslValueFormat::NetworkVector => {
            // C++ OslNetworkVectorTypeSyntax: space-separated component values
            let s = value.get_value_string();
            s.replace(',', " ").replace("  ", " ")
        }
        OslValueFormat::NetworkFilename => {
            // C++ network OSLFilenameTypeSyntax: just the filename string
            value.get_value_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::TypeSystem;

    fn make_syntax() -> Syntax {
        Syntax::new(TypeSystem::new())
    }

    #[test]
    fn qualifiers_default_empty() {
        let s = make_syntax();
        assert_eq!(s.get_input_qualifier(), "");
        assert_eq!(s.get_output_qualifier(), "");
    }

    #[test]
    fn get_output_type_name_unknown_falls_back_to_desc_name() {
        let s = make_syntax();
        // TypeDesc with no registered syntax: falls back to get_name()
        let td = TypeDesc::new(
            "myvec3",
            crate::gen_shader::BaseType::Float,
            crate::gen_shader::Semantic::None,
            3u16,
        );
        assert_eq!(s.get_output_type_name(&td), "myvec3");
    }

    #[test]
    fn get_output_type_name_registered() {
        let mut s = make_syntax();
        let td = TypeDesc::new(
            "float3",
            crate::gen_shader::BaseType::Float,
            crate::gen_shader::Semantic::None,
            3u16,
        );
        s.register_type_syntax(
            td.clone(),
            TypeSyntax::aggregate("float3", "float3(0)", "float3(0)"),
        );
        assert_eq!(s.get_output_type_name(&td), "float3");
    }

    #[test]
    fn create_struct_syntax_registered() {
        let mut s = make_syntax();
        let ts = s.create_struct_syntax(
            "MyStruct",
            "{0, 0}",
            "{0, 0}",
            "",
            "struct MyStruct { float a; float b; };",
        );
        assert_eq!(ts.name, "MyStruct");
        assert_eq!(ts.kind, TypeSyntaxKind::Struct);
        // Must be findable via a TypeDesc keyed by the struct name
        let td = TypeDesc::new(
            "MyStruct",
            crate::gen_shader::BaseType::Float,
            crate::gen_shader::Semantic::None,
            2u16,
        );
        let found = s.get_type_syntax(&td);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "MyStruct");
    }

    #[test]
    fn create_struct_syntax_reserved_word() {
        let mut s = make_syntax();
        s.create_struct_syntax("ClosureData", "{}", "{}", "", "struct ClosureData {};");
        assert!(s.get_reserved_words().contains("ClosureData"));
    }
}
