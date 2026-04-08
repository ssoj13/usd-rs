//! HdStShaderCode - Base shader code abstraction.
//!
//! Provides the interface for shader code generation and resource binding.
//! Shader code objects encapsulate GLSL/MSL/SPIR-V source and parameter bindings.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// Texture sampling parameters.
#[derive(Debug, Clone)]
pub struct TextureSamplerParams {
    /// Wrap mode in S direction
    pub wrap_s: Token,
    /// Wrap mode in T direction
    pub wrap_t: Token,
    /// Wrap mode in R direction (3D textures)
    pub wrap_r: Token,
    /// Minification filter
    pub min_filter: Token,
    /// Magnification filter
    pub mag_filter: Token,
    /// Enable anisotropic filtering
    pub enable_aniso: bool,
}

impl Default for TextureSamplerParams {
    fn default() -> Self {
        Self {
            wrap_s: Token::new("clamp"),
            wrap_t: Token::new("clamp"),
            wrap_r: Token::new("clamp"),
            min_filter: Token::new("linear"),
            mag_filter: Token::new("linear"),
            enable_aniso: false,
        }
    }
}

/// Named texture handle for binding.
///
/// Provides information necessary to bind textures and create
/// texture accessors in shader code.
#[derive(Debug, Clone)]
pub struct NamedTextureHandle {
    /// Name of the texture parameter
    pub name: Token,
    /// Texture type (2D, 3D, cube, etc.)
    pub texture_type: Token,
    /// GPU texture handle ID (wgpu resource index)
    pub handle: u64,
    /// Texture content hash used for per-batch grouping.
    /// Mirrors C++ HdStTextureHandle hash field (separate from the GPU handle).
    pub hash: u64,
    /// Sampling parameters
    pub sampler_params: TextureSamplerParams,
    /// Texture is bindless
    pub is_bindless: bool,
}

impl NamedTextureHandle {
    /// Create a new named texture handle.
    pub fn new(name: Token, texture_type: Token) -> Self {
        Self {
            name,
            texture_type,
            handle: 0,
            hash: 0,
            sampler_params: TextureSamplerParams::default(),
            is_bindless: false,
        }
    }

    /// Set sampler parameters.
    pub fn set_sampler_params(&mut self, params: TextureSamplerParams) {
        self.sampler_params = params;
    }

    /// Set GPU handle.
    pub fn set_handle(&mut self, handle: u64) {
        self.handle = handle;
    }
}

/// Shader parameter binding.
#[derive(Debug, Clone)]
pub struct ShaderParameter {
    /// Parameter name
    pub name: Token,
    /// Parameter value
    pub value: Value,
    /// Is this a constant (uniform)?
    pub is_constant: bool,
}

impl ShaderParameter {
    /// Create a new shader parameter.
    pub fn new(name: Token, value: Value) -> Self {
        Self {
            name,
            value,
            is_constant: true,
        }
    }

    /// Create a varying parameter.
    pub fn new_varying(name: Token, value: Value) -> Self {
        Self {
            name,
            value,
            is_constant: false,
        }
    }
}

/// Resource context for shader compilation.
///
/// Provides context information available during resource binding
/// and shader code generation.
#[derive(Debug, Default)]
pub struct ResourceContext {
    /// Available texture units
    pub texture_units: Vec<u32>,
    /// Available uniform buffer bindings
    pub buffer_bindings: Vec<u32>,
    /// Feature flags
    pub features: HashMap<Token, bool>,
}

impl ResourceContext {
    /// Create a new resource context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a feature flag.
    pub fn add_feature(&mut self, name: Token, enabled: bool) {
        self.features.insert(name, enabled);
    }

    /// Check if a feature is enabled.
    pub fn has_feature(&self, name: &Token) -> bool {
        self.features.get(name).copied().unwrap_or(false)
    }
}

/// Shader code interface.
///
/// Encapsulates shader source code, parameter bindings, and texture bindings.
/// Implementations provide shader code for different rendering contexts
/// (surface, geometric, lighting, etc.).
///
/// # Shader Stages
///
/// Storm shaders are composed of multiple stages:
/// - Vertex shader
/// - Tessellation control/evaluation (optional)
/// - Geometry shader (optional)
/// - Fragment shader
///
/// # Resource Binding
///
/// Shaders can bind:
/// - Uniform parameters (constants)
/// - Textures with samplers
/// - Buffer resources (vertex, constant buffers)
pub trait HdStShaderCode: fmt::Debug + Send + Sync {
    /// Get a unique identifier for this shader code.
    fn get_id(&self) -> u64;

    /// Get shader source for a specific stage.
    ///
    /// Returns GLSL source code for the requested shader stage.
    /// Empty string if stage is not used.
    fn get_source(&self, stage: ShaderStage) -> String;

    /// Get all shader parameters.
    fn get_params(&self) -> Vec<ShaderParameter>;

    /// Get named texture handles for binding.
    fn get_textures(&self) -> Vec<NamedTextureHandle>;

    /// Add resources from textures to the resource context.
    ///
    /// Called during shader compilation to populate texture bindings.
    fn add_resources_from_textures(&self, _context: &mut ResourceContext) {
        // Default: no-op
    }

    /// Bind shader parameters.
    ///
    /// Updates GPU uniform buffers with current parameter values.
    /// Called before drawing with this shader.
    fn bind_params(&self) {
        // Default: no-op until Hgi integration
    }

    /// Unbind shader resources.
    fn unbind(&self) {
        // Default: no-op
    }

    /// Check if shader code is valid.
    fn is_valid(&self) -> bool {
        true
    }

    /// Get hash for caching.
    fn get_hash(&self) -> u64 {
        self.get_id()
    }
}

/// Shader stage enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    /// Vertex shader stage
    Vertex,
    /// Tessellation control shader
    TessControl,
    /// Tessellation evaluation shader
    TessEval,
    /// Geometry shader
    Geometry,
    /// Fragment (pixel) shader
    Fragment,
    /// Compute shader
    Compute,
}

impl ShaderStage {
    /// Get stage name as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::TessControl => "tess_control",
            Self::TessEval => "tess_eval",
            Self::Geometry => "geometry",
            Self::Fragment => "fragment",
            Self::Compute => "compute",
        }
    }
}

/// Shared pointer to shader code.
pub type HdStShaderCodeSharedPtr = Arc<dyn HdStShaderCode>;

/// Simple shader code implementation for testing.
///
/// Provides basic shader with configurable parameters.
#[derive(Debug)]
pub struct SimpleShaderCode {
    /// Unique ID
    id: u64,
    /// Shader parameters
    params: Vec<ShaderParameter>,
    /// Texture handles
    textures: Vec<NamedTextureHandle>,
    /// Source code per stage
    sources: HashMap<ShaderStage, String>,
}

impl SimpleShaderCode {
    /// Create a new simple shader code.
    pub fn new(id: u64) -> Self {
        Self {
            id,
            params: Vec::new(),
            textures: Vec::new(),
            sources: HashMap::new(),
        }
    }

    /// Add a parameter.
    pub fn add_param(&mut self, param: ShaderParameter) {
        self.params.push(param);
    }

    /// Add a texture.
    pub fn add_texture(&mut self, texture: NamedTextureHandle) {
        self.textures.push(texture);
    }

    /// Set source for a stage.
    pub fn set_source(&mut self, stage: ShaderStage, source: String) {
        self.sources.insert(stage, source);
    }
}

impl HdStShaderCode for SimpleShaderCode {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        self.sources.get(&stage).cloned().unwrap_or_default()
    }

    fn get_params(&self) -> Vec<ShaderParameter> {
        self.params.clone()
    }

    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        self.textures.clone()
    }

    fn is_valid(&self) -> bool {
        !self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_stage() {
        assert_eq!(ShaderStage::Vertex.as_str(), "vertex");
        assert_eq!(ShaderStage::Fragment.as_str(), "fragment");
    }

    #[test]
    fn test_texture_handle() {
        let mut handle = NamedTextureHandle::new(Token::new("diffuse"), Token::new("2D"));
        assert_eq!(handle.name, Token::new("diffuse"));
        assert_eq!(handle.handle, 0);

        handle.set_handle(42);
        assert_eq!(handle.handle, 42);
    }

    #[test]
    fn test_shader_parameter() {
        let param = ShaderParameter::new(Token::new("roughness"), Value::from(0.5f32));
        assert_eq!(param.name, Token::new("roughness"));
        assert!(param.is_constant);

        let varying = ShaderParameter::new_varying(Token::new("color"), Value::from(1.0f32));
        assert!(!varying.is_constant);
    }

    #[test]
    fn test_resource_context() {
        let mut ctx = ResourceContext::new();
        assert!(!ctx.has_feature(&Token::new("normals")));

        ctx.add_feature(Token::new("normals"), true);
        assert!(ctx.has_feature(&Token::new("normals")));
    }

    #[test]
    fn test_simple_shader_code() {
        let mut shader = SimpleShaderCode::new(123);
        assert_eq!(shader.get_id(), 123);
        assert!(!shader.is_valid());

        shader.set_source(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );
        assert!(shader.is_valid());
        assert!(!shader.get_source(ShaderStage::Vertex).is_empty());
        assert!(shader.get_source(ShaderStage::Fragment).is_empty());
    }

    #[test]
    fn test_shader_params() {
        let mut shader = SimpleShaderCode::new(1);
        shader.add_param(ShaderParameter::new(
            Token::new("color"),
            Value::from(1.0f32),
        ));

        let params = shader.get_params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, Token::new("color"));
    }

    #[test]
    fn test_shader_textures() {
        let mut shader = SimpleShaderCode::new(1);
        shader.add_texture(NamedTextureHandle::new(
            Token::new("baseColor"),
            Token::new("2D"),
        ));

        let textures = shader.get_textures();
        assert_eq!(textures.len(), 1);
        assert_eq!(textures[0].name, Token::new("baseColor"));
    }
}
