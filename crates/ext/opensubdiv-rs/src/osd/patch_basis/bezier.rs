/// Cubic Bezier basis functions — quad and triangle variants.
///
/// Mirrors `Osd_evalBezierCurve`, `Osd_EvalBasisBezier`, and
/// `Osd_EvalBasisBezierTri` from patchBasis.h.

// ---------------------------------------------------------------------------
//  Quad Bezier (16 CVs)
// ---------------------------------------------------------------------------

/// Evaluate cubic Bezier curve basis at `t`.
#[inline]
pub fn eval_bezier_curve(
    t: f32,
    wp:   Option<&mut [f32; 4]>,
    wdp:  Option<&mut [f32; 4]>,
    wdp2: Option<&mut [f32; 4]>,
) {
    let t2 = t * t;
    let tc = 1.0 - t;
    let tc2 = tc * tc;

    if let Some(w) = wp {
        w[0] = tc2 * tc;
        w[1] = tc2 * t * 3.0;
        w[2] = t2 * tc * 3.0;
        w[3] = t2 * t;
    }
    if let Some(d) = wdp {
        d[0] = -3.0 * tc2;
        d[1] =  9.0 * t2 - 12.0 * t + 3.0;
        d[2] = -9.0 * t2 +  6.0 * t;
        d[3] =  3.0 * t2;
    }
    if let Some(d) = wdp2 {
        d[0] =   6.0 * tc;
        d[1] =  18.0 * t - 12.0;
        d[2] = -18.0 * t +  6.0;
        d[3] =   6.0 * t;
    }
}

/// Evaluate Bezier patch (tensor product, 4×4 = 16 CVs).  Returns 16.
pub fn eval_basis_bezier(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 16]>,
    wds:  Option<&mut [f32; 16]>,
    wdt:  Option<&mut [f32; 16]>,
    wdss: Option<&mut [f32; 16]>,
    wdst: Option<&mut [f32; 16]>,
    wdtt: Option<&mut [f32; 16]>,
) -> i32 {
    let need_p  = wp.is_some();
    let need_d1 = wds.is_some() || wdt.is_some();
    let need_d2 = wdss.is_some() || wdst.is_some() || wdtt.is_some();

    let mut sw = [0f32; 4];
    let mut tw = [0f32; 4];
    let mut dsw = [0f32; 4];
    let mut dtw = [0f32; 4];
    let mut dssw = [0f32; 4];
    let mut dttw = [0f32; 4];

    eval_bezier_curve(s,
        if need_p  { Some(&mut sw)   } else { None },
        if need_d1 { Some(&mut dsw)  } else { None },
        if need_d2 { Some(&mut dssw) } else { None });
    eval_bezier_curve(t,
        if need_p  { Some(&mut tw)   } else { None },
        if need_d1 { Some(&mut dtw)  } else { None },
        if need_d2 { Some(&mut dttw) } else { None });

    if let Some(w) = wp {
        for i in 0..4 {
            for j in 0..4 {
                w[4 * i + j] = sw[j] * tw[i];
            }
        }
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        for i in 0..4 {
            for j in 0..4 {
                ds[4 * i + j] = dsw[j] * tw[i];
                dt[4 * i + j] =  sw[j] * dtw[i];
            }
        }
        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            for i in 0..4 {
                for j in 0..4 {
                    dss[4 * i + j] = dssw[j] * tw[i];
                    dst[4 * i + j] =  dsw[j] * dtw[i];
                    dtt[4 * i + j] =   sw[j] * dttw[i];
                }
            }
        }
    }
    16
}

// ---------------------------------------------------------------------------
//  Triangle Bezier (15 CVs — quartic triangular Bezier)
// ---------------------------------------------------------------------------

/// Compute quartic triangular Bezier basis weights and derivatives.
///
/// `ds` / `dt` control which order partial derivative to compute
/// (0,0 = position, 1,0 = ds, 0,1 = dt, 2,0 = dss, 1,1 = dst, 0,2 = dtt).
pub fn eval_bezier_tri_deriv_weights(s: f32, t: f32, ds: i32, dt: i32, wb: &mut [f32; 15]) {
    let u  = s;
    let v  = t;
    let w  = 1.0 - u - v;

    let uu = u * u;
    let vv = v * v;
    let ww = w * w;

    let uv = u * v;
    let vw = v * w;
    let uw = u * w;

    let total = ds + dt;
    match total {
        0 => {
            wb[0]  = ww * ww;
            wb[1]  = 4.0 * uw * ww;
            wb[2]  = 6.0 * uw * uw;
            wb[3]  = 4.0 * uw * uu;
            wb[4]  = uu * uu;
            wb[5]  = 4.0 * vw * ww;
            wb[6]  = 12.0 * ww * uv;
            wb[7]  = 12.0 * uu * vw;
            wb[8]  = 4.0 * uv * uu;
            wb[9]  = 6.0 * vw * vw;
            wb[10] = 12.0 * vv * uw;
            wb[11] = 6.0 * uv * uv;
            wb[12] = 4.0 * vw * vv;
            wb[13] = 4.0 * uv * vv;
            wb[14] = vv * vv;
        }
        1 if ds == 1 => {
            wb[0]  = -4.0 * ww * w;
            wb[1]  =  4.0 * ww * (w - 3.0 * u);
            wb[2]  = 12.0 * uw * (w - u);
            wb[3]  =  4.0 * uu * (3.0 * w - u);
            wb[4]  =  4.0 * uu * u;
            wb[5]  = -12.0 * vw * w;
            wb[6]  =  12.0 * vw * (w - 2.0 * u);
            wb[7]  =  12.0 * uv * (2.0 * w - u);
            wb[8]  =  12.0 * uv * u;
            wb[9]  = -12.0 * vv * w;
            wb[10] =  12.0 * vv * (w - u);
            wb[11] =  12.0 * vv * u;
            wb[12] =  -4.0 * vv * v;
            wb[13] =   4.0 * vv * v;
            wb[14] =   0.0;
        }
        1 => { // dt == 1
            wb[0]  = -4.0 * ww * w;
            wb[1]  = -12.0 * ww * u;
            wb[2]  = -12.0 * uu * w;
            wb[3]  =  -4.0 * uu * u;
            wb[4]  =   0.0;
            wb[5]  =  4.0 * ww * (w - 3.0 * v);
            wb[6]  = 12.0 * uw * (w - 2.0 * v);
            wb[7]  = 12.0 * uu * (w - v);
            wb[8]  =  4.0 * uu * u;
            wb[9]  = 12.0 * vw * (w - v);
            wb[10] = 12.0 * uv * (2.0 * w - v);
            wb[11] = 12.0 * uv * u;
            wb[12] =  4.0 * vv * (3.0 * w - v);
            wb[13] = 12.0 * vv * u;
            wb[14] =  4.0 * vv * v;
        }
        2 if ds == 2 => {
            wb[0]  =  12.0 * ww;
            wb[1]  =  24.0 * (uw - ww);
            wb[2]  =  12.0 * (uu - 4.0 * uw + ww);
            wb[3]  =  24.0 * (uw - uu);
            wb[4]  =  12.0 * uu;
            wb[5]  =  24.0 * vw;
            wb[6]  =  24.0 * (uv - 2.0 * vw);
            wb[7]  =  24.0 * (vw - 2.0 * uv);
            wb[8]  =  24.0 * uv;
            wb[9]  =  12.0 * vv;
            wb[10] = -24.0 * vv;
            wb[11] =  12.0 * vv;
            wb[12] =   0.0;
            wb[13] =   0.0;
            wb[14] =   0.0;
        }
        2 if dt == 2 => {
            wb[0]  =  12.0 * ww;
            wb[1]  =  24.0 * uw;
            wb[2]  =  12.0 * uu;
            wb[3]  =   0.0;
            wb[4]  =   0.0;
            wb[5]  =  24.0 * (vw - ww);
            wb[6]  =  24.0 * (uv - 2.0 * uw);
            wb[7]  = -24.0 * uu;
            wb[8]  =   0.0;
            wb[9]  =  12.0 * (vv - 4.0 * vw + ww);
            wb[10] =  24.0 * (uw - 2.0 * uv);
            wb[11] =  12.0 * uu;
            wb[12] =  24.0 * (vw - vv);
            wb[13] =  24.0 * uv;
            wb[14] =  12.0 * vv;
        }
        2 => { // ds == 1, dt == 1
            wb[0]  =  12.0 * ww;
            wb[3]  = -12.0 * uu;
            wb[13] =  12.0 * vv;
            wb[11] =  24.0 * uv;
            wb[1]  =  24.0 * uw - wb[0];
            wb[2]  = -24.0 * uw - wb[3];
            wb[5]  =  24.0 * vw - wb[0];
            wb[6]  = -24.0 * vw + wb[11] - wb[1];
            wb[8]  = -wb[3];
            wb[7]  = -(wb[11] + wb[2]);
            wb[9]  =   wb[13] - wb[5] - wb[0];
            wb[10] = -(wb[9] + wb[11]);
            wb[12] = -wb[13];
            wb[4]  =   0.0;
            wb[14] =   0.0;
        }
        _ => {
            // higher order not supported
        }
    }
}

/// Evaluate quartic triangular Bezier basis.  Returns 15.
pub fn eval_basis_bezier_tri(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 15]>,
    wds:  Option<&mut [f32; 15]>,
    wdt:  Option<&mut [f32; 15]>,
    wdss: Option<&mut [f32; 15]>,
    wdst: Option<&mut [f32; 15]>,
    wdtt: Option<&mut [f32; 15]>,
) -> i32 {
    if let Some(w) = wp {
        eval_bezier_tri_deriv_weights(s, t, 0, 0, w);
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        eval_bezier_tri_deriv_weights(s, t, 1, 0, ds);
        eval_bezier_tri_deriv_weights(s, t, 0, 1, dt);

        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            eval_bezier_tri_deriv_weights(s, t, 2, 0, dss);
            eval_bezier_tri_deriv_weights(s, t, 1, 1, dst);
            eval_bezier_tri_deriv_weights(s, t, 0, 2, dtt);
        }
    }
    15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_curve_at_zero() {
        let mut w = [0f32; 4];
        eval_bezier_curve(0.0, Some(&mut w), None, None);
        assert!((w[0] - 1.0).abs() < 1e-6);
        assert!(w[1].abs() < 1e-6);
    }

    #[test]
    fn bezier_curve_at_one() {
        let mut w = [0f32; 4];
        eval_bezier_curve(1.0, Some(&mut w), None, None);
        assert!(w[0].abs() < 1e-6);
        assert!((w[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bezier_patch_partition_of_unity() {
        let mut wp = [0f32; 16];
        eval_basis_bezier(0.4, 0.6, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bezier_tri_partition_of_unity() {
        let mut wp = [0f32; 15];
        eval_basis_bezier_tri(0.3, 0.4, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "sum={}", wp.iter().sum::<f32>());
    }
}
