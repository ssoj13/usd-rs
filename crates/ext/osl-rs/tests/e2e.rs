//! End-to-end integration tests for osl-rs.
//!
//! Each test exercises the full pipeline:
//! .osl source → preprocess → parse → typecheck → codegen → optimize → interpret → verify

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use osl_rs::codegen;
use osl_rs::interp::{Interpreter, run_shader};
use osl_rs::math::Vec3;
use osl_rs::optimizer;
use osl_rs::oslc::{CompilerOptions, compile_string};
use osl_rs::parser;
use osl_rs::preprocess;
use osl_rs::renderer::{BasicRenderer, NullRenderer};
use osl_rs::shaderglobals::ShaderGlobals;
use osl_rs::shadingsys::ShadingSystem;

fn find_ref_oslc() -> Option<String> {
    if let Ok(path) = std::env::var("OSL_REF_OSLC") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    if let Ok(path) = std::env::var("OSL_OSLC") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    match Command::new("oslc").arg("--version").output() {
        Ok(_) => Some("oslc".to_string()),
        Err(_) => None,
    }
}

fn canon_escape_oso(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn canon_trim_trailing_zeros(s: &str) -> String {
    if let Some(dot) = s.find('.') {
        let bytes = s.as_bytes();
        let mut end = s.len();
        while end > dot + 1 && bytes[end - 1] == b'0' {
            end -= 1;
        }
        if end == dot + 1 {
            end -= 1;
        }
        let mut out = s[..end].to_string();
        if out == "-0" {
            out = "0".to_string();
        }
        return out;
    }
    s.to_string()
}

fn canon_float_g9(v: f32) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = (v as f64).abs();
    let exp = abs.log10().floor() as i32;
    if exp < -4 || exp >= 9 {
        let s = format!("{:.9e}", v);
        if let Some(pos) = s.find('e') {
            let mant = canon_trim_trailing_zeros(&s[..pos]);
            let exp_part = &s[pos + 1..];
            let mut out = format!("{mant}e{exp_part}");
            if out.starts_with("-0e") {
                out = out.replacen("-0e", "0e", 1);
            }
            out
        } else {
            s
        }
    } else {
        let prec = (9 - (exp + 1)).max(0) as usize;
        canon_trim_trailing_zeros(&format!("{:.*}", prec, v))
    }
}

fn canonicalize_oso(oso: &osl_rs::oso::OsoFile) -> String {
    use osl_rs::symbol::SymType;

    let mut out = String::new();
    out.push_str(&format!(
        "OpenShadingLanguage {}.{:02}\n",
        oso.version.0, oso.version.1
    ));
    out.push_str(&format!("{} {}\n", oso.shader_type.name(), oso.shader_name));

    let mut sym_by_name: HashMap<String, &osl_rs::oso::OsoSymbol> = HashMap::new();
    for sym in &oso.symbols {
        sym_by_name.insert(sym.name.clone(), sym);
    }

    let mut used_names: HashMap<String, bool> = HashMap::new();
    for instr in &oso.instructions {
        for arg in &instr.args {
            used_names.insert(arg.clone(), true);
        }
    }

    let mut key_by_name: HashMap<String, String> = HashMap::new();
    let mut keys: Vec<String> = Vec::new();
    for sym in &oso.symbols {
        let include = used_names.contains_key(&sym.name)
            || matches!(sym.symtype, SymType::Param | SymType::OutputParam);
        if !include {
            continue;
        }
        let mut key = format!("{}|{}", sym.symtype.short_name(), sym.typespec);
        for v in &sym.idefault {
            key.push_str(&format!("|i:{v}"));
        }
        for v in &sym.fdefault {
            key.push_str(&format!("|f:{}", canon_float_g9(*v)));
        }
        for v in &sym.sdefault {
            key.push_str(&format!("|s:{}", canon_escape_oso(v)));
        }
        key_by_name.insert(sym.name.clone(), key.clone());
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys.sort();

    let mut key_index: HashMap<String, usize> = HashMap::new();
    for (i, key) in keys.iter().enumerate() {
        key_index.insert(key.clone(), i);
    }

    let mut canon_map: HashMap<String, String> = HashMap::new();
    let mut key_occ: HashMap<String, usize> = HashMap::new();

    let mut assign = |name: &str, sym: &osl_rs::oso::OsoSymbol| {
        if canon_map.contains_key(name) {
            return;
        }
        let key = match key_by_name.get(name) {
            Some(k) => k.clone(),
            None => return,
        };
        let occ = key_occ.entry(key.clone()).or_insert(0);
        let kidx = *key_index.get(&key).unwrap_or(&0);
        let canon_name = format!("{}{}_{}", sym.symtype.short_name(), kidx, *occ);
        *occ += 1;
        canon_map.insert(name.to_string(), canon_name);
    };

    // Assign in order of first use in instructions.
    for instr in &oso.instructions {
        for arg in &instr.args {
            if let Some(sym) = sym_by_name.get(arg) {
                assign(arg, sym);
            }
        }
    }

    // Ensure params and output params are present even if unused.
    for sym in &oso.symbols {
        if matches!(sym.symtype, SymType::Param | SymType::OutputParam) {
            assign(&sym.name, sym);
        }
    }

    // Build sorted symbol entries by key index and occurrence.
    let mut entries: Vec<(usize, usize, &osl_rs::oso::OsoSymbol, String)> = Vec::new();
    for (name, canon) in &canon_map {
        let sym = match sym_by_name.get(name) {
            Some(s) => *s,
            None => continue,
        };
        let key = match key_by_name.get(name) {
            Some(k) => k,
            None => continue,
        };
        let kidx = *key_index.get(key).unwrap_or(&0);
        let occ = canon
            .split('_')
            .last()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        entries.push((kidx, occ, sym, canon.clone()));
    }
    entries.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    for (_kidx, _occ, sym, canon_name) in entries {
        out.push_str(&format!(
            "{} {} {}",
            sym.symtype.short_name(),
            sym.typespec,
            canon_name
        ));

        if !sym.idefault.is_empty() {
            for v in &sym.idefault {
                out.push(' ');
                out.push_str(&v.to_string());
            }
        }
        if !sym.fdefault.is_empty() {
            for v in &sym.fdefault {
                out.push(' ');
                out.push_str(&canon_float_g9(*v));
            }
        }
        if !sym.sdefault.is_empty() {
            for v in &sym.sdefault {
                out.push(' ');
                out.push('"');
                out.push_str(&canon_escape_oso(v));
                out.push('"');
            }
        }
        out.push('\n');
    }

    out.push_str("code\n");
    for instr in &oso.instructions {
        out.push('\t');
        out.push_str(&instr.opcode);
        for arg in &instr.args {
            let canon = canon_map.get(arg).cloned().unwrap_or_else(|| arg.clone());
            out.push(' ');
            out.push_str(&canon);
        }
        for j in &instr.jumps {
            out.push_str(&format!(" ${j}"));
        }
        out.push('\n');
    }
    out.push_str("end\n");
    out
}

fn normalize_oso(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    let mut cleaned = String::new();
    for line in normalized.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    if let Ok(oso) = osl_rs::oso::read_oso_string(&cleaned) {
        return canonicalize_oso(&oso);
    }
    if !cleaned.ends_with('\n') {
        cleaned.push('\n');
    }
    cleaned
}

fn first_diff_line(a: &str, b: &str) -> Option<(usize, String, String)> {
    let mut a_iter = a.lines();
    let mut b_iter = b.lines();
    let mut line = 1usize;
    loop {
        match (a_iter.next(), b_iter.next()) {
            (None, None) => return None,
            (Some(al), Some(bl)) => {
                if al != bl {
                    return Some((line, al.to_string(), bl.to_string()));
                }
            }
            (Some(al), None) => return Some((line, al.to_string(), String::new())),
            (None, Some(bl)) => return Some((line, String::new(), bl.to_string())),
        }
        line += 1;
    }
}

/// Helper: compile and run an OSL shader, return (IR, Interpreter).
fn compile_and_run(src: &str) -> (osl_rs::codegen::ShaderIR, Interpreter) {
    let ast = parser::parse(src).expect("parse failed").ast;
    let ir = codegen::generate(&ast);
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);
    (ir, interp)
}

/// Helper: compile, optimize, and run.
fn compile_optimize_run(
    src: &str,
    level: optimizer::OptLevel,
) -> (osl_rs::codegen::ShaderIR, Interpreter) {
    let ast = parser::parse(src).expect("parse failed").ast;
    let mut ir = codegen::generate(&ast);
    let _stats = optimizer::optimize(&mut ir, level);
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);
    (ir, interp)
}

// ============================================================================
// End-to-end: arithmetic and data flow
// ============================================================================

#[test]
fn e2e_basic_arithmetic() {
    let src = r#"
shader test() {
    float a = 10.0;
    float b = 3.0;
    float sum = a + b;
    float diff = a - b;
    float prod = a * b;
    float quot = a / b;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "sum").unwrap(), 13.0);
    assert_eq!(interp.get_float(&ir, "diff").unwrap(), 7.0);
    assert_eq!(interp.get_float(&ir, "prod").unwrap(), 30.0);
    assert!((interp.get_float(&ir, "quot").unwrap() - 10.0 / 3.0).abs() < 1e-6);
}

#[test]
fn e2e_chained_assignments() {
    let src = r#"
shader test() {
    float x = 1.0;
    x = x + 1.0;
    x = x + 1.0;
    x = x + 1.0;
    x = x * 2.0;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let x = interp.get_float(&ir, "x").unwrap();
    // 1+1+1+1 = 4, 4*2 = 8
    assert_eq!(x, 8.0);
}

#[test]
fn e2e_compound_assignment() {
    let src = r#"
shader test() {
    float x = 5.0;
    x += 3.0;
    x *= 2.0;
    x -= 1.0;
}
"#;
    let (ir, interp) = compile_and_run(src);
    // x = 5, +=3 -> 8, *=2 -> 16, -=1 -> 15
    assert_eq!(interp.get_float(&ir, "x").unwrap(), 15.0);
}

// ============================================================================
// End-to-end: control flow
// ============================================================================

#[test]
fn e2e_if_true_branch() {
    let src = r#"
shader test() {
    float x = 10.0;
    float result = 0.0;
    if (x > 5.0) {
        result = 1.0;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "result").unwrap(), 1.0);
}

#[test]
fn e2e_if_false_branch() {
    let src = r#"
shader test() {
    float x = 2.0;
    float result = 0.0;
    if (x > 5.0) {
        result = 1.0;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "result").unwrap(), 0.0);
}

#[test]
fn e2e_if_else() {
    let src = r#"
shader test() {
    float x = 2.0;
    float result = 0.0;
    if (x > 5.0) {
        result = 1.0;
    } else {
        result = -1.0;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "result").unwrap(), -1.0);
}

#[test]
fn e2e_nested_if() {
    let src = r#"
shader test() {
    float x = 10.0;
    float result = 0.0;
    if (x > 5.0) {
        if (x > 8.0) {
            result = 2.0;
        } else {
            result = 1.0;
        }
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "result").unwrap(), 2.0);
}

// ============================================================================
// End-to-end: parameter defaults
// ============================================================================

#[test]
fn e2e_param_defaults() {
    let src = r#"
surface simple(float Kd = 0.8, float Ks = 0.2) {
    float total = Kd + Ks;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "Kd").unwrap(), 0.8);
    assert_eq!(interp.get_float(&ir, "Ks").unwrap(), 0.2);
    assert!((interp.get_float(&ir, "total").unwrap() - 1.0).abs() < 1e-6);
}

#[test]
fn e2e_param_computation() {
    let src = r#"
surface test(float roughness = 0.5) {
    float alpha = roughness * roughness;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "alpha").unwrap(), 0.25);
}

// ============================================================================
// End-to-end: shader globals
// ============================================================================

#[test]
fn e2e_shader_globals() {
    let src = r#"
shader test() {
    float su = u;
    float sv = v;
    float st = time;
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.u = 0.3;
    globals.v = 0.7;
    globals.time = 1.5;

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    assert_eq!(interp.get_float(&ir, "su").unwrap(), 0.3);
    assert_eq!(interp.get_float(&ir, "sv").unwrap(), 0.7);
    assert_eq!(interp.get_float(&ir, "st").unwrap(), 1.5);
}

#[test]
fn e2e_position_global() {
    let src = r#"
shader test() {
    point pos = P;
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.p = Vec3::new(1.0, 2.0, 3.0);

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    let pos = interp.get_vec3(&ir, "pos").unwrap();
    assert_eq!(pos.x, 1.0);
    assert_eq!(pos.y, 2.0);
    assert_eq!(pos.z, 3.0);
}

// ============================================================================
// End-to-end: math builtins
// ============================================================================

#[test]
fn e2e_trig_functions() {
    let src = r#"
shader test() {
    float a = sin(0.0);
    float b = cos(0.0);
    float c = sqrt(9.0);
    float d = abs(-7.5);
    float e = pow(2.0, 10.0);
    float f = exp(0.0);
    float g = log(1.0);
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert!((interp.get_float(&ir, "a").unwrap() - 0.0).abs() < 1e-6);
    assert!((interp.get_float(&ir, "b").unwrap() - 1.0).abs() < 1e-6);
    assert!((interp.get_float(&ir, "c").unwrap() - 3.0).abs() < 1e-6);
    assert!((interp.get_float(&ir, "d").unwrap() - 7.5).abs() < 1e-6);
    assert!((interp.get_float(&ir, "e").unwrap() - 1024.0).abs() < 1e-1);
    assert!((interp.get_float(&ir, "f").unwrap() - 1.0).abs() < 1e-6);
    assert!((interp.get_float(&ir, "g").unwrap() - 0.0).abs() < 1e-6);
}

#[test]
fn e2e_clamp_and_smoothstep() {
    let src = r#"
shader test() {
    float a = clamp(5.0, 0.0, 1.0);
    float b = clamp(-1.0, 0.0, 1.0);
    float c = smoothstep(0.0, 1.0, 0.5);
    float d = mix(0.0, 10.0, 0.3);
    float e = min(3.0, 7.0);
    float f = max(3.0, 7.0);
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "a").unwrap(), 1.0);
    assert_eq!(interp.get_float(&ir, "b").unwrap(), 0.0);
    assert!((interp.get_float(&ir, "c").unwrap() - 0.5).abs() < 1e-6);
    assert!((interp.get_float(&ir, "d").unwrap() - 3.0).abs() < 1e-6);
    assert_eq!(interp.get_float(&ir, "e").unwrap(), 3.0);
    assert_eq!(interp.get_float(&ir, "f").unwrap(), 7.0);
}

// ============================================================================
// End-to-end: vector operations
// ============================================================================

#[test]
fn e2e_vector_dot() {
    let src = r#"
shader test() {
    float d = dot(N, I);
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.n = Vec3::new(0.0, 1.0, 0.0);
    globals.i = Vec3::new(0.0, -1.0, 0.0);

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    assert_eq!(interp.get_float(&ir, "d").unwrap(), -1.0);
}

#[test]
fn e2e_normalize() {
    let src = r#"
shader test() {
    float ilen = length(I);
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.i = Vec3::new(3.0, 4.0, 0.0);

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    let ilen = interp.get_float(&ir, "ilen").unwrap();
    assert!(
        (ilen - 5.0).abs() < 1e-5,
        "length of (3,4,0) should be 5.0, got {ilen}"
    );
}

// ============================================================================
// End-to-end: preprocessor integration
// ============================================================================

#[test]
fn e2e_preprocess_then_compile() {
    let src = r#"
#define INTENSITY 0.75
#define SCALE 2.0

shader test() {
    float val = INTENSITY * SCALE;
}
"#;
    let preprocessed = preprocess::preprocess(src).expect("preprocess failed");
    let ast = parser::parse(&preprocessed).expect("parse failed").ast;
    let ir = codegen::generate(&ast);
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    assert_eq!(interp.get_float(&ir, "val").unwrap(), 1.5);
}

#[test]
fn e2e_preprocess_conditional() {
    let src = r#"
#define USE_ADVANCED 1

shader test() {
#ifdef USE_ADVANCED
    float val = 42.0;
#else
    float val = 0.0;
#endif
}
"#;
    let preprocessed = preprocess::preprocess(src).expect("preprocess failed");
    let ast = parser::parse(&preprocessed).expect("parse failed").ast;
    let ir = codegen::generate(&ast);
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    assert_eq!(interp.get_float(&ir, "val").unwrap(), 42.0);
}

// ============================================================================
// End-to-end: full oslc compile pipeline
// ============================================================================

#[test]
fn e2e_oslc_compile_surface() {
    let src = r#"
surface matte(color Cd = color(0.8, 0.8, 0.8), float Kd = 1.0) {
    float diffuse_weight = Kd * 0.5;
}
"#;
    let opts = CompilerOptions::default();
    let result = compile_string(src, &opts);
    assert!(
        result.success,
        "Compilation should succeed: {:?}",
        result.errors
    );
    assert!(
        !result.oso_text.is_empty(),
        "OSO output should not be empty"
    );
    assert!(result.oso_text.contains("OpenShadingLanguage"));
    assert!(result.oso_text.contains("surface matte"));
}

#[test]
fn e2e_oslc_compile_and_run() {
    let src = r#"
shader compute(float x = 3.0, float y = 4.0) {
    float hyp = sqrt(x * x + y * y);
}
"#;
    let opts = CompilerOptions::default();
    let result = compile_string(src, &opts);
    assert!(result.success);

    // Now run the compiled IR
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&result.ir, &globals, None);

    let hyp = interp.get_float(&result.ir, "hyp").unwrap();
    assert!(
        (hyp - 5.0).abs() < 1e-5,
        "sqrt(3^2 + 4^2) should be 5.0, got {hyp}"
    );
}

// ============================================================================
// End-to-end: optimizer
// ============================================================================

#[test]
fn e2e_optimize_then_run() {
    let src = r#"
shader test() {
    float a = 2.0;
    float b = 3.0;
    float c = a + b;
    float d = c * 2.0;
}
"#;
    let (ir, interp) = compile_optimize_run(src, optimizer::OptLevel::O2);
    assert_eq!(interp.get_float(&ir, "d").unwrap(), 10.0);
}

// ============================================================================
// End-to-end: realistic shaders
// ============================================================================

#[test]
fn e2e_simple_diffuse_shader() {
    let src = r#"
surface simple_diffuse(
    color diffuse_color = color(0.8, 0.2, 0.1),
    float roughness = 0.5
) {
    float alpha = roughness * roughness;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "roughness").unwrap(), 0.5);
    assert_eq!(interp.get_float(&ir, "alpha").unwrap(), 0.25);
}

#[test]
fn e2e_lambert_shading() {
    let src = r#"
surface lambert(float Kd = 1.0) {
    float NdotI = dot(N, I);
    float diffuse = Kd * abs(NdotI);
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.n = Vec3::new(0.0, 0.0, 1.0);
    globals.i = Vec3::new(0.0, 0.0, -1.0);

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    let ndoti = interp.get_float(&ir, "NdotI").unwrap();
    assert_eq!(ndoti, -1.0, "N.I should be -1.0");
    let diffuse = interp.get_float(&ir, "diffuse").unwrap();
    assert_eq!(diffuse, 1.0, "Kd * |N.I| should be 1.0");
}

#[test]
fn e2e_fresnel_approximation() {
    let src = r#"
shader fresnel_test() {
    float f0 = 0.04;
    float cos_theta = 0.5;
    float one_minus = 1.0 - cos_theta;
    float p2 = one_minus * one_minus;
    float p5 = p2 * p2 * one_minus;
    float fresnel = f0 + (1.0 - f0) * p5;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let fresnel = interp.get_float(&ir, "fresnel").unwrap();
    // Schlick's approximation: F0 + (1-F0)*(1-cos(theta))^5
    // = 0.04 + 0.96 * 0.5^5 = 0.04 + 0.96 * 0.03125 = 0.04 + 0.03 = 0.07
    assert!(
        (fresnel - 0.07).abs() < 1e-5,
        "Fresnel should be ~0.07, got {fresnel}"
    );
}

#[test]
fn e2e_uvs_to_color() {
    let src = r#"
shader uv_color() {
    float r = u;
    float g = v;
    float b = 0.0;
}
"#;
    let mut globals = ShaderGlobals::default();
    globals.u = 0.5;
    globals.v = 0.8;

    let ast = parser::parse(src).unwrap().ast;
    let ir = codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);

    assert_eq!(interp.get_float(&ir, "r").unwrap(), 0.5);
    assert_eq!(interp.get_float(&ir, "g").unwrap(), 0.8);
    assert_eq!(interp.get_float(&ir, "b").unwrap(), 0.0);
}

// ============================================================================
// End-to-end: convenience function
// ============================================================================

#[test]
fn e2e_run_shader_convenience() {
    let interp = run_shader(
        r#"
shader test() {
    float answer = 42.0;
}
"#,
    )
    .unwrap();
    assert!(interp.messages.is_empty());
}

// ============================================================================
// End-to-end: division safety
// ============================================================================

#[test]
fn e2e_safe_division() {
    let src = r#"
shader test() {
    float a = 1.0;
    float b = 0.0;
    float c = a / b;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(
        interp.get_float(&ir, "c").unwrap(),
        0.0,
        "Division by zero should produce 0.0"
    );
}

// ============================================================================
// End-to-end: full pipeline with defines
// ============================================================================

#[test]
fn e2e_oslc_with_defines() {
    let src = r#"
#ifdef HIGH_QUALITY
#define SAMPLES 64
#else
#define SAMPLES 4
#endif

shader test() {
    int num_samples = SAMPLES;
}
"#;
    let mut opts = CompilerOptions::default();
    opts.defines.push(("HIGH_QUALITY".into(), "1".into()));
    let result = compile_string(src, &opts);
    assert!(
        result.success,
        "Compilation should succeed: {:?}",
        result.errors
    );
}

// ============================================================================
// End-to-end: negation and unary ops
// ============================================================================

#[test]
fn e2e_negation() {
    let src = r#"
shader test() {
    float a = 5.0;
    float b = -a;
    float c = -b;
}
"#;
    let (ir, interp) = compile_and_run(src);
    assert_eq!(interp.get_float(&ir, "b").unwrap(), -5.0);
    assert_eq!(interp.get_float(&ir, "c").unwrap(), 5.0);
}

// ============================================================================
// End-to-end: complex expressions
// ============================================================================

#[test]
fn e2e_complex_expression() {
    let src = r#"
shader test() {
    float a = 2.0;
    float b = 3.0;
    float c = (a + b) * (a - b);
}
"#;
    let (ir, interp) = compile_and_run(src);
    // (2+3) * (2-3) = 5 * -1 = -5
    assert_eq!(interp.get_float(&ir, "c").unwrap(), -5.0);
}

// ============================================================================
// End-to-end: for loop
// ============================================================================

#[test]
fn e2e_for_loop() {
    let src = r#"
shader test() {
    float sum = 0.0;
    float i = 0.0;
    for (i = 0.0; i < 5.0; i += 1.0) {
        sum += i;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    let sum = interp.get_float(&ir, "sum").unwrap();
    // 0+1+2+3+4 = 10
    assert_eq!(sum, 10.0, "for loop sum should be 10, got {sum}");
}

#[test]
fn e2e_while_loop() {
    let src = r#"
shader test() {
    float x = 1.0;
    float count = 0.0;
    while (x < 100.0) {
        x = x * 2.0;
        count += 1.0;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    let x = interp.get_float(&ir, "x").unwrap();
    let count = interp.get_float(&ir, "count").unwrap();
    // 1*2=2, 2*2=4, 4*2=8, 8*2=16, 16*2=32, 32*2=64, 64*2=128 → x=128, count=7
    assert_eq!(x, 128.0, "while loop: x should be 128, got {x}");
    assert_eq!(count, 7.0, "while loop: count should be 7, got {count}");
}

#[test]
fn e2e_nested_loops() {
    let src = r#"
shader test() {
    float sum = 0.0;
    float i = 0.0;
    float j = 0.0;
    for (i = 0.0; i < 3.0; i += 1.0) {
        for (j = 0.0; j < 3.0; j += 1.0) {
            sum += 1.0;
        }
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    let sum = interp.get_float(&ir, "sum").unwrap();
    // 3 * 3 = 9 iterations
    assert_eq!(sum, 9.0, "nested loops: sum should be 9, got {sum}");
}

// ============================================================================
// End-to-end: struct parsing
// ============================================================================

#[test]
fn e2e_struct_parse() {
    let src = r#"
struct Material {
    float roughness;
    float metallic;
    color baseColor;
};

shader test() {
    float x = 1.0;
}
"#;
    // Should parse without error
    let ast = parser::parse(src);
    assert!(
        ast.is_ok(),
        "Struct parsing should succeed: {:?}",
        ast.err()
    );
}

// ============================================================================
// End-to-end: printf formatting
// ============================================================================

#[test]
fn e2e_printf_basic() {
    let src = r#"
shader test() {
    printf("hello world");
}
"#;
    let interp = run_shader(src).unwrap();
    assert_eq!(interp.messages.len(), 1);
    assert_eq!(interp.messages[0], "hello world");
}

#[test]
fn e2e_printf_format_int() {
    let src = r#"
shader test() {
    int x = 42;
    printf("value is %d", x);
}
"#;
    // Note: codegen treats `int x = 42` as const assignment
    // The printf requires the interpreter to resolve %d
    let interp = run_shader(src).unwrap();
    assert!(!interp.messages.is_empty());
    assert!(
        interp.messages[0].contains("42"),
        "printf should contain 42: {:?}",
        interp.messages
    );
}

// ============================================================================
// End-to-end: ShadingSystem.execute_source
// ============================================================================

#[test]
fn e2e_shading_system_execute_source() {
    let ss = ShadingSystem::new(Arc::new(NullRenderer), None);
    let globals = ShaderGlobals::default();
    let result = ss.execute_source(
        r#"
shader test() {
    float x = 3.0;
    float y = 4.0;
    float hyp = sqrt(x * x + y * y);
}
"#,
        &globals,
    );
    assert!(result.is_ok());
    let result = result.unwrap();
    let hyp = result.get_float("hyp").unwrap();
    assert!((hyp - 5.0).abs() < 1e-5, "hyp should be 5.0, got {hyp}");
}

// ============================================================================
// End-to-end: OSO → IR → execute
// ============================================================================

#[test]
fn e2e_oso_compile_then_execute() {
    // Compile to OSO, then load and execute through ShadingSystem
    let src = r#"
surface test_material(float Kd = 0.8) {
    float result = Kd * 2.0;
}
"#;
    let opts = CompilerOptions::default();
    let compile_result = compile_string(src, &opts);
    assert!(compile_result.success);

    // Now execute the compiled IR directly
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&compile_result.ir, &globals, None);
    let result = interp.get_float(&compile_result.ir, "result").unwrap();
    assert_eq!(result, 1.6, "Kd(0.8) * 2.0 should be 1.6");
}

// ============================================================================
// End-to-end: break in loop
// ============================================================================

#[test]
fn e2e_for_loop_with_break() {
    let src = r#"
shader test() {
    float sum = 0.0;
    float i = 0.0;
    for (i = 0.0; i < 100.0; i += 1.0) {
        if (i > 4.0) {
            break;
        }
        sum += i;
    }
}
"#;
    let (ir, interp) = compile_and_run(src);
    let sum = interp.get_float(&ir, "sum").unwrap();
    // 0+1+2+3+4 = 10 (break when i > 4)
    assert_eq!(sum, 10.0, "break loop: sum should be 10, got {sum}");
}

// ============================================================================
// Dual2<Vec3> operations
// ============================================================================

#[test]
fn e2e_dual_vec3_operations() {
    use osl_rs::dual::Dual2;
    use osl_rs::dual_vec::*;
    use osl_rs::math::Vec3;

    let a = Dual2::<Vec3>::new(
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.1, 0.0, 0.0),
        Vec3::ZERO,
    );
    let b = Dual2::<Vec3>::new(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 0.1, 0.0),
    );
    let c = cross(&a, &b);
    assert!((c.val.z - 1.0).abs() < 1e-5, "cross z should be 1");
    let d = dot(&a, &b);
    assert!((d.val).abs() < 1e-5, "perpendicular dot should be 0");
}

// ============================================================================
// Noise variants
// ============================================================================

#[test]
fn e2e_noise_4d() {
    use osl_rs::noise;
    let v = noise::perlin4(1.5, 2.5, 3.5, 4.5);
    assert!(v.is_finite());
    assert!(v > -2.0 && v < 2.0);

    let c = noise::cellnoise4(1.5, 2.5, 3.5, 4.5);
    assert!(c >= 0.0 && c < 1.0);
}

#[test]
fn e2e_noise_periodic() {
    use osl_rs::math::Vec3;
    use osl_rs::noise;
    let period = Vec3::new(4.0, 4.0, 4.0);
    let p1 = Vec3::new(1.5, 2.5, 3.5);
    let p2 = Vec3::new(1.5 + 4.0, 2.5 + 4.0, 3.5 + 4.0);
    let v1 = noise::pperlin3(p1, period);
    let v2 = noise::pperlin3(p2, period);
    assert!((v1 - v2).abs() < 1e-5, "periodic noise must repeat");

    let v1d = noise::pperlin1(1.5, 4.0);
    let v2d = noise::pperlin1(1.5 + 4.0, 4.0);
    assert!((v1d - v2d).abs() < 1e-5, "1D periodic noise must repeat");
}

#[test]
fn e2e_noise_deriv() {
    use osl_rs::noise;
    let (val, d) = noise::perlin1_deriv(1.5);
    assert!(val.is_finite());
    assert!(d.is_finite());

    let (val2, dx, dy) = noise::perlin2_deriv(1.5, 2.5);
    assert!(val2.is_finite());
    assert!(dx.is_finite());
    assert!(dy.is_finite());
}

#[test]
fn e2e_noise_vec3_valued() {
    use osl_rs::math::Vec3;
    use osl_rs::noise;
    let p = Vec3::new(1.5, 2.5, 3.5);
    let v = noise::vperlin3(p);
    assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
    // Components should be different
    assert!((v.x - v.y).abs() > 1e-6 || (v.x - v.z).abs() > 1e-6);
}

// ============================================================================
// Spline variants
// ============================================================================

#[test]
fn e2e_spline_all_bases() {
    use osl_rs::spline::*;
    let knots = [0.0_f32, 0.0, 1.0, 1.0];

    let v_cr = spline_float(SplineBasis::CatmullRom, 0.5, &knots);
    assert!(v_cr.is_finite());

    let v_bs = spline_float(SplineBasis::BSpline, 0.5, &knots);
    assert!(v_bs.is_finite());

    let v_bz = spline_float(SplineBasis::Bezier, 0.5, &knots);
    assert!(v_bz.is_finite());

    let v_hm = spline_float(SplineBasis::Hermite, 0.5, &knots);
    assert!(v_hm.is_finite());
}

#[test]
fn e2e_spline_vec3() {
    use osl_rs::math::Vec3;
    use osl_rs::spline::*;
    let knots = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];
    let v = spline_vec3(SplineBasis::CatmullRom, 0.5, &knots);
    assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
}

#[test]
fn e2e_spline_derivative() {
    use osl_rs::spline::*;
    let knots = [0.0_f32, 0.0, 1.0, 1.0];
    let d = spline_float_deriv(SplineBasis::CatmullRom, 0.5, &knots);
    assert!(d.is_finite());
    assert!(
        d > 0.0,
        "monotonically increasing spline should have positive deriv"
    );
}

// ============================================================================
// Color space transforms
// ============================================================================

#[test]
fn e2e_color_spaces_full() {
    use osl_rs::color::*;
    use osl_rs::math::Color3;

    let c = Color3::new(0.5, 0.3, 0.8);

    // Test all color space roundtrips
    let spaces = ["hsv", "hsl", "YIQ", "XYZ", "sRGB"];
    for space in &spaces {
        let converted = transform_color("rgb", space, c);
        let back = transform_color(space, "rgb", converted);
        let eps = if *space == "XYZ" || *space == "xyY" {
            0.05
        } else {
            0.02
        };
        assert!(
            (c.x - back.x).abs() < eps && (c.y - back.y).abs() < eps && (c.z - back.z).abs() < eps,
            "Roundtrip failed for space {space}: orig={:?}, back={:?}",
            c,
            back
        );
    }
}

// ============================================================================
// Gabor noise
// ============================================================================

#[test]
fn e2e_gabor_modes() {
    use osl_rs::gabor::*;
    use osl_rs::math::Vec3;

    let p = Vec3::new(1.5, 2.5, 3.5);
    let v_iso = gabor3_isotropic(p, 1.0, 1.0);
    let v_aniso = gabor3_anisotropic(p, Vec3::new(1.0, 0.0, 0.0), 1.0, 2.0);
    let v_default = gabor3_default(p);

    assert!(v_iso.is_finite());
    assert!(v_aniso.is_finite());
    assert!(v_default.is_finite());
}

// ============================================================================
// Degrees / Radians
// ============================================================================

#[test]
fn e2e_degrees_radians() {
    let src = r#"
shader test(output float deg = 0, output float rad = 0) {
    deg = degrees(3.14159265);
    rad = radians(180.0);
}
"#;
    let (ir, interp) = compile_and_run(src);
    let d = interp.get_float(&ir, "deg").unwrap();
    let r = interp.get_float(&ir, "rad").unwrap();
    assert!((d - 180.0).abs() < 0.01, "degrees(pi) ~= 180, got {d}");
    assert!(
        (r - std::f32::consts::PI).abs() < 0.001,
        "radians(180) ~= pi, got {r}"
    );
}

// ============================================================================
// Null noise
// ============================================================================

#[test]
fn e2e_nullnoise() {
    // Null noise always returns 0, unsigned null always 0.5
    let v0 = osl_rs::noise::nullnoise(Vec3::new(1.0, 2.0, 3.0));
    let v1 = osl_rs::noise::unullnoise(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(v0, 0.0);
    assert_eq!(v1, 0.5);
}

// ============================================================================
// Periodic noise by name dispatch
// ============================================================================

#[test]
fn e2e_pnoise_by_name() {
    let p = Vec3::new(1.5, 2.5, 3.5);
    let period = Vec3::new(4.0, 4.0, 4.0);

    let v1 = osl_rs::noise::pnoise_by_name("perlin", p, period);
    assert!(v1.abs() <= 1.0, "Periodic perlin should be in [-1,1]");

    // Check periodicity
    let p2 = Vec3::new(p.x + period.x, p.y + period.y, p.z + period.z);
    let v2 = osl_rs::noise::pnoise_by_name("perlin", p2, period);
    assert!(
        (v1 - v2).abs() < 1e-4,
        "Periodic noise should repeat: {v1} vs {v2}"
    );

    // Cell noise periodic
    let vc1 = osl_rs::noise::pnoise_by_name("cellnoise", p, period);
    let vc2 = osl_rs::noise::pnoise_by_name("cellnoise", p2, period);
    assert!(
        (vc1 - vc2).abs() < 1e-6,
        "Periodic cell noise should repeat"
    );

    // Hash noise periodic
    let vh1 = osl_rs::noise::pnoise_by_name("hashnoise", p, period);
    let vh2 = osl_rs::noise::pnoise_by_name("hashnoise", p2, period);
    assert!(
        (vh1 - vh2).abs() < 1e-6,
        "Periodic hash noise should repeat"
    );
}

// ============================================================================
// Vec3-valued periodic noise
// ============================================================================

#[test]
fn e2e_vpnoise_by_name() {
    let p = Vec3::new(0.7, 1.3, 2.1);
    let period = Vec3::new(3.0, 3.0, 3.0);

    let v1 = osl_rs::noise::vpnoise_by_name("perlin", p, period);
    let p2 = Vec3::new(p.x + period.x, p.y + period.y, p.z + period.z);
    let v2 = osl_rs::noise::vpnoise_by_name("perlin", p2, period);
    assert!(
        (v1.x - v2.x).abs() < 1e-4,
        "vec3 periodic noise x should repeat"
    );
    assert!(
        (v1.y - v2.y).abs() < 1e-4,
        "vec3 periodic noise y should repeat"
    );
    assert!(
        (v1.z - v2.z).abs() < 1e-4,
        "vec3 periodic noise z should repeat"
    );
}

// ============================================================================
// Struct-like field access in interpreter
// ============================================================================

#[test]
fn e2e_struct_field_access() {
    // Test that the interpreter's Value::Struct variant and getfield/setfield work.
    // We verify via the noise module's internal struct-like types + direct API.
    use osl_rs::interp::Value;
    let fields = vec![Value::Float(10.0), Value::Float(20.0), Value::Float(30.0)];
    let sv = Value::Struct(fields);
    if let Value::Struct(ref f) = sv {
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].as_float(), 10.0);
        assert_eq!(f[1].as_float(), 20.0);
        assert_eq!(f[2].as_float(), 30.0);
    } else {
        panic!("Expected Value::Struct");
    }
}

// ============================================================================
// Optimizer: constant folding
// ============================================================================

#[test]
fn e2e_optimizer_math_folding() {
    use osl_rs::optimizer::{OptLevel, optimize};

    let src = r#"
shader test() {
    float a = 2.0 + 3.0;
    float b = a * 2.0;
}
"#;
    let ast = osl_rs::parser::parse(src).unwrap().ast;
    let mut ir = osl_rs::codegen::generate(&ast);
    let stats = optimize(&mut ir, OptLevel::O2);
    assert!(
        stats.constant_folds > 0,
        "Should have folded some constants"
    );
}

// ============================================================================
// Fuzz / robustness tests — malformed input must not panic
// ============================================================================

/// Empty input should produce a parse error, not a panic.
#[test]
fn fuzz_empty_input() {
    let result = osl_rs::parser::parse("");
    assert!(result.is_err() || result.unwrap().ast.is_empty());
}

/// Garbage bytes should produce a parse error, not a panic.
#[test]
fn fuzz_garbage_osl() {
    let garbage = "\x00\x7fgarbage!!@@##$$\n\t\r{}()[];";
    let _ = osl_rs::parser::parse(garbage);
    // Not asserting error — just must not panic
}

/// Deeply nested braces should not stack-overflow.
#[test]
fn fuzz_deep_nesting() {
    let depth = 100;
    let mut src = String::from("shader test() { float x = 0; ");
    for _ in 0..depth {
        src.push_str("if (x > 0) { ");
    }
    src.push_str("x = 1.0; ");
    for _ in 0..depth {
        src.push_str("} ");
    }
    src.push('}');
    let _ = osl_rs::parser::parse(&src);
}

/// Extremely long identifier should not panic.
#[test]
fn fuzz_long_identifier() {
    let long_name: String = std::iter::repeat('a').take(10_000).collect();
    let src = format!(
        "shader test(output float {} = 0) {{ {} = 42.0; }}",
        long_name, long_name
    );
    let _ = osl_rs::parser::parse(&src);
}

/// String with unclosed quote should produce error, not panic.
#[test]
fn fuzz_unclosed_string() {
    let src = r#"shader test() { string s = "unclosed; }"#;
    let _ = osl_rs::parser::parse(src);
}

/// Malformed .oso should not panic (may or may not produce error).
#[test]
fn fuzz_malformed_oso() {
    let bad_oso = "OpenShadingLanguage 1.00\nthis is not valid oso data\n\x00\x7f";
    let _ = osl_rs::oso::read_oso_string(bad_oso);
    // Must not panic — the parser may accept or reject this gracefully
}

/// Empty .oso should produce an error, not a panic.
#[test]
fn fuzz_empty_oso() {
    let result = osl_rs::oso::read_oso_string("");
    assert!(result.is_err());
}

/// .oso with only header should not panic.
#[test]
fn fuzz_header_only_oso() {
    let result = osl_rs::oso::read_oso_string("OpenShadingLanguage 1.00\n");
    // Either an error or a minimal (possibly empty) shader — must not panic
    let _ = result;
}

/// Huge number literal should not panic.
#[test]
fn fuzz_huge_number() {
    let src = "shader test(output float result = 0) { result = 999999999999999999999999999999999999999.0; }";
    let _ = osl_rs::parser::parse(src);
}

/// Missing semicolons everywhere should not panic.
#[test]
fn fuzz_no_semicolons() {
    let src = "shader test() { float x = 1 float y = 2 float z = x + y }";
    let _ = osl_rs::parser::parse(src);
}

/// Repeated keywords should not panic.
#[test]
fn fuzz_repeated_keywords() {
    let src = "shader shader shader test() { float float float x = 0; }";
    let _ = osl_rs::parser::parse(src);
}

/// Unicode in identifiers should not panic.
#[test]
fn fuzz_unicode_identifiers() {
    let src = "shader тест() { float результат = 0; }";
    let _ = osl_rs::parser::parse(src);
}

/// Null bytes in source should not panic.
#[test]
fn fuzz_null_bytes() {
    let src = "shader\0test\0() { float\0x = 0; }";
    let _ = osl_rs::parser::parse(src);
}

/// Preprocessor edge cases should not panic.
#[test]
fn fuzz_preprocessor_recursive() {
    let src = "#define X X\nshader test() { float x = X; }";
    let _ = osl_rs::preprocess::preprocess(src);
}

/// Malformed .oso with truncated opcode section.
#[test]
fn fuzz_truncated_oso() {
    let oso = concat!(
        "OpenShadingLanguage 1.00\n",
        "shader test\n",
        "param float x 0\n",
        "code test\n",
        // Truncated — no opcodes, no "end"
    );
    let _ = osl_rs::oso::read_oso_string(oso);
}

/// Interpreter executing empty IR should not panic.
#[test]
fn fuzz_empty_ir_execute() {
    let ir = osl_rs::codegen::ShaderIR::new();
    let mut sg = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &mut sg, None);
    // Must complete without panic
}

// ============================================================================
// Reference testsuite — parse all .osl shaders from _ref/OpenShadingLanguage/testsuite
// ============================================================================

/// Compare OSO output against C++ oslc for a subset (or full set) of shaders.
/// Requires a reference oslc executable via `OSL_REF_OSLC` or `OSL_OSLC`, or
/// `oslc` available on PATH. Set `OSL_OSO_PARITY_FULL=1` to test all shaders.
#[test]
fn oso_parity_vs_ref_oslc() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let testsuite_dir = manifest_dir
        .join("..")
        .join("..")
        .join("_ref")
        .join("OpenShadingLanguage")
        .join("testsuite");

    if !testsuite_dir.exists() {
        eprintln!("Skipping oso_parity_vs_ref_oslc: _ref not found");
        return;
    }

    let oslc_path = match find_ref_oslc() {
        Some(p) => p,
        None => {
            eprintln!(
                "Skipping oso_parity_vs_ref_oslc: reference oslc not found (set OSL_REF_OSLC)"
            );
            return;
        }
    };

    let full = std::env::var("OSL_OSO_PARITY_FULL").is_ok();
    let mut test_dirs: Vec<PathBuf> = if full {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&testsuite_dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        entries
    } else {
        let sample = [
            "arithmetic",
            "function-overloads",
            "closure",
            "struct",
            "matrix",
            "noise",
            "texture-simple",
            "gettextureinfo",
            "vector",
            "initlist",
        ];
        sample.iter().map(|name| testsuite_dir.join(name)).collect()
    };

    let common_shaders = testsuite_dir.join("common").join("shaders");
    let src_shaders = testsuite_dir.parent().unwrap().join("src").join("shaders");
    let out_dir = manifest_dir.join("target").join("oso_parity");
    let _ = std::fs::create_dir_all(&out_dir);

    let mut compared = 0usize;
    let mut skipped = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for dir in test_dirs.drain(..) {
        let test_osl = dir.join("test.osl");
        if !test_osl.exists() {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let ref_oso = out_dir.join(format!("{name}.ref.oso"));
        let ours_oso = out_dir.join(format!("{name}.ours.oso"));

        let mut cmd = Command::new(&oslc_path);
        cmd.arg("-O0")
            .arg("-q")
            .arg("-o")
            .arg(&ref_oso)
            .arg("-I")
            .arg(&dir)
            .arg("-I")
            .arg(&common_shaders)
            .arg("-I")
            .arg(&src_shaders)
            .arg(&test_osl);

        let output = match cmd.output() {
            Ok(out) => out,
            Err(e) => {
                skipped += 1;
                eprintln!("Skipping {name}: failed to run oslc ({e})");
                continue;
            }
        };
        if !output.status.success() {
            skipped += 1;
            eprintln!("Skipping {name}: oslc failed");
            continue;
        }

        let mut opts = CompilerOptions::default();
        opts.opt_level = 0;
        opts.include_paths = vec![dir.clone(), common_shaders.clone(), src_shaders.clone()];
        let ours = osl_rs::oslc::compile_file(&test_osl, &opts);
        if !ours.success {
            skipped += 1;
            eprintln!("Skipping {name}: osl-rs oslc failed");
            if !ours.errors.is_empty() {
                eprintln!("  Errors:");
                for err in ours.errors.iter().take(10) {
                    eprintln!("    - {err}");
                }
                if ours.errors.len() > 10 {
                    eprintln!("    ... and {} more", ours.errors.len() - 10);
                }
            }
            continue;
        }

        if std::env::var("OSL_OSO_PARITY_DUMP").is_ok() {
            let _ = std::fs::write(&ours_oso, &ours.oso_text);
        }

        let ref_bytes = match std::fs::read(&ref_oso) {
            Ok(b) => b,
            Err(e) => {
                skipped += 1;
                eprintln!("Skipping {name}: failed to read ref oso ({e})");
                continue;
            }
        };
        let ref_text = String::from_utf8_lossy(&ref_bytes).to_string();

        let ref_norm = normalize_oso(&ref_text);
        let ours_norm = normalize_oso(&ours.oso_text);

        compared += 1;
        if ref_norm != ours_norm {
            if let Some((line, a, b)) = first_diff_line(&ref_norm, &ours_norm) {
                mismatches.push(format!(
                    "{name}: first diff at line {line}\n  ref:  {a}\n  ours: {b}"
                ));
            } else {
                mismatches.push(format!(
                    "{name}: outputs differ (size {} vs {})",
                    ref_norm.len(),
                    ours_norm.len()
                ));
            }
        }
    }

    if !mismatches.is_empty() {
        let preview = mismatches.join("\n\n");
        panic!("OSO parity mismatches:\n{preview}");
    }
    assert!(compared > 0, "No shaders compared (skipped={skipped}).");
}

/// Walk the reference testsuite directory and attempt to parse every test.osl.
/// This validates our parser covers the full OSL grammar.
#[test]
fn ref_testsuite_parse_all() {
    let testsuite_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("_ref")
        .join("OpenShadingLanguage")
        .join("testsuite");

    if !testsuite_dir.exists() {
        eprintln!("Skipping ref_testsuite_parse_all: _ref not found");
        return;
    }

    let mut total = 0u32;
    let mut parsed_ok = 0u32;
    let mut parse_fail = 0u32;
    let mut failures: Vec<(String, String)> = Vec::new();

    for entry in std::fs::read_dir(&testsuite_dir).unwrap() {
        let entry = entry.unwrap();
        let test_osl = entry.path().join("test.osl");
        if !test_osl.exists() {
            continue;
        }

        // Skip oslc-err-* tests: they intentionally contain invalid code
        // that is expected to produce compile errors (not parse errors).
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if dir_name.starts_with("oslc-err-") {
            continue;
        }

        total += 1;
        let src = std::fs::read_to_string(&test_osl).unwrap_or_default();

        // Preprocess with proper include paths and filename context
        let mut pp = osl_rs::preprocess::Preprocessor::new();
        let test_dir = entry.path();
        pp.include_paths
            .push(test_dir.to_string_lossy().to_string());
        // Common shaders and headers
        pp.include_paths.push(
            testsuite_dir
                .join("common")
                .join("shaders")
                .to_string_lossy()
                .to_string(),
        );
        // The src/shaders directory has color2.h, color4.h, vector2.h, vector4.h
        let src_shaders = testsuite_dir.parent().unwrap().join("src").join("shaders");
        pp.include_paths
            .push(src_shaders.to_string_lossy().to_string());
        let test_osl_str = test_osl.to_string_lossy().to_string();
        let preprocessed = pp
            .process_file(&src, &test_osl_str)
            .unwrap_or_else(|_| src.clone());
        match osl_rs::parser::parse(&preprocessed) {
            Ok(_) => parsed_ok += 1,
            Err(e) => {
                parse_fail += 1;
                let name = entry.file_name().to_string_lossy().to_string();
                failures.push((name, format!("{:?}", e)));
            }
        }
    }

    eprintln!("Reference testsuite parse results:");
    eprintln!("  Total:  {}", total);
    eprintln!(
        "  Parsed: {} ({:.1}%)",
        parsed_ok,
        parsed_ok as f64 / total as f64 * 100.0
    );
    eprintln!(
        "  Failed: {} ({:.1}%)",
        parse_fail,
        parse_fail as f64 / total as f64 * 100.0
    );

    if !failures.is_empty() {
        eprintln!("\nFirst 10 failures:");
        for (name, err) in failures.iter().take(10) {
            eprintln!("  {}: {}", name, &err[..err.len().min(120)]);
        }
    }

    // Parser now handles 100% of the reference testsuite (191/191).
    let success_rate = parsed_ok as f64 / total as f64;
    assert!(
        success_rate >= 1.0,
        "Expected 100% parse success rate, got {:.1}% ({}/{})",
        success_rate * 100.0,
        parsed_ok,
        total,
    );
}

/// Full-pipeline parity audit: preprocess → parse → typecheck → codegen on ALL ref shaders.
#[test]
fn ref_testsuite_full_pipeline() {
    let testsuite_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("_ref")
        .join("OpenShadingLanguage")
        .join("testsuite");

    if !testsuite_dir.exists() {
        eprintln!("Skipping ref_testsuite_full_pipeline: _ref not found");
        return;
    }

    let src_shaders = testsuite_dir.parent().unwrap().join("src").join("shaders");
    let common_shaders = testsuite_dir.join("common").join("shaders");

    let mut total = 0u32;
    let mut parse_ok = 0u32;
    let mut typecheck_ok = 0u32;
    let mut typecheck_warn_only = 0u32;
    let mut codegen_ok = 0u32;
    let mut codegen_has_ops = 0u32;

    let mut parse_fails: Vec<String> = Vec::new();
    let mut typecheck_fails: Vec<(String, Vec<String>)> = Vec::new();
    let mut codegen_fails: Vec<String> = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(&testsuite_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let test_osl = entry.path().join("test.osl");
        if !test_osl.exists() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip oslc-err-* tests: they intentionally contain invalid code.
        if name.starts_with("oslc-err-") {
            continue;
        }

        total += 1;
        let src = std::fs::read_to_string(&test_osl).unwrap_or_default();

        // ── Stage 1: Preprocess ──
        let mut pp = osl_rs::preprocess::Preprocessor::new();
        pp.include_paths
            .push(entry.path().to_string_lossy().to_string());
        pp.include_paths
            .push(common_shaders.to_string_lossy().to_string());
        pp.include_paths
            .push(src_shaders.to_string_lossy().to_string());
        let test_osl_str = test_osl.to_string_lossy().to_string();
        let preprocessed = pp
            .process_file(&src, &test_osl_str)
            .unwrap_or_else(|_| src.clone());

        // ── Stage 2: Parse ──
        let mut po = match osl_rs::parser::parse(&preprocessed) {
            Ok(po) => {
                parse_ok += 1;
                po
            }
            Err(_e) => {
                parse_fails.push(name.clone());
                continue;
            }
        };

        // ── Stage 3: Typecheck ──
        let (errors, warnings) = osl_rs::typecheck::typecheck_full(&mut po.ast);
        if errors.is_empty() {
            typecheck_ok += 1;
            if !warnings.is_empty() {
                typecheck_warn_only += 1;
            }
        } else {
            let err_msgs: Vec<String> = errors
                .iter()
                .take(3)
                .map(|e| format!("{}: {}", e.loc, e.message))
                .collect();
            typecheck_fails.push((name.clone(), err_msgs));
            // Continue to codegen anyway to measure it
        }

        // ── Stage 4: Codegen ──
        let ir = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            osl_rs::codegen::generate(&po.ast)
        }));
        match ir {
            Ok(ir) => {
                codegen_ok += 1;
                if !ir.opcodes.is_empty() || !ir.symbols.is_empty() {
                    codegen_has_ops += 1;
                }
            }
            Err(_) => {
                codegen_fails.push(name.clone());
            }
        }
    }

    eprintln!("\n══════════════════════════════════════════════════");
    eprintln!("  FULL PIPELINE PARITY AUDIT — {} ref shaders", total);
    eprintln!("══════════════════════════════════════════════════");
    eprintln!(
        "  Parse:     {}/{} ({:.1}%)",
        parse_ok,
        total,
        parse_ok as f64 / total as f64 * 100.0
    );
    eprintln!(
        "  Typecheck: {}/{} ({:.1}%) [{} warn-only]",
        typecheck_ok,
        parse_ok,
        typecheck_ok as f64 / parse_ok.max(1) as f64 * 100.0,
        typecheck_warn_only
    );
    eprintln!(
        "  Codegen:   {}/{} ({:.1}%) [{} with opcodes]",
        codegen_ok,
        parse_ok,
        codegen_ok as f64 / parse_ok.max(1) as f64 * 100.0,
        codegen_has_ops
    );
    eprintln!("══════════════════════════════════════════════════");

    if !parse_fails.is_empty() {
        eprintln!("\nParse failures ({}):", parse_fails.len());
        for f in &parse_fails {
            eprintln!("  - {}", f);
        }
    }
    if !typecheck_fails.is_empty() {
        eprintln!("\nTypecheck failures ({}):", typecheck_fails.len());
        for (name, errs) in typecheck_fails.iter().take(20) {
            eprintln!("  - {}: {}", name, errs.join("; "));
        }
        if typecheck_fails.len() > 20 {
            eprintln!("  ... and {} more", typecheck_fails.len() - 20);
        }
    }
    if !codegen_fails.is_empty() {
        eprintln!("\nCodegen panics ({}):", codegen_fails.len());
        for f in codegen_fails.iter().take(10) {
            eprintln!("  - {}", f);
        }
    }

    // Parse must stay at 100%
    assert_eq!(parse_ok, total, "Parse regression! Was 100%.");
}

/// Execution parity: compile + execute each ref shader, compare printf output with ref/out.txt.
#[test]
fn ref_testsuite_execution_parity() {
    let testsuite_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("_ref")
        .join("OpenShadingLanguage")
        .join("testsuite");

    if !testsuite_dir.exists() {
        eprintln!("Skipping ref_testsuite_execution_parity: _ref not found");
        return;
    }

    let src_shaders = testsuite_dir.parent().unwrap().join("src").join("shaders");
    let common_shaders = testsuite_dir.join("common").join("shaders");

    let mut total = 0u32;
    let mut exec_ok = 0u32;
    let mut exec_match = 0u32;
    let mut exec_panic = 0u32;
    let mut no_ref = 0u32;
    let mut mismatches: Vec<(String, String)> = Vec::new();
    let mut panics: Vec<String> = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(&testsuite_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let test_osl = entry.path().join("test.osl");
        let ref_out = entry.path().join("ref").join("out.txt");
        if !test_osl.exists() {
            continue;
        }
        total += 1;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip tests that are expected to fail compilation (oslc-err-*)
        if name.starts_with("oslc-err") {
            no_ref += 1;
            continue;
        }

        let src = std::fs::read_to_string(&test_osl).unwrap_or_default();

        // Preprocess
        let mut pp = osl_rs::preprocess::Preprocessor::new();
        pp.include_paths
            .push(entry.path().to_string_lossy().to_string());
        pp.include_paths
            .push(common_shaders.to_string_lossy().to_string());
        pp.include_paths
            .push(src_shaders.to_string_lossy().to_string());
        let test_osl_str = test_osl.to_string_lossy().to_string();
        let preprocessed = pp
            .process_file(&src, &test_osl_str)
            .unwrap_or_else(|_| src.clone());

        // Parse
        let po = match osl_rs::parser::parse(&preprocessed) {
            Ok(po) => po,
            Err(_) => continue,
        };

        // Typecheck (ignore errors, continue anyway)
        let mut ast_mut = po.ast;
        let _ = osl_rs::typecheck::typecheck(&mut ast_mut);

        // Codegen
        let ir = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            osl_rs::codegen::generate(&ast_mut)
        })) {
            Ok(ir) => ir,
            Err(_) => {
                exec_panic += 1;
                panics.push(name.clone());
                continue;
            }
        };

        // Determine grid size and params from run.py
        let params = parse_run_py(&entry.path().join("run.py"));
        let (xres, yres) = (params.xres, params.yres);

        // Set up renderer with shader/object space transforms matching C++ testshade
        let renderer = {
            use osl_rs::math::Matrix44;
            let mut r = BasicRenderer::new();
            let c = std::f32::consts::FRAC_1_SQRT_2;
            r.set_transform(
                "shader",
                Matrix44 {
                    m: [
                        [c, c, 0.0, 0.0],
                        [-c, c, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [1.0, 0.0, 0.0, 1.0],
                    ],
                },
            );
            r.set_transform(
                "object",
                Matrix44 {
                    m: [
                        [0.0, 1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 1.0, 0.0, 1.0],
                    ],
                },
            );
            // "myspace" = scale(1, 2, 1), matching C++ testshade
            r.set_transform(
                "myspace",
                Matrix44 {
                    m: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                },
            );
            Arc::new(r)
        };

        // Execute with timeout (some shaders may infinite-loop)
        let (tx, rx) = std::sync::mpsc::channel();
        let ir_clone = ir.clone();
        let renderer_clone = renderer.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut all_output = String::new();
                let mut seen_errors = std::collections::HashSet::new();
                for y in 0..yres {
                    for x in 0..xres {
                        let globals = setup_globals(x, y, &params);
                        let mut interp = Interpreter::new();
                        interp.set_renderer(renderer_clone.clone());
                        interp.execute(&ir_clone, &globals, None);
                        // Dedup ERROR messages across grid points (C++ OSL deduplicates)
                        for msg in &interp.messages {
                            if msg.starts_with("ERROR:") {
                                if seen_errors.insert(msg.clone()) {
                                    all_output.push_str(msg);
                                }
                            } else {
                                all_output.push_str(msg);
                            }
                        }
                    }
                }
                all_output
            }));
            let _ = tx.send(result);
        });

        let timeout_secs = if xres * yres > 16 { 30 } else { 10 };
        let output = match rx.recv_timeout(std::time::Duration::from_secs(timeout_secs)) {
            Ok(Ok(out)) => {
                exec_ok += 1;
                out
            }
            Ok(Err(_)) => {
                exec_panic += 1;
                panics.push(format!("{} (panic)", name));
                continue;
            }
            Err(_) => {
                exec_panic += 1;
                panics.push(format!("{} (timeout)", name));
                continue;
            }
        };

        // Compare with ref/out.txt
        if !ref_out.exists() {
            no_ref += 1;
            continue;
        }

        let expected_raw = std::fs::read_to_string(&ref_out).unwrap_or_default();
        // Skip non-shader-output lines from expected output (renderer/compiler messages)
        let expected: String = expected_raw
            .lines()
            .filter(|l| {
                !l.starts_with("Compiled ") &&
                !l.starts_with("FAILED ") &&
                !l.starts_with("Output ") &&
                !l.starts_with("Optimized ") &&
                !l.starts_with("shader \"") &&
                !(l.starts_with("    \"") || l.starts_with("\t\"")) &&
                !l.starts_with("\t\tDefault value:") &&
                !l.starts_with("        Default value:") &&
                !l.starts_with("\t\tmetadata:") &&
                !l.starts_with("        metadata:") &&
                // Skip compiler warnings/diagnostics from oslc
                !l.contains(": warning:") &&
                !l.starts_with("  Chosen ") &&
                !l.starts_with("  Other ") &&
                !l.starts_with("    test.osl:") &&
                // Skip LLVM batched diagnostics
                !l.contains("is forced llvm bool") &&
                !l.starts_with("Connect ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let expected = expected.trim();

        let actual = output.trim();

        // Fuzzy float comparison: treat lines as matching if all float tokens
        // are within relative tolerance (platform f32 differences)
        let fuzzy_float_eq = |fa: f64, fb: f64| -> bool {
            if fa == fb {
                return true;
            }
            if fa == 0.0 && fb == 0.0 {
                return true;
            }
            let abs_diff = (fa - fb).abs();
            let rel = abs_diff / fa.abs().max(fb.abs()).max(1e-30);
            rel <= 1e-3 || abs_diff <= 1e-4
        };
        // Extract all float-like tokens from a string for comparison
        let extract_floats = |s: &str| -> Vec<f64> {
            let re_float = regex::Regex::new(r"-?\d+\.\d+(?:e[+-]?\d+)?").unwrap();
            re_float
                .find_iter(s)
                .filter_map(|m| m.as_str().parse::<f64>().ok())
                .collect()
        };
        let fuzzy_line_match = |a: &str, b: &str| -> bool {
            if a == b {
                return true;
            }
            // First try whitespace-token comparison
            let at: Vec<&str> = a.split_whitespace().collect();
            let bt: Vec<&str> = b.split_whitespace().collect();
            if at.len() == bt.len() {
                let all_match = at.iter().zip(bt.iter()).all(|(ta, tb)| {
                    if ta == tb {
                        return true;
                    }
                    if let (Ok(fa), Ok(fb)) = (ta.parse::<f64>(), tb.parse::<f64>()) {
                        fuzzy_float_eq(fa, fb)
                    } else {
                        false
                    }
                });
                if all_match {
                    return true;
                }
            }
            // Fallback: extract all floats from both lines and compare
            let fa = extract_floats(a);
            let fb = extract_floats(b);
            if fa.len() == fb.len() && !fa.is_empty() {
                // Check non-float text is same
                let re = regex::Regex::new(r"-?\d+\.\d+(?:e[+-]?\d+)?").unwrap();
                let text_a = re.replace_all(a, "#").to_string();
                let text_b = re.replace_all(b, "#").to_string();
                if text_a == text_b {
                    return fa
                        .iter()
                        .zip(fb.iter())
                        .all(|(a, b)| fuzzy_float_eq(*a, *b));
                }
            }
            false
        };

        // Normalize signed zero: "-0" and "0" are equivalent in IEEE754
        let norm_zero = |s: &str| -> String {
            s.replace(" -0 ", " 0 ")
                .replace(" -0,", " 0,")
                .replace("= -0,", "= 0,")
                .replace("= -0\n", "= 0\n")
                .replace(" -0)", " 0)")
        };

        let exp_lines: Vec<String> = expected.lines().map(|l| norm_zero(l)).collect();
        let act_lines: Vec<String> = actual.lines().map(|l| norm_zero(l)).collect();
        // Check fuzzy match (all lines match with float tolerance)
        let all_fuzzy_match = exp_lines.len() == act_lines.len()
            && exp_lines
                .iter()
                .zip(act_lines.iter())
                .all(|(a, b)| fuzzy_line_match(a, b));

        if actual == expected || all_fuzzy_match {
            exec_match += 1;
        } else {
            let matching = exp_lines
                .iter()
                .zip(act_lines.iter())
                .filter(|(a, b)| fuzzy_line_match(a, b))
                .count();
            let total_lines = exp_lines.len().max(1);
            let pct = matching as f64 / total_lines as f64 * 100.0;
            if actual.is_empty() {
                mismatches.push((name.clone(), format!("EMPTY (exp {} lines)", total_lines)));
            } else {
                mismatches.push((
                    name.clone(),
                    format!("{}/{} lines match ({:.0}%)", matching, total_lines, pct),
                ));
                // Show diffs for near-100% tests (>75% match)
                if pct > 40.0 {
                    eprintln!("\n=== DIFF for {} ({}/{}) ===", name, matching, total_lines);
                    for (i, (e, a)) in exp_lines.iter().zip(act_lines.iter()).enumerate() {
                        if !fuzzy_line_match(e, a) {
                            eprintln!("  line {}: exp: {}", i + 1, e);
                            eprintln!("  line {}: got: {}", i + 1, a);
                        }
                    }
                    if act_lines.len() != exp_lines.len() {
                        eprintln!(
                            "  line count: exp={}, got={}",
                            exp_lines.len(),
                            act_lines.len()
                        );
                    }
                    eprintln!("=== END DIFF ===\n");
                }
            }
        }
    }

    // Count empty outputs
    let empty_count = mismatches
        .iter()
        .filter(|(_, d)| d.starts_with("EMPTY"))
        .count();

    eprintln!("\n══════════════════════════════════════════════════");
    eprintln!("  EXECUTION PARITY — {} ref shaders", total);
    eprintln!("══════════════════════════════════════════════════");
    eprintln!(
        "  Executed OK:    {}/{} ({:.1}%)",
        exec_ok,
        total,
        exec_ok as f64 / total as f64 * 100.0
    );
    eprintln!(
        "  Output match:   {}/{} ({:.1}%)",
        exec_match,
        exec_ok.max(1),
        exec_match as f64 / exec_ok.max(1) as f64 * 100.0
    );
    eprintln!("  Panicked:       {}", exec_panic);
    eprintln!("  No ref/out.txt: {}", no_ref);
    eprintln!(
        "  Mismatches:     {} (of which {} empty output)",
        mismatches.len(),
        empty_count
    );
    eprintln!("══════════════════════════════════════════════════");

    if !panics.is_empty() {
        eprintln!("\nExecution panics ({}):", panics.len());
        for p in panics.iter().take(20) {
            eprintln!("  - {}", p);
        }
    }
    if !mismatches.is_empty() {
        eprintln!("\nOutput mismatches ({}):", mismatches.len());
        for (name, detail) in mismatches.iter().take(80) {
            eprintln!("  - {}: {}", name, detail);
        }
        if mismatches.len() > 80 {
            eprintln!("  ... and {} more", mismatches.len() - 80);
        }
    }

    // For now, just require execution doesn't panic on most shaders
    let panic_rate = exec_panic as f64 / total as f64;
    assert!(
        panic_rate < 0.1,
        "Too many panics: {}/{}",
        exec_panic,
        total
    );
}

/// Parse run.py for grid size and pixelcenters flag.
/// Test parameters extracted from run.py
struct TestParams {
    xres: u32,
    yres: u32,
    pixelcenters: bool,
    raytype: i32,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            xres: 1,
            yres: 1,
            pixelcenters: false,
            raytype: 0,
        }
    }
}

fn parse_run_py(run_py_path: &std::path::Path) -> TestParams {
    if !run_py_path.exists() {
        return TestParams::default();
    }
    let run_py = std::fs::read_to_string(run_py_path).unwrap_or_default();
    let center = run_py.contains("-center");
    let (xres, yres) = if let Some(pos) = run_py.find("-g ") {
        let after = &run_py[pos + 3..];
        let parts: Vec<&str> = after.split_whitespace().collect();
        if parts.len() >= 2 {
            let x: u32 = parts[0].parse().unwrap_or(1);
            let y: u32 = parts[1].parse().unwrap_or(1);
            if x * y > 256 { (1u32, 1u32) } else { (x, y) }
        } else {
            (1, 1)
        }
    } else {
        (1, 1)
    };
    // Parse --raytype flags (can appear multiple times, OR the bits together)
    let mut raytype = 0i32;
    for cap in run_py.match_indices("--raytype ") {
        let after = &run_py[cap.0 + 10..];
        if let Some(name) = after.split_whitespace().next() {
            raytype |= match name {
                "camera" => 1,
                "shadow" => 2,
                "diffuse" => 4,
                "glossy" => 8,
                "reflection" => 16,
                "refraction" => 32,
                _ => 0,
            };
        }
    }
    TestParams {
        xres,
        yres,
        pixelcenters: center,
        raytype,
    }
}

/// Compute (u, v, dudx, dvdy) matching C++ testshade.
fn compute_uv(x: u32, y: u32, xres: u32, yres: u32, pixelcenters: bool) -> (f32, f32, f32, f32) {
    if pixelcenters {
        // Image mode: sample at pixel centers
        let u = (x as f32 + 0.5) / xres as f32;
        let v = (y as f32 + 0.5) / yres as f32;
        let dudx = 1.0 / xres as f32;
        let dvdy = 1.0 / yres as f32;
        (u, v, dudx, dvdy)
    } else {
        // Reyes mode: corners at 0,1
        let u = if xres == 1 {
            0.5f32
        } else {
            x as f32 / (xres - 1) as f32
        };
        let v = if yres == 1 {
            0.5f32
        } else {
            y as f32 / (yres - 1) as f32
        };
        let dudx = 1.0 / (xres as f32 - 1.0).max(1.0);
        let dvdy = 1.0 / (yres as f32 - 1.0).max(1.0);
        (u, v, dudx, dvdy)
    }
}

/// Set up ShaderGlobals matching C++ testshade.
fn setup_globals(x: u32, y: u32, params: &TestParams) -> osl_rs::ShaderGlobals {
    let (u, v, dudx, dvdy) = compute_uv(x, y, params.xres, params.yres, params.pixelcenters);
    let mut globals = osl_rs::ShaderGlobals::default();
    globals.u = u;
    globals.v = v;
    globals.dudx = dudx;
    globals.dvdy = dvdy;
    // P = (u, v, 1) matching C++ testshade SimpleRenderer
    globals.p = Vec3::new(u, v, 1.0);
    globals.dp_dx = Vec3::new(dudx, 0.0, 0.0);
    globals.dp_dy = Vec3::new(0.0, dvdy, 0.0);
    // N and Ng default to (0,0,1)
    globals.n = Vec3::new(0.0, 0.0, 1.0);
    globals.ng = Vec3::new(0.0, 0.0, 1.0);
    globals.raytype = params.raytype;
    globals
}

#[test]
#[ignore] // Requires _ref/OpenShadingLanguage testsuite (external dependency)
fn debug_single_ref_shader() {
    let test_name = std::env::var("OSL_TEST").unwrap_or_else(|_| "loop".to_string());
    let testsuite_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("_ref")
        .join("OpenShadingLanguage")
        .join("testsuite");
    let src_shaders = testsuite_dir.parent().unwrap().join("src").join("shaders");
    let common_shaders = testsuite_dir.join("common").join("shaders");
    let test_dir = testsuite_dir.join(test_name);
    let test_osl = test_dir.join("test.osl");
    let ref_out = test_dir.join("ref").join("out.txt");

    let src = std::fs::read_to_string(&test_osl).unwrap();
    let mut pp = osl_rs::preprocess::Preprocessor::new();
    pp.include_paths
        .push(test_dir.to_string_lossy().to_string());
    pp.include_paths
        .push(common_shaders.to_string_lossy().to_string());
    pp.include_paths
        .push(src_shaders.to_string_lossy().to_string());
    let preprocessed = pp
        .process_file(&src, &test_osl.to_string_lossy())
        .unwrap_or_else(|_| src.clone());

    let ast = osl_rs::parser::parse(&preprocessed).unwrap().ast;
    let mut ast_mut = ast;
    let _ = osl_rs::typecheck::typecheck(&mut ast_mut);
    let ir = osl_rs::codegen::generate(&ast_mut);

    eprintln!(
        "IR: {} symbols, {} opcodes",
        ir.symbols.len(),
        ir.opcodes.len()
    );

    // Determine grid and params from run.py
    let params = parse_run_py(&test_dir.join("run.py"));
    let (xres, yres) = (params.xres, params.yres);
    eprintln!(
        "\n=== Grid: {}x{}{}{} ===",
        xres,
        yres,
        if params.pixelcenters { " (center)" } else { "" },
        if params.raytype != 0 {
            format!(" raytype={}", params.raytype)
        } else {
            String::new()
        }
    );

    // Set up renderer with shader/object space transforms matching C++ testshade
    let renderer = {
        use osl_rs::math::Matrix44;
        let mut r = BasicRenderer::new();
        // Mshad: Identity → translate(1,0,0) → rotate(0,0,PI/4)
        let c = std::f32::consts::FRAC_1_SQRT_2; // cos(PI/4) = sin(PI/4)
        r.set_transform(
            "shader",
            Matrix44 {
                m: [
                    [c, c, 0.0, 0.0],
                    [-c, c, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0, 1.0], // Imath rotate() only affects rows 0-2
                ],
            },
        );
        // Mobj: Identity → translate(0,1,0) → rotate(0,0,PI/2)
        r.set_transform(
            "object",
            Matrix44 {
                m: [
                    [0.0, 1.0, 0.0, 0.0],
                    [-1.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0, 1.0], // Imath rotate() only affects rows 0-2
                ],
            },
        );
        Arc::new(r)
    };

    let mut actual = String::new();
    let mut seen_errors = std::collections::HashSet::new();
    for y in 0..yres {
        for x in 0..xres {
            let globals = setup_globals(x, y, &params);
            let mut interp = osl_rs::interp::Interpreter::new();
            interp.set_renderer(renderer.clone());
            interp.execute(&ir, &globals, None);
            // Dedup ERROR messages across grid points (C++ OSL deduplicates)
            for msg in &interp.messages {
                if msg.starts_with("ERROR:") {
                    if seen_errors.insert(msg.clone()) {
                        actual.push_str(msg);
                    }
                } else {
                    actual.push_str(msg);
                }
            }
        }
    }

    let expected_raw = std::fs::read_to_string(&ref_out).unwrap_or_default();
    let expected: String = expected_raw
        .lines()
        .filter(|l| {
            !l.starts_with("Compiled ")
                && !l.starts_with("FAILED ")
                && !l.starts_with("Output ")
                && !l.starts_with("Optimized ")
                && !l.contains(": warning:")
                && !l.starts_with("  Chosen ")
                && !l.starts_with("  Other ")
                && !l.starts_with("    test.osl:")
                && !l.contains("is forced llvm bool")
                && !l.starts_with("Connect ")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Normalize signed zero: "-0" and "0" are equivalent in IEEE754
    let norm_zero = |s: &str| -> String {
        s.replace(" -0 ", " 0 ")
            .replace(" -0,", " 0,")
            .replace("= -0,", "= 0,")
            .replace("= -0\n", "= 0\n")
    };
    // Show first divergence
    let exp_lines: Vec<String> = expected.trim().lines().map(|l| norm_zero(l)).collect();
    let act_lines: Vec<String> = actual.trim().lines().map(|l| norm_zero(l)).collect();
    let seq_matching = exp_lines
        .iter()
        .zip(act_lines.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let total_matching = exp_lines
        .iter()
        .zip(act_lines.iter())
        .filter(|(a, b)| a == b)
        .count();
    let matching = seq_matching;
    eprintln!(
        "\nMatch: {}/{} lines ({:.0}%) [total: {}/{} = {:.0}%]",
        matching,
        exp_lines.len(),
        matching as f64 / exp_lines.len().max(1) as f64 * 100.0,
        total_matching,
        exp_lines.len(),
        total_matching as f64 / exp_lines.len().max(1) as f64 * 100.0
    );
    if total_matching < exp_lines.len() {
        // Show ALL diffs
        eprintln!("\n=== ALL DIFFS ===");
        for i in 0..exp_lines.len().max(act_lines.len()) {
            let eof = "<EOF>".to_string();
            let e = exp_lines.get(i).unwrap_or(&eof);
            let a = act_lines.get(i).unwrap_or(&eof);
            if e != a {
                eprintln!("  {:4} EXP| {}", i + 1, e);
                eprintln!("  {:4} ACT| {}", i + 1, a);
            }
        }
    } else {
        eprintln!("PERFECT MATCH!");
    }
}

// ============================================================================
// End-to-end: struct compound initializer
// ============================================================================

#[test]
fn e2e_struct_compound_init() {
    // MyInfo s = {1.5, 2} should assign s.val=1.5, s.count=2
    // Use distinct names to avoid collision with OSL shader globals (e.g., "v" = texture coord)
    let src = r#"
struct MyInfo {
    float val;
    int count;
};

shader test() {
    MyInfo s = {1.5, 2};
    float result_val = s.val;
    int result_cnt = s.count;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let result_val = interp.get_float(&ir, "result_val").unwrap_or(-1.0);
    let result_cnt = interp.get_int(&ir, "result_cnt").unwrap_or(-1);
    assert!(
        (result_val - 1.5).abs() < 1e-6,
        "s.val should be 1.5, got {}",
        result_val
    );
    assert_eq!(result_cnt, 2, "s.count should be 2, got {}", result_cnt);
}

#[test]
fn e2e_struct_compound_init_color() {
    // Struct with a color field initialized from compound init
    let src = r#"
struct Surface {
    color diffuse;
    float roughness;
};

shader test() {
    Surface s = {{0.1, 0.5, 0.9}, 0.3};
    float r = s.diffuse[0];
    float g = s.diffuse[1];
    float b = s.diffuse[2];
    float rough = s.roughness;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let r = interp.get_float(&ir, "r").unwrap_or(-1.0);
    let g = interp.get_float(&ir, "g").unwrap_or(-1.0);
    let b = interp.get_float(&ir, "b").unwrap_or(-1.0);
    let rough = interp.get_float(&ir, "rough").unwrap_or(-1.0);
    assert!((r - 0.1).abs() < 1e-5, "diffuse.r should be 0.1, got {}", r);
    assert!((g - 0.5).abs() < 1e-5, "diffuse.g should be 0.5, got {}", g);
    assert!((b - 0.9).abs() < 1e-5, "diffuse.b should be 0.9, got {}", b);
    assert!(
        (rough - 0.3).abs() < 1e-5,
        "roughness should be 0.3, got {}",
        rough
    );
}

#[test]
fn e2e_struct_type_constructor() {
    // OSL type constructor syntax: MyStruct(field1, field2, ...)
    // This mirrors C++ `MyStruct s = MyStruct(1.5, 2);` (codegen.cpp:~1799)
    let src = r#"
struct Pair {
    float val;
    int count;
};

shader test() {
    Pair p = Pair(3.14, 7);
    float pval = p.val;
    int pcnt = p.count;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let pval = interp.get_float(&ir, "pval").unwrap_or(-1.0);
    let pcnt = interp.get_int(&ir, "pcnt").unwrap_or(-1);
    assert!(
        (pval - 3.14).abs() < 1e-5,
        "p.val should be 3.14, got {}",
        pval
    );
    assert_eq!(pcnt, 7, "p.count should be 7, got {}", pcnt);
}

#[test]
fn e2e_struct_type_constructor_color_field() {
    // Struct constructor with a color field
    let src = r#"
struct Mat {
    color albedo;
    float roughness;
};

shader test() {
    Mat m = Mat(color(0.2, 0.4, 0.8), 0.5);
    float r = m.albedo[0];
    float rough = m.roughness;
}
"#;
    let (ir, interp) = compile_and_run(src);
    let r = interp.get_float(&ir, "r").unwrap_or(-1.0);
    let rough = interp.get_float(&ir, "rough").unwrap_or(-1.0);
    assert!((r - 0.2).abs() < 1e-5, "albedo.r should be 0.2, got {}", r);
    assert!(
        (rough - 0.5).abs() < 1e-5,
        "roughness should be 0.5, got {}",
        rough
    );
}
