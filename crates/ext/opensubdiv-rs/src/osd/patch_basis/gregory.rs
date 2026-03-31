/// Gregory patch basis functions — quad (20 CVs) and triangle (18 CVs).
///
/// Mirrors `Osd_EvalBasisGregory` and `Osd_EvalBasisGregoryTri` from
/// patchBasis.h.  Uses the approximate derivative formulation (not the
/// `OPENSUBDIV_GREGORY_EVAL_TRUE_DERIVATIVES` path) by default, matching the
/// GPU shader behaviour.
use super::bezier::{eval_bezier_curve, eval_bezier_tri_deriv_weights};

// ---------------------------------------------------------------------------
//  Gregory quad patch (20 CVs)
// ---------------------------------------------------------------------------

/// Index tables mapping Gregory control points to the corresponding 4×4
/// Bezier grid positions.
const BOUNDARY_GREGORY: [usize; 12] = [0, 1, 7, 5, 2, 6, 16, 12, 15, 17, 11, 10];
const BOUNDARY_BEZ_S_COL: [usize; 12] = [0, 1, 2, 3, 0, 3, 0, 3, 0, 1, 2, 3];
const BOUNDARY_BEZ_T_ROW: [usize; 12] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 3, 3];

const INTERIOR_GREGORY:   [usize; 8] = [3, 4, 8, 9, 13, 14, 18, 19];
const INTERIOR_BEZ_S_COL: [usize; 8] = [1, 1, 2, 2,  2,  2,  1,  1];
const INTERIOR_BEZ_T_ROW: [usize; 8] = [1, 1, 1, 1,  2,  2,  2,  2];

/// Evaluate Gregory quad patch basis.  Returns 20.
///
/// Uses the approximated derivative (same as GPU shaders).
pub fn eval_basis_gregory(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 20]>,
    wds:  Option<&mut [f32; 20]>,
    wdt:  Option<&mut [f32; 20]>,
    wdss: Option<&mut [f32; 20]>,
    wdst: Option<&mut [f32; 20]>,
    wdtt: Option<&mut [f32; 20]>,
) -> i32 {
    let need_d1 = wds.is_some() || wdt.is_some();
    let need_d2 = wdss.is_some() || wdst.is_some() || wdtt.is_some();

    // Bezier curve bases at s and t
    let mut bs   = [0f32; 4];
    let mut bt   = [0f32; 4];
    let mut bds  = [0f32; 4];
    let mut bdt  = [0f32; 4];
    let mut bdss = [0f32; 4];
    let mut bdtt = [0f32; 4];

    eval_bezier_curve(s, Some(&mut bs),
        if need_d1 { Some(&mut bds)  } else { None },
        if need_d2 { Some(&mut bdss) } else { None });
    eval_bezier_curve(t, Some(&mut bt),
        if need_d1 { Some(&mut bdt)  } else { None },
        if need_d2 { Some(&mut bdtt) } else { None });

    // Rational multipliers G for interior points
    let sc = 1.0 - s;
    let tc = 1.0 - t;

    let df0 = { let d = s + t;   if d <= 0.0 { 1.0 } else { 1.0 / d } };
    let df1 = { let d = sc + t;  if d <= 0.0 { 1.0 } else { 1.0 / d } };
    let df2 = { let d = sc + tc; if d <= 0.0 { 1.0 } else { 1.0 / d } };
    let df3 = { let d = s + tc;  if d <= 0.0 { 1.0 } else { 1.0 / d } };

    let g: [f32; 8] = [
         s * df0, 1.0 -  s * df0,
         t * df1, 1.0 -  t * df1,
        sc * df2, 1.0 - sc * df2,
        tc * df3, 1.0 - tc * df3,
    ];

    // Position weights
    if let Some(w) = wp {
        for k in w.iter_mut() { *k = 0.0; }
        for i in 0..12 {
            w[BOUNDARY_GREGORY[i]] = bs[BOUNDARY_BEZ_S_COL[i]] * bt[BOUNDARY_BEZ_T_ROW[i]];
        }
        for j in 0..8 {
            w[INTERIOR_GREGORY[j]] = bs[INTERIOR_BEZ_S_COL[j]] * bt[INTERIOR_BEZ_T_ROW[j]] * g[j];
        }
    }

    // Derivative weights (approximate — Bezier differentiation of the
    // G-weighted patch, matching the GPU shader path)
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        for k in ds.iter_mut() { *k = 0.0; }
        for k in dt.iter_mut() { *k = 0.0; }
        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            for k in dss.iter_mut() { *k = 0.0; }
            for k in dst.iter_mut() { *k = 0.0; }
            for k in dtt.iter_mut() { *k = 0.0; }

            // Boundary points
            for i in 0..12 {
                let dst_i = BOUNDARY_GREGORY[i];
                let tr = BOUNDARY_BEZ_T_ROW[i];
                let sc_i = BOUNDARY_BEZ_S_COL[i];
                ds[dst_i]  = bds[sc_i]  * bt[tr];
                dt[dst_i]  = bdt[tr]    * bs[sc_i];
                dss[dst_i] = bdss[sc_i] * bt[tr];
                dst[dst_i] = bds[sc_i]  * bdt[tr];
                dtt[dst_i] = bs[sc_i]   * bdtt[tr];
            }
            // Interior points (approximate)
            for j in 0..8 {
                let dst_j = INTERIOR_GREGORY[j];
                let tr = INTERIOR_BEZ_T_ROW[j];
                let sc_j = INTERIOR_BEZ_S_COL[j];
                let gj = g[j];
                ds[dst_j]  = bds[sc_j]  * bt[tr]   * gj;
                dt[dst_j]  = bdt[tr]    * bs[sc_j]  * gj;
                dss[dst_j] = bdss[sc_j] * bt[tr]    * gj;
                dst[dst_j] = bds[sc_j]  * bdt[tr]   * gj;
                dtt[dst_j] = bs[sc_j]   * bdtt[tr]  * gj;
            }
        } else {
            // First derivs only
            for i in 0..12 {
                let dst_i = BOUNDARY_GREGORY[i];
                let tr = BOUNDARY_BEZ_T_ROW[i];
                let sc_i = BOUNDARY_BEZ_S_COL[i];
                ds[dst_i] = bds[sc_i] * bt[tr];
                dt[dst_i] = bdt[tr]   * bs[sc_i];
            }
            for j in 0..8 {
                let dst_j = INTERIOR_GREGORY[j];
                let tr = INTERIOR_BEZ_T_ROW[j];
                let sc_j = INTERIOR_BEZ_S_COL[j];
                let gj = g[j];
                ds[dst_j] = bds[sc_j] * bt[tr]  * gj;
                dt[dst_j] = bdt[tr]   * bs[sc_j] * gj;
            }
        }
    }
    20
}

// ---------------------------------------------------------------------------
//  Gregory triangle patch (18 CVs)
// ---------------------------------------------------------------------------

/// Convert 15 Bezier triangle weights + 6 rational multipliers to 18
/// Gregory triangle weights.  Mirrors `Osd_convertBezierWeightsToGregory`.
fn convert_bezier_to_gregory(wb: &[f32; 15], rg: &[f32; 6], wg: &mut [f32; 18]) {
    wg[0]  = wb[0];
    wg[1]  = wb[1];
    wg[2]  = wb[5];
    wg[3]  = wb[6] * rg[0];
    wg[4]  = wb[6] * rg[1];

    wg[5]  = wb[4];
    wg[6]  = wb[8];
    wg[7]  = wb[3];
    wg[8]  = wb[7] * rg[2];
    wg[9]  = wb[7] * rg[3];

    wg[10] = wb[14];
    wg[11] = wb[12];
    wg[12] = wb[13];
    wg[13] = wb[10] * rg[4];
    wg[14] = wb[10] * rg[5];

    wg[15] = wb[2];
    wg[16] = wb[11];
    wg[17] = wb[9];
}

/// Evaluate Gregory triangle patch basis.  Returns 18.
pub fn eval_basis_gregory_tri(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 18]>,
    wds:  Option<&mut [f32; 18]>,
    wdt:  Option<&mut [f32; 18]>,
    wdss: Option<&mut [f32; 18]>,
    wdst: Option<&mut [f32; 18]>,
    wdtt: Option<&mut [f32; 18]>,
) -> i32 {
    let u = s;
    let v = t;
    let w = 1.0 - u - v;

    // Rational multipliers for the 3 pairs of interior points
    let mut g = [1.0_f32, 0.0, 1.0, 0.0, 1.0, 0.0];
    if u + v > 0.0 { g[0] = u / (u + v); g[1] = v / (u + v); }
    if v + w > 0.0 { g[2] = v / (v + w); g[3] = w / (v + w); }
    if w + u > 0.0 { g[4] = w / (w + u); g[5] = u / (w + u); }

    let mut bp = [0f32; 15];
    let mut bds = [0f32; 15];
    let mut bdt = [0f32; 15];
    let mut bdss = [0f32; 15];
    let mut bdst = [0f32; 15];
    let mut bdtt = [0f32; 15];

    if let Some(w) = wp {
        eval_bezier_tri_deriv_weights(s, t, 0, 0, &mut bp);
        convert_bezier_to_gregory(&bp, &g, w);
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        eval_bezier_tri_deriv_weights(s, t, 1, 0, &mut bds);
        eval_bezier_tri_deriv_weights(s, t, 0, 1, &mut bdt);
        convert_bezier_to_gregory(&bds, &g, ds);
        convert_bezier_to_gregory(&bdt, &g, dt);

        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            eval_bezier_tri_deriv_weights(s, t, 2, 0, &mut bdss);
            eval_bezier_tri_deriv_weights(s, t, 1, 1, &mut bdst);
            eval_bezier_tri_deriv_weights(s, t, 0, 2, &mut bdtt);
            convert_bezier_to_gregory(&bdss, &g, dss);
            convert_bezier_to_gregory(&bdst, &g, dst);
            convert_bezier_to_gregory(&bdtt, &g, dtt);
        }
    }
    18
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gregory_quad_returns_20() {
        let mut wp = [0f32; 20];
        let n = eval_basis_gregory(0.3, 0.4, Some(&mut wp), None, None, None, None, None);
        assert_eq!(n, 20);
    }

    #[test]
    fn gregory_quad_partition_of_unity() {
        let mut wp = [0f32; 20];
        eval_basis_gregory(0.3, 0.4, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "sum={}", wp.iter().sum::<f32>());
    }

    #[test]
    fn gregory_tri_returns_18() {
        let mut wp = [0f32; 18];
        let n = eval_basis_gregory_tri(0.3, 0.4, Some(&mut wp), None, None, None, None, None);
        assert_eq!(n, 18);
    }

    #[test]
    fn gregory_tri_partition_of_unity() {
        let mut wp = [0f32; 18];
        eval_basis_gregory_tri(0.3, 0.4, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "sum={}", wp.iter().sum::<f32>());
    }
}
