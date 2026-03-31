//! Typed enums for OpenSubdiv subdivision parameters.
//!
//! Mirrors the token-based API in C++ pxOsd but provides Rust-friendly
//! typed enums for vertex boundary interpolation, face-varying linear
//! interpolation, crease method, and triangle subdivision.

use std::fmt;
use std::str::FromStr;

// ============================================================================
// VtxBoundaryInterpolation
// ============================================================================

/// Vertex boundary interpolation rule for subdivision surfaces.
///
/// Controls how the boundary edges of a mesh are treated during subdivision.
/// Matches OpenSubdiv `Sdc::Options::VtxBoundaryInterpolation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VtxBoundaryInterpolation {
    /// No boundary interpolation — boundary vertices are not constrained.
    None,
    /// Boundary edges only are interpolated (corners remain free).
    EdgeOnly,
    /// Both boundary edges and corners are interpolated.
    /// This is the OpenSubdiv default and matches USD `edgeAndCorner`.
    EdgeAndCorner,
}

impl Default for VtxBoundaryInterpolation {
    /// Default per OpenSubdiv spec: `EdgeAndCorner`.
    fn default() -> Self {
        Self::EdgeAndCorner
    }
}

impl fmt::Display for VtxBoundaryInterpolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::EdgeOnly => "edgeOnly",
            Self::EdgeAndCorner => "edgeAndCorner",
        })
    }
}

impl FromStr for VtxBoundaryInterpolation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "edgeOnly" => Ok(Self::EdgeOnly),
            "edgeAndCorner" => Ok(Self::EdgeAndCorner),
            _ => Err(format!("unknown VtxBoundaryInterpolation: {s}")),
        }
    }
}

// ============================================================================
// FVarLinearInterpolation
// ============================================================================

/// Face-varying linear interpolation rule for subdivision surfaces.
///
/// Controls how face-varying data (e.g. UVs) is interpolated at boundaries.
/// Matches OpenSubdiv `Sdc::Options::FVarLinearInterpolation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FVarLinearInterpolation {
    /// No linear interpolation — smooth everywhere.
    None,
    /// Sharp only at corner vertices.
    CornersOnly,
    /// Sharp at corners and one-ring of edges around them.
    CornersPlus1,
    /// Sharp at corners and two-ring of edges.
    CornersPlus2,
    /// Sharp at all boundary edges and corners.
    Boundaries,
    /// Fully linear (bilinear) — all face-varying data is interpolated linearly.
    /// This is the default for USD compatibility.
    All,
}

impl Default for FVarLinearInterpolation {
    /// Default per OpenSubdiv/USD spec: `All`.
    fn default() -> Self {
        Self::All
    }
}

impl fmt::Display for FVarLinearInterpolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::CornersOnly => "cornersOnly",
            Self::CornersPlus1 => "cornersPlus1",
            Self::CornersPlus2 => "cornersPlus2",
            Self::Boundaries => "boundaries",
            Self::All => "all",
        })
    }
}

impl FromStr for FVarLinearInterpolation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "cornersOnly" => Ok(Self::CornersOnly),
            "cornersPlus1" => Ok(Self::CornersPlus1),
            "cornersPlus2" => Ok(Self::CornersPlus2),
            "boundaries" => Ok(Self::Boundaries),
            "all" => Ok(Self::All),
            _ => Err(format!("unknown FVarLinearInterpolation: {s}")),
        }
    }
}

// ============================================================================
// CreasingMethod
// ============================================================================

/// Crease sharpness computation method.
///
/// Matches OpenSubdiv `Sdc::Options::CreasingMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CreasingMethod {
    /// Standard uniform crease (default).
    #[default]
    Uniform,
    /// Chaikin crease — smoothes the crease sharpness along a chain.
    Chaikin,
}

impl fmt::Display for CreasingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Uniform => "uniform",
            Self::Chaikin => "chaikin",
        })
    }
}

impl FromStr for CreasingMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "uniform" | "" => Ok(Self::Uniform),
            "chaikin" => Ok(Self::Chaikin),
            _ => Err(format!("unknown CreasingMethod: {s}")),
        }
    }
}

// ============================================================================
// TriangleSubdivision
// ============================================================================

/// Triangle subdivision method for Catmull-Clark surfaces.
///
/// Controls how triangular faces are treated during Catmull-Clark subdivision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TriangleSubdivision {
    /// Standard Catmull-Clark treatment of triangles.
    #[default]
    CatmullClark,
    /// Smooth triangle subdivision (per-Loop-like handling inside CC).
    Smooth,
}

impl fmt::Display for TriangleSubdivision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CatmullClark => "catmullClark",
            Self::Smooth => "smooth",
        })
    }
}

impl FromStr for TriangleSubdivision {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "catmullClark" | "" => Ok(Self::CatmullClark),
            "smooth" => Ok(Self::Smooth),
            _ => Err(format!("unknown TriangleSubdivision: {s}")),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtx_boundary_defaults_edge_and_corner() {
        assert_eq!(
            VtxBoundaryInterpolation::default(),
            VtxBoundaryInterpolation::EdgeAndCorner
        );
    }

    #[test]
    fn vtx_boundary_round_trip() {
        for v in [
            VtxBoundaryInterpolation::None,
            VtxBoundaryInterpolation::EdgeOnly,
            VtxBoundaryInterpolation::EdgeAndCorner,
        ] {
            let s = v.to_string();
            assert_eq!(s.parse::<VtxBoundaryInterpolation>().unwrap(), v);
        }
    }

    #[test]
    fn fvar_linear_defaults_all() {
        assert_eq!(
            FVarLinearInterpolation::default(),
            FVarLinearInterpolation::All
        );
    }

    #[test]
    fn fvar_linear_round_trip() {
        for v in [
            FVarLinearInterpolation::None,
            FVarLinearInterpolation::CornersOnly,
            FVarLinearInterpolation::CornersPlus1,
            FVarLinearInterpolation::CornersPlus2,
            FVarLinearInterpolation::Boundaries,
            FVarLinearInterpolation::All,
        ] {
            let s = v.to_string();
            assert_eq!(s.parse::<FVarLinearInterpolation>().unwrap(), v);
        }
    }

    #[test]
    fn crease_method_defaults_uniform() {
        assert_eq!(CreasingMethod::default(), CreasingMethod::Uniform);
        // Empty string also maps to Uniform
        assert_eq!(
            "".parse::<CreasingMethod>().unwrap(),
            CreasingMethod::Uniform
        );
    }

    #[test]
    fn crease_method_round_trip() {
        for v in [CreasingMethod::Uniform, CreasingMethod::Chaikin] {
            let s = v.to_string();
            assert_eq!(s.parse::<CreasingMethod>().unwrap(), v);
        }
    }

    #[test]
    fn triangle_subdivision_defaults_catmull_clark() {
        assert_eq!(
            TriangleSubdivision::default(),
            TriangleSubdivision::CatmullClark
        );
        assert_eq!(
            "".parse::<TriangleSubdivision>().unwrap(),
            TriangleSubdivision::CatmullClark
        );
    }

    #[test]
    fn triangle_subdivision_round_trip() {
        for v in [
            TriangleSubdivision::CatmullClark,
            TriangleSubdivision::Smooth,
        ] {
            let s = v.to_string();
            assert_eq!(s.parse::<TriangleSubdivision>().unwrap(), v);
        }
    }

    #[test]
    fn unknown_values_return_err() {
        assert!("bogus".parse::<VtxBoundaryInterpolation>().is_err());
        assert!("bogus".parse::<FVarLinearInterpolation>().is_err());
        assert!("bogus".parse::<CreasingMethod>().is_err());
        assert!("bogus".parse::<TriangleSubdivision>().is_err());
    }
}
