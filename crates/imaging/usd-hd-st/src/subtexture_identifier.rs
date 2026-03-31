#![allow(dead_code)]

//! Full subtexture identifier system for Storm textures.
//!
//! Provides detailed subtexture identification beyond the simple enum
//! in texture_identifier.rs. Covers asset UV, UDIM, Ptex, field,
//! dynamic UV, and dynamic cubemap subtexture types with per-type
//! metadata (flip, premultiply, color space, field name/index, etc.).
//!
//! Port of pxr/imaging/hdSt/subtextureIdentifier.h

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Trait for subtexture identifier types.
///
/// Each variant provides a unique hash for deduplication in registries.
///
/// Port of HdStSubtextureIdentifier base class.
pub trait HdStSubtextureIdentifierTrait: std::fmt::Debug + Send + Sync {
    /// Compute unique hash for this subtexture identifier.
    fn sub_hash(&self) -> u64;

    /// Clone into a boxed trait object.
    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait>;
}

// ---------------------------------------------------------------------------
// HdStAssetUvSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Subtexture identifier for asset-backed UV textures.
///
/// Controls vertical flip, alpha premultiplication, and source color space.
/// Flip allows supporting both legacy HwUvTexture_1 (flip=true) and
/// UsdUvTexture (flip=false) conventions.
///
/// Port of HdStAssetUvSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStAssetUvSubtextureId {
    /// Flip texture vertically on load
    pub flip_vertically: bool,
    /// Premultiply RGB by alpha on load
    pub premultiply_alpha: bool,
    /// Source color space (e.g. "sRGB", "raw", "auto")
    pub source_color_space: Token,
}

impl HdStAssetUvSubtextureId {
    pub fn new(flip_vertically: bool, premultiply_alpha: bool, source_color_space: Token) -> Self {
        Self {
            flip_vertically,
            premultiply_alpha,
            source_color_space,
        }
    }
}

impl HdStSubtextureIdentifierTrait for HdStAssetUvSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "AssetUv".hash(&mut h);
        self.hash(&mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// HdStFieldBaseSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Base subtexture identifier for volume field grids.
///
/// Identifies a specific grid by field name and index within a volume
/// field file (e.g. OpenVDB). Parallels FieldBase in usdVol.
///
/// Port of HdStFieldBaseSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStFieldSubtextureId {
    /// Field/grid name (e.g. "density", "temperature")
    pub field_name: Token,
    /// Field index (for files with multiple grids of the same name)
    pub field_index: i32,
}

impl HdStFieldSubtextureId {
    pub fn new(field_name: Token, field_index: i32) -> Self {
        Self {
            field_name,
            field_index,
        }
    }
}

impl HdStSubtextureIdentifierTrait for HdStFieldSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "Field".hash(&mut h);
        self.hash(&mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// HdStPtexSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Subtexture identifier for Ptex textures.
///
/// Controls alpha premultiplication on load.
///
/// Port of HdStPtexSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStPtexSubtextureId {
    /// Premultiply RGB by alpha on load
    pub premultiply_alpha: bool,
}

impl HdStPtexSubtextureId {
    pub fn new(premultiply_alpha: bool) -> Self {
        Self { premultiply_alpha }
    }
}

impl HdStSubtextureIdentifierTrait for HdStPtexSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "Ptex".hash(&mut h);
        self.hash(&mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// HdStUdimSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Subtexture identifier for UDIM tiled textures.
///
/// Controls alpha premultiplication and source color space.
///
/// Port of HdStUdimSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStUdimSubtextureId {
    /// Premultiply RGB by alpha on load
    pub premultiply_alpha: bool,
    /// Source color space
    pub source_color_space: Token,
}

impl HdStUdimSubtextureId {
    pub fn new(premultiply_alpha: bool, source_color_space: Token) -> Self {
        Self {
            premultiply_alpha,
            source_color_space,
        }
    }
}

impl HdStSubtextureIdentifierTrait for HdStUdimSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "Udim".hash(&mut h);
        self.hash(&mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// HdStDynamicUvSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Subtexture identifier for dynamic (client-managed) UV textures.
///
/// Tags a texture as dynamically populated by an external client
/// rather than loaded from a file by Storm. Used for AOVs, procedural
/// textures, and render targets.
///
/// Port of HdStDynamicUvSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStDynamicUvSubtextureId {
    /// Optional tag for distinguishing dynamic textures
    pub tag: Token,
}

impl HdStDynamicUvSubtextureId {
    pub fn new() -> Self {
        Self {
            tag: Token::default(),
        }
    }

    /// Create with a custom tag.
    pub fn with_tag(tag: Token) -> Self {
        Self { tag }
    }
}

impl Default for HdStDynamicUvSubtextureId {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStSubtextureIdentifierTrait for HdStDynamicUvSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "DynamicUv".hash(&mut h);
        Hash::hash(&self.tag, &mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// HdStDynamicCubemapSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Subtexture identifier for dynamic (client-managed) cubemap textures.
///
/// Tags a cubemap texture as dynamically populated by an external client.
///
/// Port of HdStDynamicCubemapSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdStDynamicCubemapSubtextureId {
    /// Optional tag for distinguishing dynamic cubemaps
    pub tag: Token,
}

impl HdStDynamicCubemapSubtextureId {
    pub fn new() -> Self {
        Self {
            tag: Token::default(),
        }
    }
}

impl Default for HdStDynamicCubemapSubtextureId {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStSubtextureIdentifierTrait for HdStDynamicCubemapSubtextureId {
    fn sub_hash(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        "DynamicCubemap".hash(&mut h);
        Hash::hash(&self.tag, &mut h);
        h.finish()
    }

    fn clone_boxed(&self) -> Box<dyn HdStSubtextureIdentifierTrait> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Unified enum for convenience
// ---------------------------------------------------------------------------

/// All subtexture identifier types as a single enum.
///
/// Provides a unified way to work with subtexture identifiers
/// without trait objects.
#[derive(Debug, Clone)]
pub enum HdStSubtextureId {
    /// Asset-backed UV texture
    AssetUv(HdStAssetUvSubtextureId),
    /// Volume field grid
    Field(HdStFieldSubtextureId),
    /// Ptex subdivision texture
    Ptex(HdStPtexSubtextureId),
    /// UDIM tiled texture
    Udim(HdStUdimSubtextureId),
    /// Dynamic (client-managed) UV texture
    DynamicUv(HdStDynamicUvSubtextureId),
    /// Dynamic (client-managed) cubemap texture
    DynamicCubemap(HdStDynamicCubemapSubtextureId),
}

impl HdStSubtextureId {
    /// Get premultiply alpha flag if applicable.
    pub fn premultiply_alpha(&self) -> bool {
        match self {
            Self::AssetUv(id) => id.premultiply_alpha,
            Self::Ptex(id) => id.premultiply_alpha,
            Self::Udim(id) => id.premultiply_alpha,
            _ => false,
        }
    }

    /// Get source color space if applicable.
    pub fn source_color_space(&self) -> Option<&Token> {
        match self {
            Self::AssetUv(id) => Some(&id.source_color_space),
            Self::Udim(id) => Some(&id.source_color_space),
            _ => None,
        }
    }

    /// Whether this is a dynamic subtexture type.
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::DynamicUv(_) | Self::DynamicCubemap(_))
    }

    /// Compute hash for registry deduplication.
    pub fn compute_hash(&self) -> u64 {
        match self {
            Self::AssetUv(id) => id.sub_hash(),
            Self::Field(id) => id.sub_hash(),
            Self::Ptex(id) => id.sub_hash(),
            Self::Udim(id) => id.sub_hash(),
            Self::DynamicUv(id) => id.sub_hash(),
            Self::DynamicCubemap(id) => id.sub_hash(),
        }
    }
}

impl PartialEq for HdStSubtextureId {
    fn eq(&self, other: &Self) -> bool {
        self.compute_hash() == other.compute_hash()
    }
}

impl Eq for HdStSubtextureId {}

impl Hash for HdStSubtextureId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.compute_hash().hash(state);
    }
}

/// Check if a file path represents a supported Ptex texture.
///
/// Simply checks the file extension (.ptx, .ptex).
pub fn is_supported_ptex(file_path: &str) -> bool {
    let lower = file_path.to_lowercase();
    lower.ends_with(".ptx") || lower.ends_with(".ptex")
}

/// Check if a file path represents a UDIM texture.
///
/// Checks for the `<UDIM>` or `<udim>` tag in the file name.
pub fn is_supported_udim(file_path: &str) -> bool {
    file_path.contains("<UDIM>") || file_path.contains("<udim>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_uv_subtexture() {
        let id = HdStAssetUvSubtextureId::new(true, false, Token::new("sRGB"));
        assert!(id.flip_vertically);
        assert!(!id.premultiply_alpha);
        assert_eq!(id.source_color_space.as_str(), "sRGB");
    }

    #[test]
    fn test_field_subtexture() {
        let id = HdStFieldSubtextureId::new(Token::new("density"), 0);
        assert_eq!(id.field_name.as_str(), "density");
        assert_eq!(id.field_index, 0);
    }

    #[test]
    fn test_ptex_subtexture() {
        let id = HdStPtexSubtextureId::new(true);
        assert!(id.premultiply_alpha);
    }

    #[test]
    fn test_udim_subtexture() {
        let id = HdStUdimSubtextureId::new(false, Token::new("raw"));
        assert!(!id.premultiply_alpha);
        assert_eq!(id.source_color_space.as_str(), "raw");
    }

    #[test]
    fn test_dynamic_uv_subtexture() {
        let id = HdStDynamicUvSubtextureId::new();
        assert_eq!(id.tag, Token::default());
    }

    #[test]
    fn test_enum_premultiply() {
        let uv = HdStSubtextureId::AssetUv(
            HdStAssetUvSubtextureId::new(false, true, Token::new("auto")),
        );
        assert!(uv.premultiply_alpha());

        let dyn_uv = HdStSubtextureId::DynamicUv(HdStDynamicUvSubtextureId::new());
        assert!(!dyn_uv.premultiply_alpha());
        assert!(dyn_uv.is_dynamic());
    }

    #[test]
    fn test_ptex_detection() {
        assert!(is_supported_ptex("model.ptx"));
        assert!(is_supported_ptex("model.ptex"));
        assert!(is_supported_ptex("MODEL.PTEX"));
        assert!(!is_supported_ptex("model.png"));
    }

    #[test]
    fn test_udim_detection() {
        assert!(is_supported_udim("diffuse.<UDIM>.exr"));
        assert!(is_supported_udim("diffuse.<udim>.exr"));
        assert!(!is_supported_udim("diffuse.1001.exr"));
    }

    #[test]
    fn test_hash_equality() {
        let a = HdStSubtextureId::Ptex(HdStPtexSubtextureId::new(true));
        let b = HdStSubtextureId::Ptex(HdStPtexSubtextureId::new(true));
        assert_eq!(a, b);
        assert_eq!(a.compute_hash(), b.compute_hash());
    }
}
