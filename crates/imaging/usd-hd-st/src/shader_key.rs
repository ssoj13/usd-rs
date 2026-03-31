
//! HdStShaderKey - Shader key for cache lookup.
//!
//! Provides a key structure for identifying and caching compiled shaders.
//! Keys encode all the state needed to uniquely identify a shader variant.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Primitive type for geometric shaders.
///
/// Defines the type of geometric primitive this shader will process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    /// Points primitive
    Points,
    /// Lines primitive  
    Lines,
    /// Line strips
    LineStrip,
    /// Triangles primitive
    Triangles,
    /// Triangle strips
    TriangleStrip,
    /// Triangle fans
    TriangleFan,
    /// Patches for tessellation
    Patches,
    /// Quads (converted to triangles)
    Quads,
}

impl PrimitiveType {
    /// Get the primitive type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Points => "points",
            Self::Lines => "lines",
            Self::LineStrip => "line_strip",
            Self::Triangles => "triangles",
            Self::TriangleStrip => "triangle_strip",
            Self::TriangleFan => "triangle_fan",
            Self::Patches => "patches",
            Self::Quads => "quads",
        }
    }

    /// Check if this is a patch primitive (requires tessellation).
    pub fn is_patch(&self) -> bool {
        matches!(self, Self::Patches)
    }
}

/// Geometric primitive style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometricStyle {
    /// Render as points
    Points,
    /// Render as wireframe edges
    Edges,
    /// Render as solid surface
    Surface,
}

/// Shader key for cache lookup.
///
/// Encodes all state needed to identify a unique shader variant:
/// - Primitive type (points, lines, triangles, etc.)
/// - Geometric style (points, edges, surface)
/// - Feature flags (normals, colors, textures, etc.)
/// - Precision (float vs double)
///
/// Keys must be hashable and comparable for use in shader caches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdStShaderKey {
    /// Primitive type to render
    primitive_type: PrimitiveType,

    /// Geometric rendering style
    geom_style: GeometricStyle,

    /// Use smooth normals (vs flat)
    smooth_normals: bool,

    /// Use double precision
    double_sided: bool,

    /// Enable face culling
    cull_style: u8,

    /// Has vertex colors
    has_vertex_colors: bool,

    /// Has texture coordinates
    has_texture_coords: bool,

    /// Enable instancing
    instanced: bool,

    /// Custom hash for quick lookup
    hash: u64,
}

impl HdStShaderKey {
    /// Create a new shader key.
    pub fn new(primitive_type: PrimitiveType, geom_style: GeometricStyle) -> Self {
        let mut key = Self {
            primitive_type,
            geom_style,
            smooth_normals: true,
            double_sided: false,
            cull_style: 0,
            has_vertex_colors: false,
            has_texture_coords: false,
            instanced: false,
            hash: 0,
        };
        key.compute_hash();
        key
    }

    /// Get primitive type.
    pub fn get_primitive_type(&self) -> PrimitiveType {
        self.primitive_type
    }

    /// Set primitive type.
    pub fn set_primitive_type(&mut self, prim_type: PrimitiveType) {
        self.primitive_type = prim_type;
        self.compute_hash();
    }

    /// Get geometric style.
    pub fn get_geom_style(&self) -> GeometricStyle {
        self.geom_style
    }

    /// Set geometric style.
    pub fn set_geom_style(&mut self, style: GeometricStyle) {
        self.geom_style = style;
        self.compute_hash();
    }

    /// Check if using smooth normals.
    pub fn use_smooth_normals(&self) -> bool {
        self.smooth_normals
    }

    /// Set smooth normals flag.
    pub fn set_smooth_normals(&mut self, smooth: bool) {
        self.smooth_normals = smooth;
        self.compute_hash();
    }

    /// Check if double-sided rendering is enabled.
    pub fn is_double_sided(&self) -> bool {
        self.double_sided
    }

    /// Set double-sided rendering.
    pub fn set_double_sided(&mut self, double_sided: bool) {
        self.double_sided = double_sided;
        self.compute_hash();
    }

    /// Get cull style.
    pub fn get_cull_style(&self) -> u8 {
        self.cull_style
    }

    /// Set cull style.
    pub fn set_cull_style(&mut self, cull_style: u8) {
        self.cull_style = cull_style;
        self.compute_hash();
    }

    /// Check if has vertex colors.
    pub fn has_vertex_colors(&self) -> bool {
        self.has_vertex_colors
    }

    /// Set vertex colors flag.
    pub fn set_vertex_colors(&mut self, has_colors: bool) {
        self.has_vertex_colors = has_colors;
        self.compute_hash();
    }

    /// Check if has texture coordinates.
    pub fn has_texture_coords(&self) -> bool {
        self.has_texture_coords
    }

    /// Set texture coordinates flag.
    pub fn set_texture_coords(&mut self, has_uvs: bool) {
        self.has_texture_coords = has_uvs;
        self.compute_hash();
    }

    /// Check if instanced rendering is enabled.
    pub fn is_instanced(&self) -> bool {
        self.instanced
    }

    /// Set instanced rendering flag.
    pub fn set_instanced(&mut self, instanced: bool) {
        self.instanced = instanced;
        self.compute_hash();
    }

    /// Get precomputed hash for quick lookups.
    pub fn get_hash(&self) -> u64 {
        self.hash
    }

    /// Compute hash from all key components.
    fn compute_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.primitive_type.hash(&mut hasher);
        self.geom_style.hash(&mut hasher);
        self.smooth_normals.hash(&mut hasher);
        self.double_sided.hash(&mut hasher);
        self.cull_style.hash(&mut hasher);
        self.has_vertex_colors.hash(&mut hasher);
        self.has_texture_coords.hash(&mut hasher);
        self.instanced.hash(&mut hasher);
        self.hash = hasher.finish();
    }
}

impl Hash for HdStShaderKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use precomputed hash
        self.hash.hash(state);
    }
}

impl fmt::Display for HdStShaderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ShaderKey({}/{}/{}{}{}{}{}{})",
            self.primitive_type.as_str(),
            match self.geom_style {
                GeometricStyle::Points => "pts",
                GeometricStyle::Edges => "edge",
                GeometricStyle::Surface => "surf",
            },
            if self.smooth_normals { "S" } else { "F" },
            if self.double_sided { "D" } else { "" },
            if self.has_vertex_colors { "C" } else { "" },
            if self.has_texture_coords { "T" } else { "" },
            if self.instanced { "I" } else { "" },
            self.hash
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_type() {
        assert_eq!(PrimitiveType::Triangles.as_str(), "triangles");
        assert!(!PrimitiveType::Triangles.is_patch());
        assert!(PrimitiveType::Patches.is_patch());
    }

    #[test]
    fn test_shader_key_creation() {
        let key = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        assert_eq!(key.get_primitive_type(), PrimitiveType::Triangles);
        assert_eq!(key.get_geom_style(), GeometricStyle::Surface);
        assert!(key.use_smooth_normals());
        assert!(!key.is_double_sided());
    }

    #[test]
    fn test_shader_key_hash() {
        let key1 = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        let key2 = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        assert_eq!(key1.get_hash(), key2.get_hash());

        let key3 = HdStShaderKey::new(PrimitiveType::Points, GeometricStyle::Surface);
        assert_ne!(key1.get_hash(), key3.get_hash());
    }

    #[test]
    fn test_shader_key_modification() {
        let mut key = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        let hash1 = key.get_hash();

        key.set_vertex_colors(true);
        let hash2 = key.get_hash();
        assert_ne!(hash1, hash2);
        assert!(key.has_vertex_colors());

        key.set_smooth_normals(false);
        let hash3 = key.get_hash();
        assert_ne!(hash2, hash3);
        assert!(!key.use_smooth_normals());
    }

    #[test]
    fn test_shader_key_equality() {
        let key1 = HdStShaderKey::new(PrimitiveType::Lines, GeometricStyle::Edges);
        let key2 = HdStShaderKey::new(PrimitiveType::Lines, GeometricStyle::Edges);
        assert_eq!(key1, key2);

        let mut key3 = HdStShaderKey::new(PrimitiveType::Lines, GeometricStyle::Edges);
        key3.set_instanced(true);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_shader_key_display() {
        let mut key = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        key.set_vertex_colors(true);
        key.set_texture_coords(true);

        let display = format!("{}", key);
        assert!(display.contains("triangles"));
        assert!(display.contains("surf"));
        assert!(display.contains("C")); // has colors
        assert!(display.contains("T")); // has UVs
    }
}
