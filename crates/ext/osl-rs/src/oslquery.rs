//! OSLQuery — interrogate compiled shaders for parameter information.
//!
//! `OslQuery` reads a `.oso` file (or bytecode string) and extracts
//! information about the shader's type, name, and parameters (including
//! default values, metadata, and types).
//!
//! This mirrors the C++ `OSLQuery` class API.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use crate::oso::{self, OsoFile};
use crate::symbol::{ShaderType, SymType};
use crate::typedesc::TypeDesc;
use crate::ustring::UString;

/// Information about a single shader parameter.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Parameter name.
    pub name: UString,
    /// Data type.
    pub type_desc: TypeDesc,
    /// Is this an output parameter?
    pub is_output: bool,
    /// Does this have a valid default?
    pub valid_default: bool,
    /// Is this a variable-length array?
    pub varlen_array: bool,
    /// Is this a struct?
    pub is_struct: bool,
    /// Is this a closure?
    pub is_closure: bool,
    /// Default int values.
    pub idefault: Vec<i32>,
    /// Default float values.
    pub fdefault: Vec<f32>,
    /// Default string values.
    pub sdefault: Vec<UString>,
    /// Space names for matrices/triples.
    pub spacename: Vec<UString>,
    /// Field names (if struct).
    pub fields: Vec<UString>,
    /// Struct name.
    pub structname: UString,
    /// Metadata about the parameter.
    pub metadata: Vec<Parameter>,
    /// Raw default value bytes (type-erased). Corresponds to C++ `void* data`.
    /// Empty when no default is present or when typed defaults suffice.
    pub data: Vec<u8>,
}

impl Parameter {
    fn new() -> Self {
        Self {
            name: UString::default(),
            type_desc: TypeDesc::UNKNOWN,
            is_output: false,
            valid_default: false,
            varlen_array: false,
            is_struct: false,
            is_closure: false,
            idefault: Vec::new(),
            fdefault: Vec::new(),
            sdefault: Vec::new(),
            spacename: Vec::new(),
            fields: Vec::new(),
            structname: UString::default(),
            metadata: Vec::new(),
            data: Vec::new(),
        }
    }
}

/// Query interface for compiled OSL shaders.
///
/// Load a `.oso` file and inspect its parameters, metadata, and type.
pub struct OslQuery {
    shader_name: UString,
    shader_type_name: UString,
    shader_type: ShaderType,
    params: Vec<Parameter>,
    metadata: Vec<Parameter>,
    error: String,
}

impl OslQuery {
    /// Create an uninitialized query.
    pub fn new() -> Self {
        Self {
            shader_name: UString::default(),
            shader_type_name: UString::default(),
            shader_type: ShaderType::Unknown,
            params: Vec::new(),
            metadata: Vec::new(),
            error: String::new(),
        }
    }

    /// Open and parse a compiled shader from a `.oso` file on disk.
    /// The `shader_name` may be either a full path to a `.oso` file
    /// or a shader name to search for in `search_path`.
    pub fn open(&mut self, shader_name: &str, search_path: &str) -> bool {
        self.params.clear();
        self.metadata.clear();
        self.error.clear();

        // Try to find the file
        let path = find_oso_file(shader_name, search_path);
        let path = match path {
            Some(p) => p,
            None => {
                self.error = format!("Could not find shader '{shader_name}'");
                return false;
            }
        };

        match oso::read_oso_file(&path) {
            Ok(oso) => self.populate_from_oso(&oso),
            Err(e) => {
                self.error = format!("Error reading '{path}': {e}");
                false
            }
        }
    }

    /// Parse a compiled shader from a bytecode string (`.oso` format).
    pub fn open_bytecode(&mut self, bytecode: &str) -> bool {
        self.params.clear();
        self.metadata.clear();
        self.error.clear();

        match oso::read_oso_string(bytecode) {
            Ok(oso) => self.populate_from_oso(&oso),
            Err(e) => {
                self.error = format!("Error parsing bytecode: {e}");
                false
            }
        }
    }

    /// Get the shader type name ("surface", "displacement", etc.).
    pub fn shader_type_name(&self) -> UString {
        self.shader_type_name
    }

    /// Get the shader type enum.
    pub fn shader_type(&self) -> ShaderType {
        self.shader_type
    }

    /// Get the shader name.
    pub fn shader_name(&self) -> UString {
        self.shader_name
    }

    /// Number of parameters.
    pub fn nparams(&self) -> usize {
        self.params.len()
    }

    /// Get a parameter by index.
    pub fn getparam(&self, index: usize) -> Option<&Parameter> {
        self.params.get(index)
    }

    /// Get a parameter by name.
    pub fn getparam_by_name(&self, name: &str) -> Option<&Parameter> {
        let uname = UString::new(name);
        self.params.iter().find(|p| p.name == uname)
    }

    /// Get all parameters.
    pub fn parameters(&self) -> &[Parameter] {
        &self.params
    }

    /// Get shader-level metadata.
    pub fn metadata(&self) -> &[Parameter] {
        &self.metadata
    }

    /// Get and clear the error string.
    pub fn get_error(&mut self) -> String {
        std::mem::take(&mut self.error)
    }

    /// Check if there was an error.
    pub fn has_error(&self) -> bool {
        !self.error.is_empty()
    }

    // -- Internal --

    fn populate_from_oso(&mut self, oso: &OsoFile) -> bool {
        self.shader_name = UString::new(&oso.shader_name);
        self.shader_type = oso.shader_type;
        self.shader_type_name = UString::new(oso.shader_type.name());

        self.metadata = oso
            .shader_metadata
            .iter()
            .map(|(mtype, mname, mval)| parameter_from_shader_meta_triple(mtype, mname, mval))
            .collect();

        for sym in &oso.symbols {
            match sym.symtype {
                SymType::Param | SymType::OutputParam => {
                    let mut p = Parameter::new();
                    p.name = UString::new(&sym.name);
                    p.type_desc = sym.typespec.simpletype();
                    p.is_output = sym.symtype == SymType::OutputParam;
                    p.is_closure = sym.typespec.is_closure_based();
                    p.is_struct = sym.is_struct;
                    p.varlen_array = sym.typespec.is_unsized_array();

                    // Copy defaults
                    p.idefault = sym.idefault.clone();
                    p.fdefault = sym.fdefault.clone();
                    p.sdefault = sym.sdefault.iter().map(|s| UString::new(s)).collect();
                    p.valid_default =
                        !p.idefault.is_empty() || !p.fdefault.is_empty() || !p.sdefault.is_empty();

                    if sym.is_struct {
                        p.structname = UString::new(&sym.structname);
                        p.fields = sym.fields.iter().map(|s| UString::new(s)).collect();
                    }

                    // Wire parsed hints as metadata; merge structured `%meta` triples from oso
                    // (see `parse_symbol_line`) without duplicating names already parsed from hints.
                    let mut meta_params = parse_hints_as_metadata(&sym.hints);
                    let mut seen: HashSet<String> = meta_params
                        .iter()
                        .map(|p| p.name.as_str().to_string())
                        .collect();
                    for (mtype, mname, mval) in &sym.metadata {
                        if seen.insert(mname.clone()) {
                            meta_params.push(parameter_from_shader_meta_triple(
                                mtype.as_str(),
                                mname.as_str(),
                                mval.as_str(),
                            ));
                        }
                    }
                    p.metadata = meta_params;

                    self.params.push(p);
                }
                _ => {} // Skip non-parameter symbols
            }
        }

        true
    }
}

/// Build a [`Parameter`] from a `%meta{type,name,value}` triple on the shader declaration line.
fn parameter_from_shader_meta_triple(mtype: &str, mname: &str, mval: &str) -> Parameter {
    let mut mp = Parameter::new();
    mp.name = UString::new(mname);
    mp.type_desc = parse_meta_type(mtype);
    match mtype {
        "int" => {
            if let Ok(v) = mval.parse::<i32>() {
                mp.idefault.push(v);
                mp.valid_default = true;
            }
        }
        "float" => {
            if let Ok(v) = mval.parse::<f32>() {
                mp.fdefault.push(v);
                mp.valid_default = true;
            }
        }
        _ => {
            mp.sdefault.push(UString::new(mval));
            mp.valid_default = true;
        }
    }
    mp
}

impl Default for OslQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for OslQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OslQuery({} {} params={})",
            self.shader_type_name,
            self.shader_name,
            self.nparams()
        )
    }
}

/// Iterator support.
impl OslQuery {
    pub fn iter(&self) -> std::slice::Iter<'_, Parameter> {
        self.params.iter()
    }
}

impl<'a> IntoIterator for &'a OslQuery {
    type Item = &'a Parameter;
    type IntoIter = std::slice::Iter<'a, Parameter>;

    fn into_iter(self) -> Self::IntoIter {
        self.params.iter()
    }
}

// ---------------------------------------------------------------------------
// File search
// ---------------------------------------------------------------------------

/// Parse OSO hint strings into metadata Parameters.
///
/// Hints come in pairs: `["%meta{type,name,value}", ...]` or as
/// `["%meta", "value"]` pairs depending on the parser.
fn parse_hints_as_metadata(hints: &[String]) -> Vec<Parameter> {
    let mut metadata = Vec::new();
    let mut i = 0;
    while i < hints.len() {
        let h = &hints[i];
        // Format: %meta{type,name,value} or just %meta followed by value
        if h.starts_with("%meta{") && h.ends_with('}') {
            let inner = &h[6..h.len() - 1];
            // Split: type,name,value (value may contain commas if quoted)
            let parts: Vec<&str> = inner.splitn(3, ',').collect();
            if parts.len() >= 2 {
                let mut mp = Parameter::new();
                let meta_type = parts[0].trim();
                let meta_name = parts[1].trim();
                mp.name = UString::new(meta_name);
                mp.type_desc = parse_meta_type(meta_type);
                if parts.len() >= 3 {
                    let val = parts[2].trim().trim_matches('"');
                    match mp.type_desc.basetype {
                        b if b == crate::typedesc::BaseType::Int32 as u8 => {
                            if let Ok(v) = val.parse::<i32>() {
                                mp.idefault.push(v);
                            }
                        }
                        b if b == crate::typedesc::BaseType::Float as u8 => {
                            if let Ok(v) = val.parse::<f32>() {
                                mp.fdefault.push(v);
                            }
                        }
                        _ => {
                            mp.sdefault.push(UString::new(val));
                        }
                    }
                    mp.valid_default = true;
                }
                metadata.push(mp);
            }
        } else if h == "%meta" && i + 1 < hints.len() {
            // Simple pair: %meta followed by value token
            let mut mp = Parameter::new();
            mp.name = UString::new(&hints[i + 1]);
            mp.type_desc = TypeDesc::STRING;
            mp.sdefault.push(UString::new(&hints[i + 1]));
            mp.valid_default = true;
            metadata.push(mp);
            i += 1; // skip the value token
        }
        i += 1;
    }
    metadata
}

/// Parse a metadata type name to TypeDesc.
fn parse_meta_type(s: &str) -> TypeDesc {
    match s {
        "int" => TypeDesc::INT,
        "float" => TypeDesc::FLOAT,
        "string" => TypeDesc::STRING,
        "color" => TypeDesc::COLOR,
        "point" => TypeDesc::POINT,
        "vector" => TypeDesc::VECTOR,
        "normal" => TypeDesc::NORMAL,
        "matrix" => TypeDesc::MATRIX,
        _ => TypeDesc::STRING,
    }
}

fn find_oso_file(name: &str, search_path: &str) -> Option<String> {
    // If the name already has .oso extension and is an existing file
    let p = Path::new(name);
    if p.exists() {
        return Some(name.to_string());
    }

    // Try with .oso extension
    let with_ext = format!("{name}.oso");
    if Path::new(&with_ext).exists() {
        return Some(with_ext);
    }

    // Search in paths (colon or semicolon separated)
    let sep = if cfg!(windows) { ';' } else { ':' };
    for dir in search_path.split(sep) {
        let dir = dir.trim();
        if dir.is_empty() {
            continue;
        }

        let full = Path::new(dir).join(name);
        if full.exists() {
            return Some(full.to_string_lossy().to_string());
        }

        let full_ext = Path::new(dir).join(&with_ext);
        if full_ext.exists() {
            return Some(full_ext.to_string_lossy().to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OSO: &str = r#"OpenShadingLanguage 1.00
surface test_shader
param float Kd 0.5
param color diffuse_color 1 0.8 0.6
oparam color result_color 0 0 0
const float $const1 3.14
temp float $tmp1
code ___main___
	assign result_color diffuse_color
end
"#;

    #[test]
    fn test_query_basic() {
        let mut q = OslQuery::new();
        assert!(q.open_bytecode(SAMPLE_OSO));
        assert!(!q.has_error());

        assert_eq!(q.shader_name().as_str(), "test_shader");
        assert_eq!(q.shader_type(), ShaderType::Surface);
        assert_eq!(q.nparams(), 3);
    }

    #[test]
    fn test_query_params() {
        let mut q = OslQuery::new();
        q.open_bytecode(SAMPLE_OSO);

        let kd = q.getparam(0).unwrap();
        assert_eq!(kd.name.as_str(), "Kd");
        assert!(kd.type_desc.is_float());
        assert!(!kd.is_output);
        assert!(kd.valid_default);
        assert_eq!(kd.fdefault, vec![0.5]);

        let dc = q.getparam(1).unwrap();
        assert_eq!(dc.name.as_str(), "diffuse_color");
        assert!(dc.type_desc.is_triple());
        assert_eq!(dc.fdefault, vec![1.0, 0.8, 0.6]);

        let rc = q.getparam(2).unwrap();
        assert_eq!(rc.name.as_str(), "result_color");
        assert!(rc.is_output);
    }

    #[test]
    fn test_query_by_name() {
        let mut q = OslQuery::new();
        q.open_bytecode(SAMPLE_OSO);

        let p = q.getparam_by_name("Kd").unwrap();
        assert_eq!(p.fdefault, vec![0.5]);

        assert!(q.getparam_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_shader_declaration_metadata() {
        let oso = r#"OpenShadingLanguage 1.00
shader TestNodeOSL %meta{string,category,"testing"} %meta{string,label,"TestNodeLabel"} %meta{string,primvars,"a|b|c"}
param float Kd 0.5
code ___main___
	end
end
"#;
        let mut q = OslQuery::new();
        assert!(q.open_bytecode(oso));
        let meta = q.metadata();
        let labels: Vec<&str> = meta.iter().map(|p| p.name.as_str()).collect();
        assert!(labels.contains(&"label"));
        assert!(labels.contains(&"primvars"));
        let label = meta.iter().find(|p| p.name.as_str() == "label").unwrap();
        assert_eq!(label.sdefault.len(), 1);
        assert_eq!(label.sdefault[0].as_str(), "TestNodeLabel");
    }

    #[test]
    fn test_query_error() {
        let mut q = OslQuery::new();
        assert!(!q.open("nonexistent_shader", ""));
        assert!(q.has_error());
        let err = q.get_error();
        assert!(err.contains("Could not find"));
    }
}
