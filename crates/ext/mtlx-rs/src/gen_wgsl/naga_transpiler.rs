//! Naga-based GLSL 450 -> WGSL transpiler.
//!
//! Takes Vulkan GLSL source (as produced by VkShaderGenerator) and converts
//! it to native WGSL via naga's GLSL frontend and WGSL backend.
//!
//! Includes a lightweight preprocessor to expand MaterialX `#define` type
//! aliases (e.g. `#define EDF vec3`) that naga's GLSL frontend cannot handle.

use naga::back::wgsl as wgsl_back;
use naga::front::glsl as glsl_front;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::collections::HashMap;

/// Shader stage hint for the GLSL parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}

/// State for one level of #if/#elif/#else/#endif nesting.
struct CondState {
    /// Are we currently emitting lines in this block?
    emitting: bool,
    /// Has any branch in this #if chain been taken?
    any_taken: bool,
    /// Was the parent scope active when this #if was entered?
    parent_active: bool,
}

/// Evaluate a simple `#if NAME == VALUE` or `#if NAME != VALUE` condition.
/// Returns false for unrecognized expressions (conservative: exclude unknown code).
fn eval_pp_condition(expr: &str, int_defines: &HashMap<String, i64>) -> bool {
    // Handle: NAME == VALUE
    if let Some((lhs, rhs)) = expr.split_once("==") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        let lval = resolve_pp_value(lhs, int_defines);
        let rval = resolve_pp_value(rhs, int_defines);
        return lval == rval;
    }
    // Handle: NAME != VALUE
    if let Some((lhs, rhs)) = expr.split_once("!=") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        let lval = resolve_pp_value(lhs, int_defines);
        let rval = resolve_pp_value(rhs, int_defines);
        return lval != rval;
    }
    // Handle: bare `#if NAME` (true if defined and non-zero)
    let name = expr.trim();
    int_defines.get(name).map_or(false, |v| *v != 0)
}

/// Resolve a preprocessor token to an integer value.
fn resolve_pp_value(token: &str, int_defines: &HashMap<String, i64>) -> Option<i64> {
    if let Ok(v) = token.parse::<i64>() {
        return Some(v);
    }
    int_defines.get(token).copied()
}

impl From<ShaderStage> for naga::ShaderStage {
    fn from(s: ShaderStage) -> Self {
        match s {
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
        }
    }
}

/// Error from the GLSL -> WGSL transpilation pipeline.
#[derive(Debug)]
pub enum TranspileError {
    /// naga GLSL parser failed
    Parse(glsl_front::ParseErrors),
    /// naga validation failed
    Validation(naga::WithSpan<naga::valid::ValidationError>),
    /// naga WGSL writer failed
    WgslWrite(wgsl_back::Error),
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(errors) => write!(f, "GLSL parse errors: {errors}"),
            Self::Validation(e) => write!(f, "naga validation: {e}"),
            Self::WgslWrite(e) => write!(f, "WGSL write: {e}"),
        }
    }
}

impl std::error::Error for TranspileError {}

/// Transpile Vulkan GLSL 450 source into WGSL.
///
/// `stage` hints the GLSL parser about the expected entry point type.
/// Runs a lightweight preprocessor first to expand MaterialX `#define`
/// type aliases, then feeds clean GLSL to naga.
pub fn glsl_to_wgsl(glsl_source: &str, stage: ShaderStage) -> Result<String, TranspileError> {
    // 0. Preprocess: expand #define macros that naga can't handle
    let preprocessed = preprocess_mtlx_glsl(glsl_source);

    // 1. Parse GLSL -> naga Module
    let opts = glsl_front::Options {
        stage: stage.into(),
        defines: Default::default(),
    };
    let module = glsl_front::Frontend::default()
        .parse(&opts, &preprocessed)
        .map_err(TranspileError::Parse)?;

    // 2. Validate module (required before WGSL emission)
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(TranspileError::Validation)?;

    // 3. Emit WGSL
    let wgsl = wgsl_back::write_string(&module, &info, wgsl_back::WriterFlags::empty())
        .map_err(TranspileError::WgslWrite)?;

    Ok(wgsl)
}

/// Lightweight GLSL preprocessor for MaterialX-generated code.
///
/// naga's GLSL frontend does not support preprocessor directives. This handles:
/// - `#define NAME VALUE` type aliases → whole-word text substitution
/// - `#define NAME 42` integer constants → `const int NAME = 42;`
/// - `#define x bool(x)` boolean casts → stripped (naga handles int in conditionals)
/// - `#pragma` directives → stripped (stage is set via naga options)
/// - `#include` directives → stripped (unresolved includes; code should be inlined)
/// - `#if`/`#elif`/`#else`/`#endif` conditionals → evaluated using known integer defines
/// - `in/out InterfaceBlock { ... } name;` → flattened to individual in/out variables
pub fn preprocess_mtlx_glsl(source: &str) -> String {
    let mut defines: HashMap<String, String> = HashMap::new();
    // Integer define values for #if conditional evaluation
    let mut int_defines: HashMap<String, i64> = HashMap::new();
    // Instance name rewrites: "vd.texcoord_0" → "texcoord_0"
    let mut instance_rewrites: Vec<(String, String)> = Vec::new();
    let mut output_lines: Vec<String> = Vec::new();
    // Conditional (#if/#elif/#else/#endif) stack: true = emitting lines
    let mut cond_stack: Vec<CondState> = Vec::new();
    // Bool uniforms inside blocks: converted to int for WGSL host-shareability.
    // After preprocessing, usages are wrapped with `(name != 0)` for bool conversion.
    let mut bool_uniform_names: Vec<String> = Vec::new();
    // Tracks whether we're currently inside a `uniform { ... }` block.
    let mut in_uniform_block = false;

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Evaluate #if/#elif/#else/#endif conditionals
        if let Some(rest) = trimmed.strip_prefix("#if ") {
            let active = cond_stack.last().map_or(true, |s| s.emitting);
            let result = active && eval_pp_condition(rest.trim(), &int_defines);
            cond_stack.push(CondState {
                emitting: result,
                any_taken: result,
                parent_active: active,
            });
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#elif ") {
            if let Some(state) = cond_stack.last_mut() {
                if state.parent_active && !state.any_taken {
                    let result = eval_pp_condition(rest.trim(), &int_defines);
                    state.emitting = result;
                    if result {
                        state.any_taken = true;
                    }
                } else {
                    state.emitting = false;
                }
            }
            i += 1;
            continue;
        }
        if trimmed == "#else" {
            if let Some(state) = cond_stack.last_mut() {
                state.emitting = state.parent_active && !state.any_taken;
            }
            i += 1;
            continue;
        }
        if trimmed == "#endif" {
            cond_stack.pop();
            i += 1;
            continue;
        }

        // If inside a false conditional branch, skip this line
        if cond_stack.last().map_or(false, |s| !s.emitting) {
            i += 1;
            continue;
        }

        // Strip #pragma directives (naga gets stage from options)
        if trimmed.starts_with("#pragma") {
            i += 1;
            continue;
        }

        // Strip #extension directives (naga doesn't support these)
        if trimmed.starts_with("#extension") {
            i += 1;
            continue;
        }

        // Strip #include directives (should have been inlined by codegen)
        if trimmed.starts_with("#include") {
            i += 1;
            continue;
        }

        // naga GLSL supports `layout(binding=N) uniform texture2D` and `uniform sampler`
        // natively with sampler2D() constructor — pass through as-is.

        // Handle #define
        if let Some(rest) = trimmed.strip_prefix("#define ") {
            let mut parts = rest.splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("").trim();

            if name.is_empty() {
                output_lines.push(line.to_string());
                i += 1;
                continue;
            }

            // Skip self-referencing bool cast defines: `#define x bool(x)`
            // naga handles int→bool implicitly in conditionals.
            if value.starts_with("bool(") && value.ends_with(')') {
                i += 1;
                continue;
            }

            // Integer constant: `#define NAME 42` → `const int NAME = 42;`
            // Also store in int_defines for #if conditional evaluation.
            if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(v) = value.parse::<i64>() {
                    int_defines.insert(name.to_string(), v);
                }
                output_lines.push(format!("const int {} = {};", name, value));
                i += 1;
                continue;
            }

            // Float constant: `#define M_PI 3.14159...` → `const float M_PI = 3.14159...;`
            // Emitting as a real const avoids HashMap-order-dependent substitution bugs
            // when another define references this one (e.g. M_PI_INV = 1.0/M_PI).
            if value.contains('.') && value.parse::<f64>().is_ok() {
                output_lines.push(format!("const float {} = {};", name, value));
                i += 1;
                continue;
            }

            // Type alias: `#define EDF vec3` → record for substitution
            if !value.is_empty() {
                defines.insert(name.to_string(), value.to_string());
                i += 1;
                continue;
            }

            // Bare `#define NAME` — just remove
            i += 1;
            continue;
        }

        // Flatten interface blocks: `layout(...) in/out BlockName { ... } inst;`
        // naga doesn't support GLSL interface blocks — flatten to individual in/out vars.
        if let Some(flattened) = try_flatten_interface_block(&lines, i) {
            for flat_line in &flattened.lines {
                output_lines.push(flat_line.clone());
            }
            // Register instance.member → member rewrites
            if !flattened.instance_name.is_empty() {
                for (_, member_name) in &flattened.members {
                    let from = format!("{}.{}", flattened.instance_name, member_name);
                    instance_rewrites.push((from, member_name.clone()));
                }
            }
            i = flattened.end_line;
            continue;
        }

        // Track uniform block scope to detect bool members.
        // The header `layout (...) uniform Name` may be on a separate line from `{`.
        if trimmed.contains("uniform") && !trimmed.starts_with("//") {
            in_uniform_block = true;
        }
        if in_uniform_block && trimmed.starts_with('}') {
            in_uniform_block = false;
        }

        // Convert `bool varname;` inside uniform blocks to `int varname;`.
        // WGSL uniform buffers only allow host-shareable types (no bool).
        if in_uniform_block {
            if let Some(rest) = trimmed.strip_prefix("bool ") {
                let var_name = rest.trim_end_matches(';').trim();
                if !var_name.is_empty() {
                    bool_uniform_names.push(var_name.to_string());
                    output_lines.push(line.replace("bool ", "int "));
                    i += 1;
                    continue;
                }
            }
        }

        output_lines.push(line.to_string());
        i += 1;
    }

    // Replace bool uniform usages with `(name != 0)` for int→bool conversion.
    // Only applied outside uniform block declarations (which already have `int`).
    if !bool_uniform_names.is_empty() {
        output_lines = output_lines
            .into_iter()
            .map(|line| {
                // Skip uniform block member declarations (contain `int name;`)
                let t = line.trim();
                if t.starts_with("int ") && t.ends_with(';') {
                    return line;
                }
                let mut r = line;
                for name in &bool_uniform_names {
                    r = replace_whole_word(&r, name, &format!("({} != 0)", name));
                }
                r
            })
            .collect();
    }

    // Apply instance.member → member rewrites (e.g. vd.texcoord_0 → texcoord_0)
    if !instance_rewrites.is_empty() {
        output_lines = output_lines
            .into_iter()
            .map(|line| {
                let mut r = line;
                for (from, to) in &instance_rewrites {
                    r = r.replace(from, to);
                }
                r
            })
            .collect();
    }

    // Apply whole-word substitution for all type aliases
    if !defines.is_empty() {
        output_lines = output_lines
            .into_iter()
            .map(|line| {
                let mut result = line;
                for (name, replacement) in &defines {
                    result = replace_whole_word(&result, name, replacement);
                }
                result
            })
            .collect();
    }

    output_lines.join("\n")
}

/// Result of flattening an interface block.
struct FlattenedBlock {
    lines: Vec<String>,
    /// (type, name) pairs of flattened members
    members: Vec<(String, String)>,
    /// Instance name (e.g. "vd") for rewriting `vd.member` → `member`
    instance_name: String,
    /// Line index to resume parsing from (past the closing `};`)
    end_line: usize,
}

/// Try to detect and flatten a GLSL interface block starting at `start`.
///
/// Matches patterns like:
/// ```glsl
/// layout (location = 0) in VertexData
/// {
///     vec2 texcoord_0;
/// } vd;
/// ```
/// Flattens to:
/// ```glsl
/// layout(location = 0) in vec2 texcoord_0;
/// ```
/// Also stores the instance name for later substitution (`vd.texcoord_0` → `texcoord_0`).
fn try_flatten_interface_block(lines: &[&str], start: usize) -> Option<FlattenedBlock> {
    let header = lines[start].trim();

    // Match: layout(...) in/out BLOCKNAME
    // We look for "in" or "out" qualifier with a block name (not a simple variable)
    let (qualifier, _loc_str) = parse_io_block_header(header)?;

    // Find opening brace — may be on same line or next
    let mut brace_line = start;
    if !header.contains('{') {
        brace_line = start + 1;
        if brace_line >= lines.len() || !lines[brace_line].trim().starts_with('{') {
            return None;
        }
    }

    // Collect member declarations until closing `} instance_name;`
    let mut members: Vec<(String, String)> = Vec::new(); // (type, name)
    let mut instance_name = String::new();
    let mut end = brace_line + 1;

    while end < lines.len() {
        let member_line = lines[end].trim();

        // Closing brace: `} vd;` or `};`
        if member_line.starts_with('}') {
            // Extract instance name if present: `} vd;` → "vd"
            let after_brace = member_line.trim_start_matches('}').trim();
            let inst = after_brace.trim_end_matches(';').trim();
            if !inst.is_empty() {
                instance_name = inst.to_string();
            }
            end += 1;
            break;
        }

        // Parse member: `vec2 texcoord_0;`
        let member = member_line.trim_end_matches(';').trim();
        if let Some(space_pos) = member.rfind(char::is_whitespace) {
            let ty = member[..space_pos].trim();
            let name = member[space_pos..].trim();
            if !ty.is_empty() && !name.is_empty() {
                members.push((ty.to_string(), name.to_string()));
            }
        }

        end += 1;
    }

    if members.is_empty() {
        return None;
    }

    // Emit flattened individual in/out declarations with sequential locations
    let mut result_lines = Vec::new();
    for (loc_idx, (ty, name)) in members.iter().enumerate() {
        result_lines.push(format!(
            "layout(location = {}) {} {} {};",
            loc_idx, qualifier, ty, name
        ));
    }

    Some(FlattenedBlock {
        lines: result_lines,
        members,
        instance_name,
        end_line: end,
    })
}

/// Parse an interface block header like `layout (location = 0) in VertexData`.
/// Returns (qualifier, location_string) if this looks like an IO interface block.
fn parse_io_block_header(header: &str) -> Option<(&str, &str)> {
    // Must start with 'layout' — distinguishes from function signatures
    if !header.starts_with("layout") {
        return None;
    }
    // Must contain 'in' or 'out' keyword
    let has_in = header.contains(" in ");
    let has_out = header.contains(" out ");
    if !has_in && !has_out {
        return None;
    }
    // Must NOT end with semicolon (that's a simple variable declaration)
    if header.contains(';') {
        return None;
    }
    // Must not be a uniform block
    if header.contains("uniform") {
        return None;
    }
    // The part after `in`/`out` must be a bare block name (no parens = not a function)
    let keyword = if has_in { " in " } else { " out " };
    let after_keyword = header.split(keyword).last().unwrap_or("");
    if after_keyword.contains('(') {
        return None;
    }

    let qualifier = if has_in { "in" } else { "out" };
    Some((qualifier, ""))
}

/// Replace whole-word occurrences of `word` with `replacement`.
/// A word boundary is a transition between identifier and non-identifier chars.
fn replace_whole_word(source: &str, word: &str, replacement: &str) -> String {
    if word.is_empty() || !source.contains(word) {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let src_bytes = source.as_bytes();
    let word_bytes = word.as_bytes();
    let word_len = word_bytes.len();
    let src_len = src_bytes.len();
    let mut i = 0;

    while i < src_len {
        if i + word_len <= src_len && &src_bytes[i..i + word_len] == word_bytes {
            // Check word boundaries
            let before_ok = i == 0 || !is_ident_char(src_bytes[i - 1]);
            let after_ok = i + word_len >= src_len || !is_ident_char(src_bytes[i + word_len]);
            if before_ok && after_ok {
                result.push_str(replacement);
                i += word_len;
                continue;
            }
        }
        result.push(src_bytes[i] as char);
        i += 1;
    }

    result
}

/// Check if byte is a valid GLSL identifier character.
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_vertex_transpile() {
        let glsl = r#"#version 450
layout(location = 0) in vec3 a_position;
layout(location = 0) out vec4 v_color;
void main() {
    gl_Position = vec4(a_position, 1.0);
    v_color = vec4(1.0, 0.0, 0.0, 1.0);
}
"#;
        let wgsl = glsl_to_wgsl(glsl, ShaderStage::Vertex).expect("transpile failed");
        assert!(wgsl.contains("fn main"), "should contain WGSL entry point");
        assert!(
            !wgsl.contains("#version"),
            "should not contain GLSL directives"
        );
    }

    #[test]
    fn test_simple_fragment_transpile() {
        let glsl = r#"#version 450
layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 fragColor;
void main() {
    fragColor = v_color;
}
"#;
        let wgsl = glsl_to_wgsl(glsl, ShaderStage::Fragment).expect("transpile failed");
        assert!(wgsl.contains("fn main"), "should contain WGSL entry point");
        assert!(
            !wgsl.contains("#version"),
            "should not contain GLSL directives"
        );
    }

    #[test]
    fn test_uniform_block_transpile() {
        let glsl = r#"#version 450
layout(std140, binding = 0) uniform MaterialParams {
    vec4 base_color;
    float roughness;
};
layout(location = 0) out vec4 fragColor;
void main() {
    fragColor = base_color * roughness;
}
"#;
        let wgsl = glsl_to_wgsl(glsl, ShaderStage::Fragment).expect("transpile failed");
        // naga should produce @group/@binding annotations
        assert!(
            wgsl.contains("@group") || wgsl.contains("@binding") || wgsl.contains("var<uniform>"),
            "should contain WGSL uniform binding syntax, got:\n{wgsl}"
        );
    }

    #[test]
    fn test_texture_sampler_transpile() {
        let glsl = r#"#version 450
layout(binding = 0) uniform texture2D tex_texture;
layout(binding = 1) uniform sampler tex_sampler;
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 fragColor;
void main() {
    fragColor = texture(sampler2D(tex_texture, tex_sampler), v_uv);
}
"#;
        let wgsl = glsl_to_wgsl(glsl, ShaderStage::Fragment).expect("transpile failed");
        assert!(
            wgsl.contains("texture_2d")
                || wgsl.contains("textureSample")
                || wgsl.contains("sampler"),
            "should contain WGSL texture sampling, got:\n{wgsl}"
        );
    }

    #[test]
    fn test_invalid_glsl_returns_error() {
        let bad_glsl = "this is not valid glsl";
        assert!(glsl_to_wgsl(bad_glsl, ShaderStage::Fragment).is_err());
    }

    // --- Preprocessor tests ---

    #[test]
    fn preprocess_expands_type_alias() {
        let src = "#version 450\n#define EDF vec3\nEDF emission = EDF(0.0);\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(!out.contains("#define"), "defines should be removed");
        assert!(
            out.contains("vec3 emission = vec3(0.0);"),
            "EDF should expand to vec3, got:\n{out}"
        );
    }

    #[test]
    fn preprocess_expands_chained_alias() {
        // material -> surfaceshader (struct already declared)
        let src = "#version 450\n\
            struct surfaceshader { vec3 color; vec3 transparency; };\n\
            #define material surfaceshader\n\
            material out1 = material(vec3(0.0), vec3(0.0));\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(
            out.contains("surfaceshader out1 = surfaceshader("),
            "material should expand to surfaceshader, got:\n{out}"
        );
    }

    #[test]
    fn preprocess_integer_constant_becomes_const() {
        let src = "#version 450\n#define MAX_LIGHTS 8\nvoid main() {}\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(out.contains("const int MAX_LIGHTS = 8;"), "got:\n{out}");
        assert!(!out.contains("#define"));
    }

    #[test]
    fn preprocess_drops_bool_cast_define() {
        let src = "#version 450\n#define use_flag bool(use_flag)\nvoid main() {}\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(
            !out.contains("#define"),
            "bool cast define should be removed"
        );
        assert!(!out.contains("bool(use_flag)"), "should not inject cast");
    }

    #[test]
    fn preprocess_whole_word_boundary() {
        // EDF should not match inside SEDF or EDF_val
        let src = "#version 450\n#define EDF vec3\nSEDF x; EDF y; EDF_val z;\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(out.contains("SEDF x;"), "SEDF should be untouched");
        assert!(out.contains("vec3 y;"), "standalone EDF should expand");
        assert!(out.contains("EDF_val z;"), "EDF_val should be untouched");
    }

    #[test]
    fn preprocess_no_defines_passthrough() {
        let src = "#version 450\nvoid main() { gl_FragColor = vec4(1.0); }\n";
        let out = preprocess_mtlx_glsl(src);
        assert_eq!(
            out,
            "#version 450\nvoid main() { gl_FragColor = vec4(1.0); }"
        );
    }

    #[test]
    fn preprocess_materialx_structs_with_defines_transpiles() {
        // Full MaterialX-like GLSL with struct + #define + usage
        let glsl = r#"#version 450
struct BSDF { vec3 response; vec3 throughput; };
#define EDF vec3
struct surfaceshader { vec3 color; vec3 transparency; };
#define material surfaceshader
layout(location = 0) out vec4 fragColor;
void main() {
    EDF emission = EDF(0.0);
    BSDF bsdf = BSDF(vec3(1.0), vec3(1.0));
    surfaceshader ss = surfaceshader(bsdf.response + emission, bsdf.throughput);
    material out1 = ss;
    fragColor = vec4(out1.color, 1.0);
}
"#;
        let wgsl = glsl_to_wgsl(glsl, ShaderStage::Fragment)
            .expect("MaterialX-style GLSL should transpile");
        assert!(wgsl.contains("fn main"), "should produce WGSL entry point");
        assert!(
            !wgsl.contains("#define"),
            "no preprocessor directives in WGSL"
        );
    }

    #[test]
    fn naga_handles_out_parameter() {
        let glsl = "#version 450\nlayout(location = 0) out vec4 fragColor;\nvoid compute_val(float x, out float result) {\n    result = x * 2.0;\n}\nvoid main() {\n    float val = 0.0;\n    compute_val(1.0, val);\n    fragColor = vec4(val, 0.0, 0.0, 1.0);\n}\n";
        match glsl_to_wgsl(glsl, ShaderStage::Fragment) {
            Ok(wgsl) => eprintln!("[OK] out param:\n{}", wgsl),
            Err(e) => panic!("naga cannot handle out param: {}", e),
        }
    }

    #[test]
    fn naga_handles_texture2d_param() {
        let glsl = "#version 450\nlayout(binding = 0) uniform texture2D tex;\nlayout(binding = 1) uniform sampler samp;\nlayout(location = 0) in vec2 uv;\nlayout(location = 0) out vec4 fragColor;\nfloat sample_tex(texture2D t, sampler s, vec2 coord) {\n    return texture(sampler2D(t, s), coord).r;\n}\nvoid main() {\n    fragColor = vec4(sample_tex(tex, samp, uv));\n}\n";
        match glsl_to_wgsl(glsl, ShaderStage::Fragment) {
            Ok(wgsl) => eprintln!("[OK] texture2D param:\n{}", wgsl),
            Err(e) => panic!("naga cannot handle texture2D param: {}", e),
        }
    }

    #[test]
    fn naga_handles_texture2d_out_combined() {
        let glsl = "#version 450\nlayout(binding = 0) uniform texture2D tex;\nlayout(binding = 1) uniform sampler samp;\nlayout(location = 0) in vec2 uv;\nlayout(location = 0) out vec4 fragColor;\nvoid sample_tex(texture2D t, sampler s, vec2 coord, out float result) {\n    result = texture(sampler2D(t, s), coord).r;\n}\nvoid main() {\n    float val = 0.0;\n    sample_tex(tex, samp, uv, val);\n    fragColor = vec4(val, 0.0, 0.0, 1.0);\n}\n";
        match glsl_to_wgsl(glsl, ShaderStage::Fragment) {
            Ok(wgsl) => eprintln!("[OK] combined tex+out:\n{}", wgsl),
            Err(e) => panic!("naga cannot handle combined tex+out: {}", e),
        }
    }

    #[test]
    fn preprocess_flattens_layout_interface_block() {
        let src = "#version 450\nlayout (location = 0) in VertexData\n{\n    vec2 texcoord_0;\n} vd;\nlayout (location = 0) out vec4 out1;\nvoid main() {\n    out1 = vec4(vd.texcoord_0, 0.0, 1.0);\n}\n";
        let out = preprocess_mtlx_glsl(src);
        assert!(
            !out.contains("VertexData"),
            "interface block should be flattened, got:\n{out}"
        );
        assert!(
            !out.contains("vd."),
            "instance access should be rewritten, got:\n{out}"
        );
        assert!(
            out.contains("texcoord_0"),
            "member should survive flattening"
        );
    }
}
