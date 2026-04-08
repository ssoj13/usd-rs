//! MaterialX conversion utilities — full implementation mirroring hdMtlx.cpp.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::OnceCell;

use mtlx_rs::core::{
    Document, ElementPtr, add_child_of_category, add_input, add_output, create_document,
    get_active_input, get_active_inputs, get_outputs, nodedef_get_node_string,
};
use mtlx_rs::format::util::load_libraries;

use usd_gf::{Matrix3d, Matrix4d, Vec2f, Vec3f, Vec4f};
use usd_sdf::Path as SdfPath;
use usd_sdr::SdrRegistry;
use usd_tf::Token;
use usd_vt::Value as VtValue;

use super::network_interface::{HdMaterialNetworkInterface, InputConnection};
use super::types::HdMtlxTexturePrimvarData;

// ---------------------------------------------------------------------------
// MaterialX version threshold for ND_normalmap rename (MX 1.39)
const MTLX_COMBINED_VERSION_139: u32 = 13900;

/// Combined version integer from (major, minor) — e.g. (1, 39) -> 13900.
fn combined_version(maj: i32, min: i32) -> u32 {
    (maj as u32) * 1000 + (min as u32) * 100
}

// ---------------------------------------------------------------------------
// Stdlib cache

/// Cached copy of the loaded MaterialX standard libraries document.
static STD_LIBRARIES: OnceCell<Mutex<Option<Document>>> = OnceCell::new();

/// Returns search paths in C++ order: PLUGIN paths first, then STDLIB paths.
///
/// Env vars read (path separator is `;` on Windows, `:` on Unix):
/// - `PXR_MTLX_PLUGIN_SEARCH_PATHS`  (first, plugins override stdlib)
/// - `PXR_MTLX_STDLIB_SEARCH_PATHS`  (second)
/// - `PXR_MTLX_STDLIB_SEARCH_PATHS` fallback from `DCC_LOCATION/libraries`
pub fn get_search_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    #[cfg(target_os = "windows")]
    const PATH_SEP: char = ';';
    #[cfg(not(target_os = "windows"))]
    const PATH_SEP: char = ':';

    let mut push = |s: String| {
        let s = s.trim().to_string();
        if !s.is_empty() && seen.insert(s.clone()) {
            paths.push(s);
        }
    };

    // 1. Plugin paths first (highest priority, mirrors C++ _ComputeSearchPaths).
    if let Ok(v) = std::env::var("PXR_MTLX_PLUGIN_SEARCH_PATHS") {
        for p in v.split(PATH_SEP) {
            push(p.to_string());
        }
    }

    // 2. Standard library paths.
    if let Ok(v) = std::env::var("PXR_MTLX_STDLIB_SEARCH_PATHS") {
        for p in v.split(PATH_SEP) {
            push(p.to_string());
        }
    }

    // 3. DCC_LOCATION/libraries fallback.
    if let Ok(dcc) = std::env::var("DCC_LOCATION") {
        let lib_path = format!("{}/libraries", dcc.trim_end_matches('/'));
        push(lib_path);
    }

    // 4. Auto-detect: look for libraries/ relative to exe or workspace root.
    if let Ok(exe) = std::env::current_exe() {
        // Check <exe_dir>/libraries/ and <exe_dir>/../libraries/
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("libraries");
            if candidate.is_dir() {
                push(candidate.to_string_lossy().to_string());
            }
            if let Some(parent) = dir.parent() {
                let candidate = parent.join("libraries");
                if candidate.is_dir() {
                    push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    // 5. Check _ref/MaterialX/libraries relative to CARGO_MANIFEST_DIR (dev builds).
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .find(|p| p.join("_ref").is_dir())
            .map(|p| p.to_path_buf());
        if let Some(ws) = workspace {
            let candidate = ws.join("_ref/MaterialX/libraries");
            if candidate.is_dir() {
                push(candidate.to_string_lossy().to_string());
            }
        }
    }

    paths
}

/// Returns a cached handle to the MaterialX standard libraries document.
///
/// Loads stdlib once via `load_libraries()` using `get_search_paths()`.
/// Returns a clone of the cached document, or an empty document on failure.
pub fn get_std_libraries() -> Document {
    let cell = STD_LIBRARIES.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap();

    if guard.is_none() {
        let search_paths: Vec<PathBuf> =
            get_search_paths().into_iter().map(PathBuf::from).collect();

        let mut stdlib = create_document();
        // Empty library_names = load all .mtlx files found in search paths.
        if let Err(e) = load_libraries(&mut stdlib, &search_paths, &[]) {
            log::warn!("HdMtlx: failed to load MaterialX stdlib: {e}");
        }
        *guard = Some(stdlib);
    }

    // Return a fresh document with stdlib imported.
    let mut out = create_document();
    if let Some(ref stdlib) = *guard {
        out.import_library(stdlib);
    }
    out
}

/// Converts a Hydra parameter value to a MaterialX-compatible string.
///
/// Handles: bool, i32, i64, f32, f64, String, Vec2f, Vec3f, Vec4f,
/// Matrix3d, Matrix4d, Token, SdfAssetPath.  Falls back to Debug repr.
pub fn convert_to_string(value: &VtValue) -> String {
    if value.is_empty() {
        return String::new();
    }

    if let Some(v) = value.get::<bool>() {
        return v.to_string();
    }
    if let Some(v) = value.get::<i32>() {
        return v.to_string();
    }
    if let Some(v) = value.get::<i64>() {
        return v.to_string();
    }
    if let Some(v) = value.get::<f32>() {
        return format!("{v}");
    }
    if let Some(v) = value.get::<f64>() {
        return format!("{v}");
    }
    if let Some(v) = value.get::<String>() {
        return v.clone();
    }
    if let Some(v) = value.get::<Token>() {
        return v.as_str().to_string();
    }
    // GfVec2f -> "x, y"
    if let Some(v) = value.get::<Vec2f>() {
        return format!("{}, {}", v.x, v.y);
    }
    // GfVec3f -> "x, y, z"
    if let Some(v) = value.get::<Vec3f>() {
        return format!("{}, {}, {}", v.x, v.y, v.z);
    }
    // GfVec4f -> "x, y, z, w"
    if let Some(v) = value.get::<Vec4f>() {
        return format!("{}, {}, {}, {}", v.x, v.y, v.z, v.w);
    }
    // GfMatrix3d -> space-separated row-major floats
    if let Some(v) = value.get::<Matrix3d>() {
        let mut parts = Vec::with_capacity(9);
        for row in 0..3usize {
            for col in 0..3usize {
                parts.push(v[row][col].to_string());
            }
        }
        return parts.join(", ");
    }
    // GfMatrix4d -> space-separated row-major floats
    if let Some(v) = value.get::<Matrix4d>() {
        let mut parts = Vec::with_capacity(16);
        for row in 0..4usize {
            for col in 0..4usize {
                parts.push(v[row][col].to_string());
            }
        }
        return parts.join(", ");
    }

    // Fallback: Debug repr, strip "Value(...)" wrapper.
    let debug_str = format!("{:?}", value);
    if let Some(inner) = debug_str
        .strip_prefix("Value(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return inner.trim_matches('"').to_string();
    }
    debug_str
}

// ---------------------------------------------------------------------------
// USD type -> MaterialX type mapping (mirrors C++ _ConvertToMtlxType)

/// Converts a USD type name token to the corresponding MaterialX type string.
/// Returns empty string if the type is unknown.
fn convert_to_mtlx_type(usd_type_name: &str) -> &'static str {
    match usd_type_name {
        "bool" => "boolean",
        "int" => "integer",
        "intarray" => "integerarray",
        "float" => "float",
        "floatarray" => "floatarray",
        "color3f" => "color3",
        "color3fArray" => "color3array",
        "color4f" => "color4",
        "color4fArray" => "color4array",
        "float2" => "vector2",
        "float2Array" => "vector2array",
        "float3" => "vector3",
        "float3Array" => "vector3array",
        "float4" => "vector4",
        "float4Array" => "vector4array",
        "matrix3d" => "matrix33",
        "matrix4d" => "matrix44",
        "asset" => "filename",
        "string" => "string",
        "stringArray" => "stringarray",
        _ => "",
    }
}

/// Returns the MaterialX input type, mirroring C++ _GetMxInputType.
///
/// If `usd_type_name` is non-empty, uses the USD→MX type table.
/// Otherwise looks up the input type from the NodeDef.
fn get_mx_input_type(
    mx_node_def: Option<&ElementPtr>,
    mx_input_name: &str,
    usd_type_name: &str,
) -> String {
    // If a USD type name is given, convert it to MX type
    if !usd_type_name.is_empty() {
        let mx_type = convert_to_mtlx_type(usd_type_name);
        if !mx_type.is_empty() {
            return mx_type.to_string();
        }
    }

    // Otherwise look to the nodedef to get the input type
    if let Some(nd) = mx_node_def {
        if let Some(inp) = get_active_input(nd, mx_input_name) {
            if let Some(t) = inp.borrow().get_type() {
                return t.to_string();
            }
        }
    }

    String::new()
}

/// Returns the MaterialX node string with namespace prepended when present.
/// Mirrors C++ _GetMxNodeString.
fn get_mx_node_string(mx_node_def: &ElementPtr) -> String {
    let node_string = nodedef_get_node_string(mx_node_def).unwrap_or_default();
    let borrowed = mx_node_def.borrow();
    if borrowed.has_namespace() {
        format!("{}:{}", borrowed.get_namespace(), node_string)
    } else {
        node_string
    }
}

/// Checks if a NodeDef's implementation NodeGraph uses texcoord nodes.
/// Mirrors C++ _UsesTexcoordNode.
fn uses_texcoord_node(mx_node_def: &ElementPtr) -> bool {
    // Check if the nodedef's implementation is a NodeGraph that contains texcoord nodes.
    // In mtlx-rs we check children of the implementation for "texcoord" category.
    let borrowed = mx_node_def.borrow();
    for child in borrowed.get_children() {
        let cat = child.borrow().get_category().to_string();
        if cat == "nodegraph" || cat == "NodeGraph" {
            for node in child.borrow().get_children() {
                if node.borrow().get_category() == "texcoord" {
                    return true;
                }
            }
        }
    }
    false
}

/// Creates a valid MaterialX identifier from a USD path.
///
/// Returns the *last* path component (name), matching C++ `HdMtlxCreateNameFromPath`
/// which calls `path.GetName()`.  Falls back to full sanitized path if the
/// name is empty (e.g. for the absolute root path "/").
pub fn create_name_from_path(path: &SdfPath) -> String {
    // Get the last element name — equivalent to SdfPath::GetName() in C++.
    let name = path.get_name();
    if !name.is_empty() && name != "/" {
        return sanitize_mtlx_name(name);
    }

    // Edge case: root path "/" — sanitize the full path string.
    let path_str = path.as_str();
    let sanitized = sanitize_mtlx_name(path_str);
    if sanitized.is_empty() {
        "mtlx_node".to_string()
    } else {
        sanitized
    }
}

/// Sanitize a string into a valid MaterialX identifier.
fn sanitize_mtlx_name(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for (i, ch) in s.chars().enumerate() {
        match ch {
            'a'..='z' | 'A'..='Z' | '_' => result.push(ch),
            '0'..='9' if i > 0 => result.push(ch),
            '/' | '.' | ':' => result.push('_'),
            _ => {}
        }
    }
    result
}

/// Returns the MaterialX terminal name for a given Hydra terminal type.
///
/// Matches C++ HdMtlxGetMxTerminalName:
/// - "surfaceshader"     -> "Surface"
/// - "displacementshader"-> "Displacement"
/// - everything else    -> "Surface" (including "volumeshader")
pub fn get_mx_terminal_name(terminal_type: &str) -> String {
    match terminal_type {
        "surfaceshader" => "Surface".to_string(),
        "displacementshader" => "Displacement".to_string(),
        _ => "Surface".to_string(),
    }
}

/// Gets the MaterialX terminal name from an interface path and terminal token.
///
/// Looks up the nodedef type for the terminal node, then delegates to
/// `get_mx_terminal_name()`.
pub fn get_mx_terminal_name_from_interface(
    _terminal_path: &SdfPath,
    terminal_token: &str,
) -> String {
    get_mx_terminal_name(terminal_token)
}

/// Returns MaterialX standard library search paths. Same as `get_search_paths`.
pub fn get_search_paths_cached() -> Vec<String> {
    get_search_paths()
}

/// Looks up a MaterialX NodeDef by token from the standard library.
///
/// Steps (mirroring C++ HdMtlxGetNodeDef):
/// 1. If name starts with "ND_" look it up directly in stdlib.
/// 2. Otherwise try `get_matching_node_defs()` to find it by node name.
/// 3. Handles MX >= 1.39 swizzle node backward compat.
pub fn get_node_def(node_id: &str) -> Option<String> {
    let stdlib = get_std_libraries();

    // Direct ND_ lookup.
    if node_id.starts_with("ND_") {
        if stdlib.get_node_def(node_id).is_some() {
            return Some(node_id.to_string());
        }
        // MX >= 1.39 renamed ND_normalmap -> ND_normalmap_float.
        if node_id == "ND_normalmap" {
            let (maj, min) = stdlib.get_version_integers();
            if combined_version(maj, min) >= MTLX_COMBINED_VERSION_139 {
                let alt = "ND_normalmap_float";
                if stdlib.get_node_def(alt).is_some() {
                    return Some(alt.to_string());
                }
            }
        }
        return None;
    }

    // Try matching by node name.
    let matches = stdlib.get_matching_node_defs(node_id);
    if !matches.is_empty() {
        let name = matches[0].borrow().get_name().to_string();
        return Some(name);
    }

    // Swizzle backward compat: not in stdlib since MX 1.39.
    if node_id == "swizzle" {
        let (maj, min) = stdlib.get_version_integers();
        if combined_version(maj, min) >= MTLX_COMBINED_VERSION_139 {
            // Return synthetic name; callers handle the absent NodeDef.
            return Some("ND_swizzle".to_string());
        }
    }

    None
}

/// Returns the versioned MaterialX NodeDef name for a node identifier.
///
/// Matches C++ HdMtlxGetNodeDefName:
/// - If already "ND_*", return as-is (possibly applying 1.39 renames).
/// - Otherwise prefix with "ND_".
/// - If MX stdlib >= 1.39, rename ND_normalmap -> ND_normalmap_float.
pub fn get_node_def_name(node_id: &str) -> String {
    let base = if node_id.starts_with("ND_") {
        node_id.to_string()
    } else {
        format!("ND_{node_id}")
    };

    // Apply MX >= 1.39 renames if stdlib is available.
    if base == "ND_normalmap" {
        let stdlib = get_std_libraries();
        let (maj, min) = stdlib.get_version_integers();
        if combined_version(maj, min) >= MTLX_COMBINED_VERSION_139 {
            return "ND_normalmap_float".to_string();
        }
    }

    base
}

// ---------------------------------------------------------------------------
// Internal helpers for building a MaterialX document from a Hydra network.

/// Cache for swizzle synthetic NodeDefs (MX >= 1.39 backward compat).
static SWIZZLE_DOC: OnceCell<Mutex<Document>> = OnceCell::new();

/// Resolves a NodeDef from the stdlib document by Hydra node type string.
/// Mirrors C++ `_GetNodeDef` — tries stdlib, then SDR registry, then swizzle compat.
fn resolve_node_def_from_stdlib(hd_node_type: &str, stdlib: &Document) -> Option<ElementPtr> {
    // 1. Direct lookup by name in stdlib.
    if let Some(nd) = stdlib.get_node_def(hd_node_type) {
        return Some(nd);
    }

    // 2. Try with versioned name (e.g. ND_normalmap -> ND_normalmap_float for MX >= 1.39).
    let versioned = get_node_def_name(hd_node_type);
    if versioned != hd_node_type {
        if let Some(nd) = stdlib.get_node_def(&versioned) {
            return Some(nd);
        }
    }

    // 3. Try SDR registry for custom node implementations (mirrors C++ SdrRegistry fallback).
    let mtlx_token = Token::new("mtlx");
    let hd_token = Token::new(hd_node_type);
    let registry = SdrRegistry::get_instance();
    if let Some(sdr_node) = registry.get_shader_node_by_identifier_and_type(&hd_token, &mtlx_token)
    {
        let asset_path = sdr_node.get_resolved_implementation_uri();
        if !asset_path.is_empty() {
            // Load the external .mtlx file into stdlib and retry lookup.
            // Note: load_library modifies the shared stdlib — acceptable since C++ does the same.
            if let Err(e) = mtlx_rs::format::util::load_library(
                &mut stdlib.clone(),
                std::path::Path::new(asset_path),
            ) {
                log::warn!("HdMtlx: failed to load custom mtlx '{}': {}", asset_path, e);
            } else {
                let impl_name = sdr_node.get_implementation_name();
                if let Some(nd) = stdlib.get_node_def(&impl_name) {
                    return Some(nd);
                }
            }
        }
    }

    // 4. Try matching by node name (for non-ND_ identifiers).
    let matches = stdlib.get_matching_node_defs(hd_node_type);
    if !matches.is_empty() {
        return Some(matches[0].clone());
    }

    // 5. Swizzle backward compat: ND_swizzle nodes removed in MX >= 1.39.
    //    C++ creates synthetic NodeDefs matching regex ND_swizzle_<intype>_<outtype>.
    if hd_node_type.starts_with("ND_swizzle_") {
        let (maj, min) = stdlib.get_version_integers();
        if combined_version(maj, min) >= MTLX_COMBINED_VERSION_139 {
            return create_swizzle_node_def(hd_node_type);
        }
    }

    None
}

/// Creates a synthetic swizzle NodeDef for MX >= 1.39 backward compat.
/// Parses "ND_swizzle_<intype>_<outtype>" and creates a temporary NodeDef
/// with "in" input and "channels" input.
fn create_swizzle_node_def(node_type_str: &str) -> Option<ElementPtr> {
    // Parse ND_swizzle_<intype>_<outtype>
    let rest = node_type_str.strip_prefix("ND_swizzle_")?;
    let parts: Vec<&str> = rest.splitn(2, '_').collect();
    if parts.len() != 2 {
        return None;
    }
    let in_type = parts[0];
    let out_type = parts[1];

    let cell = SWIZZLE_DOC.get_or_init(|| Mutex::new(create_document()));
    let mut doc = cell.lock().unwrap();

    // Return existing if already created.
    if let Some(nd) = doc.get_node_def(node_type_str) {
        return Some(nd);
    }

    // Create synthetic NodeDef.
    match doc.add_node_def(node_type_str, out_type, "swizzle") {
        Ok(nd) => {
            let _ = add_input(&nd, "in", in_type);
            let _ = add_input(&nd, "channels", "string");
            Some(nd)
        }
        Err(_) => None,
    }
}

/// Node info collected during upstream traversal.
struct MtlxNodeInfo {
    /// MaterialX node category (= the "node" string from NodeDef).
    category: String,
    /// MaterialX type string (= output type).
    mx_type: String,
    /// Hydra node name token.
    hd_node_name: Token,
    /// Resolved NodeDef element (for type resolution in add_parameter_inputs).
    mx_node_def: Option<ElementPtr>,
}

/// Recursively gather all upstream nodes reachable from `node_name`
/// (mirrors C++ `_GatherUpstreamNodes`).
///
/// Populates `visited` with Hydra node names and `infos` with node metadata.
fn gather_upstream_nodes<I: HdMaterialNetworkInterface>(
    interface: &I,
    node_name: &Token,
    visited: &mut HashSet<String>,
    infos: &mut Vec<MtlxNodeInfo>,
) {
    if !visited.insert(node_name.as_str().to_string()) {
        return;
    }

    let node_type = interface.get_node_type(node_name);
    if node_type.as_str().is_empty() {
        log::warn!("Could not find the connected Node '{}'", node_name.as_str());
        return;
    }

    // Look up the NodeDef from stdlib (mirrors C++ _GetNodeDef + _AddMaterialXNode).
    let stdlib = get_std_libraries();
    let mx_node_def = resolve_node_def_from_stdlib(node_type.as_str(), &stdlib);

    // Get category and output type from the NodeDef.
    let (category, mx_type) = if let Some(ref nd) = mx_node_def {
        let cat = get_mx_node_string(nd);
        let outputs = get_outputs(nd);
        let out_type = outputs
            .first()
            .and_then(|o| o.borrow().get_type().map(|s| s.to_string()))
            .unwrap_or_else(|| "surfaceshader".to_string());
        (cat, out_type)
    } else {
        // Unknown node — use ND_surface fallback like C++.
        log::warn!("NodeDef not found for Node '{}'", node_type.as_str());
        let nd_name = get_node_def_name(node_type.as_str());
        let cat = if nd_name.starts_with("ND_") {
            nd_name[3..].to_string()
        } else {
            nd_name
        };
        (cat, "surfaceshader".to_string())
    };

    infos.push(MtlxNodeInfo {
        category,
        mx_type,
        hd_node_name: node_name.clone(),
        mx_node_def,
    });

    // Recurse into connected inputs.
    for input_name in interface.get_node_input_connection_names(node_name) {
        let connections = interface.get_node_input_connection(node_name, &input_name);
        for conn in connections {
            gather_upstream_nodes(interface, &conn.upstream_node_name, visited, infos);
        }
    }
}

/// Add parameter inputs to an MX node from the Hydra interface.
///
/// Mirrors C++ `_AddParameterInputs`.
fn add_parameter_inputs<I: HdMaterialNetworkInterface>(
    interface: &I,
    node_name: &Token,
    mx_node_def: Option<&ElementPtr>,
    mx_node: &ElementPtr,
    _texture_primvar_data: &mut Option<&mut HdMtlxTexturePrimvarData>,
) {
    let param_names = interface.get_authored_node_parameter_names(node_name);
    for param_name in &param_names {
        let mx_input_name = param_name.as_str();

        // Skip colorSpace:inputName and typeName:inputName parameters —
        // these are metadata already captured in the paramData.
        if mx_input_name.starts_with("colorSpace:") || mx_input_name.starts_with("typeName:") {
            continue;
        }

        let param_data = interface.get_node_parameter_data(node_name, param_name);
        let value_str = convert_to_string(&param_data.value);

        // Determine the MaterialX input type using the full C++ logic:
        // 1. Use usdTypeName → MX type table if available
        // 2. Fall back to NodeDef input type
        let type_str = get_mx_input_type(mx_node_def, mx_input_name, param_data.type_name.as_str());

        // Add input to the MX node with the resolved type.
        let effective_type = if type_str.is_empty() {
            "string"
        } else {
            &type_str
        };
        if let Ok(inp) = add_input(mx_node, mx_input_name, effective_type) {
            inp.borrow_mut().set_attribute("value", &value_str);

            // Set colorSpace on the input if present.
            if !param_data.color_space.as_str().is_empty() {
                inp.borrow_mut()
                    .set_color_space(param_data.color_space.as_str());
            }
        }
    }
}

/// Wire up inter-node connections inside a NodeGraph.
/// Mirrors C++ `_AddNodeInput` — handles multi-output nodes specially.
fn add_node_connections<I: HdMaterialNetworkInterface>(
    interface: &I,
    node_name: &Token,
    mx_node: &ElementPtr,
    mx_node_map: &HashMap<String, ElementPtr>,
    stdlib: &Document,
) {
    let conn_names = interface.get_node_input_connection_names(node_name);
    for input_name in &conn_names {
        let connections = interface.get_node_input_connection(node_name, input_name);
        for conn in &connections {
            let upstream_name = conn.upstream_node_name.as_str();
            let upstream_mx = match mx_node_map.get(upstream_name) {
                Some(n) => n,
                None => continue,
            };

            let out_name = conn.upstream_output_name.as_str();

            // Multi-output node handling (mirrors C++ _AddNodeInput).
            if upstream_mx.borrow().is_multi_output_type() {
                // Look up the upstream NodeDef to find the named output type.
                let hd_next_type = interface.get_node_type(&conn.upstream_node_name);
                if let Some(next_nd) = resolve_node_def_from_stdlib(hd_next_type.as_str(), stdlib) {
                    let nd_outputs = get_outputs(&next_nd);
                    if let Some(conn_output) = nd_outputs
                        .iter()
                        .find(|o| o.borrow().get_name() == out_name)
                    {
                        let out_type = conn_output
                            .borrow()
                            .get_type()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "color3".to_string());
                        if let Ok(inp) = add_input(mx_node, input_name.as_str(), &out_type) {
                            // setConnectedOutput on the input.
                            inp.borrow_mut()
                                .set_node_name(upstream_mx.borrow().get_name());
                            inp.borrow_mut().set_attribute("output", out_name);
                        }
                    }
                }
            } else {
                // Normal single-output node connection.
                let conn_type = upstream_mx
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "color3".to_string());

                if let Ok(inp) = add_input(mx_node, input_name.as_str(), &conn_type) {
                    inp.borrow_mut()
                        .set_node_name(upstream_mx.borrow().get_name());
                }
            }
        }
    }
}

/// Creates a NodeGraph from terminal node connections.
///
/// Mirrors C++ `_CreateNodeGraphFromTerminalNodeConnections`.
fn create_node_graph_from_terminal_connections<I: HdMaterialNetworkInterface>(
    interface: &I,
    material_path: &SdfPath,
    terminal_node_name: &Token,
    terminal_connections: &[InputConnection],
    mx_doc: &mut Document,
    stdlib: &Document,
    texture_primvar_data: &mut Option<&mut HdMtlxTexturePrimvarData>,
) -> Option<ElementPtr> {
    // Gather all upstream nodes.
    let mut visited: HashSet<String> = HashSet::new();
    let mut infos: Vec<MtlxNodeInfo> = Vec::new();

    for conn in terminal_connections {
        gather_upstream_nodes(
            interface,
            &conn.upstream_node_name,
            &mut visited,
            &mut infos,
        );
    }

    if infos.is_empty() {
        return None;
    }

    // Create a NodeGraph named after the material path.
    let ng_name = create_name_from_path(material_path) + "_graph";
    let ng = mx_doc.add_node_graph(&ng_name).ok()?;

    // Build name -> ElementPtr map for wiring connections.
    let mut mx_node_map: HashMap<String, ElementPtr> = HashMap::new();

    // Add all upstream nodes to the NodeGraph.
    for info in &infos {
        let raw_path = format!("/{}", info.hd_node_name.as_str().replace('/', "_"));
        let tmp_path = SdfPath::from_string(&raw_path).unwrap_or_else(SdfPath::absolute_root);
        let mx_name = create_name_from_path(&tmp_path);

        let nd_name = get_node_def_name(&info.category);
        let _ = &nd_name; // Used conceptually for NodeDef lookup.

        // Add node to NodeGraph: add_child_of_category(ng, category, name).
        if let Ok(mx_node) = add_child_of_category(&ng, &info.category, &mx_name) {
            mx_node.borrow_mut().set_type(&info.mx_type);

            // Add parameter inputs (pass nodedef for type resolution).
            let nd_for_params = info.mx_node_def.as_ref();
            add_parameter_inputs(
                interface,
                &info.hd_node_name,
                nd_for_params,
                &mx_node,
                texture_primvar_data,
            );

            mx_node_map.insert(info.hd_node_name.as_str().to_string(), mx_node);
        }
    }

    // Wire connections between nodes.
    for info in &infos {
        if let Some(mx_node) = mx_node_map.get(info.hd_node_name.as_str()).cloned() {
            add_node_connections(
                interface,
                &info.hd_node_name,
                &mx_node,
                &mx_node_map,
                stdlib,
            );
        }
    }

    // Add outputs on the NodeGraph for each terminal connection.
    for conn in terminal_connections {
        let upstream_name = conn.upstream_node_name.as_str();
        if let Some(upstream_mx) = mx_node_map.get(upstream_name) {
            let out_type = upstream_mx
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "color3".to_string());

            let out_name = if conn.upstream_output_name.as_str().is_empty() {
                "out".to_string()
            } else {
                conn.upstream_output_name.as_str().to_string()
            };

            if let Ok(out) = add_output(&ng, &out_name, &out_type) {
                out.borrow_mut()
                    .set_node_name(upstream_mx.borrow().get_name());
            }
        }
    }

    // Track texture and primvar nodes (mirrors C++ _AddMaterialXNode mxHdData logic).
    if let Some(tpd) = texture_primvar_data.as_mut() {
        for info in &infos {
            let hd_node_path = SdfPath::from_string(&format!(
                "/{}",
                info.hd_node_name.as_str().replace('/', "_")
            ));

            if let Some(ref nd) = info.mx_node_def {
                let mx_name = mx_node_map
                    .get(info.hd_node_name.as_str())
                    .map(|n| n.borrow().get_name().to_string())
                    .unwrap_or_default();

                // Texture nodes: NodeDef has filename-type inputs.
                let active_inputs = get_active_inputs(nd);
                for inp in &active_inputs {
                    if inp.borrow().get_type() == Some("filename") {
                        tpd.mxHdTextureMap_insert(&mx_name, &inp.borrow().get_name().to_string());
                        if let Some(ref p) = hd_node_path {
                            tpd.add_texture_node(p.clone());
                        }
                    }
                }

                // Primvar nodes: geompropvalue category.
                if info.category == "geompropvalue" {
                    if let Some(ref p) = hd_node_path {
                        tpd.add_primvar_node(p.clone());
                    }
                }

                // Nodes using texcoords: texcoord category or NodeDef uses texcoord.
                if info.category == "texcoord" || uses_texcoord_node(nd) {
                    if let Some(ref p) = hd_node_path {
                        tpd.add_primvar_node(p.clone());
                    }
                }
            }
        }
    }

    // Connect NodeGraph outputs to the terminal node.
    // (The terminal node itself is added by the caller.)
    let _ = terminal_node_name;

    Some(ng)
}

/// Creates a MaterialX document from a Hydra material network interface.
///
/// Full implementation mirroring C++ `HdMtlxCreateMtlxDocumentFromHdMaterialNetworkInterface`.
///
/// Steps:
/// 1. Create document, import stdlib.
/// 2. Add the terminal shader node (e.g. "standard_surface").
/// 3. If it has upstream connections, build a NodeGraph.
/// 4. Add a material node connecting to the shader.
/// 5. Upgrade document version if needed.
/// 6. Validate.
pub fn create_mtlx_document_from_hd_network_interface<I: HdMaterialNetworkInterface>(
    interface: &I,
    material_path: &SdfPath,
    terminal_node: &Token,
    terminal_node_connections: &[InputConnection],
    texture_primvar_data: Option<&mut HdMtlxTexturePrimvarData>,
) -> Document {
    let mut mx_doc = get_std_libraries();
    let stdlib = get_std_libraries();
    let mut tpd = texture_primvar_data;

    // Set document version from material config, default to "1.38".
    // Mirrors C++ which reads "mtlx:version" config.
    let mtlx_version_key = Token::new("mtlx:version");
    let version_value = interface.get_material_config_value(&mtlx_version_key);
    let version_string = version_value
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| "1.38".to_string());
    mx_doc.set_doc_version_string(&version_string);

    // Get the terminal node type (e.g. "ND_standard_surface_surfaceshader").
    let node_type = interface.get_node_type(terminal_node);
    let terminal_node_def = resolve_node_def_from_stdlib(node_type.as_str(), &stdlib);

    // Get category and output type from the NodeDef.
    let (category, terminal_type) = if let Some(ref nd) = terminal_node_def {
        let cat = get_mx_node_string(nd);
        let outs = get_outputs(nd);
        let out_type = outs
            .first()
            .and_then(|o| o.borrow().get_type().map(|s| s.to_string()))
            .unwrap_or_else(|| "surfaceshader".to_string());
        (cat, out_type)
    } else {
        log::warn!(
            "Unsupported terminal node type '{}' cannot find the associated NodeDef.",
            node_type.as_str()
        );
        let nd_name = get_node_def_name(node_type.as_str());
        let cat = if nd_name.starts_with("ND_") {
            nd_name[3..].to_string()
        } else {
            nd_name
        };
        (cat, "surfaceshader".to_string())
    };

    // Terminal name in the MX doc (e.g. "Surface" or "Displacement").
    let mx_terminal_name = get_mx_terminal_name(&terminal_type);

    // Add terminal shader node to the document.
    let shader_node = match mx_doc.add_node(&category, &mx_terminal_name, &terminal_type) {
        Ok(n) => n,
        Err(e) => {
            log::warn!("HdMtlx: failed to add terminal node '{category}': {e}");
            return mx_doc;
        }
    };

    // Set nodedef attribute from the resolved NodeDef (like C++ mxNode->setNodeDefString).
    if let Some(ref nd) = terminal_node_def {
        let nd_name_str = nd.borrow().get_name().to_string();
        shader_node
            .borrow_mut()
            .set_attribute("nodedef", &nd_name_str);
    } else {
        // Fallback: use the hdNodeType string as nodedef (mirrors C++ ND_surface fallback).
        shader_node
            .borrow_mut()
            .set_attribute("nodedef", node_type.as_str());
    }

    // Add authored parameter inputs to the terminal node.
    add_parameter_inputs(
        interface,
        terminal_node,
        terminal_node_def.as_ref(),
        &shader_node,
        &mut tpd,
    );

    // Build NodeGraph for upstream connections if any.
    if !terminal_node_connections.is_empty() {
        let ng = create_node_graph_from_terminal_connections(
            interface,
            material_path,
            terminal_node,
            terminal_node_connections,
            &mut mx_doc,
            &stdlib,
            &mut tpd,
        );

        if let Some(ng_elem) = ng {
            // Connect NodeGraph outputs to terminal shader inputs.
            let ng_name = ng_elem.borrow().get_name().to_string();
            for conn in terminal_node_connections {
                // Determine input name and type.
                let input_name = if conn.upstream_output_name.as_str().is_empty() {
                    conn.upstream_node_name.as_str().to_string()
                } else {
                    conn.upstream_output_name.as_str().to_string()
                };

                // Check if NodeGraph has a matching output.
                let ng_outputs = get_outputs(&ng_elem);
                if let Some(ng_out) = ng_outputs.first() {
                    let out_type = ng_out
                        .borrow()
                        .get_type()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "color3".to_string());

                    if let Ok(inp) = add_input(&shader_node, &input_name, &out_type) {
                        inp.borrow_mut().set_attribute("nodegraph", &ng_name);
                        let out_name = ng_out.borrow().get_name().to_string();
                        inp.borrow_mut().set_attribute("output", &out_name);
                    }
                }
            }
        }
    }

    // Add material node connecting to the shader.
    // C++ uses mxDoc->createValidChildName(materialName).
    let material_name = interface.get_material_prim_path().get_name().to_string();
    let material_node_name = if material_name.is_empty() {
        create_name_from_path(material_path)
    } else {
        material_name
    };
    if let Err(e) = mx_doc.add_material_node(&material_node_name, Some(&shader_node)) {
        log::warn!("HdMtlx: failed to add material node: {e}");
    }

    // Upgrade document to current MX version.
    mx_doc.upgrade_version();

    // Validate.
    if !mx_doc.validate() {
        log::warn!(
            "HdMtlx: MaterialX document validation failed for {}",
            material_path.as_str()
        );
    }

    mx_doc
}

/// Creates a MaterialX document from a Hydra material network (legacy API).
///
/// Stub — legacy `HdMaterialNetwork2`-based API.  Returns an empty document
/// because full traversal requires the `HdMaterialNetwork2` type which is not
/// stabilised in this crate yet.
///
/// # Note
/// Prefer `create_mtlx_document_from_hd_network_interface` for new code.
pub fn create_mtlx_document_from_hd_network(
    _material_path: &SdfPath,
    _texture_primvar_data: Option<&mut HdMtlxTexturePrimvarData>,
) -> Document {
    // Would require HdMaterialNetwork2 traversal — not yet implemented.
    create_document()
}

// ---------------------------------------------------------------------------
// Legacy Vec<u8> shim wrappers kept for backward-compat with old call sites.

/// Serialize a MaterialX document to bytes (placeholder).
///
/// Full implementation would call `mx::XmlIo::writeToXmlString()`.
pub fn document_to_bytes(_doc: &Document) -> Vec<u8> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_convert_to_string_bool() {
        let value = VtValue::new(true);
        assert_eq!(convert_to_string(&value), "true");
        let value = VtValue::new(false);
        assert_eq!(convert_to_string(&value), "false");
    }

    #[test]
    fn test_convert_to_string_int() {
        let value = VtValue::new(42i32);
        assert_eq!(convert_to_string(&value), "42");
        let value = VtValue::new(-100i32);
        assert_eq!(convert_to_string(&value), "-100");
    }

    #[test]
    fn test_convert_to_string_float() {
        let value = VtValue::from(3.14f64);
        assert!(convert_to_string(&value).starts_with("3.14"));
        let value = VtValue::from(0.0f32);
        assert_eq!(convert_to_string(&value), "0");
    }

    #[test]
    fn test_convert_to_string_string() {
        let value = VtValue::new("test".to_string());
        assert_eq!(convert_to_string(&value), "test");
        let value = VtValue::new("hello world".to_string());
        assert_eq!(convert_to_string(&value), "hello world");
    }

    #[test]
    fn test_convert_to_string_empty() {
        let value = VtValue::empty();
        assert_eq!(convert_to_string(&value), "");
    }

    #[test]
    fn test_create_name_from_path() {
        let path = SdfPath::from_string("/Material/Shader").unwrap();
        let name = create_name_from_path(&path);
        // Last component of "/Material/Shader" is "Shader"
        assert_eq!(name, "Shader");
    }

    #[test]
    fn test_create_name_from_root_path() {
        let path = SdfPath::absolute_root();
        let name = create_name_from_path(&path);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_get_mx_terminal_name_surface() {
        assert_eq!(get_mx_terminal_name("surfaceshader"), "Surface");
    }

    #[test]
    fn test_get_mx_terminal_name_displacement() {
        assert_eq!(get_mx_terminal_name("displacementshader"), "Displacement");
    }

    #[test]
    fn test_get_mx_terminal_name_volume_defaults_to_surface() {
        // C++ defaults "volumeshader" to "Surface" too.
        assert_eq!(get_mx_terminal_name("volumeshader"), "Surface");
    }

    #[test]
    fn test_get_mx_terminal_name_unknown() {
        assert_eq!(get_mx_terminal_name("unknown"), "Surface");
    }

    #[test]
    fn test_get_search_paths() {
        // Without env vars set, returns empty (or DCC_LOCATION paths).
        let paths = get_search_paths();
        for path in &paths {
            assert!(!path.is_empty());
        }
    }

    #[test]
    fn test_get_node_def_name_nd_prefix() {
        assert_eq!(
            get_node_def_name("ND_standard_surface_surfaceshader"),
            "ND_standard_surface_surfaceshader"
        );
    }

    #[test]
    fn test_get_node_def_name_bare() {
        assert_eq!(get_node_def_name("standard_surface"), "ND_standard_surface");
    }

    #[test]
    fn test_create_mtlx_document_from_hd_network_stub() {
        let path = SdfPath::from_string("/Material").unwrap();
        let mut data = HdMtlxTexturePrimvarData::new();
        // Stub returns empty document (no network2 type yet).
        let _doc = create_mtlx_document_from_hd_network(&path, Some(&mut data));
    }

    #[test]
    fn test_get_std_libraries_returns_document() {
        // Should not panic; may be empty if no stdlib paths in test env.
        let _doc = get_std_libraries();
    }

    // --- Tests for newly added parity functions ---

    #[test]
    fn test_convert_to_mtlx_type_known_types() {
        assert_eq!(convert_to_mtlx_type("bool"), "boolean");
        assert_eq!(convert_to_mtlx_type("int"), "integer");
        assert_eq!(convert_to_mtlx_type("intarray"), "integerarray");
        assert_eq!(convert_to_mtlx_type("float"), "float");
        assert_eq!(convert_to_mtlx_type("floatarray"), "floatarray");
        assert_eq!(convert_to_mtlx_type("color3f"), "color3");
        assert_eq!(convert_to_mtlx_type("color3fArray"), "color3array");
        assert_eq!(convert_to_mtlx_type("color4f"), "color4");
        assert_eq!(convert_to_mtlx_type("color4fArray"), "color4array");
        assert_eq!(convert_to_mtlx_type("float2"), "vector2");
        assert_eq!(convert_to_mtlx_type("float2Array"), "vector2array");
        assert_eq!(convert_to_mtlx_type("float3"), "vector3");
        assert_eq!(convert_to_mtlx_type("float3Array"), "vector3array");
        assert_eq!(convert_to_mtlx_type("float4"), "vector4");
        assert_eq!(convert_to_mtlx_type("float4Array"), "vector4array");
        assert_eq!(convert_to_mtlx_type("matrix3d"), "matrix33");
        assert_eq!(convert_to_mtlx_type("matrix4d"), "matrix44");
        assert_eq!(convert_to_mtlx_type("asset"), "filename");
        assert_eq!(convert_to_mtlx_type("string"), "string");
        assert_eq!(convert_to_mtlx_type("stringArray"), "stringarray");
    }

    #[test]
    fn test_convert_to_mtlx_type_unknown() {
        assert_eq!(convert_to_mtlx_type("unknown"), "");
        assert_eq!(convert_to_mtlx_type(""), "");
    }

    #[test]
    fn test_get_mx_input_type_with_usd_type() {
        // When USD type name is given, it takes priority over NodeDef.
        assert_eq!(get_mx_input_type(None, "base_color", "color3f"), "color3");
        assert_eq!(get_mx_input_type(None, "roughness", "float"), "float");
        assert_eq!(get_mx_input_type(None, "file", "asset"), "filename");
    }

    #[test]
    fn test_get_mx_input_type_empty_usd_type_no_nodedef() {
        // No USD type name and no NodeDef — returns empty string.
        assert_eq!(get_mx_input_type(None, "base_color", ""), "");
    }

    #[test]
    fn test_get_mx_input_type_unknown_usd_type_no_nodedef() {
        // Unknown USD type and no NodeDef — returns empty.
        assert_eq!(get_mx_input_type(None, "x", "weird_type"), "");
    }
}
