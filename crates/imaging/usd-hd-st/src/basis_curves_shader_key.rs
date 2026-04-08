//! BasisCurvesShaderKey - shader variant selection for curve rendering.
//!
//! Port of C++ `HdSt_BasisCurvesShaderKey`. Encodes curve type, basis,
//! draw style, normal style, and feature flags to select the correct
//! shader mixins for each pipeline stage.
//!
//! Draw styles balance offline renderer fidelity (e.g. RenderMan half-tubes)
//! with interactive performance. Not all DrawStyle x NormalStyle combos
//! are meaningful (e.g. Oriented only makes sense with Ribbon).

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Curve draw style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CurveDrawStyle {
    /// Draw only the control vertices as points.
    Points,
    /// Draw as lines or isolines, tessellated along length.
    #[default]
    Wire,
    /// Draw as a flat ribbon patch, tessellated along length only.
    Ribbon,
    /// Draw as patch displaced into a half-tube shape.
    HalfTube,
}

/// Curve normal generation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CurveNormalStyle {
    /// Orient to user-supplied normals.
    Oriented,
    /// Camera-oriented normal, thin hair appearance.
    #[default]
    Hair,
    /// Camera-oriented normal faking a round tube cross-section.
    Round,
}

/// Shader key for basis curve rendering.
///
/// Determines which shader mixins to include for each pipeline stage
/// based on curve type, basis, draw/normal style, and feature flags.
/// Used as a cache key for compiled pipelines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BasisCurvesShaderKey {
    /// Curve type token (e.g. "linear", "cubic")
    pub curve_type: Token,
    /// Curve basis token (e.g. "bezier", "bspline", "catmullRom")
    pub basis: Token,
    /// How to draw the curve
    pub draw_style: CurveDrawStyle,
    /// How to generate normals
    pub normal_style: CurveNormalStyle,
    /// Interpolate width along the basis (vs constant)
    pub basis_width_interp: bool,
    /// Interpolate normals along the basis
    pub basis_normal_interp: bool,
    /// Shading terminal token (e.g. "surfaceShader")
    pub shading_terminal: Token,
    /// Has authored topological visibility
    pub has_topo_visibility: bool,
    /// Point-based shading enabled
    pub points_shading: bool,
    /// Use metal tessellation path
    pub metal_tessellation: bool,
    /// Use native round points (hardware point rasterization)
    pub native_round_points: bool,

    // Cached stage mixins
    /// GLSLFX/WGSLFX source file token
    pub glslfx: Token,
    /// Vertex shader mixins
    pub vs: Vec<Token>,
    /// Tessellation control shader mixins
    pub tcs: Vec<Token>,
    /// Tessellation evaluation shader mixins
    pub tes: Vec<Token>,
    /// Post-tessellation control shader mixins
    pub ptcs: Vec<Token>,
    /// Post-tessellation vertex shader mixins
    pub ptvs: Vec<Token>,
    /// Fragment shader mixins
    pub fs: Vec<Token>,
}

impl BasisCurvesShaderKey {
    /// Build a shader key for the given curve configuration.
    ///
    /// Populates stage mixin lists based on the draw style, normal style,
    /// and feature flags (mirroring C++ HdSt_BasisCurvesShaderKey ctor).
    pub fn new(
        curve_type: Token,
        basis: Token,
        draw_style: CurveDrawStyle,
        normal_style: CurveNormalStyle,
        basis_width_interp: bool,
        basis_normal_interp: bool,
        shading_terminal: Token,
        has_topo_visibility: bool,
        points_shading: bool,
        metal_tessellation: bool,
        native_round_points: bool,
    ) -> Self {
        let glslfx = Token::new("basisCurves.glslfx");

        // -- Vertex shader mixins --
        let mut vs = vec![
            Token::new("Instancing.Transform"),
            Token::new("Curves.Vertex.Common"),
        ];
        match draw_style {
            CurveDrawStyle::Points => {
                vs.push(Token::new("Curves.Vertex.Points"));
                if native_round_points {
                    vs.push(Token::new("Curves.Vertex.NativeRoundPoints"));
                }
            }
            CurveDrawStyle::Wire => {
                vs.push(Token::new("Curves.Vertex.Wire"));
            }
            CurveDrawStyle::Ribbon | CurveDrawStyle::HalfTube => {
                vs.push(Token::new("Curves.Vertex.Patch"));
                if basis_width_interp {
                    vs.push(Token::new("Curves.Vertex.BasisWidthInterp"));
                }
                if basis_normal_interp {
                    vs.push(Token::new("Curves.Vertex.BasisNormalInterp"));
                }
            }
        }
        if has_topo_visibility {
            vs.push(Token::new("Curves.Vertex.Visibility"));
        }

        // -- Tessellation control shader (for ribbon/halftube) --
        let mut tcs = Vec::new();
        let mut tes = Vec::new();
        let mut ptcs = Vec::new();
        let mut ptvs = Vec::new();

        let is_patch = matches!(
            draw_style,
            CurveDrawStyle::Ribbon | CurveDrawStyle::HalfTube
        );

        if is_patch {
            if metal_tessellation {
                // Metal post-tessellation path
                ptcs.push(Token::new("Curves.PostTessControl.Patch"));
                ptcs.push(Token::new("Curves.PostTessControl.Common"));
                if basis_width_interp {
                    ptcs.push(Token::new("Curves.PostTessControl.BasisWidthInterp"));
                }

                ptvs.push(Token::new("Curves.PostTessVertex.Patch"));
                ptvs.push(Token::new("Curves.PostTessVertex.Common"));
                if matches!(draw_style, CurveDrawStyle::HalfTube) {
                    ptvs.push(Token::new("Curves.PostTessVertex.HalfTube"));
                }
                if basis_width_interp {
                    ptvs.push(Token::new("Curves.PostTessVertex.BasisWidthInterp"));
                }
                if basis_normal_interp {
                    ptvs.push(Token::new("Curves.PostTessVertex.BasisNormalInterp"));
                }
            } else {
                // Standard tessellation path
                tcs.push(Token::new("Curves.TessControl.Patch"));
                tcs.push(Token::new("Curves.TessControl.Common"));
                if basis_width_interp {
                    tcs.push(Token::new("Curves.TessControl.BasisWidthInterp"));
                }

                tes.push(Token::new("Curves.TessEval.Patch"));
                tes.push(Token::new("Curves.TessEval.Common"));
                if matches!(draw_style, CurveDrawStyle::HalfTube) {
                    tes.push(Token::new("Curves.TessEval.HalfTube"));
                }
                if basis_width_interp {
                    tes.push(Token::new("Curves.TessEval.BasisWidthInterp"));
                }
                if basis_normal_interp {
                    tes.push(Token::new("Curves.TessEval.BasisNormalInterp"));
                }
            }
        }

        // -- Fragment shader mixins --
        let mut fs = vec![Token::new("Curves.Fragment.Common")];
        match draw_style {
            CurveDrawStyle::Points => {
                fs.push(Token::new("Curves.Fragment.Points"));
                if native_round_points {
                    fs.push(Token::new("Curves.Fragment.NativeRoundPoints"));
                }
            }
            CurveDrawStyle::Wire => {
                fs.push(Token::new("Curves.Fragment.Wire"));
            }
            CurveDrawStyle::Ribbon => {
                fs.push(Token::new("Curves.Fragment.Ribbon"));
                match normal_style {
                    CurveNormalStyle::Oriented => {
                        fs.push(Token::new("Curves.Fragment.Oriented"));
                    }
                    CurveNormalStyle::Hair => {
                        fs.push(Token::new("Curves.Fragment.Hair"));
                    }
                    CurveNormalStyle::Round => {
                        fs.push(Token::new("Curves.Fragment.Round"));
                    }
                }
            }
            CurveDrawStyle::HalfTube => {
                fs.push(Token::new("Curves.Fragment.HalfTube"));
                fs.push(Token::new("Curves.Fragment.Round"));
            }
        }
        if points_shading {
            fs.push(Token::new("Curves.Fragment.PointsShading"));
        }
        if !shading_terminal.is_empty() {
            fs.push(shading_terminal.clone());
        }

        Self {
            curve_type,
            basis,
            draw_style,
            normal_style,
            basis_width_interp,
            basis_normal_interp,
            shading_terminal,
            has_topo_visibility,
            points_shading,
            metal_tessellation,
            native_round_points,
            glslfx,
            vs,
            tcs,
            tes,
            ptcs,
            ptvs,
            fs,
        }
    }

    /// Compute a u64 hash for pipeline cache lookup.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Whether this key uses tessellation (ribbon or halftube).
    pub fn uses_tessellation(&self) -> bool {
        matches!(
            self.draw_style,
            CurveDrawStyle::Ribbon | CurveDrawStyle::HalfTube
        )
    }

    /// Whether this key uses metal post-tessellation path.
    pub fn uses_metal_tessellation(&self) -> bool {
        self.metal_tessellation && self.uses_tessellation()
    }

    /// Whether this is a points-only draw (no curves).
    pub fn is_points_only(&self) -> bool {
        matches!(self.draw_style, CurveDrawStyle::Points)
    }
}

impl Default for BasisCurvesShaderKey {
    fn default() -> Self {
        Self::new(
            Token::new("linear"),
            Token::default(),
            CurveDrawStyle::Wire,
            CurveNormalStyle::Hair,
            false,
            false,
            Token::default(),
            false,
            false,
            false,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_linear() {
        let key = BasisCurvesShaderKey::new(
            Token::new("linear"),
            Token::default(),
            CurveDrawStyle::Wire,
            CurveNormalStyle::Hair,
            false,
            false,
            Token::default(),
            false,
            false,
            false,
            false,
        );
        assert!(!key.uses_tessellation());
        assert!(!key.is_points_only());
        assert!(key.vs.iter().any(|t| t.as_str().contains("Wire")));
        assert!(key.fs.iter().any(|t| t.as_str().contains("Wire")));
        assert!(key.tcs.is_empty());
    }

    #[test]
    fn test_ribbon_cubic() {
        let key = BasisCurvesShaderKey::new(
            Token::new("cubic"),
            Token::new("bezier"),
            CurveDrawStyle::Ribbon,
            CurveNormalStyle::Oriented,
            true,
            true,
            Token::new("surfaceShader"),
            false,
            false,
            false,
            false,
        );
        assert!(key.uses_tessellation());
        assert!(!key.tcs.is_empty());
        assert!(!key.tes.is_empty());
        assert!(key.fs.iter().any(|t| t.as_str().contains("Ribbon")));
        assert!(key.fs.iter().any(|t| t.as_str().contains("Oriented")));
    }

    #[test]
    fn test_halftube() {
        let key = BasisCurvesShaderKey::new(
            Token::new("cubic"),
            Token::new("bspline"),
            CurveDrawStyle::HalfTube,
            CurveNormalStyle::Round,
            false,
            false,
            Token::default(),
            false,
            false,
            false,
            false,
        );
        assert!(key.uses_tessellation());
        assert!(key.fs.iter().any(|t| t.as_str().contains("HalfTube")));
        assert!(key.fs.iter().any(|t| t.as_str().contains("Round")));
    }

    #[test]
    fn test_points_draw_style() {
        let key = BasisCurvesShaderKey::new(
            Token::new("linear"),
            Token::default(),
            CurveDrawStyle::Points,
            CurveNormalStyle::Hair,
            false,
            false,
            Token::default(),
            false,
            false,
            false,
            true,
        );
        assert!(key.is_points_only());
        assert!(
            key.vs
                .iter()
                .any(|t| t.as_str().contains("NativeRoundPoints"))
        );
    }

    #[test]
    fn test_metal_tessellation() {
        let key = BasisCurvesShaderKey::new(
            Token::new("cubic"),
            Token::new("catmullRom"),
            CurveDrawStyle::Ribbon,
            CurveNormalStyle::Hair,
            true,
            false,
            Token::default(),
            false,
            false,
            true,
            false,
        );
        assert!(key.uses_metal_tessellation());
        assert!(!key.ptcs.is_empty());
        assert!(!key.ptvs.is_empty());
        // Standard tessellation should be empty when metal path is used
        assert!(key.tcs.is_empty());
        assert!(key.tes.is_empty());
    }

    #[test]
    fn test_cache_hash_differs() {
        let k1 = BasisCurvesShaderKey::default();
        let k2 = BasisCurvesShaderKey::new(
            Token::new("cubic"),
            Token::new("bezier"),
            CurveDrawStyle::Ribbon,
            CurveNormalStyle::Round,
            true,
            true,
            Token::default(),
            false,
            false,
            false,
            false,
        );
        assert_ne!(k1.cache_hash(), k2.cache_hash());
    }
}
