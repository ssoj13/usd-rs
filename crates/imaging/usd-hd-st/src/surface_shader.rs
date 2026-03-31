
//! Surface shader interface for Storm.
//!
//! Provides the material/surface shader component that contributes:
//! - Material parameter bindings (uniforms, textures)
//! - Surface evaluation shader code (WGSL)
//! - Texture handle management
//!
//! This is the shader code that evaluates the material surface
//! (e.g. UsdPreviewSurface or MaterialX standard_surface).

use crate::material_param::{HdStMaterialParam, FallbackValue};
use crate::shader_code::{HdStShaderCode, NamedTextureHandle};
use crate::texture_handle::HdStTextureHandle;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_tf::Token;

/// Shared pointer type.
pub type HdStSurfaceShaderSharedPtr = Arc<HdStSurfaceShader>;

/// Surface shader for material evaluation.
///
/// Wraps the shader code that evaluates a material surface.
/// Contains:
/// - WGSL source for fragment shader contributions
/// - Material parameter list (with fallback values)
/// - Texture bindings
/// - Metadata (material tag, etc.)
#[derive(Debug)]
pub struct HdStSurfaceShader {
    /// Fragment shader source (WGSL)
    fragment_source: String,
    /// Displacement shader source (WGSL, optional)
    displacement_source: String,
    /// Material parameters
    params: Vec<HdStMaterialParam>,
    /// Named texture handles
    named_texture_handles: Vec<NamedTextureHandle>,
    /// Material tag for render pass sorting
    material_tag: Token,
    /// Whether this shader is enabled
    enabled: bool,
    /// Cached hash
    hash: u64,
    hash_valid: bool,
}

impl HdStSurfaceShader {
    /// Create a new empty surface shader.
    pub fn new() -> Self {
        Self {
            fragment_source: String::new(),
            displacement_source: String::new(),
            params: Vec::new(),
            named_texture_handles: Vec::new(),
            material_tag: Token::new("defaultMaterialTag"),
            enabled: true,
            hash: 0,
            hash_valid: false,
        }
    }

    /// Create with source code and parameters.
    pub fn with_source(
        fragment_source: String,
        params: Vec<HdStMaterialParam>,
        material_tag: Token,
    ) -> Self {
        Self {
            fragment_source,
            displacement_source: String::new(),
            params,
            named_texture_handles: Vec::new(),
            material_tag,
            enabled: true,
            hash: 0,
            hash_valid: false,
        }
    }

    /// Set fragment shader source.
    pub fn set_fragment_source(&mut self, source: String) {
        self.fragment_source = source;
        self.hash_valid = false;
    }

    /// Get fragment shader source.
    pub fn get_fragment_source(&self) -> &str {
        &self.fragment_source
    }

    /// Set displacement shader source.
    pub fn set_displacement_source(&mut self, source: String) {
        self.displacement_source = source;
        self.hash_valid = false;
    }

    /// Get displacement shader source.
    pub fn get_displacement_source(&self) -> &str {
        &self.displacement_source
    }

    /// Set material parameters.
    pub fn set_params(&mut self, params: Vec<HdStMaterialParam>) {
        self.params = params;
        self.hash_valid = false;
    }

    /// Get material parameters.
    pub fn get_params(&self) -> &[HdStMaterialParam] {
        &self.params
    }

    /// Set named texture handles.
    pub fn set_named_texture_handles(&mut self, handles: Vec<NamedTextureHandle>) {
        self.named_texture_handles = handles;
        self.hash_valid = false;
    }

    /// Get named texture handles.
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }

    /// Set material tag.
    pub fn set_material_tag(&mut self, tag: Token) {
        self.material_tag = tag;
        self.hash_valid = false;
    }

    /// Get material tag.
    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    /// Enable or disable this shader.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if this shader is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Reload shader source (e.g. after hot-reload).
    pub fn reload(&mut self) {
        self.hash_valid = false;
    }
}

impl Default for HdStSurfaceShader {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStShaderCode for HdStSurfaceShader {
    fn compute_hash(&self) -> u64 {
        if self.hash_valid {
            return self.hash;
        }

        let mut hasher = DefaultHasher::new();
        self.fragment_source.hash(&mut hasher);
        self.displacement_source.hash(&mut hasher);
        self.material_tag.hash(&mut hasher);
        self.enabled.hash(&mut hasher);

        for p in &self.params {
            p.name.hash(&mut hasher);
            p.param_type.hash(&mut hasher);
        }

        hasher.finish()
    }

    fn get_source(&self, shader_stage: &Token) -> String {
        let stage = shader_stage.as_str();
        match stage {
            "fragmentShader" => self.fragment_source.clone(),
            "displacementShader" => self.displacement_source.clone(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_shader_default() {
        let shader = HdStSurfaceShader::new();
        assert!(shader.is_enabled());
        assert!(shader.get_fragment_source().is_empty());
        assert_eq!(shader.get_material_tag().as_str(), "defaultMaterialTag");
    }

    #[test]
    fn test_surface_shader_with_source() {
        let params = vec![HdStMaterialParam::fallback(
            Token::new("diffuseColor"),
            FallbackValue::Vec3([0.18, 0.18, 0.18]),
        )];

        let shader = HdStSurfaceShader::with_source(
            "fn surface() -> vec4<f32> { return vec4(1.0); }".into(),
            params,
            Token::new("translucent"),
        );

        assert_eq!(shader.get_params().len(), 1);
        assert_eq!(shader.get_material_tag().as_str(), "translucent");

        let fs = shader.get_source(&Token::new("fragmentShader"));
        assert!(fs.contains("surface"));
    }

    #[test]
    fn test_surface_shader_hash() {
        let s1 = HdStSurfaceShader::with_source(
            "fn a() {}".into(),
            vec![],
            Token::new("tag1"),
        );
        let s2 = HdStSurfaceShader::with_source(
            "fn b() {}".into(),
            vec![],
            Token::new("tag2"),
        );

        assert_ne!(s1.compute_hash(), s2.compute_hash());
    }

    #[test]
    fn test_enable_disable() {
        let mut shader = HdStSurfaceShader::new();
        assert!(shader.is_enabled());
        shader.set_enabled(false);
        assert!(!shader.is_enabled());
    }
}
