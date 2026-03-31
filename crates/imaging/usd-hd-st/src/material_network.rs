
//! HdStMaterialNetwork - Material network processing for Storm.
//!
//! Processes a Hydra material network topology into:
//! - Shader source code (fragment, volume, displacement)
//! - Material parameters with fallback values
//! - Texture descriptors for resource allocation
//!
//! Handles both UsdPreviewSurface and MaterialX networks.

use crate::material_param::{FallbackValue, HdStMaterialParam, HdStTextureType};
use crate::texture_identifier::HdStTextureIdentifier;
use std::collections::HashMap;
use usd_sdf::AssetPath;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Texture sampler parameters.
#[derive(Debug, Clone)]
pub struct SamplerParams {
    /// Wrap mode for U axis
    pub wrap_s: WrapMode,
    /// Wrap mode for V axis
    pub wrap_t: WrapMode,
    /// Min filter
    pub min_filter: FilterMode,
    /// Mag filter
    pub mag_filter: FilterMode,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            wrap_s: WrapMode::Repeat,
            wrap_t: WrapMode::Repeat,
            min_filter: FilterMode::LinearMipmapLinear,
            mag_filter: FilterMode::Linear,
        }
    }
}

/// Texture wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    Clamp,
    Repeat,
    Mirror,
    Black,
}

/// Texture filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    LinearMipmapLinear,
}

/// Describes a texture to be allocated for a material.
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    /// Accessor name in shader (HdGet_<name>)
    pub name: Token,
    /// Texture identifier (file path + subtexture)
    pub texture_id: HdStTextureIdentifier,
    /// Texture type (UV, Ptex, UDIM, etc.)
    pub texture_type: HdStTextureType,
    /// Sampler parameters
    pub sampler_params: SamplerParams,
    /// Memory request in bytes (0 = default)
    pub memory_request: usize,
    /// If true, use special API on the texture prim (e.g. draw targets)
    pub use_texture_prim: bool,
    /// Path to the texture prim (for draw targets and hashing)
    pub texture_prim: SdfPath,
}

/// Material node in the network.
#[derive(Debug, Clone)]
pub struct MaterialNode {
    /// Node identifier (shader id like UsdPreviewSurface)
    pub identifier: Token,
    /// Path in the network
    pub path: SdfPath,
    /// Parameter values
    pub parameters: HashMap<Token, FallbackValue>,
    /// Input connections: input_name -> (upstream_node_path, output_name)
    pub input_connections: HashMap<Token, Vec<(SdfPath, Token)>>,
}

/// Processed material network result.
///
/// After `process()`, contains all data needed by Storm to render
/// the material: compiled shader source, parameters, and textures.
pub struct HdStMaterialNetwork {
    /// Material tag for render pass sorting (e.g. "defaultMaterialTag", "translucent")
    material_tag: Token,
    /// Fragment shader source (WGSL for wgpu)
    fragment_source: String,
    /// Volume shader source
    volume_source: String,
    /// Displacement shader source
    displacement_source: String,
    /// Material metadata
    metadata: HashMap<String, String>,
    /// Extracted material parameters
    material_params: Vec<HdStMaterialParam>,
    /// Textures to allocate
    texture_descriptors: Vec<TextureDescriptor>,
}

impl HdStMaterialNetwork {
    pub fn new() -> Self {
        Self {
            material_tag: Token::new("defaultMaterialTag"),
            fragment_source: String::new(),
            volume_source: String::new(),
            displacement_source: String::new(),
            metadata: HashMap::new(),
            material_params: Vec::new(),
            texture_descriptors: Vec::new(),
        }
    }

    /// Process a material network, extracting shader source, params, textures.
    ///
    /// Walks the node graph from the terminal surface node backwards,
    /// collecting parameter values, texture connections, and generating
    /// the appropriate shader code.
    pub fn process(
        &mut self,
        material_id: &SdfPath,
        nodes: &[MaterialNode],
        terminal_node: Option<&SdfPath>,
    ) {
        self.material_params.clear();
        self.texture_descriptors.clear();
        self.fragment_source.clear();

        // Find the terminal (surface) node
        let surface_node = terminal_node
            .and_then(|path| nodes.iter().find(|n| &n.path == path))
            .or_else(|| nodes.last());

        let Some(surface) = surface_node else {
            log::warn!("No surface node found for material {}", material_id);
            return;
        };

        // Determine material tag from surface node
        self.material_tag = self.determine_material_tag(surface);

        // Extract parameters from the surface node
        self.extract_params(surface, nodes);

        // Generate fragment shader source
        self.fragment_source = self.generate_surface_shader(surface, nodes);

        log::debug!(
            "Processed material {} -> {} params, {} textures",
            material_id,
            self.material_params.len(),
            self.texture_descriptors.len()
        );
    }

    /// Get the material tag for render pass sorting.
    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    /// Get generated fragment shader source.
    pub fn get_fragment_code(&self) -> &str {
        &self.fragment_source
    }

    /// Get generated volume shader source.
    pub fn get_volume_code(&self) -> &str {
        &self.volume_source
    }

    /// Get generated displacement shader source.
    pub fn get_displacement_code(&self) -> &str {
        &self.displacement_source
    }

    /// Get metadata.
    pub fn get_metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Get extracted material parameters.
    pub fn get_material_params(&self) -> &[HdStMaterialParam] {
        &self.material_params
    }

    /// Get texture descriptors.
    pub fn get_texture_descriptors(&self) -> &[TextureDescriptor] {
        &self.texture_descriptors
    }

    // --- Private helpers ---

    /// Determine material tag from the surface node.
    fn determine_material_tag(&self, surface: &MaterialNode) -> Token {
        // Check for explicit opacity < 1.0
        if let Some(FallbackValue::Float(opacity)) = surface.parameters.get(&Token::new("opacity"))
        {
            if *opacity < 1.0 {
                return Token::new("translucent");
            }
        }

        // Check for connected opacity
        if surface
            .input_connections
            .contains_key(&Token::new("opacity"))
        {
            return Token::new("translucent");
        }

        Token::new("defaultMaterialTag")
    }

    /// Extract material parameters from a surface node and its inputs.
    fn extract_params(&mut self, surface: &MaterialNode, nodes: &[MaterialNode]) {
        for (input_name, value) in &surface.parameters {
            // Check if this input has a texture connection
            if let Some(connections) = surface.input_connections.get(input_name) {
                for (upstream_path, _output_name) in connections {
                    if let Some(upstream) = nodes.iter().find(|n| &n.path == upstream_path) {
                        if is_texture_node(&upstream.identifier) {
                            self.add_texture_param(input_name, upstream, nodes);
                            continue;
                        }
                        if is_primvar_reader_node(&upstream.identifier) {
                            self.add_primvar_redirect(input_name, upstream, value);
                            continue;
                        }
                    }
                }
            }

            // Fallback parameter
            self.material_params.push(HdStMaterialParam::fallback(
                input_name.clone(),
                value.clone(),
            ));
        }
    }

    /// Add a texture parameter and descriptor.
    ///
    /// We resolve the texcoord primvar name from the authored network instead
    /// of hard-coding `st`. DCC exports routinely use names such as `st1`,
    /// `map1`, or a custom primvar routed through `UsdPrimvarReader_*`, and
    /// collapsing all of those to `st` breaks compatibility across real assets.
    fn add_texture_param(
        &mut self,
        input_name: &Token,
        texture_node: &MaterialNode,
        nodes: &[MaterialNode],
    ) {
        let uv_primvar = resolve_texture_coordinate_primvar(texture_node, nodes);

        self.material_params
            .push(HdStMaterialParam::texture(input_name.clone(), uv_primvar));

        // Get file path from node parameters
        let file_path = texture_node
            .parameters
            .get(&Token::new("file"))
            .and_then(|v| match v {
                FallbackValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        self.texture_descriptors.push(TextureDescriptor {
            name: input_name.clone(),
            texture_id: HdStTextureIdentifier::from_path(AssetPath::new(&file_path)),
            texture_type: HdStTextureType::Uv,
            sampler_params: SamplerParams::default(),
            memory_request: 0,
            use_texture_prim: false,
            texture_prim: SdfPath::empty(),
        });
    }

    /// Add a primvar redirect parameter.
    fn add_primvar_redirect(
        &mut self,
        input_name: &Token,
        primvar_node: &MaterialNode,
        fallback: &FallbackValue,
    ) {
        let primvar_name = primvar_node
            .parameters
            .get(&Token::new("varname"))
            .and_then(|v| match v {
                FallbackValue::String(s) => Some(Token::new(s)),
                _ => None,
            })
            .unwrap_or_else(|| input_name.clone());

        self.material_params
            .push(HdStMaterialParam::primvar_redirect(
                input_name.clone(),
                primvar_name,
                fallback.clone(),
            ));
    }

    /// Generate WGSL surface shader function.
    fn generate_surface_shader(&self, surface: &MaterialNode, _nodes: &[MaterialNode]) -> String {
        use std::fmt::Write;
        let mut src = String::with_capacity(1024);

        let id = surface.identifier.as_str();
        writeln!(src, "// Surface shader: {}", id).unwrap();

        if id == "UsdPreviewSurface" {
            self.gen_preview_surface(&mut src, surface);
        } else {
            // Generic fallback: output diffuse color
            writeln!(
                src,
                "fn surfaceShader(N: vec3<f32>, V: vec3<f32>) -> vec4<f32> {{"
            )
            .unwrap();
            writeln!(src, "    return vec4<f32>(0.18, 0.18, 0.18, 1.0);").unwrap();
            writeln!(src, "}}").unwrap();
        }

        src
    }

    /// Generate WGSL for UsdPreviewSurface.
    fn gen_preview_surface(&self, src: &mut String, _surface: &MaterialNode) {
        use std::fmt::Write;
        writeln!(
            src,
            "fn surfaceShader(N: vec3<f32>, V: vec3<f32>) -> vec4<f32> {{"
        )
        .unwrap();
        writeln!(src, "    let diffuseColor = HdGet_diffuseColor();").unwrap();
        writeln!(src, "    let metallic = HdGet_metallic();").unwrap();
        writeln!(src, "    let roughness = HdGet_roughness();").unwrap();
        writeln!(src, "    let opacity = HdGet_opacity();").unwrap();
        writeln!(src, "    let emissiveColor = HdGet_emissiveColor();").unwrap();
        writeln!(src, "    // PBR evaluation delegated to lighting shader").unwrap();
        writeln!(
            src,
            "    return vec4<f32>(diffuseColor + emissiveColor, opacity);"
        )
        .unwrap();
        writeln!(src, "}}").unwrap();
    }
}

impl Default for HdStMaterialNetwork {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a node identifier is a texture reader.
fn is_texture_node(identifier: &Token) -> bool {
    let id = identifier.as_str();
    id == "UsdUVTexture" || id.contains("Texture") || id.contains("Image")
}

/// Check if a node identifier is a primvar reader.
fn is_primvar_reader_node(identifier: &Token) -> bool {
    let id = identifier.as_str();
    id == "UsdPrimvarReader_float"
        || id == "UsdPrimvarReader_float2"
        || id == "UsdPrimvarReader_float3"
        || id == "UsdPrimvarReader_float4"
        || id == "UsdPrimvarReader_int"
        || id == "UsdPrimvarReader_string"
        || id.starts_with("UsdPrimvarReader")
}

fn token_param(node: &MaterialNode, name: &str) -> Option<Token> {
    node.parameters
        .get(&Token::new(name))
        .and_then(|v| match v {
            FallbackValue::String(s) => Some(Token::new(s)),
            _ => None,
        })
}

fn connected_primvar_name(
    node: &MaterialNode,
    input_name: &str,
    nodes: &[MaterialNode],
) -> Option<Token> {
    node.input_connections
        .get(&Token::new(input_name))
        .and_then(|connections| {
            connections
                .iter()
                .find_map(|(upstream_path, _output_name)| {
                    let upstream = nodes
                        .iter()
                        .find(|candidate| &candidate.path == upstream_path)?;
                    if is_primvar_reader_node(&upstream.identifier) {
                        token_param(upstream, "varname")
                    } else {
                        None
                    }
                })
        })
}

/// Resolve the texture-coordinate primvar that a texture node should sample.
///
/// The fast path is an authored literal `st` parameter on the texture node.
/// When the network instead routes texcoords through `UsdPrimvarReader_*`, we
/// must honor the reader's authored `varname`. Falling back to plain `st` is
/// only correct when the network provided no stronger signal.
fn resolve_texture_coordinate_primvar(
    texture_node: &MaterialNode,
    nodes: &[MaterialNode],
) -> Token {
    token_param(texture_node, "st")
        .or_else(|| connected_primvar_name(texture_node, "st", nodes))
        .or_else(|| token_param(texture_node, "varname"))
        .unwrap_or_else(|| Token::new("st"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_material_network_default() {
        let net = HdStMaterialNetwork::new();
        assert_eq!(net.get_material_tag().as_str(), "defaultMaterialTag");
        assert!(net.get_fragment_code().is_empty());
        assert!(net.get_material_params().is_empty());
        assert!(net.get_texture_descriptors().is_empty());
    }

    #[test]
    fn test_texture_node_detection() {
        assert!(is_texture_node(&Token::new("UsdUVTexture")));
        assert!(is_texture_node(&Token::new("MtlxImage")));
        assert!(!is_texture_node(&Token::new("UsdPreviewSurface")));
    }

    #[test]
    fn test_primvar_reader_detection() {
        assert!(is_primvar_reader_node(&Token::new(
            "UsdPrimvarReader_float3"
        )));
        assert!(!is_primvar_reader_node(&Token::new("UsdUVTexture")));
    }

    #[test]
    fn test_sampler_params_default() {
        let sp = SamplerParams::default();
        assert_eq!(sp.wrap_s, WrapMode::Repeat);
        assert_eq!(sp.mag_filter, FilterMode::Linear);
    }

    #[test]
    fn test_texture_param_uses_connected_primvar_reader_varname() {
        let texture_path = SdfPath::from_string("/Tex").unwrap();
        let reader_path = SdfPath::from_string("/Reader").unwrap();
        let texture_node = MaterialNode {
            identifier: Token::new("UsdUVTexture"),
            path: texture_path.clone(),
            parameters: HashMap::new(),
            input_connections: HashMap::from([(
                Token::new("st"),
                vec![(reader_path.clone(), Token::new("result"))],
            )]),
        };
        let reader_node = MaterialNode {
            identifier: Token::new("UsdPrimvarReader_float2"),
            path: reader_path,
            parameters: HashMap::from([(
                Token::new("varname"),
                FallbackValue::String("st1".to_string()),
            )]),
            input_connections: HashMap::new(),
        };

        assert_eq!(
            resolve_texture_coordinate_primvar(&texture_node, &[texture_node.clone(), reader_node])
                .as_str(),
            "st1"
        );
    }
}
