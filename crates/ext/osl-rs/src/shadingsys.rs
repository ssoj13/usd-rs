//! ShadingSystem — the main runtime object for OSL shader execution.
//!
//! Port of `OSL::ShadingSystem` from `oslexec.h`.
//! Manages shader loading, group construction, parameter binding,
//! optimization, and execution dispatch.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::closure::ClosureParam;
use crate::math::{Color3, Matrix44, Vec3};
use crate::oso::OsoFile;
use crate::renderer::RendererServices;
use crate::symbol::ShaderType;
use crate::ustring::UString;

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

/// Error handler trait for the shading system.
/// Matches OIIO/OSL ErrorHandler semantics (EH_MESSAGE, EH_INFO, EH_WARNING, EH_ERROR, EH_SEVERE).
pub trait ErrorHandler: Send + Sync {
    /// Message/debug output (printf, EH_MESSAGE, EH_DEBUG). Default forwards to info().
    fn message(&self, msg: &str) {
        self.info(msg);
    }
    /// Informational (EH_INFO).
    fn info(&self, msg: &str);
    /// Warning (EH_WARNING).
    fn warning(&self, msg: &str);
    /// Error (EH_ERROR).
    fn error(&self, msg: &str);
    /// Severe error (EH_SEVERE).
    fn severe(&self, msg: &str);
}

/// Default error handler that prints to stderr.
pub struct StdErrorHandler;

impl ErrorHandler for StdErrorHandler {
    fn message(&self, msg: &str) {
        eprintln!("[OSL] {msg}");
    }
    fn info(&self, msg: &str) {
        eprintln!("[OSL INFO] {msg}");
    }
    fn warning(&self, msg: &str) {
        eprintln!("[OSL WARNING] {msg}");
    }
    fn error(&self, msg: &str) {
        eprintln!("[OSL ERROR] {msg}");
    }
    fn severe(&self, msg: &str) {
        eprintln!("[OSL SEVERE] {msg}");
    }
}

// ---------------------------------------------------------------------------
// Shader master / instance
// ---------------------------------------------------------------------------

/// A compiled shader "master" — the immutable template loaded from .oso.
#[derive(Debug, Clone)]
pub struct ShaderMaster {
    pub name: UString,
    pub shader_type: ShaderType,
    pub oso: OsoFile,
}

/// A shader instance — a master plus per-instance parameter overrides.
#[derive(Debug, Clone)]
pub struct ShaderInstance {
    pub master: Arc<ShaderMaster>,
    pub layer_name: UString,
    /// Per-instance parameter overrides (param name -> serialized value).
    pub param_overrides: HashMap<UString, ParamValue>,
    /// Per-param hints (interpolated, interactive) from Parameter(..., hints) / groupspec [[...]].
    pub param_hints: HashMap<UString, ParamHints>,
    /// Marked true when this instance has been merged into another.
    /// Matches C++ `m_unused` flag on ShaderInstance.
    pub unused: bool,
}

/// A parameter value that can be int, float, string, or arrays thereof.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Int(i32),
    Float(f32),
    String(UString),
    Color(Color3),
    Point(Vec3),
    Vector(Vec3),
    Normal(Vec3),
    Matrix(Matrix44),
    IntArray(Vec<i32>),
    FloatArray(Vec<f32>),
    StringArray(Vec<UString>),
}

// ---------------------------------------------------------------------------
// OSLQuery types (Task 1)
// ---------------------------------------------------------------------------

/// Per-parameter info returned by `ShadingSystem::oslquery`.
/// Mirrors C++ `OSLQuery::Parameter` but simplified for the Rust API.
#[derive(Debug, Clone)]
pub struct ParamInfo {
    /// Parameter name.
    pub name: String,
    /// Human-readable type string ("float", "color", "int", ...).
    pub type_name: String,
    /// Is this an output parameter?
    pub is_output: bool,
    /// Does the parameter have a known default value?
    pub valid_default: bool,
    /// Default int values (scalar int or int array).
    pub idefault: Vec<i32>,
    /// Default float values (scalar float, triple, matrix, etc.).
    pub fdefault: Vec<f32>,
    /// Default string values.
    pub sdefault: Vec<String>,
    /// Is this a closure type?
    pub is_closure: bool,
    /// Is this a struct type?
    pub is_struct: bool,
    /// Is this a variable-length array?
    pub varlen_array: bool,
}

/// Result of `ShadingSystem::oslquery`: describes a single shader layer.
/// Mirrors C++ `OSLQuery` API for a particular layer in a `ShaderGroup`.
#[derive(Debug, Clone)]
pub struct OSLQueryInfo {
    /// Shader name (from the `.oso` master).
    pub shader_name: String,
    /// Shader type enum.
    pub shader_type: ShaderType,
    /// Number of parameters.
    pub num_params: usize,
    /// Parameter descriptors.
    pub params: Vec<ParamInfo>,
}

// ---------------------------------------------------------------------------
// Shader connections
// ---------------------------------------------------------------------------

/// A connection between two layers.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub src_layer: i32,
    pub src_param: UString,
    pub dst_layer: i32,
    pub dst_param: UString,
}

// ---------------------------------------------------------------------------
// ShaderGroup
// ---------------------------------------------------------------------------

/// A group of interconnected shader layers.
pub struct ShaderGroup {
    pub name: UString,
    pub layers: Vec<ShaderInstance>,
    pub connections: Vec<Connection>,
    pub optimized: bool,
    /// Whether shader_group_end() has been called and validation passed.
    pub complete: bool,
    /// Pending parameter to apply to the next Shader() call.
    /// Each entry is (name, value, hints) — hints default to NONE when not specified.
    pending_params: Vec<(UString, ParamValue, ParamHints)>,
    /// Renderer outputs for this group (C++ m_renderer_outputs).
    pub renderer_outputs: Vec<UString>,
    /// Entry layer names for this group (C++ entry_layer flags).
    pub entry_layers: Vec<UString>,
    /// Per-group symbol location mappings (sorted by name). C++ m_symlocs.
    pub symlocs: Vec<SymLocationDesc>,
}

impl ShaderGroup {
    pub fn new(name: &str) -> Self {
        Self {
            name: UString::new(name),
            layers: Vec::new(),
            connections: Vec::new(),
            optimized: false,
            complete: false,
            pending_params: Vec::new(), // (name, value, hints)
            renderer_outputs: Vec::new(),
            entry_layers: Vec::new(),
            symlocs: Vec::new(),
        }
    }

    /// Clear per-group symbol location mappings. Matching C++ ShaderGroup::clear_symlocs.
    pub fn clear_symlocs(&mut self) {
        self.symlocs.clear();
    }

    /// Add per-group symbol location mappings (sorted by name). Matching C++ ShaderGroup::add_symlocs.
    pub fn add_symlocs(&mut self, symlocs: &[SymLocationDesc]) {
        for s in symlocs {
            match self
                .symlocs
                .binary_search_by(|probe| probe.name.cmp(&s.name))
            {
                Ok(idx) => self.symlocs[idx] = s.clone(),
                Err(idx) => self.symlocs.insert(idx, s.clone()),
            }
        }
    }

    /// Find a symbol location by name in this group. Matching C++ ShaderGroup::find_symloc.
    pub fn find_symloc(&self, name: &str) -> Option<SymLocationDesc> {
        let name = UString::new(name);
        match self.symlocs.binary_search_by(|probe| probe.name.cmp(&name)) {
            Ok(idx) => Some(self.symlocs[idx].clone()),
            Err(_) => None,
        }
    }

    /// Find symloc by name and arena; returns None if arena/offset mismatch. C++ find_symloc(name, arena).
    pub fn find_symloc_arena(
        &self,
        name: &str,
        arena: crate::symbol::SymArena,
    ) -> Option<SymLocationDesc> {
        let name = UString::new(name);
        match self.symlocs.binary_search_by(|probe| probe.name.cmp(&name)) {
            Ok(idx) => {
                let s = &self.symlocs[idx];
                if s.arena == arena && s.offset != -1 {
                    Some(s.clone())
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Find symloc by name and arena; try "layer.name" first, then name. C++ find_symloc(name, layer, arena).
    pub fn find_symloc_layer(
        &self,
        name: &str,
        layer: &str,
        arena: crate::symbol::SymArena,
    ) -> Option<SymLocationDesc> {
        let layersym = format!("{}.{}", layer, name);
        self.find_symloc_arena(&layersym, arena)
            .or_else(|| self.find_symloc_arena(name, arena))
    }

    /// Serialize the shader group to text format. Matching C++ ShaderGroup::serialize.
    /// Format: param/shader/connect statements per shadergroups.md.
    pub fn serialize(&self) -> String {
        fn escape_param_str(s: &str) -> String {
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        }
        fn param_value_to_str(pv: &ParamValue) -> (String, String) {
            match pv {
                ParamValue::Int(v) => ("int".into(), v.to_string()),
                ParamValue::Float(v) => ("float".into(), format!("{v:.9}")),
                ParamValue::String(u) => (
                    "string".into(),
                    format!("\"{}\"", escape_param_str(u.as_str())),
                ),
                ParamValue::Color(c) => ("color".into(), format!("{} {} {}", c.x, c.y, c.z)),
                ParamValue::Point(v) => ("point".into(), format!("{} {} {}", v.x, v.y, v.z)),
                ParamValue::Vector(v) => ("vector".into(), format!("{} {} {}", v.x, v.y, v.z)),
                ParamValue::Normal(v) => ("normal".into(), format!("{} {} {}", v.x, v.y, v.z)),
                ParamValue::Matrix(m) => (
                    "matrix".into(),
                    (0..4)
                        .flat_map(|r| (0..4).map(move |c| m.m[r][c].to_string()))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                ParamValue::IntArray(arr) => (
                    "int[]".into(),
                    arr.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                ParamValue::FloatArray(arr) => (
                    "float[]".into(),
                    arr.iter()
                        .map(|v| format!("{v:.9}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                ParamValue::StringArray(arr) => (
                    "string[]".into(),
                    arr.iter()
                        .map(|u| format!("\"{}\"", escape_param_str(u.as_str())))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
            }
        }
        let mut out = String::new();
        for (dst_idx, layer) in self.layers.iter().enumerate() {
            for (name, val) in &layer.param_overrides {
                let (ty, vals) = param_value_to_str(val);
                out.push_str(&format!("param {ty} {} {vals}", name.as_str()));
                if let Some(h) = layer.param_hints.get(name) {
                    if h.contains(ParamHints::INTERPOLATED) {
                        out.push_str(" [[int interpolated=1]]");
                    }
                    if h.contains(ParamHints::INTERACTIVE) {
                        out.push_str(" [[int interactive=1]]");
                    }
                }
                out.push_str(" ;\n");
            }
            out.push_str(&format!(
                "shader {} {} ;\n",
                layer.master.name.as_str(),
                layer.layer_name.as_str()
            ));
            for conn in self
                .connections
                .iter()
                .filter(|c| c.dst_layer == dst_idx as i32)
            {
                let src_layer = self.layers.get(conn.src_layer as usize);
                let src_name = src_layer.map(|l| l.layer_name.as_str()).unwrap_or("?");
                out.push_str(&format!(
                    "connect {}.{} {}.{} ;\n",
                    src_name,
                    conn.src_param.as_str(),
                    layer.layer_name.as_str(),
                    conn.dst_param.as_str()
                ));
            }
        }
        out
    }

    /// Number of layers.
    pub fn nlayers(&self) -> usize {
        self.layers.len()
    }

    /// Find a layer index by name.
    /// Searches from last to first (matching C++ find_layer behavior).
    pub fn find_layer(&self, name: &str) -> Option<usize> {
        let u = UString::new(name);
        self.layers.iter().rposition(|l| l.layer_name == u)
    }

    /// Merge identical shader instances within this group.
    ///
    /// Port of `ShadingSystemImpl::merge_instances()` from C++ reference.
    ///
    /// Finds pairs of layers that use the same shader master with the same
    /// parameter values and the same incoming connections. When such a pair
    /// is found, the duplicate (later layer) is eliminated and all outgoing
    /// connections from it are rewired to the kept (earlier) layer.
    ///
    /// Returns the number of merges performed.
    pub fn merge_instances(&mut self) -> usize {
        let nlayers = self.layers.len();
        if nlayers < 2 {
            return 0;
        }

        let mut merges = 0;
        // Track which layers have been merged away
        let mut merged_away: Vec<bool> = vec![false; nlayers];

        // O(n^2) scan — same as C++ reference. Fast in practice because
        // the master pointer comparison rejects most pairs immediately.
        for a in 0..nlayers.saturating_sub(1) {
            if merged_away[a] {
                continue;
            }
            for b in (a + 1)..nlayers {
                if merged_away[b] {
                    continue;
                }
                // Don't merge the last layer (it's the group entry point)
                if b == nlayers - 1 {
                    continue;
                }

                if !Self::instances_mergeable(
                    &self.layers[a],
                    &self.layers[b],
                    &self.connections,
                    a,
                    b,
                ) {
                    continue;
                }

                // Merge: keep A, eliminate B.
                // Rewire all connections from B to A in downstream layers.
                for con in self.connections.iter_mut() {
                    if con.src_layer == b as i32 {
                        con.src_layer = a as i32;
                    }
                }

                // Remove connections going INTO B (they are now dead)
                self.connections.retain(|c| c.dst_layer != b as i32);

                // Mark merged-away instance as unused (matches C++ m_merged_unused)
                self.layers[b].unused = true;
                merged_away[b] = true;
                merges += 1;
            }
        }

        // Mark merged-away layers via the `unused` flag. They stay in the
        // vector but are skipped during execution. Matches C++ behavior.
        for (i, is_merged) in merged_away.iter().enumerate() {
            if *is_merged {
                // Mark the layer as merged/unused. The layer stays in the vector
                // but is skipped during execution.
                self.layers[i].unused = true;
            }
        }

        merges
    }

    /// Check if two shader instances are mergeable (identical in all meaningful ways).
    fn instances_mergeable(
        a: &ShaderInstance,
        b: &ShaderInstance,
        connections: &[Connection],
        a_idx: usize,
        b_idx: usize,
    ) -> bool {
        // Must use the same master shader (fast rejection for most pairs)
        if !Arc::ptr_eq(&a.master, &b.master) {
            return false;
        }

        // Must have the same parameter overrides
        if a.param_overrides.len() != b.param_overrides.len() {
            return false;
        }
        for (key, val_a) in &a.param_overrides {
            match b.param_overrides.get(key) {
                Some(val_b) if val_a == val_b => {}
                _ => return false,
            }
        }
        // Must have identical param hints for all overridden params
        for (key, hints_a) in &a.param_hints {
            match b.param_hints.get(key) {
                Some(hints_b) if hints_a.0 == hints_b.0 => {}
                None if hints_a.0 == 0 => {}
                _ => return false,
            }
        }
        for (key, hints_b) in &b.param_hints {
            if !a.param_hints.contains_key(key) && hints_b.0 != 0 {
                return false;
            }
        }

        // Must have identical incoming connections
        let conns_a: Vec<_> = connections
            .iter()
            .filter(|c| c.dst_layer == a_idx as i32)
            .collect();
        let conns_b: Vec<_> = connections
            .iter()
            .filter(|c| c.dst_layer == b_idx as i32)
            .collect();

        if conns_a.len() != conns_b.len() {
            return false;
        }

        // Check each incoming connection pair
        // Both must connect from the same source to the same parameter
        for ca in &conns_a {
            let found = conns_b.iter().any(|cb| {
                ca.src_layer == cb.src_layer
                    && ca.src_param == cb.src_param
                    && ca.dst_param == cb.dst_param
            });
            if !found {
                return false;
            }
        }

        true
    }
}

/// Thread-safe reference to a shader group.
pub type ShaderGroupRef = Arc<Mutex<ShaderGroup>>;

// ---------------------------------------------------------------------------
// Groupspec parser helpers (for ShaderGroupBegin serialized form)
// ---------------------------------------------------------------------------

fn skip_ws(p: &mut &str) {
    *p = p.trim_start();
}

fn parse_word(p: &mut &str) -> String {
    skip_ws(p);
    let orig = *p;
    let end = orig
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    if end == 0 {
        return String::new();
    }
    let word = orig[..end].to_string();
    *p = &orig[end..];
    word
}

fn parse_identifier(p: &mut &str) -> String {
    parse_word(p)
}

fn parse_until_space_or_delim<'a>(p: &mut &'a str) -> &'a str {
    parse_until_one_of(p, " \t\r\n,;")
}

/// Parse until any of the given delimiter chars (stops before the first occurrence).
fn parse_until_one_of<'a>(p: &mut &'a str, delims: &str) -> &'a str {
    skip_ws(p);
    let delims: Vec<u8> = delims.bytes().collect();
    let end = p
        .bytes()
        .position(|b| delims.contains(&b))
        .unwrap_or(p.len());
    let (head, tail) = p.split_at(end);
    *p = tail;
    head.trim_end()
}

fn eat_char(p: &mut &str, ch: char) {
    if p.starts_with(ch) {
        *p = &p[1..];
    }
}

fn parse_type_and_array(p: &mut &str, typestr: String) -> Result<(String, Option<i32>), String> {
    skip_ws(p);
    let mut array_len: Option<i32> = None;
    if p.starts_with('[') {
        *p = &p[1..];
        let mut val: i32 = -1;
        let mut consumed = 0;
        for (i, c) in p.bytes().enumerate() {
            if c.is_ascii_digit() {
                val = if val < 0 { 0 } else { val };
                val = val * 10 + (c - b'0') as i32;
                consumed = i + 1;
            } else if c == b']' {
                consumed = i + 1;
                break;
            } else {
                break;
            }
        }
        *p = &p[consumed..];
        if p.starts_with(']') {
            *p = &p[1..];
        }
        array_len = Some(val);
    }
    Ok((typestr, array_len))
}

fn parse_param_name(p: &mut &str) -> String {
    skip_ws(p);
    let mut name = String::new();
    loop {
        let w = parse_word(p);
        if w.is_empty() {
            break;
        }
        name.push_str(&w);
        skip_ws(p);
        if p.starts_with('.') {
            name.push('.');
            *p = &p[1..];
        } else {
            break;
        }
    }
    name
}

fn parse_one_float(p: &mut &str) -> f32 {
    skip_ws(p);
    let orig = *p;
    let len = orig
        .bytes()
        .take_while(|b| *b == b'-' || *b == b'.' || *b == b'e' || *b == b'E' || b.is_ascii_digit())
        .count();
    let s = &orig[..len];
    *p = &orig[len..];
    s.parse().unwrap_or(0.0)
}

fn parse_one_int(p: &mut &str) -> i32 {
    skip_ws(p);
    let orig = *p;
    let mut len = 0;
    if orig.starts_with('-') || orig.starts_with('+') {
        len = 1;
    }
    let rest = &orig[len..];
    let num_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    len += num_len;
    let s = &orig[..len];
    *p = &orig[len..];
    s.parse().unwrap_or(0)
}

fn parse_param_values(
    p: &mut &str,
    base_type: &str,
    array_len: Option<i32>,
) -> Result<ParamValue, String> {
    skip_ws(p);
    let agg = match base_type {
        "int" => 1,
        "float" => 1,
        "color" | "point" | "vector" | "normal" => 3,
        "matrix" => 16,
        "string" => 1,
        _ => return Err(format!("Unknown param type: {}", base_type)),
    };
    let n = array_len.unwrap_or(1).max(1) as usize * agg;
    match base_type {
        "int" => {
            let mut vals = Vec::with_capacity(n);
            for _ in 0..n {
                vals.push(parse_one_int(p));
            }
            vals.resize(n, 0);
            Ok(if vals.len() == 1 {
                ParamValue::Int(vals[0])
            } else {
                ParamValue::IntArray(vals)
            })
        }
        "float" => {
            let mut vals = Vec::with_capacity(n);
            for _ in 0..n {
                vals.push(parse_one_float(p));
            }
            vals.resize(n, 0.0);
            Ok(if vals.len() == 1 {
                ParamValue::Float(vals[0])
            } else {
                ParamValue::FloatArray(vals)
            })
        }
        "string" => {
            let mut vals = Vec::new();
            for _ in 0..n {
                skip_ws(p);
                let s = if p.starts_with('"') {
                    *p = &p[1..];
                    let mut out = String::new();
                    while let Some(c) = p.chars().next() {
                        if c == '\\' {
                            *p = &p[1..];
                            if let Some(nc) = p.chars().next() {
                                *p = &p[nc.len_utf8()..];
                                out.push(match nc {
                                    'n' => '\n',
                                    'r' => '\r',
                                    't' => '\t',
                                    '"' => '"',
                                    '\\' => '\\',
                                    _ => nc,
                                });
                            }
                        } else if c == '"' {
                            *p = &p[1..];
                            break;
                        } else {
                            *p = &p[c.len_utf8()..];
                            out.push(c);
                        }
                    }
                    out
                } else {
                    let tok = parse_until_space_or_delim(p).to_string();
                    if tok.is_empty() {
                        break;
                    }
                    tok
                };
                vals.push(UString::new(&s));
            }
            vals.resize(n, UString::new(""));
            Ok(if vals.len() == 1 {
                ParamValue::String(vals[0].clone())
            } else {
                ParamValue::StringArray(vals)
            })
        }
        "color" | "point" | "vector" | "normal" => {
            let n_triplets = n / 3;
            let mut all_floats = Vec::with_capacity(n);
            for _ in 0..n {
                all_floats.push(parse_one_float(p));
            }
            all_floats.resize(n, 0.0);
            let v = Vec3::new(
                *all_floats.get(0).unwrap_or(&0.0),
                *all_floats.get(1).unwrap_or(&0.0),
                *all_floats.get(2).unwrap_or(&0.0),
            );
            Ok(match (base_type, n_triplets) {
                ("color", 1) => ParamValue::Color(Color3::new(v.x, v.y, v.z)),
                ("point", 1) => ParamValue::Point(v),
                ("vector", 1) => ParamValue::Vector(v),
                ("normal", 1) => ParamValue::Normal(v),
                _ => ParamValue::FloatArray(all_floats),
            })
        }
        "matrix" => {
            let mut m = [[0.0f32; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    m[r][c] = parse_one_float(p);
                }
            }
            Ok(ParamValue::Matrix(Matrix44::from_row_major(&[
                m[0][0], m[0][1], m[0][2], m[0][3], m[1][0], m[1][1], m[1][2], m[1][3], m[2][0],
                m[2][1], m[2][2], m[2][3], m[3][0], m[3][1], m[3][2], m[3][3],
            ])))
        }
        _ => Err(format!("Unsupported param type: {}", base_type)),
    }
}

/// Parse one `[[ type name = value [, ...] ]]` hints block. Matching C++ ShaderGroupBegin.
/// Returns ParamHints (interpolated, interactive) from lockgeom, interpolated, interactive.
/// Call in a loop to handle multiple blocks like " [[int interpolated=1]] [[int interactive=1]]".
fn parse_param_hints(p: &mut &str) -> Result<ParamHints, String> {
    if !p.starts_with("[[") {
        return Ok(ParamHints::NONE);
    }
    *p = &p[2..];
    let mut hints = ParamHints::NONE;
    loop {
        skip_ws(p);
        if p.starts_with("]]") {
            *p = &p[2..];
            return Ok(hints);
        }
        let hint_typename = parse_word(p);
        if hint_typename.is_empty() {
            return Err("malformed hint".into());
        }
        skip_ws(p);
        let hint_name = parse_word(p);
        if hint_name.is_empty() {
            return Err("malformed hint".into());
        }
        if !p.starts_with('=') {
            return Err("hint expected value".into());
        }
        *p = &p[1..];
        let val = parse_one_int(p);
        match hint_name.as_str() {
            "lockgeom" if hint_typename == "int" => {
                if val == 0 {
                    hints = hints | ParamHints::INTERPOLATED;
                }
            }
            "interpolated" if hint_typename == "int" => {
                if val != 0 {
                    hints = hints | ParamHints::INTERPOLATED;
                }
            }
            "interactive" if hint_typename == "int" => {
                if val != 0 {
                    hints = hints | ParamHints::INTERACTIVE;
                }
            }
            _ => return Err(format!("unknown hint '{} {}'", hint_typename, hint_name)),
        }
        skip_ws(p);
        if p.starts_with(',') {
            *p = &p[1..];
        } else if !p.starts_with("]]") {
            return Err("malformed hint".into());
        }
    }
}

// ---------------------------------------------------------------------------
// Closure registration
// ---------------------------------------------------------------------------

/// Registered closure info.
struct ClosureEntry {
    name: UString,
    id: i32,
    params: Vec<ClosureParam>,
}

// ---------------------------------------------------------------------------
// ShadingSystem
// ---------------------------------------------------------------------------

/// Runtime statistics counters for the shading system.
/// Matches C++ `ShadingSystemImpl` stat counters.
pub struct ShadingStats {
    // -- Shader loading --
    pub shaders_loaded: AtomicU64,
    pub shaders_requested: AtomicU64,
    pub groups_created: AtomicU64,
    pub regexes_compiled: AtomicU64,
    pub connections_total: AtomicU64,
    // -- Execution --
    pub layers_executed: AtomicU64,
    pub total_shading_time_ticks: AtomicU64,
    // -- Compilation / merging --
    pub groups_compiled: AtomicU64,
    pub instances_compiled: AtomicU64,
    pub merged_inst: AtomicU64,
    pub merged_inst_opt: AtomicU64,
    pub empty_instances: AtomicU64,
    pub empty_groups: AtomicU64,
    // -- Memory tracking --
    pub memory_current: AtomicU64,
    pub memory_peak: AtomicU64,
    // -- Optimization --
    pub optimization_time_ticks: AtomicU64,
    pub preopt_syms: AtomicU64,
    pub postopt_syms: AtomicU64,
    pub preopt_ops: AtomicU64,
    pub postopt_ops: AtomicU64,
    // -- Texture --
    pub tex_calls_codegened: AtomicU64,
    pub tex_calls_as_handles: AtomicU64,
    // -- Runtime calls --
    pub getattribute_calls: AtomicU64,
    pub get_userdata_calls: AtomicU64,
    pub noise_calls: AtomicU64,
    pub pointcloud_searches: AtomicU64,
}

impl ShadingStats {
    pub fn new() -> Self {
        Self {
            shaders_loaded: AtomicU64::new(0),
            shaders_requested: AtomicU64::new(0),
            groups_created: AtomicU64::new(0),
            regexes_compiled: AtomicU64::new(0),
            connections_total: AtomicU64::new(0),
            layers_executed: AtomicU64::new(0),
            total_shading_time_ticks: AtomicU64::new(0),
            groups_compiled: AtomicU64::new(0),
            instances_compiled: AtomicU64::new(0),
            merged_inst: AtomicU64::new(0),
            merged_inst_opt: AtomicU64::new(0),
            empty_instances: AtomicU64::new(0),
            empty_groups: AtomicU64::new(0),
            memory_current: AtomicU64::new(0),
            memory_peak: AtomicU64::new(0),
            optimization_time_ticks: AtomicU64::new(0),
            preopt_syms: AtomicU64::new(0),
            postopt_syms: AtomicU64::new(0),
            preopt_ops: AtomicU64::new(0),
            postopt_ops: AtomicU64::new(0),
            tex_calls_codegened: AtomicU64::new(0),
            tex_calls_as_handles: AtomicU64::new(0),
            getattribute_calls: AtomicU64::new(0),
            get_userdata_calls: AtomicU64::new(0),
            noise_calls: AtomicU64::new(0),
            pointcloud_searches: AtomicU64::new(0),
        }
    }
}

impl Default for ShadingStats {
    fn default() -> Self {
        Self::new()
    }
}

/// The main shading system object.
pub struct ShadingSystem {
    /// Renderer services.
    renderer: Arc<dyn RendererServices>,
    /// Error handler.
    errhandler: Arc<dyn ErrorHandler>,
    /// Loaded shader masters, keyed by name.
    masters: Mutex<HashMap<UString, Arc<ShaderMaster>>>,
    /// Registered closures.
    closures: Mutex<Vec<ClosureEntry>>,
    /// Shader search paths (colon/semicolon separated).
    searchpath: Mutex<String>,
    /// System attributes.
    attributes: Mutex<HashMap<String, AttributeValue>>,
    /// Raytype name -> bit mapping.
    /// Ordered list of raytype names (index i => bit 1<<i). Populated from attribute("raytypes").
    raytypes: Mutex<Vec<UString>>,
    /// Strict message validation (duplicate/type/layer checks). Default true, matching C++.
    strict_messages: Mutex<bool>,
    /// Known symbol location mappings (sorted by name).
    symlocs: Mutex<Vec<SymLocationDesc>>,
    /// Runtime statistics counters.
    stats: ShadingStats,
    /// Set of function names that should be force-inlined during JIT/codegen.
    /// Populated via `register_inline_function(name, true)`.
    inline_functions: Mutex<HashSet<String>>,
    /// Set of function names that should never be inlined during JIT/codegen.
    /// Populated via `register_inline_function(name, false)`.
    noinline_functions: Mutex<HashSet<String>>,
    /// JIT compilation cache: group name -> compiled shader.
    /// Avoids re-compiling the same shader group on every `execute_jit` call.
    /// Matches the C++ OSL pattern where compiled code is cached per group.
    #[cfg(feature = "jit")]
    jit_cache: Mutex<HashMap<UString, Arc<crate::jit::CompiledShader>>>,
}

/// Attribute values.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    Int(i32),
    Float(f32),
    String(String),
    /// String array, used for "raytypes", "renderer_outputs", etc.
    StringArray(Vec<String>),
}

impl ShadingSystem {
    /// Create a new ShadingSystem.
    pub fn new(
        renderer: Arc<dyn RendererServices>,
        errhandler: Option<Arc<dyn ErrorHandler>>,
    ) -> Self {
        Self {
            renderer,
            errhandler: errhandler.unwrap_or_else(|| Arc::new(StdErrorHandler)),
            masters: Mutex::new(HashMap::new()),
            closures: Mutex::new(Vec::new()),
            searchpath: Mutex::new(String::new()),
            attributes: Mutex::new(HashMap::new()),
            raytypes: Mutex::new(vec![
                UString::new("camera"),
                UString::new("shadow"),
                UString::new("reflection"),
                UString::new("refraction"),
                UString::new("diffuse"),
                UString::new("glossy"),
                UString::new("subsurface"),
                UString::new("displacement"),
            ]),
            strict_messages: Mutex::new(true),
            symlocs: Mutex::new(Vec::new()),
            stats: ShadingStats::new(),
            inline_functions: Mutex::new(HashSet::new()),
            noinline_functions: Mutex::new(HashSet::new()),
            #[cfg(feature = "jit")]
            jit_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get the error handler. Used by ShadingContext::process_errors.
    pub fn errhandler(&self) -> &Arc<dyn ErrorHandler> {
        &self.errhandler
    }

    /// Get the runtime statistics counters.
    pub fn stats(&self) -> &ShadingStats {
        &self.stats
    }

    /// Set an attribute on the shading system.
    pub fn attribute(&self, name: &str, val: AttributeValue) {
        if name == "searchpath:shader" {
            if let AttributeValue::String(ref s) = val {
                *self.searchpath.lock().unwrap() = s.clone();
            }
        }
        if name == "strict_messages" {
            if let AttributeValue::Int(i) = val {
                *self.strict_messages.lock().unwrap() = i != 0;
            }
        }
        if name == "raytypes" {
            if let AttributeValue::StringArray(ref arr) = val {
                let mut rt = self.raytypes.lock().unwrap();
                rt.clear();
                for s in arr {
                    rt.push(UString::new(s));
                }
            }
            self.attributes
                .lock()
                .unwrap()
                .insert(name.to_string(), val);
            return;
        }
        // C++ uses "commonspace"; mirror to both for getattribute compatibility
        if (name == "commonspace" || name == "commonspace_synonym")
            && matches!(val, AttributeValue::String(_))
        {
            self.attributes
                .lock()
                .unwrap()
                .insert("commonspace".to_string(), val.clone());
            self.attributes
                .lock()
                .unwrap()
                .insert("commonspace_synonym".to_string(), val);
            return;
        }
        self.attributes
            .lock()
            .unwrap()
            .insert(name.to_string(), val);
    }

    /// Whether strict message validation is enabled (duplicate/type/layer checks).
    /// Matching C++ ShadingSystemImpl::strict_messages.
    pub fn strict_messages(&self) -> bool {
        *self.strict_messages.lock().unwrap()
    }

    /// Synonym for "common" space in transform/getmatrix (e.g. "world").
    /// Matching C++ ShadingSystem::commonspace_synonym.
    /// Attribute name in C++ API is "commonspace"; "commonspace_synonym" supported for compat.
    pub fn commonspace_synonym(&self) -> UString {
        match self
            .getattribute("commonspace")
            .or_else(|| self.getattribute("commonspace_synonym"))
        {
            Some(AttributeValue::String(s)) => UString::new(&s),
            _ => UString::new("world"),
        }
    }

    /// Whether range checking is enabled for aref/aassign/compref/compassign/mxcompref/mxcompassign.
    /// Matching C++ ShadingSystem::range_checking.
    pub fn range_checking(&self) -> bool {
        match self.getattribute("range_checking") {
            Some(AttributeValue::Int(i)) => i != 0,
            _ => true,
        }
    }

    /// Whether to emit errors when unknown coordinate systems are used in getmatrix/transform.
    /// Matching C++ ShadingSystem::unknown_coordsys_error.
    pub fn unknown_coordsys_error(&self) -> bool {
        match self.getattribute("unknown_coordsys_error") {
            Some(AttributeValue::Int(i)) => i != 0,
            _ => true,
        }
    }

    /// Get an attribute from the shading system.
    ///
    /// Supports well-known named attributes:
    /// - `"statistics:level"` -> Int (verbosity)
    /// - `"optimize"` -> Int (optimization level)
    /// - `"lockgeom"` -> Int (lock geometry flag)
    /// - `"debug"` -> Int (debug level)
    /// - `"commonspace"` -> String (C++ canonical; "commonspace_synonym" alias)
    pub fn getattribute(&self, name: &str) -> Option<AttributeValue> {
        // Check user-set attributes first
        let attrs = self.attributes.lock().unwrap();
        if let Some(val) = attrs.get(name) {
            return Some(val.clone());
        }
        drop(attrs);

        // Return defaults for well-known attributes (matching C++ ShadingSystemImpl)
        match name {
            "statistics:level" => Some(AttributeValue::Int(0)),
            "optimize" => Some(AttributeValue::Int(2)),
            "lockgeom" => Some(AttributeValue::Int(1)),
            "debug" => Some(AttributeValue::Int(0)),
            "commonspace" | "commonspace_synonym" => {
                Some(AttributeValue::String("world".to_string()))
            }
            "searchpath:shader" => Some(AttributeValue::String(String::new())),
            "searchpath:texture" => Some(AttributeValue::String(String::new())),
            "colorspace" => Some(AttributeValue::String("Rec709".to_string())),
            "range_checking" => Some(AttributeValue::Int(1)),
            "strict_messages" => Some(AttributeValue::Int(
                if *self.strict_messages.lock().unwrap() {
                    1
                } else {
                    0
                },
            )),
            "greedyjit" => Some(AttributeValue::Int(0)),
            "max_warnings_per_thread" => Some(AttributeValue::Int(100)),
            "error_repeats" => Some(AttributeValue::Int(0)),
            "lazylayers" => Some(AttributeValue::Int(1)),
            "lazyerror" => Some(AttributeValue::Int(1)),
            "relaxed_param_typecheck" => Some(AttributeValue::Int(0)),
            "unknown_coordsys_error" => Some(AttributeValue::Int(1)),
            "connection_error" => Some(AttributeValue::Int(1)),
            "countlayerexecs" => Some(AttributeValue::Int(0)),
            "profile" => Some(AttributeValue::Int(0)),
            "raytypes" => {
                let rt = self.raytypes.lock().unwrap();
                Some(AttributeValue::StringArray(
                    rt.iter().map(|u| u.as_str().to_string()).collect(),
                ))
            }
            _ => None,
        }
    }

    /// For a shader globals name, return the corresponding SGBits. C++ ShadingSystem::globals_bit.
    pub fn globals_bit(name: &str) -> crate::shaderglobals::SGBits {
        use crate::shaderglobals::SGBits;
        match name {
            "P" => SGBits::P,
            "I" => SGBits::I,
            "N" => SGBits::N,
            "Ng" => SGBits::NG,
            "u" => SGBits::U,
            "v" => SGBits::V,
            "dPdu" => SGBits::DPDU,
            "dPdv" => SGBits::DPDV,
            "time" => SGBits::TIME,
            "dtime" => SGBits::DTIME,
            "dPdtime" => SGBits::DPDTIME,
            "Ps" => SGBits::PS,
            "Ci" => SGBits::CI,
            _ => SGBits::empty(),
        }
    }

    /// For an SGBits value, return the shader globals name. C++ ShadingSystem::globals_name.
    pub fn globals_name(bit: crate::shaderglobals::SGBits) -> Option<&'static str> {
        use crate::shaderglobals::SGBits;
        if bit == SGBits::empty() {
            return None;
        }
        // Return name for single-bit; C++ returns ustring() for None/empty
        if bit == SGBits::P {
            Some("P")
        } else if bit == SGBits::I {
            Some("I")
        } else if bit == SGBits::N {
            Some("N")
        } else if bit == SGBits::NG {
            Some("Ng")
        } else if bit == SGBits::U {
            Some("u")
        } else if bit == SGBits::V {
            Some("v")
        } else if bit == SGBits::DPDU {
            Some("dPdu")
        } else if bit == SGBits::DPDV {
            Some("dPdv")
        } else if bit == SGBits::TIME {
            Some("time")
        } else if bit == SGBits::DTIME {
            Some("dtime")
        } else if bit == SGBits::DPDTIME {
            Some("dPdtime")
        } else if bit == SGBits::PS {
            Some("Ps")
        } else if bit == SGBits::CI {
            Some("Ci")
        } else {
            None
        }
    }

    /// Return the bit for a raytype name (from attribute "raytypes"). 0 if unknown. C++ raytype_bit.
    pub fn raytype_bit(&self, name: &str) -> u32 {
        let u = UString::new(name);
        let raytypes = self.raytypes.lock().unwrap();
        for (i, rt) in raytypes.iter().enumerate() {
            if *rt == u {
                return 1 << i;
            }
        }
        0
    }

    // ----- Closure registration -----

    /// Register a closure type.
    pub fn register_closure(&self, name: &str, id: i32, params: Vec<ClosureParam>) {
        let entry = ClosureEntry {
            name: UString::new(name),
            id,
            params,
        };
        self.closures.lock().unwrap().push(entry);
    }

    /// Query a registered closure by name.
    pub fn query_closure(&self, name: &str) -> Option<(i32, Vec<ClosureParam>)> {
        let u = UString::new(name);
        let closures = self.closures.lock().unwrap();
        closures
            .iter()
            .find(|c| c.name == u)
            .map(|c| (c.id, c.params.clone()))
    }

    // ----- Shader loading -----

    /// Load a shader master from an OSO file or memory buffer.
    pub fn load_shader(&self, name: &str) -> Result<Arc<ShaderMaster>, String> {
        self.stats.shaders_requested.fetch_add(1, Ordering::Relaxed);
        let uname = UString::new(name);

        // Check cache first
        {
            let masters = self.masters.lock().unwrap();
            if let Some(m) = masters.get(&uname) {
                return Ok(m.clone());
            }
        }

        // Search for the .oso file
        let searchpath = self.searchpath.lock().unwrap().clone();
        let oso_data = self.find_and_load_oso(name, &searchpath)?;

        let oso = crate::oso::read_oso_string(&oso_data)
            .map_err(|e| format!("Failed to parse shader '{}': {}", name, e))?;

        let master = Arc::new(ShaderMaster {
            name: uname,
            shader_type: oso.shader_type,
            oso,
        });

        self.masters.lock().unwrap().insert(uname, master.clone());
        self.stats.shaders_loaded.fetch_add(1, Ordering::Relaxed);
        Ok(master)
    }

    /// Load a shader from a memory buffer.
    pub fn load_memory_shader(
        &self,
        name: &str,
        buffer: &str,
    ) -> Result<Arc<ShaderMaster>, String> {
        let uname = UString::new(name);

        let oso = crate::oso::read_oso_string(buffer)
            .map_err(|e| format!("Failed to parse shader '{}': {}", name, e))?;

        let master = Arc::new(ShaderMaster {
            name: uname,
            shader_type: oso.shader_type,
            oso,
        });

        self.masters.lock().unwrap().insert(uname, master.clone());
        Ok(master)
    }

    fn find_and_load_oso(&self, name: &str, searchpath: &str) -> Result<String, String> {
        let filename = if name.ends_with(".oso") {
            name.to_string()
        } else {
            format!("{name}.oso")
        };

        // Try absolute/relative path first
        if let Ok(content) = std::fs::read_to_string(&filename) {
            return Ok(content);
        }

        // Search in paths
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in searchpath.split(sep) {
            let dir = dir.trim();
            if dir.is_empty() {
                continue;
            }
            let path = format!("{dir}/{filename}");
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Ok(content);
            }
        }

        Err(format!("Shader '{}' not found in searchpath", name))
    }

    // ----- ShaderGroup management -----

    /// Begin constructing a new shader group.
    /// Group inherits global symlocs active at creation time (matching C++ ShaderGroupBegin).
    pub fn shader_group_begin(&self, name: &str) -> ShaderGroupRef {
        self.stats.groups_created.fetch_add(1, Ordering::Relaxed);
        let mut group = ShaderGroup::new(name);
        let global_symlocs = self.symlocs.lock().unwrap().clone();
        group.add_symlocs(&global_symlocs);
        Arc::new(Mutex::new(group))
    }

    /// Begin a shader group by parsing a serialized group specification.
    /// Matching C++ ShadingSystem::ShaderGroupBegin(groupname, usage, groupspec).
    /// Parses param/shader/connect statements and builds the group.
    /// Caller must call shader_group_end() when done.
    pub fn shader_group_begin_from_serialized(
        &self,
        name: &str,
        usage: &str,
        groupspec: &str,
    ) -> Result<ShaderGroupRef, String> {
        let group = self.shader_group_begin(name);
        self.parse_and_build_groupspec(&group, usage, groupspec)?;
        Ok(group)
    }

    /// Parse groupspec string and build the group. Matching C++ ShaderGroupBegin groupspec parser.
    fn parse_and_build_groupspec(
        &self,
        group: &ShaderGroupRef,
        usage: &str,
        spec: &str,
    ) -> Result<(), String> {
        let mut p = spec.trim();
        while !p.is_empty() {
            p = p.trim_start();
            if p.is_empty() {
                break;
            }
            while p.starts_with(';') || p.starts_with(',') {
                p = &p[1..];
                p = p.trim_start();
            }
            if p.is_empty() {
                break;
            }
            let word = parse_word(&mut p);
            if word.is_empty() {
                break;
            }

            if word == "shader" {
                let shadername = parse_identifier(&mut p);
                p = p.trim_start();
                let layername = parse_until_space_or_delim(&mut p).to_string();
                eat_char(&mut p, ';');
                eat_char(&mut p, ',');
                self.shader(group, usage, &shadername, &layername)
                    .map_err(|e| format!("shader {} {}: {}", shadername, layername, e))?;
                continue;
            }

            if word == "connect" {
                skip_ws(&mut p);
                let lay1 = parse_until_one_of(&mut p, " \t\r\n,;.").to_string();
                eat_char(&mut p, '.');
                skip_ws(&mut p);
                let param1 = parse_until_space_or_delim(&mut p).to_string();
                skip_ws(&mut p);
                let lay2 = parse_until_one_of(&mut p, " \t\r\n,;.").to_string();
                eat_char(&mut p, '.');
                skip_ws(&mut p);
                let param2 = parse_until_space_or_delim(&mut p).to_string();
                eat_char(&mut p, ';');
                eat_char(&mut p, ',');
                self.connect_shaders(group, &lay1, &param1, &lay2, &param2)
                    .map_err(|e| {
                        format!("connect {}.{} {}.{}: {}", lay1, param1, lay2, param2, e)
                    })?;
                continue;
            }

            let typestr = if word == "param" {
                parse_word(&mut p)
            } else {
                word.clone()
            };
            let (base_type, array_len) = parse_type_and_array(&mut p, typestr)?;
            let paramname = parse_param_name(&mut p);
            let pv = parse_param_values(&mut p, &base_type, array_len)?;
            skip_ws(&mut p);
            let mut hints = ParamHints::NONE;
            while p.starts_with("[[") {
                hints = hints | parse_param_hints(&mut p)?;
            }
            eat_char(&mut p, ';');
            eat_char(&mut p, ',');
            self.parameter(group, &paramname, pv, hints);
        }
        Ok(())
    }

    /// End construction of a shader group. Validates the group.
    ///
    /// Checks:
    /// - At least one layer must be present
    /// - All connections reference valid layer indices
    /// - Marks the group as complete
    pub fn shader_group_end(&self, group: &ShaderGroupRef) -> Result<(), String> {
        let mut grp = group.lock().unwrap();

        if grp.layers.is_empty() {
            return Err(format!(
                "Shader group '{}': no layers added",
                grp.name.as_str()
            ));
        }

        let nlayers = grp.layers.len() as i32;
        for conn in &grp.connections {
            if conn.src_layer < 0 || conn.src_layer >= nlayers {
                return Err(format!(
                    "Shader group '{}': connection source layer {} out of range (0..{})",
                    grp.name.as_str(),
                    conn.src_layer,
                    nlayers
                ));
            }
            if conn.dst_layer < 0 || conn.dst_layer >= nlayers {
                return Err(format!(
                    "Shader group '{}': connection destination layer {} out of range (0..{})",
                    grp.name.as_str(),
                    conn.dst_layer,
                    nlayers
                ));
            }
        }

        self.stats
            .connections_total
            .fetch_add(grp.connections.len() as u64, Ordering::Relaxed);
        grp.complete = true;
        Ok(())
    }

    /// Set a parameter that will be applied to the next Shader() call.
    /// Matching C++ `ShadingSystem::Parameter(name, type, val, hints)`.
    #[allow(clippy::fn_params_default_trait)]
    pub fn parameter(
        &self,
        group: &ShaderGroupRef,
        name: &str,
        value: ParamValue,
        hints: ParamHints,
    ) {
        group
            .lock()
            .unwrap()
            .pending_params
            .push((UString::new(name), value, hints));
    }

    /// Convenience: set a parameter with no hints (same as parameter(..., ParamHints::NONE)).
    pub fn parameter_simple(&self, group: &ShaderGroupRef, name: &str, value: ParamValue) {
        self.parameter(group, name, value, ParamHints::NONE);
    }

    /// Add a shader layer to the group.
    pub fn shader(
        &self,
        group: &ShaderGroupRef,
        _usage: &str,
        shader_name: &str,
        layer_name: &str,
    ) -> Result<(), String> {
        let master = self.load_shader(shader_name)?;

        let mut grp = group.lock().unwrap();
        let pending = std::mem::take(&mut grp.pending_params);

        let mut overrides = HashMap::new();
        let mut hints_map = HashMap::new();
        for (k, v, h) in pending {
            overrides.insert(k.clone(), v);
            if h.0 != 0 {
                hints_map.insert(k, h);
            }
        }

        let instance = ShaderInstance {
            master,
            layer_name: UString::new(layer_name),
            param_overrides: overrides,
            param_hints: hints_map,
            unused: false,
        };

        grp.layers.push(instance);
        Ok(())
    }

    /// Connect two shader layers.
    pub fn connect_shaders(
        &self,
        group: &ShaderGroupRef,
        src_layer: &str,
        src_param: &str,
        dst_layer: &str,
        dst_param: &str,
    ) -> Result<(), String> {
        let grp = group.lock().unwrap();
        let src_idx = grp
            .find_layer(src_layer)
            .ok_or_else(|| format!("Source layer '{}' not found", src_layer))?;
        let dst_idx = grp
            .find_layer(dst_layer)
            .ok_or_else(|| format!("Destination layer '{}' not found", dst_layer))?;
        drop(grp);

        let conn = Connection {
            src_layer: src_idx as i32,
            src_param: UString::new(src_param),
            dst_layer: dst_idx as i32,
            dst_param: UString::new(dst_param),
        };

        group.lock().unwrap().connections.push(conn);
        Ok(())
    }

    /// Get the renderer services.
    pub fn renderer(&self) -> &dyn RendererServices {
        &*self.renderer
    }

    /// Report an error.
    pub fn error(&self, msg: &str) {
        self.errhandler.error(msg);
    }

    /// Report a warning.
    pub fn warning(&self, msg: &str) {
        self.errhandler.warning(msg);
    }

    /// Report info.
    pub fn info(&self, msg: &str) {
        self.errhandler.info(msg);
    }

    // ----- Thread / context lifecycle (C++ parity) -----

    /// Create per-thread data needed for shader execution.
    /// Matches C++ `ShadingSystem::create_thread_info()`.
    /// Each renderer thread should hold exactly one and never share it.
    pub fn create_thread_info(&self) -> crate::context::PerThreadInfo {
        // Use 0 as default thread index; callers can set it.
        crate::context::PerThreadInfo::new(0)
    }

    /// Destroy per-thread info. Matches C++ `ShadingSystem::destroy_thread_info()`.
    /// In Rust this is a no-op (drop semantics), kept for API parity.
    pub fn destroy_thread_info(&self, _info: crate::context::PerThreadInfo) {}

    /// Acquire a ShadingContext for the current thread.
    /// Matches C++ `ShadingSystem::get_context()`.
    /// The context should not be shared between threads.
    pub fn get_context(
        &self,
        thread_info: &crate::context::PerThreadInfo,
    ) -> crate::context::ShadingContext {
        crate::context::ShadingContext::new(thread_info.thread_index)
    }

    /// Release a ShadingContext back to the pool.
    /// Matches C++ `ShadingSystem::release_context()`.
    /// In Rust, simply drops the context (no actual pool yet).
    pub fn release_context(&self, _ctx: crate::context::ShadingContext) {}

    // ----- Execution -----

    /// Execute a shader group at a single shading point.
    ///
    /// This is the main entry point for running shaders. It processes
    /// each layer in the group in order, passing connected outputs to
    /// downstream inputs.
    pub fn execute(
        &self,
        group: &ShaderGroupRef,
        globals: &crate::shaderglobals::ShaderGlobals,
    ) -> Result<ExecuteResult, String> {
        let grp = group.lock().unwrap();
        if grp.layers.is_empty() {
            return Err("Shader group has no layers".into());
        }

        let mut layer_results: Vec<Option<crate::interp::Interpreter>> = Vec::new();

        for (layer_idx, instance) in grp.layers.iter().enumerate() {
            // Convert the OSO to ShaderIR
            let mut ir = oso_to_ir(&instance.master.oso);

            // Apply parameter overrides
            for (name, value) in &instance.param_overrides {
                let hints = instance
                    .param_hints
                    .get(name)
                    .copied()
                    .unwrap_or(ParamHints::NONE);
                apply_param_override(&mut ir, name, value, hints);
            }

            // Apply connections from upstream layers
            for conn in &grp.connections {
                if conn.dst_layer == layer_idx as i32 {
                    if let Some(Some(src_interp)) = layer_results.get(conn.src_layer as usize) {
                        let src_ir = oso_to_ir(&grp.layers[conn.src_layer as usize].master.oso);
                        if let Some(val) =
                            src_interp.get_symbol_value(&src_ir, conn.src_param.as_str())
                        {
                            apply_connection_value(&mut ir, conn.dst_param.as_str(), &val);
                        }
                    }
                }
            }

            // Execute this layer with renderer services
            let mut interp = crate::interp::Interpreter::with_renderer(self.renderer.clone());
            interp.set_commonspace_synonym(self.commonspace_synonym());
            interp.set_range_checking(self.range_checking());
            interp.set_unknown_coordsys_error(self.unknown_coordsys_error());
            interp.execute(&ir, globals, None);
            layer_results.push(Some(interp));
        }

        // The result is from the last layer
        let last_interp = layer_results
            .pop()
            .flatten()
            .ok_or_else(|| "No execution result".to_string())?;
        let last_ir = oso_to_ir(&grp.layers.last().unwrap().master.oso);

        Ok(ExecuteResult {
            ir: last_ir,
            interp: last_interp,
        })
    }

    /// Optimize a shader group.
    /// Matches C++ `ShadingSystem::optimize_group`.
    pub fn optimize_group(
        &self,
        group: &ShaderGroupRef,
        level: Option<crate::optimizer::OptLevel>,
    ) -> crate::optimizer::OptStats {
        let mut grp = group.lock().unwrap();
        if grp.optimized {
            return crate::optimizer::OptStats::default();
        }

        let opt_level = level.unwrap_or(crate::optimizer::OptLevel::O2);
        let mut total_stats = crate::optimizer::OptStats::default();

        for instance in &grp.layers {
            let mut ir = oso_to_ir(&instance.master.oso);
            // Apply overrides before optimizing
            for (name, value) in &instance.param_overrides {
                let hints = instance
                    .param_hints
                    .get(name)
                    .copied()
                    .unwrap_or(ParamHints::NONE);
                apply_param_override(&mut ir, name, value, hints);
            }
            let stats = crate::optimizer::optimize(&mut ir, opt_level);
            total_stats.constant_folds += stats.constant_folds;
            total_stats.dead_ops_eliminated += stats.dead_ops_eliminated;
            total_stats.temps_coalesced += stats.temps_coalesced;
            total_stats.peephole_opts += stats.peephole_opts;
            total_stats.total_passes += stats.total_passes;
        }

        grp.optimized = true;
        total_stats
    }

    /// Get statistics about the shading system.
    pub fn getstats(&self) -> ShadingSystemStats {
        let masters = self.masters.lock().unwrap();
        let closures = self.closures.lock().unwrap();
        let s = &self.stats;
        ShadingSystemStats {
            masters_loaded: masters.len() as u32,
            closures_registered: closures.len() as u32,
            shaders_requested: s.shaders_requested.load(Ordering::Relaxed),
            shaders_loaded: s.shaders_loaded.load(Ordering::Relaxed),
            groups_created: s.groups_created.load(Ordering::Relaxed),
            groups_compiled: s.groups_compiled.load(Ordering::Relaxed),
            instances_compiled: s.instances_compiled.load(Ordering::Relaxed),
            merged_inst: s.merged_inst.load(Ordering::Relaxed),
            merged_inst_opt: s.merged_inst_opt.load(Ordering::Relaxed),
            empty_instances: s.empty_instances.load(Ordering::Relaxed),
            empty_groups: s.empty_groups.load(Ordering::Relaxed),
            layers_executed: s.layers_executed.load(Ordering::Relaxed),
            regexes_compiled: s.regexes_compiled.load(Ordering::Relaxed),
            connections_total: s.connections_total.load(Ordering::Relaxed),
            memory_current: s.memory_current.load(Ordering::Relaxed),
            memory_peak: s.memory_peak.load(Ordering::Relaxed),
            optimization_time_ticks: s.optimization_time_ticks.load(Ordering::Relaxed),
            preopt_syms: s.preopt_syms.load(Ordering::Relaxed),
            postopt_syms: s.postopt_syms.load(Ordering::Relaxed),
            preopt_ops: s.preopt_ops.load(Ordering::Relaxed),
            postopt_ops: s.postopt_ops.load(Ordering::Relaxed),
            tex_calls_codegened: s.tex_calls_codegened.load(Ordering::Relaxed),
            tex_calls_as_handles: s.tex_calls_as_handles.load(Ordering::Relaxed),
            getattribute_calls: s.getattribute_calls.load(Ordering::Relaxed),
            get_userdata_calls: s.get_userdata_calls.load(Ordering::Relaxed),
            noise_calls: s.noise_calls.load(Ordering::Relaxed),
            pointcloud_searches: s.pointcloud_searches.load(Ordering::Relaxed),
            total_shading_time_ticks: s.total_shading_time_ticks.load(Ordering::Relaxed),
        }
    }

    /// Generate a formatted statistics report string.
    /// Matches C++ `ShadingSystemImpl::getstats(int level)` output.
    pub fn generate_report(&self) -> String {
        use std::fmt::Write;
        let st = self.getstats();
        let mut out = String::with_capacity(2048);

        let _ = writeln!(out, "OSL ShadingSystem Statistics");

        if st.shaders_requested == 0 && st.shaders_loaded == 0 {
            let _ = writeln!(out, "  No shaders requested or loaded");
            return out;
        }

        let _ = writeln!(out, "  Shaders:");
        let _ = writeln!(out, "    Requested: {}", st.shaders_requested);
        let _ = writeln!(out, "    Loaded:    {}", st.shaders_loaded);
        let _ = writeln!(out, "    Masters:   {}", st.masters_loaded);
        let _ = writeln!(out, "  Shading groups:   {}", st.groups_created);
        let _ = writeln!(out, "    Connections:     {}", st.connections_total);

        if st.layers_executed > 0 {
            let _ = writeln!(out, "  Total layers executed: {}", st.layers_executed);
        }

        let _ = writeln!(
            out,
            "  Compiled {} groups, {} instances",
            st.groups_compiled, st.instances_compiled
        );
        let total_merged = st.merged_inst + st.merged_inst_opt;
        let _ = writeln!(
            out,
            "  Merged {} instances ({} initial, {} after opt)",
            total_merged, st.merged_inst, st.merged_inst_opt
        );

        if st.instances_compiled > 0 {
            let pct = 100.0 * st.empty_instances as f64 / st.instances_compiled as f64;
            let _ = writeln!(
                out,
                "  After optimization, {} empty instances ({:.0}%)",
                st.empty_instances, pct
            );
        }
        if st.groups_compiled > 0 {
            let pct = 100.0 * st.empty_groups as f64 / st.groups_compiled as f64;
            let _ = writeln!(
                out,
                "  After optimization, {} empty groups ({:.0}%)",
                st.empty_groups, pct
            );
        }

        if st.preopt_ops > 0 {
            let ops_pct = 100.0 * (st.postopt_ops as f64 / st.preopt_ops.max(1) as f64 - 1.0);
            let _ = writeln!(
                out,
                "  Optimized {} ops to {} ({:.1}%)",
                st.preopt_ops, st.postopt_ops, ops_pct
            );
        }
        if st.preopt_syms > 0 {
            let syms_pct = 100.0 * (st.postopt_syms as f64 / st.preopt_syms.max(1) as f64 - 1.0);
            let _ = writeln!(
                out,
                "  Optimized {} symbols to {} ({:.1}%)",
                st.preopt_syms, st.postopt_syms, syms_pct
            );
        }

        let _ = writeln!(
            out,
            "  Texture calls compiled: {} ({} used handles)",
            st.tex_calls_codegened, st.tex_calls_as_handles
        );
        let _ = writeln!(out, "  Regex's compiled: {}", st.regexes_compiled);

        if st.getattribute_calls > 0 {
            let _ = writeln!(out, "  getattribute calls: {}", st.getattribute_calls);
        }
        let _ = writeln!(
            out,
            "  Number of get_userdata calls: {}",
            st.get_userdata_calls
        );
        if st.noise_calls > 0 {
            let _ = writeln!(out, "  Number of noise calls: {}", st.noise_calls);
        }
        if st.pointcloud_searches > 0 {
            let _ = writeln!(out, "  pointcloud_search calls: {}", st.pointcloud_searches);
        }

        // Memory
        let _ = writeln!(
            out,
            "  Memory: current {} bytes, peak {} bytes",
            st.memory_current, st.memory_peak
        );

        if st.total_shading_time_ticks > 0 {
            let _ = writeln!(
                out,
                "  Total shader execution time: {} ticks",
                st.total_shading_time_ticks
            );
        }

        out
    }

    // ----- Advanced API (matching C++ ShadingSystem) -----

    /// Replace a parameter value in a previously-declared shader group.
    /// Matching C++ `ShadingSystem::ReParameter`.
    pub fn reparameter(
        &self,
        group: &ShaderGroupRef,
        layer_name: &str,
        param_name: &str,
        value: ParamValue,
    ) -> Result<(), String> {
        let mut grp = group.lock().unwrap();
        let layer_idx = grp
            .find_layer(layer_name)
            .ok_or_else(|| format!("Layer '{}' not found", layer_name))?;
        grp.layers[layer_idx]
            .param_overrides
            .insert(UString::new(param_name), value);
        // Reset optimized flag since params changed
        grp.optimized = false;
        Ok(())
    }

    /// Set an attribute on a specific shader group.
    /// Matching C++ `ShadingSystem::attribute(ShaderGroup*, ...)`.
    pub fn group_attribute(&self, group: &ShaderGroupRef, name: &str, val: AttributeValue) {
        let mut grp = group.lock().unwrap();
        match name {
            "renderer_outputs" => {
                // C++: clear then emplace_back each element
                if let AttributeValue::String(ref s) = val {
                    grp.renderer_outputs.clear();
                    for item in s.split(',') {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            let u = UString::new(trimmed);
                            if !grp.renderer_outputs.contains(&u) {
                                grp.renderer_outputs.push(u);
                            }
                        }
                    }
                }
                return;
            }
            "entry_layers" => {
                // C++: clear_entry_layers then mark_entry_layer for each
                if let AttributeValue::String(ref s) = val {
                    grp.entry_layers.clear();
                    for item in s.split(',') {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            let u = UString::new(trimmed);
                            if !grp.entry_layers.contains(&u) {
                                grp.entry_layers.push(u);
                            }
                        }
                    }
                }
                return;
            }
            _ => {}
        }
        drop(grp);
        // Store globally for retrieval
        let key = format!("group:{}", name);
        self.attributes.lock().unwrap().insert(key, val);
    }

    /// Get an attribute about a particular shader group.
    /// Matching C++ `ShadingSystem::getattribute(ShaderGroup*, ...)`.
    pub fn group_getattribute(&self, group: &ShaderGroupRef, name: &str) -> Option<AttributeValue> {
        let grp = group.lock().unwrap();
        match name {
            "groupname" => Some(AttributeValue::String(grp.name.as_str().to_string())),
            "num_layers" => Some(AttributeValue::Int(grp.nlayers() as i32)),
            "num_renderer_outputs" => Some(AttributeValue::Int(grp.renderer_outputs.len() as i32)),
            "renderer_outputs" => {
                let joined = grp
                    .renderer_outputs
                    .iter()
                    .map(|u| u.as_str().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                Some(AttributeValue::String(joined))
            }
            "num_entry_layers" => Some(AttributeValue::Int(grp.entry_layers.len() as i32)),
            "entry_layers" => {
                let joined = grp
                    .entry_layers
                    .iter()
                    .map(|u| u.as_str().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                Some(AttributeValue::String(joined))
            }
            "pickle" => Some(AttributeValue::String(grp.serialize())),
            _ => {
                let key = format!("group:{}", name);
                self.attributes.lock().unwrap().get(&key).cloned()
            }
        }
    }

    /// Configure the default raytypes for a shader group.
    /// Matching C++ `ShadingSystem::set_raytypes`.
    pub fn set_raytypes(&self, _group: &ShaderGroupRef, _raytypes_on: i32, _raytypes_off: i32) {
        // Store for optimizer to use
    }

    /// Clear known symbol location mappings (global).
    /// Matching C++ `ShadingSystem::clear_symlocs`.
    pub fn clear_symlocs(&self) {
        self.symlocs.lock().unwrap().clear();
    }

    /// Clear symbol locations for a group, or global if None. Matching C++ `clear_symlocs(ShaderGroup*)`.
    pub fn clear_symlocs_group(&self, group: Option<&ShaderGroupRef>) {
        match group {
            Some(g) => g.lock().unwrap().clear_symlocs(),
            None => self.clear_symlocs(),
        }
    }

    /// Add symbol location mappings (global). Matching C++ `ShadingSystem::add_symlocs`.
    pub fn add_symlocs(&self, symlocs: &[SymLocationDesc]) {
        let mut stored = self.symlocs.lock().unwrap();
        for s in symlocs {
            match stored.binary_search_by(|probe| probe.name.cmp(&s.name)) {
                Ok(idx) => stored[idx] = s.clone(),
                Err(idx) => stored.insert(idx, s.clone()),
            }
        }
    }

    /// Add symbol locations for a group, or global if None. Matching C++ `add_symlocs(ShaderGroup*, symlocs)`.
    pub fn add_symlocs_group(&self, group: Option<&ShaderGroupRef>, symlocs: &[SymLocationDesc]) {
        match group {
            Some(g) => g.lock().unwrap().add_symlocs(symlocs),
            None => self.add_symlocs(symlocs),
        }
    }

    /// Find a symbol location by name (global). Matching C++ `ShadingSystem::find_symloc`.
    pub fn find_symloc(&self, name: &str) -> Option<SymLocationDesc> {
        let name = UString::new(name);
        let stored = self.symlocs.lock().unwrap();
        match stored.binary_search_by(|probe| probe.name.cmp(&name)) {
            Ok(idx) => Some(stored[idx].clone()),
            Err(_) => None,
        }
    }

    /// Find symbol location by name in group, or global if None. Matching C++ `find_symloc(ShaderGroup*, name)`.
    pub fn find_symloc_group(
        &self,
        group: Option<&ShaderGroupRef>,
        name: &str,
    ) -> Option<SymLocationDesc> {
        match group {
            Some(g) => g.lock().unwrap().find_symloc(name),
            None => self.find_symloc(name),
        }
    }

    /// Find symloc by name and arena (global). C++ find_symloc(name, SymArena).
    pub fn find_symloc_arena(
        &self,
        name: &str,
        arena: crate::symbol::SymArena,
    ) -> Option<SymLocationDesc> {
        let name = UString::new(name);
        let stored = self.symlocs.lock().unwrap();
        match stored.binary_search_by(|probe| probe.name.cmp(&name)) {
            Ok(idx) => {
                let s = &stored[idx];
                if s.arena == arena && s.offset != -1 {
                    Some(s.clone())
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Find symloc by name and arena in group, or global if None. C++ find_symloc(ShaderGroup*, name, SymArena).
    pub fn find_symloc_group_arena(
        &self,
        group: Option<&ShaderGroupRef>,
        name: &str,
        arena: crate::symbol::SymArena,
    ) -> Option<SymLocationDesc> {
        match group {
            Some(g) => g.lock().unwrap().find_symloc_arena(name, arena),
            None => self.find_symloc_arena(name, arena),
        }
    }

    // ----- OSLQuery integration (Task 1) -----

    /// Query parameter information for a specific layer in a shader group.
    ///
    /// Returns `None` if `layer` index is out of range.
    /// The returned `OSLQueryInfo` mirrors what `OSLQuery::open()` provides
    /// in the C++ API, but operates directly on an already-loaded group.
    pub fn oslquery(&self, group: &ShaderGroupRef, layer: usize) -> Option<OSLQueryInfo> {
        let grp = group.lock().unwrap();
        let instance = grp.layers.get(layer)?;
        let oso = &instance.master.oso;

        let params = oso
            .symbols
            .iter()
            .filter(|s| {
                matches!(
                    s.symtype,
                    crate::symbol::SymType::Param | crate::symbol::SymType::OutputParam
                )
            })
            .map(|s| {
                let td = s.typespec.simpletype();
                ParamInfo {
                    name: s.name.clone(),
                    type_name: format!("{}", td),
                    is_output: s.symtype == crate::symbol::SymType::OutputParam,
                    valid_default: !s.idefault.is_empty()
                        || !s.fdefault.is_empty()
                        || !s.sdefault.is_empty(),
                    idefault: s.idefault.clone(),
                    fdefault: s.fdefault.clone(),
                    sdefault: s.sdefault.clone(),
                    is_closure: s.typespec.is_closure_based(),
                    is_struct: s.is_struct,
                    varlen_array: s.typespec.is_unsized_array(),
                }
            })
            .collect::<Vec<_>>();

        Some(OSLQueryInfo {
            shader_name: oso.shader_name.clone(),
            shader_type: oso.shader_type,
            num_params: params.len(),
            params,
        })
    }

    // ----- Inline function registration (Task 2) -----

    /// Mark a function as always-inline (`inline=true`) or never-inline (`inline=false`).
    ///
    /// When `inline=true` the function name is added to the force-inline set
    /// and removed from the no-inline set (and vice-versa).
    /// The JIT/codegen backend consults `is_inline_function` / `is_noinline_function`
    /// when deciding whether to emit an inline hint.
    ///
    /// Matches C++ `ShadingSystem::register_inline_function` / `register_noinline_function`.
    pub fn register_inline_function(&self, name: &str, inline: bool) {
        if inline {
            self.inline_functions
                .lock()
                .unwrap()
                .insert(name.to_string());
            self.noinline_functions.lock().unwrap().remove(name);
        } else {
            self.noinline_functions
                .lock()
                .unwrap()
                .insert(name.to_string());
            self.inline_functions.lock().unwrap().remove(name);
        }
    }

    /// Remove a function from both inline and no-inline sets (reset to default).
    ///
    /// Matches C++ `ShadingSystem::unregister_inline_function`.
    pub fn unregister_inline_function(&self, name: &str) {
        self.inline_functions.lock().unwrap().remove(name);
        self.noinline_functions.lock().unwrap().remove(name);
    }

    /// Returns true if `name` is registered as a force-inline function.
    pub fn is_inline_function(&self, name: &str) -> bool {
        self.inline_functions.lock().unwrap().contains(name)
    }

    /// Returns true if `name` is registered as a no-inline function.
    pub fn is_noinline_function(&self, name: &str) -> bool {
        self.noinline_functions.lock().unwrap().contains(name)
    }

    /// Execute init phase — bind group to context, optimize/JIT if needed.
    /// Matching C++ `ShadingSystem::execute_init`.
    pub fn execute_init(
        &self,
        group: &ShaderGroupRef,
        globals: &crate::shaderglobals::ShaderGlobals,
    ) -> Result<ExecuteContext, String> {
        let grp = group.lock().unwrap();
        if grp.layers.is_empty() {
            return Err("Shader group has no layers".into());
        }

        // Build IR for all layers
        let mut layer_irs: Vec<crate::codegen::ShaderIR> = Vec::new();
        for instance in &grp.layers {
            let mut ir = oso_to_ir(&instance.master.oso);
            for (name, value) in &instance.param_overrides {
                let hints = instance
                    .param_hints
                    .get(name)
                    .copied()
                    .unwrap_or(ParamHints::NONE);
                apply_param_override(&mut ir, name, value, hints);
            }
            layer_irs.push(ir);
        }

        Ok(ExecuteContext {
            layer_irs,
            layer_interps: Vec::new(),
            connections: grp.connections.clone(),
            globals: globals.clone(),
            executed: vec![false; grp.layers.len()],
            shared_messages: crate::message::MessageStore::new(),
        })
    }

    /// Execute a specific layer by index.
    /// Matching C++ `ShadingSystem::execute_layer`.
    pub fn execute_layer(
        &self,
        ctx: &mut ExecuteContext,
        layer_index: usize,
    ) -> Result<(), String> {
        if layer_index >= ctx.layer_irs.len() {
            return Err(format!("Layer index {} out of range", layer_index));
        }
        if ctx.executed[layer_index] {
            return Ok(()); // Already executed
        }

        // Execute upstream dependencies first
        for conn in &ctx.connections.clone() {
            if conn.dst_layer == layer_index as i32 && !ctx.executed[conn.src_layer as usize] {
                self.execute_layer(ctx, conn.src_layer as usize)?;
            }
        }

        // Apply connections from already-executed upstream layers
        for conn in &ctx.connections.clone() {
            if conn.dst_layer == layer_index as i32 {
                if let Some(interp) = ctx.layer_interps.get(conn.src_layer as usize) {
                    if let Some(interp) = interp {
                        let src_ir = &ctx.layer_irs[conn.src_layer as usize];
                        if let Some(val) = interp.get_symbol_value(src_ir, conn.src_param.as_str())
                        {
                            apply_connection_value(
                                &mut ctx.layer_irs[layer_index],
                                conn.dst_param.as_str(),
                                &val,
                            );
                        }
                    }
                }
            }
        }

        let mut interp = crate::interp::Interpreter::with_renderer(self.renderer.clone());
        interp.set_commonspace_synonym(self.commonspace_synonym());
        interp.set_range_checking(self.range_checking());
        interp.set_unknown_coordsys_error(self.unknown_coordsys_error());
        interp.execute(
            &ctx.layer_irs[layer_index],
            &ctx.globals,
            Some(crate::interp::ExecuteMessageConfig {
                shared_messages: &mut ctx.shared_messages,
                layeridx: layer_index as i32,
                strict: self.strict_messages(),
                errhandler: self.errhandler.as_ref(),
            }),
        );

        // Ensure we have enough slots
        while ctx.layer_interps.len() <= layer_index {
            ctx.layer_interps.push(None);
        }
        ctx.layer_interps[layer_index] = Some(interp);
        ctx.executed[layer_index] = true;
        Ok(())
    }

    /// Execute a layer by name.
    pub fn execute_layer_by_name(
        &self,
        ctx: &mut ExecuteContext,
        group: &ShaderGroupRef,
        layer_name: &str,
    ) -> Result<(), String> {
        let grp = group.lock().unwrap();
        let idx = grp
            .find_layer(layer_name)
            .ok_or_else(|| format!("Layer '{}' not found", layer_name))?;
        drop(grp);
        self.execute_layer(ctx, idx)
    }

    /// Finish execution — cleanup.
    /// Matching C++ `ShadingSystem::execute_cleanup`.
    pub fn execute_cleanup(&self, _ctx: &mut ExecuteContext) -> Result<(), String> {
        // Cleanup: nothing needed in interpreter mode
        Ok(())
    }

    /// Find the named layer within a group and return its index.
    /// Matching C++ `ShadingSystem::find_layer`.
    pub fn find_layer(&self, group: &ShaderGroupRef, layer_name: &str) -> Option<usize> {
        group.lock().unwrap().find_layer(layer_name)
    }

    /// Get a raw symbol value after execution.
    /// Matching C++ `ShadingSystem::get_symbol`.
    pub fn get_symbol(
        &self,
        ctx: &ExecuteContext,
        layer_name: &str,
        symbol_name: &str,
    ) -> Option<crate::interp::Value> {
        // Find layer
        let layer_idx = ctx.layer_irs.iter().enumerate().find_map(|(i, ir)| {
            if ir.shader_name == layer_name {
                Some(i)
            } else {
                None
            }
        });

        if let Some(idx) = layer_idx {
            if let Some(Some(interp)) = ctx.layer_interps.get(idx) {
                return interp.get_symbol_value(&ctx.layer_irs[idx], symbol_name);
            }
        }

        // Search all layers in reverse
        for i in (0..ctx.layer_interps.len()).rev() {
            if let Some(Some(interp)) = ctx.layer_interps.get(i) {
                if let Some(val) = interp.get_symbol_value(&ctx.layer_irs[i], symbol_name) {
                    return Some(val);
                }
            }
        }
        None
    }

    /// Helper function — copy or convert a source value to destination type.
    /// Matching C++ `ShadingSystem::convert_value`.
    pub fn convert_value(
        src: &crate::interp::Value,
        dst_type: &crate::typedesc::TypeDesc,
    ) -> Option<crate::interp::Value> {
        use crate::interp::Value;
        use crate::typedesc::{Aggregate, BaseType};

        let is_float_scalar = dst_type.basetype == BaseType::Float as u8
            && dst_type.aggregate == Aggregate::Scalar as u8;
        let is_int_scalar = dst_type.basetype == BaseType::Int32 as u8
            && dst_type.aggregate == Aggregate::Scalar as u8;
        let is_triple = dst_type.basetype == BaseType::Float as u8
            && dst_type.aggregate == Aggregate::Vec3 as u8;

        let is_string = dst_type.basetype == BaseType::String as u8;
        let is_matrix = dst_type.aggregate == Aggregate::Matrix44 as u8;
        let is_float_arr2 = dst_type.basetype == BaseType::Float as u8
            && dst_type.aggregate == Aggregate::Scalar as u8
            && dst_type.arraylen == 2;
        let is_float_arr4 = dst_type.basetype == BaseType::Float as u8
            && dst_type.aggregate == Aggregate::Scalar as u8
            && dst_type.arraylen == 4;
        let is_float_arr3 = dst_type.basetype == BaseType::Float as u8
            && dst_type.aggregate == Aggregate::Scalar as u8
            && dst_type.arraylen == 3;

        match src {
            Value::Int(i) => {
                if is_int_scalar {
                    Some(src.clone())
                } else if is_float_scalar {
                    Some(Value::Float(*i as f32))
                } else if is_triple {
                    let f = *i as f32;
                    Some(Value::Vec3(crate::math::Vec3::new(f, f, f)))
                } else {
                    None
                }
            }
            Value::Float(f) => {
                if is_float_scalar {
                    Some(src.clone())
                } else if is_int_scalar {
                    Some(Value::Int(*f as i32))
                } else if is_triple {
                    Some(Value::Vec3(crate::math::Vec3::new(*f, *f, *f)))
                } else if is_float_arr2 {
                    // C++ float → float[2]: fill both with same value
                    Some(Value::FloatArray(vec![*f, *f]))
                } else if is_float_arr4 {
                    // C++ float → float[4]
                    Some(Value::FloatArray(vec![*f, *f, *f, *f]))
                } else {
                    None
                }
            }
            Value::Vec3(v) | Value::Color(v) => {
                if is_triple {
                    Some(src.clone())
                } else if is_float_scalar {
                    Some(Value::Float(v.x))
                } else if is_float_arr3 {
                    // triple → float[3]
                    Some(Value::FloatArray(vec![v.x, v.y, v.z]))
                } else {
                    None
                }
            }
            Value::FloatArray(a) if a.len() == 3 && is_triple => {
                // float[3] → triple
                Some(Value::Vec3(crate::math::Vec3::new(a[0], a[1], a[2])))
            }
            Value::FloatArray(a) if a.len() == 2 && is_triple => {
                // float[2] → triple (x,y,0)
                Some(Value::Vec3(crate::math::Vec3::new(a[0], a[1], 0.0)))
            }
            Value::String(_) if is_string => Some(src.clone()),
            Value::Matrix(_) if is_matrix => Some(src.clone()),
            _ => None,
        }
    }

    /// Optimize all shader groups.
    /// Matching C++ `ShadingSystem::optimize_all_groups`.
    pub fn optimize_all_groups(&self, groups: &[ShaderGroupRef]) {
        for group in groups {
            self.optimize_group(group, None);
        }
    }

    /// Archive the entire shader group to a .tar, .tar.gz, .tgz, or .zip file.
    /// Matching C++ `ShadingSystem::archive_shadergroup`.
    /// Creates temp dir, writes shadergroup + OSO files, runs tar/zip, cleans up.
    pub fn archive_shadergroup(
        &self,
        group: &ShaderGroupRef,
        filename: &str,
    ) -> Result<(), String> {
        use std::fs;
        use std::process::Command;

        let grp = group.lock().unwrap();
        let ext = if let Some(dot) = filename.rfind('.') {
            &filename[dot..]
        } else {
            return Err("archive_shadergroup: invalid filename (no extension)".to_string());
        };
        if ext.len() < 2 {
            return Err("archive_shadergroup: invalid filename".to_string());
        }
        let supported = matches!(ext, ".tar" | ".tar.gz" | ".tgz" | ".zip");
        if !supported {
            return Err(format!(
                "archive_shadergroup: unsupported extension \"{}\" (use .tar, .tar.gz, .tgz, or .zip)",
                ext
            ));
        }

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_dir =
            std::env::temp_dir().join(format!("OSL-{:016x}-{:04x}", unique, std::process::id()));
        fs::create_dir_all(&temp_dir).map_err(|e| {
            format!(
                "archive_shadergroup: could not create temp directory: {}",
                e
            )
        })?;

        let cleanup = |e: &str| {
            let _ = fs::remove_dir_all(&temp_dir);
            e.to_string()
        };

        // Write shadergroup
        let group_path = temp_dir.join("shadergroup");
        fs::write(&group_path, grp.serialize())
            .map_err(|e| cleanup(&format!("could not write shadergroup: {}", e)))?;

        // Write each unique OSO
        let mut seen = std::collections::HashSet::new();
        for layer in &grp.layers {
            let name = layer.master.name.as_str();
            let osoname = if name.ends_with(".oso") {
                name.to_string()
            } else {
                format!("{}.oso", name)
            };
            if seen.insert(osoname.clone()) {
                let oso_str = crate::oso::write_oso_string(&layer.master.oso)
                    .map_err(|e| cleanup(&format!("OSO write error: {}", e)))?;
                let oso_path = temp_dir.join(&osoname);
                fs::write(&oso_path, oso_str)
                    .map_err(|e| cleanup(&format!("could not write OSO {}: {}", osoname, e)))?;
            }
        }

        let tmpdir_str = temp_dir.to_str().unwrap();
        let mut tar_files = vec!["shadergroup".to_string()];
        tar_files.extend(seen.iter().cloned());

        // Resolve output path (absolute for zip when cwd is temp_dir)
        let out_path = if std::path::Path::new(filename).is_absolute() {
            std::path::PathBuf::from(filename)
        } else {
            std::env::current_dir().unwrap_or_default().join(filename)
        };
        let out_str = out_path.to_str().unwrap_or(filename);

        let ok = if ext == ".tar" {
            let mut cmd = Command::new("tar");
            cmd.arg("-c")
                .arg("-C")
                .arg(tmpdir_str)
                .arg("-f")
                .arg(out_str);
            cmd.args(&tar_files);
            cmd.status().map(|s| s.success()).unwrap_or(false)
        } else if ext == ".tar.gz" || ext == ".tgz" {
            let mut cmd = Command::new("tar");
            cmd.arg("-cz")
                .arg("-C")
                .arg(tmpdir_str)
                .arg("-f")
                .arg(out_str);
            cmd.args(&tar_files);
            cmd.status().map(|s| s.success()).unwrap_or(false)
        } else if ext == ".zip" {
            let mut cmd = Command::new("zip");
            cmd.arg("-q").arg(out_str).current_dir(&temp_dir);
            cmd.arg("shadergroup");
            cmd.args(&tar_files[1..]);
            cmd.status().map(|s| s.success()).unwrap_or(false)
        } else {
            false
        };

        let _ = fs::remove_dir_all(&temp_dir);

        if ok {
            Ok(())
        } else {
            Err("archive_shadergroup: tar/zip command failed".to_string())
        }
    }

    /// JIT-compile all layers in a shader group and execute them sequentially.
    ///
    /// This is the high-performance path: each layer is compiled to native code
    /// via Cranelift, and connections between layers are resolved by copying
    /// output values from upstream layers into downstream layers' parameter slots.
    ///
    /// Returns the result of the last (entry-point) layer.
    /// Execute a shader group using the Cranelift JIT backend.
    ///
    /// All layers are compiled into a **single native function** with a unified
    /// heap. Connections between layers are resolved at compile time as direct
    /// memory copies, avoiding per-execution overhead.
    #[cfg(feature = "jit")]
    pub fn execute_jit(
        &self,
        group: &ShaderGroupRef,
        sg: &mut crate::shaderglobals::ShaderGlobals,
    ) -> Result<(), String> {
        let grp = group.lock().unwrap();
        if grp.layers.is_empty() {
            return Err("Shader group has no layers".into());
        }
        let group_name = grp.name;

        // Check the JIT cache first — avoid recompilation if already cached.
        // This matches the C++ OSL pattern where compiled code is stored per
        // ShaderGroup and only rebuilt when parameters/connections change.
        {
            let cache = self.jit_cache.lock().unwrap();
            if let Some(compiled) = cache.get(&group_name) {
                let compiled = Arc::clone(compiled);
                drop(cache);
                drop(grp);
                let mut heap = vec![0u8; compiled.heap_size()];
                let syn = self.commonspace_synonym();
                compiled.execute_with_renderer(
                    sg,
                    &mut heap,
                    self.renderer.as_ref(),
                    Some(syn.as_str()),
                    self.unknown_coordsys_error(),
                );
                return Ok(());
            }
        }

        let backend = crate::jit::CraneliftBackend::new();

        // Build IR for each layer (applying parameter overrides)
        let mut irs: Vec<crate::codegen::ShaderIR> = Vec::new();
        for instance in &grp.layers {
            let mut ir = oso_to_ir(&instance.master.oso);
            for (name, value) in &instance.param_overrides {
                let hints = instance
                    .param_hints
                    .get(name)
                    .copied()
                    .unwrap_or(ParamHints::NONE);
                apply_param_override(&mut ir, name, value, hints);
            }
            irs.push(ir);
        }
        let connections = grp.connections.clone();
        drop(grp); // Release the lock before compilation

        // Compile all layers into a single native function with unified heap
        let ir_refs: Vec<&crate::codegen::ShaderIR> = irs.iter().collect();
        let compiled = backend
            .compile_group(&ir_refs, &connections, self.range_checking())
            .map_err(|e| format!("{e:?}"))?;

        let compiled = Arc::new(compiled);

        // Store in cache for future calls
        {
            let mut cache = self.jit_cache.lock().unwrap();
            cache.insert(group_name, Arc::clone(&compiled));
        }

        // Execute with a single heap allocation and one function call
        let mut heap = vec![0u8; compiled.heap_size()];
        let syn = self.commonspace_synonym();
        compiled.execute_with_renderer(
            sg,
            &mut heap,
            self.renderer.as_ref(),
            Some(syn.as_str()),
            self.unknown_coordsys_error(),
        );

        Ok(())
    }

    /// Invalidate the JIT cache for a specific shader group.
    /// Call this after modifying group parameters or connections.
    #[cfg(feature = "jit")]
    pub fn invalidate_jit_cache(&self, group_name: &str) {
        let key = UString::new(group_name);
        let mut cache = self.jit_cache.lock().unwrap();
        cache.remove(&key);
    }

    /// Clear the entire JIT compilation cache.
    #[cfg(feature = "jit")]
    pub fn clear_jit_cache(&self) {
        let mut cache = self.jit_cache.lock().unwrap();
        cache.clear();
    }

    /// Execute a single shader from source string (convenience for testing).
    pub fn execute_source(
        &self,
        source: &str,
        globals: &crate::shaderglobals::ShaderGlobals,
    ) -> Result<ExecuteResult, String> {
        let ast = crate::parser::parse(source)
            .map_err(|e| format!("{e:?}"))?
            .ast;
        let ir = crate::codegen::generate(&ast);
        let mut interp = crate::interp::Interpreter::with_renderer(self.renderer.clone());
        interp.set_commonspace_synonym(self.commonspace_synonym());
        interp.set_range_checking(self.range_checking());
        interp.set_unknown_coordsys_error(self.unknown_coordsys_error());
        interp.execute(&ir, globals, None);
        Ok(ExecuteResult { ir, interp })
    }
}

/// Statistics about the shading system.
#[derive(Debug, Clone, Default)]
pub struct ShadingSystemStats {
    pub masters_loaded: u32,
    pub closures_registered: u32,
    pub shaders_requested: u64,
    pub shaders_loaded: u64,
    pub groups_created: u64,
    pub groups_compiled: u64,
    pub instances_compiled: u64,
    pub merged_inst: u64,
    pub merged_inst_opt: u64,
    pub empty_instances: u64,
    pub empty_groups: u64,
    pub layers_executed: u64,
    pub regexes_compiled: u64,
    pub connections_total: u64,
    pub memory_current: u64,
    pub memory_peak: u64,
    pub optimization_time_ticks: u64,
    pub preopt_syms: u64,
    pub postopt_syms: u64,
    pub preopt_ops: u64,
    pub postopt_ops: u64,
    pub tex_calls_codegened: u64,
    pub tex_calls_as_handles: u64,
    pub getattribute_calls: u64,
    pub get_userdata_calls: u64,
    pub noise_calls: u64,
    pub pointcloud_searches: u64,
    pub total_shading_time_ticks: u64,
}

// ---------------------------------------------------------------------------
// ParamHints — matching C++ `ParamHints` enum
// ---------------------------------------------------------------------------

/// Parameter property hint bitflag values (matching C++ `ParamHints`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParamHints(pub u32);

impl ParamHints {
    pub const NONE: Self = Self(0);
    /// Parameter may be an interpolated "user data" / geometric primitive var.
    pub const INTERPOLATED: Self = Self(1);
    /// Parameter may have its value interactively modified by ReParameter.
    pub const INTERACTIVE: Self = Self(2);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for ParamHints {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for ParamHints {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ---------------------------------------------------------------------------
// SymLocationDesc — matching C++ `SymLocationDesc`
// ---------------------------------------------------------------------------

/// Describes where a symbol is located in the renderer's memory.
/// Matching C++ `SymLocationDesc`.
#[derive(Debug, Clone)]
pub struct SymLocationDesc {
    pub name: UString,
    pub typedesc: crate::typedesc::TypeDesc,
    pub offset: i64,
    pub stride: i64,
    pub arena: crate::symbol::SymArena,
    pub derivs: bool,
}

impl SymLocationDesc {
    pub const AUTO_STRIDE: i64 = i64::MIN;

    pub fn new(
        name: &str,
        typedesc: crate::typedesc::TypeDesc,
        derivs: bool,
        arena: crate::symbol::SymArena,
        offset: i64,
        stride: i64,
    ) -> Self {
        let actual_stride = if stride == Self::AUTO_STRIDE {
            typedesc.size() as i64
        } else {
            stride
        };
        Self {
            name: UString::new(name),
            typedesc,
            offset,
            stride: actual_stride,
            arena,
            derivs,
        }
    }
}

/// Location mode for `shade_image`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadeImageLocations {
    /// Shade at pixel centers (u,v = (x+0.5)/w, (y+0.5)/h).
    PixelCenters,
    /// Shade at pixel corners (u,v = x/(w-1), y/(h-1)).
    PixelCorners,
}

/// Shade an image buffer pixel-by-pixel using a shader group.
/// Matches `shade_image` from `shadeimage.cpp`.
///
/// `width` and `height` define the image dimensions.
/// `shade_fn` is called for each pixel with (x, y, u, v, ShaderGlobals).
/// The function returns the float output buffer (width × height × channels).
pub fn shade_image(
    shading_system: &ShadingSystem,
    group: &ShaderGroupRef,
    width: usize,
    height: usize,
    output_name: &str,
    locations: ShadeImageLocations,
) -> Result<Vec<f32>, String> {
    let mut result_buf = Vec::with_capacity(width * height * 3);

    for y in 0..height {
        for x in 0..width {
            let (u, v) = match locations {
                ShadeImageLocations::PixelCenters => (
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                ),
                ShadeImageLocations::PixelCorners => (
                    if width <= 1 {
                        0.5
                    } else {
                        x as f32 / (width - 1) as f32
                    },
                    if height <= 1 {
                        0.5
                    } else {
                        y as f32 / (height - 1) as f32
                    },
                ),
            };

            let mut sg = crate::shaderglobals::ShaderGlobals::default();
            sg.p = crate::math::Vec3::new(x as f32, y as f32, 0.0);
            sg.u = u;
            sg.v = v;
            sg.dudx = 1.0 / width.max(1) as f32;
            sg.dvdy = 1.0 / height.max(1) as f32;
            sg.dp_dx = crate::math::Vec3::new(1.0, 0.0, 0.0);
            sg.dp_dy = crate::math::Vec3::new(0.0, 1.0, 0.0);
            sg.dp_dz = crate::math::Vec3::new(0.0, 0.0, 1.0);
            sg.n = crate::math::Vec3::new(0.0, 0.0, 1.0);
            sg.ng = crate::math::Vec3::new(0.0, 0.0, 1.0);
            sg.surfacearea = 1.0;

            let exec_result = shading_system.execute(group, &sg)?;
            if let Some(c) = exec_result.get_vec3(output_name) {
                result_buf.push(c.x);
                result_buf.push(c.y);
                result_buf.push(c.z);
            } else if let Some(f) = exec_result.get_float(output_name) {
                result_buf.push(f);
                result_buf.push(f);
                result_buf.push(f);
            } else {
                result_buf.push(0.0);
                result_buf.push(0.0);
                result_buf.push(0.0);
            }
        }
    }

    Ok(result_buf)
}

/// Context for staged execution (execute_init / execute_layer / execute_cleanup).
pub struct ExecuteContext {
    pub layer_irs: Vec<crate::codegen::ShaderIR>,
    pub layer_interps: Vec<Option<crate::interp::Interpreter>>,
    pub connections: Vec<Connection>,
    pub globals: crate::shaderglobals::ShaderGlobals,
    pub executed: Vec<bool>,
    /// Shared message store across layers (setmessage/getmessage).
    pub shared_messages: crate::message::MessageStore,
}

/// Result of executing a shader group.
pub struct ExecuteResult {
    pub ir: crate::codegen::ShaderIR,
    pub interp: crate::interp::Interpreter,
}

impl ExecuteResult {
    /// Get a float value by symbol name.
    pub fn get_float(&self, name: &str) -> Option<f32> {
        self.interp.get_float(&self.ir, name)
    }

    /// Get an int value by symbol name.
    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.interp.get_int(&self.ir, name)
    }

    /// Get a vec3 value by symbol name.
    pub fn get_vec3(&self, name: &str) -> Option<Vec3> {
        self.interp.get_vec3(&self.ir, name)
    }
}

/// Convert an OsoFile into a ShaderIR for interpretation.
pub fn oso_to_ir(oso: &crate::oso::OsoFile) -> crate::codegen::ShaderIR {
    use crate::codegen::{ConstValue, ShaderIR};
    use crate::symbol::{Opcode, SymType, Symbol};
    use crate::ustring::UString;

    let mut ir = ShaderIR::new();
    ir.shader_type = oso.shader_type;
    ir.shader_name = oso.shader_name.clone();

    // Create a name → index mapping for resolving args
    let mut name_to_idx: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    // First pass: create all symbols
    for oso_sym in &oso.symbols {
        let idx = ir.symbols.len();
        name_to_idx.insert(oso_sym.name.clone(), idx);

        let mut sym = Symbol::new(
            UString::new(&oso_sym.name),
            oso_sym.typespec,
            oso_sym.symtype,
        );
        sym.initializers = if !oso_sym.idefault.is_empty()
            || !oso_sym.fdefault.is_empty()
            || !oso_sym.sdefault.is_empty()
        {
            1
        } else {
            0
        };
        // Apply %read and %write hints (C++ loadshader parity)
        if let Some((first, last)) = oso_sym.read_range {
            sym.firstread = first;
            sym.lastread = last;
        }
        if let Some((first, last)) = oso_sym.write_range {
            sym.firstwrite = first;
            sym.lastwrite = last;
        }
        ir.symbols.push(sym);

        // Store constant/default values
        if oso_sym.symtype == SymType::Const {
            if let Some(&v) = oso_sym.idefault.first() {
                ir.const_values.push((idx, ConstValue::Int(v)));
            } else if let Some(&v) = oso_sym.fdefault.first() {
                ir.const_values.push((idx, ConstValue::Float(v)));
            } else if let Some(v) = oso_sym.sdefault.first() {
                ir.const_values
                    .push((idx, ConstValue::String(UString::new(v))));
            }
        } else if oso_sym.symtype == SymType::Param || oso_sym.symtype == SymType::OutputParam {
            if let Some(&v) = oso_sym.idefault.first() {
                ir.param_defaults.push((idx, ConstValue::Int(v)));
            } else if let Some(&v) = oso_sym.fdefault.first() {
                ir.param_defaults.push((idx, ConstValue::Float(v)));
            } else if let Some(v) = oso_sym.sdefault.first() {
                ir.param_defaults
                    .push((idx, ConstValue::String(UString::new(v))));
            }
        }
    }

    // Second pass: create opcodes
    for inst in &oso.instructions {
        if inst.opcode == "end" {
            break;
        }

        let firstarg = ir.args.len() as i32;
        let mut nargs = 0i32;

        for arg_name in &inst.args {
            if let Some(&idx) = name_to_idx.get(arg_name) {
                ir.args.push(idx as i32);
                nargs += 1;
            } else {
                // Try parsing as integer (some OSO formats use indices directly)
                if let Ok(idx) = arg_name.parse::<i32>() {
                    ir.args.push(idx);
                    nargs += 1;
                }
            }
        }

        let mut op = Opcode::new(
            UString::new(&inst.opcode),
            UString::empty(),
            firstarg,
            nargs,
        );

        // Set jump targets
        for (i, &j) in inst.jumps.iter().enumerate() {
            if i < 4 {
                op.jump[i] = j;
            }
        }

        if let Some(line) = inst.sourceline {
            op.sourceline = line;
        }

        // Apply %argrw hint (C++ loadshader parity)
        if let Some(ref rw) = inst.argrw {
            let nargs = nargs as usize;
            let chars: Vec<char> = rw.chars().collect();
            for (j, &c) in chars.iter().take(nargs).enumerate() {
                let read = c == 'r' || c == 'W';
                let write = c == 'w' || c == 'W';
                op.set_arg_read(j as u32, read);
                op.set_arg_written(j as u32, write);
            }
        }

        // Apply %argderivs hint
        if let Some(ref derivs) = inst.argderivs {
            for &arg in derivs {
                if arg >= 0 {
                    op.set_arg_takes_derivs(arg as u32, true);
                }
            }
        }

        // Fix old oslc bug: getmatrix last arg is write-only (C++ loadshader)
        if inst.opcode == "getmatrix" && nargs > 0 {
            op.arg_writeonly((nargs - 1) as u32);
        }
        // Fix old oslc bug: regex_search/regex_match arg 2 is write-only
        if (inst.opcode == "regex_search" || inst.opcode == "regex_match") && nargs > 3 {
            op.arg_writeonly(2);
        }

        ir.opcodes.push(op);
    }

    ir
}

/// Apply a parameter override to a ShaderIR. Optionally apply ParamHints to Symbol.
fn apply_param_override(
    ir: &mut crate::codegen::ShaderIR,
    name: &UString,
    value: &ParamValue,
    hints: ParamHints,
) {
    use crate::codegen::ConstValue;
    use crate::symbol::SymType;

    for (idx, sym) in ir.symbols.iter().enumerate() {
        if (sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam)
            && sym.name == *name
        {
            // Remove any existing default for this param
            ir.param_defaults.retain(|&(i, _)| i != idx);

            // Add the override
            match value {
                ParamValue::Int(v) => ir.param_defaults.push((idx, ConstValue::Int(*v))),
                ParamValue::Float(v) => ir.param_defaults.push((idx, ConstValue::Float(*v))),
                ParamValue::String(s) => ir.param_defaults.push((idx, ConstValue::String(*s))),
                ParamValue::Color(c)
                | ParamValue::Point(c)
                | ParamValue::Vector(c)
                | ParamValue::Normal(c) => ir.param_defaults.push((idx, ConstValue::Vec3(*c))),
                ParamValue::Matrix(m) => ir.param_defaults.push((idx, ConstValue::Matrix(*m))),
                ParamValue::IntArray(a) => ir
                    .param_defaults
                    .push((idx, ConstValue::IntArray(a.clone()))),
                ParamValue::FloatArray(a) => ir
                    .param_defaults
                    .push((idx, ConstValue::FloatArray(a.clone()))),
                ParamValue::StringArray(a) => ir
                    .param_defaults
                    .push((idx, ConstValue::StringArray(a.clone()))),
            }
            // Apply hints to Symbol (matching C++ SymOverrideInfo)
            if hints.0 != 0 {
                if hints.contains(ParamHints::INTERPOLATED) {
                    ir.symbols[idx].interpolated = true;
                }
                if hints.contains(ParamHints::INTERACTIVE) {
                    ir.symbols[idx].interactive = true;
                }
            }
            break;
        }
    }
}

/// Apply a connected value from an upstream layer.
fn apply_connection_value(
    ir: &mut crate::codegen::ShaderIR,
    param_name: &str,
    val: &crate::interp::Value,
) {
    use crate::codegen::ConstValue;
    use crate::symbol::SymType;

    let uname = UString::new(param_name);
    for (idx, sym) in ir.symbols.iter().enumerate() {
        if (sym.symtype == SymType::Param || sym.symtype == SymType::OutputParam)
            && sym.name == uname
        {
            ir.param_defaults.retain(|&(i, _)| i != idx);
            match val {
                crate::interp::Value::Int(v) => ir.param_defaults.push((idx, ConstValue::Int(*v))),
                crate::interp::Value::Float(v) => {
                    ir.param_defaults.push((idx, ConstValue::Float(*v)))
                }
                crate::interp::Value::String(s) => {
                    ir.param_defaults.push((idx, ConstValue::String(*s)))
                }
                _ => {}
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::NullRenderer;

    fn make_system() -> ShadingSystem {
        let renderer = Arc::new(NullRenderer);
        ShadingSystem::new(renderer, None)
    }

    #[test]
    fn test_create() {
        let ss = make_system();
        ss.attribute(
            "searchpath:shader",
            AttributeValue::String("/tmp/shaders".into()),
        );
        assert!(matches!(
            ss.getattribute("searchpath:shader"),
            Some(AttributeValue::String(s)) if s == "/tmp/shaders"
        ));
    }

    #[test]
    fn test_raytype() {
        let ss = make_system();
        let camera = ss.raytype_bit("camera");
        let shadow = ss.raytype_bit("shadow");
        let camera2 = ss.raytype_bit("camera");
        assert_eq!(camera, camera2);
        assert_ne!(camera, shadow);
        assert_eq!(camera & shadow, 0);
        // Unknown raytype returns 0 (C++ parity)
        assert_eq!(ss.raytype_bit("unknown_ray"), 0);
        // Default order: camera=1, shadow=2, reflection=4, refraction=8, ...
        assert_eq!(ss.raytype_bit("reflection"), 4);
        // Override via attribute
        ss.attribute(
            "raytypes",
            AttributeValue::StringArray(vec!["primary".into(), "bounce".into()]),
        );
        assert_eq!(ss.raytype_bit("primary"), 1);
        assert_eq!(ss.raytype_bit("bounce"), 2);
        assert_eq!(ss.raytype_bit("camera"), 0);
    }

    #[test]
    fn test_register_closure() {
        let ss = make_system();
        ss.register_closure("diffuse", 1, vec![]);
        let result = ss.query_closure("diffuse");
        assert!(result.is_some());
        let (id, _params) = result.unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn test_group_construction() {
        let ss = make_system();
        let group = ss.shader_group_begin("test_group");
        // Empty group should fail validation
        assert!(ss.shader_group_end(&group).is_err());
        let grp = group.lock().unwrap();
        assert_eq!(grp.nlayers(), 0);
    }

    #[test]
    fn test_load_memory_shader() {
        let ss = make_system();
        let oso = "\
OpenShadingLanguage 1.0
surface test_shader
symtype param float Kd 1
code main
    end
";
        let result = ss.load_memory_shader("test_shader", oso);
        assert!(result.is_ok());
        let master = result.unwrap();
        assert_eq!(master.shader_type, ShaderType::Surface);
    }

    #[test]
    fn test_merge_instances_identical() {
        let master = Arc::new(ShaderMaster {
            name: UString::new("noise_tex"),
            shader_type: ShaderType::Surface,
            oso: crate::oso::OsoFile::default(),
        });

        let inst_a = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_A"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };
        let inst_b = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_B"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };
        let inst_c = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_C"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };

        let mut group = ShaderGroup::new("test");
        group.layers.push(inst_a);
        group.layers.push(inst_b);
        group.layers.push(inst_c); // entry layer, not merged

        // A and B are identical and share same master with same params.
        // B should be merged into A. C is last layer (entry), untouched.
        let merges = group.merge_instances();
        assert_eq!(merges, 1);
        // B should now be marked as merged
        assert!(group.layers[1].unused);
    }

    #[test]
    fn test_merge_instances_different_params() {
        let master = Arc::new(ShaderMaster {
            name: UString::new("noise_tex"),
            shader_type: ShaderType::Surface,
            oso: crate::oso::OsoFile::default(),
        });

        let mut overrides = HashMap::new();
        overrides.insert(UString::new("scale"), ParamValue::Float(2.0));

        let inst_a = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_A"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };
        let inst_b = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_B"),
            param_overrides: overrides,
            param_hints: HashMap::new(),
            unused: false,
        };
        let inst_c = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("layer_C"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };

        let mut group = ShaderGroup::new("test");
        group.layers.push(inst_a);
        group.layers.push(inst_b);
        group.layers.push(inst_c);

        // A and B differ in param overrides, should NOT merge
        let merges = group.merge_instances();
        assert_eq!(merges, 0);
    }

    #[test]
    fn test_merge_instances_rewires_connections() {
        let master = Arc::new(ShaderMaster {
            name: UString::new("noise_tex"),
            shader_type: ShaderType::Surface,
            oso: crate::oso::OsoFile::default(),
        });

        let mut group = ShaderGroup::new("test");
        group.layers.push(ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("A"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        });
        group.layers.push(ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("B"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        });
        group.layers.push(ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("entry"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        });

        // Entry layer (2) reads from B (1)
        group.connections.push(Connection {
            src_layer: 1,
            src_param: UString::new("Cout"),
            dst_layer: 2,
            dst_param: UString::new("Cin"),
        });

        let merges = group.merge_instances();
        assert_eq!(merges, 1);

        // Connection should now point to A (0) instead of B (1)
        assert_eq!(group.connections[0].src_layer, 0);
    }

    #[test]
    fn test_named_attribute_defaults() {
        let ss = make_system();
        // Verify new well-known attribute defaults
        assert!(matches!(
            ss.getattribute("colorspace"),
            Some(AttributeValue::String(s)) if s == "Rec709"
        ));
        assert!(matches!(
            ss.getattribute("range_checking"),
            Some(AttributeValue::Int(1))
        ));
        assert!(matches!(
            ss.getattribute("greedyjit"),
            Some(AttributeValue::Int(0))
        ));
        assert!(matches!(
            ss.getattribute("max_warnings_per_thread"),
            Some(AttributeValue::Int(100))
        ));
        assert!(matches!(
            ss.getattribute("error_repeats"),
            Some(AttributeValue::Int(0))
        ));
        assert!(matches!(
            ss.getattribute("lazylayers"),
            Some(AttributeValue::Int(1))
        ));
        assert!(matches!(
            ss.getattribute("lazyerror"),
            Some(AttributeValue::Int(1))
        ));
        assert!(matches!(
            ss.getattribute("relaxed_param_typecheck"),
            Some(AttributeValue::Int(0))
        ));
        assert!(matches!(
            ss.getattribute("searchpath:texture"),
            Some(AttributeValue::String(s)) if s.is_empty()
        ));
    }

    #[test]
    fn test_attribute_override() {
        let ss = make_system();
        // Override a default
        ss.attribute("colorspace", AttributeValue::String("ACEScg".into()));
        assert!(matches!(
            ss.getattribute("colorspace"),
            Some(AttributeValue::String(s)) if s == "ACEScg"
        ));
        // Override int
        ss.attribute("greedyjit", AttributeValue::Int(1));
        assert!(matches!(
            ss.getattribute("greedyjit"),
            Some(AttributeValue::Int(1))
        ));
    }

    #[test]
    fn test_commonspace_attribute() {
        // C++ API uses "commonspace"; "commonspace_synonym" supported for compat
        let ss = make_system();
        assert_eq!(ss.commonspace_synonym().as_str(), "world");
        assert!(matches!(
            ss.getattribute("commonspace"),
            Some(AttributeValue::String(s)) if s == "world"
        ));
        ss.attribute("commonspace", AttributeValue::String("camera".into()));
        assert_eq!(ss.commonspace_synonym().as_str(), "camera");
        assert!(matches!(
            ss.getattribute("commonspace"),
            Some(AttributeValue::String(s)) if s == "camera"
        ));
        assert!(matches!(
            ss.getattribute("commonspace_synonym"),
            Some(AttributeValue::String(s)) if s == "camera"
        ));
    }

    #[test]
    fn test_stat_counters() {
        let stats = ShadingStats::new();
        stats.layers_executed.fetch_add(42, Ordering::Relaxed);
        stats.memory_current.fetch_add(1024, Ordering::Relaxed);
        stats.memory_peak.fetch_add(2048, Ordering::Relaxed);
        assert_eq!(stats.layers_executed.load(Ordering::Relaxed), 42);
        assert_eq!(stats.memory_current.load(Ordering::Relaxed), 1024);
        assert_eq!(stats.memory_peak.load(Ordering::Relaxed), 2048);
    }

    #[test]
    fn test_symlocs_per_group() {
        use crate::symbol::SymArena;
        use crate::typedesc::TypeDesc;

        let ss = make_system();
        // Global symlocs
        let global = SymLocationDesc::new(
            "global_var",
            TypeDesc::FLOAT,
            false,
            SymArena::Outputs,
            0,
            4,
        );
        ss.add_symlocs(&[global]);
        assert!(ss.find_symloc("global_var").is_some());

        // New group inherits global symlocs at creation
        let group = ss.shader_group_begin("grp");
        {
            let grp = group.lock().unwrap();
            assert!(
                grp.find_symloc("global_var").is_some(),
                "group inherits global symlocs"
            );
        }

        // Per-group symlocs
        let group_symloc = SymLocationDesc::new(
            "group_out",
            TypeDesc::VECTOR,
            true,
            SymArena::Outputs,
            16,
            12,
        );
        ss.add_symlocs_group(Some(&group), &[group_symloc]);
        assert!(ss.find_symloc_group(Some(&group), "group_out").is_some());
        assert!(ss.find_symloc_group(Some(&group), "global_var").is_some());

        // Global find_symloc doesn't see group-only symloc
        assert!(ss.find_symloc("group_out").is_none());

        ss.clear_symlocs_group(Some(&group));
        assert!(ss.find_symloc_group(Some(&group), "group_out").is_none());
        assert!(ss.find_symloc("global_var").is_some());

        // find_symloc_arena: filters by arena and offset != -1
        let ss2 = make_system();
        let with_arena =
            SymLocationDesc::new("arena_var", TypeDesc::FLOAT, false, SymArena::Outputs, 8, 4);
        ss2.add_symlocs(&[with_arena]);
        assert!(
            ss2.find_symloc_arena("arena_var", SymArena::Outputs)
                .is_some()
        );
        assert!(
            ss2.find_symloc_arena("arena_var", SymArena::UserData)
                .is_none()
        );

        // find_symloc_layer: tries "layer.name" then name
        let mut grp = ShaderGroup::new("layer_test");
        let layer_sym =
            SymLocationDesc::new("layer1.x", TypeDesc::FLOAT, false, SymArena::Outputs, 0, 4);
        grp.add_symlocs(&[layer_sym]);
        assert!(
            grp.find_symloc_layer("x", "layer1", SymArena::Outputs)
                .is_some()
        );
    }

    #[test]
    fn test_shader_group_begin_from_serialized() {
        let ss = make_system();
        let oso = "\
OpenShadingLanguage 1.0
surface from_serialized_shader
symtype param float Kd 1
code main
    end
";
        ss.load_memory_shader("from_serialized_shader", oso)
            .unwrap();
        let spec = "param float Kd 0.75 ; shader from_serialized_shader layer1 ;";
        let group = ss.shader_group_begin_from_serialized("from_serialized_group", "surface", spec);
        let group = group.expect("shader_group_begin_from_serialized failed");
        assert!(ss.shader_group_end(&group).is_ok());
        let grp = group.lock().unwrap();
        assert_eq!(grp.nlayers(), 1);
        let layer = &grp.layers[0];
        assert_eq!(layer.layer_name.as_str(), "layer1");
        assert_eq!(layer.master.name.as_str(), "from_serialized_shader");
        let kd = layer.param_overrides.get(&UString::new("Kd"));
        assert!(kd.is_some());
        if let Some(ParamValue::Float(v)) = kd {
            assert!((*v - 0.75).abs() < 1e-6);
        }
    }

    #[test]
    fn test_shader_group_begin_from_serialized_connect() {
        let ss = make_system();
        let oso_a = "\
OpenShadingLanguage 1.0
surface shader_a
symtype output color Cout 0
code main
    end
";
        let oso_b = "\
OpenShadingLanguage 1.0
surface shader_b
symtype param color Cin 1
code main
    end
";
        ss.load_memory_shader("shader_a", oso_a).unwrap();
        ss.load_memory_shader("shader_b", oso_b).unwrap();
        let spec = "\
shader shader_a a_layer ;
shader shader_b b_layer ;
connect a_layer.Cout b_layer.Cin ;
";
        let group = ss
            .shader_group_begin_from_serialized("conn_group", "surface", spec)
            .expect("shader_group_begin_from_serialized failed");
        assert!(ss.shader_group_end(&group).is_ok());
        let grp = group.lock().unwrap();
        assert_eq!(grp.nlayers(), 2);
        assert_eq!(grp.connections.len(), 1);
        let c = &grp.connections[0];
        assert_eq!(
            grp.layers[c.src_layer as usize].layer_name.as_str(),
            "a_layer"
        );
        assert_eq!(c.src_param.as_str(), "Cout");
        assert_eq!(
            grp.layers[c.dst_layer as usize].layer_name.as_str(),
            "b_layer"
        );
        assert_eq!(c.dst_param.as_str(), "Cin");
    }

    #[test]
    fn test_shader_group_begin_from_serialized_hints() {
        let ss = make_system();
        let oso = "\
OpenShadingLanguage 1.0
surface hints_shader
symtype param float Kd 1
code main
    end
";
        ss.load_memory_shader("hints_shader", oso).unwrap();
        let spec = "param float Kd 0.5 [[int lockgeom=0]] ; shader hints_shader layer1 ;";
        let group = ss
            .shader_group_begin_from_serialized("hints_group", "surface", spec)
            .expect("from_serialized failed");
        assert!(ss.shader_group_end(&group).is_ok());
        let grp = group.lock().unwrap();
        let layer = &grp.layers[0];
        let hints = layer.param_hints.get(&UString::new("Kd"));
        assert!(hints.is_some(), "expected param_hints for Kd");
        assert!(hints.unwrap().contains(ParamHints::INTERPOLATED));
    }

    #[test]
    fn test_serialize_roundtrip() {
        let ss = make_system();
        let oso = "\
OpenShadingLanguage 1.0
surface roundtrip_shader
symtype param float x 1
symtype output color Cout 0
code main
    end
";
        ss.load_memory_shader("roundtrip_shader", oso).unwrap();
        let spec = "param float x 2.5 ; shader roundtrip_shader layer1 ;";
        let group = ss
            .shader_group_begin_from_serialized("roundtrip_group", "surface", spec)
            .expect("from_serialized failed");
        assert!(ss.shader_group_end(&group).is_ok());
        let ser = group.lock().unwrap().serialize();
        let group2 = ss
            .shader_group_begin_from_serialized("roundtrip_group2", "surface", &ser)
            .expect("roundtrip from_serialized failed");
        assert!(ss.shader_group_end(&group2).is_ok());
        let grp2 = group2.lock().unwrap();
        assert_eq!(grp2.nlayers(), 1);
        let x = grp2.layers[0].param_overrides.get(&UString::new("x"));
        assert!(x.is_some());
        if let Some(ParamValue::Float(v)) = x {
            assert!((*v - 2.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_group_serialize_and_pickle() {
        let mut overrides_a = HashMap::new();
        overrides_a.insert(UString::new("x"), ParamValue::Float(2.5));
        let master = Arc::new(ShaderMaster {
            name: UString::new("A"),
            shader_type: ShaderType::Surface,
            oso: crate::oso::OsoFile::default(),
        });
        let inst_a = ShaderInstance {
            master: master.clone(),
            layer_name: UString::new("a1"),
            param_overrides: HashMap::new(),
            param_hints: HashMap::new(),
            unused: false,
        };
        let inst_b = ShaderInstance {
            master,
            layer_name: UString::new("a2"),
            param_overrides: overrides_a,
            param_hints: HashMap::new(),
            unused: false,
        };
        let mut group = ShaderGroup::new("sg");
        group.layers.push(inst_a);
        group.layers.push(inst_b);
        group.connections.push(Connection {
            src_layer: 0,
            src_param: UString::new("Cout"),
            dst_layer: 1,
            dst_param: UString::new("x"),
        });

        let ser = group.serialize();
        assert!(ser.contains("shader A a1"));
        assert!(ser.contains("shader A a2"));
        assert!(ser.contains("param float x 2.500000000"));
        assert!(ser.contains("connect a1.Cout a2.x"));

        let group_ref = Arc::new(Mutex::new(group));
        let ss = make_system();
        let pickle = ss.group_getattribute(&group_ref, "pickle");
        if let Some(AttributeValue::String(s)) = pickle {
            assert_eq!(s, ser);
        }
    }

    #[test]
    fn test_globals_bit_and_name() {
        use crate::shaderglobals::SGBits;
        assert_eq!(ShadingSystem::globals_bit("P"), SGBits::P);
        assert_eq!(ShadingSystem::globals_bit("Ng"), SGBits::NG);
        assert_eq!(ShadingSystem::globals_bit("unknown"), SGBits::empty());
        assert_eq!(ShadingSystem::globals_name(SGBits::P), Some("P"));
        assert_eq!(ShadingSystem::globals_name(SGBits::NG), Some("Ng"));
        assert_eq!(ShadingSystem::globals_name(SGBits::empty()), None);
        assert_eq!(ShadingSystem::globals_name(SGBits::P | SGBits::N), None);
    }

    #[test]
    fn test_generate_report_empty() {
        let ss = make_system();
        let report = ss.generate_report();
        assert!(report.contains("No shaders requested or loaded"));
    }

    #[test]
    fn test_generate_report_with_shaders() {
        let ss = make_system();
        ss.stats.shaders_requested.fetch_add(5, Ordering::Relaxed);
        ss.stats.shaders_loaded.fetch_add(3, Ordering::Relaxed);
        ss.stats.groups_created.fetch_add(2, Ordering::Relaxed);
        ss.stats.regexes_compiled.fetch_add(1, Ordering::Relaxed);
        ss.stats.getattribute_calls.fetch_add(10, Ordering::Relaxed);
        let report = ss.generate_report();
        assert!(report.contains("Requested: 5"));
        assert!(report.contains("Loaded:    3"));
        assert!(report.contains("Shading groups:   2"));
        assert!(report.contains("Regex's compiled: 1"));
        assert!(report.contains("getattribute calls: 10"));
    }

    #[test]
    fn test_getstats_expanded() {
        let ss = make_system();
        ss.stats.groups_compiled.fetch_add(7, Ordering::Relaxed);
        ss.stats.instances_compiled.fetch_add(14, Ordering::Relaxed);
        ss.stats.merged_inst.fetch_add(3, Ordering::Relaxed);
        ss.stats
            .tex_calls_codegened
            .fetch_add(99, Ordering::Relaxed);
        let st = ss.getstats();
        assert_eq!(st.groups_compiled, 7);
        assert_eq!(st.instances_compiled, 14);
        assert_eq!(st.merged_inst, 3);
        assert_eq!(st.tex_calls_codegened, 99);
    }
}
