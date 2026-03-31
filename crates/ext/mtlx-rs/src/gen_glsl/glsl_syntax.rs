//! GlslSyntax -- GLSL type syntax and qualifiers (by ref MaterialX GenGlsl GlslSyntax).

use crate::gen_shader::{GlslValueFormat, Syntax, TypeSyntax, TypeSystem};

/// GLSL qualifiers
pub const INPUT_QUALIFIER: &str = "in";
pub const OUTPUT_QUALIFIER: &str = "out";
pub const UNIFORM_QUALIFIER: &str = "uniform";
pub const CONSTANT_QUALIFIER: &str = "const";
pub const FLAT_QUALIFIER: &str = "flat";
pub const SOURCE_FILE_EXTENSION: &str = ".glsl";

/// Vec member accessors (matches C++ GlslSyntax::VEC2_MEMBERS etc.)
pub const VEC2_MEMBERS: &[&str] = &[".x", ".y"];
pub const VEC3_MEMBERS: &[&str] = &[".x", ".y", ".z"];
pub const VEC4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];

/// GLSL syntax -- configured Syntax for OpenGL Shading Language.
pub struct GlslSyntax {
    pub syntax: Syntax,
}

impl GlslSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_glsl_types(&mut syntax);
        Self::register_reserved_words(&mut syntax);
        Self::register_invalid_tokens(&mut syntax);
        syntax.enum_remap_mode = crate::gen_shader::EnumRemapMode::StringToInteger;
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    /// Create syntax for WGSL (Vulkan GLSL + WGSL reserved words).
    pub fn create_wgsl(type_system: TypeSystem) -> Self {
        let mut s = Self::new(type_system);
        Self::register_wgsl_reserved_words(&mut s.syntax);
        s
    }

    /// Returns false for STRING type (GLSL has no native string support).
    pub fn type_supported(type_desc: &crate::gen_shader::TypeDesc) -> bool {
        type_desc.get_name() != "string"
    }

    fn register_glsl_types(syntax: &mut Syntax) {
        let types = [
            // Scalar types
            ("float", TypeSyntax::scalar("float", "0.0", "0.0")),
            ("integer", TypeSyntax::scalar("int", "0", "0")),
            // BOOLEAN -> bool (not int), matches C++ GlslSyntax
            ("boolean", TypeSyntax::scalar("bool", "false", "false")),
            // Array types: GlslFloatArrayTypeSyntax / GlslIntegerArrayTypeSyntax
            // C++ getValue: float[N](values) / int[N](values)
            ("floatarray", {
                let mut ts = TypeSyntax::scalar("float", "", "");
                ts.glsl_value_format = GlslValueFormat::GlslFloatArray;
                ts
            }),
            ("integerarray", {
                let mut ts = TypeSyntax::scalar("int", "", "");
                ts.glsl_value_format = GlslValueFormat::GlslIntegerArray;
                ts
            }),
            // Aggregate types with members
            (
                "color3",
                TypeSyntax::aggregate_full(
                    "vec3",
                    "vec3(0.0)",
                    "vec3(0.0)",
                    "",
                    "",
                    VEC3_MEMBERS.iter().map(|s| s.to_string()).collect(),
                ),
            ),
            (
                "color4",
                TypeSyntax::aggregate_full(
                    "vec4",
                    "vec4(0.0)",
                    "vec4(0.0)",
                    "",
                    "",
                    VEC4_MEMBERS.iter().map(|s| s.to_string()).collect(),
                ),
            ),
            (
                "vector2",
                TypeSyntax::aggregate_full(
                    "vec2",
                    "vec2(0.0)",
                    "vec2(0.0)",
                    "",
                    "",
                    VEC2_MEMBERS.iter().map(|s| s.to_string()).collect(),
                ),
            ),
            (
                "vector3",
                TypeSyntax::aggregate_full(
                    "vec3",
                    "vec3(0.0)",
                    "vec3(0.0)",
                    "",
                    "",
                    VEC3_MEMBERS.iter().map(|s| s.to_string()).collect(),
                ),
            ),
            (
                "vector4",
                TypeSyntax::aggregate_full(
                    "vec4",
                    "vec4(0.0)",
                    "vec4(0.0)",
                    "",
                    "",
                    VEC4_MEMBERS.iter().map(|s| s.to_string()).collect(),
                ),
            ),
            // Matrices
            (
                "matrix33",
                TypeSyntax::aggregate("mat3", "mat3(1.0)", "mat3(1.0)"),
            ),
            (
                "matrix44",
                TypeSyntax::aggregate("mat4", "mat4(1.0)", "mat4(1.0)"),
            ),
            // STRING -> int "0" (GLSL has no strings, always returns "0")
            ("string", TypeSyntax::scalar("int", "0", "0")),
            // FILENAME -> sampler2D (not int), matches C++ GlslSyntax
            ("filename", TypeSyntax::scalar("sampler2D", "", "")),
            // BSDF struct
            (
                "BSDF",
                TypeSyntax::aggregate_full(
                    "BSDF",
                    "BSDF(vec3(0.0),vec3(1.0))",
                    "",
                    "",
                    "struct BSDF { vec3 response; vec3 throughput; };",
                    Vec::new(),
                ),
            ),
            // EDF = vec3 via #define
            (
                "EDF",
                TypeSyntax::aggregate_full(
                    "EDF",
                    "EDF(0.0)",
                    "EDF(0.0)",
                    "vec3",
                    "#define EDF vec3",
                    Vec::new(),
                ),
            ),
            // VDF reuses BSDF type
            (
                "VDF",
                TypeSyntax::aggregate("BSDF", "BSDF(vec3(0.0),vec3(1.0))", ""),
            ),
            // surfaceshader struct
            (
                "surfaceshader",
                TypeSyntax::aggregate_full(
                    "surfaceshader",
                    "surfaceshader(vec3(0.0),vec3(0.0))",
                    "",
                    "",
                    "struct surfaceshader { vec3 color; vec3 transparency; };",
                    Vec::new(),
                ),
            ),
            // volumeshader struct
            (
                "volumeshader",
                TypeSyntax::aggregate_full(
                    "volumeshader",
                    "volumeshader(vec3(0.0),vec3(0.0))",
                    "",
                    "",
                    "struct volumeshader { vec3 color; vec3 transparency; };",
                    Vec::new(),
                ),
            ),
            // displacementshader struct
            (
                "displacementshader",
                TypeSyntax::aggregate_full(
                    "displacementshader",
                    "displacementshader(vec3(0.0),1.0)",
                    "",
                    "",
                    "struct displacementshader { vec3 offset; float scale; };",
                    Vec::new(),
                ),
            ),
            // lightshader struct
            (
                "lightshader",
                TypeSyntax::aggregate_full(
                    "lightshader",
                    "lightshader(vec3(0.0),vec3(0.0))",
                    "",
                    "",
                    "struct lightshader { vec3 intensity; vec3 direction; };",
                    Vec::new(),
                ),
            ),
            // material = surfaceshader via #define
            (
                "material",
                TypeSyntax::aggregate_full(
                    "material",
                    "material(vec3(0.0),vec3(0.0))",
                    "",
                    "surfaceshader",
                    "#define material surfaceshader",
                    Vec::new(),
                ),
            ),
        ];
        for (name, ts) in types {
            let td = syntax.type_system.get_type(name);
            syntax.register_type_syntax(td, ts);
        }
    }

    /// Full GLSL reserved words list (~130 entries), matching C++ GlslSyntax constructor.
    fn register_reserved_words(syntax: &mut Syntax) {
        // Full GLSL reserved word list (per C++ GlslSyntax.cpp)
        let words = [
            // Qualifiers
            "centroid",
            "flat",
            "smooth",
            "noperspective",
            "patch",
            "sample",
            // Control flow
            "break",
            "continue",
            "do",
            "for",
            "while",
            "switch",
            "case",
            "default",
            "if",
            "else,",
            "subroutine",
            "in",
            "out",
            "inout",
            // Basic types
            "float",
            "double",
            "int",
            "void",
            "bool",
            "true",
            "false",
            "invariant",
            "discard",
            "return",
            // Matrix types
            "mat2",
            "mat3",
            "mat4",
            "dmat2",
            "dmat3",
            "dmat4",
            "mat2x2",
            "mat2x3",
            "mat2x4",
            "dmat2x2",
            "dmat2x3",
            "dmat2x4",
            "mat3x2",
            "mat3x3",
            "mat3x4",
            "dmat3x2",
            "dmat3x3",
            "dmat3x4",
            "mat4x2",
            "mat4x3",
            "mat4x4",
            "dmat4x2",
            "dmat4x3",
            "dmat4x4",
            // Vector types
            "vec2",
            "vec3",
            "vec4",
            "ivec2",
            "ivec3",
            "ivec4",
            "bvec2",
            "bvec3",
            "bvec4",
            "dvec2",
            "dvec3",
            "dvec4",
            "uint",
            "uvec2",
            "uvec3",
            "uvec4",
            // Precision
            "lowp",
            "mediump",
            "highp",
            "precision",
            // Sampler types
            "sampler1D",
            "sampler2D",
            "sampler3D",
            "samplerCube",
            "sampler1DShadow",
            "sampler2DShadow",
            "samplerCubeShadow",
            "sampler1DArray",
            "sampler2DArray",
            "sampler1DArrayShadow",
            "sampler2DArrayShadow",
            "isampler1D",
            "isampler2D",
            "isampler3D",
            "isamplerCube",
            "isampler1DArray",
            "isampler2DArray",
            "usampler1D",
            "usampler2D",
            "usampler3D",
            "usamplerCube",
            "usampler1DArray",
            "usampler2DArray",
            "sampler2DRect",
            "sampler2DRectShadow",
            "isampler2DRect",
            "usampler2DRect",
            "samplerBuffer",
            "isamplerBuffer",
            "usamplerBuffer",
            "sampler2DMS",
            "isampler2DMS",
            "usampler2DMS",
            "sampler2DMSArray",
            "isampler2DMSArray",
            "usampler2DMSArray",
            "samplerCubeArray",
            "samplerCubeArrayShadow",
            "isamplerCubeArray",
            "usamplerCubeArray",
            // Reserved identifiers
            "common",
            "partition",
            "active",
            "asm",
            "struct",
            "class",
            "union",
            "enum",
            "typedef",
            "template",
            "this",
            "packed",
            "goto",
            "inline",
            "noinline",
            "volatile",
            "public",
            "static",
            "extern",
            "external",
            "interface",
            "long",
            "short",
            "half",
            "fixed",
            "unsigned",
            "superp",
            "input",
            "output",
            "hvec2",
            "hvec3",
            "hvec4",
            "fvec2",
            "fvec3",
            "fvec4",
            "sampler3DRect",
            "filter",
            // Image types
            "image1D",
            "image2D",
            "image3D",
            "imageCube",
            "iimage1D",
            "iimage2D",
            "iimage3D",
            "iimageCube",
            "uimage1D",
            "uimage2D",
            "uimage3D",
            "uimageCube",
            "image1DArray",
            "image2DArray",
            "iimage1DArray",
            "iimage2DArray",
            "uimage1DArray",
            "uimage2DArray",
            "image1DShadow",
            "image2DShadow",
            "image1DArrayShadow",
            "image2DArrayShadow",
            "imageBuffer",
            "iimageBuffer",
            "uimageBuffer",
            // Miscellaneous
            "sizeof",
            "cast",
            "namespace",
            "using",
            "row_major",
            "mix",
            "sampler",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    /// Register WGSL reserved words (for WgslSyntax).
    /// Full list matching C++ WgslSyntax::WgslSyntax constructor.
    fn register_wgsl_reserved_words(syntax: &mut Syntax) {
        let words = [
            // Keywords (https://www.w3.org/TR/WGSL/#keyword-summary)
            "alias",
            "break",
            "case",
            "const",
            "const_assert",
            "continue",
            "continuing",
            "default",
            "diagnostic",
            "discard",
            "else",
            "enable",
            "false",
            "fn",
            "for",
            "if",
            "let",
            "loop",
            "override",
            "requires",
            "return",
            "struct",
            "switch",
            "true",
            "var",
            "while",
            // Reserved Words (https://www.w3.org/TR/WGSL/#reserved-words)
            "NULL",
            "Self",
            "abstract",
            "active",
            "alignas",
            "alignof",
            "as",
            "asm",
            "asm_fragment",
            "async",
            "attribute",
            "auto",
            "await",
            "become",
            "cast",
            "catch",
            "class",
            "co_await",
            "co_return",
            "co_yield",
            "coherent",
            "column_major",
            "common",
            "compile",
            "compile_fragment",
            "concept",
            "const_cast",
            "consteval",
            "constexpr",
            "constinit",
            "crate",
            "debugger",
            "decltype",
            "delete",
            "demote",
            "demote_to_helper",
            "do",
            "dynamic_cast",
            "enum",
            "explicit",
            "export",
            "extends",
            "extern",
            "external",
            "fallthrough",
            "filter",
            "final",
            "finally",
            "friend",
            "from",
            "fxgroup",
            "get",
            "goto",
            "groupshared",
            "highp",
            "impl",
            "implements",
            "import",
            "inline",
            "instanceof",
            "interface",
            "layout",
            "lowp",
            "macro",
            "macro_rules",
            "match",
            "mediump",
            "meta",
            "mod",
            "module",
            "move",
            "mut",
            "mutable",
            "namespace",
            "new",
            "nil",
            "noexcept",
            "noinline",
            "nointerpolation",
            "non_coherent",
            "noncoherent",
            "noperspective",
            "null",
            "nullptr",
            "of",
            "operator",
            "package",
            "packoffset",
            "partition",
            "pass",
            "patch",
            "pixelfragment",
            "precise",
            "precision",
            "premerge",
            "priv",
            "protected",
            "pub",
            "public",
            "readonly",
            "ref",
            "regardless",
            "register",
            "reinterpret_cast",
            "require",
            "resource",
            "restrict",
            "self",
            "set",
            "shared",
            "sizeof",
            "smooth",
            "snorm",
            "static",
            "static_assert",
            "static_cast",
            "std",
            "subroutine",
            "super",
            "target",
            "template",
            "this",
            "thread_local",
            "throw",
            "trait",
            "try",
            "type",
            "typedef",
            "typeid",
            "typename",
            "typeof",
            "union",
            "unless",
            "unorm",
            "unsafe",
            "unsized",
            "use",
            "using",
            "varying",
            "virtual",
            "volatile",
            "wgsl",
            "where",
            "with",
            "writeonly",
            "yield",
            // WebGPU type keywords (added by VkShaderGenerator in C++, included here for completeness)
            "texture2D",
            "sampler",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    /// GLSL-specific invalid token replacements (matches C++ GlslSyntax).
    /// Prevents GLSL reserved prefixes: __ -> _, gl_ -> gll, webgl_ -> webgll, _webgl -> wwebgl
    fn register_invalid_tokens(syntax: &mut Syntax) {
        // C++ GlslSyntax restricted tokens (per GlslSyntax.cpp)
        syntax.register_invalid_tokens([
            ("__".to_string(), "_".to_string()),
            ("gl_".to_string(), "gll".to_string()),
            ("webgl_".to_string(), "webgll".to_string()),
            ("_webgl".to_string(), "wwebgl".to_string()),
        ]);
    }

    /// Create struct syntax with GLSL-specific recursive member formatting.
    /// Matches C++ GlslSyntax::createStructSyntax which returns GlslStructTypeSyntax.
    pub fn create_struct_syntax(
        &mut self,
        struct_type_name: &str,
        default_value: &str,
        uniform_default_value: &str,
        type_alias: &str,
        type_definition: &str,
    ) {
        let mut ts = self.syntax.create_struct_syntax(
            struct_type_name,
            default_value,
            uniform_default_value,
            type_alias,
            type_definition,
        );
        // Override with GLSL struct format: TypeName(member0,member1,...)
        ts.glsl_value_format = GlslValueFormat::GlslStruct;
        // Re-register the updated syntax
        let key = ts.name.clone();
        self.syntax.type_syntax_mut().insert(key, ts);
    }

    pub fn get_syntax(&self) -> &Syntax {
        &self.syntax
    }

    pub fn get_input_qualifier(&self) -> &str {
        INPUT_QUALIFIER
    }

    pub fn get_output_qualifier(&self) -> &str {
        OUTPUT_QUALIFIER
    }

    pub fn get_uniform_qualifier(&self) -> &str {
        UNIFORM_QUALIFIER
    }

    pub fn get_constant_qualifier(&self) -> &str {
        CONSTANT_QUALIFIER
    }

    pub fn get_flat_qualifier(&self) -> &str {
        FLAT_QUALIFIER
    }

    pub fn get_source_file_extension(&self) -> &str {
        SOURCE_FILE_EXTENSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::TypeSystem;

    fn make_syntax() -> GlslSyntax {
        GlslSyntax::new(TypeSystem::new())
    }

    // -- Type name mapping (matches C++ GlslSyntax.cpp) --

    #[test]
    fn glsl_type_float() {
        let s = make_syntax();
        let td = s.syntax.get_type("float");
        assert_eq!(s.syntax.get_type_name(&td), Some("float"));
    }

    #[test]
    fn glsl_type_color3_is_vec3() {
        let s = make_syntax();
        let td = s.syntax.get_type("color3");
        assert_eq!(s.syntax.get_type_name(&td), Some("vec3"));
    }

    #[test]
    fn glsl_type_vector3_is_vec3() {
        let s = make_syntax();
        let td = s.syntax.get_type("vector3");
        assert_eq!(s.syntax.get_type_name(&td), Some("vec3"));
    }

    #[test]
    fn glsl_type_color4_is_vec4() {
        let s = make_syntax();
        let td = s.syntax.get_type("color4");
        assert_eq!(s.syntax.get_type_name(&td), Some("vec4"));
    }

    #[test]
    fn glsl_type_vector4_is_vec4() {
        let s = make_syntax();
        let td = s.syntax.get_type("vector4");
        assert_eq!(s.syntax.get_type_name(&td), Some("vec4"));
    }

    #[test]
    fn glsl_type_vector2_is_vec2() {
        let s = make_syntax();
        let td = s.syntax.get_type("vector2");
        assert_eq!(s.syntax.get_type_name(&td), Some("vec2"));
    }

    #[test]
    fn glsl_type_matrix33_is_mat3() {
        let s = make_syntax();
        let td = s.syntax.get_type("matrix33");
        assert_eq!(s.syntax.get_type_name(&td), Some("mat3"));
    }

    #[test]
    fn glsl_type_matrix44_is_mat4() {
        let s = make_syntax();
        let td = s.syntax.get_type("matrix44");
        assert_eq!(s.syntax.get_type_name(&td), Some("mat4"));
    }

    #[test]
    fn glsl_type_boolean_is_bool() {
        let s = make_syntax();
        let td = s.syntax.get_type("boolean");
        assert_eq!(s.syntax.get_type_name(&td), Some("bool"));
    }

    #[test]
    fn glsl_type_integer_is_int() {
        let s = make_syntax();
        let td = s.syntax.get_type("integer");
        assert_eq!(s.syntax.get_type_name(&td), Some("int"));
    }

    #[test]
    fn glsl_type_string_is_int() {
        let s = make_syntax();
        let td = s.syntax.get_type("string");
        assert_eq!(s.syntax.get_type_name(&td), Some("int"));
    }

    #[test]
    fn glsl_type_filename_is_sampler2d() {
        let s = make_syntax();
        let td = s.syntax.get_type("filename");
        assert_eq!(s.syntax.get_type_name(&td), Some("sampler2D"));
    }

    #[test]
    fn glsl_type_surfaceshader() {
        let s = make_syntax();
        let td = s.syntax.get_type("surfaceshader");
        assert_eq!(s.syntax.get_type_name(&td), Some("surfaceshader"));
    }

    #[test]
    fn glsl_type_material() {
        let s = make_syntax();
        let td = s.syntax.get_type("material");
        assert_eq!(s.syntax.get_type_name(&td), Some("material"));
    }

    #[test]
    fn glsl_type_bsdf() {
        let s = make_syntax();
        let td = s.syntax.get_type("BSDF");
        assert_eq!(s.syntax.get_type_name(&td), Some("BSDF"));
    }

    #[test]
    fn glsl_type_edf() {
        let s = make_syntax();
        let td = s.syntax.get_type("EDF");
        assert_eq!(s.syntax.get_type_name(&td), Some("EDF"));
    }

    #[test]
    fn glsl_type_vdf_uses_bsdf() {
        let s = make_syntax();
        let td = s.syntax.get_type("VDF");
        assert_eq!(s.syntax.get_type_name(&td), Some("BSDF"));
    }

    #[test]
    fn glsl_type_volumeshader() {
        let s = make_syntax();
        let td = s.syntax.get_type("volumeshader");
        assert_eq!(s.syntax.get_type_name(&td), Some("volumeshader"));
    }

    #[test]
    fn glsl_type_displacementshader() {
        let s = make_syntax();
        let td = s.syntax.get_type("displacementshader");
        assert_eq!(s.syntax.get_type_name(&td), Some("displacementshader"));
    }

    #[test]
    fn glsl_type_lightshader() {
        let s = make_syntax();
        let td = s.syntax.get_type("lightshader");
        assert_eq!(s.syntax.get_type_name(&td), Some("lightshader"));
    }

    #[test]
    fn glsl_type_floatarray() {
        let s = make_syntax();
        let td = s.syntax.get_type("floatarray");
        assert_eq!(s.syntax.get_type_name(&td), Some("float"));
    }

    #[test]
    fn glsl_type_integerarray() {
        let s = make_syntax();
        let td = s.syntax.get_type("integerarray");
        assert_eq!(s.syntax.get_type_name(&td), Some("int"));
    }

    // -- Default values --

    #[test]
    fn glsl_default_float() {
        let s = make_syntax();
        let td = s.syntax.get_type("float");
        assert_eq!(s.syntax.get_default_value(&td, false), "0.0");
    }

    #[test]
    fn glsl_default_boolean() {
        let s = make_syntax();
        let td = s.syntax.get_type("boolean");
        assert_eq!(s.syntax.get_default_value(&td, false), "false");
    }

    #[test]
    fn glsl_default_color3() {
        let s = make_syntax();
        let td = s.syntax.get_type("color3");
        assert_eq!(s.syntax.get_default_value(&td, false), "vec3(0.0)");
    }

    #[test]
    fn glsl_default_color4() {
        let s = make_syntax();
        let td = s.syntax.get_type("color4");
        assert_eq!(s.syntax.get_default_value(&td, false), "vec4(0.0)");
    }

    #[test]
    fn glsl_default_matrix33() {
        let s = make_syntax();
        let td = s.syntax.get_type("matrix33");
        assert_eq!(s.syntax.get_default_value(&td, false), "mat3(1.0)");
    }

    #[test]
    fn glsl_default_matrix44() {
        let s = make_syntax();
        let td = s.syntax.get_type("matrix44");
        assert_eq!(s.syntax.get_default_value(&td, false), "mat4(1.0)");
    }

    #[test]
    fn glsl_default_bsdf() {
        let s = make_syntax();
        let td = s.syntax.get_type("BSDF");
        assert_eq!(
            s.syntax.get_default_value(&td, false),
            "BSDF(vec3(0.0),vec3(1.0))"
        );
    }

    #[test]
    fn glsl_default_surfaceshader() {
        let s = make_syntax();
        let td = s.syntax.get_type("surfaceshader");
        assert_eq!(
            s.syntax.get_default_value(&td, false),
            "surfaceshader(vec3(0.0),vec3(0.0))"
        );
    }

    // -- Type definitions --

    #[test]
    fn glsl_type_definition_bsdf() {
        let s = make_syntax();
        let td = s.syntax.get_type("BSDF");
        let def = s.syntax.get_type_definition(&td);
        assert_eq!(def, "struct BSDF { vec3 response; vec3 throughput; };");
    }

    #[test]
    fn glsl_type_definition_edf() {
        let s = make_syntax();
        let td = s.syntax.get_type("EDF");
        let def = s.syntax.get_type_definition(&td);
        assert_eq!(def, "#define EDF vec3");
    }

    #[test]
    fn glsl_type_definition_material() {
        let s = make_syntax();
        let td = s.syntax.get_type("material");
        let def = s.syntax.get_type_definition(&td);
        assert_eq!(def, "#define material surfaceshader");
    }

    #[test]
    fn glsl_type_alias_edf() {
        let s = make_syntax();
        let td = s.syntax.get_type("EDF");
        let alias = s.syntax.get_type_alias(&td);
        assert_eq!(alias, "vec3");
    }

    #[test]
    fn glsl_type_alias_material() {
        let s = make_syntax();
        let td = s.syntax.get_type("material");
        let alias = s.syntax.get_type_alias(&td);
        assert_eq!(alias, "surfaceshader");
    }

    // -- type_supported --

    #[test]
    fn glsl_type_supported_float() {
        let s = make_syntax();
        let td = s.syntax.get_type("float");
        assert!(GlslSyntax::type_supported(&td));
    }

    #[test]
    fn glsl_type_not_supported_string() {
        let s = make_syntax();
        let td = s.syntax.get_type("string");
        assert!(!GlslSyntax::type_supported(&td));
    }

    // -- Qualifiers --

    #[test]
    fn glsl_qualifiers() {
        assert_eq!(INPUT_QUALIFIER, "in");
        assert_eq!(OUTPUT_QUALIFIER, "out");
        assert_eq!(UNIFORM_QUALIFIER, "uniform");
        assert_eq!(CONSTANT_QUALIFIER, "const");
        assert_eq!(FLAT_QUALIFIER, "flat");
        assert_eq!(SOURCE_FILE_EXTENSION, ".glsl");
    }

    // -- Reserved words --

    #[test]
    fn glsl_reserved_words_contain_keywords() {
        let s = make_syntax();
        let rw = s.syntax.get_reserved_words();
        // Core GLSL keywords
        assert!(rw.contains("if"));
        assert!(rw.contains("for"));
        assert!(rw.contains("while"));
        assert!(rw.contains("return"));
        assert!(rw.contains("discard"));
        // Type keywords
        assert!(rw.contains("float"));
        assert!(rw.contains("int"));
        assert!(rw.contains("bool"));
        assert!(rw.contains("void"));
        assert!(rw.contains("vec2"));
        assert!(rw.contains("vec3"));
        assert!(rw.contains("vec4"));
        assert!(rw.contains("mat2"));
        assert!(rw.contains("mat3"));
        assert!(rw.contains("mat4"));
        // Extended matrix types
        assert!(rw.contains("dmat2"));
        assert!(rw.contains("mat2x2"));
        assert!(rw.contains("dmat4x4"));
        // Extended vector types
        assert!(rw.contains("ivec2"));
        assert!(rw.contains("bvec3"));
        assert!(rw.contains("dvec4"));
        assert!(rw.contains("uint"));
        assert!(rw.contains("uvec2"));
        // Sampler keywords
        assert!(rw.contains("sampler2D"));
        assert!(rw.contains("sampler3D"));
        assert!(rw.contains("samplerCube"));
        assert!(rw.contains("sampler1DShadow"));
        assert!(rw.contains("isampler2D"));
        assert!(rw.contains("usampler3D"));
        assert!(rw.contains("samplerCubeArray"));
        // Image types
        assert!(rw.contains("image1D"));
        assert!(rw.contains("image2D"));
        assert!(rw.contains("iimage1D"));
        assert!(rw.contains("uimage1D"));
        // Qualifier keywords
        assert!(rw.contains("in"));
        assert!(rw.contains("out"));
        assert!(rw.contains("inout"));
        assert!(rw.contains("centroid"));
        assert!(rw.contains("flat"));
        assert!(rw.contains("smooth"));
        // Precision
        assert!(rw.contains("lowp"));
        assert!(rw.contains("mediump"));
        assert!(rw.contains("highp"));
        assert!(rw.contains("precision"));
        // Misc
        assert!(rw.contains("mix"));
        assert!(rw.contains("sampler"));
        assert!(rw.contains("row_major"));
    }

    #[test]
    fn glsl_reserved_words_include_type_syntax_names() {
        let s = make_syntax();
        let rw = s.syntax.get_reserved_words();
        assert!(rw.contains("float"));
        assert!(rw.contains("int"));
        assert!(rw.contains("vec2"));
        assert!(rw.contains("vec3"));
        assert!(rw.contains("vec4"));
        assert!(rw.contains("mat3"));
        assert!(rw.contains("mat4"));
    }

    // -- WGSL reserved words --

    #[test]
    fn wgsl_syntax_has_extra_reserved_words() {
        let wgsl = GlslSyntax::create_wgsl(TypeSystem::new());
        let rw = wgsl.syntax.get_reserved_words();
        // WGSL-specific
        assert!(rw.contains("fn"));
        assert!(rw.contains("let"));
        assert!(rw.contains("var"));
        assert!(rw.contains("struct"));
        assert!(rw.contains("texture2D"));
        assert!(rw.contains("sampler"));
        // Also has GLSL base words
        assert!(rw.contains("if"));
        assert!(rw.contains("float"));
    }

    // -- Enum remap mode --

    #[test]
    fn glsl_enum_remap_is_string_to_integer() {
        let s = make_syntax();
        assert!(matches!(
            s.syntax.enum_remap_mode,
            crate::gen_shader::EnumRemapMode::StringToInteger
        ));
    }

    // -- Invalid token replacement --

    #[test]
    fn glsl_invalid_tokens_replace_gl_prefix() {
        let s = make_syntax();
        let mut name = "gl_Position".to_string();
        s.syntax.make_valid_name(&mut name);
        assert!(
            !name.starts_with("gl_"),
            "gl_ prefix should be replaced, got: {}",
            name
        );
    }

    #[test]
    fn glsl_invalid_tokens_replace_double_underscore() {
        let s = make_syntax();
        let mut name = "my__var".to_string();
        s.syntax.make_valid_name(&mut name);
        assert!(!name.contains("__"), "__ should be replaced, got: {}", name);
    }

    #[test]
    fn glsl_make_valid_name_appends_suffix_for_reserved() {
        let s = make_syntax();
        let mut name = "float".to_string();
        s.syntax.make_valid_name(&mut name);
        assert_ne!(name, "float");
        assert!(
            name.starts_with("float"),
            "should start with original name, got: {}",
            name
        );
    }

    // -- Shader type definitions --

    #[test]
    fn glsl_bsdf_type() {
        let s = make_syntax();
        let td = s.syntax.get_type("BSDF");
        assert_eq!(s.syntax.get_type_name(&td), Some("BSDF"));
    }

    #[test]
    fn glsl_edf_type() {
        let s = make_syntax();
        let td = s.syntax.get_type("EDF");
        assert_eq!(s.syntax.get_type_name(&td), Some("EDF"));
    }

    #[test]
    fn glsl_volumeshader_type() {
        let s = make_syntax();
        let td = s.syntax.get_type("volumeshader");
        assert_eq!(s.syntax.get_type_name(&td), Some("volumeshader"));
    }

    // -- GlslArrayTypeSyntax getValue tests --

    #[test]
    fn glsl_float_array_value_format() {
        let s = make_syntax();
        let td = s.syntax.get_type("floatarray");
        let val = crate::core::Value::FloatArray(vec![1.0, 2.0, 3.0]);
        let result = s.syntax.get_value(&td, &val, false);
        // C++ GlslFloatArrayTypeSyntax: float[3](1, 2, 3)
        assert!(result.starts_with("float["), "got: {}", result);
        assert!(
            result.contains("[3]"),
            "should have size 3, got: {}",
            result
        );
    }

    #[test]
    fn glsl_integer_array_value_format() {
        let s = make_syntax();
        let td = s.syntax.get_type("integerarray");
        let val = crate::core::Value::IntegerArray(vec![10, 20]);
        let result = s.syntax.get_value(&td, &val, false);
        // C++ GlslIntegerArrayTypeSyntax: int[2](10, 20)
        assert!(result.starts_with("int["), "got: {}", result);
        assert!(
            result.contains("[2]"),
            "should have size 2, got: {}",
            result
        );
    }

    #[test]
    fn glsl_empty_float_array_returns_empty() {
        let s = make_syntax();
        let td = s.syntax.get_type("floatarray");
        let val = crate::core::Value::FloatArray(vec![]);
        let result = s.syntax.get_value(&td, &val, false);
        assert!(
            result.is_empty(),
            "empty array should return empty, got: {}",
            result
        );
    }

    // -- GlslStructTypeSyntax createStructSyntax test --

    #[test]
    fn glsl_create_struct_syntax_sets_glsl_format() {
        let mut s = make_syntax();
        s.create_struct_syntax(
            "MyGlslStruct",
            "MyGlslStruct(0.0,0)",
            "",
            "",
            "struct MyGlslStruct { float a; int b; };",
        );
        // Verify it was registered with GlslStruct format
        let ts = s.syntax.type_syntax_mut().get("MyGlslStruct");
        assert!(ts.is_some());
        let ts = ts.unwrap();
        assert_eq!(ts.glsl_value_format, GlslValueFormat::GlslStruct);
    }

    // -- remapEnumeration test --

    #[test]
    fn glsl_remap_enumeration() {
        let s = make_syntax();
        let string_td = s.syntax.get_type("string");
        let result = s.syntax.remap_enumeration("bar", &string_td, "foo,bar,baz");
        assert!(result.is_some(), "should remap 'bar' to integer index");
        let (td, val) = result.unwrap();
        assert_eq!(td.get_name(), "integer");
        if let crate::core::Value::Integer(idx) = val {
            assert_eq!(idx, 1, "'bar' is at index 1");
        } else {
            panic!("expected Value::Integer, got: {:?}", val);
        }
    }

    #[test]
    fn glsl_remap_enumeration_not_found() {
        let s = make_syntax();
        let string_td = s.syntax.get_type("string");
        let result = s
            .syntax
            .remap_enumeration("missing", &string_td, "foo,bar,baz");
        assert!(result.is_none(), "should return None for unknown value");
    }

    #[test]
    fn glsl_remap_enumeration_non_string_type() {
        let s = make_syntax();
        let float_td = s.syntax.get_type("float");
        let result = s.syntax.remap_enumeration("foo", &float_td, "foo,bar");
        assert!(result.is_none(), "should return None for non-string type");
    }

    // -- WGSL reserved words completeness --

    #[test]
    fn wgsl_has_cpp_reserved_words() {
        let wgsl = GlslSyntax::create_wgsl(TypeSystem::new());
        let rw = wgsl.syntax.get_reserved_words();
        // Spot-check words that were missing before the fix
        assert!(rw.contains("alignas"), "missing: alignas");
        assert!(rw.contains("co_await"), "missing: co_await");
        assert!(rw.contains("demote_to_helper"), "missing: demote_to_helper");
        assert!(rw.contains("fxgroup"), "missing: fxgroup");
        assert!(rw.contains("groupshared"), "missing: groupshared");
        assert!(rw.contains("macro_rules"), "missing: macro_rules");
        assert!(rw.contains("nointerpolation"), "missing: nointerpolation");
        assert!(rw.contains("snorm"), "missing: snorm");
        assert!(rw.contains("unorm"), "missing: unorm");
        assert!(rw.contains("unsafe"), "missing: unsafe");
    }
}
