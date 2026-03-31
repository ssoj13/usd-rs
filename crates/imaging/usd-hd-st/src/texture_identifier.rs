#![allow(dead_code)]

//! HdStTextureIdentifier - Unique identifier for texture resources.
//!
//! Identifies a texture by file path, optional subtexture parameters,
//! fallback value, and texture type. Two textures with the same identifier
//! share GPU resources.
//!
//! Port of pxr/imaging/hdSt/textureIdentifier.h

use std::hash::{Hash, Hasher};
use usd_sdf::AssetPath;
use usd_tf::Token;
use usd_vt::Value as VtValue;

/// Subtexture identifier for addressing sub-resources within a texture file.
///
/// Identifies a specific layer, frame, grid, or other sub-resource.
/// E.g. an EXR layer, a VDB grid name, or a movie frame index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubtextureIdentifier {
    /// Named layer (e.g. EXR layer name)
    Layer(Token),
    /// VDB field name
    Field(Token),
    /// UDIM identifier (base tile number)
    Udim(u32),
    /// Ptex face index
    Ptex,
    /// Dynamic texture (procedural)
    Dynamic(Token),
}

impl SubtextureIdentifier {
    /// Create a layer-based subtexture identifier.
    pub fn layer(name: Token) -> Self {
        Self::Layer(name)
    }

    /// Create a VDB field subtexture identifier.
    pub fn field(name: Token) -> Self {
        Self::Field(name)
    }

    /// Create a UDIM subtexture identifier.
    pub fn udim(base_tile: u32) -> Self {
        Self::Udim(base_tile)
    }

    /// Whether this is a ptex subtexture.
    pub fn is_ptex(&self) -> bool {
        matches!(self, Self::Ptex)
    }

    /// Whether this is a UDIM subtexture.
    pub fn is_udim(&self) -> bool {
        matches!(self, Self::Udim(_))
    }

    /// Whether this is a field (volume) subtexture.
    pub fn is_field(&self) -> bool {
        matches!(self, Self::Field(_))
    }

    /// Get premultiply-alpha hint. Only UV textures may premultiply.
    pub fn premultiply_alpha(&self) -> bool {
        false
    }
}

/// Texture identifier combining file path, subtexture, and fallback.
///
/// Primary key for texture resources in Storm. Two textures with the same
/// identifier share GPU resources.
///
/// Port of HdStTextureIdentifier from pxr/imaging/hdSt/textureIdentifier.h
#[derive(Debug, Clone)]
pub struct HdStTextureIdentifier {
    /// Asset path to the texture file
    file_path: AssetPath,

    /// Optional subtexture identifier
    subtexture_id: Option<SubtextureIdentifier>,

    /// Fallback value used if loading from file_path fails
    fallback: Option<VtValue>,

    /// If true, skip reading file_path and use fallback directly
    default_to_fallback: bool,
}

impl HdStTextureIdentifier {
    /// Create a texture identifier with all parameters.
    pub fn new(
        file_path: AssetPath,
        subtexture_id: Option<SubtextureIdentifier>,
        fallback: Option<VtValue>,
        default_to_fallback: bool,
    ) -> Self {
        Self {
            file_path,
            subtexture_id,
            fallback,
            default_to_fallback,
        }
    }

    /// Create identifier from file path only (most common case).
    pub fn from_path(file_path: AssetPath) -> Self {
        Self::new(file_path, None, None, false)
    }

    /// Create identifier with subtexture selection.
    pub fn with_subtexture(file_path: AssetPath, sub: SubtextureIdentifier) -> Self {
        Self::new(file_path, Some(sub), None, false)
    }

    /// Get the file path.
    pub fn file_path(&self) -> &AssetPath {
        &self.file_path
    }

    /// Get the subtexture identifier if present.
    pub fn subtexture_id(&self) -> Option<&SubtextureIdentifier> {
        self.subtexture_id.as_ref()
    }

    /// Get the fallback value.
    pub fn fallback(&self) -> Option<&VtValue> {
        self.fallback.as_ref()
    }

    /// Whether to skip file loading and use fallback directly.
    pub fn should_default_to_fallback(&self) -> bool {
        self.default_to_fallback
    }

    /// Check if this identifier has a subtexture specifier.
    pub fn has_subtexture(&self) -> bool {
        self.subtexture_id.is_some()
    }

    /// Check if this is a valid identifier (non-empty path or has fallback).
    pub fn is_valid(&self) -> bool {
        !self.file_path.get_asset_path().is_empty() || self.fallback.is_some()
    }
}

impl PartialEq for HdStTextureIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.file_path == other.file_path
            && self.subtexture_id == other.subtexture_id
            && self.default_to_fallback == other.default_to_fallback
    }
}

impl Eq for HdStTextureIdentifier {}

impl Hash for HdStTextureIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file_path.hash(state);
        self.subtexture_id.hash(state);
        self.default_to_fallback.hash(state);
    }
}

impl Default for HdStTextureIdentifier {
    fn default() -> Self {
        Self {
            file_path: AssetPath::new(""),
            subtexture_id: None,
            fallback: None,
            default_to_fallback: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtexture_variants() {
        let layer = SubtextureIdentifier::layer(Token::new("beauty"));
        assert!(!layer.is_ptex());
        assert!(!layer.is_udim());

        let udim = SubtextureIdentifier::udim(1001);
        assert!(udim.is_udim());

        let field = SubtextureIdentifier::field(Token::new("density"));
        assert!(field.is_field());
    }

    #[test]
    fn test_identifier_simple() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("tex/diffuse.png"));
        assert!(id.is_valid());
        assert!(!id.has_subtexture());
        assert!(!id.should_default_to_fallback());
    }

    #[test]
    fn test_identifier_with_subtexture() {
        let id = HdStTextureIdentifier::with_subtexture(
            AssetPath::new("render.exr"),
            SubtextureIdentifier::layer(Token::new("beauty")),
        );
        assert!(id.has_subtexture());
        assert!(id.subtexture_id().is_some());
    }

    #[test]
    fn test_identifier_hash_eq() {
        use std::collections::HashMap;

        let id1 = HdStTextureIdentifier::from_path(AssetPath::new("a.png"));
        let id2 = HdStTextureIdentifier::from_path(AssetPath::new("b.png"));
        let id3 = HdStTextureIdentifier::from_path(AssetPath::new("a.png"));

        let mut map = HashMap::new();
        map.insert(id1.clone(), 1);
        map.insert(id2, 2);

        assert_eq!(map.get(&id3), Some(&1));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_default_invalid() {
        let id = HdStTextureIdentifier::default();
        assert!(!id.is_valid());
    }
}
