/// Box-spline triangle patch basis (12 CVs — cubic triangular box spline).
///
/// Mirrors `Osd_evalBivariateMonomialsQuartic`, `Osd_evalBoxSplineTriDerivWeights`,
/// `Osd_adjustBoxSplineTriBoundaryWeights`, `Osd_boundBasisBoxSplineTri`, and
/// `Osd_EvalBasisBoxSplineTri` from patchBasis.h.

/// Compute the 15 bivariate monomials up to degree 4.
///
/// M[0] = 1
/// M[1] = s,   M[2] = t
/// M[3] = s^2, M[4] = s*t, M[5] = t^2
/// ... (quartic terms)
#[inline]
fn eval_bivariate_monomials_quartic(s: f32, t: f32, m: &mut [f32; 15]) {
    m[0] = 1.0;
    m[1] = s;
    m[2] = t;
    m[3] = s * s;
    m[4] = s * t;
    m[5] = t * t;
    m[6] = m[3] * s;
    m[7] = m[4] * s;
    m[8] = m[4] * t;
    m[9] = m[5] * t;
    m[10] = m[6] * s;
    m[11] = m[7] * s;
    m[12] = m[3] * m[5];
    m[13] = m[8] * t;
    m[14] = m[9] * t;
}

/// Compute box-spline triangle derivative weights for the given order.
///
/// `ds` + `dt` = 0 → position weights
/// `ds` + `dt` = 1 → first partial derivative
/// `ds` + `dt` = 2 → second partial derivative
pub fn eval_box_spline_tri_deriv_weights(m: &[f32; 15], ds: i32, dt: i32, w: &mut [f32; 12]) {
    let total = ds + dt;
    match total {
        0 => {
            let s = 1.0_f32 / 12.0;
            w[0]  = s * (1.0 - 2.0*m[1] - 4.0*m[2]                + 6.0*m[4] + 6.0*m[5] + 2.0*m[6]           - 6.0*m[8] - 4.0*m[9] -     m[10] - 2.0*m[11] + 2.0*m[13] +     m[14]);
            w[1]  = s * (1.0 + 2.0*m[1] - 2.0*m[2]                - 6.0*m[4]             - 4.0*m[6]           + 6.0*m[8] + 2.0*m[9] + 2.0*m[10] + 4.0*m[11] - 2.0*m[13] -     m[14]);
            w[2]  = s * (                                                                  2.0*m[6]                                   -     m[10] - 2.0*m[11]                          );
            w[3]  = s * (1.0 - 4.0*m[1] - 2.0*m[2] + 6.0*m[3] + 6.0*m[4]               - 4.0*m[6] - 6.0*m[7]           + 2.0*m[9] +     m[10] + 2.0*m[11] - 2.0*m[13] -     m[14]);
            w[4]  = s * (6.0                        -12.0*m[3] -12.0*m[4] -12.0*m[5]   + 8.0*m[6] +12.0*m[7] +12.0*m[8] + 8.0*m[9] -     m[10] - 2.0*m[11] - 2.0*m[13] -     m[14]);
            w[5]  = s * (1.0 + 4.0*m[1] + 2.0*m[2] + 6.0*m[3] + 6.0*m[4]               - 4.0*m[6] - 6.0*m[7] -12.0*m[8] - 4.0*m[9] -     m[10] - 2.0*m[11] + 4.0*m[13] + 2.0*m[14]);
            w[6]  = s * (                                                                                                                   m[10] + 2.0*m[11]                          );
            w[7]  = s * (1.0 - 2.0*m[1] + 2.0*m[2]              - 6.0*m[4]              + 2.0*m[6] + 6.0*m[7]           - 4.0*m[9] -     m[10] - 2.0*m[11] + 4.0*m[13] + 2.0*m[14]);
            w[8]  = s * (1.0 + 2.0*m[1] + 4.0*m[2]              + 6.0*m[4] + 6.0*m[5]  - 4.0*m[6] -12.0*m[7] - 6.0*m[8] - 4.0*m[9] + 2.0*m[10] + 4.0*m[11] - 2.0*m[13] -     m[14]);
            w[9]  = s * (                                                                  2.0*m[6] + 6.0*m[7] + 6.0*m[8] + 2.0*m[9] -     m[10] - 2.0*m[11] - 2.0*m[13] -     m[14]);
            w[10] = s * (                                                                                        2.0*m[9]                                      - 2.0*m[13] -     m[14]);
            w[11] = s * (                                                                                                                                         2.0*m[13] +     m[14]);
        }
        1 if ds != 0 => {
            let s = 1.0_f32 / 6.0;
            w[0]  = s * (-1.0            + 3.0*m[2] + 3.0*m[3]             - 3.0*m[5] - 2.0*m[6] - 3.0*m[7] +     m[9]);
            w[1]  = s * ( 1.0            - 3.0*m[2] - 6.0*m[3]             + 3.0*m[5] + 4.0*m[6] + 6.0*m[7] -     m[9]);
            w[2]  = s * (                              3.0*m[3]                       - 2.0*m[6] - 3.0*m[7]             );
            w[3]  = s * (-2.0 + 6.0*m[1] + 3.0*m[2] - 6.0*m[3] - 6.0*m[4]            + 2.0*m[6] + 3.0*m[7] -     m[9]);
            w[4]  = s * (      -12.0*m[1] - 6.0*m[2] +12.0*m[3] +12.0*m[4] + 6.0*m[5] - 2.0*m[6] - 3.0*m[7] -     m[9]);
            w[5]  = s * ( 2.0 + 6.0*m[1] + 3.0*m[2] - 6.0*m[3] - 6.0*m[4] - 6.0*m[5] - 2.0*m[6] - 3.0*m[7] + 2.0*m[9]);
            w[6]  = s * (                                                               2.0*m[6] + 3.0*m[7]              );
            w[7]  = s * (-1.0            - 3.0*m[2] + 3.0*m[3] + 6.0*m[4]             - 2.0*m[6] - 3.0*m[7] + 2.0*m[9]);
            w[8]  = s * ( 1.0            + 3.0*m[2] - 6.0*m[3] -12.0*m[4] - 3.0*m[5] + 4.0*m[6] + 6.0*m[7] -     m[9]);
            w[9]  = s * (                              3.0*m[3] + 6.0*m[4] + 3.0*m[5] - 2.0*m[6] - 3.0*m[7] -     m[9]);
            w[10] = s * (                                                                                      -     m[9]);
            w[11] = s * (                                                                                            m[9]);
        }
        1 => { // dt != 0
            let s = 1.0_f32 / 6.0;
            w[0]  = s * (-2.0 + 3.0*m[1] + 6.0*m[2]              - 6.0*m[4] - 6.0*m[5] -     m[6] + 3.0*m[8] + 2.0*m[9]);
            w[1]  = s * (-1.0 - 3.0*m[1]              + 6.0*m[4] + 3.0*m[5] + 2.0*m[6] - 3.0*m[8] - 2.0*m[9]);
            w[2]  = s * (                                                      -     m[6]                               );
            w[3]  = s * (-1.0 + 3.0*m[1]              - 3.0*m[3]             + 3.0*m[5] +     m[6] - 3.0*m[8] - 2.0*m[9]);
            w[4]  = s * (      - 6.0*m[1] -12.0*m[2] + 6.0*m[3] +12.0*m[4] +12.0*m[5] -     m[6] - 3.0*m[8] - 2.0*m[9]);
            w[5]  = s * ( 1.0 + 3.0*m[1]              - 3.0*m[3] -12.0*m[4] - 6.0*m[5] -     m[6] + 6.0*m[8] + 4.0*m[9]);
            w[6]  = s * (                                                            m[6]                               );
            w[7]  = s * ( 1.0 - 3.0*m[1]              + 3.0*m[3]             - 6.0*m[5] -     m[6] + 6.0*m[8] + 4.0*m[9]);
            w[8]  = s * ( 2.0 + 3.0*m[1] + 6.0*m[2] - 6.0*m[3] - 6.0*m[4] - 6.0*m[5] + 2.0*m[6] - 3.0*m[8] - 2.0*m[9]);
            w[9]  = s * (                    3.0*m[3] + 6.0*m[4] + 3.0*m[5] -     m[6] - 3.0*m[8] - 2.0*m[9]);
            w[10] = s * (                               3.0*m[5]              - 3.0*m[8] - 2.0*m[9]);
            w[11] = s * (                                                        3.0*m[8] + 2.0*m[9]);
        }
        2 if ds == 2 => {
            w[0]  =       m[1]              -     m[3] -     m[4];
            w[1]  =  -2.0*m[1]             + 2.0*m[3] + 2.0*m[4];
            w[2]  =       m[1]              -     m[3] -     m[4];
            w[3]  =  1.0 - 2.0*m[1] -     m[2] +     m[3] +     m[4];
            w[4]  = -2.0 + 4.0*m[1] + 2.0*m[2] -     m[3] -     m[4];
            w[5]  =  1.0 - 2.0*m[1] -     m[2] -     m[3] -     m[4];
            w[6]  =                               m[3] +     m[4];
            w[7]  =       m[1] +     m[2] -     m[3] -     m[4];
            w[8]  =  -2.0*m[1] - 2.0*m[2] + 2.0*m[3] + 2.0*m[4];
            w[9]  =       m[1] +     m[2] -     m[3] -     m[4];
            w[10] = 0.0;
            w[11] = 0.0;
        }
        2 if dt == 2 => {
            w[0]  =  1.0 -     m[1] - 2.0*m[2] +     m[4] +     m[5];
            w[1]  =             m[1] +     m[2] -     m[4] -     m[5];
            w[2]  =  0.0;
            w[3]  =                         m[2] -     m[4] -     m[5];
            w[4]  = -2.0 + 2.0*m[1] + 4.0*m[2] -     m[4] -     m[5];
            w[5]  =  -2.0*m[1] - 2.0*m[2] + 2.0*m[4] + 2.0*m[5];
            w[6]  =  0.0;
            w[7]  =               - 2.0*m[2] + 2.0*m[4] + 2.0*m[5];
            w[8]  =  1.0 -     m[1] - 2.0*m[2] -     m[4] -     m[5];
            w[9]  =             m[1] +     m[2] -     m[4] -     m[5];
            w[10] =                         m[2] -     m[4] -     m[5];
            w[11] =                               m[4] +     m[5];
        }
        2 => { // ds==1, dt==1
            let s = 0.5_f32;
            w[0]  = s * ( 1.0          - 2.0*m[2] -     m[3] +     m[5]);
            w[1]  = s * (-1.0          + 2.0*m[2] + 2.0*m[3] -     m[5]);
            w[2]  = s * (                           -     m[3]            );
            w[3]  = s * ( 1.0 - 2.0*m[1]           +     m[3] -     m[5]);
            w[4]  = s * (-2.0 + 4.0*m[1] + 4.0*m[2] -     m[3] -     m[5]);
            w[5]  = s * ( 1.0 - 2.0*m[1] - 4.0*m[2] -     m[3] + 2.0*m[5]);
            w[6]  = s * (                              m[3]               );
            w[7]  = s * (-1.0 + 2.0*m[1]           -     m[3] + 2.0*m[5]);
            w[8]  = s * ( 1.0 - 4.0*m[1] - 2.0*m[2] + 2.0*m[3] -     m[5]);
            w[9]  = s * (       2.0*m[1] + 2.0*m[2] -     m[3] -     m[5]);
            w[10] = s * (                                          -     m[5]);
            w[11] = s * (                                                m[5]);
        }
        _ => {}
    }
}

/// Adjust boundary phantom-point weights for a triangular box-spline patch.
///
/// Mirrors `Osd_adjustBoxSplineTriBoundaryWeights` from `patchBasis.h`.
///
/// The C++ reference for this function is in `far/patchBasis.cpp`
/// (the osd/ header only provides the GPU GLSL variant).  The
/// Rust implementation decomposes `boundary_mask` into
/// `upper_bits` / `lower_bits` / `e_bits` / `v_bits` following
/// the same boundary-folding logic as the C++ source.
pub fn adjust_box_spline_tri_boundary_weights(boundary_mask: i32, weights: &mut [f32; 12]) {
    if boundary_mask == 0 { return; }

    let upper_bits = (boundary_mask >> 3) & 0x3;
    let lower_bits = boundary_mask & 7;

    let mut e_bits = lower_bits;
    let mut v_bits = 0i32;

    if upper_bits == 1 {
        v_bits = e_bits;
        e_bits = 0;
    } else if upper_bits == 2 {
        v_bits = ((e_bits & 1) << 2) | (e_bits >> 1);
    }

    let edge0 = (e_bits & 1) != 0;
    let edge1 = (e_bits & 2) != 0;
    let edge2 = (e_bits & 4) != 0;

    if edge0 {
        let w0 = weights[0];
        if edge2 {
            weights[4] += w0 * 2.0;
            weights[8] -= w0;
        } else {
            weights[4] += w0;
            weights[3] += w0;
            weights[7] -= w0;
        }
        let w1 = weights[1];
        weights[4] += w1;
        weights[5] += w1;
        weights[8] -= w1;
        let w2 = weights[2];
        if edge1 {
            weights[5] += w2 * 2.0;
            weights[8] -= w2;
        } else {
            weights[5] += w2;
            weights[6] += w2;
            weights[9] -= w2;
        }
        weights[0] = 0.0; weights[1] = 0.0; weights[2] = 0.0;
    }
    if edge1 {
        let w0 = weights[6];
        if edge0 {
            weights[5] += w0 * 2.0;
            weights[4] -= w0;
        } else {
            weights[5] += w0;
            weights[2] += w0;
            weights[1] -= w0;
        }
        let w1 = weights[9];
        weights[5] += w1;
        weights[8] += w1;
        weights[4] -= w1;
        let w2 = weights[11];
        if edge2 {
            weights[8] += w2 * 2.0;
            weights[4] -= w2;
        } else {
            weights[8]  += w2;
            weights[10] += w2;
            weights[7]  -= w2;
        }
        weights[6] = 0.0; weights[9] = 0.0; weights[11] = 0.0;
    }
    if edge2 {
        let w0 = weights[10];
        if edge1 {
            weights[8] += w0 * 2.0;
            weights[5] -= w0;
        } else {
            weights[8]  += w0;
            weights[11] += w0;
            weights[9]  -= w0;
        }
        let w1 = weights[7];
        weights[8] += w1;
        weights[4] += w1;
        weights[5] -= w1;
        let w2 = weights[3];
        if edge0 {
            weights[4] += w2 * 2.0;
            weights[5] -= w2;
        } else {
            weights[4] += w2;
            weights[0] += w2;
            weights[1] -= w2;
        }
        weights[10] = 0.0; weights[7] = 0.0; weights[3] = 0.0;
    }

    if (v_bits & 1) != 0 {
        let w0 = weights[3];
        weights[4] += w0;
        weights[7] += w0;
        weights[8] -= w0;
        let w1 = weights[0];
        weights[4] += w1;
        weights[1] += w1;
        weights[5] -= w1;
        weights[3] = 0.0; weights[0] = 0.0;
    }
    if (v_bits & 2) != 0 {
        let w0 = weights[2];
        weights[5] += w0;
        weights[1] += w0;
        weights[4] -= w0;
        let w1 = weights[6];
        weights[5] += w1;
        weights[9] += w1;
        weights[8] -= w1;
        weights[2] = 0.0; weights[6] = 0.0;
    }
    if (v_bits & 4) != 0 {
        let w0 = weights[11];
        weights[8] += w0;
        weights[9] += w0;
        weights[5] -= w0;
        let w1 = weights[10];
        weights[8] += w1;
        weights[7] += w1;
        weights[4] -= w1;
        weights[11] = 0.0; weights[10] = 0.0;
    }
}

/// Apply boundary adjustments to all weight arrays (position + derivatives).
pub fn bound_basis_box_spline_tri(
    boundary: i32,
    wp:   Option<&mut [f32; 12]>,
    wds:  Option<&mut [f32; 12]>,
    wdt:  Option<&mut [f32; 12]>,
    wdss: Option<&mut [f32; 12]>,
    wdst: Option<&mut [f32; 12]>,
    wdtt: Option<&mut [f32; 12]>,
) {
    if let Some(w) = wp {
        adjust_box_spline_tri_boundary_weights(boundary, w);
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        adjust_box_spline_tri_boundary_weights(boundary, ds);
        adjust_box_spline_tri_boundary_weights(boundary, dt);
        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            adjust_box_spline_tri_boundary_weights(boundary, dss);
            adjust_box_spline_tri_boundary_weights(boundary, dst);
            adjust_box_spline_tri_boundary_weights(boundary, dtt);
        }
    }
}

/// Evaluate cubic triangular box-spline basis.  Returns 12.
pub fn eval_basis_box_spline_tri(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 12]>,
    wds:  Option<&mut [f32; 12]>,
    wdt:  Option<&mut [f32; 12]>,
    wdss: Option<&mut [f32; 12]>,
    wdst: Option<&mut [f32; 12]>,
    wdtt: Option<&mut [f32; 12]>,
) -> i32 {
    let mut m = [0f32; 15];
    eval_bivariate_monomials_quartic(s, t, &mut m);

    if let Some(w) = wp {
        eval_box_spline_tri_deriv_weights(&m, 0, 0, w);
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        eval_box_spline_tri_deriv_weights(&m, 1, 0, ds);
        eval_box_spline_tri_deriv_weights(&m, 0, 1, dt);
        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            eval_box_spline_tri_deriv_weights(&m, 2, 0, dss);
            eval_box_spline_tri_deriv_weights(&m, 1, 1, dst);
            eval_box_spline_tri_deriv_weights(&m, 0, 2, dtt);
        }
    }
    12
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_spline_tri_returns_12() {
        let mut wp = [0f32; 12];
        let n = eval_basis_box_spline_tri(0.3, 0.3, Some(&mut wp), None, None, None, None, None);
        assert_eq!(n, 12);
    }

    #[test]
    fn box_spline_tri_partition_of_unity() {
        // The box-spline basis sums to 1 everywhere inside the triangle.
        let mut wp = [0f32; 12];
        eval_basis_box_spline_tri(0.2, 0.3, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "sum = {}", wp.iter().sum::<f32>());
    }
}
