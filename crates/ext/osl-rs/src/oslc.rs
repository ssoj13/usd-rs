//! oslc — OSL compiler: reads .osl source, writes .oso bytecode.
//!
//! Port of `oslcmain.cpp`. Provides the full compilation pipeline:
//! source → lexer → parser → AST → typecheck → codegen → OSO writer.

use std::path::{Path, PathBuf};

use crate::codegen::{self, ShaderIR};
use crate::parser;
use crate::typecheck;

/// Compiler options.
#[derive(Debug, Clone)]
pub struct CompilerOptions {
    /// Output file path. If None, derives from input path.
    pub output: Option<PathBuf>,
    /// Include search paths.
    pub include_paths: Vec<PathBuf>,
    /// Preprocessor defines: name -> value.
    pub defines: Vec<(String, String)>,
    /// Optimization level (0, 1, 2).
    pub opt_level: u32,
    /// Verbose output.
    pub verbose: bool,
    /// Quiet mode (suppress warnings).
    pub quiet: bool,
    /// Debug output.
    pub debug: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            output: None,
            include_paths: Vec::new(),
            defines: Vec::new(),
            opt_level: 1,
            verbose: false,
            quiet: false,
            debug: false,
        }
    }
}

/// Compilation result.
#[derive(Debug)]
pub struct CompileResult {
    /// The compiled shader IR.
    pub ir: ShaderIR,
    /// The OSO bytecode as a string.
    pub oso_text: String,
    /// Any warnings generated during compilation.
    pub warnings: Vec<String>,
    /// Any errors generated during compilation.
    pub errors: Vec<String>,
    /// Whether compilation succeeded.
    pub success: bool,
}

/// Compile an OSL source string into OSO bytecode.
pub fn compile_string(source: &str, opts: &CompilerOptions) -> CompileResult {
    compile_string_internal(source, None, opts)
}

/// Compile an OSL file.
pub fn compile_file(path: &Path, opts: &CompilerOptions) -> CompileResult {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return CompileResult {
                ir: ShaderIR::default(),
                oso_text: String::new(),
                warnings: Vec::new(),
                errors: vec![format!("Cannot read file '{}': {}", path.display(), e)],
                success: false,
            };
        }
    };
    compile_string_internal(&source, Some(path), opts)
}

fn compile_string_internal(
    source: &str,
    filename: Option<&Path>,
    opts: &CompilerOptions,
) -> CompileResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Preprocess
    let preprocessed = {
        let mut pp = crate::preprocess::Preprocessor::new();
        for (name, value) in &opts.defines {
            pp.define_object(name, value);
        }
        for path in &opts.include_paths {
            pp.include_paths.push(path.to_string_lossy().to_string());
        }
        if let Some(file) = filename {
            if let Some(parent) = file.parent() {
                pp.include_paths.push(parent.to_string_lossy().to_string());
            }
            match pp.process_file(source, &file.to_string_lossy()) {
                Ok(text) => text,
                Err(pp_errors) => {
                    return CompileResult {
                        ir: ShaderIR::default(),
                        oso_text: String::new(),
                        warnings,
                        errors: pp_errors,
                        success: false,
                    };
                }
            }
        } else {
            match pp.process(source) {
                Ok(text) => text,
                Err(pp_errors) => {
                    return CompileResult {
                        ir: ShaderIR::default(),
                        oso_text: String::new(),
                        warnings,
                        errors: pp_errors,
                        success: false,
                    };
                }
            }
        }
    };

    // Parse
    let mut ast = match parser::parse(&preprocessed) {
        Ok(output) => {
            warnings.extend(output.warnings);
            output.ast
        }
        Err(parse_errors) => {
            for e in &parse_errors {
                errors.push(format!("{}", e));
            }
            return CompileResult {
                ir: ShaderIR::default(),
                oso_text: String::new(),
                warnings,
                errors,
                success: false,
            };
        }
    };

    // Type check
    let type_errors = typecheck::typecheck(&mut ast);
    for e in &type_errors {
        errors.push(format!("{}", e));
    }
    if !errors.is_empty() {
        return CompileResult {
            ir: ShaderIR::default(),
            oso_text: String::new(),
            warnings,
            errors,
            success: false,
        };
    }

    // Code generation
    let mut ir = codegen::generate(&ast);

    // Optimization passes based on opt_level
    if opts.opt_level >= 1 {
        codegen::propagate_derivs(&mut ir);
        codegen::fold_constants(&mut ir);
    }
    if opts.opt_level >= 2 {
        codegen::eliminate_dead_code(&mut ir);
    }

    // Generate OSO text
    let oso = ir_to_oso(&ir);

    CompileResult {
        ir,
        oso_text: oso,
        warnings,
        errors,
        success: true,
    }
}

/// Convert a ShaderIR to OSO text format.
///
/// The format follows the official OSL `.oso` specification:
/// ```text
/// OpenShadingLanguage <version>
/// <shader_type> <shader_name>
/// symtype <type> <name> [<value>]
///     ...
/// code <opname>
///     <args> [%jump ...]
///     ...
/// end
/// ```
/// Emit a ConstValue as OSO text (appended to `out`).
fn emit_const_value_oso(out: &mut String, cv: &crate::codegen::ConstValue) {
    use crate::codegen::ConstValue;
    match cv {
        ConstValue::Int(v) => out.push_str(&format!("{v}")),
        ConstValue::Float(v) => out.push_str(&format!("{}", format_float_g9(*v))),
        ConstValue::String(s) => out.push_str(&format!("\"{}\"", escape_oso_string(s.as_str()))),
        ConstValue::Vec3(v) => out.push_str(&format!(
            "{} {} {}",
            format_float_g9(v.x),
            format_float_g9(v.y),
            format_float_g9(v.z)
        )),
        ConstValue::Matrix(m) => {
            let mut first = true;
            for r in 0..4 {
                for c in 0..4 {
                    if !first {
                        out.push(' ');
                    }
                    first = false;
                    out.push_str(&format!("{}", format_float_g9(m.m[r][c])));
                }
            }
        }
        ConstValue::IntArray(arr) => {
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&format!("{v}"));
            }
        }
        ConstValue::FloatArray(arr) => {
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&format!("{}", format_float_g9(*v)));
            }
        }
        ConstValue::StringArray(arr) => {
            for (i, s) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&format!("\"{}\"", escape_oso_string(s.as_str())));
            }
        }
    }
}

fn escape_oso_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn trim_trailing_zeros(s: &str) -> String {
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

fn format_float_g9(v: f32) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = (v as f64).abs();
    let exp = abs.log10().floor() as i32;
    if exp < -4 || exp >= 9 {
        let s = format!("{:.9e}", v);
        if let Some(pos) = s.find('e') {
            let mant = trim_trailing_zeros(&s[..pos]);
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
        trim_trailing_zeros(&format!("{:.*}", prec, v))
    }
}

fn ir_to_oso(ir: &ShaderIR) -> String {
    use crate::symbol::SymType;

    let mut out = String::new();

    // OSO version line
    out.push_str(&format!(
        "OpenShadingLanguage {}.{:02}\n",
        crate::OSO_FILE_VERSION_MAJOR,
        crate::OSO_FILE_VERSION_MINOR
    ));
    out.push_str(&format!(
        "# Compiled by osl-rs {}\n",
        env!("CARGO_PKG_VERSION")
    ));

    // Shader type and name
    out.push_str(&format!("{} {}\n", ir.shader_type.name(), ir.shader_name));

    let n_syms = ir.symbols.len();
    let mut firstread = vec![i32::MAX; n_syms];
    let mut lastread = vec![-1i32; n_syms];
    let mut firstwrite = vec![i32::MAX; n_syms];
    let mut lastwrite = vec![-1i32; n_syms];
    let mut used = vec![false; n_syms];

    // Map opcode indices to output indices (skip empty ops)
    let mut op_index_map: Vec<Option<i32>> = vec![None; ir.opcodes.len()];
    let mut out_op_index = 0i32;
    for (op_idx, op) in ir.opcodes.iter().enumerate() {
        if op.op.as_str().is_empty() {
            continue;
        }
        op_index_map[op_idx] = Some(out_op_index);
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            if sym_idx >= n_syms {
                continue;
            }
            if op.is_arg_read(j as u32) {
                firstread[sym_idx] = firstread[sym_idx].min(out_op_index);
                lastread[sym_idx] = lastread[sym_idx].max(out_op_index);
                used[sym_idx] = true;
            }
            if op.is_arg_written(j as u32) {
                firstwrite[sym_idx] = firstwrite[sym_idx].min(out_op_index);
                lastwrite[sym_idx] = lastwrite[sym_idx].max(out_op_index);
                used[sym_idx] = true;
            }
        }
        out_op_index += 1;
    }

    // Symbols — emit in OSO format (skip unused non-interface symbols)
    let mut name_map: Vec<String> = vec![String::new(); n_syms];
    let mut const_id = 1;
    let mut temp_id = 1;

    for (idx, sym) in ir.symbols.iter().enumerate() {
        let must_emit = match sym.symtype {
            SymType::Param | SymType::OutputParam | SymType::Type | SymType::Function => true,
            SymType::Global => used[idx],
            SymType::Const | SymType::Temp | SymType::Local => used[idx],
        };
        if !must_emit {
            continue;
        }

        let out_name = match sym.symtype {
            SymType::Const => {
                let name = format!("$const{const_id}");
                const_id += 1;
                name
            }
            SymType::Temp => {
                let name = format!("$tmp{temp_id}");
                temp_id += 1;
                name
            }
            _ => sym.mangled(),
        };
        name_map[idx] = out_name.clone();

        out.push_str(&format!(
            "{}\t{}\t{}",
            sym.symtype.short_name(),
            sym.typespec,
            out_name
        ));

        // For constants, emit the value inline
        if sym.symtype == SymType::Const {
            if let Some((_, cv)) = ir.const_values.iter().find(|(cv_idx, _)| *cv_idx == idx) {
                out.push('\t');
                emit_const_value_oso(&mut out, cv);
                out.push('\t');
            }
        }

        // For params with defaults, emit the default value
        if sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam {
            if let Some((_, cv)) = ir.param_defaults.iter().find(|(pd_idx, _)| *pd_idx == idx) {
                out.push('\t');
                emit_const_value_oso(&mut out, cv);
                out.push('\t');
            }
        }

        let mut hints = 0;
        let push_hint = |h: &str, out: &mut String, hints: &mut i32| {
            if *hints == 0 {
                out.push('\t');
            } else {
                out.push(' ');
            }
            out.push_str(h);
            *hints += 1;
        };

        // %read and %write ranges
        push_hint(
            &format!(
                "%read{{{},{}}} %write{{{},{}}}",
                firstread[idx], lastread[idx], firstwrite[idx], lastwrite[idx]
            ),
            &mut out,
            &mut hints,
        );

        // %meta{type,name,value} from AST [[ metadata ]]
        for (mtype, mname, mval) in &sym.metadata {
            let needs_quotes = mtype == "string" || mval.contains(' ');
            let meta_str = if needs_quotes {
                format!("%meta{{{mtype},{mname},\"{mval}\"}}")
            } else {
                format!("%meta{{{mtype},{mname},{mval}}}")
            };
            push_hint(&meta_str, &mut out, &mut hints);
        }

        // %derivs hint marks symbols that need to carry derivatives
        if sym.has_derivs {
            push_hint("%derivs", &mut out, &mut hints);
        }

        // %struct, %structfields, %structfieldtypes, %structnfields
        // C++ oslcomp.cpp:780-796: document struct definition and field names
        if sym.typespec.is_structure() {
            let sid = sym.typespec.structure_id();
            if let Some(spec) = crate::typespec::get_struct(sid as i32) {
                let mut fieldlist = String::new();
                let mut signature = String::new();
                for i in 0..spec.num_fields() {
                    if i > 0 {
                        fieldlist.push(',');
                    }
                    fieldlist.push_str(spec.field(i).name.as_str());
                    signature.push_str(&spec.field(i).type_spec.code_from_type());
                }
                push_hint(
                    &format!(
                        "%struct{{\"{}\"}} %structfields{{{}}} %structfieldtypes{{\"{}\"}} %structnfields{{{}}}",
                        spec.mangled(),
                        fieldlist,
                        signature,
                        spec.num_fields()
                    ),
                    &mut out,
                    &mut hints,
                );
            }
        }

        // %initexpr for params with init ops
        if (sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam)
            && sym.has_init_ops()
        {
            push_hint("%initexpr", &mut out, &mut hints);
        }

        out.push('\n');
    }

    // Opcodes
    let mut last_method = String::new();
    let mut last_file = String::new();
    let mut last_line: i32 = -1;
    let mut any_op = false;
    for (op_idx, op) in ir.opcodes.iter().enumerate() {
        let opname = op.op.as_str();
        if opname.is_empty() {
            continue;
        }
        any_op = true;

        let method = op.method.as_str();
        if method != last_method {
            out.push_str(&format!("code {}\n", method));
            last_method = method.to_string();
        }

        out.push('\t');
        out.push_str(opname);

        // Args
        let nargs = op.nargs as usize;
        let firstarg = op.firstarg as usize;
        for j in 0..nargs {
            if firstarg + j >= ir.args.len() {
                break;
            }
            let sym_idx = ir.args[firstarg + j] as usize;
            if sym_idx >= n_syms {
                continue;
            }
            out.push(' ');
            if !name_map[sym_idx].is_empty() {
                out.push_str(&name_map[sym_idx]);
            } else {
                let fallback = ir.symbols[sym_idx].mangled();
                out.push_str(&fallback);
            }
        }

        // Jumps
        for &j in &op.jump {
            if j >= 0 {
                let mapped = if (j as usize) < op_index_map.len() {
                    op_index_map[j as usize].unwrap_or(out_op_index)
                } else {
                    out_op_index
                };
                out.push_str(&format!(" ${mapped}"));
            }
        }

        // Hints
        let mut firsthint = true;
        if !op.sourcefile.as_str().is_empty() {
            if op.sourcefile.as_str() != last_file {
                last_file = op.sourcefile.as_str().to_string();
                out.push_str(&format!(
                    "{}%filename{{\"{}\"}}",
                    if firsthint { '\t' } else { ' ' },
                    escape_oso_string(&last_file)
                ));
                firsthint = false;
            }
            if op.sourceline != last_line {
                last_line = op.sourceline;
                out.push_str(&format!(
                    "{}%line{{{}}}",
                    if firsthint { '\t' } else { ' ' },
                    last_line
                ));
                firsthint = false;
            }
        }

        if op.nargs > 0 {
            let mut rw = String::with_capacity(nargs);
            for j in 0..nargs {
                let r = op.is_arg_read(j as u32);
                let w = op.is_arg_written(j as u32);
                let ch = if w {
                    if r { 'W' } else { 'w' }
                } else {
                    if r { 'r' } else { '-' }
                };
                rw.push(ch);
            }
            out.push_str(&format!(
                "{}%argrw{{\"{}\"}}",
                if firsthint { '\t' } else { ' ' },
                rw
            ));
        }

        if op.argtakesderivs != 0 {
            let mut any = false;
            let mut list = String::new();
            for j in 0..nargs {
                if op.arg_takes_derivs(j as u32) {
                    if any {
                        list.push(',');
                    }
                    any = true;
                    list.push_str(&j.to_string());
                }
            }
            if any {
                out.push_str(&format!(" %argderivs{{{}}}", list));
            }
        }

        out.push('\n');

        let _ = op_idx;
    }

    if !any_op {
        out.push_str("code ___main___\n");
    }
    out.push_str("\tend\n");
    out
}

/// Parse command-line arguments for the oslc compiler.
pub fn parse_args(args: &[String]) -> Result<(PathBuf, CompilerOptions), String> {
    let mut opts = CompilerOptions::default();
    let mut input = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i >= args.len() {
                    return Err("-o requires an argument".into());
                }
                opts.output = Some(PathBuf::from(&args[i]));
            }
            "-I" => {
                i += 1;
                if i >= args.len() {
                    return Err("-I requires an argument".into());
                }
                opts.include_paths.push(PathBuf::from(&args[i]));
            }
            "-D" => {
                i += 1;
                if i >= args.len() {
                    return Err("-D requires an argument".into());
                }
                let def = &args[i];
                if let Some(eq) = def.find('=') {
                    opts.defines
                        .push((def[..eq].to_string(), def[eq + 1..].to_string()));
                } else {
                    opts.defines.push((def.clone(), "1".to_string()));
                }
            }
            "-O0" => opts.opt_level = 0,
            "-O1" | "-O" => opts.opt_level = 1,
            "-O2" => opts.opt_level = 2,
            "-v" => opts.verbose = true,
            "-q" => opts.quiet = true,
            "-d" => opts.debug = true,
            s if !s.starts_with('-') => {
                input = Some(PathBuf::from(s));
            }
            other => {
                return Err(format!("Unknown option: {}", other));
            }
        }
        i += 1;
    }

    match input {
        Some(path) => Ok((path, opts)),
        None => Err("No input file specified".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple() {
        let src = r#"
surface test(float Kd = 0.5) {
    Ci = Kd * diffuse(N);
}
"#;
        let result = compile_string(src, &CompilerOptions::default());
        // Parser should succeed
        assert!(result.success, "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_compile_empty_shader() {
        let src = "shader empty() {}";
        let result = compile_string(src, &CompilerOptions::default());
        assert!(result.success);
        assert!(result.oso_text.contains("OpenShadingLanguage"));
    }

    #[test]
    fn test_compile_syntax_error() {
        let src = "surface test( {{{";
        let result = compile_string(src, &CompilerOptions::default());
        assert!(!result.success);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_parse_args_basic() {
        let args: Vec<String> = vec!["-o", "out.oso", "-v", "input.osl"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let (path, opts) = parse_args(&args).unwrap();
        assert_eq!(path, PathBuf::from("input.osl"));
        assert_eq!(opts.output, Some(PathBuf::from("out.oso")));
        assert!(opts.verbose);
    }

    #[test]
    fn test_parse_args_no_input() {
        let args: Vec<String> = vec!["-o", "out.oso"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_oso_output_format() {
        let src = "shader test(float a = 1.0) {}";
        let result = compile_string(src, &CompilerOptions::default());
        assert!(result.success);
        assert!(result.oso_text.starts_with("OpenShadingLanguage"));
        assert!(result.oso_text.contains("end"));
    }

    #[test]
    fn test_compile_param_metadata_emits_oso_meta() {
        // AST metadata [[ int lockgeom = 0 ]] must flow to OSO %meta
        let src = r#"
shader test(float Kd = 0.5 [[ int lockgeom = 0 ]]) {
    float x = Kd;
}
"#;
        let result = compile_string(src, &CompilerOptions::default());
        assert!(result.success, "compile failed: {:?}", result.errors);
        let expected = "%meta{int,lockgeom,0}";
        assert!(
            result.oso_text.contains(expected),
            "OSO should contain {}, got: {}",
            expected,
            &result.oso_text[..result.oso_text.len().min(800)]
        );
    }
}
