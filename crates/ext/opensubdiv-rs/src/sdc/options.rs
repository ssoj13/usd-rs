// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/options.h

/// All supported options that affect the shape of the limit surface.
///
/// Mirrors the C++ `Sdc::Options` class.  Stored as plain u8 fields — identical
/// in-memory layout and semantics as the C++ packed byte-field representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    vtx_bound_interp: VtxBoundaryInterpolation,
    fvar_lin_interp: FVarLinearInterpolation,
    creasing_method: CreasingMethod,
    triangle_sub: TriangleSubdivision,
}

/// How boundary edges and corner vertices are interpolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VtxBoundaryInterpolation {
    /// No boundary interpolation, except where boundary edges were explicitly
    /// sharpened.
    #[default]
    None = 0,
    /// All boundary edges sharpened and interpolated.
    EdgeOnly = 1,
    /// All boundary edges **and** corner vertices sharpened and interpolated.
    EdgeAndCorner = 2,
}

/// Face-varying data interpolation at boundaries and creases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FVarLinearInterpolation {
    /// Smooth everywhere ("edge only").
    None = 0,
    /// Sharpen corners only.
    CornersOnly = 1,
    /// "edge corner"
    CornersPlus1 = 2,
    /// "edge and corner + propagate corner"
    CornersPlus2 = 3,
    /// Sharpen all boundaries ("always sharp").
    Boundaries = 4,
    /// Bilinear interpolation ("bilinear") — matches C++ `FVAR_LINEAR_ALL` default.
    #[default]
    All = 5,
}

/// Semi-sharp crease subdivision method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CreasingMethod {
    /// Standard Catmark rule — decrement by 1.0 each level.
    #[default]
    Uniform = 0,
    /// Chaikin rule — sharpness affected by neighbouring edges.
    Chaikin = 1,
}

/// Triangle subdivision weights (Catmark scheme only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TriangleSubdivision {
    /// Standard Catmark weights.
    #[default]
    Catmark = 0,
    /// "smooth triangle" weights.
    Smooth = 1,
}

impl Default for Options {
    /// Matches the C++ constructor defaults:
    ///   VTX_BOUNDARY_NONE, FVAR_LINEAR_ALL, CREASE_UNIFORM, TRI_SUB_CATMARK
    fn default() -> Self {
        Self {
            vtx_bound_interp: VtxBoundaryInterpolation::None,
            fvar_lin_interp: FVarLinearInterpolation::All,
            creasing_method: CreasingMethod::Uniform,
            triangle_sub: TriangleSubdivision::Catmark,
        }
    }
}

impl Options {
    /// Construct with explicit defaults (same as `Default`).
    pub fn new() -> Self {
        Self::default()
    }

    // ── Getters ──────────────────────────────────────────────────────────────

    #[inline]
    pub fn get_vtx_boundary_interpolation(&self) -> VtxBoundaryInterpolation {
        self.vtx_bound_interp
    }
    #[inline]
    pub fn get_fvar_linear_interpolation(&self) -> FVarLinearInterpolation {
        self.fvar_lin_interp
    }
    #[inline]
    pub fn get_creasing_method(&self) -> CreasingMethod {
        self.creasing_method
    }
    #[inline]
    pub fn get_triangle_subdivision(&self) -> TriangleSubdivision {
        self.triangle_sub
    }

    // ── Setters ──────────────────────────────────────────────────────────────

    #[inline]
    pub fn set_vtx_boundary_interpolation(&mut self, v: VtxBoundaryInterpolation) {
        self.vtx_bound_interp = v;
    }
    #[inline]
    pub fn set_fvar_linear_interpolation(&mut self, v: FVarLinearInterpolation) {
        self.fvar_lin_interp = v;
    }
    #[inline]
    pub fn set_creasing_method(&mut self, v: CreasingMethod) {
        self.creasing_method = v;
    }
    #[inline]
    pub fn set_triangle_subdivision(&mut self, v: TriangleSubdivision) {
        self.triangle_sub = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let o = Options::default();
        assert_eq!(
            o.get_vtx_boundary_interpolation(),
            VtxBoundaryInterpolation::None
        );
        assert_eq!(
            o.get_fvar_linear_interpolation(),
            FVarLinearInterpolation::All
        );
        assert_eq!(o.get_creasing_method(), CreasingMethod::Uniform);
        assert_eq!(o.get_triangle_subdivision(), TriangleSubdivision::Catmark);
    }

    #[test]
    fn setters() {
        let mut o = Options::default();
        o.set_vtx_boundary_interpolation(VtxBoundaryInterpolation::EdgeAndCorner);
        o.set_fvar_linear_interpolation(FVarLinearInterpolation::Boundaries);
        o.set_creasing_method(CreasingMethod::Chaikin);
        o.set_triangle_subdivision(TriangleSubdivision::Smooth);

        assert_eq!(
            o.get_vtx_boundary_interpolation(),
            VtxBoundaryInterpolation::EdgeAndCorner
        );
        assert_eq!(
            o.get_fvar_linear_interpolation(),
            FVarLinearInterpolation::Boundaries
        );
        assert_eq!(o.get_creasing_method(), CreasingMethod::Chaikin);
        assert_eq!(o.get_triangle_subdivision(), TriangleSubdivision::Smooth);
    }

    #[test]
    fn enum_discriminants() {
        // Verify numeric values match C++ enum constants
        assert_eq!(VtxBoundaryInterpolation::None as u8, 0);
        assert_eq!(VtxBoundaryInterpolation::EdgeOnly as u8, 1);
        assert_eq!(VtxBoundaryInterpolation::EdgeAndCorner as u8, 2);

        assert_eq!(FVarLinearInterpolation::None as u8, 0);
        assert_eq!(FVarLinearInterpolation::CornersOnly as u8, 1);
        assert_eq!(FVarLinearInterpolation::CornersPlus1 as u8, 2);
        assert_eq!(FVarLinearInterpolation::CornersPlus2 as u8, 3);
        assert_eq!(FVarLinearInterpolation::Boundaries as u8, 4);
        assert_eq!(FVarLinearInterpolation::All as u8, 5);

        assert_eq!(CreasingMethod::Uniform as u8, 0);
        assert_eq!(CreasingMethod::Chaikin as u8, 1);

        assert_eq!(TriangleSubdivision::Catmark as u8, 0);
        assert_eq!(TriangleSubdivision::Smooth as u8, 1);
    }
}
