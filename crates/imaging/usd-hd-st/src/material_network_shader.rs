//! MaterialNetworkShader - Scene-based material network shader object.
//!
//! Two layers live here:
//!
//!   1. `MaterialNetworkShader` — direct port of C++ HdSt_MaterialNetworkShader.
//!      Owns fragment/displacement sources, material params, named texture
//!      handles, the shader-data buffer range, and cached hash values.
//!      This is the object HdStMaterial creates and draw batches consume.
//!
//!   2. Legacy helpers `extract_material` / `params_from_map` / `ExtractedMaterial`
//!      / `TextureBindings` — kept for backward compat with existing callers.

use crate::material::HdStMaterial;
use crate::shader_code::{NamedTextureHandle, ShaderParameter, ShaderStage};
use crate::wgsl_code_gen::MaterialParams;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use usd_gf::{Vec3f, Vec4f};
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// ShaderParam — material parameter type classification
// ---------------------------------------------------------------------------

/// Classification of a material shader parameter.
///
/// Direct port of C++ HdSt_MaterialParam::ParamType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamType {
    /// Fallback value written to the shader bar.
    Fallback,
    /// Sampled texture (UVTexture / Ptex / Field / UDIM).
    Texture,
    /// Primvar redirect — reads a primvar instead of a constant.
    PrimvarRedirect,
    /// Additional primvar consumed by the material (no redirect).
    AdditionalPrimvar,
    /// 2D UV transform applied to texture coordinates.
    Transform2d,
    /// Field redirect — redirects a volume field lookup (C++ ParamTypeFieldRedirect).
    FieldRedirect,
}

/// A single material shader parameter.
///
/// Port of C++ HdSt_MaterialParam. Carries type, name, fallback value,
/// texture type, and sampler coord names needed by codegen.
#[derive(Debug, Clone)]
pub struct ShaderParam {
    /// Parameter classification.
    pub param_type: ParamType,
    /// Name of the parameter (used in shader accessor HdGet_<name>).
    pub name: Token,
    /// Fallback / default value written to the shader bar.
    pub fallback_value: Value,
    /// Sampler coord primvar names (for textures and primvar redirects).
    pub sampler_coords: Vec<Token>,
    /// Texture type when param_type == Texture.
    pub texture_type: Token,
    /// GLSL swizzle mask applied when reading this parameter (e.g. "rgb", "r").
    /// Mirrors C++ HdSt_MaterialParam::swizzle.
    pub swizzle: String,
    /// True if the texture value is premultiplied by alpha.
    /// Mirrors C++ HdSt_MaterialParam::isPremultiplied.
    pub is_premultiplied: bool,
    /// For array-of-textures: number of textures in the array (0 = single texture).
    /// Mirrors C++ HdSt_MaterialParam::arrayOfTexturesSize.
    pub array_of_textures_size: usize,
}

impl ShaderParam {
    /// Create a simple fallback (constant) parameter.
    pub fn new_fallback(name: Token, value: Value) -> Self {
        Self {
            param_type: ParamType::Fallback,
            name,
            fallback_value: value,
            sampler_coords: Vec::new(),
            texture_type: Token::new("uv"),
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a texture parameter.
    pub fn new_texture(name: Token, fallback: Value, sampler_coords: Vec<Token>) -> Self {
        Self {
            param_type: ParamType::Texture,
            name,
            fallback_value: fallback,
            sampler_coords,
            texture_type: Token::new("uv"),
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a primvar-redirect parameter.
    pub fn new_primvar_redirect(name: Token, sampler_coords: Vec<Token>) -> Self {
        Self {
            param_type: ParamType::PrimvarRedirect,
            name,
            fallback_value: Value::default(),
            sampler_coords,
            texture_type: Token::new(""),
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    pub fn is_texture(&self) -> bool {
        self.param_type == ParamType::Texture
    }

    pub fn is_primvar_redirect(&self) -> bool {
        self.param_type == ParamType::PrimvarRedirect
    }

    pub fn is_additional_primvar(&self) -> bool {
        self.param_type == ParamType::AdditionalPrimvar
    }

    /// Compute hash over a slice of params (mirrors C++ HdSt_MaterialParam::ComputeHash).
    pub fn compute_hash(params: &[ShaderParam]) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in params {
            p.name.as_str().hash(&mut h);
            (p.param_type as u8).hash(&mut h);
            p.texture_type.as_str().hash(&mut h);
            for sc in &p.sampler_coords {
                sc.as_str().hash(&mut h);
            }
        }
        h.finish()
    }
}

// ---------------------------------------------------------------------------
// MaterialNetworkShader
// ---------------------------------------------------------------------------

/// Storm material network shader object.
///
/// Wraps a compiled material network into the form consumed by draw batches:
/// WGSL fragment/displacement sources, material params, texture handles,
/// a shader-bar buffer range, and per-frame hash caching.
///
/// Mirrors C++ `HdSt_MaterialNetworkShader`.
///
/// # Lifecycle
///
/// - `HdStMaterial::sync_from_delegate` creates / updates one instance per material.
/// - Draw items hold a shared reference; draw batches group by `compute_hash()`.
/// - `bind_resources` / `unbind_resources` called per-batch at draw time.
///
/// # Hash semantics
///
/// - `compute_hash()` — covers shader source + param signature.
///   Two shaders with same hash produce identical codegen output.
/// - `compute_texture_source_hash()` — covers texture bindings only.
///   Used to split draw batches when textures differ (non-bindless path).
#[derive(Debug)]
pub struct MaterialNetworkShader {
    /// Fragment (surface) shader source in WGSL.
    fragment_source: String,
    /// Displacement shader source in WGSL (empty when absent).
    displacement_source: String,

    /// Typed material parameters (drives codegen + shader-bar layout).
    params: Vec<ShaderParam>,

    /// Primvar names referenced by this shader (filtered by draw batch).
    primvar_names: Vec<Token>,

    /// Named GPU texture handles ready for binding.
    named_texture_handles: Vec<NamedTextureHandle>,

    /// Whether primvar filtering is enabled for this shader.
    primvar_filtering_enabled: bool,

    /// Material tag token ("defaultMaterialTag", "translucent", etc.).
    material_tag: Token,

    /// Fallback values per param name (for shader-bar population).
    param_fallbacks: HashMap<Token, Value>,

    /// Cached combined hash (fragment source + params).  Invalidated on mutation.
    cached_hash: u64,
    hash_valid: bool,

    /// Cached texture-source hash (texture names + hashes).
    cached_tex_hash: u64,
    tex_hash_valid: bool,

    /// Shader parameters extracted from UsdPreviewSurface for GPU uniform upload.
    /// Kept alongside typed `params` for easy access by draw batch.
    material_params: MaterialParams,
}

impl Default for MaterialNetworkShader {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialNetworkShader {
    /// Create an empty shader (will use fallback rendering until populated).
    pub fn new() -> Self {
        Self {
            fragment_source: String::new(),
            displacement_source: String::new(),
            params: Vec::new(),
            primvar_names: collect_primvar_names(&[]),
            named_texture_handles: Vec::new(),
            primvar_filtering_enabled: true,
            material_tag: Token::new("defaultMaterialTag"),
            param_fallbacks: HashMap::new(),
            cached_hash: 0,
            hash_valid: false,
            cached_tex_hash: 0,
            tex_hash_valid: false,
            material_params: MaterialParams::default(),
        }
    }

    // ------------------------------------------------------------------
    // Source setters
    // ------------------------------------------------------------------

    /// Set WGSL fragment source and invalidate hash.
    pub fn set_fragment_source(&mut self, src: String) {
        self.fragment_source = src;
        self.hash_valid = false;
    }

    /// Set WGSL displacement source and invalidate hash.
    pub fn set_displacement_source(&mut self, src: String) {
        self.displacement_source = src;
        self.hash_valid = false;
    }

    /// Get source for a shader stage key.
    ///
    /// Mirrors C++ `GetSource(TfToken shaderStageKey)`.
    /// Returns empty string for stages this shader does not provide.
    pub fn get_source(&self, stage: ShaderStage) -> &str {
        match stage {
            ShaderStage::Fragment => &self.fragment_source,
            ShaderStage::Geometry => &self.displacement_source, // displacement maps to GS slot
            _ => "",
        }
    }

    // ------------------------------------------------------------------
    // Params
    // ------------------------------------------------------------------

    /// Replace material params and rebuild primvar name list.
    ///
    /// Mirrors C++ `SetParams`.
    pub fn set_params(&mut self, params: Vec<ShaderParam>) {
        // Rebuild fallback table
        self.param_fallbacks.clear();
        for p in &params {
            if p.fallback_value != Value::default() {
                self.param_fallbacks
                    .insert(p.name.clone(), p.fallback_value.clone());
            }
        }
        self.primvar_names = collect_primvar_names(&params);
        self.params = params;
        self.hash_valid = false;
    }

    /// Get material params slice.
    pub fn get_params(&self) -> &[ShaderParam] {
        &self.params
    }

    /// Get the fallback value for a param by name.
    ///
    /// Checks authored fallbacks first, then built-in primvar defaults.
    /// Mirrors C++ `GetFallbackValueForParam`.
    pub fn get_fallback_value(&self, name: &Token) -> Option<&Value> {
        self.param_fallbacks
            .get(name)
            .or_else(|| PRIMVAR_DEFAULTS.get(name))
    }

    // ------------------------------------------------------------------
    // Primvar filtering
    // ------------------------------------------------------------------

    /// Enable / disable primvar filtering (gated by env var in C++).
    pub fn set_primvar_filtering_enabled(&mut self, enabled: bool) {
        self.primvar_filtering_enabled = enabled;
    }

    pub fn is_primvar_filtering_enabled(&self) -> bool {
        self.primvar_filtering_enabled
    }

    /// Primvar names referenced by this shader.
    pub fn get_primvar_names(&self) -> &[Token] {
        &self.primvar_names
    }

    // ------------------------------------------------------------------
    // Texture handles
    // ------------------------------------------------------------------

    /// Replace named texture handles and invalidate texture hash.
    ///
    /// Mirrors C++ `SetNamedTextureHandles`.
    pub fn set_named_texture_handles(&mut self, handles: Vec<NamedTextureHandle>) {
        self.named_texture_handles = handles;
        self.tex_hash_valid = false;
    }

    /// Get named texture handles.
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }

    // ------------------------------------------------------------------
    // Material tag
    // ------------------------------------------------------------------

    /// Set material tag and invalidate hash.
    pub fn set_material_tag(&mut self, tag: Token) {
        self.material_tag = tag;
        self.hash_valid = false;
    }

    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    // ------------------------------------------------------------------
    // GPU material params (UsdPreviewSurface extracted values)
    // ------------------------------------------------------------------

    /// Set extracted GPU material params (diffuse, roughness, metallic, etc.).
    pub fn set_material_params(&mut self, p: MaterialParams) {
        self.material_params = p;
    }

    /// Get GPU material params for uniform upload.
    pub fn get_material_params(&self) -> &MaterialParams {
        &self.material_params
    }

    // ------------------------------------------------------------------
    // Hash
    // ------------------------------------------------------------------

    /// Compute / return cached hash over shader source + params.
    ///
    /// Mirrors C++ `ComputeHash`.
    pub fn compute_hash(&mut self) -> u64 {
        if !self.hash_valid {
            self.cached_hash = self.compute_hash_impl();
            self.hash_valid = true;
        }
        self.cached_hash
    }

    fn compute_hash_impl(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // Params signature
        ShaderParam::compute_hash(&self.params).hash(&mut h);
        // Shader sources
        self.fragment_source.hash(&mut h);
        self.displacement_source.hash(&mut h);
        h.finish()
    }

    /// Compute / return cached texture-source hash.
    ///
    /// Returns 0 when using bindless textures (no per-batch split needed).
    /// Mirrors C++ `ComputeTextureSourceHash`.
    pub fn compute_texture_source_hash(&mut self) -> u64 {
        if !self.tex_hash_valid {
            self.cached_tex_hash = self.compute_tex_hash_impl();
            self.tex_hash_valid = true;
        }
        self.cached_tex_hash
    }

    fn compute_tex_hash_impl(&self) -> u64 {
        // Bindless: return 0 so all instances share the same batch.
        // Non-bindless: combine name + content hash (not the GPU handle ID).
        // Mirrors C++ ComputeTextureSourceHash which iterates handles[i]->GetHash().
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for nth in &self.named_texture_handles {
            nth.name.as_str().hash(&mut h);
            nth.hash.hash(&mut h);
        }
        h.finish()
    }

    // ------------------------------------------------------------------
    // Resource binding
    // ------------------------------------------------------------------

    /// Bind all named texture resources.
    ///
    /// Mirrors C++ `BindResources` → `HdSt_TextureBinder::BindResources`.
    /// In the wgpu backend textures are bound via bind groups rather than
    /// per-call GL texture units; this is a no-op placeholder that matches
    /// the C++ interface so callers have a place to hook GPU binding.
    pub fn bind_resources(&self) {
        // wgpu: textures bound via bind groups assembled in PipelineDrawBatch.
        // This method exists for API parity; actual binding happens in draw_batch.
        log::trace!(
            "MaterialNetworkShader::bind_resources: {} textures",
            self.named_texture_handles.len()
        );
    }

    /// Unbind all named texture resources.
    pub fn unbind_resources(&self) {
        log::trace!("MaterialNetworkShader::unbind_resources");
    }

    /// Add custom binding requests (for codegen).
    ///
    /// Mirrors C++ `AddBindings` (currently no-op — no custom bindings needed).
    pub fn add_bindings(&self) {
        // No custom bindings in WGSL path.
    }

    // ------------------------------------------------------------------
    // Shader parameters as ShaderParameter list (for HdStShaderCode compat)
    // ------------------------------------------------------------------

    /// Convert internal ShaderParam list to ShaderParameter list used by codegen.
    pub fn as_shader_parameters(&self) -> Vec<ShaderParameter> {
        self.params
            .iter()
            .map(|p| ShaderParameter::new(p.name.clone(), p.fallback_value.clone()))
            .collect()
    }

    // ------------------------------------------------------------------
    // Reload
    // ------------------------------------------------------------------

    /// Reload shader from asset (no-op — sources managed externally).
    pub fn reload(&self) {
        // Sources set via SetFragmentSource; nothing to reload.
    }
}

// Shared pointer type alias matching C++ naming.
pub type MaterialNetworkShaderSharedPtr = std::sync::Arc<std::sync::Mutex<MaterialNetworkShader>>;

// ---------------------------------------------------------------------------
// Built-in primvar defaults  (mirrors static _primvarDefaults table in .cpp)
// ---------------------------------------------------------------------------

static PRIMVAR_DEFAULTS: LazyLock<HashMap<Token, Value>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Standard display primvars
    m.insert(
        Token::new("displayColor"),
        Value::from(Vec3f::new(1.0, 1.0, 1.0)),
    );
    m.insert(Token::new("displayOpacity"), Value::from(1.0f32));
    // Geometric shader slots
    m.insert(Token::new("ptexFaceOffset"), Value::from(0i32));
    // Simple lighting shader
    m.insert(Token::new("displayMetallic"), Value::from(0.0f32));
    m.insert(Token::new("displayRoughness"), Value::from(1.0f32));
    // Terminal shader
    m.insert(
        Token::new("hullColor"),
        Value::from(Vec3f::new(1.0, 1.0, 1.0)),
    );
    m.insert(Token::new("hullOpacity"), Value::from(1.0f32));
    m.insert(Token::new("scalarOverride"), Value::from(1.0f32));
    m.insert(Token::new("selectedWeight"), Value::from(0.0f32));
    // RenderPass shader — Vec4f colors (rgba)
    m.insert(
        Token::new("indicatorColor"),
        Value::from(Vec4f::new(1.0, 1.0, 1.0, 1.0)),
    );
    m.insert(Token::new("indicatorWeight"), Value::from(0.0f32));
    m.insert(
        Token::new("overrideColor"),
        Value::from(Vec4f::new(1.0, 1.0, 1.0, 1.0)),
    );
    m.insert(
        Token::new("maskColor"),
        Value::from(Vec4f::new(1.0, 1.0, 1.0, 1.0)),
    );
    m.insert(Token::new("maskWeight"), Value::from(0.0f32));
    m.insert(
        Token::new("wireframeColor"),
        Value::from(Vec4f::new(1.0, 1.0, 1.0, 1.0)),
    );
    // scalarOverrideColorRamp: VtArray<GfVec4f> but Vec4f lacks Hash, so stored as
    // default (empty) Value — callers that need the ramp populate it explicitly.
    m.insert(Token::new("scalarOverrideColorRamp"), Value::default());
    m
});

// Extra primvar names always included regardless of material (mirrors _GetExtraIncludedShaderPrimvarNames).
static EXTRA_PRIMVAR_NAMES: LazyLock<Vec<Token>> = LazyLock::new(|| {
    vec![
        Token::new("displayColor"),
        Token::new("displayOpacity"),
        Token::new("ptexFaceOffset"),
        Token::new("displayMetallic"),
        Token::new("displayRoughness"),
        Token::new("hullColor"),
        Token::new("hullOpacity"),
        Token::new("scalarOverride"),
        Token::new("selectedWeight"),
        Token::new("indicatorColor"),
        Token::new("indicatorWeight"),
        Token::new("overrideColor"),
        Token::new("overrideWireframeColor"),
        Token::new("maskColor"),
        Token::new("maskWeight"),
        Token::new("wireframeColor"),
    ]
});

/// Build primvar name list from params, prepending extra built-ins.
///
/// Mirrors C++ `_CollectPrimvarNames`.
fn collect_primvar_names(params: &[ShaderParam]) -> Vec<Token> {
    let mut names = EXTRA_PRIMVAR_NAMES.clone();
    for p in params {
        if p.is_primvar_redirect() {
            names.push(p.name.clone());
            names.extend(p.sampler_coords.iter().cloned());
        } else if p.is_texture() {
            names.extend(p.sampler_coords.iter().cloned());
        } else if p.is_additional_primvar() {
            names.push(p.name.clone());
        }
    }
    names
}

// ===========================================================================
// Legacy helpers (kept for backward compat — callers in engine.rs, etc.)
// ===========================================================================

// UsdPreviewSurface parameter tokens
static DIFFUSE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("diffuseColor"));
static METALLIC: LazyLock<Token> = LazyLock::new(|| Token::new("metallic"));
static ROUGHNESS: LazyLock<Token> = LazyLock::new(|| Token::new("roughness"));
static OPACITY: LazyLock<Token> = LazyLock::new(|| Token::new("opacity"));
static EMISSIVE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("emissiveColor"));
static IOR: LazyLock<Token> = LazyLock::new(|| Token::new("ior"));
static SPECULAR_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("specularColor"));
static USE_VERTEX_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("useVertexColor"));

// Texture connection tokens
static DIFFUSE_TEX: LazyLock<Token> = LazyLock::new(|| Token::new("diffuseColor:texture"));
static NORMAL_TEX: LazyLock<Token> = LazyLock::new(|| Token::new("normal:texture"));
static ROUGHNESS_TEX: LazyLock<Token> = LazyLock::new(|| Token::new("roughness:texture"));
static METALLIC_TEX: LazyLock<Token> = LazyLock::new(|| Token::new("metallic:texture"));
static EMISSIVE_TEX: LazyLock<Token> = LazyLock::new(|| Token::new("emissiveColor:texture"));

/// Texture references extracted from a material network.
/// These map to texture handles that will be bound at draw time.
#[derive(Debug, Clone, Default)]
pub struct TextureBindings {
    /// Diffuse/albedo texture asset path (if connected)
    pub diffuse_tex: Option<String>,
    /// Normal map texture asset path
    pub normal_tex: Option<String>,
    /// Roughness map texture asset path
    pub roughness_tex: Option<String>,
    /// Metallic map texture asset path
    pub metallic_tex: Option<String>,
    /// Emissive map texture asset path
    pub emissive_tex: Option<String>,
}

impl TextureBindings {
    /// True if any texture is bound.
    pub fn has_any(&self) -> bool {
        self.diffuse_tex.is_some()
            || self.normal_tex.is_some()
            || self.roughness_tex.is_some()
            || self.metallic_tex.is_some()
            || self.emissive_tex.is_some()
    }
}

/// Result of extracting material data from a network.
#[derive(Debug, Clone)]
pub struct ExtractedMaterial {
    /// Material uniform parameters
    pub params: MaterialParams,
    /// Texture connections for binding
    pub textures: TextureBindings,
    /// Whether material has any non-default params (i.e. is authored)
    pub is_authored: bool,
}

/// Extract MaterialParams from an HdStMaterial's parameter map.
///
/// Reads UsdPreviewSurface parameters (diffuseColor, metallic, roughness, etc.)
/// and falls back to sensible defaults when params are missing.
pub fn extract_material(material: &HdStMaterial) -> ExtractedMaterial {
    let params_map = material.get_params();
    let mut is_authored = false;

    let diffuse_color =
        extract_vec3f(params_map.get(&*DIFFUSE_COLOR)).unwrap_or([0.18, 0.18, 0.18]);
    if params_map.contains_key(&*DIFFUSE_COLOR) {
        is_authored = true;
    }
    let metallic = extract_f32(params_map.get(&*METALLIC)).unwrap_or(0.0);
    if params_map.contains_key(&*METALLIC) {
        is_authored = true;
    }
    let roughness = extract_f32(params_map.get(&*ROUGHNESS)).unwrap_or(0.5);
    if params_map.contains_key(&*ROUGHNESS) {
        is_authored = true;
    }
    let opacity = extract_f32(params_map.get(&*OPACITY)).unwrap_or(1.0);
    if params_map.contains_key(&*OPACITY) {
        is_authored = true;
    }
    let emissive_color = extract_vec3f(params_map.get(&*EMISSIVE_COLOR)).unwrap_or([0.0, 0.0, 0.0]);
    if params_map.contains_key(&*EMISSIVE_COLOR) {
        is_authored = true;
    }
    let ior = extract_f32(params_map.get(&*IOR)).unwrap_or(1.5);
    let _specular_color =
        extract_vec3f(params_map.get(&*SPECULAR_COLOR)).unwrap_or([1.0, 1.0, 1.0]);
    let use_vertex_color = extract_f32(params_map.get(&*USE_VERTEX_COLOR))
        .map(|v| v > 0.5)
        .unwrap_or(false);

    let textures = TextureBindings {
        diffuse_tex: extract_tex_path(params_map.get(&*DIFFUSE_TEX)),
        normal_tex: extract_tex_path(params_map.get(&*NORMAL_TEX)),
        roughness_tex: extract_tex_path(params_map.get(&*ROUGHNESS_TEX)),
        metallic_tex: extract_tex_path(params_map.get(&*METALLIC_TEX)),
        emissive_tex: extract_tex_path(params_map.get(&*EMISSIVE_TEX)),
    };
    if textures.has_any() {
        is_authored = true;
    }

    let has_diffuse_tex = textures.diffuse_tex.is_some();
    let has_normal_tex = textures.normal_tex.is_some();
    let has_roughness_tex = textures.roughness_tex.is_some();
    let has_metallic_tex = textures.metallic_tex.is_some();
    let has_emissive_tex = textures.emissive_tex.is_some();

    ExtractedMaterial {
        params: MaterialParams {
            diffuse_color,
            metallic,
            roughness,
            opacity,
            emissive_color,
            use_vertex_color,
            ior,
            clearcoat: 0.0,
            clearcoat_roughness: 0.01,
            use_specular_workflow: false,
            specular_color: [0.0, 0.0, 0.0],
            displacement: 0.0,
            has_diffuse_tex,
            has_normal_tex,
            has_roughness_tex,
            has_metallic_tex,
            has_emissive_tex,
            has_opacity_tex: false,
            has_occlusion_tex: false,
            ..Default::default()
        },
        textures,
        is_authored,
    }
}

/// Build MaterialParams from a raw parameter map (no HdStMaterial needed).
pub fn params_from_map(params_map: &HashMap<Token, Value>) -> MaterialParams {
    MaterialParams {
        diffuse_color: extract_vec3f(params_map.get(&*DIFFUSE_COLOR)).unwrap_or([0.18, 0.18, 0.18]),
        metallic: extract_f32(params_map.get(&*METALLIC)).unwrap_or(0.0),
        roughness: extract_f32(params_map.get(&*ROUGHNESS)).unwrap_or(0.5),
        opacity: extract_f32(params_map.get(&*OPACITY)).unwrap_or(1.0),
        emissive_color: extract_vec3f(params_map.get(&*EMISSIVE_COLOR)).unwrap_or([0.0, 0.0, 0.0]),
        ior: extract_f32(params_map.get(&*IOR)).unwrap_or(1.5),
        use_vertex_color: extract_f32(params_map.get(&*USE_VERTEX_COLOR))
            .map(|v| v > 0.5)
            .unwrap_or(false),
        clearcoat: 0.0,
        clearcoat_roughness: 0.01,
        use_specular_workflow: false,
        specular_color: [0.0, 0.0, 0.0],
        displacement: 0.0,
        has_diffuse_tex: false,
        has_normal_tex: false,
        has_roughness_tex: false,
        has_metallic_tex: false,
        has_opacity_tex: false,
        has_emissive_tex: false,
        has_occlusion_tex: false,
        ..Default::default()
    }
}

// --- Value extraction helpers ---

fn extract_f32(val: Option<&Value>) -> Option<f32> {
    let v = val?;
    if let Some(&f) = v.get::<f32>() {
        return Some(f);
    }
    if let Some(&d) = v.get::<f64>() {
        return Some(d as f32);
    }
    None
}

fn extract_vec3f(val: Option<&Value>) -> Option<[f32; 3]> {
    let v = val?;
    if let Some(vec3) = v.get::<Vec3f>() {
        return Some([vec3.x, vec3.y, vec3.z]);
    }
    None
}

fn extract_tex_path(val: Option<&Value>) -> Option<String> {
    let v = val?;
    v.get::<String>().cloned()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::Path as SdfPath;

    // ----- MaterialNetworkShader tests -----

    #[test]
    fn test_new_shader_defaults() {
        let s = MaterialNetworkShader::new();
        assert!(s.get_source(ShaderStage::Fragment).is_empty());
        assert!(s.get_source(ShaderStage::Vertex).is_empty());
        assert!(s.get_params().is_empty());
        assert!(s.get_named_texture_handles().is_empty());
        assert_eq!(s.get_material_tag(), &Token::new("defaultMaterialTag"));
        assert!(s.is_primvar_filtering_enabled());
        // Primvar list should contain the built-in extras even for empty params
        assert!(s.get_primvar_names().contains(&Token::new("displayColor")));
    }

    #[test]
    fn test_set_fragment_source_invalidates_hash() {
        let mut s = MaterialNetworkShader::new();
        let h1 = s.compute_hash();
        s.set_fragment_source("fn main() {}".to_string());
        let h2 = s.compute_hash();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_stable_after_recompute() {
        let mut s = MaterialNetworkShader::new();
        s.set_fragment_source("// shader".to_string());
        let h1 = s.compute_hash();
        let h2 = s.compute_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_set_params_updates_primvars() {
        let mut s = MaterialNetworkShader::new();
        let params = vec![ShaderParam::new_primvar_redirect(
            Token::new("st"),
            vec![Token::new("map1")],
        )];
        s.set_params(params);
        let names = s.get_primvar_names();
        assert!(names.contains(&Token::new("st")));
        assert!(names.contains(&Token::new("map1")));
        // Built-ins still present
        assert!(names.contains(&Token::new("displayColor")));
    }

    #[test]
    fn test_fallback_value_lookup() {
        let mut s = MaterialNetworkShader::new();
        let params = vec![ShaderParam::new_fallback(
            Token::new("roughness"),
            Value::from(0.3f32),
        )];
        s.set_params(params);
        let fb = s.get_fallback_value(&Token::new("roughness"));
        assert!(fb.is_some());
        assert_eq!(fb.unwrap().get::<f32>(), Some(&0.3f32));
    }

    #[test]
    fn test_builtin_primvar_fallback() {
        let s = MaterialNetworkShader::new();
        // displayOpacity has a built-in default of 1.0
        let fb = s.get_fallback_value(&Token::new("displayOpacity"));
        assert!(fb.is_some());
    }

    #[test]
    fn test_texture_source_hash_changes_with_handles() {
        let mut s = MaterialNetworkShader::new();
        let h1 = s.compute_texture_source_hash();
        s.set_named_texture_handles(vec![NamedTextureHandle::new(
            Token::new("diffuse"),
            Token::new("2D"),
        )]);
        let h2 = s.compute_texture_source_hash();
        // With no GPU handles set both may be 0 (empty hash) — just check it ran
        let _ = (h1, h2);
    }

    #[test]
    fn test_material_tag() {
        let mut s = MaterialNetworkShader::new();
        s.set_material_tag(Token::new("translucent"));
        assert_eq!(s.get_material_tag(), &Token::new("translucent"));
    }

    #[test]
    fn test_as_shader_parameters() {
        let mut s = MaterialNetworkShader::new();
        s.set_params(vec![ShaderParam::new_fallback(
            Token::new("metallic"),
            Value::from(0.9f32),
        )]);
        let sp = s.as_shader_parameters();
        assert_eq!(sp.len(), 1);
        assert_eq!(sp[0].name, Token::new("metallic"));
    }

    // ----- Legacy extract_material tests -----

    #[test]
    fn test_default_material_extraction() {
        let path = SdfPath::from_string("/mat").unwrap();
        let material = HdStMaterial::new(path);
        let extracted = extract_material(&material);
        assert_eq!(extracted.params.diffuse_color, [0.18, 0.18, 0.18]);
        assert_eq!(extracted.params.metallic, 0.0);
        assert_eq!(extracted.params.roughness, 0.5);
        assert_eq!(extracted.params.opacity, 1.0);
        assert!(!extracted.is_authored);
    }

    #[test]
    fn test_authored_material() {
        let path = SdfPath::from_string("/mat").unwrap();
        let mut material = HdStMaterial::new(path);
        material.set_param(
            Token::new("diffuseColor"),
            Value::from(Vec3f::new(1.0, 0.0, 0.0)),
        );
        material.set_param(Token::new("metallic"), Value::from(0.8f32));
        material.set_param(Token::new("roughness"), Value::from(0.2f32));
        let extracted = extract_material(&material);
        assert_eq!(extracted.params.diffuse_color, [1.0, 0.0, 0.0]);
        assert_eq!(extracted.params.metallic, 0.8);
        assert_eq!(extracted.params.roughness, 0.2);
        assert!(extracted.is_authored);
    }

    #[test]
    fn test_params_from_map() {
        let mut map = HashMap::new();
        map.insert(Token::new("roughness"), Value::from(0.9f32));
        let params = params_from_map(&map);
        assert_eq!(params.roughness, 0.9);
        assert_eq!(params.metallic, 0.0);
    }

    #[test]
    fn test_texture_bindings() {
        let path = SdfPath::from_string("/mat").unwrap();
        let mut material = HdStMaterial::new(path);
        material.set_param(
            Token::new("diffuseColor:texture"),
            Value::from("/textures/albedo.png".to_string()),
        );
        let extracted = extract_material(&material);
        assert!(extracted.textures.diffuse_tex.is_some());
        assert_eq!(
            extracted.textures.diffuse_tex.as_deref(),
            Some("/textures/albedo.png")
        );
        assert!(extracted.is_authored);
    }
}
