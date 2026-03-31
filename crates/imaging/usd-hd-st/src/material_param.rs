
//! HdSt_MaterialParam - Material parameter descriptor.
//!
//! Describes a single material parameter: its type, name, fallback value,
//! texture connections, primvar redirects, and swizzle configuration.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Texture type for material parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdStTextureType {
    /// Standard 2D UV texture
    Uv,
    /// Ptex texture
    Ptex,
    /// UDIM tiled texture
    Udim,
    /// Field (volume) texture
    Field,
    /// Cubemap (environment) texture
    Cubemap,
}

impl Default for HdStTextureType {
    fn default() -> Self {
        Self::Uv
    }
}

/// Kind of material parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamType {
    /// Shader fallback value (not connected to primvar or texture)
    Fallback,
    /// Connected to a texture
    Texture,
    /// Primvar redirect: reads primvar by (potentially different) name,
    /// falling back to a default value if not present
    PrimvarRedirect,
    /// Field redirect: reads from a field texture by name
    FieldRedirect,
    /// Additional primvar needed by material (not connected to an input)
    AdditionalPrimvar,
    /// Connected to a transform2d node
    Transform2d,
}

impl Default for ParamType {
    fn default() -> Self {
        Self::Fallback
    }
}

/// Fallback value for a material parameter.
#[derive(Debug, Clone)]
pub enum FallbackValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
    String(String),
    Matrix(Box<[[f32; 4]; 4]>),
    /// No value
    None,
}

impl Default for FallbackValue {
    fn default() -> Self {
        Self::None
    }
}

impl FallbackValue {
    /// Get the WGSL type string for this value.
    pub fn wgsl_type(&self) -> &'static str {
        match self {
            Self::Float(_) => "f32",
            Self::Vec2(_) => "vec2<f32>",
            Self::Vec3(_) => "vec3<f32>",
            Self::Vec4(_) => "vec4<f32>",
            Self::Int(_) => "i32",
            Self::Bool(_) => "bool",
            Self::String(_) => "u32", // string handle
            Self::Matrix(_) => "mat4x4<f32>",
            Self::None => "f32",
        }
    }
}

/// Material parameter descriptor.
///
/// Describes a single parameter of a Storm material:
/// - Its semantic type (fallback, texture, primvar redirect, etc.)
/// - Name used in shader code (HdGet_<name>)
/// - Fallback value when the connection is missing
/// - Sampler coordinates (primvar names for texture lookup)
/// - Texture type (UV, Ptex, UDIM, etc.)
/// - Swizzle mask for channel selection
#[derive(Debug, Clone)]
pub struct HdStMaterialParam {
    /// Kind of parameter
    pub param_type: ParamType,
    /// Parameter name (used as HdGet_<name>() in shader)
    pub name: Token,
    /// Fallback value when not connected
    pub fallback_value: FallbackValue,
    /// Sampler coordinate primvar names (for texture lookups)
    pub sampler_coords: Vec<Token>,
    /// Texture type
    pub texture_type: HdStTextureType,
    /// Channel swizzle (e.g. "rgb", "a", "rrr")
    pub swizzle: String,
    /// Whether texture is pre-multiplied alpha
    pub is_premultiplied: bool,
    /// Size of texture array (0 = single texture, >0 = array of textures)
    pub array_of_textures_size: usize,
}

impl HdStMaterialParam {
    /// Create a fallback parameter with a default value.
    pub fn fallback(name: Token, value: FallbackValue) -> Self {
        Self {
            param_type: ParamType::Fallback,
            name,
            fallback_value: value,
            sampler_coords: Vec::new(),
            texture_type: HdStTextureType::Uv,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a texture parameter.
    pub fn texture(name: Token, uv_primvar: Token) -> Self {
        Self {
            param_type: ParamType::Texture,
            name,
            fallback_value: FallbackValue::Vec4([0.0, 0.0, 0.0, 1.0]),
            sampler_coords: vec![uv_primvar],
            texture_type: HdStTextureType::Uv,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a primvar redirect parameter.
    pub fn primvar_redirect(name: Token, primvar_name: Token, fallback: FallbackValue) -> Self {
        Self {
            param_type: ParamType::PrimvarRedirect,
            name,
            fallback_value: fallback,
            sampler_coords: vec![primvar_name],
            texture_type: HdStTextureType::Uv,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a field redirect parameter.
    pub fn field_redirect(name: Token, field_name: Token, fallback: FallbackValue) -> Self {
        Self {
            param_type: ParamType::FieldRedirect,
            name,
            fallback_value: fallback,
            sampler_coords: vec![field_name],
            texture_type: HdStTextureType::Field,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create an additional primvar parameter.
    pub fn additional_primvar(name: Token) -> Self {
        Self {
            param_type: ParamType::AdditionalPrimvar,
            name,
            fallback_value: FallbackValue::None,
            sampler_coords: Vec::new(),
            texture_type: HdStTextureType::Uv,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    /// Create a transform2d parameter.
    pub fn transform2d(name: Token, uv_primvar: Token) -> Self {
        Self {
            param_type: ParamType::Transform2d,
            name,
            fallback_value: FallbackValue::None,
            sampler_coords: vec![uv_primvar],
            texture_type: HdStTextureType::Uv,
            swizzle: String::new(),
            is_premultiplied: false,
            array_of_textures_size: 0,
        }
    }

    // --- Query helpers ---

    pub fn is_texture(&self) -> bool {
        self.param_type == ParamType::Texture
    }

    pub fn is_primvar_redirect(&self) -> bool {
        self.param_type == ParamType::PrimvarRedirect
    }

    pub fn is_field_redirect(&self) -> bool {
        self.param_type == ParamType::FieldRedirect
    }

    pub fn is_fallback(&self) -> bool {
        self.param_type == ParamType::Fallback
    }

    pub fn is_additional_primvar(&self) -> bool {
        self.param_type == ParamType::AdditionalPrimvar
    }

    pub fn is_transform2d(&self) -> bool {
        self.param_type == ParamType::Transform2d
    }

    pub fn is_array_of_textures(&self) -> bool {
        self.is_texture() && self.array_of_textures_size > 0
    }

    /// Get the WGSL data type for this parameter's value.
    pub fn get_wgsl_type(&self) -> &str {
        self.fallback_value.wgsl_type()
    }
}

/// Compute a structural hash for a list of material params.
///
/// Uses name, texture type, primvar names -- but NOT the fallback value
/// (which may change without requiring shader recompilation).
pub fn compute_params_hash(params: &[HdStMaterialParam]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for p in params {
        p.param_type.hash(&mut hasher);
        std::hash::Hash::hash(&p.name, &mut hasher);
        p.texture_type.hash(&mut hasher);
        for coord in &p.sampler_coords {
            std::hash::Hash::hash(coord, &mut hasher);
        }
        p.swizzle.hash(&mut hasher);
        p.is_premultiplied.hash(&mut hasher);
        p.array_of_textures_size.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_param() {
        let p = HdStMaterialParam::fallback(
            Token::new("diffuseColor"),
            FallbackValue::Vec3([0.18, 0.18, 0.18]),
        );
        assert!(p.is_fallback());
        assert!(!p.is_texture());
        assert_eq!(p.get_wgsl_type(), "vec3<f32>");
    }

    #[test]
    fn test_texture_param() {
        let p = HdStMaterialParam::texture(
            Token::new("diffuseTexture"),
            Token::new("st"),
        );
        assert!(p.is_texture());
        assert_eq!(p.sampler_coords.len(), 1);
        assert_eq!(p.sampler_coords[0].as_str(), "st");
    }

    #[test]
    fn test_params_hash_stability() {
        let params = vec![
            HdStMaterialParam::fallback(Token::new("a"), FallbackValue::Float(1.0)),
            HdStMaterialParam::texture(Token::new("b"), Token::new("st")),
        ];
        let h1 = compute_params_hash(&params);
        let h2 = compute_params_hash(&params);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_array_of_textures() {
        let mut p = HdStMaterialParam::texture(Token::new("tex"), Token::new("st"));
        assert!(!p.is_array_of_textures());
        p.array_of_textures_size = 4;
        assert!(p.is_array_of_textures());
    }
}
