//! MslSyntax -- Metal Shading Language type syntax (ref: MaterialXGenMsl MslSyntax).

use crate::gen_shader::{EnumRemapMode, Syntax, TypeSyntax, TypeSystem};

/// MSL qualifiers (ref: MslSyntax.h)
pub const INPUT_QUALIFIER: &str = "in";
pub const OUTPUT_QUALIFIER: &str = "out";
pub const UNIFORM_QUALIFIER: &str = "constant";
pub const CONSTANT_QUALIFIER: &str = "const";
pub const FLAT_QUALIFIER: &str = "flat";
pub const SOURCE_FILE_EXTENSION: &str = ".metal";
pub const STRUCT_KEYWORD: &str = "struct";

/// VEC member access patterns (ref: MslSyntax.cpp)
pub const VEC2_MEMBERS: &[&str] = &[".x", ".y"];
pub const VEC3_MEMBERS: &[&str] = &[".x", ".y", ".z"];
pub const VEC4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];

/// MSL syntax -- Metal type mappings (float2, float3, float4, float3x3, float4x4).
pub struct MslSyntax {
    pub syntax: Syntax,
}

impl MslSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_reserved_words(&mut syntax);
        Self::register_invalid_tokens(&mut syntax);
        Self::register_msl_types(&mut syntax);
        syntax.enum_remap_mode = EnumRemapMode::StringToInteger;
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    fn register_msl_types(syntax: &mut Syntax) {
        // Scalar primitives
        let scalar_types = [
            ("boolean", TypeSyntax::scalar("bool", "false", "false")),
            ("integer", TypeSyntax::scalar("int", "0", "0")),
            ("float", TypeSyntax::scalar("float", "0.0", "0.0")),
            // string -> int (MSL has no string type; MslStringTypeSyntax always returns "0")
            ("string", TypeSyntax::scalar("int", "0", "0")),
        ];
        for (name, ts) in scalar_types {
            let td = syntax.type_system.get_type(name);
            syntax.register_type_syntax(td, ts);
        }

        // Vector types with member access (ref: VEC2/3/4_MEMBERS)
        let vec2_members: Vec<String> = VEC2_MEMBERS.iter().map(|s| s.to_string()).collect();
        let vec3_members: Vec<String> = VEC3_MEMBERS.iter().map(|s| s.to_string()).collect();
        let vec4_members: Vec<String> = VEC4_MEMBERS.iter().map(|s| s.to_string()).collect();

        // vector2: float2 with .x, .y members
        let td = syntax.type_system.get_type("vector2");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float2",
                "float2(0.0)",
                "float2(0.0)",
                "",
                "",
                vec2_members.clone(),
            ),
        );

        // vector3: float3 with .x, .y, .z members
        let td = syntax.type_system.get_type("vector3");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float3",
                "float3(0.0)",
                "float3(0.0)",
                "",
                "",
                vec3_members.clone(),
            ),
        );

        // vector4: float4 with .x, .y, .z, .w members
        let td = syntax.type_system.get_type("vector4");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float4",
                "float4(0.0)",
                "float4(0.0)",
                "",
                "",
                vec4_members.clone(),
            ),
        );

        // color3: float3 with .x, .y, .z members
        let td = syntax.type_system.get_type("color3");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float3",
                "float3(0.0)",
                "float3(0.0)",
                "",
                "",
                vec3_members.clone(),
            ),
        );

        // color4: float4 with .x, .y, .z, .w members
        let td = syntax.type_system.get_type("color4");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float4",
                "float4(0.0)",
                "float4(0.0)",
                "",
                "",
                vec4_members.clone(),
            ),
        );

        // matrix33, matrix44 (no members)
        let td = syntax.type_system.get_type("matrix33");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float3x3",
                "float3x3(1.0)",
                "float3x3(1.0)",
                "",
                "",
                vec![],
            ),
        );
        let td = syntax.type_system.get_type("matrix44");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "float4x4",
                "float4x4(1.0)",
                "float4x4(1.0)",
                "",
                "",
                vec![],
            ),
        );

        // filename -> MetalTexture
        let td = syntax.type_system.get_type("filename");
        syntax.register_type_syntax(td, TypeSyntax::scalar("MetalTexture", "", ""));

        // BSDF struct { float3 response; float3 throughput; }
        let td = syntax.type_system.get_type("BSDF");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "BSDF",
                "BSDF{float3(0.0),float3(1.0)}",
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
            TypeSyntax::aggregate_full(
                "EDF",
                "EDF(0.0)",
                "EDF(0.0)",
                "float3",
                "#define EDF float3",
                vec![],
            ),
        );

        // VDF: same struct as BSDF
        let td = syntax.type_system.get_type("VDF");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full("BSDF", "BSDF{float3(0.0),float3(1.0)}", "", "", "", vec![]),
        );

        // surfaceshader struct { float3 color; float3 transparency; }
        let td = syntax.type_system.get_type("surfaceshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "surfaceshader",
                "surfaceshader{float3(0.0),float3(0.0)}",
                "",
                "",
                "struct surfaceshader { float3 color; float3 transparency; };",
                vec![],
            ),
        );

        // volumeshader struct { float3 color; float3 transparency; }
        let td = syntax.type_system.get_type("volumeshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "volumeshader",
                "volumeshader{float3(0.0),float3(0.0)}",
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
            TypeSyntax::aggregate_full(
                "displacementshader",
                "displacementshader{float3(0.0),1.0}",
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
            TypeSyntax::aggregate_full(
                "lightshader",
                "lightshader{float3(0.0),float3(0.0)}",
                "",
                "",
                "struct lightshader { float3 intensity; float3 direction; };",
                vec![],
            ),
        );

        // material: alias to surfaceshader
        let td = syntax.type_system.get_type("material");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "material",
                "material{float3(0.0),float3(0.0)}",
                "",
                "surfaceshader",
                "#define material surfaceshader",
                vec![],
            ),
        );

        // floatarray: MslFloatArrayTypeSyntax -- returns "{v1, v2, ...}" for arrays
        let td = syntax.type_system.get_type("floatarray");
        syntax.register_type_syntax(td, TypeSyntax::scalar("float", "", ""));

        // integerarray: MslIntegerArrayTypeSyntax -- returns "{v1, v2, ...}" for arrays
        let td = syntax.type_system.get_type("integerarray");
        syntax.register_type_syntax(td, TypeSyntax::scalar("int", "", ""));
    }

    fn register_reserved_words(syntax: &mut Syntax) {
        // Full MSL reserved word list from C++ MslSyntax constructor
        let words = [
            "centroid",
            "flat",
            "smooth",
            "noperspective",
            "patch",
            "sample",
            "break",
            "continue",
            "do",
            "for",
            "while",
            "switch",
            "case",
            "default",
            "if",
            "else",
            "subroutine",
            "in",
            "out",
            "inout",
            "float",
            "double",
            "int",
            "void",
            "bool",
            "true",
            "false",
            "invariant",
            "discard_fragment",
            "return",
            "float2x2",
            "float2x3",
            "float2x4",
            "float3x2",
            "float3x3",
            "float3x4",
            "float4x2",
            "float4x3",
            "float4x4",
            "float2",
            "float3",
            "float4",
            "int2",
            "int3",
            "int4",
            "bool2",
            "bool3",
            "bool4",
            "uint",
            "uint2",
            "uint3",
            "uint4",
            "lowp",
            "mediump",
            "highp",
            "precision",
            "sampler",
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
            "half2",
            "half3",
            "half4",
            "sampler3DRect",
            "filter",
            "texture1d",
            "texture2d",
            "texture3d",
            "textureCube",
            "buffer",
            "sizeof",
            "cast",
            "namespace",
            "using",
            "row_major",
            "mix",
            "sampler",
            // MSL-specific builtins
            "device",
            "constant",
            "thread",
            "threadgroup",
            "vertex",
            "fragment",
            "kernel",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    fn register_invalid_tokens(syntax: &mut Syntax) {
        // MSL invalid token prefixes (ref: MslSyntax constructor)
        syntax.register_invalid_tokens([
            ("__".to_string(), "_".to_string()),
            ("gl_".to_string(), "gll".to_string()),
            ("webgl_".to_string(), "webgll".to_string()),
            ("_webgl".to_string(), "wwebgl".to_string()),
        ]);
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
    pub fn get_struct_keyword(&self) -> &str {
        STRUCT_KEYWORD
    }
    pub fn get_source_file_extension(&self) -> &str {
        SOURCE_FILE_EXTENSION
    }

    /// Return MSL output type name: "thread TYPE&" (ref: MslSyntax::getOutputTypeName).
    pub fn get_output_type_name(&self, type_name: &str) -> String {
        let td = self.syntax.type_system.get_type(type_name);
        let name = self.syntax.get_type_name(&td).unwrap_or(type_name);
        format!("thread {}&", name)
    }

    /// Return false for STRING type (ref: MslSyntax::typeSupported).
    /// MSL has no string type; strings are remapped to integers.
    pub fn type_supported(&self, type_name: &str) -> bool {
        type_name != "string"
    }

    /// Remap enumeration values to integer indices (ref: MslSyntax::remapEnumeration).
    /// For MSL we always convert STRING enums to INTEGER with the value being the index.
    pub fn remap_enumeration(
        &self,
        value: &str,
        type_name: &str,
        enum_names: &str,
    ) -> Option<(String, i32)> {
        // Early out if not an enum input
        if enum_names.is_empty() {
            return None;
        }
        // Don't convert already supported types
        if type_name != "string" {
            return None;
        }
        // Early out if no valid value provided
        if value.is_empty() {
            return None;
        }
        // Split enum names by comma and find value index
        let enums: Vec<&str> = enum_names.split(',').map(|s| s.trim()).collect();
        let index = enums.iter().position(|&e| e == value)?;
        Some(("integer".to_string(), index as i32))
    }
}
