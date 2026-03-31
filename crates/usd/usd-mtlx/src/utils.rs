//! MaterialX utilities for USD integration.
//!
//! This module provides utility functions for converting MaterialX types to USD types,
//! parsing MaterialX values, managing search paths, and caching MaterialX documents.
//!
//! Port of `pxr/usd/usdMtlx/utils.cpp`

use crate::{Document, Element, MtlxValue, NodeDef, create_value_from_strings, split_string};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use usd_gf::{Matrix3d, Matrix4d, Vec2f, Vec3f, Vec4f};
use usd_sdf::{AssetPath, ValueTypeName, ValueTypeRegistry};
use usd_sdr::SdrVersion;
use usd_tf::Token;
use usd_vt::Value as VtValue;

// ============================================================================
// UsdTypeInfo
// ============================================================================

/// Information about USD type mapping for MaterialX types.
///
/// This struct holds the mapping between MaterialX type names and their
/// corresponding USD value types and shader property types.
#[derive(Debug, Clone)]
pub struct UsdTypeInfo {
    /// The USD value type name (e.g., Float, Color3f, Matrix4d).
    pub value_type_name: ValueTypeName,
    /// The shader property type token (e.g., "Float", "Color", "Matrix").
    pub shader_property_type: Token,
    /// Array size for fixed-size arrays/tuples (0 for dynamic arrays).
    pub array_size: i32,
    /// True if the value type name is an exact match to the MaterialX type.
    pub value_type_name_is_exact: bool,
}

impl UsdTypeInfo {
    /// Create a new UsdTypeInfo with the given parameters.
    pub fn new(
        value_type_name: ValueTypeName,
        value_type_name_is_exact: bool,
        shader_property_type: Token,
        array_size: i32,
    ) -> Self {
        Self {
            value_type_name,
            shader_property_type,
            array_size,
            value_type_name_is_exact,
        }
    }

    /// Create an invalid UsdTypeInfo (for unknown types).
    pub fn invalid() -> Self {
        Self {
            value_type_name: ValueTypeName::invalid(),
            shader_property_type: Token::new(""),
            array_size: 0,
            value_type_name_is_exact: false,
        }
    }
}

// ============================================================================
// Type Mapping
// ============================================================================

/// Get USD type information for a MaterialX type name.
///
/// This function maps MaterialX type names (like "color3", "float", "matrix44")
/// to their corresponding USD value types and shader property types.
///
/// # Examples
///
/// ```ignore
/// let type_info = get_usd_type("color3");
/// assert!(type_info.value_type_name_is_exact);
/// assert_eq!(type_info.shader_property_type, Token::new("Color"));
/// ```
pub fn get_usd_type(mtlx_type_name: &str) -> UsdTypeInfo {
    static TYPE_MAP: LazyLock<HashMap<&'static str, UsdTypeInfo>> = LazyLock::new(|| {
        let reg = ValueTypeRegistry::instance();

        let mut map = HashMap::new();

        // Helper to create UsdTypeInfo entries
        let entry =
            |type_name: &str, exact: bool, sdr_type: &str, array_size: i32| -> UsdTypeInfo {
                UsdTypeInfo::new(
                    reg.find_type(type_name),
                    exact,
                    Token::new(sdr_type),
                    array_size,
                )
            };

        // Registry type names are lowercase ("bool", "float", "color3f", etc.)
        map.insert("boolean", entry("bool", true, "", 0));
        map.insert("color3", entry("color3f", true, "Color", 0));
        map.insert("color3array", entry("color3f[]", true, "Color", 0));
        map.insert("color4", entry("color4f", true, "Color4", 0));
        map.insert("color4array", entry("color4f[]", true, "Color4", 0));
        map.insert("filename", entry("asset", true, "String", 0));
        map.insert("float", entry("float", true, "Float", 0));
        map.insert("floatarray", entry("float[]", true, "Float", 0));
        map.insert("geomnamearray", entry("string[]", false, "", 0));
        map.insert("geomname", entry("string", false, "", 0));
        map.insert("integer", entry("int", true, "Int", 0));
        map.insert("integerarray", entry("int[]", true, "Int", 0));
        map.insert("matrix33", entry("matrix3d", true, "", 0));
        map.insert("matrix44", entry("matrix4d", true, "Matrix", 0));
        map.insert("string", entry("string", true, "String", 0));
        map.insert("stringarray", entry("string[]", true, "String", 0));
        map.insert("surfaceshader", entry("token", true, "Terminal", 0));
        // Note: C++ UsdMtlxGetUsdType only maps "surfaceshader". Other shader
        // types (displacementshader, volumeshader, etc.) intentionally return
        // notFound in C++, so we don't add them here.
        map.insert("vector2", entry("float2", true, "Float", 2));
        map.insert("vector2array", entry("float2[]", true, "", 0));
        map.insert("vector3", entry("float3", true, "Float", 3));
        map.insert("vector3array", entry("float3[]", true, "", 0));
        map.insert("vector4", entry("float4", true, "Float", 4));
        map.insert("vector4array", entry("float4[]", true, "", 0));

        map
    });

    TYPE_MAP
        .get(mtlx_type_name)
        .cloned()
        .unwrap_or_else(UsdTypeInfo::invalid)
}

// ============================================================================
// Value Conversion
// ============================================================================

/// Convert a MaterialX element's value to a VtValue.
///
/// This function extracts the value or default value from a MaterialX element
/// and converts it to the appropriate USD VtValue type.
///
/// # Arguments
///
/// * `element` - The MaterialX element containing the value
/// * `get_default` - If true, get the "default" attribute instead of "value"
///
/// # Examples
///
/// ```ignore
/// let value = get_usd_value(&input_element, false);
/// if !value.is_empty() {
///     // Use the value
/// }
/// ```
pub fn get_usd_value(element: &Element, get_default: bool) -> VtValue {
    // Get the value string
    let attr_name = if get_default { "default" } else { "value" };
    let value_string = element.get_attribute(attr_name);

    if value_string.is_empty() {
        return VtValue::empty();
    }

    // Get the type string
    let type_string = element.get_attribute("type");

    // Parse and convert
    convert_mtlx_value_string(value_string, type_string)
}

/// Convert a MaterialX value string to VtValue given its type name.
fn convert_mtlx_value_string(value_str: &str, type_name: &str) -> VtValue {
    if value_str.is_empty() {
        return VtValue::empty();
    }

    // Parse the MaterialX value
    let mtlx_value = match create_value_from_strings(value_str, type_name) {
        Some(v) => v,
        None => return VtValue::empty(),
    };

    // Convert to VtValue
    match mtlx_value {
        // Scalars
        MtlxValue::Bool(b) => VtValue::from(b),
        MtlxValue::Int(i) => VtValue::from(i),
        MtlxValue::Float(f) => VtValue::from(f),
        MtlxValue::String(s) => {
            // Special handling for filename and geomname types
            if type_name == "filename" {
                VtValue::from_no_hash(AssetPath::new(&s))
            } else {
                VtValue::from(s)
            }
        }

        // Vectors
        MtlxValue::Color3(v) => VtValue::from(Vec3f::new(v[0], v[1], v[2])),
        MtlxValue::Color4(v) => VtValue::from(Vec4f::new(v[0], v[1], v[2], v[3])),
        MtlxValue::Vector2(v) => VtValue::from(Vec2f::new(v[0], v[1])),
        MtlxValue::Vector3(v) => VtValue::from(Vec3f::new(v[0], v[1], v[2])),
        MtlxValue::Vector4(v) => VtValue::from(Vec4f::new(v[0], v[1], v[2], v[3])),

        // Matrices - both MaterialX and GfMatrix are row-major, direct copy
        MtlxValue::Matrix33(m) => VtValue::from(Matrix3d::new(
            m[0][0] as f64,
            m[0][1] as f64,
            m[0][2] as f64,
            m[1][0] as f64,
            m[1][1] as f64,
            m[1][2] as f64,
            m[2][0] as f64,
            m[2][1] as f64,
            m[2][2] as f64,
        )),
        MtlxValue::Matrix44(m) => VtValue::from(Matrix4d::new(
            m[0][0] as f64,
            m[0][1] as f64,
            m[0][2] as f64,
            m[0][3] as f64,
            m[1][0] as f64,
            m[1][1] as f64,
            m[1][2] as f64,
            m[1][3] as f64,
            m[2][0] as f64,
            m[2][1] as f64,
            m[2][2] as f64,
            m[2][3] as f64,
            m[3][0] as f64,
            m[3][1] as f64,
            m[3][2] as f64,
            m[3][3] as f64,
        )),

        // Arrays (float-containing use from_no_hash)
        MtlxValue::BoolArray(arr) => VtValue::new(arr),
        MtlxValue::IntArray(arr) => VtValue::new(arr),
        MtlxValue::FloatArray(arr) => VtValue::from_no_hash(arr),
        MtlxValue::StringArray(arr) => VtValue::new(arr),
        MtlxValue::Color3Array(arr) => {
            let vec: Vec<Vec3f> = arr.iter().map(|v| Vec3f::new(v[0], v[1], v[2])).collect();
            VtValue::from(vec)
        }
        MtlxValue::Color4Array(arr) => {
            let vec: Vec<Vec4f> = arr
                .iter()
                .map(|v| Vec4f::new(v[0], v[1], v[2], v[3]))
                .collect();
            VtValue::from_no_hash(vec)
        }
        MtlxValue::Vector2Array(arr) => {
            let vec: Vec<Vec2f> = arr.iter().map(|v| Vec2f::new(v[0], v[1])).collect();
            VtValue::from_no_hash(vec)
        }
        MtlxValue::Vector3Array(arr) => {
            let vec: Vec<Vec3f> = arr.iter().map(|v| Vec3f::new(v[0], v[1], v[2])).collect();
            VtValue::from(vec)
        }
        MtlxValue::Vector4Array(arr) => {
            let vec: Vec<Vec4f> = arr
                .iter()
                .map(|v| Vec4f::new(v[0], v[1], v[2], v[3]))
                .collect();
            VtValue::from_no_hash(vec)
        }
    }
}

/// Convert a packed MaterialX value string to a vector of VtValues.
///
/// This splits a comma-separated list of values and converts each one.
/// Array types cannot be packed, so this returns an empty vector for those.
///
/// # Arguments
///
/// * `values` - Comma-separated value string
/// * `type_name` - MaterialX type name for each value
///
/// # C++ Parity
///
/// Unlike filter_map, this clears ALL results and breaks on ANY failure,
/// matching C++ UsdMtlxGetPackedUsdValues() semantics.
pub fn get_packed_usd_values(values: &str, type_name: &str) -> Vec<VtValue> {
    // Cannot parse packed arrays
    if type_name.ends_with("array") {
        return Vec::new();
    }

    // Explicit loop: clear ALL results on any failure (C++ semantics)
    let mut result = Vec::new();
    for value_str in split_string(values, ",") {
        let converted = convert_mtlx_value_string(&value_str, type_name);
        if converted.is_empty() {
            result.clear();
            break;
        }
        result.push(converted);
    }
    result
}

/// Split a MaterialX string array into individual strings.
///
/// Splits on commas only — exactly matching C++ `UsdMtlxSplitStringArray`
/// which calls `mx::splitString(s, ",")`. Leading/trailing whitespace on
/// each element is stripped.
pub fn split_string_array(s: &str) -> Vec<String> {
    split_string(s, ",")
}

// ============================================================================
// Search Paths
// ============================================================================

/// Get MaterialX standard library search paths.
///
/// Priority order (highest to lowest, matching C++ `_ComputeStdlibSearchPaths`):
/// 1. `PXR_MTLX_STDLIB_SEARCH_PATHS` env var
/// 2. Build-time `PXR_MATERIALX_STDLIB_DIR` env var (Rust equivalent of the C++ macro)
pub fn standard_library_paths() -> &'static Vec<String> {
    static PATHS: LazyLock<Vec<String>> = LazyLock::new(|| {
        let mut paths = get_search_paths_from_env("PXR_MTLX_STDLIB_SEARCH_PATHS");

        // Build-time stdlib dir. C++ uses a compile-time PXR_MATERIALX_STDLIB_DIR macro;
        // we read the same name as an env var so it can be set in the build environment.
        if let Ok(stdlib_dir) = env::var("PXR_MATERIALX_STDLIB_DIR") {
            if !stdlib_dir.is_empty() {
                paths.push(stdlib_dir);
            }
        }

        paths
    });
    &PATHS
}

/// Get custom MaterialX plugin search paths.
///
/// These paths are read from the PXR_MTLX_PLUGIN_SEARCH_PATHS environment variable.
pub fn custom_search_paths() -> &'static Vec<String> {
    static PATHS: LazyLock<Vec<String>> =
        LazyLock::new(|| get_search_paths_from_env("PXR_MTLX_PLUGIN_SEARCH_PATHS"));
    &PATHS
}

/// Get all MaterialX search paths (custom + standard library).
pub fn search_paths() -> &'static Vec<String> {
    static PATHS: LazyLock<Vec<String>> = LazyLock::new(|| {
        let mut all_paths = custom_search_paths().clone();
        all_paths.extend(standard_library_paths().iter().cloned());
        all_paths
    });
    &PATHS
}

/// Get standard MaterialX file extensions.
pub fn standard_file_extensions() -> Vec<String> {
    vec!["mtlx".to_string()]
}

/// Helper to read search paths from an environment variable.
fn get_search_paths_from_env(var_name: &str) -> Vec<String> {
    match env::var(var_name) {
        Ok(paths) => {
            #[cfg(windows)]
            const SEP: char = ';';
            #[cfg(not(windows))]
            const SEP: char = ':';

            paths
                .split(SEP)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Recursively collect all `.mtlx` file paths under `dir`.
/// Mirrors C++ `SdrFsHelpersDiscoverFiles` recursive behaviour.
fn collect_mtlx_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let ep = entry.path();
        if ep.is_dir() {
            collect_mtlx_files(&ep, out);
        } else if ep.extension().is_some_and(|e| e == "mtlx") {
            out.push(ep);
        }
    }
}

/// Import all `.mtlx` files from one search-path entry (file or directory) into `doc`.
fn import_from_search_path(dir_path: &str, doc: &mut Document) {
    let path = std::path::Path::new(dir_path);
    if path.is_file() {
        if let Some(lib_doc) = read_document(dir_path) {
            doc.import_library(&lib_doc);
        }
    } else if path.is_dir() {
        let mut files = Vec::new();
        collect_mtlx_files(path, &mut files);
        for ep in files {
            let uri = ep.to_string_lossy();
            if let Some(lib_doc) = read_document(uri.as_ref()) {
                doc.import_library(&lib_doc);
            }
        }
    }
}

// ============================================================================
// Document Caching
// ============================================================================

/// Cache for MaterialX documents (URI -> Document).
static DOCUMENT_CACHE: LazyLock<Mutex<HashMap<String, Option<Document>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Read a MaterialX document from a resolved path (no caching).
///
/// This function loads a MaterialX document from the given path.
/// For production use, this should integrate with the ArResolver system.
pub fn read_document(resolved_path: &str) -> Option<Document> {
    #[cfg(feature = "mtlx-rs")]
    {
        if let Some(doc) = read_document_mtlx_rs(resolved_path) {
            return Some(doc);
        }
    }

    use crate::read_from_xml_file;

    match read_from_xml_file(resolved_path) {
        Ok(doc) => Some(doc),
        Err(_e) => {
            // In production, log error with TF_RUNTIME_ERROR
            None
        }
    }
}

/// Read a MaterialX document via mtlx-rs (full XInclude, FileSearchPath).
///
/// Only available when the `mtlx-rs` feature is enabled.
#[cfg(feature = "mtlx-rs")]
pub fn read_document_mtlx_rs(resolved_path: &str) -> Option<Document> {
    use crate::document_from_mtlx_rs;
    use mtlx_rs::format::{FilePath, FileSearchPath, read_from_xml_file};

    let path = std::path::Path::new(resolved_path);

    let mut search_path = FileSearchPath::new();
    if let Some(parent) = path.parent() {
        search_path.append(FilePath::new(parent));
    }
    for dir in search_paths().iter() {
        search_path.append(FilePath::new(dir));
    }

    let opts = mtlx_rs::format::XmlReadOptions {
        read_xinclude: true,
        search_path: Some(search_path),
        parent_xincludes: vec![],
    };

    match read_from_xml_file(path, FileSearchPath::new(), Some(&opts)) {
        Ok(mtlx_doc) => Some(document_from_mtlx_rs(&mtlx_doc)),
        Err(_) => None,
    }
}

/// Get a cached MaterialX document by resolved URI.
///
/// If the URI is empty, this returns a document containing all standard library
/// documents merged together. Otherwise, it loads and caches the document at the
/// given URI.
pub fn get_document(resolved_uri: &str) -> Option<Document> {
    let mut cache = DOCUMENT_CACHE.lock();

    // Check cache
    if let Some(cached) = cache.get(resolved_uri) {
        return cached.clone();
    }

    // Load document
    let document = if resolved_uri.is_empty() {
        // Empty URI: discover and merge all standard + custom library .mtlx files.
        // Mirrors C++ UsdMtlxGetDocument("") -> _ImportLibraries (recursive).
        let mut doc = Document::create();

        // Standard library paths (recursive walk, matches C++ _ImportLibraries)
        for dir_path in standard_library_paths().iter() {
            import_from_search_path(dir_path, &mut doc);
        }

        // Custom plugin paths (recursive walk)
        for dir_path in custom_search_paths().iter() {
            import_from_search_path(dir_path, &mut doc);
        }

        Some(doc)
    } else {
        read_document(resolved_uri)
    };

    // Cache and return
    cache.insert(resolved_uri.to_string(), document.clone());
    document
}

/// Get a MaterialX document from XML string (cached by hash).
///
/// This parses the XML string and caches the result based on a hash of the content.
pub fn get_document_from_string(mtlx_xml: &str) -> Option<Document> {
    use crate::read_from_xml_string;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Compute hash for caching
    let mut hasher = DefaultHasher::new();
    mtlx_xml.hash(&mut hasher);
    let hash_str = hasher.finish().to_string();

    let mut cache = DOCUMENT_CACHE.lock();

    // Check cache
    if let Some(cached) = cache.get(&hash_str) {
        return cached.clone();
    }

    // Parse document
    let document = match read_from_xml_string(mtlx_xml) {
        Ok(doc) => Some(doc),
        Err(_e) => {
            // In production, log error with TF_DEBUG
            None
        }
    };

    // Cache and return
    cache.insert(hash_str, document.clone());
    document
}

// ============================================================================
// Version and URI Utilities
// ============================================================================

/// Get the version from a MaterialX NodeDef element.
///
/// This reads the version string and isdefaultversion attributes and returns
/// an SdrVersion plus an implicit_default flag.
///
/// # Returns
///
/// * `(SdrVersion, bool)` - The version and implicit_default flag
///   - `implicit_default == false` means explicitly marked as default
///   - `implicit_default == true` means NOT explicitly marked (potential implicit default)
///
/// # C++ Signature
///
/// `SdrVersion UsdMtlxGetVersion(const mx::ConstInterfaceElementPtr& mtlx, bool* implicitDefault = nullptr)`
pub fn get_version(nodedef: &NodeDef) -> (SdrVersion, bool) {
    let version_str = nodedef.get_version_string();
    let is_default = nodedef.get_default_version();

    // C++: auto version = SdrVersion().GetAsDefault();
    // Start with invalid-but-default version, same as C++.
    // If version string is non-empty but malformed, fall back to invalid-as-default.
    let mut version = if version_str.is_empty() {
        SdrVersion::invalid().as_default()
    } else {
        let parsed = SdrVersion::from_string(version_str);
        if parsed.is_valid() {
            parsed
        } else {
            // Invalid version string -> keep invalid-but-default (C++ behavior)
            SdrVersion::invalid().as_default()
        }
    };

    let implicit_default;
    if is_default {
        implicit_default = false; // Explicitly marked as default
        version = version.as_default();
    } else {
        // No opinion means implicitly a (potential) default.
        implicit_default = true;
    }

    (version, implicit_default)
}

/// Get the source URI for a MaterialX element.
///
/// This walks up the parent chain to find the first non-empty source URI.
pub fn get_source_uri(element: &Element) -> String {
    let mut current = Some(element.clone());
    while let Some(elem) = current {
        let uri = elem.get_source_uri();
        if !uri.is_empty() {
            return uri.to_string();
        }
        current = elem.get_parent();
    }
    String::new()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_usd_type_basic() {
        // Boolean
        let info = get_usd_type("boolean");
        assert!(info.value_type_name_is_exact);
        assert!(info.value_type_name.is_valid());

        // Float
        let info = get_usd_type("float");
        assert!(info.value_type_name_is_exact);
        assert!(info.value_type_name.is_valid());
        assert_eq!(info.shader_property_type, Token::new("Float"));

        // Color3
        let info = get_usd_type("color3");
        assert!(info.value_type_name_is_exact);
        assert!(info.value_type_name.is_valid());
        assert_eq!(info.shader_property_type, Token::new("Color"));

        // Matrix44
        let info = get_usd_type("matrix44");
        assert!(info.value_type_name_is_exact);
        assert!(info.value_type_name.is_valid());
        assert_eq!(info.shader_property_type, Token::new("Matrix"));

        // Vector2 with array_size
        let info = get_usd_type("vector2");
        assert!(info.value_type_name_is_exact);
        assert_eq!(info.array_size, 2);
    }

    #[test]
    fn test_get_usd_type_unknown() {
        let info = get_usd_type("unknowntype");
        assert!(!info.value_type_name_is_exact);
        assert!(!info.value_type_name.is_valid());
    }

    #[test]
    fn test_split_string_array() {
        let result = split_string_array("red, green, blue");
        assert_eq!(result, vec!["red", "green", "blue"]);

        let result = split_string_array("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_search_paths() {
        // These should return valid vectors (may be empty if env vars not set)
        let stdlib = standard_library_paths();
        let custom = custom_search_paths();
        let all = search_paths();

        // All should be accessible
        let _ = &stdlib;
        let _ = &custom;
        let _ = &all;
    }

    #[test]
    fn test_standard_file_extensions() {
        let exts = standard_file_extensions();
        assert_eq!(exts, vec!["mtlx"]);
    }

    #[test]
    fn test_get_packed_usd_values_fail_semantics() {
        // C++ semantics: clear ALL results on ANY failure
        let result = get_packed_usd_values("1.0, invalid, 3.0", "float");
        assert_eq!(result.len(), 0, "Should clear all on failure");

        // Valid values should work
        let result = get_packed_usd_values("1.0, 2.0, 3.0", "float");
        assert_eq!(result.len(), 3);

        // Array types should return empty
        let result = get_packed_usd_values("1.0, 2.0", "floatarray");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_get_version_tuple() {
        use crate::read_from_xml_string;

        // NodeDef without version, not default
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="test_node" type="surfaceshader" node="shader"/>
            </materialx>"#;
        let doc = read_from_xml_string(xml).unwrap();
        let nodedef = doc.get_node_def("test_node").unwrap();

        let (version, implicit_default) = get_version(&nodedef);
        assert!(!version.is_valid());
        assert!(implicit_default, "Should be implicit when not marked");
    }

    #[test]
    fn test_get_version_with_version_string() {
        use crate::read_from_xml_string;

        // NodeDef with version string, not default
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="test_node" type="surfaceshader" node="shader" version="2.1"/>
            </materialx>"#;
        let doc = read_from_xml_string(xml).unwrap();
        let nodedef = doc.get_node_def("test_node").unwrap();

        let (version, implicit_default) = get_version(&nodedef);
        assert!(version.is_valid());
        assert_eq!(version.major(), 2);
        assert_eq!(version.minor(), 1);
        assert!(implicit_default, "Not explicitly default");
    }

    #[test]
    fn test_get_version_explicit_default() {
        use crate::read_from_xml_string;

        // NodeDef with version and isdefaultversion="true"
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="test_node" type="surfaceshader" node="shader" version="2.1" isdefaultversion="true"/>
            </materialx>"#;
        let doc = read_from_xml_string(xml).unwrap();
        let nodedef = doc.get_node_def("test_node").unwrap();

        let (version, implicit_default) = get_version(&nodedef);
        assert!(version.is_default());
        assert!(!implicit_default, "Explicitly marked as default");
    }
}
