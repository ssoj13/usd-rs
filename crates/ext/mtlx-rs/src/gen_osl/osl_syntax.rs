//! OslSyntax — OSL type syntax (ref: MaterialXGenOsl OslSyntax).
//! Open Shading Language types: color, vector, vector2, vector4, matrix, etc.

use crate::gen_shader::{OslValueFormat, Syntax, TypeSyntax, TypeSystem};

/// OSL output qualifier
pub const OUTPUT_QUALIFIER: &str = "output";
/// OSL source file extension
pub const SOURCE_FILE_EXTENSION: &str = ".osl";

/// OSL member accessors — used by emit code for component swizzling
#[allow(dead_code)] // Used by OSL emit code to swizzle vector3 components
pub const VECTOR_MEMBERS: &[&str] = &["[0]", "[1]", "[2]"];
#[allow(dead_code)] // Used by OSL emit code to swizzle vector2 components
pub const VECTOR2_MEMBERS: &[&str] = &[".x", ".y"];
#[allow(dead_code)] // Used by OSL emit code to swizzle vector4 components
pub const VECTOR4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];
#[allow(dead_code)] // Used by OSL emit code to swizzle color4 components
pub const COLOR4_MEMBERS: &[&str] = &[".rgb[0]", ".rgb[1]", ".rgb[2]", ".a"];

/// OSL syntax — configured Syntax for Open Shading Language.
pub struct OslSyntax {
    pub syntax: Syntax,
}

impl OslSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_reserved_words(&mut syntax);
        Self::register_invalid_tokens(&mut syntax);
        Self::register_osl_types(&mut syntax);
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    fn register_osl_types(syntax: &mut Syntax) {
        // boolean -> int (OslBooleanTypeSyntax: true/false -> 1/0)
        let td = syntax.type_system.get_type("boolean");
        let mut ts =
            TypeSyntax::scalar_full("int", "0", "0", "", "#define true 1\n#define false 0");
        ts.osl_value_format = OslValueFormat::Boolean;
        syntax.register_type_syntax(td, ts);

        let td = syntax.type_system.get_type("integer");
        syntax.register_type_syntax(td, TypeSyntax::scalar("int", "0", "0"));

        let td = syntax.type_system.get_type("float");
        syntax.register_type_syntax(td, TypeSyntax::scalar("float", "0.0", "0.0"));

        // color3: built-in OSL type (AggregateTypeSyntax, NOT OslVecTypeSyntax)
        let td = syntax.type_system.get_type("color3");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "color",
                "color(0.0)",
                "color(0.0)",
                "",
                "",
                VECTOR_MEMBERS.iter().map(|s| s.to_string()).collect(),
            ),
        );

        // color4: struct { color rgb; float a } (OslColor4TypeSyntax)
        let td = syntax.type_system.get_type("color4");
        let mut ts = TypeSyntax::aggregate_full(
            "color4",
            "color4(color(0.0), 0.0)",
            "{color(0.0), 0.0}",
            "",
            "",
            COLOR4_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::OslColor4;
        syntax.register_type_syntax(td, ts);

        // vector2: custom struct (OslVecTypeSyntax: uniform->{vals}, non-uniform->type(vals))
        let td = syntax.type_system.get_type("vector2");
        let mut ts = TypeSyntax::aggregate_full(
            "vector2",
            "vector2(0.0, 0.0)",
            "{0.0, 0.0}",
            "",
            "",
            VECTOR2_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::OslVec;
        syntax.register_type_syntax(td, ts);

        // vector3: built-in OSL "vector" (AggregateTypeSyntax, NOT OslVecTypeSyntax)
        let td = syntax.type_system.get_type("vector3");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "vector",
                "vector(0.0)",
                "vector(0.0)",
                "",
                "",
                VECTOR_MEMBERS.iter().map(|s| s.to_string()).collect(),
            ),
        );

        // vector4: custom struct (OslVecTypeSyntax)
        let td = syntax.type_system.get_type("vector4");
        let mut ts = TypeSyntax::aggregate_full(
            "vector4",
            "vector4(0.0, 0.0, 0.0, 0.0)",
            "{0.0, 0.0, 0.0, 0.0}",
            "",
            "",
            VECTOR4_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::OslVec;
        syntax.register_type_syntax(td, ts);

        // matrix33 -> OSL "matrix" (OSLMatrix3TypeSyntax: expand 3x3 to 4x4)
        let td = syntax.type_system.get_type("matrix33");
        let mut ts =
            TypeSyntax::aggregate_full("matrix", "matrix(1.0)", "matrix(1.0)", "", "", vec![]);
        ts.osl_value_format = OslValueFormat::OslMatrix3;
        syntax.register_type_syntax(td, ts);

        // matrix44: standard aggregate (no expand needed)
        let td = syntax.type_system.get_type("matrix44");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full("matrix", "matrix(1.0)", "matrix(1.0)", "", "", vec![]),
        );

        let td = syntax.type_system.get_type("string");
        syntax.register_type_syntax(td, TypeSyntax::scalar("string", "\"\"", "\"\""));

        // filename -> textureresource (OSLFilenameTypeSyntax)
        let td = syntax.type_system.get_type("filename");
        let mut ts = TypeSyntax::aggregate_full(
            "textureresource ",
            "textureresource (\"\", \"\")",
            "{\"\", \"\"}",
            "",
            "struct textureresource { string filename; string colorspace; };",
            vec![],
        );
        ts.osl_value_format = OslValueFormat::OslFilename;
        syntax.register_type_syntax(td, ts);

        // BSDF: closure color, default "null_closure()"
        let td = syntax.type_system.get_type("BSDF");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "BSDF",
                "null_closure()",
                "0",
                "closure color",
                "#define BSDF closure color",
            ),
        );

        // EDF: closure color, default "null_closure()"
        let td = syntax.type_system.get_type("EDF");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "EDF",
                "null_closure()",
                "0",
                "closure color",
                "#define EDF closure color",
            ),
        );

        // VDF: closure color, default "null_closure()"
        let td = syntax.type_system.get_type("VDF");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "VDF",
                "null_closure()",
                "0",
                "closure color",
                "#define VDF closure color",
            ),
        );

        // surfaceshader: struct { closure color bsdf; closure color edf; float opacity; }
        let td = syntax.type_system.get_type("surfaceshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full(
                "surfaceshader",
                "surfaceshader(null_closure(), null_closure(), 1.0)",
                "{ 0, 0, 1.0 }",
                "closure color",
                "struct surfaceshader { closure color bsdf; closure color edf; float opacity; };",
                vec![],
            ),
        );

        // volumeshader: closure color alias
        let td = syntax.type_system.get_type("volumeshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "volumeshader",
                "null_closure()",
                "0",
                "closure color",
                "#define volumeshader closure color",
            ),
        );

        // displacementshader: vector alias
        let td = syntax.type_system.get_type("displacementshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "displacementshader",
                "vector(0.0)",
                "vector(0.0)",
                "vector",
                "#define displacementshader vector",
            ),
        );

        // lightshader: closure color alias
        let td = syntax.type_system.get_type("lightshader");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "lightshader",
                "null_closure()",
                "0",
                "closure color",
                "#define lightshader closure color",
            ),
        );

        // material: closure color alias
        let td = syntax.type_system.get_type("material");
        syntax.register_type_syntax(
            td,
            TypeSyntax::scalar_full(
                "MATERIAL",
                "null_closure()",
                "0",
                "closure color",
                "#define MATERIAL closure color",
            ),
        );

        // floatarray (OslFloatArrayTypeSyntax: wraps in {})
        let td = syntax.type_system.get_type("floatarray");
        let mut ts = TypeSyntax::scalar("float", "", "");
        ts.osl_value_format = OslValueFormat::FloatArray;
        syntax.register_type_syntax(td, ts);

        // integerarray (OslIntegerArrayTypeSyntax: wraps in {})
        let td = syntax.type_system.get_type("integerarray");
        let mut ts = TypeSyntax::scalar("int", "", "");
        ts.osl_value_format = OslValueFormat::IntegerArray;
        syntax.register_type_syntax(td, ts);
    }

    fn register_reserved_words(syntax: &mut Syntax) {
        // Full OSL reserved word list from C++ OslSyntax constructor (H-OSL3 fix)
        let words = [
            // OSL types and keywords
            "and",
            "break",
            "closure",
            "color",
            "continue",
            "do",
            "else",
            "emit",
            "float",
            "for",
            "if",
            "illuminance",
            "illuminate",
            "int",
            "matrix",
            "normal",
            "not",
            "or",
            "output",
            "point",
            "public",
            "return",
            "string",
            "struct",
            "vector",
            "void",
            "while",
            "bool",
            "case",
            "catch",
            "char",
            "class",
            "const",
            "delete",
            "default",
            "double",
            "enum",
            "extern",
            "false",
            "friend",
            "goto",
            "inline",
            "long",
            "new",
            "operator",
            "private",
            "protected",
            "short",
            "signed",
            "sizeof",
            "static",
            "switch",
            "template",
            "this",
            "throw",
            "true",
            "try",
            "typedef",
            "uniform",
            "union",
            "unsigned",
            "varying",
            "virtual",
            "volatile",
            // OSL standard library function names
            "degrees",
            "radians",
            "cos",
            "sin",
            "tan",
            "acos",
            "asin",
            "atan",
            "atan2",
            "cosh",
            "sinh",
            "tanh",
            "pow",
            "log",
            "log2",
            "log10",
            "logb",
            "sqrt",
            "inversesqrt",
            "cbrt",
            "hypot",
            "abs",
            "fabs",
            "sign",
            "floor",
            "ceil",
            "round",
            "trunc",
            "fmod",
            "mod",
            "min",
            "max",
            "clamp",
            "mix",
            "select",
            "isnan",
            "isinf",
            "isfinite",
            "erf",
            "erfc",
            "cross",
            "dot",
            "length",
            "distance",
            "normalize",
            "faceforward",
            "reflect",
            "fresnel",
            "transform",
            "transformu",
            "rotate",
            "luminance",
            "blackbody",
            "wavelength_color",
            "transformc",
            "determinant",
            "transpose",
            "step",
            "smoothstep",
            "linearstep",
            "smooth_linearstep",
            "aastep",
            "hash",
            "strlen",
            "getchar",
            "startswith",
            "endswith",
            "substr",
            "stof",
            "stoi",
            "concat",
            "textureresource",
            "backfacing",
            "raytype",
            "iscameraray",
            "isdiffuseray",
            "isglossyray",
            "isshadowray",
            "getmatrix",
            "emission",
            "background",
            "diffuse",
            "oren_nayer",
            "translucent",
            "phong",
            "ward",
            "microfacet",
            "reflection",
            "transparent",
            "debug",
            "holdout",
            "subsurface",
            "sheen",
            "oren_nayar_diffuse_bsdf",
            "burley_diffuse_bsdf",
            "dielectric_bsdf",
            "conductor_bsdf",
            "generalized_schlick_bsdf",
            "translucent_bsdf",
            "transparent_bsdf",
            "subsurface_bssrdf",
            "sheen_bsdf",
            "uniform_edf",
            "anisotropic_vdf",
            "medium_vdf",
            "layer",
            "artistic_ior",
        ];
        syntax.register_reserved_words(words.iter().map(|s| s.to_string()));
    }

    fn register_invalid_tokens(syntax: &mut Syntax) {
        syntax.register_invalid_tokens([
            (" ".to_string(), "_".to_string()),
            ("-".to_string(), "_".to_string()),
            (".".to_string(), "_".to_string()),
            ("/".to_string(), "_".to_string()),
        ]);
    }

    pub fn get_syntax(&self) -> &Syntax {
        &self.syntax
    }
    pub fn get_syntax_mut(&mut self) -> &mut Syntax {
        &mut self.syntax
    }

    /// Create struct syntax with OslStruct value formatting (C++ OslSyntax::createStructSyntax).
    /// Returns OslStructTypeSyntax which recursively formats struct values.
    pub fn create_struct_syntax(
        &mut self,
        struct_type_name: &str,
        default_value: &str,
        uniform_default_value: &str,
        type_alias: &str,
        type_definition: &str,
    ) -> TypeSyntax {
        self.syntax.create_struct_syntax_with_format(
            struct_type_name,
            default_value,
            uniform_default_value,
            type_alias,
            type_definition,
            OslValueFormat::OslStruct,
        )
    }

    /// OSL uses "output" qualifier (empty for constants).
    pub fn get_output_qualifier(&self) -> &str {
        OUTPUT_QUALIFIER
    }

    pub fn get_source_file_extension(&self) -> &str {
        SOURCE_FILE_EXTENSION
    }

    /// OSL output type name (surfaceshader -> closure color for output, etc.)
    pub fn get_output_type_name(&self, type_name: &str) -> String {
        match type_name {
            "surfaceshader" => "closure color".to_string(),
            "volumeshader" => "closure color".to_string(),
            "displacementshader" => "vector".to_string(),
            "BSDF" => "output BSDF".to_string(),
            _ => self
                .syntax
                .get_type_name(&self.syntax.type_system.get_type(type_name))
                .unwrap_or("float")
                .to_string(),
        }
    }
}
