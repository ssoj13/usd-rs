
//! PointsShaderKey - shader variant selection for point cloud rendering.
//!
//! Port of C++ `HdSt_PointsShaderKey`. Simple key with VS + FS stages,
//! no tessellation or geometry shader. Supports native round points
//! (hardware point rasterization with circular coverage).

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Shader key for point cloud rendering.
///
/// Selects the correct VS and FS mixins for point rendering.
/// Points skip TCS/TES/GS stages entirely.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointsShaderKey {
    /// Use native round points (hardware circular point rasterization).
    /// When false, points are rendered as quads/squares.
    pub native_round_points: bool,
    /// GLSLFX/WGSLFX source file
    pub glslfx: Token,
    /// Vertex shader mixins
    pub vs: Vec<Token>,
    /// Fragment shader mixins
    pub fs: Vec<Token>,
}

impl PointsShaderKey {
    /// Build a points shader key.
    pub fn new(native_round_points: bool) -> Self {
        let glslfx = Token::new("points.glslfx");

        // Vertex shader: instancing transform + point-specific VS
        let mut vs = vec![
            Token::new("Instancing.Transform"),
            Token::new("Points.Vertex.Common"),
            Token::new("Points.Vertex.Position"),
        ];
        if native_round_points {
            vs.push(Token::new("Points.Vertex.NativeRoundPoints"));
        }
        vs.push(Token::new("Points.Vertex.PointSize"));

        // Fragment shader: point-specific FS
        let mut fs = vec![
            Token::new("Points.Fragment.Common"),
            Token::new("Points.Fragment.Color"),
        ];
        if native_round_points {
            fs.push(Token::new("Points.Fragment.NativeRoundPoints"));
        }
        fs.push(Token::new("Points.Fragment.Lighting"));

        Self {
            native_round_points,
            glslfx,
            vs,
            fs,
        }
    }

    /// Points always use PRIM_POINTS topology.
    pub fn primitive_type_is_points(&self) -> bool {
        true
    }

    /// Compute a u64 hash for pipeline cache lookup.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Number of vertex attributes expected.
    /// Points always have position; optionally color and normals.
    pub fn vertex_attr_count(&self) -> u32 {
        // Position always present
        1
    }
}

impl Default for PointsShaderKey {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_points() {
        let key = PointsShaderKey::new(false);
        assert!(key.primitive_type_is_points());
        assert!(!key.native_round_points);
        assert!(
            key.vs
                .iter()
                .any(|t| t.as_str().contains("Points.Vertex.Common"))
        );
        assert!(
            key.fs
                .iter()
                .any(|t| t.as_str().contains("Points.Fragment.Common"))
        );
        // Should NOT have round points mixin
        assert!(
            !key.vs
                .iter()
                .any(|t| t.as_str().contains("NativeRoundPoints"))
        );
    }

    #[test]
    fn test_native_round_points() {
        let key = PointsShaderKey::new(true);
        assert!(key.native_round_points);
        assert!(
            key.vs
                .iter()
                .any(|t| t.as_str().contains("NativeRoundPoints"))
        );
        assert!(
            key.fs
                .iter()
                .any(|t| t.as_str().contains("NativeRoundPoints"))
        );
    }

    #[test]
    fn test_hash_differs() {
        let k1 = PointsShaderKey::new(false);
        let k2 = PointsShaderKey::new(true);
        assert_ne!(k1.cache_hash(), k2.cache_hash());
    }

    #[test]
    fn test_default() {
        let key = PointsShaderKey::default();
        assert!(!key.native_round_points);
        assert_eq!(key.glslfx.as_str(), "points.glslfx");
    }
}
