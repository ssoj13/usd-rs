/// Cubic B-Spline basis functions for quad patches.
///
/// Mirrors `Osd_EvalBasisBSpline`, `Osd_evalBSplineCurve`,
/// `Osd_adjustBSplineBoundaryWeights`, and `Osd_boundBasisBSpline` from
/// patchBasis.h.

/// Evaluate cubic B-Spline curve basis at parameter `t`.
///
/// `wP`   — position weights (always written)
/// `wDP`  — first derivative (written when Some)
/// `wDP2` — second derivative (written when Some)
#[inline]
pub fn eval_bspline_curve(
    t: f32,
    wp:   &mut [f32; 4],
    wdp:  Option<&mut [f32; 4]>,
    wdp2: Option<&mut [f32; 4]>,
) {
    let one6th = 1.0_f32 / 6.0;
    let t2 = t * t;
    let t3 = t * t2;

    wp[0] = one6th * (1.0 - 3.0 * (t - t2) - t3);
    wp[1] = one6th * (4.0 - 6.0 * t2 + 3.0 * t3);
    wp[2] = one6th * (1.0 + 3.0 * (t + t2 - t3));
    wp[3] = one6th * t3;

    if let Some(d) = wdp {
        d[0] = -0.5 * t2 + t - 0.5;
        d[1] =  1.5 * t2 - 2.0 * t;
        d[2] = -1.5 * t2 + t + 0.5;
        d[3] =  0.5 * t2;
    }
    if let Some(d) = wdp2 {
        d[0] = -t + 1.0;
        d[1] =  3.0 * t - 2.0;
        d[2] = -3.0 * t + 1.0;
        d[3] =  t;
    }
}

/// Adjust a 4x4 B-Spline weight array for boundary phantom points.
///
/// `boundary` is the 4-bit boundary mask from OsdPatchParam::get_boundary().
/// Mirrors `Osd_adjustBSplineBoundaryWeights`.
pub fn adjust_bspline_boundary_weights(boundary: i32, w: &mut [f32; 16]) {
    if (boundary & 1) != 0 {
        for i in 0..4 {
            w[i + 8] -= w[i];
            w[i + 4] += w[i] * 2.0;
            w[i]      = 0.0;
        }
    }
    if (boundary & 2) != 0 {
        for i in (0..16).step_by(4) {
            w[i + 1] -= w[i + 3];
            w[i + 2] += w[i + 3] * 2.0;
            w[i + 3]  = 0.0;
        }
    }
    if (boundary & 4) != 0 {
        for i in 0..4 {
            w[i + 4]  -= w[i + 12];
            w[i + 8]  += w[i + 12] * 2.0;
            w[i + 12]  = 0.0;
        }
    }
    if (boundary & 8) != 0 {
        for i in (0..16).step_by(4) {
            w[i + 2] -= w[i];
            w[i + 1] += w[i] * 2.0;
            w[i]      = 0.0;
        }
    }
}

/// Apply boundary adjustments to all weight arrays (position + derivatives).
pub fn bound_basis_bspline(
    boundary: i32,
    wp:   Option<&mut [f32; 16]>,
    wds:  Option<&mut [f32; 16]>,
    wdt:  Option<&mut [f32; 16]>,
    wdss: Option<&mut [f32; 16]>,
    wdst: Option<&mut [f32; 16]>,
    wdtt: Option<&mut [f32; 16]>,
) {
    if let Some(w) = wp {
        adjust_bspline_boundary_weights(boundary, w);
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        adjust_bspline_boundary_weights(boundary, ds);
        adjust_bspline_boundary_weights(boundary, dt);

        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            adjust_bspline_boundary_weights(boundary, dss);
            adjust_bspline_boundary_weights(boundary, dst);
            adjust_bspline_boundary_weights(boundary, dtt);
        }
    }
}

/// Evaluate cubic B-Spline patch (tensor product, 4x4 = 16 control points).
/// Returns 16.
pub fn eval_basis_bspline(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 16]>,
    wds:  Option<&mut [f32; 16]>,
    wdt:  Option<&mut [f32; 16]>,
    wdss: Option<&mut [f32; 16]>,
    wdst: Option<&mut [f32; 16]>,
    wdtt: Option<&mut [f32; 16]>,
) -> i32 {
    let mut sw = [0f32; 4]; // s position weights
    let mut tw = [0f32; 4]; // t position weights
    let mut dsw = [0f32; 4];
    let mut dtw = [0f32; 4];
    let mut dssw = [0f32; 4];
    let mut dttw = [0f32; 4];

    let need_d1  = wds.is_some() || wdt.is_some();
    let need_d2  = wdss.is_some() || wdst.is_some() || wdtt.is_some();

    eval_bspline_curve(s, &mut sw,
        if need_d1 { Some(&mut dsw)  } else { None },
        if need_d2 { Some(&mut dssw) } else { None });
    eval_bspline_curve(t, &mut tw,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bspline_curve_partition_of_unity() {
        let mut w = [0f32; 4];
        eval_bspline_curve(0.5, &mut w, None, None);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6,
            "sum={}", w.iter().sum::<f32>());
    }

    #[test]
    fn bspline_patch_weights_count() {
        let mut wp = [0f32; 16];
        let n = eval_basis_bspline(0.3, 0.7, Some(&mut wp), None, None, None, None, None);
        assert_eq!(n, 16);
    }

    #[test]
    fn bspline_patch_partition_of_unity() {
        let mut wp = [0f32; 16];
        eval_basis_bspline(0.4, 0.6, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "sum={}", wp.iter().sum::<f32>());
    }

    #[test]
    fn bspline_derivs_non_zero_at_centre() {
        let mut wp  = [0f32; 16];
        let mut wds = [0f32; 16];
        let mut wdt = [0f32; 16];
        eval_basis_bspline(0.5, 0.5,
            Some(&mut wp), Some(&mut wds), Some(&mut wdt),
            None, None, None);
        // derivatives should sum to zero (partition of unity property)
        assert!(wds.iter().sum::<f32>().abs() < 1e-5);
        assert!(wdt.iter().sum::<f32>().abs() < 1e-5);
    }
}
