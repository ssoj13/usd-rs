//! Spline interpolation matching OSL's `opspline.cpp`.
//!
//! Supports: Catmull-Rom, B-spline, Bezier, Hermite, Linear, Constant.
//! Each spline type operates on a set of knots and evaluates at parameter t.

use crate::Float;
use crate::math::Color3;

/// Spline basis types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplineBasis {
    CatmullRom,
    BSpline,
    Bezier,
    Hermite,
    Linear,
    Constant,
}

impl SplineBasis {
    /// Knot step size per segment for this basis (from splineimpl.h).
    /// CatmullRom/BSpline/Linear/Constant = 1, Hermite = 2, Bezier = 3.
    pub fn step(&self) -> usize {
        match self {
            SplineBasis::CatmullRom => 1,
            SplineBasis::BSpline => 1,
            SplineBasis::Bezier => 3,
            SplineBasis::Hermite => 2,
            SplineBasis::Linear => 1,
            SplineBasis::Constant => 1,
        }
    }

    /// Parse from OSL string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "catmull-rom" | "catmullrom" => Some(SplineBasis::CatmullRom),
            "bspline" | "b-spline" => Some(SplineBasis::BSpline),
            "bezier" => Some(SplineBasis::Bezier),
            "hermite" => Some(SplineBasis::Hermite),
            "linear" => Some(SplineBasis::Linear),
            "constant" => Some(SplineBasis::Constant),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Basis matrices (row-major, for [t^3, t^2, t, 1] * M * [P0, P1, P2, P3]^T)
// ---------------------------------------------------------------------------

/// Catmull-Rom basis matrix.
const CATMULL_ROM: [[Float; 4]; 4] = [
    [-0.5, 1.5, -1.5, 0.5],
    [1.0, -2.5, 2.0, -0.5],
    [-0.5, 0.0, 0.5, 0.0],
    [0.0, 1.0, 0.0, 0.0],
];

/// B-spline basis matrix.
const BSPLINE: [[Float; 4]; 4] = [
    [-1.0 / 6.0, 3.0 / 6.0, -3.0 / 6.0, 1.0 / 6.0],
    [3.0 / 6.0, -6.0 / 6.0, 3.0 / 6.0, 0.0],
    [-3.0 / 6.0, 0.0, 3.0 / 6.0, 0.0],
    [1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0, 0.0],
];

/// Bezier basis matrix.
const BEZIER: [[Float; 4]; 4] = [
    [-1.0, 3.0, -3.0, 1.0],
    [3.0, -6.0, 3.0, 0.0],
    [-3.0, 3.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
];

/// Hermite basis matrix.
/// Column order: [P0, T0, P1, T1] matching C++ splineimpl.h.
const HERMITE: [[Float; 4]; 4] = [
    [2.0, 1.0, -2.0, 1.0],
    [-3.0, -2.0, 3.0, -1.0],
    [0.0, 1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
];

// ---------------------------------------------------------------------------
// Scalar spline
// ---------------------------------------------------------------------------

/// Evaluate a cubic spline at parameter `t` using the given basis.
/// `knots` must have at least 4 elements for cubic, 2 for linear, 1 for constant.
pub fn spline_float(basis: SplineBasis, t: Float, knots: &[Float]) -> Float {
    let nknots = knots.len();
    if nknots == 0 {
        return 0.0;
    }
    // C++ splineimpl.h:196 — clamp input parameter to [0,1]
    let t = t.clamp(0.0, 1.0);

    match basis {
        SplineBasis::Constant => {
            // C++ uses the same nsegs formula as cubic, returns knots[segnum+1].
            // This requires at least 4 knots (first/last are phantom).
            if nknots < 4 {
                return knots[0];
            }
            let nsegs = nknots - 3; // (nknots - 4) / 1 + 1
            let seg = ((t * nsegs as Float) as usize).min(nsegs - 1);
            knots[seg + 1]
        }
        SplineBasis::Linear => {
            // C++ linear spline also uses (nknots-4)/step+1 formula with 4-knot
            // basis matrix. First/last knots are phantom (same as cubic).
            if nknots < 4 {
                return if nknots > 0 { knots[0] } else { 0.0 };
            }
            let nsegs = nknots - 3; // (nknots-4)/1 + 1, step=1 for linear
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let frac = t_scaled - seg as Float;
            // Linear interp between knots[seg+1] and knots[seg+2] (skip phantom)
            knots[seg + 1] * (1.0 - frac) + knots[seg + 2] * frac
        }
        _ => {
            // Cubic spline
            if nknots < 4 {
                return if nknots > 0 { knots[0] } else { 0.0 };
            }
            let basis_matrix = match basis {
                SplineBasis::CatmullRom => &CATMULL_ROM,
                SplineBasis::BSpline => &BSPLINE,
                SplineBasis::Bezier => &BEZIER,
                SplineBasis::Hermite => &HERMITE,
                _ => unreachable!(),
            };

            let step = basis.step();
            let nsegs = ((nknots - 4) / step) + 1;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let u = t_scaled - seg as Float;

            let s = seg * step;
            let p = [knots[s], knots[s + 1], knots[s + 2], knots[s + 3]];
            eval_cubic(basis_matrix, u, &p)
        }
    }
}

/// Evaluate a cubic basis for 4 knot values at parameter u ∈ [0,1].
fn eval_cubic(m: &[[Float; 4]; 4], u: Float, p: &[Float; 4]) -> Float {
    let u2 = u * u;
    let u3 = u2 * u;

    // [u^3, u^2, u, 1] * M * [P0, P1, P2, P3]^T
    let mut result = 0.0;
    for j in 0..4 {
        let coeff = m[0][j] * u3 + m[1][j] * u2 + m[2][j] * u + m[3][j];
        result += coeff * p[j];
    }
    result
}

// ---------------------------------------------------------------------------
// Color spline
// ---------------------------------------------------------------------------

/// Evaluate a color spline at parameter `t`.
pub fn spline_color(basis: SplineBasis, t: Float, knots: &[Color3]) -> Color3 {
    let nknots = knots.len();
    if nknots == 0 {
        return Color3::ZERO;
    }
    let t = t.clamp(0.0, 1.0);

    match basis {
        SplineBasis::Constant => {
            if nknots < 4 {
                return if nknots > 0 { knots[0] } else { Color3::ZERO };
            }
            let nsegs = nknots - 3;
            let seg = ((t * nsegs as Float) as usize).min(nsegs - 1);
            knots[seg + 1]
        }
        SplineBasis::Linear => {
            if nknots < 4 {
                return if nknots > 0 { knots[0] } else { Color3::ZERO };
            }
            let nsegs = nknots - 3;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let frac = t_scaled - seg as Float;
            knots[seg + 1] * (1.0 - frac) + knots[seg + 2] * frac
        }
        _ => {
            if nknots < 4 {
                return if nknots > 0 { knots[0] } else { Color3::ZERO };
            }
            let basis_matrix = match basis {
                SplineBasis::CatmullRom => &CATMULL_ROM,
                SplineBasis::BSpline => &BSPLINE,
                SplineBasis::Bezier => &BEZIER,
                SplineBasis::Hermite => &HERMITE,
                _ => unreachable!(),
            };

            let step = basis.step();
            let nsegs = ((nknots - 4) / step) + 1;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let u = t_scaled - seg as Float;

            let s = seg * step;
            Color3::new(
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].x, knots[s + 1].x, knots[s + 2].x, knots[s + 3].x],
                ),
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].y, knots[s + 1].y, knots[s + 2].y, knots[s + 3].y],
                ),
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].z, knots[s + 1].z, knots[s + 2].z, knots[s + 3].z],
                ),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Vec3 spline
// ---------------------------------------------------------------------------

/// Evaluate a Vec3-valued spline at parameter `t`.
pub fn spline_vec3(basis: SplineBasis, t: Float, knots: &[crate::math::Vec3]) -> crate::math::Vec3 {
    let nknots = knots.len();
    if nknots == 0 {
        return crate::math::Vec3::ZERO;
    }
    // C++ splineimpl.h:196 — clamp input to [0,1]
    let t = t.clamp(0.0, 1.0);

    match basis {
        SplineBasis::Constant => {
            if nknots < 4 {
                return if nknots > 0 {
                    knots[0]
                } else {
                    crate::math::Vec3::ZERO
                };
            }
            let nsegs = nknots - 3;
            let seg = ((t * nsegs as Float) as usize).min(nsegs - 1);
            knots[seg + 1]
        }
        SplineBasis::Linear => {
            // C++ linear uses same phantom-knot formula as cubic
            if nknots < 4 {
                return if nknots > 0 {
                    knots[0]
                } else {
                    crate::math::Vec3::ZERO
                };
            }
            let nsegs = nknots - 3;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let frac = t_scaled - seg as Float;
            knots[seg + 1] * (1.0 - frac) + knots[seg + 2] * frac
        }
        _ => {
            if nknots < 4 {
                return if nknots > 0 {
                    knots[0]
                } else {
                    crate::math::Vec3::ZERO
                };
            }
            let basis_matrix = match basis {
                SplineBasis::CatmullRom => &CATMULL_ROM,
                SplineBasis::BSpline => &BSPLINE,
                SplineBasis::Bezier => &BEZIER,
                SplineBasis::Hermite => &HERMITE,
                _ => unreachable!(),
            };
            let step = basis.step();
            let nsegs = ((nknots - 4) / step) + 1;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let u = t_scaled - seg as Float;
            let s = seg * step;
            crate::math::Vec3::new(
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].x, knots[s + 1].x, knots[s + 2].x, knots[s + 3].x],
                ),
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].y, knots[s + 1].y, knots[s + 2].y, knots[s + 3].y],
                ),
                eval_cubic(
                    basis_matrix,
                    u,
                    &[knots[s].z, knots[s + 1].z, knots[s + 2].z, knots[s + 3].z],
                ),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Spline derivative (dt)
// ---------------------------------------------------------------------------

/// Evaluate the derivative of the cubic basis at parameter u.
fn eval_cubic_deriv(m: &[[Float; 4]; 4], u: Float, p: &[Float; 4]) -> Float {
    let u2 = u * u;
    let mut result = 0.0;
    for j in 0..4 {
        let coeff = 3.0 * m[0][j] * u2 + 2.0 * m[1][j] * u + m[2][j];
        result += coeff * p[j];
    }
    result
}

/// Evaluate the derivative of a scalar spline at parameter `t`.
pub fn spline_float_deriv(basis: SplineBasis, t: Float, knots: &[Float]) -> Float {
    let nknots = knots.len();
    if nknots < 4 {
        return 0.0;
    }
    let t = t.clamp(0.0, 1.0);
    match basis {
        SplineBasis::Constant => 0.0,
        SplineBasis::Linear => {
            let nsegs = nknots - 3;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            (knots[seg + 2] - knots[seg + 1]) * nsegs as Float
        }
        _ => {
            let basis_matrix = match basis {
                SplineBasis::CatmullRom => &CATMULL_ROM,
                SplineBasis::BSpline => &BSPLINE,
                SplineBasis::Bezier => &BEZIER,
                SplineBasis::Hermite => &HERMITE,
                _ => unreachable!(),
            };
            let step = basis.step();
            let nsegs = ((nknots - 4) / step) + 1;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let u = t_scaled - seg as Float;
            let s = seg * step;
            let p = [knots[s], knots[s + 1], knots[s + 2], knots[s + 3]];
            eval_cubic_deriv(basis_matrix, u, &p) * nsegs as Float
        }
    }
}

/// Derivative of a Vec3 spline w.r.t. the parameter `t`.
/// Returns d(spline_vec3)/dt.
pub fn spline_vec3_deriv(
    basis: SplineBasis,
    t: Float,
    knots: &[crate::math::Vec3],
) -> crate::math::Vec3 {
    let nknots = knots.len();
    if nknots < 4 {
        return crate::math::Vec3::ZERO;
    }
    let t = t.clamp(0.0, 1.0);
    match basis {
        SplineBasis::Constant => crate::math::Vec3::ZERO,
        SplineBasis::Linear => {
            let nsegs = nknots - 3;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            // Derivative of linear interp between phantom-adjusted knots
            (knots[seg + 2] - knots[seg + 1]) * nsegs as Float
        }
        _ => {
            let basis_matrix = match basis {
                SplineBasis::CatmullRom => &CATMULL_ROM,
                SplineBasis::BSpline => &BSPLINE,
                SplineBasis::Bezier => &BEZIER,
                SplineBasis::Hermite => &HERMITE,
                _ => unreachable!(),
            };
            let step = basis.step();
            let nsegs = ((nknots - 4) / step) + 1;
            let t_scaled = t * nsegs as Float;
            let seg = (t_scaled as usize).min(nsegs - 1);
            let u = t_scaled - seg as Float;
            let s = seg * step;
            let scale = nsegs as Float;
            crate::math::Vec3::new(
                eval_cubic_deriv(
                    basis_matrix,
                    u,
                    &[knots[s].x, knots[s + 1].x, knots[s + 2].x, knots[s + 3].x],
                ) * scale,
                eval_cubic_deriv(
                    basis_matrix,
                    u,
                    &[knots[s].y, knots[s + 1].y, knots[s + 2].y, knots[s + 3].y],
                ) * scale,
                eval_cubic_deriv(
                    basis_matrix,
                    u,
                    &[knots[s].z, knots[s + 1].z, knots[s + 2].z, knots[s + 3].z],
                ) * scale,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Dual2 spline (derivative propagation via chain rule)
// ---------------------------------------------------------------------------

/// Evaluate a scalar spline with `Dual2<f32>` parameter, propagating
/// input derivatives through the chain rule:
///   result.val = spline(t.val)
///   result.dx  = spline'(t.val) * t.dx
///   result.dy  = spline'(t.val) * t.dy
pub fn spline_float_dual(
    basis: SplineBasis,
    t: crate::dual::Dual2<Float>,
    knots: &[Float],
) -> crate::dual::Dual2<Float> {
    use crate::dual::Dual2;
    let val = spline_float(basis, t.val, knots);
    let deriv = spline_float_deriv(basis, t.val, knots);
    Dual2 {
        val,
        dx: deriv * t.dx,
        dy: deriv * t.dy,
    }
}

/// Evaluate a Vec3 spline with `Dual2<f32>` parameter, propagating
/// input derivatives through the chain rule.
pub fn spline_vec3_dual(
    basis: SplineBasis,
    t: crate::dual::Dual2<Float>,
    knots: &[crate::math::Vec3],
) -> crate::dual::Dual2<crate::math::Vec3> {
    use crate::dual::Dual2;
    let val = spline_vec3(basis, t.val, knots);
    let deriv = spline_vec3_deriv(basis, t.val, knots);
    Dual2 {
        val,
        dx: deriv * t.dx,
        dy: deriv * t.dy,
    }
}

// ---------------------------------------------------------------------------
// Spline inverse
// ---------------------------------------------------------------------------

/// Find t such that `spline(basis, t, knots) ≈ value`. Returns the parameter in [0, 1].
///
/// Implementation: 64-sample global linear search for initial bracket, then Newton-Raphson
/// refinement. C++ OSL uses OIIO::invert which employs Brent's method per-segment for
/// guaranteed convergence even on non-monotonic splines. Newton-Raphson can fail for
/// splines with local extrema within a segment (may miss roots or converge to wrong value).
/// For typical monotone control curves used in practice this difference is negligible.
pub fn spline_inverse_float(
    basis: SplineBasis,
    value: Float,
    knots: &[Float],
    max_iter: usize,
) -> Float {
    // Initial guess: linear search for bracketing interval
    let n = 64;
    let mut best_t = 0.5_f32;
    let mut best_diff = f32::MAX;

    for i in 0..=n {
        let t = i as Float / n as Float;
        let v = spline_float(basis, t, knots);
        let diff = (v - value).abs();
        if diff < best_diff {
            best_diff = diff;
            best_t = t;
        }
    }

    // Refine with Newton's method (numerical derivative)
    let mut t = best_t;
    let eps = 1e-6_f32;
    for _ in 0..max_iter {
        let v = spline_float(basis, t, knots);
        let err = v - value;
        if err.abs() < 1e-8 {
            break;
        }
        let dv = (spline_float(basis, t + eps, knots) - v) / eps;
        if dv.abs() < 1e-12 {
            break;
        }
        t -= err / dv;
        t = t.clamp(0.0, 1.0);
    }

    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_spline() {
        // C++ linear also uses phantom knots: first/last are phantom,
        // interpolation happens between knots[1..nknots-2].
        // With [0, 1, 2, 3]: phantom=0,3; interp between 1 and 2.
        let knots = [0.0, 1.0, 2.0, 3.0];
        assert!((spline_float(SplineBasis::Linear, 0.0, &knots) - 1.0).abs() < 1e-5);
        assert!((spline_float(SplineBasis::Linear, 0.5, &knots) - 1.5).abs() < 1e-5);
        assert!((spline_float(SplineBasis::Linear, 1.0, &knots) - 2.0).abs() < 1e-5);
        // With more knots: [0, 1, 2, 3, 4] -> phantom=0,4; 2 segments: [1,2] and [2,3]
        let knots5 = [0.0, 1.0, 2.0, 3.0, 4.0];
        assert!((spline_float(SplineBasis::Linear, 0.0, &knots5) - 1.0).abs() < 1e-5);
        assert!((spline_float(SplineBasis::Linear, 0.5, &knots5) - 2.0).abs() < 1e-5);
        assert!((spline_float(SplineBasis::Linear, 1.0, &knots5) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_constant_spline() {
        // C++ constant spline needs >= 4 knots (phantom first/last).
        // nsegs = (5-4)/1 + 1 = 2; returns knots[segnum+1]
        let knots = [0.0, 1.0, 2.0, 3.0, 4.0];
        // t=0.0 -> seg=0 -> knots[1]=1.0
        assert_eq!(spline_float(SplineBasis::Constant, 0.0, &knots), 1.0);
        // t=0.99 -> seg=1 -> knots[2]=2.0
        assert_eq!(spline_float(SplineBasis::Constant, 0.99, &knots), 2.0);
        // With exactly 4 knots: nsegs=1, always knots[1]
        let k4 = [10.0, 20.0, 30.0, 40.0];
        assert_eq!(spline_float(SplineBasis::Constant, 0.0, &k4), 20.0);
        assert_eq!(spline_float(SplineBasis::Constant, 1.0, &k4), 20.0);
        // Fewer than 4 knots: fallback to first knot
        let k2 = [5.0, 6.0];
        assert_eq!(spline_float(SplineBasis::Constant, 0.5, &k2), 5.0);
    }

    #[test]
    fn test_catmullrom_endpoints() {
        // With Catmull-Rom, the spline interpolates through the middle knots
        let knots = [0.0, 0.0, 1.0, 1.0];
        let v0 = spline_float(SplineBasis::CatmullRom, 0.0, &knots);
        let v1 = spline_float(SplineBasis::CatmullRom, 1.0, &knots);
        // Should be near 0.0 and 1.0
        assert!(v0.abs() < 0.2);
        assert!((v1 - 1.0).abs() < 0.2);
    }

    #[test]
    fn test_color_spline() {
        let knots = [
            Color3::new(1.0, 0.0, 0.0),
            Color3::new(1.0, 0.0, 0.0),
            Color3::new(0.0, 1.0, 0.0),
            Color3::new(0.0, 1.0, 0.0),
        ];
        let mid = spline_color(SplineBasis::CatmullRom, 0.5, &knots);
        // At t=0.5, should be between red and green
        assert!(mid.x > 0.0 || mid.y > 0.0);
    }

    #[test]
    fn test_spline_inverse() {
        let knots = [0.0, 0.0, 1.0, 1.0];
        let t = spline_inverse_float(SplineBasis::CatmullRom, 0.5, &knots, 20);
        let v = spline_float(SplineBasis::CatmullRom, t, &knots);
        assert!((v - 0.5).abs() < 0.01, "inverse gave t={t}, spline(t)={v}");
    }

    #[test]
    fn test_basis_from_name() {
        assert_eq!(
            SplineBasis::from_name("catmull-rom"),
            Some(SplineBasis::CatmullRom)
        );
        assert_eq!(
            SplineBasis::from_name("bspline"),
            Some(SplineBasis::BSpline)
        );
        assert_eq!(SplineBasis::from_name("linear"), Some(SplineBasis::Linear));
        assert_eq!(SplineBasis::from_name("unknown"), None);
    }

    #[test]
    fn test_basis_step() {
        assert_eq!(SplineBasis::CatmullRom.step(), 1);
        assert_eq!(SplineBasis::BSpline.step(), 1);
        assert_eq!(SplineBasis::Bezier.step(), 3);
        assert_eq!(SplineBasis::Hermite.step(), 2);
        assert_eq!(SplineBasis::Linear.step(), 1);
    }

    #[test]
    fn test_bezier_nsegs() {
        // 7 knots with step=3: nsegs = (7-4)/3 + 1 = 2 segments
        // Bezier: each segment uses 4 control points, segments overlap by 1
        // Segment 0: knots[0..3], Segment 1: knots[3..6]
        let knots = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0];
        let v0 = spline_float(SplineBasis::Bezier, 0.0, &knots);
        let v1 = spline_float(SplineBasis::Bezier, 1.0, &knots);
        // First segment starts at knots[0]=0, last segment ends at knots[6]=3
        assert!(v0.abs() < 0.5, "bezier start v0={v0}");
        assert!((v1 - 3.0).abs() < 0.5, "bezier end v1={v1}");
    }

    #[test]
    fn test_hermite_nsegs() {
        // 6 knots with step=2: nsegs = (6-4)/2 + 1 = 2 segments
        let knots = [0.0, 1.0, 1.0, 0.0, 2.0, -1.0];
        let v = spline_float(SplineBasis::Hermite, 0.5, &knots);
        // Should produce some reasonable interpolated value
        assert!(v.is_finite(), "hermite mid v={v}");
    }

    #[test]
    fn test_spline_float_dual_chain_rule() {
        use crate::dual::Dual2;
        // Linear spline with phantom knots: [0, 1, 2, 3]
        // Phantom: first=0, last=3. Interp between knots[1]=1.0 and knots[2]=2.0.
        // nsegs=1. f(0.5) = 1.5, f'(t) = nsegs*(2-1) = 1.0.
        // With t = Dual2(0.5, dx=1.0, dy=0.0): result.dx = 1.0 * 1.0 = 1.0
        let knots = [0.0, 1.0, 2.0, 3.0];
        let t = Dual2::new(0.5, 1.0, 0.0);
        let r = spline_float_dual(SplineBasis::Linear, t, &knots);
        assert!((r.val - 1.5).abs() < 1e-4, "val={}", r.val);
        assert!((r.dx - 1.0).abs() < 0.2, "dx={}", r.dx);
        assert!(r.dy.abs() < 1e-6, "dy={}", r.dy);
    }

    #[test]
    fn test_spline_float_dual_zero_derivs() {
        use crate::dual::Dual2;
        // Zero input derivatives -> zero output derivatives
        let knots = [0.0, 0.0, 1.0, 1.0];
        let t = Dual2::<Float>::from_val(0.5);
        let r = spline_float_dual(SplineBasis::CatmullRom, t, &knots);
        assert!(r.val.is_finite());
        assert!(r.dx.abs() < 1e-8, "dx should be zero: {}", r.dx);
        assert!(r.dy.abs() < 1e-8, "dy should be zero: {}", r.dy);
    }

    #[test]
    fn test_spline_float_dual_constant() {
        use crate::dual::Dual2;
        // Constant spline derivative is always 0, so output derivs = 0
        let knots = [1.0, 2.0, 3.0, 4.0, 5.0];
        let t = Dual2::new(0.3, 1.0, 2.0);
        let r = spline_float_dual(SplineBasis::Constant, t, &knots);
        assert!(r.dx.abs() < 1e-8);
        assert!(r.dy.abs() < 1e-8);
    }
}
