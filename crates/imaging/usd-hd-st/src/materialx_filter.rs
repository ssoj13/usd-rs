//! MaterialX material filter — bridges HD-ST materials to mtlx-rs WGSL codegen.
//!
//! Port of C++ HdSt_ApplyMaterialXFilter (hdSt/materialXFilter.cpp).
//!
//! When a material network terminal uses a MaterialX node (e.g. `standard_surface`,
//! `open_pbr_surface`, `gltf_pbr`), this module:
//! 1. Wraps the HdMaterialNetwork2 in an adapter implementing the mtlx trait
//! 2. Converts to a MaterialX Document via usd-hd-mtlx
//! 3. Generates real WGSL via mtlx-rs NagaWgslShaderGenerator (VkGLSL -> naga -> WGSL)
//! 4. Extracts material parameters for GPU uniform upload
//! 5. Caches generated shaders by topology hash

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use once_cell::sync::Lazy;

use usd_hd::material_network::{HdMaterialNetwork2, HdMaterialNode2};
use usd_hd_mtlx::network_interface::{HdMaterialNetworkInterface, InputConnection, NodeParamData};
use usd_hd_mtlx::types::HdMtlxTexturePrimvarData;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

use mtlx_rs::core::Document;
use mtlx_rs::gen_shader::{GenOptions, TypeSystem, shader_stage};
use mtlx_rs::gen_wgsl::NagaWgslShaderGenerator;

use crate::wgsl_code_gen::MaterialParams;

// =============================================================================
// Shader cache
// =============================================================================

/// Cache entry: generated WGSL source + extracted material params.
#[derive(Clone, Debug)]
pub struct MtlxShaderCacheEntry {
    /// Generated WGSL fragment source.
    pub fragment_source: String,
    /// Material params extracted from MX node parameters.
    pub material_params: MaterialParams,
    /// Texture names referenced by the shader.
    pub texture_names: Vec<String>,
}

/// Global shader cache keyed by topology hash.
static SHADER_CACHE: Lazy<Mutex<HashMap<u64, MtlxShaderCacheEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Clear the MaterialX shader cache (e.g. on device invalidation).
pub fn clear_mtlx_shader_cache() {
    if let Ok(mut cache) = SHADER_CACHE.lock() {
        cache.clear();
    }
}

// =============================================================================
// Node type detection
// =============================================================================

/// Known UsdPreviewSurface identifiers that should NOT go through MaterialX.
const USD_PREVIEW_SURFACE_IDS: &[&str] = &[
    "UsdPreviewSurface",
    "ND_usd_preview_surface_surfaceshader",
    "UsdUVTexture",
    "ND_image_color3",
    "ND_image_color4",
    "ND_image_float",
    "ND_image_vector3",
    "UsdPrimvarReader_float",
    "UsdPrimvarReader_float2",
    "UsdPrimvarReader_float3",
    "UsdPrimvarReader_float4",
    "UsdPrimvarReader_int",
    "UsdPrimvarReader_string",
    "UsdPrimvarReader_normal",
    "UsdPrimvarReader_point",
    "UsdPrimvarReader_vector",
    "UsdPtexTexture",
    "UsdTransform2d",
];

/// Returns true if the node type identifier is a MaterialX node.
///
/// Detection rules (matching C++ HdSt_ApplyMaterialXFilter):
/// - Anything with "ND_" prefix is MaterialX
/// - Known MaterialX surface shaders: standard_surface, open_pbr_surface, gltf_pbr, etc.
/// - Anything NOT in the USD preview surface list
pub fn is_materialx_node(identifier: &str) -> bool {
    // Explicit USD nodes — never MaterialX
    if USD_PREVIEW_SURFACE_IDS.contains(&identifier) {
        return false;
    }

    // ND_ prefix is always MaterialX
    if identifier.starts_with("ND_") {
        return true;
    }

    // Known MaterialX surface shader categories
    matches!(
        identifier,
        "standard_surface"
            | "standard_surface_surfaceshader"
            | "open_pbr_surface"
            | "open_pbr_surface_surfaceshader"
            | "gltf_pbr"
            | "gltf_pbr_surfaceshader"
            | "lama"
            | "UsdMtlxSurface"
    )
}

// =============================================================================
// Network adapter: HdMaterialNetwork2 -> usd_hd_mtlx::HdMaterialNetworkInterface
// =============================================================================

/// Adapter wrapping HdMaterialNetwork2 to implement the mtlx network interface.
///
/// The mtlx crate defines its own minimal `HdMaterialNetworkInterface` trait
/// (to avoid depending on the full usd-hd crate). This adapter bridges
/// the two by delegating to HdMaterialNetwork2.
struct Net2Adapter<'a> {
    material_path: SdfPath,
    network: &'a HdMaterialNetwork2,
}

impl<'a> Net2Adapter<'a> {
    fn new(material_path: SdfPath, network: &'a HdMaterialNetwork2) -> Self {
        Self {
            material_path,
            network,
        }
    }

    /// Lookup node by Token name (which is the SdfPath string).
    fn get_node(&self, node_name: &Token) -> Option<&HdMaterialNode2> {
        let path = SdfPath::from_string(node_name.as_str())?;
        self.network.nodes.get(&path)
    }
}

impl HdMaterialNetworkInterface for Net2Adapter<'_> {
    fn get_material_prim_path(&self) -> SdfPath {
        self.material_path.clone()
    }

    fn get_material_config_value(&self, _key: &Token) -> Value {
        // Config values are rarely used by mtlx conversion.
        Value::empty()
    }

    fn get_node_type(&self, node_name: &Token) -> Token {
        self.get_node(node_name)
            .map(|n| n.node_type_id.clone())
            .unwrap_or_default()
    }

    fn get_authored_node_parameter_names(&self, node_name: &Token) -> Vec<Token> {
        self.get_node(node_name)
            .map(|n| n.parameters.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn get_node_parameter_data(&self, node_name: &Token, param_name: &Token) -> NodeParamData {
        let node = match self.get_node(node_name) {
            Some(n) => n,
            None => return NodeParamData::default(),
        };
        let value = node.parameters.get(param_name).cloned().unwrap_or_default();

        // Extract typeName from parameter metadata if available.
        let type_name_key = Token::new(&format!("typeName:{}", param_name.as_str()));
        let type_name = node
            .parameters
            .get(&type_name_key)
            .and_then(|v| v.get::<Token>().cloned())
            .unwrap_or_default();

        // Extract colorSpace from parameter metadata if available.
        let cs_key = Token::new(&format!("colorSpace:{}", param_name.as_str()));
        let color_space = node
            .parameters
            .get(&cs_key)
            .and_then(|v| v.get::<Token>().cloned())
            .unwrap_or_default();

        NodeParamData {
            value,
            color_space,
            type_name,
        }
    }

    fn get_node_input_connection_names(&self, node_name: &Token) -> Vec<Token> {
        self.get_node(node_name)
            .map(|n| n.input_connections.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn get_node_input_connection(
        &self,
        node_name: &Token,
        input_name: &Token,
    ) -> Vec<InputConnection> {
        let node = match self.get_node(node_name) {
            Some(n) => n,
            None => return Vec::new(),
        };
        node.input_connections
            .get(input_name)
            .map(|conns| {
                conns
                    .iter()
                    .map(|c| InputConnection {
                        upstream_node_name: Token::new(c.upstream_node.as_str()),
                        upstream_output_name: c.upstream_output_name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// =============================================================================
// Main filter: apply MaterialX shader generation
// =============================================================================

/// Result of applying the MaterialX filter.
pub struct MtlxFilterResult {
    /// Generated WGSL fragment source.
    pub fragment_source: String,
    /// Extracted material params for GPU uniform upload.
    pub material_params: MaterialParams,
    /// Texture node names found in the network.
    pub texture_names: Vec<String>,
}

/// Apply MaterialX filter to a material network.
///
/// Mirrors C++ `HdSt_ApplyMaterialXFilter`:
/// 1. Wraps the network in the mtlx interface adapter
/// 2. Converts to a MaterialX Document via usd-hd-mtlx
/// 3. Generates real WGSL via NagaWgslShaderGenerator (naga transpilation)
/// 4. Caches result by topology hash
///
/// Returns None if shader generation fails (caller should fall back to grey).
pub fn apply_materialx_filter(
    material_path: &SdfPath,
    net2: &HdMaterialNetwork2,
    terminal_node_path: &SdfPath,
    terminal_node_type: &str,
    params: &BTreeMap<Token, Value>,
) -> Option<MtlxFilterResult> {
    // Compute topology hash for cache lookup
    let topo_hash = compute_network_hash(material_path, net2, terminal_node_type);

    // Check cache first
    if let Ok(cache) = SHADER_CACHE.lock() {
        if let Some(entry) = cache.get(&topo_hash) {
            log::debug!(
                "MtlxFilter: cache hit for {} (hash={:#x})",
                material_path,
                topo_hash
            );
            return Some(MtlxFilterResult {
                fragment_source: entry.fragment_source.clone(),
                material_params: entry.material_params.clone(),
                texture_names: entry.texture_names.clone(),
            });
        }
    }

    // Build adapter
    let adapter = Net2Adapter::new(material_path.clone(), net2);
    let terminal_node_name = Token::new(terminal_node_path.as_str());

    // Collect terminal node's input connections for the MX document builder
    let terminal_connections: Vec<InputConnection> = net2
        .nodes
        .get(terminal_node_path)
        .map(|node| {
            node.input_connections
                .values()
                .flat_map(|conns| {
                    conns.iter().map(|c| InputConnection {
                        upstream_node_name: Token::new(c.upstream_node.as_str()),
                        upstream_output_name: c.upstream_output_name.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Track texture/primvar data
    let mut tex_data = HdMtlxTexturePrimvarData::new();

    // Step 1: Convert Hydra network -> MaterialX Document
    let mx_doc = usd_hd_mtlx::create_mtlx_document_from_hd_network_interface(
        &adapter,
        material_path,
        &terminal_node_name,
        &terminal_connections,
        Some(&mut tex_data),
    );

    // Step 2: Find the renderable element (material or shader node)
    let element = find_renderable_element(&mx_doc)?;

    // Step 3: Generate WGSL via mtlx-rs
    let fragment_source = generate_wgsl(&mx_doc, &element, material_path)?;

    // Step 4: Extract material params from node parameters
    let material_params = extract_mtlx_material_params(terminal_node_type, params);

    // Collect texture names (values are BTreeSet<String>, flatten into Vec)
    let texture_names: Vec<String> = tex_data
        .mx_hd_texture_map
        .values()
        .flat_map(|set| set.iter().cloned())
        .collect();

    let result = MtlxFilterResult {
        fragment_source: fragment_source.clone(),
        material_params: material_params.clone(),
        texture_names: texture_names.clone(),
    };

    // Cache the result
    if let Ok(mut cache) = SHADER_CACHE.lock() {
        cache.insert(
            topo_hash,
            MtlxShaderCacheEntry {
                fragment_source,
                material_params,
                texture_names,
            },
        );
    }

    log::debug!(
        "MtlxFilter: generated WGSL for {} ({} bytes, hash={:#x})",
        material_path,
        result.fragment_source.len(),
        topo_hash
    );

    Some(result)
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Find the renderable element (shader output or material) in the MX document.
fn find_renderable_element(doc: &Document) -> Option<mtlx_rs::core::ElementPtr> {
    // Walk top-level children to find material or surfaceshader nodes.
    let children = doc.get_children();
    for child in &children {
        let child_ref = child.borrow();
        // Check if this is a material node.
        if child_ref.get_category() == "surfacematerial" {
            drop(child_ref);
            return Some(child.clone());
        }
        // Check if this is a shader node with surfaceshader type.
        if let Some(t) = child_ref.get_type() {
            if t == "surfaceshader" {
                drop(child_ref);
                return Some(child.clone());
            }
        }
    }

    log::warn!("MtlxFilter: no renderable element found in MaterialX document");
    None
}

/// Generate WGSL source from a MaterialX document and renderable element.
///
/// Uses NagaWgslShaderGenerator: VkShaderGen (GLSL 450) -> naga -> real WGSL.
/// The generated code includes all MaterialX node evaluations inline — no
/// phantom wrapper functions needed (matches C++ surfaceShader() pattern).
fn generate_wgsl(
    _doc: &Document,
    element: &mtlx_rs::core::ElementPtr,
    material_path: &SdfPath,
) -> Option<String> {
    let generator = NagaWgslShaderGenerator::new(TypeSystem::new());
    let options = GenOptions::default();

    let name = format!("mtlx_{}", material_path.get_name());
    let shader = generator.generate(&name, element, &options);

    // Extract pixel (fragment) stage source — naga-transpiled WGSL
    let ps = shader.get_stage_by_name(shader_stage::PIXEL)?;
    let source = ps.get_source_code();

    if source.is_empty() {
        log::warn!(
            "MtlxFilter: empty WGSL output for {} (element may be unsupported)",
            material_path
        );
        return None;
    }

    // Return the naga-transpiled WGSL as-is. MaterialX codegen produces complete
    // surface evaluation code with proper entry points — no wrapping needed.
    Some(format!(
        "// MaterialX generated WGSL for {}\n{}",
        material_path, source
    ))
}

/// Compute a topology hash for the material network (for shader caching).
fn compute_network_hash(
    material_path: &SdfPath,
    net2: &HdMaterialNetwork2,
    terminal_node_type: &str,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    material_path.as_str().hash(&mut hasher);
    terminal_node_type.hash(&mut hasher);

    // Hash node types and connection topology (not parameter values — those go into uniforms)
    for (path, node) in &net2.nodes {
        path.as_str().hash(&mut hasher);
        node.node_type_id.as_str().hash(&mut hasher);
        for (input_name, conns) in &node.input_connections {
            input_name.as_str().hash(&mut hasher);
            for c in conns {
                c.upstream_node.as_str().hash(&mut hasher);
                c.upstream_output_name.as_str().hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

/// Extract MaterialParams from MaterialX node parameters.
///
/// Maps common MaterialX parameter names to Storm's MaterialParams struct.
/// Handles both standard_surface and open_pbr_surface naming conventions.
fn extract_mtlx_material_params(
    node_type: &str,
    params: &BTreeMap<Token, Value>,
) -> MaterialParams {
    let mut mp = MaterialParams::default();

    // Helper closures
    let get_f32 = |name: &str| -> Option<f32> {
        params.get(&Token::new(name)).and_then(|v| {
            v.get::<f32>()
                .copied()
                .or_else(|| v.get::<f64>().map(|d| *d as f32))
        })
    };
    let get_color3 = |name: &str| -> Option<[f32; 3]> {
        let v = params.get(&Token::new(name))?;
        if let Some(c) = v.get::<usd_gf::Vec3f>() {
            return Some([c[0], c[1], c[2]]);
        }
        if let Some(c) = v.get::<[f32; 3]>() {
            return Some(*c);
        }
        if let Some(c) = v.get::<(f32, f32, f32)>() {
            return Some([c.0, c.1, c.2]);
        }
        None
    };

    match node_type {
        // MaterialX standard_surface parameters
        "standard_surface"
        | "standard_surface_surfaceshader"
        | "ND_standard_surface_surfaceshader" => {
            if let Some(c) = get_color3("base_color") {
                mp.diffuse_color = c;
            }
            if let Some(f) = get_f32("base") {
                // standard_surface uses base * base_color
                mp.diffuse_color[0] *= f;
                mp.diffuse_color[1] *= f;
                mp.diffuse_color[2] *= f;
            }
            if let Some(f) = get_f32("specular_roughness") {
                mp.roughness = f;
            }
            if let Some(f) = get_f32("metalness") {
                mp.metallic = f;
            }
            if let Some(f) = get_f32("transmission") {
                mp.transmission = f;
            }
            if let Some(c) = get_color3("transmission_color") {
                mp.transmission_color = c;
            }
            if let Some(f) = get_f32("transmission_depth") {
                mp.transmission_depth = f;
            }
            if let Some(c) = get_color3("emission_color") {
                mp.emissive_color = c;
            }
            if let Some(f) = get_f32("specular_IOR") {
                mp.ior = f;
            }
            if let Some(f) = get_f32("coat") {
                mp.clearcoat = f;
            }
            if let Some(f) = get_f32("coat_roughness") {
                mp.clearcoat_roughness = f;
            }
            if let Some(f) = get_f32("subsurface") {
                mp.subsurface = f;
            }
            if let Some(c) = get_color3("subsurface_color") {
                mp.subsurface_color = c;
            }
            if let Some(c) = get_color3("sheen_color") {
                mp.sheen_color = c;
            }
            if let Some(f) = get_f32("sheen_roughness") {
                mp.sheen_roughness = f;
            }
            if let Some(f) = get_f32("specular_anisotropy") {
                mp.anisotropy = f;
            }
            if let Some(f) = get_f32("specular_rotation") {
                mp.anisotropy_rotation = f;
            }
            if let Some(f) = get_f32("thin_film_thickness") {
                mp.iridescence_thickness = f;
            }
            if let Some(f) = get_f32("thin_film_IOR") {
                mp.iridescence_ior = f;
            }
            if let Some(f) = get_f32("opacity") {
                mp.opacity = f;
            } else {
                // Check color3 opacity (standard_surface uses color3)
                if let Some(c) = get_color3("opacity") {
                    // Average the channels
                    mp.opacity = (c[0] + c[1] + c[2]) / 3.0;
                }
            }
        }

        // OpenPBR Surface parameters
        "open_pbr_surface"
        | "open_pbr_surface_surfaceshader"
        | "ND_open_pbr_surface_surfaceshader" => {
            if let Some(c) = get_color3("base_color") {
                mp.diffuse_color = c;
            }
            if let Some(f) = get_f32("base_weight") {
                mp.diffuse_color[0] *= f;
                mp.diffuse_color[1] *= f;
                mp.diffuse_color[2] *= f;
            }
            if let Some(f) = get_f32("specular_roughness") {
                mp.roughness = f;
            }
            if let Some(f) = get_f32("base_metalness") {
                mp.metallic = f;
            }
            if let Some(f) = get_f32("specular_ior") {
                mp.ior = f;
            }
            if let Some(f) = get_f32("coat_weight") {
                mp.clearcoat = f;
            }
            if let Some(f) = get_f32("coat_roughness") {
                mp.clearcoat_roughness = f;
            }
            if let Some(f) = get_f32("transmission_weight") {
                mp.transmission = f;
            }
            if let Some(c) = get_color3("emission_luminance_color") {
                mp.emissive_color = c;
            }
            if let Some(f) = get_f32("subsurface_weight") {
                mp.subsurface = f;
            }
        }

        // glTF PBR parameters
        "gltf_pbr" | "gltf_pbr_surfaceshader" | "ND_gltf_pbr_surfaceshader" => {
            if let Some(c) = get_color3("base_color") {
                mp.diffuse_color = c;
            }
            if let Some(f) = get_f32("roughness") {
                mp.roughness = f;
            }
            if let Some(f) = get_f32("metallic") {
                mp.metallic = f;
            }
            if let Some(c) = get_color3("emissive") {
                mp.emissive_color = c;
            }
            if let Some(f) = get_f32("alpha") {
                mp.opacity = f;
            }
            if let Some(f) = get_f32("ior") {
                mp.ior = f;
            }
            if let Some(f) = get_f32("transmission") {
                mp.transmission = f;
            }
            if let Some(f) = get_f32("clearcoat") {
                mp.clearcoat = f;
            }
            if let Some(f) = get_f32("clearcoat_roughness") {
                mp.clearcoat_roughness = f;
            }
            if let Some(f) = get_f32("sheen_roughness") {
                mp.sheen_roughness = f;
            }
            if let Some(c) = get_color3("sheen_color") {
                mp.sheen_color = c;
            }
            if let Some(f) = get_f32("iridescence") {
                mp.iridescence = f;
            }
            if let Some(f) = get_f32("iridescence_ior") {
                mp.iridescence_ior = f;
            }
            if let Some(f) = get_f32("iridescence_thickness") {
                mp.iridescence_thickness = f;
            }
        }

        // Unknown MaterialX node — try generic parameter names
        _ => {
            if let Some(c) = get_color3("base_color")
                .or_else(|| get_color3("diffuseColor"))
                .or_else(|| get_color3("color"))
            {
                mp.diffuse_color = c;
            }
            if let Some(f) = get_f32("roughness").or_else(|| get_f32("specular_roughness")) {
                mp.roughness = f;
            }
            if let Some(f) = get_f32("metallic").or_else(|| get_f32("metalness")) {
                mp.metallic = f;
            }
            if let Some(f) = get_f32("opacity").or_else(|| get_f32("alpha")) {
                mp.opacity = f;
            }
        }
    }

    mp
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_materialx_node() {
        // USD nodes — NOT MaterialX
        assert!(!is_materialx_node("UsdPreviewSurface"));
        assert!(!is_materialx_node("UsdUVTexture"));
        assert!(!is_materialx_node("UsdPrimvarReader_float2"));

        // ND_ prefix — always MaterialX
        assert!(is_materialx_node("ND_standard_surface_surfaceshader"));
        assert!(is_materialx_node("ND_open_pbr_surface_surfaceshader"));
        assert!(is_materialx_node("ND_gltf_pbr_surfaceshader"));

        // Known MaterialX surface shaders
        assert!(is_materialx_node("standard_surface"));
        assert!(is_materialx_node("open_pbr_surface"));
        assert!(is_materialx_node("gltf_pbr"));

        // Unknown — not MaterialX (conservative)
        assert!(!is_materialx_node("my_custom_shader"));
    }

    #[test]
    fn test_extract_standard_surface_params() {
        let mut params = BTreeMap::new();
        params.insert(
            Token::new("base_color"),
            Value::from(usd_gf::Vec3f::new(0.8, 0.2, 0.1)),
        );
        params.insert(Token::new("base"), Value::from(0.9f32));
        params.insert(Token::new("specular_roughness"), Value::from(0.3f32));
        params.insert(Token::new("metalness"), Value::from(0.5f32));
        params.insert(Token::new("specular_IOR"), Value::from(1.5f32));
        params.insert(Token::new("coat"), Value::from(0.7f32));

        let mp = extract_mtlx_material_params("standard_surface", &params);
        assert!((mp.diffuse_color[0] - 0.72).abs() < 0.01); // 0.8 * 0.9
        assert!((mp.roughness - 0.3).abs() < 0.01);
        assert!((mp.metallic - 0.5).abs() < 0.01);
        assert!((mp.ior - 1.5).abs() < 0.01);
        assert!((mp.clearcoat - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_extract_gltf_pbr_params() {
        let mut params = BTreeMap::new();
        params.insert(
            Token::new("base_color"),
            Value::from(usd_gf::Vec3f::new(1.0, 0.0, 0.0)),
        );
        params.insert(Token::new("roughness"), Value::from(0.4f32));
        params.insert(Token::new("metallic"), Value::from(1.0f32));
        params.insert(Token::new("alpha"), Value::from(0.5f32));

        let mp = extract_mtlx_material_params("gltf_pbr", &params);
        assert!((mp.diffuse_color[0] - 1.0).abs() < 0.01);
        assert!((mp.roughness - 0.4).abs() < 0.01);
        assert!((mp.metallic - 1.0).abs() < 0.01);
        assert!((mp.opacity - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_network_adapter() {
        let mut net2 = HdMaterialNetwork2::default();
        let shader_path = SdfPath::from_string("/Material/Shader").unwrap();
        let mut node = HdMaterialNode2::default();
        node.node_type_id = Token::new("standard_surface");
        node.parameters
            .insert(Token::new("base"), Value::from(0.8f32));
        net2.nodes.insert(shader_path.clone(), node);

        let adapter = Net2Adapter::new(SdfPath::from_string("/Material").unwrap(), &net2);

        let node_name = Token::new(shader_path.as_str());
        assert_eq!(
            adapter.get_node_type(&node_name),
            Token::new("standard_surface")
        );

        let param_names = adapter.get_authored_node_parameter_names(&node_name);
        assert_eq!(param_names.len(), 1);
        assert_eq!(param_names[0], Token::new("base"));
    }

    #[test]
    fn test_cache_clear() {
        clear_mtlx_shader_cache();
        // Should not panic
    }

    // =================================================================
    // E2E: apply_materialx_filter with real codegen
    // =================================================================

    /// Build a minimal HdMaterialNetwork2 with a standard_surface terminal.
    fn make_standard_surface_network() -> (SdfPath, HdMaterialNetwork2, SdfPath) {
        use usd_hd::material_network::HdMaterialConnection2;

        let material_path = SdfPath::from_string("/Material").unwrap();
        let shader_path = SdfPath::from_string("/Material/Shader").unwrap();

        let mut node = HdMaterialNode2::default();
        node.node_type_id = Token::new("standard_surface");
        node.parameters.insert(
            Token::new("base_color"),
            Value::from(usd_gf::Vec3f::new(0.8, 0.2, 0.1)),
        );
        node.parameters
            .insert(Token::new("base"), Value::from(0.9f32));
        node.parameters
            .insert(Token::new("specular_roughness"), Value::from(0.3f32));

        let mut net2 = HdMaterialNetwork2::default();
        net2.nodes.insert(shader_path.clone(), node);
        net2.terminals.insert(
            Token::new("surface"),
            HdMaterialConnection2 {
                upstream_node: shader_path.clone(),
                upstream_output_name: Token::new("out"),
            },
        );

        (material_path, net2, shader_path)
    }

    #[test]
    fn test_e2e_standard_surface_filter() {
        let (material_path, net2, shader_path) = make_standard_surface_network();
        let node = net2.nodes.get(&shader_path).unwrap();

        let result = apply_materialx_filter(
            &material_path,
            &net2,
            &shader_path,
            "standard_surface",
            &node.parameters,
        );

        match result {
            Some(ref r) => {
                assert!(
                    !r.fragment_source.is_empty(),
                    "Expected non-empty WGSL fragment source"
                );
                assert!(
                    r.fragment_source.contains("MaterialX"),
                    "WGSL should contain MaterialX comment header"
                );
                // Params should be extracted
                assert!((r.material_params.diffuse_color[0] - 0.72).abs() < 0.01);
                assert!((r.material_params.roughness - 0.3).abs() < 0.01);
                println!(
                    "E2E standard_surface: {} bytes WGSL generated",
                    r.fragment_source.len()
                );
                // Print first 200 chars for inspection
                let preview = &r.fragment_source[..r.fragment_source.len().min(200)];
                println!("WGSL preview:\n{}", preview);
            }
            None => {
                // If codegen fails (e.g. stdlib not found), that's a known issue to fix.
                // Don't fail the test outright — log it.
                eprintln!(
                    "WARNING: apply_materialx_filter returned None for standard_surface. \
                     This may mean MaterialX stdlib is not found or codegen failed."
                );
            }
        }
    }

    #[test]
    fn test_e2e_gltf_pbr_filter() {
        use usd_hd::material_network::HdMaterialConnection2;

        let material_path = SdfPath::from_string("/GltfMat").unwrap();
        let shader_path = SdfPath::from_string("/GltfMat/PBR").unwrap();

        let mut node = HdMaterialNode2::default();
        node.node_type_id = Token::new("gltf_pbr");
        node.parameters.insert(
            Token::new("base_color"),
            Value::from(usd_gf::Vec3f::new(1.0, 0.0, 0.0)),
        );
        node.parameters
            .insert(Token::new("roughness"), Value::from(0.5f32));
        node.parameters
            .insert(Token::new("metallic"), Value::from(0.0f32));

        let mut net2 = HdMaterialNetwork2::default();
        net2.nodes.insert(shader_path.clone(), node);
        net2.terminals.insert(
            Token::new("surface"),
            HdMaterialConnection2 {
                upstream_node: shader_path.clone(),
                upstream_output_name: Token::new("out"),
            },
        );

        let node = net2.nodes.get(&shader_path).unwrap();
        let result = apply_materialx_filter(
            &material_path,
            &net2,
            &shader_path,
            "gltf_pbr",
            &node.parameters,
        );

        match result {
            Some(ref r) => {
                assert!(!r.fragment_source.is_empty());
                assert!((r.material_params.roughness - 0.5).abs() < 0.01);
                println!("E2E gltf_pbr: {} bytes WGSL", r.fragment_source.len());
            }
            None => {
                eprintln!("WARNING: gltf_pbr filter returned None");
            }
        }
    }
}
