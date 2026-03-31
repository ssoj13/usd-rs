//! OslNetworkSyntax — OSL network param format (space-separated values).
//! Ref: MaterialXGenOsl/OslNetworkSyntax.cpp.
//! CRITICAL: registers OWN types different from OslSyntax (COLOR4->"color", FILENAME->"string").

use crate::gen_shader::{OslValueFormat, Syntax, TypeSyntax, TypeSystem};

const VECTOR_MEMBERS: &[&str] = &["[0]", "[1]", "[2]"];
const VECTOR2_MEMBERS: &[&str] = &[".x", ".y"];
const VECTOR4_MEMBERS: &[&str] = &[".x", ".y", ".z", ".w"];

/// OSL network syntax — own type registrations for ShaderGroup param format.
pub struct OslNetworkSyntax {
    pub syntax: Syntax,
}

impl OslNetworkSyntax {
    pub fn new(type_system: TypeSystem) -> Self {
        let mut syntax = Syntax::new(type_system);
        Self::register_reserved_words(&mut syntax);
        Self::register_invalid_tokens(&mut syntax);
        Self::register_network_types(&mut syntax);
        Self { syntax }
    }

    pub fn create(type_system: TypeSystem) -> Self {
        Self::new(type_system)
    }

    pub fn get_syntax(&self) -> &Syntax {
        &self.syntax
    }

    fn register_network_types(syntax: &mut Syntax) {
        let td = syntax.type_system.get_type("float");
        syntax.register_type_syntax(td, TypeSyntax::scalar("float", "0.0", "0.0"));

        // floatarray (OslFloatArrayTypeSyntax)
        let td = syntax.type_system.get_type("floatarray");
        let mut ts = TypeSyntax::scalar("float", "", "");
        ts.osl_value_format = OslValueFormat::FloatArray;
        syntax.register_type_syntax(td, ts);

        let td = syntax.type_system.get_type("integer");
        syntax.register_type_syntax(td, TypeSyntax::scalar("int", "0", "0"));

        // integerarray (OslIntegerArrayTypeSyntax)
        let td = syntax.type_system.get_type("integerarray");
        let mut ts = TypeSyntax::scalar("int", "", "");
        ts.osl_value_format = OslValueFormat::IntegerArray;
        syntax.register_type_syntax(td, ts);

        // boolean -> int (OslBooleanTypeSyntax)
        let td = syntax.type_system.get_type("boolean");
        let mut ts =
            TypeSyntax::scalar_full("int", "0", "0", "", "#define true 1\n#define false 0");
        ts.osl_value_format = OslValueFormat::Boolean;
        syntax.register_type_syntax(td, ts);

        // color3: OslNetworkVectorTypeSyntax<Color3> - space-separated values
        let td = syntax.type_system.get_type("color3");
        let mut ts = TypeSyntax::aggregate_full(
            "color",
            "color(0.0)",
            "color(0.0)",
            "",
            "",
            VECTOR_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::NetworkVector;
        syntax.register_type_syntax(td, ts);

        // color4: OslNetworkVectorTypeSyntax<Color4> - space-separated, type name "color" (NOT "color4")
        let td = syntax.type_system.get_type("color4");
        let mut ts = TypeSyntax::aggregate_full(
            "color",
            "color(0.0)",
            "color(0.0)",
            "",
            "",
            VECTOR4_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::NetworkVector;
        syntax.register_type_syntax(td, ts);

        // vector2: OslNetworkVectorTypeSyntax<Vector2>
        let td = syntax.type_system.get_type("vector2");
        let mut ts = TypeSyntax::aggregate_full(
            "vector2",
            "vector2(0.0, 0.0)",
            "{0.0, 0.0}",
            "",
            "",
            VECTOR2_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::NetworkVector;
        syntax.register_type_syntax(td, ts);

        // vector3: OslNetworkVectorTypeSyntax<Vector3>
        let td = syntax.type_system.get_type("vector3");
        let mut ts = TypeSyntax::aggregate_full(
            "vector",
            "vector(0.0)",
            "vector(0.0)",
            "",
            "",
            VECTOR_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::NetworkVector;
        syntax.register_type_syntax(td, ts);

        // vector4: OslNetworkVectorTypeSyntax<Vector4>
        let td = syntax.type_system.get_type("vector4");
        let mut ts = TypeSyntax::aggregate_full(
            "vector4",
            "vector4(0.0, 0.0, 0.0, 0.0)",
            "{0.0, 0.0, 0.0, 0.0}",
            "",
            "",
            VECTOR4_MEMBERS.iter().map(|s| s.to_string()).collect(),
        );
        ts.osl_value_format = OslValueFormat::NetworkVector;
        syntax.register_type_syntax(td, ts);

        // matrix33: OSLMatrix3TypeSyntax (expand 3x3 to 4x4)
        let td = syntax.type_system.get_type("matrix33");
        let mut ts =
            TypeSyntax::aggregate_full("matrix", "matrix(1.0)", "matrix(1.0)", "", "", vec![]);
        ts.osl_value_format = OslValueFormat::OslMatrix3;
        syntax.register_type_syntax(td, ts);

        let td = syntax.type_system.get_type("matrix44");
        syntax.register_type_syntax(
            td,
            TypeSyntax::aggregate_full("matrix", "matrix(1.0)", "matrix(1.0)", "", "", vec![]),
        );

        let td = syntax.type_system.get_type("string");
        syntax.register_type_syntax(td, TypeSyntax::scalar("string", "\"\"", "\"\""));

        // filename -> "string" (NOT "textureresource"!) with NetworkFilename behavior
        let td = syntax.type_system.get_type("filename");
        let mut ts = TypeSyntax::aggregate_full(
            "string",
            "textureresource (\"\", \"\")",
            "(\"\", \"\")",
            "",
            "struct textureresource { string filename; string colorspace; };",
            vec![],
        );
        ts.osl_value_format = OslValueFormat::NetworkFilename;
        syntax.register_type_syntax(td, ts);

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
    }

    fn register_reserved_words(syntax: &mut Syntax) {
        let words = [
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
}
