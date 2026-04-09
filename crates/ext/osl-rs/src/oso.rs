//! OSO (OpenShadingLanguage Object) file format reader and writer.
//!
//! The `.oso` format is a text-based bytecode representation of compiled
//! OSL shaders. This module provides both parsing (read) and serialization
//! (write) capabilities.
//!
//! ## Format overview
//!
//! ```text
//! OpenShadingLanguage <major>.<minor>
//! <shadertype> <shadername>
//! <symbols...>
//! <code...>
//! ```
//!
//! Symbols are declared as:
//! ```text
//! <symtype> <type> <name> <default_values...>
//! ```
//!
//! Instructions are:
//! ```text
//! <label>: <opcode> <args...> <jumps...> <hints...>
//! ```

use std::fmt;
use std::io::{self, BufRead, Write as IoWrite};

use crate::symbol::{ShaderType, SymType};
use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};
use crate::typespec::TypeSpec;

/// Represents a parsed OSO file.
#[derive(Debug, Clone, Default)]
pub struct OsoFile {
    /// OSL version (major, minor).
    pub version: (i32, i32),
    /// Shader type (surface, displacement, etc.).
    pub shader_type: ShaderType,
    /// Shader name.
    pub shader_name: String,
    /// Declared symbols.
    pub symbols: Vec<OsoSymbol>,
    /// Instructions (code section).
    pub instructions: Vec<OsoInstruction>,
    /// Code section markers.
    pub code_markers: Vec<(i32, String)>,
}

/// A symbol as declared in the .oso file.
#[derive(Debug, Clone)]
pub struct OsoSymbol {
    /// Symbol kind (param, oparam, local, temp, global, const).
    pub symtype: SymType,
    /// Type specification.
    pub typespec: TypeSpec,
    /// Symbol name.
    pub name: String,
    /// Default int values.
    pub idefault: Vec<i32>,
    /// Default float values.
    pub fdefault: Vec<f32>,
    /// Default string values.
    pub sdefault: Vec<String>,
    /// Unrecognized hints (raw strings).
    pub hints: Vec<String>,
    /// Is this a structure?
    pub is_struct: bool,
    /// Structure name (if is_struct).
    pub structname: String,
    /// Struct field names from `%structfields{...}`.
    pub fields: Vec<String>,
    /// Geometry lock hint from `%meta{int,lockgeom,0|1}`.
    pub lockgeom: Option<bool>,
    /// Metadata entries from `%meta{type,name,value}` as (type, name, value).
    pub metadata: Vec<(String, String, String)>,
    /// Parsed %read{first,last} — op range where symbol is read.
    pub read_range: Option<(i32, i32)>,
    /// Parsed %write{first,last} — op range where symbol is written.
    pub write_range: Option<(i32, i32)>,
}

/// An instruction in the code section.
#[derive(Debug, Clone)]
pub struct OsoInstruction {
    /// Opcode name.
    pub opcode: String,
    /// Argument symbol names.
    pub args: Vec<String>,
    /// Jump targets.
    pub jumps: Vec<i32>,
    /// Source file hint.
    pub sourcefile: Option<String>,
    /// Source line hint.
    pub sourceline: Option<i32>,
    /// Hints (raw, for write-back).
    pub hints: Vec<String>,
    /// Parsed %argrw{"rwWr"} — per-arg read/write (r=read, w=write, W=both, -=neither).
    pub argrw: Option<String>,
    /// Parsed %argderivs{0,1,2} — arg indices that take derivatives.
    pub argderivs: Option<Vec<i32>>,
}

/// Errors that can occur during OSO parsing.
#[derive(Debug)]
pub enum OsoError {
    Io(io::Error),
    Parse(String),
    InvalidVersion(String),
    InvalidShaderType(String),
    InvalidSymbol(String),
    UnexpectedEof,
}

impl fmt::Display for OsoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsoError::Io(e) => write!(f, "IO error: {e}"),
            OsoError::Parse(s) => write!(f, "Parse error: {s}"),
            OsoError::InvalidVersion(s) => write!(f, "Invalid version: {s}"),
            OsoError::InvalidShaderType(s) => write!(f, "Invalid shader type: {s}"),
            OsoError::InvalidSymbol(s) => write!(f, "Invalid symbol: {s}"),
            OsoError::UnexpectedEof => write!(f, "Unexpected end of file"),
        }
    }
}

impl std::error::Error for OsoError {}

impl From<io::Error> for OsoError {
    fn from(e: io::Error) -> Self {
        OsoError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Read and parse an OSO file from any `BufRead` source.
pub fn read_oso<R: BufRead>(reader: R) -> Result<OsoFile, OsoError> {
    let mut lines = reader.lines();
    let mut oso = OsoFile {
        version: (0, 0),
        shader_type: ShaderType::Unknown,
        shader_name: String::new(),
        symbols: Vec::new(),
        instructions: Vec::new(),
        code_markers: Vec::new(),
    };

    // First line: version (skip empty/comment lines)
    let first = next_data_line(&mut lines)?;
    parse_version(&first, &mut oso)?;

    // Second line: shader declaration (skip empty/comment lines)
    let second = next_data_line(&mut lines)?;
    parse_shader_decl(&second, &mut oso)?;

    // Remaining lines: symbols and code
    let mut in_code = false;
    for line_result in lines {
        let line = line_result.map_err(OsoError::Io)?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("code ") || trimmed == "code" {
            in_code = true;
            let marker_name = trimmed.strip_prefix("code").unwrap_or("").trim();
            oso.code_markers
                .push((oso.instructions.len() as i32, marker_name.to_string()));
            continue;
        }

        if trimmed == "end" {
            break;
        }

        if !in_code {
            parse_symbol_line(trimmed, &mut oso)?;
        } else {
            parse_instruction_line(trimmed, &mut oso)?;
        }
    }

    Ok(oso)
}

/// Read an OSO file from a string.
pub fn read_oso_string(s: &str) -> Result<OsoFile, OsoError> {
    read_oso(io::Cursor::new(s))
}

/// Read an OSO file from disk.
pub fn read_oso_file(path: &str) -> Result<OsoFile, OsoError> {
    let file = std::fs::File::open(path).map_err(OsoError::Io)?;
    read_oso(io::BufReader::new(file))
}

fn next_data_line<R: BufRead>(lines: &mut io::Lines<R>) -> Result<String, OsoError> {
    for line in lines.by_ref() {
        let line = line.map_err(OsoError::Io)?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return Ok(line);
    }
    Err(OsoError::UnexpectedEof)
}

fn parse_version(line: &str, oso: &mut OsoFile) -> Result<(), OsoError> {
    // "OpenShadingLanguage 1.00"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "OpenShadingLanguage" {
        return Err(OsoError::InvalidVersion(line.to_string()));
    }
    let version_parts: Vec<&str> = parts[1].split('.').collect();
    let major = version_parts
        .first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let minor = version_parts
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    oso.version = (major, minor);
    Ok(())
}

fn parse_shader_decl(line: &str, oso: &mut OsoFile) -> Result<(), OsoError> {
    // "surface my_shader" or "shader my_shader"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(OsoError::InvalidShaderType(line.to_string()));
    }
    oso.shader_type = ShaderType::from_name(parts[0]);
    oso.shader_name = parts[1].to_string();
    Ok(())
}

fn parse_type_string(s: &str) -> TypeSpec {
    // Handle closure
    if s == "closure" || s.starts_with("closure ") {
        return TypeSpec::closure(TypeDesc::COLOR);
    }

    // Handle arrays: "float[3]", "color[10]", etc.
    let (base_str, arraylen) = if let Some(bracket_pos) = s.find('[') {
        let len_str = &s[bracket_pos + 1..s.len() - 1];
        let len = if len_str.is_empty() {
            -1 // unsized array
        } else {
            len_str.parse::<i32>().unwrap_or(0)
        };
        (&s[..bracket_pos], len)
    } else {
        (s, 0)
    };

    let td = match base_str {
        "float" => TypeDesc::FLOAT,
        "int" => TypeDesc::INT,
        "string" => TypeDesc::STRING,
        "color" => TypeDesc::COLOR,
        "point" => TypeDesc::POINT,
        "vector" => TypeDesc::VECTOR,
        "normal" => TypeDesc::NORMAL,
        "matrix" => TypeDesc::MATRIX,
        "void" => TypeDesc::NONE,
        _ => TypeDesc::UNKNOWN,
    };

    let td = if arraylen != 0 {
        td.array(arraylen)
    } else {
        td
    };
    TypeSpec::from_simple(td)
}

fn parse_symtype(s: &str) -> Option<SymType> {
    match s {
        "param" => Some(SymType::Param),
        "oparam" => Some(SymType::OutputParam),
        "local" => Some(SymType::Local),
        "temp" => Some(SymType::Temp),
        "global" => Some(SymType::Global),
        "const" => Some(SymType::Const),
        _ => None,
    }
}

/// Extract the content inside `%hint_name{...}`, stripping the closing `}`.
fn strip_hint_braces<'a>(tok: &'a str, prefix: &str) -> Option<&'a str> {
    let full_prefix = format!("{prefix}{{");
    tok.strip_prefix(&full_prefix)
        .map(|rest| rest.trim_end_matches('}'))
}

fn parse_symbol_line(line: &str, oso: &mut OsoFile) -> Result<(), OsoError> {
    let tokens: Vec<&str> = tokenize_oso_line(line);
    if tokens.is_empty() {
        return Ok(());
    }

    // First token is the symtype
    let symtype = match parse_symtype(tokens[0]) {
        Some(s) => s,
        None => return Ok(()), // Not a symbol line, skip
    };

    if tokens.len() < 3 {
        return Err(OsoError::InvalidSymbol(line.to_string()));
    }

    let typespec = parse_type_string(tokens[1]);
    let name = tokens[2].to_string();

    let mut sym = OsoSymbol {
        symtype,
        typespec,
        name,
        idefault: Vec::new(),
        fdefault: Vec::new(),
        sdefault: Vec::new(),
        hints: Vec::new(),
        is_struct: false,
        structname: String::new(),
        fields: Vec::new(),
        lockgeom: None,
        metadata: Vec::new(),
        read_range: None,
        write_range: None,
    };

    // Determine if defaults should be float or int based on type
    let is_float_type = sym.typespec.is_float_based()
        || sym.typespec.simpletype().basetype == crate::typedesc::BaseType::Float as u8;
    let is_string_type = sym.typespec.is_string_based();

    // Parse remaining tokens as defaults and hints
    let mut i = 3;
    while i < tokens.len() {
        let tok = tokens[i];
        if tok.starts_with('%') {
            // Structured hint parsing
            if let Some(inner) = strip_hint_braces(tok, "%structfields") {
                // %structfields{field1,field2,...}
                for field in inner.split(',') {
                    let f = field.trim();
                    if !f.is_empty() {
                        sym.fields.push(f.to_string());
                    }
                }
            } else if let Some(inner) = strip_hint_braces(tok, "%meta") {
                // %meta{type,name,value} — parse comma-separated triple
                let parts: Vec<&str> = inner.splitn(3, ',').collect();
                if parts.len() == 3 {
                    let mtype = parts[0].trim();
                    let mname = parts[1].trim();
                    let mval = parts[2].trim().trim_matches('"');
                    // Special-case: lockgeom sets interpolation lock
                    if mtype == "int" && mname == "lockgeom" {
                        sym.lockgeom = match mval.parse::<i32>() {
                            Ok(v) => Some(v != 0),
                            _ => None,
                        };
                    }
                    sym.metadata
                        .push((mtype.to_string(), mname.to_string(), mval.to_string()));
                }
            } else if let Some(inner) = strip_hint_braces(tok, "%read") {
                // %read{first,last}
                let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2
                    && let (Ok(f), Ok(l)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                {
                    sym.read_range = Some((f, l));
                }
            } else if let Some(inner) = strip_hint_braces(tok, "%write") {
                // %write{first,last}
                let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2
                    && let (Ok(f), Ok(l)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                {
                    sym.write_range = Some((f, l));
                }
            } else {
                // Unrecognized hint — store raw
                sym.hints.push(tok.to_string());
                if i + 1 < tokens.len() && !tokens[i + 1].starts_with('%') {
                    i += 1;
                    sym.hints.push(tokens[i].to_string());
                }
            }
        } else if tok.starts_with('"') {
            // String default
            let s = tok.trim_matches('"').to_string();
            sym.sdefault.push(s);
        } else if is_string_type {
            // Skip non-string tokens for string types
        } else if is_float_type {
            // Parse as float (integers like "1" become 1.0)
            if let Ok(f) = tok.parse::<f32>() {
                sym.fdefault.push(f);
            }
        } else {
            // Parse as int
            if let Ok(n) = tok.parse::<i32>() {
                sym.idefault.push(n);
            } else if let Ok(f) = tok.parse::<f32>() {
                // Fallback: float literal in int context (shouldn't happen but be robust)
                sym.idefault.push(f as i32);
            }
        }
        i += 1;
    }

    oso.symbols.push(sym);
    Ok(())
}

fn parse_instruction_line(line: &str, oso: &mut OsoFile) -> Result<(), OsoError> {
    let tokens: Vec<&str> = tokenize_oso_line(line);
    if tokens.is_empty() {
        return Ok(());
    }

    // Handle optional label: "N: opcode args..."
    let (opcode_idx, _label) = if tokens.len() > 1 && tokens[0].ends_with(':') {
        (1, Some(tokens[0].trim_end_matches(':')))
    } else {
        (0, None)
    };

    if opcode_idx >= tokens.len() {
        return Ok(());
    }

    let mut instr = OsoInstruction {
        opcode: tokens[opcode_idx].to_string(),
        args: Vec::new(),
        jumps: Vec::new(),
        sourcefile: None,
        sourceline: None,
        hints: Vec::new(),
        argrw: None,
        argderivs: None,
    };

    let mut i = opcode_idx + 1;
    while i < tokens.len() {
        let tok = tokens[i];
        if tok.starts_with('%') {
            // Hints — token may be %argrw{"rrwr"} (whole hint is one token)
            if let Some(inner) = strip_hint_braces(tok, "%argrw") {
                let rw_str = inner.trim_matches('"').to_string();
                instr.argrw = Some(rw_str);
                instr.hints.push(tok.to_string());
            } else if let Some(inner) = strip_hint_braces(tok, "%argderivs") {
                let indices: Vec<i32> = inner
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                instr.argderivs = Some(indices);
                instr.hints.push(tok.to_string());
            } else if tok == "%filename" && i + 1 < tokens.len() {
                i += 1;
                instr.sourcefile = Some(tokens[i].trim_matches('"').to_string());
            } else if tok == "%line" && i + 1 < tokens.len() {
                i += 1;
                instr.sourceline = tokens[i].parse().ok();
            } else {
                instr.hints.push(tok.to_string());
            }
        } else if tok.starts_with('$') {
            // Jump target (like $1, $2)
            if let Some(digits) = tok.strip_prefix('$')
                && let Ok(target) = digits.parse::<i32>()
            {
                instr.jumps.push(target);
            }
        } else {
            // Argument (symbol name)
            instr.args.push(tok.to_string());
        }
        i += 1;
    }

    oso.instructions.push(instr);
    Ok(())
}

/// Tokenize an OSO line, handling quoted strings.
fn tokenize_oso_line(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= len {
            break;
        }

        if bytes[i] == b'"' {
            // Quoted string — find matching quote
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            tokens.push(&line[start..i]);
        } else if bytes[i] == b'%' {
            // Hint token — may contain braces with spaces (e.g., %meta{string,name,"val with space"})
            let start = i;
            let mut in_braces = false;
            while i < len && (in_braces || (bytes[i] != b' ' && bytes[i] != b'\t')) {
                if bytes[i] == b'{' {
                    in_braces = true;
                } else if bytes[i] == b'}' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(&line[start..i]);
        } else {
            // Unquoted token
            let start = i;
            while i < len && bytes[i] != b' ' && bytes[i] != b'\t' {
                i += 1;
            }
            tokens.push(&line[start..i]);
        }
    }

    tokens
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Write an OSO file to any `Write` destination.
pub fn write_oso<W: IoWrite>(writer: &mut W, oso: &OsoFile) -> io::Result<()> {
    // Version line
    writeln!(
        writer,
        "OpenShadingLanguage {}.{:02}",
        oso.version.0, oso.version.1
    )?;

    // Shader declaration
    writeln!(writer, "{} {}", oso.shader_type.name(), oso.shader_name)?;

    // Symbols
    for sym in &oso.symbols {
        write!(
            writer,
            "{} {} {}",
            sym.symtype.short_name(),
            format_typespec(&sym.typespec),
            sym.name
        )?;

        // Default values
        for v in &sym.idefault {
            write!(writer, " {v}")?;
        }
        for v in &sym.fdefault {
            write!(writer, " {v}")?;
        }
        for v in &sym.sdefault {
            write!(writer, " \"{}\"", escape_oso_string(v))?;
        }

        // Structured hints
        if !sym.fields.is_empty() {
            write!(writer, " %structfields{{{}}}", sym.fields.join(","))?;
        }
        if let Some((first, last)) = sym.read_range {
            write!(writer, " %read{{{first},{last}}}")?;
        }
        if let Some((first, last)) = sym.write_range {
            write!(writer, " %write{{{first},{last}}}")?;
        }
        for (mtype, mname, mval) in &sym.metadata {
            if mtype == "string" {
                write!(writer, " %meta{{{mtype},{mname},\"{mval}\"}}")?
            } else {
                write!(writer, " %meta{{{mtype},{mname},{mval}}}")?
            }
        }

        // Unrecognized hints
        for h in &sym.hints {
            write!(writer, " {h}")?;
        }

        writeln!(writer)?;
    }

    // Code section
    let mut marker_idx = 0;
    for (i, instr) in oso.instructions.iter().enumerate() {
        // Check for code markers
        while marker_idx < oso.code_markers.len() && oso.code_markers[marker_idx].0 == i as i32 {
            let name = &oso.code_markers[marker_idx].1;
            if name.is_empty() {
                writeln!(writer, "code")?;
            } else {
                writeln!(writer, "code {name}")?;
            }
            marker_idx += 1;
        }

        // Instruction
        write!(writer, "\t{}", instr.opcode)?;
        for arg in &instr.args {
            write!(writer, " {arg}")?;
        }
        for jump in &instr.jumps {
            write!(writer, " ${jump}")?;
        }

        // Source hints
        if let Some(ref file) = instr.sourcefile {
            write!(writer, " %filename \"{}\"", escape_oso_string(file))?;
        }
        if let Some(line) = instr.sourceline {
            write!(writer, " %line {line}")?;
        }
        for hint in &instr.hints {
            write!(writer, " {hint}")?;
        }

        writeln!(writer)?;
    }

    // Remaining code markers
    while marker_idx < oso.code_markers.len() {
        let name = &oso.code_markers[marker_idx].1;
        if name.is_empty() {
            writeln!(writer, "code")?;
        } else {
            writeln!(writer, "code {name}")?;
        }
        marker_idx += 1;
    }

    writeln!(writer, "end")?;
    Ok(())
}

/// Write an OSO file to a string.
pub fn write_oso_string(oso: &OsoFile) -> io::Result<String> {
    let mut buf = Vec::new();
    write_oso(&mut buf, oso)?;
    Ok(String::from_utf8(buf).unwrap())
}

fn format_typespec(ts: &TypeSpec) -> String {
    if ts.is_closure_based() {
        return "closure color".to_string();
    }
    let td = ts.simpletype();
    let base = match (td.base_type(), td.agg(), td.vec_semantics()) {
        (BaseType::Float, Aggregate::Scalar, _) => "float",
        (BaseType::Int32, Aggregate::Scalar, _) => "int",
        (BaseType::String, Aggregate::Scalar, _) => "string",
        (BaseType::Float, Aggregate::Vec3, VecSemantics::Color) => "color",
        (BaseType::Float, Aggregate::Vec3, VecSemantics::Point) => "point",
        (BaseType::Float, Aggregate::Vec3, VecSemantics::Vector) => "vector",
        (BaseType::Float, Aggregate::Vec3, VecSemantics::Normal) => "normal",
        (BaseType::Float, Aggregate::Matrix44, _) => "matrix",
        (BaseType::None, _, _) => "void",
        _ => "unknown",
    };
    if td.arraylen > 0 {
        format!("{base}[{}]", td.arraylen)
    } else if td.arraylen < 0 {
        format!("{base}[]")
    } else {
        base.to_string()
    }
}

fn escape_oso_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// Uses SymType::short_name() defined in symbol.rs

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OSO: &str = r#"OpenShadingLanguage 1.00
surface test_shader
param float Kd 0.5
param color Cs 1 1 1
oparam color Cout 0 0 0
const float $const1 3.14
temp float $tmp1
code ___main___
	assign Cout Cs
end
"#;

    #[test]
    fn test_parse_basic() {
        let oso = read_oso_string(SAMPLE_OSO).unwrap();
        assert_eq!(oso.version, (1, 0));
        assert_eq!(oso.shader_type, ShaderType::Surface);
        assert_eq!(oso.shader_name, "test_shader");
        assert_eq!(oso.symbols.len(), 5);
        assert_eq!(oso.instructions.len(), 1);
    }

    #[test]
    fn test_parse_symbols() {
        let oso = read_oso_string(SAMPLE_OSO).unwrap();

        let kd = &oso.symbols[0];
        assert_eq!(kd.name, "Kd");
        assert_eq!(kd.symtype, SymType::Param);
        assert!(kd.typespec.is_float());
        assert_eq!(kd.fdefault, vec![0.5]);

        let cs = &oso.symbols[1];
        assert_eq!(cs.name, "Cs");
        assert!(cs.typespec.is_color());
        assert_eq!(cs.fdefault, vec![1.0, 1.0, 1.0]);

        let cout = &oso.symbols[2];
        assert_eq!(cout.name, "Cout");
        assert_eq!(cout.symtype, SymType::OutputParam);
    }

    #[test]
    fn test_roundtrip() {
        let oso = read_oso_string(SAMPLE_OSO).unwrap();
        let written = write_oso_string(&oso).unwrap();
        let oso2 = read_oso_string(&written).unwrap();
        assert_eq!(oso2.shader_type, oso.shader_type);
        assert_eq!(oso2.shader_name, oso.shader_name);
        assert_eq!(oso2.symbols.len(), oso.symbols.len());
        assert_eq!(oso2.instructions.len(), oso.instructions.len());
    }

    #[test]
    fn test_parse_structfields() {
        let input = r#"OpenShadingLanguage 1.00
surface test_shader
param float x 0 %structfields{a,b,c}
code ___main___
	assign x x
end
"#;
        let oso = read_oso_string(input).unwrap();
        let sym = &oso.symbols[0];
        assert_eq!(sym.fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_meta_lockgeom() {
        let input = r#"OpenShadingLanguage 1.00
surface test_shader
param float Kd 0.5 %meta{int,lockgeom,0}
code ___main___
	assign Kd Kd
end
"#;
        let oso = read_oso_string(input).unwrap();
        let sym = &oso.symbols[0];
        assert_eq!(sym.lockgeom, Some(false));
        assert_eq!(sym.metadata.len(), 1);
        assert_eq!(
            sym.metadata[0],
            ("int".into(), "lockgeom".into(), "0".into())
        );
    }

    #[test]
    fn test_parse_meta_lockgeom_true() {
        let input = r#"OpenShadingLanguage 1.00
surface test_shader
param float Kd 0.5 %meta{int,lockgeom,1}
code ___main___
	assign Kd Kd
end
"#;
        let oso = read_oso_string(input).unwrap();
        let sym = &oso.symbols[0];
        assert_eq!(sym.lockgeom, Some(true));
    }

    #[test]
    fn test_parse_meta_string() {
        let input = "OpenShadingLanguage 1.00\nsurface test_shader\nparam color Cs 1 1 1 %meta{string,label,\"Diffuse Color\"}\ncode ___main___\n\tassign Cs Cs\nend\n";
        let oso = read_oso_string(input).unwrap();
        let sym = &oso.symbols[0];
        assert_eq!(sym.metadata.len(), 1);
        assert_eq!(sym.metadata[0].0, "string");
        assert_eq!(sym.metadata[0].1, "label");
        assert_eq!(sym.metadata[0].2, "Diffuse Color");
    }

    #[test]
    fn test_roundtrip_structfields() {
        let input = r#"OpenShadingLanguage 1.00
surface test_shader
param float x 0 %structfields{a,b,c}
code ___main___
	assign x x
end
"#;
        let oso = read_oso_string(input).unwrap();
        let written = write_oso_string(&oso).unwrap();
        let oso2 = read_oso_string(&written).unwrap();
        assert_eq!(oso2.symbols[0].fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_roundtrip_meta() {
        let input = r#"OpenShadingLanguage 1.00
surface test_shader
param float Kd 0.5 %meta{int,lockgeom,0}
code ___main___
	assign Kd Kd
end
"#;
        let oso = read_oso_string(input).unwrap();
        let written = write_oso_string(&oso).unwrap();
        let oso2 = read_oso_string(&written).unwrap();
        assert_eq!(oso2.symbols[0].lockgeom, Some(false));
        assert_eq!(oso2.symbols[0].metadata.len(), 1);
    }

    #[test]
    fn test_parse_type_string() {
        let ts = parse_type_string("float");
        assert!(ts.is_float());

        let ts = parse_type_string("color");
        assert!(ts.is_color());

        let ts = parse_type_string("float[10]");
        assert!(ts.is_array());
        assert_eq!(ts.arraylength(), 10);

        let ts = parse_type_string("string[]");
        assert!(ts.is_unsized_array());
    }
}
