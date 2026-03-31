//! MdlSyntax -- MDL type syntax (ref: MaterialXGenMdl MdlSyntax).

use crate::core::Value;
use crate::gen_shader::{BaseType, Semantic, Syntax, TypeDesc, TypeSyntax, TypeSystem};

/// MDL source file extension
pub const SOURCE_FILE_EXTENSION: &str = ".mdl";
/// MDL uniform qualifier
#[allow(dead_code)]
pub const UNIFORM_QUALIFIER: &str = "uniform";
/// MDL const qualifier (empty in MDL)
#[allow(dead_code)]
pub const CONST_QUALIFIER: &str = "";
/// Port name prefix (ref: MdlSyntax::PORT_NAME_PREFIX = "mxp_")
pub const PORT_NAME_PREFIX: &str = "mxp_";
/// MDL version suffix marker for {{MDL_VERSION_SUFFIX}} replacement
pub const MARKER_MDL_VERSION_SUFFIX: &str = "MDL_VERSION_SUFFIX";

/// MDL enum member lists (ref: MdlSyntax.h / MdlSyntax.cpp)
pub const ADDRESSMODE_MEMBERS: &[&str] = &["constant", "clamp", "periodic", "mirror"];
pub const COORDINATESPACE_MEMBERS: &[&str] = &["model", "object", "world"];
pub const FILTERLOOKUPMODE_MEMBERS: &[&str] = &["closest", "linear", "cubic"];
pub const FILTERTYPE_MEMBERS: &[&str] = &["box", "gaussian"];
pub const DISTRIBUTIONTYPE_MEMBERS: &[&str] = &["ggx"];
pub const SCATTER_MODE_MEMBERS: &[&str] = &["R", "T", "RT"];
pub const SHEEN_MODE_MEMBERS: &[&str] = &["conty_kulla", "zeltner"];

/// Vector/color member accessors
#[allow(dead_code)]
pub const VECTOR2_MEMBERS: &[&str] = &[".x", ".y"];
#[allow(dead_code)]
pub const VECTOR3_MEMBERS: &[&str] = &[".x", ".y", ".z"];
#[allow(dead_code)]
pub const VECTOR4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];
#[allow(dead_code)]
pub const COLOR3_MEMBERS: &[&str] = &[".x", ".y", ".z"];
#[allow(dead_code)]
pub const COLOR4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".a"];

/// All enum type definitions with their members for getEnumeratedType lookup
const ENUM_TYPES: &[(&str, &str, &str, &[&str])] = &[
    (
        "MDL_ADDRESSMODE",
        "mx_addressmode_type",
        "mx_addressmode_type_periodic",
        ADDRESSMODE_MEMBERS,
    ),
    (
        "MDL_COORDINATESPACE",
        "mx_coordinatespace_type",
        "mx_coordinatespace_type_model",
        COORDINATESPACE_MEMBERS,
    ),
    (
        "MDL_FILTERLOOKUPMODE",
        "mx_filterlookup_type",
        "mx_filterlookup_type_linear",
        FILTERLOOKUPMODE_MEMBERS,
    ),
    (
        "MDL_FILTERTYPE",
        "mx_filter_type",
        "mx_filter_type_gaussian",
        FILTERTYPE_MEMBERS,
    ),
    (
        "MDL_DISTRIBUTIONTYPE",
        "mx_distribution_type",
        "mx_distribution_type_ggx",
        DISTRIBUTIONTYPE_MEMBERS,
    ),
    (
        "MDL_SCATTER_MODE",
        "mx_scatter_mode",
        "mx_scatter_mode_R",
        SCATTER_MODE_MEMBERS,
    ),
    (
        "MDL_SHEEN_MODE",
        "mx_sheen_mode",
        "mx_sheen_mode_conty_kulla",
        SHEEN_MODE_MEMBERS,
    ),
];

/// MDL syntax -- type names and defaults for Material Definition Language.
pub struct MdlSyntax {
    pub syntax: Syntax,
}

impl MdlSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_reserved_words(&mut syntax);
        Self::register_invalid_tokens(&mut syntax);
        // Register MDL-specific enum TypeDescs before syntax lookup
        Self::register_mdl_enum_type_descs(&mut syntax);
        Self::register_mdl_types(&mut syntax);
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    pub fn get_syntax(&self) -> &Syntax {
        &self.syntax
    }

    /// Pre-register MDL enum TypeDescs in the TypeSystem so get_type() can find them.
    fn register_mdl_enum_type_descs(syntax: &mut Syntax) {
        let enum_types = [
            "MDL_ADDRESSMODE",
            "MDL_COORDINATESPACE",
            "MDL_FILTERLOOKUPMODE",
            "MDL_FILTERTYPE",
            "MDL_DISTRIBUTIONTYPE",
            "MDL_SCATTER_MODE",
            "MDL_SHEEN_MODE",
        ];
        for name in enum_types {
            syntax.type_system.register_type_custom(
                name,
                BaseType::Integer,
                Semantic::Enum,
                1,
                None,
            );
        }
    }

    fn register_mdl_types(syntax: &mut Syntax) {
        // Scalar primitives
        let scalar_types = [
            ("float", TypeSyntax::scalar("float", "0.0", "0.0")),
            ("integer", TypeSyntax::scalar("int", "0", "0")),
            ("boolean", TypeSyntax::scalar("bool", "false", "false")),
            ("string", TypeSyntax::scalar("string", "\"\"", "\"\"")),
        ];
        for (name, ts) in scalar_types {
            let td = syntax.type_system.get_type(name);
            syntax.register_type_syntax(td, ts);
        }

        // Vector/matrix aggregates
        let agg: &[(&str, &str, &str, &str)] = &[
            ("vector2", "float2", "float2(0.0)", "float2(0.0)"),
            ("vector3", "float3", "float3(0.0)", "float3(0.0)"),
            ("vector4", "float4", "float4(0.0)", "float4(0.0)"),
            ("color3", "color", "color(0.0)", "color(0.0)"),
            ("matrix33", "float3x3", "float3x3(1.0)", "float3x3(1.0)"),
            ("matrix44", "float4x4", "float4x4(1.0)", "float4x4(1.0)"),
        ];
        for &(name, type_name, dv, udv) in agg {
            let td = syntax.type_system.get_type(name);
            syntax.register_type_syntax(td, TypeSyntax::scalar(type_name, dv, udv));
        }

        // color4: mk_color4(r, g, b, a) helper (MdlColor4TypeSyntax)
        let td = syntax.type_system.get_type("color4");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "color4",
                "mk_color4(0.0)",
                "mk_color4(0.0)",
                "",
                "",
                vec![],
            ),
        );

        // filename -> texture_2d
        let td = syntax.type_system.get_type("filename");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar("texture_2d", "texture_2d()", "texture_2d()"),
        );

        // All shader/closure types map to MDL "material"
        let material_types = [
            "BSDF",
            "EDF",
            "VDF",
            "surfaceshader",
            "volumeshader",
            "displacementshader",
            "lightshader",
            "material",
        ];
        for name in material_types {
            let td = syntax.type_system.get_type(name);
            syntax.register_type_syntax(
                td,
                TypeSyntax::scalar("material", "material()", "material()"),
            );
        }

        // floatarray / integerarray
        let td = syntax.type_system.get_type("floatarray");
        syntax.register_type_syntax(td, TypeSyntax::scalar("float", "", ""));
        let td = syntax.type_system.get_type("integerarray");
        syntax.register_type_syntax(td, TypeSyntax::scalar("int", "", ""));

        // MDL enum types (ref: MdlSyntax.cpp registerTypeSyntax for MDL_* types).
        let td = syntax.type_system.get_type("MDL_ADDRESSMODE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_addressmode_type",
                "mx_addressmode_type_periodic",
                "mx_addressmode_type_periodic",
            ),
        );
        let td = syntax.type_system.get_type("MDL_COORDINATESPACE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_coordinatespace_type",
                "mx_coordinatespace_type_model",
                "mx_coordinatespace_type_model",
            ),
        );
        let td = syntax.type_system.get_type("MDL_FILTERLOOKUPMODE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_filterlookup_type",
                "mx_filterlookup_type_linear",
                "mx_filterlookup_type_linear",
            ),
        );
        let td = syntax.type_system.get_type("MDL_FILTERTYPE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_filter_type",
                "mx_filter_type_gaussian",
                "mx_filter_type_gaussian",
            ),
        );
        let td = syntax.type_system.get_type("MDL_DISTRIBUTIONTYPE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_distribution_type",
                "mx_distribution_type_ggx",
                "mx_distribution_type_ggx",
            ),
        );
        let td = syntax.type_system.get_type("MDL_SCATTER_MODE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar("mx_scatter_mode", "mx_scatter_mode_R", "mx_scatter_mode_R"),
        );
        let td = syntax.type_system.get_type("MDL_SHEEN_MODE");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar(
                "mx_sheen_mode",
                "mx_sheen_mode_conty_kulla",
                "mx_sheen_mode_conty_kulla",
            ),
        );
    }

    fn register_reserved_words(syntax: &mut Syntax) {
        // Full MDL reserved word list from MDL Specification 1.9.2 (ref: MdlSyntax.cpp)
        let words = [
            // Reserved words
            "annotation",
            "double2",
            "float",
            "in",
            "operator",
            "auto",
            "double2x2",
            "float2",
            "int",
            "package",
            "bool",
            "double2x3",
            "float2x2",
            "int2",
            "return",
            "bool2",
            "double3",
            "float2x3",
            "int3",
            "string",
            "bool3",
            "double3x2",
            "float3",
            "int4",
            "struct",
            "bool4",
            "double3x3",
            "float3x2",
            "intensity_mode",
            "struct_category",
            "break",
            "double3x4",
            "float3x3",
            "intensity_power",
            "switch",
            "bsdf",
            "double4",
            "float3x4",
            "intensity_radiant_exitance",
            "texture_2d",
            "bsdf_measurement",
            "double4x3",
            "float4",
            "let",
            "texture_3d",
            "case",
            "double4x4",
            "float4x3",
            "light_profile",
            "texture_cube",
            "cast",
            "double4x2",
            "float4x4",
            "material",
            "texture_ptex",
            "color",
            "double2x4",
            "float4x2",
            "material_emission",
            "true",
            "const",
            "edf",
            "float2x4",
            "material_geometry",
            "typedef",
            "continue",
            "else",
            "for",
            "material_surface",
            "uniform",
            "declarative",
            "enum",
            "hair_bsdf",
            "material_volume",
            "using",
            "default",
            "export",
            "if",
            "mdl",
            "vdf",
            "do",
            "false",
            "import",
            "module",
            "while",
            // Reserved for future use
            "catch",
            "friend",
            "half3x4",
            "mutable",
            "sampler",
            "throw",
            "char",
            "goto",
            "half4",
            "namespace",
            "shader",
            "try",
            "class",
            "graph",
            "half4x3",
            "native",
            "short",
            "typeid",
            "const_cast",
            "half",
            "half4x4",
            "new",
            "signed",
            "typename",
            "delete",
            "half2",
            "half4x2",
            "out",
            "sizeof",
            "union",
            "dynamic_cast",
            "half2x2",
            "half2x4",
            "phenomenon",
            "static",
            "unsigned",
            "explicit",
            "half2x3",
            "inline",
            "private",
            "static_cast",
            "virtual",
            "extern",
            "half3",
            "inout",
            "protected",
            "technique",
            "void",
            "external",
            "half3x2",
            "lambda",
            "public",
            "template",
            "volatile",
            "foreach",
            "half3x3",
            "long",
            "reinterpret_cast",
            "this",
            "wchar_t",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    fn register_invalid_tokens(syntax: &mut Syntax) {
        // MDL disallows names beginning with underscore (ref: MdlSyntax constructor)
        // Note: the C++ regex is "\\b(_)" => "u", which replaces underscore at word boundary
        // We store this for the base Syntax::makeValidName to use
        syntax.register_invalid_tokens([("_".to_string(), "u".to_string())]);
    }

    // ---- MDL-specific methods matching C++ MdlSyntax ----

    /// Make a valid MDL identifier name (ref: MdlSyntax::makeValidName).
    /// C++ calls Syntax::makeValidName first, then prepends "v" to underscore-leading names.
    pub fn make_valid_name(&self, name: &mut String) {
        // Call base Syntax::makeValidName (handles reserved words and invalid tokens)
        self.syntax.make_valid_name(name);
        // MDL variables cannot begin with underscore -- prepend "v" (ref: MdlSyntax.cpp:571)
        if !name.is_empty() && name.starts_with('_') {
            *name = format!("v{}", name);
        }
    }

    /// Prefix for port names (ref: MdlSyntax::modifyPortName = PORT_NAME_PREFIX + name).
    pub fn modify_port_name(&self, name: &str) -> String {
        format!("{}{}", PORT_NAME_PREFIX, name)
    }

    /// Get MDL version suffix marker string (ref: MdlSyntax::getMdlVersionSuffixMarker).
    pub fn get_mdl_version_suffix_marker(&self) -> &str {
        MARKER_MDL_VERSION_SUFFIX
    }

    /// Replace {{marker}} tokens in source code (ref: MdlSyntax::replaceSourceCodeMarkers).
    pub fn replace_source_code_markers(
        &self,
        node_name: &str,
        source_code: &str,
        resolver: impl Fn(&str) -> String,
    ) -> String {
        let mut result = String::with_capacity(source_code.len());
        let mut pos = 0;
        while let Some(start) = source_code[pos..].find("{{") {
            let abs_start = pos + start;
            result.push_str(&source_code[pos..abs_start]);
            let after = abs_start + 2;
            if let Some(end) = source_code[after..].find("}}") {
                let marker = &source_code[after..after + end];
                result.push_str(&resolver(marker));
                pos = after + end + 2;
            } else {
                eprintln!(
                    "WARNING: Malformed inline expression in impl for node {}",
                    node_name
                );
                result.push_str(&source_code[abs_start..]);
                pos = source_code.len();
            }
        }
        result.push_str(&source_code[pos..]);
        result
    }

    /// Get type description for an enumeration member value (ref: MdlSyntax::getEnumeratedType).
    /// Returns the TypeDesc name if the value is found in any enum member list.
    pub fn get_enumerated_type(&self, value: &str) -> Option<&'static str> {
        for &(type_name, _, _, members) in ENUM_TYPES {
            if members.contains(&value) {
                return Some(type_name);
            }
        }
        None
    }

    /// Remap an enumeration string value to its typed MDL value (ref: MdlSyntax::remapEnumeration).
    /// Returns (type_desc_name, value_string) if successful.
    pub fn remap_enumeration(
        &self,
        value: &str,
        type_desc: &TypeDesc,
        enum_names: &str,
    ) -> Option<(String, String)> {
        // Early out if not an enum input
        if enum_names.is_empty() {
            return None;
        }
        // Don't convert filenames or arrays
        if type_desc.get_name() == "filename" || type_desc.is_array() {
            return None;
        }
        if value.is_empty() {
            return None;
        }

        let enum_type_name = self.get_enumerated_type(value)?;
        // Verify it's actually an enum semantic
        let enum_td = self.syntax.type_system.get_type(enum_type_name);
        if enum_td.get_semantic() != Semantic::Enum {
            return None;
        }

        // Find the value in the enum names list
        let enum_values: Vec<&str> = enum_names.split(',').map(|s| s.trim()).collect();
        if !enum_values.contains(&value) {
            return None;
        }

        Some((enum_type_name.to_string(), value.to_string()))
    }

    /// Get array type suffix -- returns "[N]" for array types (ref: MdlSyntax::getArrayTypeSuffix).
    pub fn get_array_type_suffix(&self, type_desc: &TypeDesc, value: &Value) -> String {
        if type_desc.is_array() {
            match value {
                Value::FloatArray(arr) => format!("[{}]", arr.len()),
                Value::IntegerArray(arr) => format!("[{}]", arr.len()),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    }

    /// Get the value string for a filename type (ref: MdlFilenameTypeSyntax::getValue).
    /// Handles empty paths, folder paths, relative/absolute path conversion.
    pub fn get_filename_value(&self, value_str: &str) -> String {
        if value_str.is_empty() || value_str == "/" {
            return "texture_2d()".to_string();
        }
        // Handle empty texture (fileprefix ending with slash)
        if value_str.ends_with('/') {
            return "texture_2d()".to_string();
        }
        // Check if last segment has an extension
        let last_slash = value_str.rfind('/');
        let last_dot = value_str.rfind('.');
        match (last_slash, last_dot) {
            (Some(s), Some(d)) if s > d => return "texture_2d()".to_string(),
            (_, None) => return "texture_2d()".to_string(),
            _ => {}
        }

        // Prefix a slash to make MDL resource paths absolute
        // Don't add slash if path is explicitly relative (starts with "./" or "../")
        let path_separator = if !value_str.starts_with('/')
            && !value_str.starts_with("../")
            && !value_str.starts_with("./")
        {
            "/"
        } else {
            ""
        };

        // Convert to POSIX path (replace backslashes)
        let posix_path = value_str.replace('\\', "/");

        format!(
            "texture_2d(\"{}{}\", tex::gamma_linear)",
            path_separator, posix_path
        )
    }

    /// Get the value string for a color4 type (ref: MdlColor4TypeSyntax::getValue).
    pub fn get_color4_value(&self, r: f32, g: f32, b: f32, a: f32) -> String {
        format!("mk_color4({:.6}, {:.6}, {:.6}, {:.6})", r, g, b, a)
    }

    /// Get the value string for an enum type (ref: MdlEnumSyntax::getValue).
    /// Returns "typename_value" format.
    pub fn get_enum_value(&self, type_name: &str, value: &str) -> String {
        format!("{}_{}", type_name, value)
    }

    /// Get the value string for an array type (ref: MdlArrayTypeSyntax::getValue).
    /// Returns "name[](values...)" for non-empty arrays.
    pub fn get_array_value(&self, base_type_name: &str, value: &Value) -> String {
        match value {
            Value::FloatArray(arr) if !arr.is_empty() => {
                let vals: Vec<String> = arr.iter().map(|v| format!("{:.6}", v)).collect();
                format!("{}[]({})", base_type_name, vals.join(", "))
            }
            Value::IntegerArray(arr) if !arr.is_empty() => {
                let vals: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("{}[]({})", base_type_name, vals.join(", "))
            }
            _ => String::new(),
        }
    }

    /// Output type name for MDL (material, float3, etc.)
    pub fn get_output_type_name(&self, type_name: &str) -> &str {
        match type_name {
            "surfaceshader" | "volumeshader" | "displacementshader" | "material" | "BSDF"
            | "EDF" | "VDF" => "material",
            "vector3" | "color3" => "float3",
            "vector4" | "color4" => "float4",
            "vector2" => "float2",
            "float" => "float",
            "integer" | "boolean" => "int",
            _ => "float",
        }
    }
}
