/// Bilinear quad and linear triangle basis functions.
///
/// Mirrors `Osd_EvalBasisLinear` and `Osd_EvalBasisLinearTri` from patchBasis.h.

/// Evaluate bilinear (quad) basis.  Returns 4 (the number of control points).
///
/// `wP`  — position weights (always written)
/// `wDs` / `wDt`  — first derivative weights (written when both are Some)
/// `wDss` / `wDst` / `wDtt` — second derivative weights (written when all are Some)
pub fn eval_basis_linear(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 4]>,
    wds:  Option<&mut [f32; 4]>,
    wdt:  Option<&mut [f32; 4]>,
    wdss: Option<&mut [f32; 4]>,
    wdst: Option<&mut [f32; 4]>,
    wdtt: Option<&mut [f32; 4]>,
) -> i32 {
    let sc = 1.0 - s;
    let tc = 1.0 - t;

    if let Some(w) = wp {
        w[0] = sc * tc;
        w[1] =  s * tc;
        w[2] =  s * t;
        w[3] = sc * t;
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        ds[0] = -tc;
        ds[1] =  tc;
        ds[2] =   t;
        ds[3] =  -t;

        dt[0] = -sc;
        dt[1] =  -s;
        dt[2] =   s;
        dt[3] =  sc;

        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            for i in 0..4 {
                dss[i] = 0.0;
                dtt[i] = 0.0;
            }
            dst[0] =  1.0;
            dst[1] = -1.0;
            dst[2] =  1.0;
            dst[3] = -1.0;
        }
    }
    4
}

/// Evaluate linear triangle basis.  Returns 3.
pub fn eval_basis_linear_tri(
    s: f32, t: f32,
    wp:   Option<&mut [f32; 3]>,
    wds:  Option<&mut [f32; 3]>,
    wdt:  Option<&mut [f32; 3]>,
    wdss: Option<&mut [f32; 3]>,
    wdst: Option<&mut [f32; 3]>,
    wdtt: Option<&mut [f32; 3]>,
) -> i32 {
    if let Some(w) = wp {
        w[0] = 1.0 - s - t;
        w[1] = s;
        w[2] = t;
    }
    if let (Some(ds), Some(dt)) = (wds, wdt) {
        ds[0] = -1.0;
        ds[1] =  1.0;
        ds[2] =  0.0;

        dt[0] = -1.0;
        dt[1] =  0.0;
        dt[2] =  1.0;

        if let (Some(dss), Some(dst), Some(dtt)) = (wdss, wdst, wdtt) {
            dss[0] = 0.0; dss[1] = 0.0; dss[2] = 0.0;
            dst[0] = 0.0; dst[1] = 0.0; dst[2] = 0.0;
            dtt[0] = 0.0; dtt[1] = 0.0; dtt[2] = 0.0;
        }
    }
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_corners() {
        // At (0,0) only corner 0 should have weight 1
        let mut wp = [0f32; 4];
        eval_basis_linear(0.0, 0.0, Some(&mut wp), None, None, None, None, None);
        assert!((wp[0] - 1.0).abs() < 1e-6);
        assert!(wp[1].abs() < 1e-6);
        assert!(wp[2].abs() < 1e-6);
        assert!(wp[3].abs() < 1e-6);
    }

    #[test]
    fn bilinear_centre() {
        let mut wp = [0f32; 4];
        eval_basis_linear(0.5, 0.5, Some(&mut wp), None, None, None, None, None);
        for w in &wp {
            assert!((w - 0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn bilinear_weights_sum_to_one() {
        let mut wp = [0f32; 4];
        eval_basis_linear(0.3, 0.7, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn linear_tri_partition_of_unity() {
        let mut wp = [0f32; 3];
        eval_basis_linear_tri(0.2, 0.3, Some(&mut wp), None, None, None, None, None);
        assert!((wp.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }
}
