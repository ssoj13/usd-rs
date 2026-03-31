
//! HdStMaterial - Storm material implementation.
//!
//! Handles material/shader compilation and parameter binding.
//! Processes both V1 (HdMaterialNetworkMap) and V2 (HdMaterialNetwork2) formats.

use crate::material_network_shader::{MaterialNetworkShader, ShaderParam};
use crate::wgsl_code_gen::MaterialParams;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::material_network::{
    HdMaterialDirtyBits, HdMaterialNetwork2, HdMaterialNetworkMap, hd_convert_to_material_network2,
};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// =============================================================================
// Texture descriptor types
// =============================================================================

/// Texture type classification per C++ HdStTextureType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureType {
    /// Standard 2D UV texture.
    Uv,
    /// Ptex per-face texture (requires face-varying topology).
    Ptex,
    /// Volume field texture (3D).
    Field,
    /// UDIM tiled texture set.
    Udim,
}

impl Default for TextureType {
    fn default() -> Self {
        Self::Uv
    }
}

/// Sampler parameters for texture filtering and wrapping.
/// Mirrors C++ HdSamplerParameters.
#[derive(Debug, Clone, PartialEq)]
pub struct SamplerParams {
    /// Wrap mode for S (U) axis: "clamp", "repeat", "mirror", "black".
    pub wrap_s: Token,
    /// Wrap mode for T (V) axis.
    pub wrap_t: Token,
    /// Wrap mode for R (W) axis (3D textures).
    pub wrap_r: Token,
    /// Minification filter: "nearest", "linear".
    pub min_filter: Token,
    /// Magnification filter: "nearest", "linear".
    pub mag_filter: Token,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            wrap_s: Token::new("useMetadata"),
            wrap_t: Token::new("useMetadata"),
            wrap_r: Token::new("useMetadata"),
            min_filter: Token::new("linear"),
            mag_filter: Token::new("linear"),
        }
    }
}

/// Describes a texture resource extracted from a material network node.
/// Mirrors C++ HdStMaterialNetwork::TextureDescriptor.
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    /// Input name on the surface shader this texture feeds into
    /// (e.g. "diffuseColor", "normal", "roughness").
    pub name: Token,
    /// Resolved asset path to the texture file.
    pub file_path: String,
    /// Type of texture (UV, Ptex, Field, UDIM).
    pub texture_type: TextureType,
    /// Sampler parameters extracted from the texture node.
    pub sampler_params: SamplerParams,
}

/// Volume material data: shader source and params for volume rendering.
/// Mirrors C++ HdStMaterial::VolumeMaterialData.
#[derive(Debug, Clone, Default)]
pub struct VolumeMaterialData {
    /// Volume shader source (WGSL).
    pub source: String,
    /// Material params relevant for volumes.
    pub params: HashMap<Token, Value>,
}

// =============================================================================
// HdStMaterial
// =============================================================================

/// Storm material representation.
///
/// Manages material networks including:
/// - Shader compilation for surface, displacement, and volume terminals
/// - Parameter bindings (PBR params, textures)
/// - Dirty bit tracking per C++ HdStMaterial::Sync
/// - Ptex and limit surface evaluation flags
///
/// # Material Networks
///
/// Materials are represented as networks of shader nodes.
/// Storm compiles these networks into optimized GPU shaders (WGSL).
///
/// # Render Contexts
///
/// Storm supports multiple render contexts:
/// - glslfx - Storm's native shader format
/// - mtlx - MaterialX networks (converted to HdMaterialNetwork2 via hd_convert_to_material_network2;
///           full HdSt_ApplyMaterialXFilter pass not ported — uses UsdPreviewSurface fallback)
#[derive(Debug)]
pub struct HdStMaterial {
    /// Prim path
    path: SdfPath,

    /// Material tag (for bucketing: "defaultMaterialTag", "translucent", "masked", "additive")
    tag: Token,

    /// Shader parameters from network terminal node
    params: HashMap<Token, Value>,

    /// Compiled shader handle (placeholder - would be GPU handle)
    shader_handle: u64,

    /// Material network is dirty
    network_dirty: bool,

    /// Resolved fragment shader source (WGSL)
    fragment_source: String,

    /// Resolved displacement shader source (WGSL)
    displacement_source: String,

    /// Resolved volume shader source (WGSL)
    volume_source: String,

    /// Whether this material has been initialized at least once
    is_initialized: bool,

    /// Whether using fallback shader (no material network provided)
    using_fallback: bool,

    /// Extracted GPU material params (diffuseColor, roughness, etc.)
    material_params: MaterialParams,

    /// Texture file paths resolved from UsdUVTexture nodes.
    /// Key = USD input name on the surface shader (e.g. "diffuseColor", "normal").
    /// Value = asset path string (relative or absolute).
    texture_paths: HashMap<Token, String>,

    /// Texture descriptors with full sampler info extracted from network.
    texture_descriptors: Vec<TextureDescriptor>,

    /// Whether any texture in this material is Ptex.
    /// Cached during sync per C++ _hasPtex.
    has_ptex: bool,

    /// Whether this material requires limit surface evaluation.
    /// Derived from material metadata per C++ _hasLimitSurfaceEvaluation.
    has_limit_surface_eval: bool,

    /// Whether displacement terminal is present.
    has_displacement: bool,

    /// Texture hash for batch invalidation tracking.
    /// Changes when texture bindings change, triggering re-batching.
    texture_hash: u64,

    /// Volume material data (source + params for volume rendering).
    volume_material_data: VolumeMaterialData,

    /// Material metadata dictionary (limitSurfaceEvaluation, etc.)
    material_metadata: HashMap<String, Value>,

    /// Compiled material network shader object.
    /// Owned by HdStMaterial; shared with draw items and draw batches.
    /// Mirrors C++ HdStMaterial::_materialNetworkShader.
    network_shader: Arc<Mutex<MaterialNetworkShader>>,
}

impl HdStMaterial {
    /// Create a new Storm material.
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            tag: Token::new("defaultMaterialTag"),
            params: HashMap::new(),
            shader_handle: 0,
            network_dirty: true,
            fragment_source: String::new(),
            displacement_source: String::new(),
            volume_source: String::new(),
            is_initialized: false,
            using_fallback: false,
            material_params: MaterialParams::default(),
            texture_paths: HashMap::new(),
            texture_descriptors: Vec::new(),
            has_ptex: false,
            has_limit_surface_eval: false,
            has_displacement: false,
            texture_hash: 0,
            volume_material_data: VolumeMaterialData::default(),
            material_metadata: HashMap::new(),
            network_shader: Arc::new(Mutex::new(MaterialNetworkShader::new())),
        }
    }

    /// Get prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Set material tag.
    pub fn set_tag(&mut self, tag: Token) {
        self.tag = tag;
    }

    /// Get material tag.
    pub fn get_tag(&self) -> &Token {
        &self.tag
    }

    /// Set a shader parameter.
    pub fn set_param(&mut self, name: Token, value: Value) {
        self.params.insert(name, value);
    }

    /// Get a shader parameter.
    pub fn get_param(&self, name: &Token) -> Option<&Value> {
        self.params.get(name)
    }

    /// Get all parameters.
    pub fn get_params(&self) -> &HashMap<Token, Value> {
        &self.params
    }

    /// Check if shader is compiled.
    pub fn is_compiled(&self) -> bool {
        self.shader_handle != 0
    }

    /// Get shader handle.
    pub fn get_shader_handle(&self) -> u64 {
        self.shader_handle
    }

    /// Mark network as dirty.
    pub fn mark_network_dirty(&mut self) {
        self.network_dirty = true;
        self.shader_handle = 0; // Invalidate compiled shader
    }

    /// Check if network is dirty.
    pub fn is_network_dirty(&self) -> bool {
        self.network_dirty
    }

    /// Sync the material from scene delegate.
    ///
    /// Reads material network via `get_material_resource()`, converts V1->V2 if needed,
    /// processes surface/displacement/volume terminals, extracts texture descriptors
    /// and PBR parameters.
    ///
    /// Mirrors C++ HdStMaterial::Sync:
    /// - Early-out when neither DirtyResource nor DirtyParams are set
    /// - Pulls VtValue-wrapped HdMaterialNetworkMap from delegate
    /// - Converts to HdMaterialNetwork2 for unified processing
    /// - Extracts fragment/displacement/volume source and parameters
    /// - Sets ptex and limit surface evaluation flags
    pub fn sync_from_delegate(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        dirty_bits: &mut HdDirtyBits,
    ) {
        let bits = *dirty_bits;

        // Early-out per C++: skip when neither DirtyResource nor DirtyParams.
        // Also skip if already initialized and clean.
        if bits == 0 && self.is_initialized {
            return;
        }
        if bits != 0
            && (bits & HdMaterialDirtyBits::DIRTY_RESOURCE) == 0
            && (bits & HdMaterialDirtyBits::DIRTY_PARAMS) == 0
        {
            *dirty_bits = HdRprimDirtyBits::CLEAN;
            return;
        }

        // Pull VtValue-wrapped material network from scene delegate.
        let mat_resource = delegate.get_material_resource(&self.path);

        let mut fragment_source = String::new();
        let mut displacement_source = String::new();
        let mut volume_source = String::new();
        let mut material_tag = self.tag.clone();
        let mut texture_descs: Vec<TextureDescriptor> = Vec::new();
        let mut found_ptex = false;

        // Attempt to downcast to HdMaterialNetworkMap (v1 format).
        // C++: if (vtMat.IsHolding<HdMaterialNetworkMap>())
        if let Some(network_map) = mat_resource.get::<HdMaterialNetworkMap>() {
            if !network_map.terminals.is_empty() && !network_map.map.is_empty() {
                // Convert V1 -> V2 for unified processing path.
                let (net2, _is_volume) = hd_convert_to_material_network2(network_map);

                // Process surface terminal
                let surface_key = Token::new("surface");
                if let Some(conn) = net2.terminals.get(&surface_key) {
                    if let Some(node) = net2.nodes.get(&conn.upstream_node) {
                        let id = node.node_type_id.as_str();

                        // Try MaterialX path first for non-USD nodes
                        #[cfg(feature = "mtlx")]
                        let mtlx_result = {
                            if crate::materialx_filter::is_materialx_node(id) {
                                crate::materialx_filter::apply_materialx_filter(
                                    &self.path,
                                    &net2,
                                    &conn.upstream_node,
                                    id,
                                    &node.parameters,
                                )
                            } else {
                                None
                            }
                        };
                        #[cfg(not(feature = "mtlx"))]
                        let _mtlx_result: Option<()> = None;

                        #[cfg(feature = "mtlx")]
                        if let Some(ref mx_result) = mtlx_result {
                            fragment_source = mx_result.fragment_source.clone();
                            self.material_params = mx_result.material_params.clone();
                            log::debug!(
                                "HdStMaterial: MaterialX shader '{}' -> {} bytes WGSL",
                                id,
                                fragment_source.len()
                            );
                        }

                        // Fall back to built-in processing for USD nodes or MX failure
                        if fragment_source.is_empty() {
                            fragment_source = self.process_surface_node(id, &node.parameters);
                            self.material_params = extract_material_params(&node.parameters);
                        }

                        // Extract material tag from opacity
                        if let Some(opacity) = node.parameters.get(&Token::new("opacity")) {
                            if let Some(op) = value_to_f32(opacity) {
                                if op < 1.0 {
                                    material_tag = Token::new("translucent");
                                }
                            }
                        }

                        // Copy parameters for GPU upload
                        for (k, v) in &node.parameters {
                            self.params.insert(k.clone(), v.clone());
                        }

                        log::debug!(
                            "HdStMaterial: processed surface node '{}' -> {} bytes fragment",
                            id,
                            fragment_source.len()
                        );
                    }
                }

                // Process displacement terminal.
                // C++ HdStMaterial::Sync writes the glslfx displacement source here;
                // in our WGSL pipeline we generate a compute_displacement() function
                // that reads the "displacement" scalar param from the terminal node.
                let disp_key = Token::new("displacement");
                if let Some(conn) = net2.terminals.get(&disp_key) {
                    if let Some(node) = net2.nodes.get(&conn.upstream_node) {
                        let disp_scale = node
                            .parameters
                            .get(&Token::new("displacement"))
                            .and_then(|v| v.get::<f32>().copied())
                            .unwrap_or(0.0);
                        // Emit a WGSL displacement function.
                        // The actual per-vertex offset is applied by the VS via
                        // a separate displacement pass; this source is stored and
                        // forwarded to the MaterialNetworkShader.
                        displacement_source = format!(
                            concat!(
                                "// Displacement terminal: {node_type}\n",
                                "fn compute_displacement(in_pos: vec3<f32>, in_normal: vec3<f32>) -> vec3<f32> {{\n",
                                "    let disp_scale: f32 = {scale:.6};\n",
                                "    return in_pos + in_normal * disp_scale;\n",
                                "}}\n"
                            ),
                            node_type = node.node_type_id.as_str(),
                            scale = disp_scale,
                        );
                    }
                }

                // Process volume terminal
                let volume_key = Token::new("volume");
                if let Some(conn) = net2.terminals.get(&volume_key) {
                    if let Some(node) = net2.nodes.get(&conn.upstream_node) {
                        volume_source =
                            format!("// volume terminal: {}\n", node.node_type_id.as_str());
                    }
                }

                // Extract texture descriptors from all network nodes.
                // Walk upstream from surface terminal to find UsdUVTexture nodes.
                self.extract_texture_descriptors_from_net2(
                    &net2,
                    &mut texture_descs,
                    &mut found_ptex,
                );

                // Also populate legacy texture_paths from descriptors
                self.texture_paths.clear();
                for desc in &texture_descs {
                    if !desc.file_path.is_empty() {
                        self.texture_paths
                            .insert(desc.name.clone(), desc.file_path.clone());
                    }
                }
            }
        }

        // Fallback: use grey surface when no fragment source was produced.
        // Mirrors C++: if (fragmentSource.empty() && displacementSource.empty()) InitFallback()
        if fragment_source.is_empty() && displacement_source.is_empty() {
            self.install_fallback_shader();
        } else {
            self.fragment_source = fragment_source;
            self.using_fallback = false;
        }

        // Update displacement
        self.displacement_source = displacement_source;
        self.has_displacement = !self.displacement_source.is_empty();

        // Update volume material data per C++
        if self.volume_material_data.source != volume_source {
            self.volume_material_data.source = volume_source.clone();
            if volume_source.is_empty() {
                self.volume_material_data.params.clear();
            } else {
                self.volume_material_data.params = self.params.clone();
            }
        }
        self.volume_source = volume_source;

        // Update material tag
        self.tag = material_tag;

        // Update ptex flag
        self.has_ptex = found_ptex;

        // Update limit surface evaluation flag from metadata.
        // C++ checks metadata dict for "limitSurfaceEvaluation" key.
        self.has_limit_surface_eval = self
            .material_metadata
            .get("limitSurfaceEvaluation")
            .map(value_is_truthy)
            .unwrap_or(false);

        // Update texture descriptors and hash
        let new_hash = compute_texture_hash(&texture_descs);
        if self.texture_hash != new_hash {
            self.texture_hash = new_hash;
            // Would trigger batch invalidation in full pipeline
        }
        self.texture_descriptors = texture_descs;

        // Finalize sync state
        self.shader_handle = if self.fragment_source.is_empty() {
            0
        } else {
            1
        };
        self.network_dirty = false;
        self.is_initialized = true;

        // --- Populate MaterialNetworkShader ---
        // Mirrors C++ HdStMaterial::Sync populating _materialNetworkShader.
        // Build ShaderParam list from extracted texture descriptors + scalar params.
        {
            let mut ns = self.network_shader.lock().unwrap();

            ns.set_fragment_source(self.fragment_source.clone());
            ns.set_displacement_source(self.displacement_source.clone());
            ns.set_material_tag(self.tag.clone());
            ns.set_primvar_filtering_enabled(true);
            ns.set_material_params(self.material_params.clone());

            // Build typed ShaderParam list:
            //  - One Fallback param per scalar material param
            //  - One Texture param per texture descriptor
            let mut shader_params: Vec<ShaderParam> = Vec::new();
            for (name, value) in &self.params {
                shader_params.push(ShaderParam::new_fallback(name.clone(), value.clone()));
            }
            for desc in &self.texture_descriptors {
                shader_params.push(ShaderParam::new_texture(
                    desc.name.clone(),
                    Value::default(),
                    vec![Token::new("st")], // default UV coord primvar
                ));
            }
            ns.set_params(shader_params);
        }

        log::debug!(
            "HdStMaterial::sync_from_delegate {}: tag={}, fragment_len={}, disp_len={}, vol_len={}, fallback={}, ptex={}, limit_surf={}",
            self.path,
            self.tag,
            self.fragment_source.len(),
            self.displacement_source.len(),
            self.volume_source.len(),
            self.using_fallback,
            self.has_ptex,
            self.has_limit_surface_eval,
        );

        *dirty_bits = HdRprimDirtyBits::CLEAN;
    }

    /// Extract texture descriptors from a V2 material network.
    ///
    /// Walks all nodes looking for UsdUVTexture/UsdPrimvarReader types,
    /// extracts file paths and sampler params, and detects ptex textures.
    fn extract_texture_descriptors_from_net2(
        &mut self,
        net2: &HdMaterialNetwork2,
        descs: &mut Vec<TextureDescriptor>,
        found_ptex: &mut bool,
    ) {
        // Find surface terminal node to map input connections -> texture nodes
        let surface_key = Token::new("surface");
        let surface_conn = net2.terminals.get(&surface_key);

        // Map from upstream node path -> surface input name
        let mut upstream_to_input: HashMap<SdfPath, Token> = HashMap::new();
        if let Some(conn) = surface_conn {
            if let Some(surface_node) = net2.nodes.get(&conn.upstream_node) {
                for (input_name, connections) in &surface_node.input_connections {
                    for c in connections {
                        upstream_to_input.insert(c.upstream_node.clone(), input_name.clone());
                    }
                }
            }
        }

        // Walk all nodes to find texture nodes
        for (node_path, node) in &net2.nodes {
            let id = node.node_type_id.as_str();
            if !is_texture_node_type(id) {
                continue;
            }

            // Determine which surface input this texture feeds
            let input_name = upstream_to_input
                .get(node_path)
                .cloned()
                .unwrap_or_else(|| Token::new("unknown"));

            // Extract file path from "file" parameter
            let file_path = extract_file_path(&node.parameters);

            // Detect texture type
            let tex_type = detect_texture_type(id, &file_path);
            if tex_type == TextureType::Ptex {
                *found_ptex = true;
            }

            // Extract sampler parameters
            let sampler = extract_sampler_params(&node.parameters);

            descs.push(TextureDescriptor {
                name: input_name,
                file_path,
                texture_type: tex_type,
                sampler_params: sampler,
            });
        }
    }

    /// Get resolved fragment shader source.
    pub fn get_fragment_source(&self) -> &str {
        &self.fragment_source
    }

    /// Get resolved displacement shader source.
    pub fn get_displacement_source(&self) -> &str {
        &self.displacement_source
    }

    /// Get resolved volume shader source.
    pub fn get_volume_source(&self) -> &str {
        &self.volume_source
    }

    /// Whether using fallback shader (no material network resolved).
    pub fn is_using_fallback(&self) -> bool {
        self.using_fallback
    }

    /// Get volume material data (source + params for volume rendering).
    /// Mirrors C++ HdStMaterial::GetVolumeMaterialData.
    pub fn get_volume_material_data(&self) -> &VolumeMaterialData {
        &self.volume_material_data
    }

    /// Get texture descriptors extracted from the material network.
    pub fn get_texture_descriptors(&self) -> &[TextureDescriptor] {
        &self.texture_descriptors
    }

    /// Get texture hash for batch invalidation tracking.
    pub fn get_texture_hash(&self) -> u64 {
        self.texture_hash
    }

    /// Get material metadata dictionary.
    pub fn get_material_metadata(&self) -> &HashMap<String, Value> {
        &self.material_metadata
    }

    /// Process a single surface shader node and generate WGSL fragment source.
    ///
    /// Maps known USD preview surface identifiers (UsdPreviewSurface, etc.) to
    /// embedded WGSL. Falls back to constant grey for unknown types.
    /// Mirrors C++ HdStMaterialNetwork::ProcessMaterialNetwork + GetFragmentCode.
    fn process_surface_node(
        &self,
        identifier: &str,
        params: &std::collections::BTreeMap<Token, Value>,
    ) -> String {
        // Extract common PBR parameters with defaults
        let base_color = params
            .get(&Token::new("diffuseColor"))
            .or_else(|| params.get(&Token::new("baseColor")));
        let (r, g, b) = if let Some(cv) = base_color {
            if let Some(v) = cv.get::<[f32; 3]>() {
                (v[0], v[1], v[2])
            } else if let Some(v) = cv.get::<(f32, f32, f32)>() {
                (v.0, v.1, v.2)
            } else {
                (0.18, 0.18, 0.18)
            }
        } else {
            (0.18, 0.18, 0.18)
        };

        let roughness = params
            .get(&Token::new("roughness"))
            .and_then(|v| v.get::<f32>().copied())
            .unwrap_or(0.5);
        let metallic = params
            .get(&Token::new("metallic"))
            .and_then(|v| v.get::<f32>().copied())
            .unwrap_or(0.0);
        let opacity = params
            .get(&Token::new("opacity"))
            .and_then(|v| v.get::<f32>().copied())
            .unwrap_or(1.0);

        match identifier {
            // UsdPreviewSurface -- the standard USD surface shader
            "UsdPreviewSurface" | "ND_usd_preview_surface_surfaceshader" => {
                format!(
                    r#"// UsdPreviewSurface: diffuse={:.3},{:.3},{:.3} rough={:.3} metal={:.3}
fn compute_surface(in_pos: vec3<f32>, in_normal: vec3<f32>) -> vec4<f32> {{
    let base_color = vec3<f32>({:.3}, {:.3}, {:.3});
    let roughness: f32 = {:.3};
    let metallic: f32 = {:.3};
    let n = normalize(in_normal);
    let l = normalize(vec3<f32>(0.577, 0.577, 0.577));
    let ndotl = max(dot(n, l), 0.0);
    let diffuse = base_color * ndotl;
    let ambient = base_color * 0.05;
    let color = mix(diffuse + ambient, base_color, metallic);
    return vec4<f32>(color, {:.3});
}}
"#,
                    r, g, b, roughness, metallic, r, g, b, roughness, metallic, opacity
                )
            }
            // UsdUVTexture / MaterialX image node used directly as surface terminal.
            // This is uncommon (normally UsdPreviewSurface is the terminal), but
            // when it occurs we emit real WGSL texture sampling code.
            // Binding slot 0 is the convention used by our bind-group layout
            // (first texture = slot 0, sampler = slot 0).
            "UsdUVTexture" | "ND_image_color3" | "ND_image_color4" => {
                // Apply basic Lambertian shading on top of sampled colour.
                // wrap_mode and scale are baked into the sampler/texture at bind time;
                // here we just emit the sampling + lighting skeleton.
                format!(
                    r#"// Textured surface: UsdUVTexture terminal
@group(2) @binding(0) var t_diffuse: texture_2d<f32>;
@group(2) @binding(1) var s_diffuse: sampler;
fn compute_surface(in_pos: vec3<f32>, in_normal: vec3<f32>, in_uv: vec2<f32>) -> vec4<f32> {{
    let tex_color = textureSample(t_diffuse, s_diffuse, in_uv);
    let n = normalize(in_normal);
    let l = normalize(vec3<f32>(0.577, 0.577, 0.577));
    let ndotl = max(dot(n, l), 0.0);
    let diffuse = tex_color.rgb * (ndotl + 0.05);
    return vec4<f32>(diffuse, tex_color.a * {:.3});
}}
"#,
                    opacity
                )
            }
            _ => {
                // Unknown identifier -- generate grey constant to avoid empty source
                log::debug!(
                    "HdStMaterial: unknown surface identifier '{}', using grey",
                    identifier
                );
                format!(
                    r#"// Unknown surface '{}': fallback grey
fn compute_surface(in_pos: vec3<f32>, in_normal: vec3<f32>) -> vec4<f32> {{
    return vec4<f32>(0.18, 0.18, 0.18, 1.0);
}}
"#,
                    identifier
                )
            }
        }
    }

    /// Get extracted GPU material params (diffuseColor, roughness, metallic, etc.).
    pub fn get_material_params(&self) -> &MaterialParams {
        &self.material_params
    }

    /// Get texture file paths resolved from UsdUVTexture nodes.
    ///
    /// Keys are USD input names on the surface shader (e.g. "diffuseColor", "normal").
    /// Values are raw asset path strings to be resolved and loaded.
    pub fn get_texture_paths(&self) -> &HashMap<Token, String> {
        &self.texture_paths
    }

    fn install_fallback_shader(&mut self) {
        // Minimal WGSL fallback surface: constant grey diffuse.
        // Uses VertexOutput (the actual codegen struct name), not FragInput. (P1-9)
        self.fragment_source = r#"
// Fallback surface: constant grey
fn compute_surface(in: VertexOutput) -> vec4<f32> {
    return vec4<f32>(0.18, 0.18, 0.18, 1.0);
}
"#
        .to_owned();
        self.tag = Token::new("defaultMaterialTag");
        self.using_fallback = true;
    }

    /// Sync the material.
    ///
    /// Compiles shaders if the material network has changed.
    pub fn sync(&mut self) {
        if !self.network_dirty {
            return;
        }

        // Note: Full implementation requires HdSt_MaterialNetworkShader.
        // Would: pull network from scene delegate, compile GLSL/MSL, extract uniforms.
        // Placeholder: marks sync complete with dummy shader handle.
        self.shader_handle = 1;
        self.network_dirty = false;
    }

    /// Bind material parameters to GPU.
    ///
    /// This would update GPU uniform buffers with current parameter values.
    pub fn bind(&self) {
        // Note: Requires Hgi for uniform buffer binding and texture sampler setup.
        // No-op until shader compilation pipeline is complete.
    }

    /// Finalize material resources on destruction.
    ///
    /// Port of C++ HdStMaterial::Finalize (P1-26).
    /// Called when the material prim is removed from the render index.
    /// Releases any GPU resources (shader programs, texture handles, samplers).
    pub fn finalize(&mut self) {
        // Release compiled shader
        self.shader_handle = 0;
        self.fragment_source.clear();
        self.displacement_source.clear();
        self.volume_source.clear();
        self.params.clear();
        self.texture_paths.clear();
        self.texture_descriptors.clear();
        self.network_dirty = true;
        self.is_initialized = false;
        self.using_fallback = false;
        self.has_ptex = false;
        self.has_limit_surface_eval = false;
        self.has_displacement = false;
        self.texture_hash = 0;
        self.volume_material_data = VolumeMaterialData::default();
        self.material_metadata.clear();
        // Reset the shared network shader to a clean empty instance
        *self.network_shader.lock().unwrap() = MaterialNetworkShader::new();
        log::debug!("HdStMaterial::finalize: {}", self.path);
    }

    /// Returns true when the material has Ptex texture nodes.
    ///
    /// Port of C++ HdStMaterial::HasPtex.
    /// Uses cached flag set during sync (not recomputed each call).
    pub fn has_ptex(&self) -> bool {
        self.has_ptex
    }

    /// Returns true when the material requires limit surface evaluation.
    ///
    /// Port of C++ HdStMaterial::HasLimitSurfaceEvaluation.
    /// Derived from material metadata "limitSurfaceEvaluation" key during sync.
    pub fn has_limit_surface_evaluation(&self) -> bool {
        self.has_limit_surface_eval
    }

    /// Returns true when the material has a displacement shader.
    ///
    /// Port of C++ HdStMaterial::HasDisplacement.
    /// Controls whether the displacement pass is active for this material.
    pub fn has_displacement(&self) -> bool {
        self.has_displacement
    }

    /// Returns the initial dirty bit mask for first sync.
    /// Mirrors C++ HdStMaterial::GetInitialDirtyBitsMask -> AllDirty.
    pub fn get_initial_dirty_bits_mask() -> HdDirtyBits {
        HdMaterialDirtyBits::ALL_DIRTY
    }

    // ------------------------------------------------------------------
    // MaterialNetworkShader access
    // ------------------------------------------------------------------

    /// Get shared reference to the compiled MaterialNetworkShader.
    ///
    /// Mirrors C++ HdStMaterial::GetMaterialNetworkShader.
    /// Draw items and draw batches hold clones of this Arc to access
    /// GPU params, texture handles, and hash values without borrowing
    /// the material directly.
    pub fn get_network_shader(&self) -> Arc<Mutex<MaterialNetworkShader>> {
        Arc::clone(&self.network_shader)
    }

    /// Replace the MaterialNetworkShader (e.g. from a scene index override).
    ///
    /// Mirrors C++ HdStMaterial::SetMaterialNetworkShader.
    pub fn set_network_shader(&mut self, shader: Arc<Mutex<MaterialNetworkShader>>) {
        self.network_shader = shader;
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Extract MaterialParams from a material node's parameter map.
///
/// Handles both f32 and f64 values (USDC stores float params as f64).
/// Returns default MaterialParams if params are empty/unparseable.
fn extract_material_params(params: &std::collections::BTreeMap<Token, Value>) -> MaterialParams {
    let mut mp = MaterialParams::default();

    // diffuseColor (color3f or [f32;3] or f64 components)
    if let Some(cv) = params
        .get(&Token::new("diffuseColor"))
        .or_else(|| params.get(&Token::new("baseColor")))
    {
        if let Some(c) = value_to_f3(cv) {
            mp.diffuse_color = c;
        }
    }

    // roughness
    if let Some(v) = params.get(&Token::new("roughness")) {
        if let Some(f) = value_to_f32(v) {
            mp.roughness = f;
        }
    }

    // metallic
    if let Some(v) = params.get(&Token::new("metallic")) {
        if let Some(f) = value_to_f32(v) {
            mp.metallic = f;
        }
    }

    // opacity
    if let Some(v) = params.get(&Token::new("opacity")) {
        if let Some(f) = value_to_f32(v) {
            mp.opacity = f;
        }
    }

    // emissiveColor
    if let Some(cv) = params.get(&Token::new("emissiveColor")) {
        if let Some(c) = value_to_f3(cv) {
            mp.emissive_color = c;
        }
    }

    // ior
    if let Some(v) = params.get(&Token::new("ior")) {
        if let Some(f) = value_to_f32(v) {
            mp.ior = f;
        }
    }

    // clearcoat
    if let Some(v) = params.get(&Token::new("clearcoat")) {
        if let Some(f) = value_to_f32(v) {
            mp.clearcoat = f;
        }
    }

    // clearcoatRoughness
    if let Some(v) = params.get(&Token::new("clearcoatRoughness")) {
        if let Some(f) = value_to_f32(v) {
            mp.clearcoat_roughness = f;
        }
    }

    // useSpecularWorkflow (int param: 0 or 1)
    if let Some(v) = params.get(&Token::new("useSpecularWorkflow")) {
        if let Some(f) = value_to_f32(v) {
            mp.use_specular_workflow = f > 0.5;
        }
    }

    // specularColor
    if let Some(cv) = params.get(&Token::new("specularColor")) {
        if let Some(c) = value_to_f3(cv) {
            mp.specular_color = c;
        }
    }

    // displacement
    if let Some(v) = params.get(&Token::new("displacement")) {
        if let Some(f) = value_to_f32(v) {
            mp.displacement = f;
        }
    }

    // --- Advanced PBR ---

    // subsurface
    if let Some(v) = params.get(&Token::new("subsurface")) {
        if let Some(f) = value_to_f32(v) {
            mp.subsurface = f;
        }
    }
    if let Some(cv) = params.get(&Token::new("subsurfaceColor")) {
        if let Some(c) = value_to_f3(cv) {
            mp.subsurface_color = c;
        }
    }
    if let Some(cv) = params.get(&Token::new("subsurfaceRadius")) {
        if let Some(c) = value_to_f3(cv) {
            mp.subsurface_radius = c;
        }
    }

    // transmission
    if let Some(v) = params.get(&Token::new("transmission")) {
        if let Some(f) = value_to_f32(v) {
            mp.transmission = f;
        }
    }
    if let Some(cv) = params.get(&Token::new("transmissionColor")) {
        if let Some(c) = value_to_f3(cv) {
            mp.transmission_color = c;
        }
    }
    if let Some(v) = params.get(&Token::new("transmissionDepth")) {
        if let Some(f) = value_to_f32(v) {
            mp.transmission_depth = f;
        }
    }

    // anisotropy
    if let Some(v) = params.get(&Token::new("anisotropy")) {
        if let Some(f) = value_to_f32(v) {
            mp.anisotropy = f;
        }
    }
    if let Some(v) = params.get(&Token::new("anisotropyRotation")) {
        if let Some(f) = value_to_f32(v) {
            mp.anisotropy_rotation = f;
        }
    }

    // sheen
    if let Some(cv) = params.get(&Token::new("sheenColor")) {
        if let Some(c) = value_to_f3(cv) {
            mp.sheen_color = c;
        }
    }
    if let Some(v) = params.get(&Token::new("sheenRoughness")) {
        if let Some(f) = value_to_f32(v) {
            mp.sheen_roughness = f;
        }
    }

    // iridescence
    if let Some(v) = params.get(&Token::new("iridescence")) {
        if let Some(f) = value_to_f32(v) {
            mp.iridescence = f;
        }
    }
    if let Some(v) = params.get(&Token::new("iridescenceIor")) {
        if let Some(f) = value_to_f32(v) {
            mp.iridescence_ior = f;
        }
    }
    if let Some(v) = params.get(&Token::new("iridescenceThickness")) {
        if let Some(f) = value_to_f32(v) {
            mp.iridescence_thickness = f;
        }
    }

    // opacityThreshold (float, default 0.0)
    if let Some(v) = params.get(&Token::new("opacityThreshold")) {
        if let Some(f) = value_to_f32(v) {
            mp.opacity_threshold = f;
        }
    }

    // opacityMode: "transparent" (1, default) or "presence" (0)
    if let Some(v) = params.get(&Token::new("opacityMode")) {
        let mode_str = v
            .get::<Token>()
            .map(|t| t.as_str())
            .or_else(|| v.get::<String>().map(|s| s.as_str()));
        if let Some(s) = mode_str {
            mp.opacity_mode = if s == "presence" { 0 } else { 1 };
        }
    }

    mp
}

/// Extract f32 from Value, handling f32/f64/i32 stored values.
fn value_to_f32(v: &Value) -> Option<f32> {
    if let Some(&f) = v.get::<f32>() {
        return Some(f);
    }
    if let Some(&f) = v.get::<f64>() {
        return Some(f as f32);
    }
    if let Some(&i) = v.get::<i32>() {
        return Some(i as f32);
    }
    None
}

/// Extract [f32;3] color from Value, handling multiple representations.
fn value_to_f3(v: &Value) -> Option<[f32; 3]> {
    if let Some(c) = v.get::<usd_gf::Vec3f>() {
        return Some([c[0], c[1], c[2]]);
    }
    if let Some(c) = v.get::<[f32; 3]>() {
        return Some(*c);
    }
    if let Some(c) = v.get::<(f32, f32, f32)>() {
        return Some([c.0, c.1, c.2]);
    }
    if let Some(c) = v.get::<usd_gf::Vec3d>() {
        return Some([c[0] as f32, c[1] as f32, c[2] as f32]);
    }
    if let Some(c) = v.get::<[f64; 3]>() {
        return Some([c[0] as f32, c[1] as f32, c[2] as f32]);
    }
    None
}

fn value_is_truthy(v: &Value) -> bool {
    if let Some(&b) = v.get::<bool>() {
        return b;
    }
    if let Some(&i) = v.get::<i32>() {
        return i != 0;
    }
    if let Some(&f) = v.get::<f32>() {
        return f != 0.0;
    }
    if let Some(&f) = v.get::<f64>() {
        return f != 0.0;
    }
    if let Some(s) = v.get::<String>() {
        let l = s.to_ascii_lowercase();
        return l == "1" || l == "true" || l == "yes" || l == "on";
    }
    if let Some(t) = v.get::<Token>() {
        let l = t.as_str().to_ascii_lowercase();
        return l == "1" || l == "true" || l == "yes" || l == "on";
    }
    false
}

/// Check if a node type identifier represents a texture node.
fn is_texture_node_type(id: &str) -> bool {
    matches!(
        id,
        "UsdUVTexture"
            | "UsdPtexTexture"
            | "ND_image_color3"
            | "ND_image_color4"
            | "ND_image_float"
            | "ND_image_vector3"
    )
}

/// Detect texture type from node identifier and file path.
fn detect_texture_type(node_id: &str, file_path: &str) -> TextureType {
    if node_id == "UsdPtexTexture" || is_ptex_path(file_path) {
        return TextureType::Ptex;
    }
    if is_udim_path(file_path) {
        return TextureType::Udim;
    }
    TextureType::Uv
}

fn is_ptex_path(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.ends_with(".ptx")
        || l.ends_with(".ptex")
        || l.contains("/ptex/")
        || l.contains("\\ptex\\")
        || l.contains("ptex:")
        || l.contains("ptex_")
        || l.contains("_ptex")
}

/// Check if path contains UDIM pattern.
fn is_udim_path(path: &str) -> bool {
    path.contains("<UDIM>") || path.contains("<udim>")
}

/// Extract file path from a texture node's parameter map.
fn extract_file_path(params: &std::collections::BTreeMap<Token, Value>) -> String {
    let file_key = Token::new("file");
    if let Some(file_val) = params.get(&file_key) {
        if let Some(ap) = file_val.get::<usd_sdf::AssetPath>() {
            return ap.get_asset_path().to_string();
        }
        if let Some(s) = file_val.get::<String>() {
            return s.clone();
        }
    }
    String::new()
}

/// Extract sampler parameters from a texture node's parameter map.
fn extract_sampler_params(params: &std::collections::BTreeMap<Token, Value>) -> SamplerParams {
    let mut sp = SamplerParams::default();

    if let Some(v) = params.get(&Token::new("wrapS")) {
        if let Some(t) = v.get::<Token>() {
            sp.wrap_s = t.clone();
        } else if let Some(s) = v.get::<String>() {
            sp.wrap_s = Token::new(s);
        }
    }
    if let Some(v) = params.get(&Token::new("wrapT")) {
        if let Some(t) = v.get::<Token>() {
            sp.wrap_t = t.clone();
        } else if let Some(s) = v.get::<String>() {
            sp.wrap_t = Token::new(s);
        }
    }
    if let Some(v) = params.get(&Token::new("wrapR")) {
        if let Some(t) = v.get::<Token>() {
            sp.wrap_r = t.clone();
        } else if let Some(s) = v.get::<String>() {
            sp.wrap_r = Token::new(s);
        }
    }
    if let Some(v) = params.get(&Token::new("minFilter")) {
        if let Some(t) = v.get::<Token>() {
            sp.min_filter = t.clone();
        } else if let Some(s) = v.get::<String>() {
            sp.min_filter = Token::new(s);
        }
    }
    if let Some(v) = params.get(&Token::new("magFilter")) {
        if let Some(t) = v.get::<Token>() {
            sp.mag_filter = t.clone();
        } else if let Some(s) = v.get::<String>() {
            sp.mag_filter = Token::new(s);
        }
    }

    sp
}

/// Compute a simple hash over texture descriptors for batch invalidation.
fn compute_texture_hash(descs: &[TextureDescriptor]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for desc in descs {
        desc.name.as_str().hash(&mut hasher);
        desc.file_path.hash(&mut hasher);
        (desc.texture_type as u8).hash(&mut hasher);
    }
    hasher.finish()
}

/// Shared pointer to Storm material.
pub type HdStMaterialSharedPtr = std::sync::Arc<HdStMaterial>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let material = HdStMaterial::new(path.clone());

        assert_eq!(material.get_path(), &path);
        assert_eq!(material.get_tag(), &Token::new("defaultMaterialTag"));
        assert!(!material.is_compiled());
        assert!(material.is_network_dirty());
        assert!(!material.has_ptex());
        assert!(!material.has_limit_surface_evaluation());
        assert!(!material.has_displacement());
        assert_eq!(material.get_volume_source(), "");
        assert!(material.get_texture_descriptors().is_empty());
        assert_eq!(material.get_texture_hash(), 0);
    }

    #[test]
    fn test_material_tag() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let mut material = HdStMaterial::new(path);

        material.set_tag(Token::new("masked"));
        assert_eq!(material.get_tag(), &Token::new("masked"));
    }

    #[test]
    fn test_parameters() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let mut material = HdStMaterial::new(path);

        let roughness = Value::from(0.5f32);
        material.set_param(Token::new("roughness"), roughness.clone());

        assert_eq!(
            material.get_param(&Token::new("roughness")),
            Some(&roughness)
        );
        assert_eq!(material.get_params().len(), 1);
    }

    #[test]
    fn test_sync() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let mut material = HdStMaterial::new(path);

        assert!(material.is_network_dirty());
        assert!(!material.is_compiled());

        material.sync();

        assert!(!material.is_network_dirty());
        assert!(material.is_compiled());
        assert!(material.get_shader_handle() != 0);
    }

    #[test]
    fn test_mark_dirty() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let mut material = HdStMaterial::new(path);

        material.sync();
        assert!(!material.is_network_dirty());

        material.mark_network_dirty();
        assert!(material.is_network_dirty());
        assert!(!material.is_compiled());
    }

    #[test]
    fn test_detect_ptex_paths() {
        assert!(is_ptex_path("C:/assets/skin.ptex"));
        assert!(is_ptex_path("textures/ptex/albedo.tx"));
        assert!(!is_ptex_path("textures/albedo.png"));
    }

    #[test]
    fn test_truthy_parse() {
        assert!(value_is_truthy(&Value::from(true)));
        assert!(value_is_truthy(&Value::from(1i32)));
        assert!(value_is_truthy(&Value::from(1.0f32)));
        assert!(!value_is_truthy(&Value::from(0i32)));
        assert!(!value_is_truthy(&Value::from(0.0f32)));
    }

    #[test]
    fn test_texture_type_default() {
        assert_eq!(TextureType::default(), TextureType::Uv);
    }

    #[test]
    fn test_detect_texture_type() {
        assert_eq!(
            detect_texture_type("UsdPtexTexture", "skin.ptex"),
            TextureType::Ptex
        );
        assert_eq!(
            detect_texture_type("UsdUVTexture", "albedo.<UDIM>.exr"),
            TextureType::Udim
        );
        assert_eq!(
            detect_texture_type("UsdUVTexture", "albedo.png"),
            TextureType::Uv
        );
    }

    #[test]
    fn test_sampler_params_default() {
        let sp = SamplerParams::default();
        assert_eq!(sp.wrap_s, Token::new("useMetadata"));
        assert_eq!(sp.min_filter, Token::new("linear"));
        assert_eq!(sp.mag_filter, Token::new("linear"));
    }

    #[test]
    fn test_extract_sampler_params() {
        let mut params = std::collections::BTreeMap::new();
        params.insert(Token::new("wrapS"), Value::from(Token::new("repeat")));
        params.insert(Token::new("wrapT"), Value::from(Token::new("clamp")));
        params.insert(Token::new("minFilter"), Value::from(Token::new("nearest")));

        let sp = extract_sampler_params(&params);
        assert_eq!(sp.wrap_s, Token::new("repeat"));
        assert_eq!(sp.wrap_t, Token::new("clamp"));
        assert_eq!(sp.min_filter, Token::new("nearest"));
        assert_eq!(sp.mag_filter, Token::new("linear")); // default
    }

    #[test]
    fn test_texture_descriptor_hash() {
        let descs = vec![
            TextureDescriptor {
                name: Token::new("diffuseColor"),
                file_path: "albedo.png".to_string(),
                texture_type: TextureType::Uv,
                sampler_params: SamplerParams::default(),
            },
            TextureDescriptor {
                name: Token::new("normal"),
                file_path: "normal.png".to_string(),
                texture_type: TextureType::Uv,
                sampler_params: SamplerParams::default(),
            },
        ];
        let h1 = compute_texture_hash(&descs);
        assert_ne!(h1, 0);

        // Same descriptors -> same hash
        let h2 = compute_texture_hash(&descs);
        assert_eq!(h1, h2);

        // Different descriptors -> different hash
        let descs2 = vec![TextureDescriptor {
            name: Token::new("roughness"),
            file_path: "rough.exr".to_string(),
            texture_type: TextureType::Uv,
            sampler_params: SamplerParams::default(),
        }];
        let h3 = compute_texture_hash(&descs2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_volume_material_data_default() {
        let vmd = VolumeMaterialData::default();
        assert!(vmd.source.is_empty());
        assert!(vmd.params.is_empty());
    }

    #[test]
    fn test_finalize_clears_all() {
        let path = SdfPath::from_string("/materials/mat1").unwrap();
        let mut material = HdStMaterial::new(path);
        material.sync();
        assert!(material.is_compiled());

        material.finalize();
        assert!(!material.is_compiled());
        assert!(material.is_network_dirty());
        assert!(!material.has_ptex());
        assert!(!material.has_displacement());
        assert!(!material.has_limit_surface_evaluation());
        assert_eq!(material.get_texture_hash(), 0);
        assert!(material.get_texture_descriptors().is_empty());
        assert!(material.get_volume_source().is_empty());
    }

    #[test]
    fn test_initial_dirty_bits() {
        let bits = HdStMaterial::get_initial_dirty_bits_mask();
        assert_ne!(bits, 0);
        // ALL_DIRTY should include all material dirty flags
        assert_eq!(bits, HdMaterialDirtyBits::ALL_DIRTY);
    }

    #[test]
    fn test_is_texture_node_type() {
        assert!(is_texture_node_type("UsdUVTexture"));
        assert!(is_texture_node_type("UsdPtexTexture"));
        assert!(is_texture_node_type("ND_image_color3"));
        assert!(is_texture_node_type("ND_image_float"));
        assert!(!is_texture_node_type("UsdPreviewSurface"));
        assert!(!is_texture_node_type("UsdPrimvarReader_float2"));
    }

    #[test]
    fn test_udim_detection() {
        assert!(is_udim_path("albedo.<UDIM>.exr"));
        assert!(is_udim_path("textures/skin.<udim>.tx"));
        assert!(!is_udim_path("textures/skin.1001.tx"));
    }
}
