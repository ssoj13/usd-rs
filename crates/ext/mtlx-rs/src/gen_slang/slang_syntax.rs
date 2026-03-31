//! SlangSyntax -- Slang type syntax (ref: MaterialXGenSlang SlangSyntax).

use crate::gen_shader::{EnumRemapMode, SlangValueFormat, Syntax, TypeSyntax, TypeSystem};

/// Slang source file extension
pub const SOURCE_FILE_EXTENSION: &str = ".slang";

/// Slang qualifiers (ref: SlangSyntax.h)
pub const INPUT_QUALIFIER: &str = "";
pub const OUTPUT_QUALIFIER: &str = "out";
pub const UNIFORM_QUALIFIER: &str = "uniform";
pub const CONSTANT_QUALIFIER: &str = "const";
pub const FLAT_QUALIFIER: &str = "nointerpolation";

/// VEC member accessors (ref: SlangSyntax.cpp VEC2_MEMBERS, VEC3_MEMBERS, VEC4_MEMBERS)
pub const VEC2_MEMBERS: &[&str] = &[".x", ".y"];
pub const VEC3_MEMBERS: &[&str] = &[".x", ".y", ".z"];
pub const VEC4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];

/// Slang syntax -- HLSL-like types for Slang compiler.
pub struct SlangSyntax {
    pub syntax: Syntax,
}

impl SlangSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_reserved_words(&mut syntax);
        Self::register_slang_types(&mut syntax);
        syntax.enum_remap_mode = EnumRemapMode::StringToInteger;
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    pub fn get_syntax(&self) -> &Syntax {
        &self.syntax
    }
    pub fn get_syntax_mut(&mut self) -> &mut Syntax {
        &mut self.syntax
    }
    pub fn get_input_qualifier(&self) -> &str {
        INPUT_QUALIFIER
    }
    pub fn get_output_qualifier(&self) -> &str {
        OUTPUT_QUALIFIER
    }
    pub fn get_constant_qualifier(&self) -> &str {
        CONSTANT_QUALIFIER
    }

    /// Check type support (ref: SlangSyntax::typeSupported). STRING not supported.
    pub fn type_supported(&self, type_name: &str) -> bool {
        type_name != "string"
    }

    /// Prepend "v_" if name starts with digit (ref: SlangSyntax::makeValidName).
    pub fn make_valid_name(&self, name: &mut String) {
        self.syntax.make_valid_name(name);
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            *name = format!("v_{}", name);
        }
    }

    /// Create struct syntax for Slang (ref: SlangSyntax::createStructSyntax).
    /// Returns a SlangStructTypeSyntax that formats values with recursive member emission.
    pub fn create_struct_syntax(
        &mut self,
        struct_type_name: &str,
        default_value: &str,
        uniform_default_value: &str,
        type_alias: &str,
        type_definition: &str,
    ) -> TypeSyntax {
        self.syntax.create_struct_syntax_slang(
            struct_type_name,
            default_value,
            uniform_default_value,
            type_alias,
            type_definition,
        )
    }

    fn register_reserved_words(syntax: &mut Syntax) {
        // Reserved words from C++ SlangSyntax constructor
        let words = [
            "throws",
            "static",
            "const",
            "in",
            "out",
            "inout",
            "ref",
            "__subscript",
            "__init",
            "property",
            "get",
            "set",
            "class",
            "struct",
            "interface",
            "public",
            "private",
            "internal",
            "protected",
            "typedef",
            "typealias",
            "uniform",
            "export",
            "groupshared",
            "extension",
            "associatedtype",
            "namespace",
            "This",
            "using",
            "__generic",
            "__exported",
            "import",
            "enum",
            "cbuffer",
            "tbuffer",
            "func",
            "if",
            "else",
            "switch",
            "case",
            "default",
            "return",
            "try",
            "throw",
            "catch",
            "while",
            "for",
            "do",
            "break",
            "continue",
            "discard",
            "defer",
            "is",
            "as",
            "nullptr",
            "none",
            "true",
            "false",
            "SamplerTexture2D",
            "this",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    fn register_slang_types(syntax: &mut Syntax) {
        // Helper to create a Slang aggregate TypeSyntax with proper SlangValueFormat
        fn slang_agg(
            name: &str,
            dv: &str,
            udv: &str,
            alias: &str,
            def: &str,
            members: Vec<String>,
        ) -> TypeSyntax {
            let mut ts = TypeSyntax::aggregate_full(name, dv, udv, alias, def, members);
            ts.slang_value_format = SlangValueFormat::SlangAggregate;
            ts
        }

        // Scalar types
        let td = syntax.type_system.get_type("float");
        syntax.register_type_syntax(td, TypeSyntax::scalar("float", "0.0", "0.0"));

        // floatarray: SlangFloatArrayTypeSyntax (ref: size-aware brace wrapping)
        let td = syntax.type_system.get_type("floatarray");
        let mut ts = TypeSyntax::scalar("float", "", "");
        ts.slang_value_format = SlangValueFormat::SlangFloatArray;
        syntax.register_type_syntax(td, ts);

        let td = syntax.type_system.get_type("integer");
        syntax.register_type_syntax(td, TypeSyntax::scalar("int", "0", "0"));

        // integerarray: SlangIntegerArrayTypeSyntax (ref: size-aware brace wrapping)
        let td = syntax.type_system.get_type("integerarray");
        let mut ts = TypeSyntax::scalar("int", "", "");
        ts.slang_value_format = SlangValueFormat::SlangIntegerArray;
        syntax.register_type_syntax(td, ts);

        let td = syntax.type_system.get_type("boolean");
        syntax.register_type_syntax(td, TypeSyntax::scalar("bool", "false", "false"));

        // string -> int with SlangStringTypeSyntax (always returns "0")
        let td = syntax.type_system.get_type("string");
        let mut ts = TypeSyntax::scalar("int", "0", "0");
        ts.slang_value_format = SlangValueFormat::SlangString;
        syntax.register_type_syntax(td, ts);

        // Vector/color types with VEC member accessors (ref: SlangSyntax.cpp)
        let vec2: Vec<String> = VEC2_MEMBERS.iter().map(|s| s.to_string()).collect();
        let vec3: Vec<String> = VEC3_MEMBERS.iter().map(|s| s.to_string()).collect();
        let vec4: Vec<String> = VEC4_MEMBERS.iter().map(|s| s.to_string()).collect();

        let td = syntax.type_system.get_type("color3");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float3",
                "float3(0.0)",
                "0.0, 0.0, 0.0",
                "",
                "",
                vec3.clone(),
            ),
        );

        let td = syntax.type_system.get_type("color4");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float4",
                "float4(0.0)",
                "0.0, 0.0, 0.0, 0.0",
                "",
                "",
                vec4.clone(),
            ),
        );

        let td = syntax.type_system.get_type("vector2");
        syntax.register_type_syntax(
            td,
            slang_agg("float2", "float2(0.0)", "0.0, 0.0", "", "", vec2.clone()),
        );

        let td = syntax.type_system.get_type("vector3");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float3",
                "float3(0.0)",
                "0.0, 0.0, 0.0",
                "",
                "",
                vec3.clone(),
            ),
        );

        let td = syntax.type_system.get_type("vector4");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float4",
                "float4(0.0)",
                "0.0, 0.0, 0.0, 0.0",
                "",
                "",
                vec4.clone(),
            ),
        );

        // Matrix types
        let td = syntax.type_system.get_type("matrix33");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float3x3",
                "float3x3(1,0,0,  0,1,0, 0,0,1)",
                "1,0,0,  0,1,0, 0,0,1",
                "",
                "",
                vec![],
            ),
        );
        let td = syntax.type_system.get_type("matrix44");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "float4x4",
                "float4x4(1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1)",
                "1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1",
                "",
                "",
                vec![],
            ),
        );

        // filename -> SamplerTexture2D
        let td = syntax.type_system.get_type("filename");
        syntax.register_type_syntax(td, TypeSyntax::scalar("SamplerTexture2D", "", ""));

        // BSDF struct { float3 response; float3 throughput; }
        let td = syntax.type_system.get_type("BSDF");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "BSDF",
                "BSDF(float3(0.0),float3(1.0))",
                "",
                "",
                "struct BSDF { float3 response; float3 throughput; };",
                vec![],
            ),
        );

        // EDF: #define EDF float3
        let td = syntax.type_system.get_type("EDF");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "EDF",
                "EDF(0.0)",
                "0.0, 0.0, 0.0",
                "float3",
                "#define EDF float3",
                vec![],
            ),
        );

        // VDF: same struct as BSDF
        let td = syntax.type_system.get_type("VDF");
        syntax.register_type_syntax(
            td,
            slang_agg("BSDF", "BSDF(float3(0.0),float3(1.0))", "", "", "", vec![]),
        );

        // surfaceshader struct { float3 color; float3 transparency; }
        let td = syntax.type_system.get_type("surfaceshader");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "surfaceshader",
                "surfaceshader(float3(0.0),float3(0.0))",
                "",
                "",
                "struct surfaceshader { float3 color; float3 transparency; };",
                vec![],
            ),
        );

        // volumeshader struct
        let td = syntax.type_system.get_type("volumeshader");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "volumeshader",
                "volumeshader(float3(0.0),float3(0.0))",
                "",
                "",
                "struct volumeshader { float3 color; float3 transparency; };",
                vec![],
            ),
        );

        // displacementshader struct { float3 offset; float scale; }
        let td = syntax.type_system.get_type("displacementshader");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "displacementshader",
                "displacementshader(float3(0.0),1.0)",
                "",
                "",
                "struct displacementshader { float3 offset; float scale; };",
                vec![],
            ),
        );

        // lightshader struct { float3 intensity; float3 direction; }
        let td = syntax.type_system.get_type("lightshader");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "lightshader",
                "lightshader(float3(0.0),float3(0.0))",
                "",
                "",
                "struct lightshader { float3 intensity; float3 direction; };",
                vec![],
            ),
        );

        // material: alias to surfaceshader (#define material surfaceshader)
        let td = syntax.type_system.get_type("material");
        syntax.register_type_syntax(
            td,
            slang_agg(
                "material",
                "material(float3(0.0),float3(0.0))",
                "",
                "surfaceshader",
                "#define material surfaceshader",
                vec![],
            ),
        );
    }
}
