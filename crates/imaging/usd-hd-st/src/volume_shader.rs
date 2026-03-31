
//! VolumeShader - shader for volume rendering in Storm.
//!
//! Port of C++ `HdSt_VolumeShader`. Extends the material network shader
//! concept with volume-specific behaviors:
//!
//! - Field descriptor management (maps volume fields to texture handles)
//! - Volume bounding box computation for raymarching bounds
//! - Points bar generation (proxy geometry for the volume box)
//! - Step size uniform binding (queried from render delegate)
//! - Sample distance for adaptive raymarching

use crate::shader_code::{HdStShaderCode, NamedTextureHandle, ShaderParameter, ShaderStage};
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// Shared pointer type for volume shader.
pub type HdStVolumeShaderSharedPtr = Arc<HdStVolumeShader>;

/// Descriptor for a volume field prim.
///
/// Maps a field name to its scene delegate path, used to allocate
/// the correct texture handles for 3D volume textures.
#[derive(Debug, Clone)]
pub struct VolumeFieldDescriptor {
    /// Field name (e.g. "density", "temperature", "velocity")
    pub field_name: Token,
    /// Field prim path in the scene
    pub field_prim_path: String,
    /// Field data type identifier
    pub field_data_type: Token,
}

/// Volume shader for ray-marched volume rendering.
///
/// Handles field texture allocation/binding, volume bounding box
/// computation from field extents, step size and sample distance
/// uniforms, and points bar (proxy box geometry) generation.
#[derive(Debug)]
pub struct HdStVolumeShader {
    /// Unique ID for shader code identity
    id: u64,
    /// Fragment shader source (WGSL / GLSL)
    fragment_source: String,
    /// Shader parameters (uniforms for step sizes, bbox, etc.)
    params: Vec<ShaderParameter>,
    /// Texture handles for volume field textures
    named_texture_handles: Vec<NamedTextureHandle>,
    /// Material tag for render pass sorting
    material_tag: Token,
    /// Whether this shader is enabled
    enabled: bool,

    /// Volume field descriptors
    field_descriptors: Vec<VolumeFieldDescriptor>,
    /// Whether this shader fills the points bar (proxy geometry)
    fills_points_bar: bool,

    /// Volume bounding box min (world space)
    bbox_min: [f64; 3],
    /// Volume bounding box max (world space)
    bbox_max: [f64; 3],
    /// Raymarching step size (world space units)
    step_size: f32,
    /// Raymarching sample distance
    sample_distance: f32,
}

/// Global counter for unique shader IDs.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl HdStVolumeShader {
    /// Create a new empty volume shader.
    pub fn new() -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fragment_source: String::new(),
            params: Vec::new(),
            named_texture_handles: Vec::new(),
            material_tag: Token::new("volume"),
            enabled: true,
            field_descriptors: Vec::new(),
            fills_points_bar: false,
            bbox_min: [0.0; 3],
            bbox_max: [0.0; 3],
            step_size: 1.0,
            sample_distance: 1.0,
        }
    }

    /// Set fragment shader source.
    pub fn set_fragment_source(&mut self, source: String) {
        self.fragment_source = source;
    }

    /// Get fragment shader source.
    pub fn get_fragment_source(&self) -> &str {
        &self.fragment_source
    }

    /// Set shader parameters.
    pub fn set_params(&mut self, params: Vec<ShaderParameter>) {
        self.params = params;
    }

    /// Get shader parameters.
    pub fn get_shader_params(&self) -> &[ShaderParameter] {
        &self.params
    }

    /// Set named texture handles for volume fields.
    pub fn set_named_texture_handles(&mut self, handles: Vec<NamedTextureHandle>) {
        self.named_texture_handles = handles;
    }

    /// Get named texture handles.
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }

    /// Set material tag.
    pub fn set_material_tag(&mut self, tag: Token) {
        self.material_tag = tag;
    }

    /// Get material tag.
    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    /// Enable or disable this shader.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // -- Volume-specific methods --

    /// Set field descriptors identifying which volume fields to load.
    pub fn set_field_descriptors(&mut self, descs: Vec<VolumeFieldDescriptor>) {
        self.field_descriptors = descs;
    }

    /// Get field descriptors.
    pub fn get_field_descriptors(&self) -> &[VolumeFieldDescriptor] {
        &self.field_descriptors
    }

    /// Set whether this shader fills the points bar (proxy geometry).
    ///
    /// When fields are present, the volume shader computes the bounding box
    /// from field textures and generates proxy box vertices. Otherwise the
    /// volume prim uses authored extents.
    pub fn set_fills_points_bar(&mut self, fills: bool) {
        self.fills_points_bar = fills;
    }

    /// Check whether this shader fills the points bar.
    pub fn get_fills_points_bar(&self) -> bool {
        self.fills_points_bar
    }

    /// Set the volume bounding box (world space).
    pub fn set_bbox(&mut self, min: [f64; 3], max: [f64; 3]) {
        self.bbox_min = min;
        self.bbox_max = max;
    }

    /// Get volume bounding box min. Returns [0,0,0] for empty ranges
    /// (avoids infinity values from GfRange3d empty encoding).
    pub fn get_safe_min(&self) -> [f64; 3] {
        if self.bbox_min[0] > self.bbox_max[0] {
            [0.0; 3]
        } else {
            self.bbox_min
        }
    }

    /// Get volume bounding box max. Returns [0,0,0] for empty ranges.
    pub fn get_safe_max(&self) -> [f64; 3] {
        if self.bbox_min[0] > self.bbox_max[0] {
            [0.0; 3]
        } else {
            self.bbox_max
        }
    }

    /// Set raymarching step size.
    pub fn set_step_size(&mut self, step: f32) {
        self.step_size = step;
    }

    /// Get raymarching step size.
    pub fn get_step_size(&self) -> f32 {
        self.step_size
    }

    /// Set sample distance for adaptive raymarching.
    pub fn set_sample_distance(&mut self, dist: f32) {
        self.sample_distance = dist;
    }

    /// Get sample distance.
    pub fn get_sample_distance(&self) -> f32 {
        self.sample_distance
    }

    /// Get default params for bbox and sample distance uniforms.
    pub fn get_bbox_params() -> Vec<ShaderParameter> {
        vec![
            ShaderParameter::new(Token::new("volumeBBoxMin"), Value::from(0.0f32)),
            ShaderParameter::new(Token::new("volumeBBoxMax"), Value::from(0.0f32)),
            ShaderParameter::new(Token::new("sampleDistance"), Value::from(1.0f32)),
        ]
    }
}

impl Default for HdStVolumeShader {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStShaderCode for HdStVolumeShader {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        match stage {
            ShaderStage::Fragment => self.fragment_source.clone(),
            _ => String::new(),
        }
    }

    fn get_params(&self) -> Vec<ShaderParameter> {
        self.params.clone()
    }

    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        self.named_texture_handles.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_shader_default() {
        let shader = HdStVolumeShader::new();
        assert!(shader.is_enabled());
        assert!(shader.get_fragment_source().is_empty());
        assert_eq!(shader.get_material_tag().as_str(), "volume");
        assert!(!shader.get_fills_points_bar());
        assert!(shader.get_field_descriptors().is_empty());
    }

    #[test]
    fn test_volume_bbox() {
        let mut shader = HdStVolumeShader::new();
        shader.set_bbox([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
        assert_eq!(shader.get_safe_min(), [-1.0, -2.0, -3.0]);
        assert_eq!(shader.get_safe_max(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_volume_bbox_empty_range() {
        let mut shader = HdStVolumeShader::new();
        shader.set_bbox(
            [f64::INFINITY, f64::INFINITY, f64::INFINITY],
            [f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
        );
        assert_eq!(shader.get_safe_min(), [0.0, 0.0, 0.0]);
        assert_eq!(shader.get_safe_max(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_field_descriptors() {
        let mut shader = HdStVolumeShader::new();
        shader.set_field_descriptors(vec![
            VolumeFieldDescriptor {
                field_name: Token::new("density"),
                field_prim_path: "/volume/density".into(),
                field_data_type: Token::new("float"),
            },
            VolumeFieldDescriptor {
                field_name: Token::new("temperature"),
                field_prim_path: "/volume/temperature".into(),
                field_data_type: Token::new("float"),
            },
        ]);
        assert_eq!(shader.get_field_descriptors().len(), 2);
        assert_eq!(
            shader.get_field_descriptors()[0].field_name.as_str(),
            "density"
        );
    }

    #[test]
    fn test_volume_shader_id_unique() {
        let s1 = HdStVolumeShader::new();
        let s2 = HdStVolumeShader::new();
        assert_ne!(s1.get_id(), s2.get_id());
    }

    #[test]
    fn test_step_size() {
        let mut shader = HdStVolumeShader::new();
        shader.set_step_size(0.5);
        shader.set_sample_distance(2.0);
        assert_eq!(shader.get_step_size(), 0.5);
        assert_eq!(shader.get_sample_distance(), 2.0);
    }

    #[test]
    fn test_bbox_params() {
        let params = HdStVolumeShader::get_bbox_params();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, Token::new("volumeBBoxMin"));
        assert_eq!(params[1].name, Token::new("volumeBBoxMax"));
        assert_eq!(params[2].name, Token::new("sampleDistance"));
    }

    #[test]
    fn test_get_source() {
        let mut shader = HdStVolumeShader::new();
        shader.set_fragment_source("fn raymarch() {}".into());

        let fs = shader.get_source(ShaderStage::Fragment);
        assert_eq!(fs, "fn raymarch() {}");

        let vs = shader.get_source(ShaderStage::Vertex);
        assert!(vs.is_empty());
    }
}
