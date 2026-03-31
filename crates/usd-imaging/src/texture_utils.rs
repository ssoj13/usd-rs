//! Texture utilities.
//!
//! Port of pxr/usdImaging/usdImaging/textureUtils.h/cpp
//!
//! Provides helper functions for working with texture assets, including
//! UDIM texture pattern expansion, path resolution, and texture parameter
//! extraction (wrap modes, filtering, etc.).
//!
//! # UDIM Textures
//!
//! UDIM (U-DIMension) is a tiling system where texture coordinates outside
//! the 0-1 range map to different texture files. The pattern uses <UDIM>
//! as a placeholder that gets replaced with 4-digit tile indices (1001, 1002, etc.).
//!
//! # Examples
//!
//! ```ignore
//! use usd_imaging::texture_utils::{
//!     is_udim_pattern, expand_udim_tiles, TextureInfo,
//! };
//!
//! // Check if a path is a UDIM pattern
//! let path = "textures/albedo.<UDIM>.png";
//! if is_udim_pattern(path) {
//!     let tiles = expand_udim_tiles(path, 10, 10);
//!     // tiles = ["textures/albedo.1001.png", "textures/albedo.1002.png", ...]
//! }
//! ```

use std::path::PathBuf;
use usd_core::{Attribute, Prim};
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// Types
// ============================================================================

/// Texture wrap mode enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    /// Clamp texture coordinates to [0, 1].
    Clamp,
    /// Repeat texture coordinates (modulo).
    Repeat,
    /// Mirror texture coordinates.
    Mirror,
    /// Use black color outside [0, 1].
    Black,
    /// Use fallback/default behavior.
    UseMetadata,
}

impl WrapMode {
    /// Parse wrap mode from USD token.
    pub fn from_token(token: &Token) -> Self {
        match token.as_str() {
            "clamp" => WrapMode::Clamp,
            "repeat" => WrapMode::Repeat,
            "mirror" => WrapMode::Mirror,
            "black" => WrapMode::Black,
            "useMetadata" => WrapMode::UseMetadata,
            _ => WrapMode::UseMetadata,
        }
    }

    /// Convert to USD token string.
    pub fn to_token(&self) -> Token {
        Token::new(match self {
            WrapMode::Clamp => "clamp",
            WrapMode::Repeat => "repeat",
            WrapMode::Mirror => "mirror",
            WrapMode::Black => "black",
            WrapMode::UseMetadata => "useMetadata",
        })
    }
}

/// Texture filtering mode enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    /// Nearest neighbor filtering.
    Nearest,
    /// Linear filtering.
    Linear,
    /// Trilinear filtering (with mipmaps).
    Trilinear,
    /// Anisotropic filtering.
    Anisotropic,
}

impl FilterMode {
    /// Parse filter mode from USD token.
    pub fn from_token(token: &Token) -> Self {
        match token.as_str() {
            "nearest" => FilterMode::Nearest,
            "linear" => FilterMode::Linear,
            "trilinear" => FilterMode::Trilinear,
            "anisotropic" => FilterMode::Anisotropic,
            _ => FilterMode::Linear,
        }
    }

    /// Convert to USD token string.
    pub fn to_token(&self) -> Token {
        Token::new(match self {
            FilterMode::Nearest => "nearest",
            FilterMode::Linear => "linear",
            FilterMode::Trilinear => "trilinear",
            FilterMode::Anisotropic => "anisotropic",
        })
    }
}

/// Complete texture information bundle.
#[derive(Debug, Clone)]
pub struct TextureInfo {
    /// Resolved texture file path.
    pub file_path: PathBuf,
    /// Wrap mode in S (U) direction.
    pub wrap_s: WrapMode,
    /// Wrap mode in T (V) direction.
    pub wrap_t: WrapMode,
    /// Minification filter mode.
    pub min_filter: FilterMode,
    /// Magnification filter mode.
    pub mag_filter: FilterMode,
    /// Whether this is a UDIM texture.
    pub is_udim: bool,
    /// Color space (e.g., "sRGB", "linear", "raw").
    pub color_space: Option<Token>,
}

impl Default for TextureInfo {
    fn default() -> Self {
        Self {
            file_path: PathBuf::new(),
            wrap_s: WrapMode::Repeat,
            wrap_t: WrapMode::Repeat,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            is_udim: false,
            color_space: None,
        }
    }
}

// ============================================================================
// UDIM Pattern Functions
// ============================================================================

/// Check if a path contains a UDIM pattern.
///
/// UDIM patterns use <UDIM> as a placeholder for tile indices.
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// True if the path contains "<UDIM>" or similar pattern
pub fn is_udim_pattern(path: &str) -> bool {
    path.contains("<UDIM>") || path.contains("<udim>")
}

/// Extract the UDIM pattern from a path.
///
/// Returns the base pattern with <UDIM> placeholder.
///
/// # Arguments
/// * `path` - Path potentially containing a UDIM tile number
///
/// # Returns
/// Path with tile number replaced by <UDIM>, or original path if not a UDIM texture
pub fn extract_udim_pattern(path: &str) -> String {
    // Look for 4-digit tile numbers (1001-1999)
    if let Some(pos) = find_udim_tile_number(path) {
        let mut pattern = path.to_string();
        pattern.replace_range(pos..pos + 4, "<UDIM>");
        pattern
    } else {
        path.to_string()
    }
}

/// Find the position of a UDIM tile number in a path.
///
/// Searches for 4-digit numbers in the range 1001-1999 (valid UDIM tiles).
fn find_udim_tile_number(path: &str) -> Option<usize> {
    let bytes = path.as_bytes();
    for i in 0..path.len().saturating_sub(3) {
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
        {
            // Check if it's in valid UDIM range (1001-1999)
            let tile_str = &path[i..i + 4];
            if let Ok(tile) = tile_str.parse::<u32>() {
                if (1001..=1999).contains(&tile) {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Expand a UDIM pattern to a list of tile paths.
///
/// Replaces <UDIM> with tile numbers for a grid of tiles.
///
/// # Arguments
/// * `pattern` - Path containing <UDIM> placeholder
/// * `u_tiles` - Number of tiles in U direction (columns)
/// * `v_tiles` - Number of tiles in V direction (rows)
///
/// # Returns
/// Vector of expanded paths with tile numbers (1001, 1002, ...)
pub fn expand_udim_tiles(pattern: &str, u_tiles: u32, v_tiles: u32) -> Vec<String> {
    let mut paths = Vec::with_capacity((u_tiles * v_tiles) as usize);

    for v in 0..v_tiles {
        for u in 0..u_tiles {
            // UDIM formula: 1001 + u + v * 10
            let tile_num = 1001 + u + v * 10;
            let expanded = pattern
                .replace("<UDIM>", &tile_num.to_string())
                .replace("<udim>", &tile_num.to_string());
            paths.push(expanded);
        }
    }

    paths
}

/// Compute UDIM tile number from UV coordinates.
///
/// # Arguments
/// * `u` - U texture coordinate
/// * `v` - V texture coordinate
///
/// # Returns
/// UDIM tile number (1001-based)
pub fn compute_udim_tile(u: f32, v: f32) -> u32 {
    let u_tile = u.floor() as i32;
    let v_tile = v.floor() as i32;
    1001 + u_tile.max(0) as u32 + (v_tile.max(0) as u32) * 10
}

// ============================================================================
// Texture Path Resolution
// ============================================================================

/// Resolve a texture file path relative to a context.
///
/// # Arguments
/// * `file_path` - The texture file path (may be relative)
/// * `context_path` - Context path for resolution (layer or asset path)
///
/// # Returns
/// Resolved absolute path
pub fn resolve_texture_path(file_path: &str, context_path: &Path) -> Option<PathBuf> {
    if file_path.is_empty() {
        return None;
    }

    // Handle absolute paths (including Unix-style on Windows)
    let path = PathBuf::from(file_path);
    if path.is_absolute() || file_path.starts_with('/') {
        return Some(path);
    }

    // Resolve relative to context
    let context_str = context_path.as_str();
    if let Some(dir_end) = context_str.rfind('/') {
        let dir = &context_str[..=dir_end];
        let resolved = PathBuf::from(format!("{}{}", dir, file_path));
        return Some(resolved);
    }

    Some(path)
}

// ============================================================================
// Texture Parameter Extraction
// ============================================================================

/// Extract texture information from a UsdUVTexture shader prim.
///
/// Reads all relevant texture parameters (file, wrap modes, filtering, etc.)
/// and packages them into a TextureInfo struct.
///
/// # Arguments
/// * `texture_shader` - The UsdUVTexture shader prim
///
/// # Returns
/// Complete texture information bundle
pub fn extract_texture_info(texture_shader: &Prim) -> Option<TextureInfo> {
    let mut info = TextureInfo::default();

    // Get file path
    if let Some(file_attr) = texture_shader.get_attribute("inputs:file") {
        if let Some(file_value) = file_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(file_path) = file_value.get::<String>() {
                info.file_path = PathBuf::from(file_path.as_str());
                info.is_udim = is_udim_pattern(file_path);
            } else {
                return None; // File path is required
            }
        } else {
            return None;
        }
    } else {
        return None;
    }

    // Get wrap modes
    if let Some(wrap_s_attr) = texture_shader.get_attribute("inputs:wrapS") {
        if let Some(wrap_value) = wrap_s_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(wrap_str) = wrap_value.get::<String>() {
                info.wrap_s = WrapMode::from_token(&Token::new(wrap_str));
            }
        }
    }

    if let Some(wrap_t_attr) = texture_shader.get_attribute("inputs:wrapT") {
        if let Some(wrap_value) = wrap_t_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(wrap_str) = wrap_value.get::<String>() {
                info.wrap_t = WrapMode::from_token(&Token::new(wrap_str));
            }
        }
    }

    // Get filter modes (if available)
    if let Some(min_filter_attr) = texture_shader.get_attribute("inputs:minFilter") {
        if let Some(filter_value) = min_filter_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(filter_str) = filter_value.get::<String>() {
                info.min_filter = FilterMode::from_token(&Token::new(filter_str));
            }
        }
    }

    if let Some(mag_filter_attr) = texture_shader.get_attribute("inputs:magFilter") {
        if let Some(filter_value) = mag_filter_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(filter_str) = filter_value.get::<String>() {
                info.mag_filter = FilterMode::from_token(&Token::new(filter_str));
            }
        }
    }

    // Get color space
    if let Some(color_space_attr) = texture_shader.get_attribute("inputs:sourceColorSpace") {
        if let Some(cs_value) = color_space_attr.get(usd_sdf::TimeCode::default()) {
            if let Some(cs_str) = cs_value.get::<String>() {
                info.color_space = Some(Token::new(cs_str));
            }
        }
    }

    Some(info)
}

/// Get wrap mode from an attribute.
///
/// Helper function to extract wrap mode from a texture attribute.
///
/// # Arguments
/// * `attr` - The wrap mode attribute
///
/// # Returns
/// The wrap mode, or default (Repeat) if not found
pub fn get_wrap_mode(attr: &Attribute) -> WrapMode {
    if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
        if let Some(wrap_str) = value.get::<String>() {
            return WrapMode::from_token(&Token::new(wrap_str));
        }
    }
    WrapMode::Repeat
}

/// Get filter mode from an attribute.
///
/// Helper function to extract filter mode from a texture attribute.
///
/// # Arguments
/// * `attr` - The filter mode attribute
///
/// # Returns
/// The filter mode, or default (Linear) if not found
pub fn get_filter_mode(attr: &Attribute) -> FilterMode {
    if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
        if let Some(filter_str) = value.get::<String>() {
            return FilterMode::from_token(&Token::new(filter_str));
        }
    }
    FilterMode::Linear
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_udim_pattern() {
        assert!(is_udim_pattern("texture.<UDIM>.png"));
        assert!(is_udim_pattern("path/to/texture.<udim>.exr"));
        assert!(!is_udim_pattern("texture.1001.png"));
        assert!(!is_udim_pattern("normal_texture.png"));
    }

    #[test]
    fn test_find_udim_tile() {
        assert_eq!(find_udim_tile_number("texture.1001.png"), Some(8));
        assert_eq!(find_udim_tile_number("texture.1015.png"), Some(8));
        assert_eq!(find_udim_tile_number("texture.9999.png"), None); // Out of range
        assert_eq!(find_udim_tile_number("texture.png"), None);
    }

    #[test]
    fn test_extract_udim_pattern() {
        let pattern = extract_udim_pattern("texture.1001.png");
        assert_eq!(pattern, "texture.<UDIM>.png");

        let pattern = extract_udim_pattern("path/to/albedo.1015.exr");
        assert_eq!(pattern, "path/to/albedo.<UDIM>.exr");

        let pattern = extract_udim_pattern("no_udim.png");
        assert_eq!(pattern, "no_udim.png");
    }

    #[test]
    fn test_expand_udim_tiles() {
        let pattern = "texture.<UDIM>.png";
        let tiles = expand_udim_tiles(pattern, 3, 2);

        assert_eq!(tiles.len(), 6);
        assert_eq!(tiles[0], "texture.1001.png");
        assert_eq!(tiles[1], "texture.1002.png");
        assert_eq!(tiles[2], "texture.1003.png");
        assert_eq!(tiles[3], "texture.1011.png");
        assert_eq!(tiles[4], "texture.1012.png");
        assert_eq!(tiles[5], "texture.1013.png");
    }

    #[test]
    fn test_compute_udim_tile() {
        assert_eq!(compute_udim_tile(0.5, 0.5), 1001);
        assert_eq!(compute_udim_tile(1.5, 0.5), 1002);
        assert_eq!(compute_udim_tile(0.5, 1.5), 1011);
        assert_eq!(compute_udim_tile(2.3, 1.7), 1013);
    }

    #[test]
    fn test_wrap_mode_conversion() {
        let token = Token::new("repeat");
        let wrap = WrapMode::from_token(&token);
        assert_eq!(wrap, WrapMode::Repeat);
        assert_eq!(wrap.to_token().as_str(), "repeat");
    }

    #[test]
    fn test_filter_mode_conversion() {
        let token = Token::new("linear");
        let filter = FilterMode::from_token(&token);
        assert_eq!(filter, FilterMode::Linear);
        assert_eq!(filter.to_token().as_str(), "linear");
    }

    #[test]
    fn test_texture_info_default() {
        let info = TextureInfo::default();
        assert_eq!(info.wrap_s, WrapMode::Repeat);
        assert_eq!(info.wrap_t, WrapMode::Repeat);
        assert_eq!(info.min_filter, FilterMode::Linear);
        assert_eq!(info.mag_filter, FilterMode::Linear);
        assert!(!info.is_udim);
    }

    #[test]
    fn test_resolve_texture_path() {
        let context = Path::from_string("/assets/materials/material.usda").unwrap();

        // Absolute path
        let resolved = resolve_texture_path("/textures/albedo.png", &context);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), PathBuf::from("/textures/albedo.png"));

        // Empty path
        let resolved = resolve_texture_path("", &context);
        assert!(resolved.is_none());
    }
}
