//! Patch basis evaluation -- CPU host implementation.
//!
//! Mirrors the `patchBasis.h` / `patchBasisTypes.h` headers from OpenSubdiv
//! 3.7.0.  All functions operate on `f32` arrays and are pure (no allocation).

pub mod bezier;
pub mod box_spline_tri;
pub mod bspline;
pub mod gregory;
pub mod linear;
pub mod patch_param;

pub use bezier::{
    eval_basis_bezier, eval_basis_bezier_tri, eval_bezier_curve, eval_bezier_tri_deriv_weights,
};
pub use box_spline_tri::{
    adjust_box_spline_tri_boundary_weights, bound_basis_box_spline_tri, eval_basis_box_spline_tri,
    eval_box_spline_tri_deriv_weights,
};
pub use bspline::{
    adjust_bspline_boundary_weights, bound_basis_bspline, eval_basis_bspline, eval_bspline_curve,
};
pub use gregory::{eval_basis_gregory, eval_basis_gregory_tri};
pub use linear::{eval_basis_linear, eval_basis_linear_tri};
pub use patch_param::{OsdPatchParam, patch_type};

// ---------------------------------------------------------------------------
// Internal helpers: turn an Option<&mut [f32]> into a fixed-size array ref
// without consuming the Option (so the same buffer can be passed twice for
// the eval + boundary-fixup calls).  We reborrow via as_deref_mut() so only
// a temporary mutable borrow is taken.
// ---------------------------------------------------------------------------

#[inline]
fn reborrow16<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 16]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 16] = (&mut s[..16]).try_into().expect("weight buf <16");
        arr
    })
}
#[inline]
fn reborrow20<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 20]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 20] = (&mut s[..20]).try_into().expect("weight buf <20");
        arr
    })
}
#[inline]
fn reborrow18<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 18]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 18] = (&mut s[..18]).try_into().expect("weight buf <18");
        arr
    })
}
#[inline]
fn reborrow12<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 12]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 12] = (&mut s[..12]).try_into().expect("weight buf <12");
        arr
    })
}
#[inline]
fn reborrow4<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 4]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 4] = (&mut s[..4]).try_into().expect("weight buf <4");
        arr
    })
}
#[inline]
fn reborrow3<'a>(o: &'a mut Option<&mut [f32]>) -> Option<&'a mut [f32; 3]> {
    o.as_deref_mut().map(|s: &mut [f32]| {
        let arr: &mut [f32; 3] = (&mut s[..3]).try_into().expect("weight buf <3");
        arr
    })
}

// Helper to cast a mutable slice into a fixed-size array ref.
#[inline]
fn slice_to_arr16(s: &mut [f32]) -> &mut [f32; 16] {
    (&mut s[..16])
        .try_into()
        .expect("wp too small for 16 weights")
}
#[inline]
fn slice_to_arr20(s: &mut [f32]) -> &mut [f32; 20] {
    (&mut s[..20])
        .try_into()
        .expect("wp too small for 20 weights")
}
#[inline]
fn slice_to_arr18(s: &mut [f32]) -> &mut [f32; 18] {
    (&mut s[..18])
        .try_into()
        .expect("wp too small for 18 weights")
}
#[inline]
fn slice_to_arr12(s: &mut [f32]) -> &mut [f32; 12] {
    (&mut s[..12])
        .try_into()
        .expect("wp too small for 12 weights")
}
#[inline]
fn slice_to_arr4(s: &mut [f32]) -> &mut [f32; 4] {
    (&mut s[..4])
        .try_into()
        .expect("wp too small for 4 weights")
}
#[inline]
fn slice_to_arr3(s: &mut [f32]) -> &mut [f32; 3] {
    (&mut s[..3])
        .try_into()
        .expect("wp too small for 3 weights")
}

/// Evaluate patch basis weights for a given patch type, with (s,t) already
/// normalised to patch-local [0,1] coordinates.
///
/// Mirrors `OsdEvaluatePatchBasisNormalized` from patchBasis.h.
/// Returns the number of control points for the patch type.
pub fn evaluate_patch_basis_normalized(
    patch_type_id: i32,
    param: &OsdPatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
    mut wds: Option<&mut [f32]>,
    mut wdt: Option<&mut [f32]>,
    mut wdss: Option<&mut [f32]>,
    mut wdst: Option<&mut [f32]>,
    mut wdtt: Option<&mut [f32]>,
) -> i32 {
    let boundary = param.get_boundary();

    if patch_type_id == patch_type::REGULAR {
        let npts = eval_basis_bspline(
            s,
            t,
            Some(slice_to_arr16(wp)),
            reborrow16(&mut wds),
            reborrow16(&mut wdt),
            reborrow16(&mut wdss),
            reborrow16(&mut wdst),
            reborrow16(&mut wdtt),
        );
        if boundary != 0 {
            bound_basis_bspline(
                boundary,
                Some(slice_to_arr16(wp)),
                reborrow16(&mut wds),
                reborrow16(&mut wdt),
                reborrow16(&mut wdss),
                reborrow16(&mut wdst),
                reborrow16(&mut wdtt),
            );
        }
        npts
    } else if patch_type_id == patch_type::LOOP {
        let npts = eval_basis_box_spline_tri(
            s,
            t,
            Some(slice_to_arr12(wp)),
            reborrow12(&mut wds),
            reborrow12(&mut wdt),
            reborrow12(&mut wdss),
            reborrow12(&mut wdst),
            reborrow12(&mut wdtt),
        );
        if boundary != 0 {
            bound_basis_box_spline_tri(
                boundary,
                Some(slice_to_arr12(wp)),
                reborrow12(&mut wds),
                reborrow12(&mut wdt),
                reborrow12(&mut wdss),
                reborrow12(&mut wdst),
                reborrow12(&mut wdtt),
            );
        }
        npts
    } else if patch_type_id == patch_type::GREGORY_BASIS {
        eval_basis_gregory(
            s,
            t,
            Some(slice_to_arr20(wp)),
            reborrow20(&mut wds),
            reborrow20(&mut wdt),
            reborrow20(&mut wdss),
            reborrow20(&mut wdst),
            reborrow20(&mut wdtt),
        )
    } else if patch_type_id == patch_type::GREGORY_TRIANGLE {
        eval_basis_gregory_tri(
            s,
            t,
            Some(slice_to_arr18(wp)),
            reborrow18(&mut wds),
            reborrow18(&mut wdt),
            reborrow18(&mut wdss),
            reborrow18(&mut wdst),
            reborrow18(&mut wdtt),
        )
    } else if patch_type_id == patch_type::QUADS {
        eval_basis_linear(
            s,
            t,
            Some(slice_to_arr4(wp)),
            reborrow4(&mut wds),
            reborrow4(&mut wdt),
            reborrow4(&mut wdss),
            reborrow4(&mut wdst),
            reborrow4(&mut wdtt),
        )
    } else if patch_type_id == patch_type::TRIANGLES {
        eval_basis_linear_tri(
            s,
            t,
            Some(slice_to_arr3(wp)),
            reborrow3(&mut wds),
            reborrow3(&mut wdt),
            reborrow3(&mut wdss),
            reborrow3(&mut wdst),
            reborrow3(&mut wdtt),
        )
    } else {
        0
    }
}

/// Evaluate patch basis weights including full param normalisation.
///
/// Mirrors `OsdEvaluatePatchBasis` from patchBasis.h.
/// Returns the number of control points.
pub fn evaluate_patch_basis(
    patch_type_id: i32,
    param: &OsdPatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
    mut wds: Option<&mut [f32]>,
    mut wdt: Option<&mut [f32]>,
    mut wdss: Option<&mut [f32]>,
    mut wdst: Option<&mut [f32]>,
    mut wdtt: Option<&mut [f32]>,
) -> i32 {
    let is_tri = patch_type_id == patch_type::LOOP
        || patch_type_id == patch_type::GREGORY_TRIANGLE
        || patch_type_id == patch_type::TRIANGLES;

    let mut deriv_sign = 1.0_f32;
    let (ns, nt) = if is_tri {
        let (ns, nt) = param.normalize_triangle(s, t);
        if param.is_triangle_rotated() {
            deriv_sign = -1.0;
        }
        (ns, nt)
    } else {
        param.normalize(s, t)
    };

    let npts = evaluate_patch_basis_normalized(
        patch_type_id,
        param,
        ns,
        nt,
        wp,
        wds.as_deref_mut(),
        wdt.as_deref_mut(),
        wdss.as_deref_mut(),
        wdst.as_deref_mut(),
        wdtt.as_deref_mut(),
    );

    // Scale first-order derivatives by depth factor.
    if wds.is_some() && wdt.is_some() {
        let d1_scale = deriv_sign * (1 << param.get_depth()) as f32;
        if let Some(ds) = wds.as_deref_mut() {
            for v in ds[..npts as usize].iter_mut() {
                *v *= d1_scale;
            }
        }
        if let Some(dt) = wdt.as_deref_mut() {
            for v in dt[..npts as usize].iter_mut() {
                *v *= d1_scale;
            }
        }
        // Scale second-order derivatives.
        if wdss.is_some() && wdst.is_some() && wdtt.is_some() {
            let d2_scale = deriv_sign * d1_scale * d1_scale;
            if let Some(dss) = wdss {
                for v in dss[..npts as usize].iter_mut() {
                    *v *= d2_scale;
                }
            }
            if let Some(dst) = wdst {
                for v in dst[..npts as usize].iter_mut() {
                    *v *= d2_scale;
                }
            }
            if let Some(dtt) = wdtt {
                for v in dtt[..npts as usize].iter_mut() {
                    *v *= d2_scale;
                }
            }
        }
    }
    npts
}

/// Evaluate basis, position only.
pub fn osd_evaluate_patch_basis(
    patch_type_id: i32,
    param: &OsdPatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
) -> i32 {
    evaluate_patch_basis(patch_type_id, param, s, t, wp, None, None, None, None, None)
}

/// Evaluate basis with first derivatives.
pub fn osd_evaluate_patch_basis_d1(
    patch_type_id: i32,
    param: &OsdPatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
    wds: &mut [f32],
    wdt: &mut [f32],
) -> i32 {
    evaluate_patch_basis(
        patch_type_id,
        param,
        s,
        t,
        wp,
        Some(wds),
        Some(wdt),
        None,
        None,
        None,
    )
}

/// Evaluate basis with first and second derivatives.
pub fn osd_evaluate_patch_basis_d2(
    patch_type_id: i32,
    param: &OsdPatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
    wds: &mut [f32],
    wdt: &mut [f32],
    wdss: &mut [f32],
    wdst: &mut [f32],
    wdtt: &mut [f32],
) -> i32 {
    evaluate_patch_basis(
        patch_type_id,
        param,
        s,
        t,
        wp,
        Some(wds),
        Some(wdt),
        Some(wdss),
        Some(wdst),
        Some(wdtt),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_param() -> OsdPatchParam {
        // depth=1, is_regular=true
        OsdPatchParam::new(0, (1 << 5) | 1, 0.0)
    }

    #[test]
    fn dispatch_regular_returns_16() {
        let param = simple_param();
        let mut wp = vec![0f32; 20];
        let n = evaluate_patch_basis_normalized(
            patch_type::REGULAR,
            &param,
            0.5,
            0.5,
            &mut wp,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 16);
    }

    #[test]
    fn dispatch_gregory_returns_20() {
        let param = simple_param();
        let mut wp = vec![0f32; 20];
        let n = evaluate_patch_basis_normalized(
            patch_type::GREGORY_BASIS,
            &param,
            0.3,
            0.4,
            &mut wp,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 20);
    }

    #[test]
    fn dispatch_loop_returns_12() {
        let param = simple_param();
        let mut wp = vec![0f32; 20];
        let n = evaluate_patch_basis_normalized(
            patch_type::LOOP,
            &param,
            0.2,
            0.3,
            &mut wp,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 12);
    }

    #[test]
    fn dispatch_quads_returns_4() {
        let param = simple_param();
        let mut wp = vec![0f32; 20];
        let n = evaluate_patch_basis_normalized(
            patch_type::QUADS,
            &param,
            0.5,
            0.5,
            &mut wp,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 4);
    }

    #[test]
    fn full_basis_regular_partition_of_unity() {
        let param = simple_param();
        let mut wp = vec![0f32; 20];
        let n = evaluate_patch_basis(
            patch_type::REGULAR,
            &param,
            0.25,
            0.25,
            &mut wp,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 16);
        let sum: f32 = wp[..16].iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "partition of unity failed: sum={}",
            sum
        );
    }
}
