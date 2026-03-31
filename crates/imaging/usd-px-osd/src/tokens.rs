//! OpenSubdiv tokens for subdivision schemes and interpolation rules.
//!
//! This module defines the tokens used by OpenSubdiv for configuring
//! subdivision surface behavior.

use std::sync::LazyLock;
use usd_tf::Token;

// ============================================================================
// Subdivision Schemes
// ============================================================================

/// Catmull-Clark subdivision scheme.
/// Used for quadrilateral meshes.
pub static CATMULL_CLARK: LazyLock<Token> = LazyLock::new(|| Token::new("catmullClark"));

/// Loop subdivision scheme.
/// Used for triangular meshes.
pub static LOOP: LazyLock<Token> = LazyLock::new(|| Token::new("loop"));

/// Bilinear subdivision scheme.
/// No smoothing, just linear interpolation.
pub static BILINEAR: LazyLock<Token> = LazyLock::new(|| Token::new("bilinear"));

/// No subdivision.
pub static NONE: LazyLock<Token> = LazyLock::new(|| Token::new("none"));

// ============================================================================
// Orientation
// ============================================================================

/// Right-handed face winding order.
pub static RIGHT_HANDED: LazyLock<Token> = LazyLock::new(|| Token::new("rightHanded"));

/// Left-handed face winding order.
pub static LEFT_HANDED: LazyLock<Token> = LazyLock::new(|| Token::new("leftHanded"));

// ============================================================================
// Vertex Interpolation Rules
// ============================================================================

/// No boundary interpolation.
pub static EDGE_ONLY: LazyLock<Token> = LazyLock::new(|| Token::new("edgeOnly"));

/// Interpolate edges and corners.
pub static EDGE_AND_CORNER: LazyLock<Token> = LazyLock::new(|| Token::new("edgeAndCorner"));

// ============================================================================
// Face-Varying Interpolation Rules
// ============================================================================

/// Interpolate all boundaries.
pub static ALL: LazyLock<Token> = LazyLock::new(|| Token::new("all"));

/// Interpolate only boundaries.
pub static BOUNDARIES: LazyLock<Token> = LazyLock::new(|| Token::new("boundaries"));

/// Interpolate only corners.
pub static CORNERS_ONLY: LazyLock<Token> = LazyLock::new(|| Token::new("cornersOnly"));

/// Interpolate corners plus one adjacent edge.
pub static CORNERS_PLUS1: LazyLock<Token> = LazyLock::new(|| Token::new("cornersPlus1"));

/// Interpolate corners plus two adjacent edges.
pub static CORNERS_PLUS2: LazyLock<Token> = LazyLock::new(|| Token::new("cornersPlus2"));

// ============================================================================
// Triangle Subdivision
// ============================================================================

/// Smooth triangle subdivision.
pub static SMOOTH: LazyLock<Token> = LazyLock::new(|| Token::new("smooth"));

// Note: catmullClark token is also valid for triangle subdivision

// ============================================================================
// Crease Methods
// ============================================================================

/// Uniform crease method.
pub static UNIFORM: LazyLock<Token> = LazyLock::new(|| Token::new("uniform"));

/// Chaikin crease method.
pub static CHAIKIN: LazyLock<Token> = LazyLock::new(|| Token::new("chaikin"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdivision_schemes() {
        assert_eq!(CATMULL_CLARK.as_str(), "catmullClark");
        assert_eq!(LOOP.as_str(), "loop");
        assert_eq!(BILINEAR.as_str(), "bilinear");
        assert_eq!(NONE.as_str(), "none");
    }

    #[test]
    fn test_orientation() {
        assert_eq!(RIGHT_HANDED.as_str(), "rightHanded");
        assert_eq!(LEFT_HANDED.as_str(), "leftHanded");
    }

    #[test]
    fn test_vertex_interpolation() {
        assert_eq!(EDGE_ONLY.as_str(), "edgeOnly");
        assert_eq!(EDGE_AND_CORNER.as_str(), "edgeAndCorner");
        assert_eq!(NONE.as_str(), "none");
    }

    #[test]
    fn test_face_varying_interpolation() {
        assert_eq!(ALL.as_str(), "all");
        assert_eq!(BOUNDARIES.as_str(), "boundaries");
        assert_eq!(CORNERS_ONLY.as_str(), "cornersOnly");
        assert_eq!(CORNERS_PLUS1.as_str(), "cornersPlus1");
        assert_eq!(CORNERS_PLUS2.as_str(), "cornersPlus2");
        assert_eq!(NONE.as_str(), "none");
    }

    #[test]
    fn test_triangle_subdivision() {
        assert_eq!(SMOOTH.as_str(), "smooth");
        assert_eq!(CATMULL_CLARK.as_str(), "catmullClark");
    }

    #[test]
    fn test_crease_methods() {
        assert_eq!(UNIFORM.as_str(), "uniform");
        assert_eq!(CHAIKIN.as_str(), "chaikin");
    }
}
