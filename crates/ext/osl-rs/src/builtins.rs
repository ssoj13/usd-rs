//! Built-in function declarations for OSL.
//!
//! Port of `builtin_func_args[]` from `typecheck.cpp` and `stdosl.h`.
//! Declares all ~250 standard OSL function overloads that the compiler
//! and runtime need to know about.

use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};
use crate::typespec::TypeSpec;

/// A built-in function signature.
#[derive(Debug, Clone)]
pub struct BuiltinFunc {
    pub name: &'static str,
    pub return_type: TypeSpec,
    pub params: Vec<BuiltinParam>,
    pub takes_derivs: bool,
}

/// A parameter of a built-in function.
#[derive(Debug, Clone)]
pub struct BuiltinParam {
    pub name: &'static str,
    pub ptype: TypeSpec,
    pub is_output: bool,
}

// Type shortcuts
fn t_f() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::FLOAT)
}
fn t_i() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::INT)
}
fn t_s() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::STRING)
}
fn t_v() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::NONE)
}
fn t_c() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::new(
        BaseType::Float,
        Aggregate::Vec3,
        VecSemantics::Color,
    ))
}
fn t_p() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::new(
        BaseType::Float,
        Aggregate::Vec3,
        VecSemantics::Point,
    ))
}
fn t_vec() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::new(
        BaseType::Float,
        Aggregate::Vec3,
        VecSemantics::Vector,
    ))
}
fn t_n() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::new(
        BaseType::Float,
        Aggregate::Vec3,
        VecSemantics::Normal,
    ))
}
fn t_m() -> TypeSpec {
    TypeSpec::from_simple(TypeDesc::MATRIX)
}

fn p(name: &'static str, t: TypeSpec) -> BuiltinParam {
    BuiltinParam {
        name,
        ptype: t,
        is_output: false,
    }
}
fn p_out(name: &'static str, t: TypeSpec) -> BuiltinParam {
    BuiltinParam {
        name,
        ptype: t,
        is_output: true,
    }
}

fn builtin(name: &'static str, ret: TypeSpec, params: Vec<BuiltinParam>) -> BuiltinFunc {
    BuiltinFunc {
        name,
        return_type: ret,
        params,
        takes_derivs: false,
    }
}
fn builtin_d(name: &'static str, ret: TypeSpec, params: Vec<BuiltinParam>) -> BuiltinFunc {
    BuiltinFunc {
        name,
        return_type: ret,
        params,
        takes_derivs: true,
    }
}

/// Get all built-in function declarations.
pub fn builtin_functions() -> Vec<BuiltinFunc> {
    vec![
        // ===== Math: scalar =====
        builtin("abs", t_f(), vec![p("x", t_f())]),
        builtin("abs", t_i(), vec![p("x", t_i())]),
        builtin("abs", t_c(), vec![p("x", t_c())]),
        builtin("abs", t_p(), vec![p("x", t_p())]),
        builtin("abs", t_vec(), vec![p("x", t_vec())]),
        builtin("abs", t_n(), vec![p("x", t_n())]),
        builtin("fabs", t_f(), vec![p("x", t_f())]),
        builtin("fabs", t_i(), vec![p("x", t_i())]),
        builtin("fabs", t_c(), vec![p("x", t_c())]),
        builtin("fabs", t_p(), vec![p("x", t_p())]),
        builtin("fabs", t_vec(), vec![p("x", t_vec())]),
        builtin("fabs", t_n(), vec![p("x", t_n())]),
        builtin("ceil", t_f(), vec![p("x", t_f())]),
        builtin("ceil", t_vec(), vec![p("x", t_vec())]),
        builtin("floor", t_f(), vec![p("x", t_f())]),
        builtin("floor", t_vec(), vec![p("x", t_vec())]),
        builtin("round", t_f(), vec![p("x", t_f())]),
        builtin("round", t_vec(), vec![p("x", t_vec())]),
        builtin("trunc", t_f(), vec![p("x", t_f())]),
        builtin("trunc", t_vec(), vec![p("x", t_vec())]),
        builtin("sign", t_f(), vec![p("x", t_f())]),
        builtin("sign", t_vec(), vec![p("x", t_vec())]),
        builtin("min", t_f(), vec![p("a", t_f()), p("b", t_f())]),
        builtin("max", t_f(), vec![p("a", t_f()), p("b", t_f())]),
        builtin("min", t_i(), vec![p("a", t_i()), p("b", t_i())]),
        builtin("max", t_i(), vec![p("a", t_i()), p("b", t_i())]),
        builtin(
            "clamp",
            t_f(),
            vec![p("x", t_f()), p("lo", t_f()), p("hi", t_f())],
        ),
        builtin(
            "clamp",
            t_i(),
            vec![p("x", t_i()), p("lo", t_i()), p("hi", t_i())],
        ),
        builtin(
            "mix",
            t_f(),
            vec![p("a", t_f()), p("b", t_f()), p("t", t_f())],
        ),
        builtin(
            "mix",
            t_c(),
            vec![p("a", t_c()), p("b", t_c()), p("t", t_f())],
        ),
        builtin(
            "mix",
            t_c(),
            vec![p("a", t_c()), p("b", t_c()), p("t", t_c())],
        ),
        builtin(
            "mix",
            t_p(),
            vec![p("a", t_p()), p("b", t_p()), p("t", t_f())],
        ),
        builtin(
            "mix",
            t_vec(),
            vec![p("a", t_vec()), p("b", t_vec()), p("t", t_f())],
        ),
        builtin(
            "mix",
            t_vec(),
            vec![p("a", t_vec()), p("b", t_vec()), p("t", t_vec())],
        ),
        builtin(
            "mix",
            t_n(),
            vec![p("a", t_n()), p("b", t_n()), p("t", t_f())],
        ),
        builtin(
            "mix",
            t_n(),
            vec![p("a", t_n()), p("b", t_n()), p("t", t_n())],
        ),
        builtin("step", t_f(), vec![p("edge", t_f()), p("x", t_f())]),
        builtin("step", t_c(), vec![p("edge", t_c()), p("x", t_c())]),
        builtin("step", t_p(), vec![p("edge", t_p()), p("x", t_p())]),
        builtin("step", t_vec(), vec![p("edge", t_vec()), p("x", t_vec())]),
        builtin("step", t_n(), vec![p("edge", t_n()), p("x", t_n())]),
        builtin(
            "smoothstep",
            t_f(),
            vec![p("edge0", t_f()), p("edge1", t_f()), p("x", t_f())],
        ),
        builtin(
            "linearstep",
            t_f(),
            vec![p("edge0", t_f()), p("edge1", t_f()), p("x", t_f())],
        ),
        builtin(
            "smooth_linearstep",
            t_f(),
            vec![p("edge0", t_f()), p("edge1", t_f()), p("x", t_f())],
        ),
        builtin("mod", t_f(), vec![p("a", t_f()), p("b", t_f())]),
        builtin("mod", t_i(), vec![p("a", t_i()), p("b", t_i())]),
        builtin("fmod", t_f(), vec![p("a", t_f()), p("b", t_f())]),
        builtin("fmod", t_vec(), vec![p("a", t_vec()), p("b", t_vec())]),
        builtin("fmod", t_n(), vec![p("a", t_n()), p("b", t_n())]),
        builtin("fmod", t_vec(), vec![p("a", t_vec()), p("b", t_f())]),
        builtin("fmod", t_n(), vec![p("a", t_n()), p("b", t_f())]),
        // select (stdosl.h BUILTIN)
        builtin(
            "select",
            t_f(),
            vec![p("x", t_f()), p("y", t_f()), p("cond", t_f())],
        ),
        builtin(
            "select",
            t_c(),
            vec![p("x", t_c()), p("y", t_c()), p("cond", t_c())],
        ),
        builtin(
            "select",
            t_vec(),
            vec![p("x", t_vec()), p("y", t_vec()), p("cond", t_vec())],
        ),
        builtin(
            "select",
            t_n(),
            vec![p("x", t_n()), p("y", t_n()), p("cond", t_n())],
        ),
        builtin(
            "select",
            t_f(),
            vec![p("x", t_f()), p("y", t_f()), p("cond", t_i())],
        ),
        builtin(
            "select",
            t_c(),
            vec![p("x", t_c()), p("y", t_c()), p("cond", t_f())],
        ),
        builtin(
            "select",
            t_c(),
            vec![p("x", t_c()), p("y", t_c()), p("cond", t_i())],
        ),
        builtin(
            "select",
            t_vec(),
            vec![p("x", t_vec()), p("y", t_vec()), p("cond", t_f())],
        ),
        builtin(
            "select",
            t_vec(),
            vec![p("x", t_vec()), p("y", t_vec()), p("cond", t_i())],
        ),
        builtin(
            "select",
            t_n(),
            vec![p("x", t_n()), p("y", t_n()), p("cond", t_f())],
        ),
        builtin(
            "select",
            t_n(),
            vec![p("x", t_n()), p("y", t_n()), p("cond", t_i())],
        ),
        // isnan/isinf/isfinite (stdosl.h + builtindecl.h)
        builtin("isnan", t_i(), vec![p("x", t_f())]),
        builtin("isinf", t_i(), vec![p("x", t_f())]),
        builtin("isfinite", t_i(), vec![p("x", t_f())]),
        // ===== Math: sqrt/pow/exp/log =====
        builtin_d("sqrt", t_f(), vec![p("x", t_f())]),
        builtin_d("inversesqrt", t_f(), vec![p("x", t_f())]),
        builtin_d("cbrt", t_f(), vec![p("x", t_f())]),
        builtin_d("pow", t_f(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("pow", t_vec(), vec![p("x", t_vec()), p("y", t_vec())]),
        builtin_d("pow", t_vec(), vec![p("x", t_vec()), p("y", t_f())]),
        builtin_d("pow", t_n(), vec![p("x", t_n()), p("y", t_n())]),
        builtin_d("pow", t_n(), vec![p("x", t_n()), p("y", t_f())]),
        builtin_d("exp", t_f(), vec![p("x", t_f())]),
        builtin_d("exp2", t_f(), vec![p("x", t_f())]),
        builtin_d("expm1", t_f(), vec![p("x", t_f())]),
        builtin_d("log", t_f(), vec![p("x", t_f())]),
        builtin_d("log2", t_f(), vec![p("x", t_f())]),
        builtin_d("log10", t_f(), vec![p("x", t_f())]),
        builtin_d("log", t_f(), vec![p("x", t_f()), p("base", t_f())]),
        builtin("logb", t_f(), vec![p("x", t_f())]),
        builtin("logb", t_vec(), vec![p("x", t_vec())]),
        builtin_d("erf", t_f(), vec![p("x", t_f())]),
        builtin_d("erfc", t_f(), vec![p("x", t_f())]),
        // ===== Trigonometry =====
        builtin_d("sin", t_f(), vec![p("x", t_f())]),
        builtin_d("cos", t_f(), vec![p("x", t_f())]),
        builtin_d("tan", t_f(), vec![p("x", t_f())]),
        builtin_d("asin", t_f(), vec![p("x", t_f())]),
        builtin_d("acos", t_f(), vec![p("x", t_f())]),
        builtin_d("atan", t_f(), vec![p("y", t_f())]),
        builtin_d("atan2", t_f(), vec![p("y", t_f()), p("x", t_f())]),
        builtin_d("sinh", t_f(), vec![p("x", t_f())]),
        builtin_d("cosh", t_f(), vec![p("x", t_f())]),
        builtin_d("tanh", t_f(), vec![p("x", t_f())]),
        // sincos: all overloads from builtin_func_args (xfff, xccc, xppp, xvvv, xnnn)
        builtin(
            "sincos",
            t_v(),
            vec![
                p("x", t_f()),
                p_out("sinval", t_f()),
                p_out("cosval", t_f()),
            ],
        ),
        builtin(
            "sincos",
            t_v(),
            vec![
                p("x", t_c()),
                p_out("sinval", t_c()),
                p_out("cosval", t_c()),
            ],
        ),
        builtin(
            "sincos",
            t_v(),
            vec![
                p("x", t_p()),
                p_out("sinval", t_p()),
                p_out("cosval", t_p()),
            ],
        ),
        builtin(
            "sincos",
            t_v(),
            vec![
                p("x", t_vec()),
                p_out("sinval", t_vec()),
                p_out("cosval", t_vec()),
            ],
        ),
        builtin(
            "sincos",
            t_v(),
            vec![
                p("x", t_n()),
                p_out("sinval", t_n()),
                p_out("cosval", t_n()),
            ],
        ),
        builtin("radians", t_f(), vec![p("deg", t_f())]),
        builtin("degrees", t_f(), vec![p("rad", t_f())]),
        // ===== Geometry =====
        builtin("dot", t_f(), vec![p("a", t_vec()), p("b", t_vec())]),
        builtin("cross", t_vec(), vec![p("a", t_vec()), p("b", t_vec())]),
        builtin_d("length", t_f(), vec![p("v", t_vec())]),
        builtin("distance", t_f(), vec![p("a", t_p()), p("b", t_p())]),
        builtin_d("normalize", t_vec(), vec![p("v", t_vec())]),
        builtin_d("normalize", t_n(), vec![p("v", t_n())]),
        builtin(
            "faceforward",
            t_vec(),
            vec![p("N", t_vec()), p("I", t_vec()), p("Nref", t_vec())],
        ),
        builtin(
            "faceforward",
            t_vec(),
            vec![p("N", t_vec()), p("I", t_vec())],
        ),
        builtin("reflect", t_vec(), vec![p("I", t_vec()), p("N", t_vec())]),
        builtin(
            "refract",
            t_vec(),
            vec![p("I", t_vec()), p("N", t_vec()), p("eta", t_f())],
        ),
        // Full 5-output Fresnel: computes kr, kt, R, T
        builtin(
            "fresnel",
            t_v(),
            vec![
                p("I", t_vec()),
                p("N", t_vec()),
                p("eta", t_f()),
                p_out("Kr", t_f()),
                p_out("Kt", t_f()),
                p_out("R", t_vec()),
                p_out("T", t_vec()),
            ],
        ),
        // Simple 3-output Fresnel: only kr
        builtin(
            "fresnel",
            t_v(),
            vec![
                p("I", t_vec()),
                p("N", t_vec()),
                p("eta", t_f()),
                p_out("Kr", t_f()),
            ],
        ),
        builtin(
            "rotate",
            t_p(),
            vec![
                p("p", t_p()),
                p("angle", t_f()),
                p("a", t_p()),
                p("b", t_p()),
            ],
        ),
        // rotate with axis only (3 args, from stdosl.h)
        builtin(
            "rotate",
            t_p(),
            vec![p("p", t_p()), p("angle", t_f()), p("axis", t_vec())],
        ),
        builtin_d("area", t_f(), vec![p("p", t_p())]),
        builtin_d("calculatenormal", t_vec(), vec![p("p", t_p())]),
        // bump and displace (from builtin_func_args)
        builtin_d("bump", t_v(), vec![p("offset", t_f())]),
        builtin_d("bump", t_v(), vec![p("space", t_s()), p("offset", t_f())]),
        builtin_d("bump", t_v(), vec![p("offset", t_vec())]),
        builtin_d("displace", t_v(), vec![p("offset", t_f())]),
        builtin_d(
            "displace",
            t_v(),
            vec![p("space", t_s()), p("offset", t_f())],
        ),
        builtin_d("displace", t_v(), vec![p("offset", t_vec())]),
        // ===== Color =====
        builtin("luminance", t_f(), vec![p("c", t_c())]),
        builtin("blackbody", t_c(), vec![p("temperature", t_f())]),
        builtin("wavelength_color", t_c(), vec![p("wavelength", t_f())]),
        builtin(
            "transformc",
            t_c(),
            vec![p("from", t_s()), p("to", t_s()), p("c", t_c())],
        ),
        builtin("transformc", t_c(), vec![p("to", t_s()), p("c", t_c())]),
        // ===== Matrix =====
        builtin("determinant", t_f(), vec![p("m", t_m())]),
        builtin("transpose", t_m(), vec![p("m", t_m())]),
        // transform: point overloads
        builtin(
            "transform",
            t_p(),
            vec![p("from", t_s()), p("to", t_s()), p("p", t_p())],
        ),
        builtin("transform", t_p(), vec![p("to", t_s()), p("p", t_p())]),
        builtin("transform", t_p(), vec![p("M", t_m()), p("p", t_p())]),
        // transform: vector overloads
        builtin(
            "transform",
            t_vec(),
            vec![p("from", t_s()), p("to", t_s()), p("v", t_vec())],
        ),
        builtin("transform", t_vec(), vec![p("M", t_m()), p("v", t_vec())]),
        // transform: normal overloads
        builtin(
            "transform",
            t_n(),
            vec![p("from", t_s()), p("to", t_s()), p("n", t_n())],
        ),
        builtin("transform", t_n(), vec![p("M", t_m()), p("n", t_n())]),
        // transformu
        builtin(
            "transformu",
            t_f(),
            vec![p("from", t_s()), p("to", t_s()), p("x", t_f())],
        ),
        builtin("transformu", t_f(), vec![p("to", t_s()), p("x", t_f())]),
        // getmatrix (stdosl.h)
        builtin(
            "getmatrix",
            t_i(),
            vec![p("from", t_s()), p("to", t_s()), p_out("M", t_m())],
        ),
        builtin(
            "getmatrix",
            t_i(),
            vec![p("from", t_s()), p_out("M", t_m())],
        ),
        // ===== String =====
        builtin("concat", t_s(), vec![p("a", t_s()), p("b", t_s())]),
        builtin("strlen", t_i(), vec![p("s", t_s())]),
        builtin("startswith", t_i(), vec![p("s", t_s()), p("prefix", t_s())]),
        builtin("endswith", t_i(), vec![p("s", t_s()), p("suffix", t_s())]),
        builtin("stoi", t_i(), vec![p("s", t_s())]),
        builtin("stof", t_f(), vec![p("s", t_s())]),
        builtin(
            "substr",
            t_s(),
            vec![p("s", t_s()), p("start", t_i()), p("len", t_i())],
        ),
        builtin("getchar", t_i(), vec![p("s", t_s()), p("index", t_i())]),
        builtin(
            "regex_search",
            t_i(),
            vec![p("subject", t_s()), p("pattern", t_s())],
        ),
        // regex_search with capture array
        builtin(
            "regex_search",
            t_i(),
            vec![
                p("subject", t_s()),
                p_out("results", t_i()),
                p("pattern", t_s()),
            ],
        ),
        builtin(
            "regex_match",
            t_i(),
            vec![p("subject", t_s()), p("pattern", t_s())],
        ),
        // regex_match with capture array
        builtin(
            "regex_match",
            t_i(),
            vec![
                p("subject", t_s()),
                p_out("results", t_i()),
                p("pattern", t_s()),
            ],
        ),
        // hash: all overloads from stdosl.h
        builtin("hash", t_i(), vec![p("s", t_s())]),
        builtin("hash", t_i(), vec![p("x", t_i())]),
        builtin("hash", t_i(), vec![p("x", t_f())]),
        builtin("hash", t_i(), vec![p("u", t_f()), p("v", t_f())]),
        builtin("hash", t_i(), vec![p("p", t_p())]),
        builtin("hash", t_i(), vec![p("p", t_p()), p("t", t_f())]),
        builtin("format", t_s(), vec![p("fmt", t_s())]), // variadic
        // split (from builtin_func_args)
        builtin(
            "split",
            t_i(),
            vec![
                p("str", t_s()),
                p_out("results", t_s()),
                p("sep", t_s()),
                p("maxsplit", t_i()),
            ],
        ),
        builtin(
            "split",
            t_i(),
            vec![p("str", t_s()), p_out("results", t_s()), p("sep", t_s())],
        ),
        builtin(
            "split",
            t_i(),
            vec![p("str", t_s()), p_out("results", t_s())],
        ),
        // ===== Noise =====
        // noise: float return, all input combos (NOISE_ARGS)
        builtin_d("noise", t_f(), vec![p("x", t_f())]),
        builtin_d("noise", t_f(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("noise", t_f(), vec![p("p", t_p())]),
        builtin_d("noise", t_f(), vec![p("p", t_p()), p("t", t_f())]),
        // noise: color return
        builtin_d("noise", t_c(), vec![p("x", t_f())]),
        builtin_d("noise", t_c(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("noise", t_c(), vec![p("p", t_p())]),
        builtin_d("noise", t_c(), vec![p("p", t_p()), p("t", t_f())]),
        // noise: vector return
        builtin_d("noise", t_vec(), vec![p("x", t_f())]),
        builtin_d("noise", t_vec(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("noise", t_vec(), vec![p("p", t_p())]),
        builtin_d("noise", t_vec(), vec![p("p", t_p()), p("t", t_f())]),
        // noise: generic (with string name) - GNOISE_ARGS
        builtin_d("noise", t_f(), vec![p("name", t_s()), p("x", t_f())]),
        builtin_d(
            "noise",
            t_f(),
            vec![p("name", t_s()), p("x", t_f()), p("y", t_f())],
        ),
        builtin_d("noise", t_f(), vec![p("name", t_s()), p("p", t_p())]),
        builtin_d(
            "noise",
            t_f(),
            vec![p("name", t_s()), p("p", t_p()), p("t", t_f())],
        ),
        builtin_d("noise", t_c(), vec![p("name", t_s()), p("x", t_f())]),
        builtin_d(
            "noise",
            t_c(),
            vec![p("name", t_s()), p("x", t_f()), p("y", t_f())],
        ),
        builtin_d("noise", t_c(), vec![p("name", t_s()), p("p", t_p())]),
        builtin_d(
            "noise",
            t_c(),
            vec![p("name", t_s()), p("p", t_p()), p("t", t_f())],
        ),
        builtin_d("noise", t_vec(), vec![p("name", t_s()), p("x", t_f())]),
        builtin_d(
            "noise",
            t_vec(),
            vec![p("name", t_s()), p("x", t_f()), p("y", t_f())],
        ),
        builtin_d("noise", t_vec(), vec![p("name", t_s()), p("p", t_p())]),
        builtin_d(
            "noise",
            t_vec(),
            vec![p("name", t_s()), p("p", t_p()), p("t", t_f())],
        ),
        // snoise: full NOISE_ARGS
        builtin_d("snoise", t_f(), vec![p("x", t_f())]),
        builtin_d("snoise", t_f(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("snoise", t_f(), vec![p("p", t_p())]),
        builtin_d("snoise", t_f(), vec![p("p", t_p()), p("t", t_f())]),
        builtin_d("snoise", t_c(), vec![p("x", t_f())]),
        builtin_d("snoise", t_c(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("snoise", t_c(), vec![p("p", t_p())]),
        builtin_d("snoise", t_c(), vec![p("p", t_p()), p("t", t_f())]),
        builtin_d("snoise", t_vec(), vec![p("x", t_f())]),
        builtin_d("snoise", t_vec(), vec![p("x", t_f()), p("y", t_f())]),
        builtin_d("snoise", t_vec(), vec![p("p", t_p())]),
        builtin_d("snoise", t_vec(), vec![p("p", t_p()), p("t", t_f())]),
        // pnoise: full PNOISE_ARGS + PGNOISE_ARGS
        builtin_d("pnoise", t_f(), vec![p("x", t_f()), p("px", t_f())]),
        builtin_d(
            "pnoise",
            t_f(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin_d("pnoise", t_f(), vec![p("p", t_p()), p("period", t_p())]),
        builtin_d(
            "pnoise",
            t_f(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        builtin_d("pnoise", t_c(), vec![p("x", t_f()), p("px", t_f())]),
        builtin_d(
            "pnoise",
            t_c(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin_d("pnoise", t_c(), vec![p("p", t_p()), p("period", t_p())]),
        builtin_d(
            "pnoise",
            t_c(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        builtin_d("pnoise", t_vec(), vec![p("x", t_f()), p("px", t_f())]),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin_d("pnoise", t_vec(), vec![p("p", t_p()), p("period", t_p())]),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        // pnoise: generic with string name (PGNOISE_ARGS)
        builtin_d(
            "pnoise",
            t_f(),
            vec![p("name", t_s()), p("x", t_f()), p("px", t_f())],
        ),
        builtin_d(
            "pnoise",
            t_f(),
            vec![
                p("name", t_s()),
                p("x", t_f()),
                p("y", t_f()),
                p("px", t_f()),
                p("py", t_f()),
            ],
        ),
        builtin_d(
            "pnoise",
            t_f(),
            vec![p("name", t_s()), p("p", t_p()), p("period", t_p())],
        ),
        builtin_d(
            "pnoise",
            t_f(),
            vec![
                p("name", t_s()),
                p("p", t_p()),
                p("t", t_f()),
                p("pp", t_p()),
                p("pt", t_f()),
            ],
        ),
        builtin_d(
            "pnoise",
            t_c(),
            vec![p("name", t_s()), p("x", t_f()), p("px", t_f())],
        ),
        builtin_d(
            "pnoise",
            t_c(),
            vec![
                p("name", t_s()),
                p("x", t_f()),
                p("y", t_f()),
                p("px", t_f()),
                p("py", t_f()),
            ],
        ),
        builtin_d(
            "pnoise",
            t_c(),
            vec![p("name", t_s()), p("p", t_p()), p("period", t_p())],
        ),
        builtin_d(
            "pnoise",
            t_c(),
            vec![
                p("name", t_s()),
                p("p", t_p()),
                p("t", t_f()),
                p("pp", t_p()),
                p("pt", t_f()),
            ],
        ),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![p("name", t_s()), p("x", t_f()), p("px", t_f())],
        ),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![
                p("name", t_s()),
                p("x", t_f()),
                p("y", t_f()),
                p("px", t_f()),
                p("py", t_f()),
            ],
        ),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![p("name", t_s()), p("p", t_p()), p("period", t_p())],
        ),
        builtin_d(
            "pnoise",
            t_vec(),
            vec![
                p("name", t_s()),
                p("p", t_p()),
                p("t", t_f()),
                p("pp", t_p()),
                p("pt", t_f()),
            ],
        ),
        // psnoise: full PNOISE_ARGS (periodic signed noise)
        builtin("psnoise", t_f(), vec![p("x", t_f()), p("px", t_f())]),
        builtin(
            "psnoise",
            t_f(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin("psnoise", t_f(), vec![p("p", t_p()), p("period", t_p())]),
        builtin(
            "psnoise",
            t_f(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        builtin("psnoise", t_c(), vec![p("x", t_f()), p("px", t_f())]),
        builtin(
            "psnoise",
            t_c(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin("psnoise", t_c(), vec![p("p", t_p()), p("period", t_p())]),
        builtin(
            "psnoise",
            t_c(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        builtin("psnoise", t_vec(), vec![p("x", t_f()), p("px", t_f())]),
        builtin(
            "psnoise",
            t_vec(),
            vec![p("x", t_f()), p("y", t_f()), p("px", t_f()), p("py", t_f())],
        ),
        builtin("psnoise", t_vec(), vec![p("p", t_p()), p("period", t_p())]),
        builtin(
            "psnoise",
            t_vec(),
            vec![p("p", t_p()), p("t", t_f()), p("pp", t_p()), p("pt", t_f())],
        ),
        // cellnoise: full NOISE_ARGS
        builtin("cellnoise", t_f(), vec![p("x", t_f())]),
        builtin("cellnoise", t_f(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("cellnoise", t_f(), vec![p("p", t_p())]),
        builtin("cellnoise", t_f(), vec![p("p", t_p()), p("t", t_f())]),
        builtin("cellnoise", t_c(), vec![p("x", t_f())]),
        builtin("cellnoise", t_c(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("cellnoise", t_c(), vec![p("p", t_p())]),
        builtin("cellnoise", t_c(), vec![p("p", t_p()), p("t", t_f())]),
        builtin("cellnoise", t_vec(), vec![p("x", t_f())]),
        builtin("cellnoise", t_vec(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("cellnoise", t_vec(), vec![p("p", t_p())]),
        builtin("cellnoise", t_vec(), vec![p("p", t_p()), p("t", t_f())]),
        // hashnoise: full NOISE_ARGS
        builtin("hashnoise", t_f(), vec![p("x", t_f())]),
        builtin("hashnoise", t_f(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("hashnoise", t_f(), vec![p("p", t_p())]),
        builtin("hashnoise", t_f(), vec![p("p", t_p()), p("t", t_f())]),
        builtin("hashnoise", t_c(), vec![p("x", t_f())]),
        builtin("hashnoise", t_c(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("hashnoise", t_c(), vec![p("p", t_p())]),
        builtin("hashnoise", t_c(), vec![p("p", t_p()), p("t", t_f())]),
        builtin("hashnoise", t_vec(), vec![p("x", t_f())]),
        builtin("hashnoise", t_vec(), vec![p("x", t_f()), p("y", t_f())]),
        builtin("hashnoise", t_vec(), vec![p("p", t_p())]),
        builtin("hashnoise", t_vec(), vec![p("p", t_p()), p("t", t_f())]),
        // random (from builtin_func_args)
        builtin("random", t_f(), vec![]),
        builtin("random", t_c(), vec![]),
        builtin("random", t_p(), vec![]),
        builtin("random", t_vec(), vec![]),
        builtin("random", t_n(), vec![]),
        // ===== Texture =====
        // texture: float return (2-coord), color return (2-coord), vector return (2-coord)
        builtin(
            "texture",
            t_f(),
            vec![p("filename", t_s()), p("s", t_f()), p("t", t_f())],
        ),
        builtin(
            "texture",
            t_c(),
            vec![p("filename", t_s()), p("s", t_f()), p("t", t_f())],
        ),
        builtin(
            "texture",
            t_vec(),
            vec![p("filename", t_s()), p("s", t_f()), p("t", t_f())],
        ),
        // texture: with derivatives (8-coord)
        builtin(
            "texture",
            t_f(),
            vec![
                p("filename", t_s()),
                p("s", t_f()),
                p("t", t_f()),
                p("dsdx", t_f()),
                p("dtdx", t_f()),
                p("dsdy", t_f()),
                p("dtdy", t_f()),
            ],
        ),
        builtin(
            "texture",
            t_c(),
            vec![
                p("filename", t_s()),
                p("s", t_f()),
                p("t", t_f()),
                p("dsdx", t_f()),
                p("dtdx", t_f()),
                p("dsdy", t_f()),
                p("dtdy", t_f()),
            ],
        ),
        builtin(
            "texture",
            t_vec(),
            vec![
                p("filename", t_s()),
                p("s", t_f()),
                p("t", t_f()),
                p("dsdx", t_f()),
                p("dtdx", t_f()),
                p("dsdy", t_f()),
                p("dtdy", t_f()),
            ],
        ),
        // texture3d: point, point+derivatives
        builtin(
            "texture3d",
            t_f(),
            vec![p("filename", t_s()), p("p", t_p())],
        ),
        builtin(
            "texture3d",
            t_c(),
            vec![p("filename", t_s()), p("p", t_p())],
        ),
        builtin(
            "texture3d",
            t_vec(),
            vec![p("filename", t_s()), p("p", t_p())],
        ),
        builtin(
            "texture3d",
            t_f(),
            vec![
                p("filename", t_s()),
                p("p", t_p()),
                p("dpdx", t_vec()),
                p("dpdy", t_vec()),
                p("dpdz", t_vec()),
            ],
        ),
        builtin(
            "texture3d",
            t_c(),
            vec![
                p("filename", t_s()),
                p("p", t_p()),
                p("dpdx", t_vec()),
                p("dpdy", t_vec()),
                p("dpdz", t_vec()),
            ],
        ),
        builtin(
            "texture3d",
            t_vec(),
            vec![
                p("filename", t_s()),
                p("p", t_p()),
                p("dpdx", t_vec()),
                p("dpdy", t_vec()),
                p("dpdz", t_vec()),
            ],
        ),
        // environment: direction, direction+derivatives
        builtin(
            "environment",
            t_f(),
            vec![p("filename", t_s()), p("R", t_vec())],
        ),
        builtin(
            "environment",
            t_c(),
            vec![p("filename", t_s()), p("R", t_vec())],
        ),
        builtin(
            "environment",
            t_vec(),
            vec![p("filename", t_s()), p("R", t_vec())],
        ),
        builtin(
            "environment",
            t_f(),
            vec![
                p("filename", t_s()),
                p("R", t_vec()),
                p("dRdx", t_vec()),
                p("dRdy", t_vec()),
            ],
        ),
        builtin(
            "environment",
            t_c(),
            vec![
                p("filename", t_s()),
                p("R", t_vec()),
                p("dRdx", t_vec()),
                p("dRdy", t_vec()),
            ],
        ),
        builtin(
            "environment",
            t_vec(),
            vec![
                p("filename", t_s()),
                p("R", t_vec()),
                p("dRdx", t_vec()),
                p("dRdy", t_vec()),
            ],
        ),
        // gettextureinfo
        builtin(
            "gettextureinfo",
            t_i(),
            vec![
                p("filename", t_s()),
                p("dataname", t_s()),
                p_out("data", t_f()),
            ],
        ),
        // gettextureinfo with st coords
        builtin(
            "gettextureinfo",
            t_i(),
            vec![
                p("filename", t_s()),
                p("s", t_f()),
                p("t", t_f()),
                p("dataname", t_s()),
                p_out("data", t_f()),
            ],
        ),
        // ===== Closures =====
        builtin("diffuse", t_v(), vec![p("N", t_n())]),
        builtin("oren_nayar", t_v(), vec![p("N", t_n()), p("sigma", t_f())]),
        builtin("phong", t_v(), vec![p("N", t_n()), p("exponent", t_f())]),
        builtin(
            "ward",
            t_v(),
            vec![
                p("N", t_n()),
                p("T", t_vec()),
                p("ax", t_f()),
                p("ay", t_f()),
            ],
        ),
        builtin(
            "microfacet",
            t_v(),
            vec![
                p("dist", t_s()),
                p("N", t_n()),
                p("alpha", t_f()),
                p("eta", t_f()),
                p("refract", t_i()),
            ],
        ),
        builtin("reflection", t_v(), vec![p("N", t_n())]),
        builtin("refraction", t_v(), vec![p("N", t_n()), p("eta", t_f())]),
        builtin("transparent", t_v(), vec![]),
        builtin("translucent", t_v(), vec![p("N", t_n())]),
        builtin("emission", t_v(), vec![]),
        builtin("background", t_v(), vec![]),
        builtin("holdout", t_v(), vec![]),
        builtin("debug", t_v(), vec![p("tag", t_s())]),
        // ===== Output / diagnostics =====
        builtin("printf", t_v(), vec![p("fmt", t_s())]),
        builtin(
            "fprintf",
            t_v(),
            vec![p("filename", t_s()), p("fmt", t_s())],
        ),
        builtin("error", t_v(), vec![p("fmt", t_s())]),
        builtin("warning", t_v(), vec![p("fmt", t_s())]),
        builtin("exit", t_v(), vec![]),
        // ===== Attributes / messages =====
        // getattribute: 2-arg (name, out val)
        builtin(
            "getattribute",
            t_i(),
            vec![p("name", t_s()), p_out("val", t_f())],
        ),
        // getattribute: 3-arg (object, name, out val)
        builtin(
            "getattribute",
            t_i(),
            vec![p("obj", t_s()), p("name", t_s()), p_out("val", t_f())],
        ),
        // getattribute: with index
        builtin(
            "getattribute",
            t_i(),
            vec![p("name", t_s()), p("index", t_i()), p_out("val", t_f())],
        ),
        builtin(
            "getattribute",
            t_i(),
            vec![
                p("obj", t_s()),
                p("name", t_s()),
                p("index", t_i()),
                p_out("val", t_f()),
            ],
        ),
        // setmessage
        builtin(
            "setmessage",
            t_v(),
            vec![p("name", t_s()), p("value", t_f())],
        ),
        // getmessage: 2-arg (name, out val) and 3-arg (source, name, out val)
        builtin(
            "getmessage",
            t_i(),
            vec![p("name", t_s()), p_out("val", t_f())],
        ),
        builtin(
            "getmessage",
            t_i(),
            vec![p("source", t_s()), p("name", t_s()), p_out("val", t_f())],
        ),
        // ===== Queries =====
        builtin("isconnected", t_i(), vec![p("param", t_f())]),
        builtin("isconstant", t_i(), vec![p("expr", t_f())]),
        builtin("arraylength", t_i(), vec![p("arr", t_f())]),
        builtin("raytype", t_i(), vec![p("name", t_s())]),
        builtin("backfacing", t_i(), vec![]),
        builtin("surfacearea", t_f(), vec![]),
        // trace
        builtin_d("trace", t_i(), vec![p("P", t_p()), p("R", t_vec())]),
        // ===== Point cloud =====
        builtin(
            "pointcloud_search",
            t_i(),
            vec![
                p("filename", t_s()),
                p("center", t_p()),
                p("radius", t_f()),
                p("maxpoints", t_i()),
            ],
        ),
        builtin(
            "pointcloud_get",
            t_i(),
            vec![
                p("filename", t_s()),
                p("indices", TypeSpec::new_array(TypeDesc::INT, -1)),
                p("count", t_i()),
                p("attr", t_s()),
                p_out("data", TypeSpec::new_array(TypeDesc::FLOAT, -1)),
            ],
        ),
        builtin(
            "pointcloud_get",
            t_i(),
            vec![
                p("filename", t_s()),
                p("indices", TypeSpec::new_array(TypeDesc::INT, -1)),
                p("count", t_i()),
                p("attr", t_s()),
                p_out(
                    "data",
                    TypeSpec::new_array(
                        TypeDesc::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Color),
                        -1,
                    ),
                ),
            ],
        ),
        builtin(
            "pointcloud_get",
            t_i(),
            vec![
                p("filename", t_s()),
                p("indices", TypeSpec::new_array(TypeDesc::INT, -1)),
                p("count", t_i()),
                p("attr", t_s()),
                p_out("data", TypeSpec::new_array(TypeDesc::POINT, -1)),
            ],
        ),
        builtin(
            "pointcloud_get",
            t_i(),
            vec![
                p("filename", t_s()),
                p("indices", TypeSpec::new_array(TypeDesc::INT, -1)),
                p("count", t_i()),
                p("attr", t_s()),
                p_out("data", TypeSpec::new_array(TypeDesc::VECTOR, -1)),
            ],
        ),
        builtin(
            "pointcloud_write",
            t_i(),
            vec![p("filename", t_s()), p("P", t_p())],
        ), // varargs: "attrname", value, ...
        // ===== Spline =====
        // spline: float, color, point, vector, normal knot arrays
        builtin("spline", t_f(), vec![p("basis", t_s()), p("x", t_f())]),
        builtin("spline", t_c(), vec![p("basis", t_s()), p("x", t_f())]),
        builtin("spline", t_p(), vec![p("basis", t_s()), p("x", t_f())]),
        builtin("spline", t_vec(), vec![p("basis", t_s()), p("x", t_f())]),
        builtin("spline", t_n(), vec![p("basis", t_s()), p("x", t_f())]),
        // spline with int-indexed knot arrays
        builtin(
            "spline",
            t_f(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        builtin(
            "spline",
            t_c(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        builtin(
            "spline",
            t_p(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        builtin(
            "spline",
            t_vec(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        builtin(
            "spline",
            t_n(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        // splineinverse
        builtin(
            "splineinverse",
            t_f(),
            vec![p("basis", t_s()), p("x", t_f())],
        ),
        builtin(
            "splineinverse",
            t_f(),
            vec![p("basis", t_s()), p("x", t_f()), p("nknots", t_i())],
        ),
        // ===== Derivative =====
        // Dx: float, point, vector, normal, color
        builtin("Dx", t_f(), vec![p("x", t_f())]),
        builtin("Dx", t_vec(), vec![p("x", t_p())]),
        builtin("Dx", t_vec(), vec![p("x", t_vec())]),
        builtin("Dx", t_vec(), vec![p("x", t_n())]),
        builtin("Dx", t_c(), vec![p("x", t_c())]),
        // Dy: float, point, vector, normal, color
        builtin("Dy", t_f(), vec![p("x", t_f())]),
        builtin("Dy", t_vec(), vec![p("x", t_p())]),
        builtin("Dy", t_vec(), vec![p("x", t_vec())]),
        builtin("Dy", t_vec(), vec![p("x", t_n())]),
        builtin("Dy", t_c(), vec![p("x", t_c())]),
        // Dz: float, point, vector, normal, color
        builtin("Dz", t_f(), vec![p("x", t_f())]),
        builtin("Dz", t_vec(), vec![p("x", t_p())]),
        builtin("Dz", t_vec(), vec![p("x", t_vec())]),
        builtin("Dz", t_vec(), vec![p("x", t_n())]),
        builtin("Dz", t_c(), vec![p("x", t_c())]),
        // filterwidth: float, point, vector
        builtin("filterwidth", t_f(), vec![p("x", t_f())]),
        builtin("filterwidth", t_vec(), vec![p("x", t_p())]),
        builtin("filterwidth", t_vec(), vec![p("x", t_vec())]),
        // ===== Dict =====
        builtin(
            "dict_find",
            t_i(),
            vec![p("dictionary", t_s()), p("query", t_s())],
        ),
        builtin(
            "dict_find",
            t_i(),
            vec![p("nodeID", t_i()), p("query", t_s())],
        ),
        builtin("dict_next", t_i(), vec![p("nodeID", t_i())]),
        builtin(
            "dict_value",
            t_i(),
            vec![
                p("nodeID", t_i()),
                p("attribname", t_s()),
                p_out("value", t_f()),
            ],
        ),
    ]
}

static BUILTINS_CACHE: std::sync::OnceLock<Vec<BuiltinFunc>> = std::sync::OnceLock::new();

/// Get the static built-in function table.
pub fn get_builtins() -> &'static [BuiltinFunc] {
    BUILTINS_CACHE.get_or_init(builtin_functions)
}

/// Lookup a built-in function by name (returns all overloads).
pub fn lookup_builtin(name: &str) -> Vec<&'static BuiltinFunc> {
    get_builtins().iter().filter(|f| f.name == name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_count() {
        let funcs = builtin_functions();
        assert!(
            funcs.len() > 200,
            "Expected 200+ builtins, got {}",
            funcs.len()
        );
    }

    #[test]
    fn test_lookup() {
        let builtins = get_builtins();
        let sin_funcs: Vec<_> = builtins.iter().filter(|f| f.name == "sin").collect();
        assert!(!sin_funcs.is_empty());
        assert_eq!(
            sin_funcs[0].return_type,
            TypeSpec::from_simple(TypeDesc::FLOAT)
        );
    }

    #[test]
    fn test_overloads() {
        let builtins = get_builtins();
        let noise_funcs: Vec<_> = builtins.iter().filter(|f| f.name == "noise").collect();
        assert!(
            noise_funcs.len() >= 12,
            "Expected 12+ noise overloads, got {}",
            noise_funcs.len()
        );
    }

    #[test]
    fn test_closures() {
        let builtins = get_builtins();
        let diff: Vec<_> = builtins.iter().filter(|f| f.name == "diffuse").collect();
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_new_functions() {
        let builtins = get_builtins();
        // exit
        assert!(builtins.iter().any(|f| f.name == "exit"));
        // random
        let random: Vec<_> = builtins.iter().filter(|f| f.name == "random").collect();
        assert_eq!(random.len(), 5, "Expected 5 random overloads");
        // isnan/isinf/isfinite
        assert!(builtins.iter().any(|f| f.name == "isnan"));
        assert!(builtins.iter().any(|f| f.name == "isinf"));
        assert!(builtins.iter().any(|f| f.name == "isfinite"));
        // split
        let split: Vec<_> = builtins.iter().filter(|f| f.name == "split").collect();
        assert_eq!(split.len(), 3, "Expected 3 split overloads");
        // logb
        let logb: Vec<_> = builtins.iter().filter(|f| f.name == "logb").collect();
        assert_eq!(logb.len(), 2, "Expected 2 logb overloads");
        // select
        let sel: Vec<_> = builtins.iter().filter(|f| f.name == "select").collect();
        assert!(sel.len() >= 8, "Expected 8+ select overloads");
        // psnoise
        let psn: Vec<_> = builtins.iter().filter(|f| f.name == "psnoise").collect();
        assert!(psn.len() >= 12, "Expected 12 psnoise overloads");
        // bump, displace
        assert!(builtins.iter().any(|f| f.name == "bump"));
        assert!(builtins.iter().any(|f| f.name == "displace"));
        // getmatrix
        assert!(builtins.iter().any(|f| f.name == "getmatrix"));
        // fabs
        let fabs: Vec<_> = builtins.iter().filter(|f| f.name == "fabs").collect();
        assert!(fabs.len() >= 2, "Expected 2+ fabs overloads");
    }

    #[test]
    fn test_expanded_overloads() {
        let builtins = get_builtins();
        // sincos should have 5 overloads (f, c, p, v, n)
        let sc: Vec<_> = builtins.iter().filter(|f| f.name == "sincos").collect();
        assert_eq!(sc.len(), 5, "Expected 5 sincos overloads");
        // cellnoise: 12 overloads (NOISE_ARGS)
        let cn: Vec<_> = builtins.iter().filter(|f| f.name == "cellnoise").collect();
        assert_eq!(cn.len(), 12, "Expected 12 cellnoise overloads");
        // hashnoise: 12
        let hn: Vec<_> = builtins.iter().filter(|f| f.name == "hashnoise").collect();
        assert_eq!(hn.len(), 12, "Expected 12 hashnoise overloads");
        // snoise: 12
        let sn: Vec<_> = builtins.iter().filter(|f| f.name == "snoise").collect();
        assert_eq!(sn.len(), 12, "Expected 12 snoise overloads");
        // Dx/Dy/Dz: 5 overloads each
        let dx: Vec<_> = builtins.iter().filter(|f| f.name == "Dx").collect();
        assert_eq!(dx.len(), 5, "Expected 5 Dx overloads");
        // pnoise: 12 base + 12 generic = 24
        let pn: Vec<_> = builtins.iter().filter(|f| f.name == "pnoise").collect();
        assert_eq!(pn.len(), 24, "Expected 24 pnoise overloads");
        // hash: 6 overloads
        let h: Vec<_> = builtins.iter().filter(|f| f.name == "hash").collect();
        assert_eq!(h.len(), 6, "Expected 6 hash overloads");
    }
}
