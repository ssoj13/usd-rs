//! Patch basis evaluation for the Far layer.
//!
//! The heavy lifting is done in `crate::osd::patch_basis`.  This module
//! provides `far::` namespace wrappers that call through to those functions.
//!
//! Mirrors C++ `Far::EvaluatePatchBasis` / `Far::EvaluatePatchBasisNormalized`.

// Re-export the OSD primitives under the far:: namespace
pub use crate::osd::patch_basis::{
    evaluate_patch_basis, evaluate_patch_basis_normalized, osd_evaluate_patch_basis,
    osd_evaluate_patch_basis_d1, osd_evaluate_patch_basis_d2,
};

pub use crate::osd::patch_basis::patch_param::{OsdPatchParam, patch_type};

// ---------------------------------------------------------------------------
// Far-layer typed wrappers that use Far::PatchDescriptor::PatchType
// ---------------------------------------------------------------------------

use crate::far::patch_descriptor::PatchType;
use crate::far::patch_param::PatchParam;

/// Evaluate patch basis weights for a given Far patch type.
///
/// Maps `far::PatchType` to the OSD integer code and calls `evaluate_patch_basis`.
pub fn evaluate_far_patch_basis(
    patch_type: PatchType,
    patch_param: PatchParam,
    s: f32,
    t: f32,
    wp: &mut [f32],
    wds: Option<&mut [f32]>,
    wdt: Option<&mut [f32]>,
    wdss: Option<&mut [f32]>,
    wdst: Option<&mut [f32]>,
    wdtt: Option<&mut [f32]>,
) -> i32 {
    let type_id = far_patch_type_to_osd_code(patch_type);
    let osd_pp = OsdPatchParam {
        field0: patch_param.field0,
        field1: patch_param.field1,
        sharpness: 0.0,
    };
    evaluate_patch_basis(type_id, &osd_pp, s, t, wp, wds, wdt, wdss, wdst, wdtt)
}

/// Convert a `far::PatchType` to the OSD integer patch-type code.
///
/// Maps `Far::PatchDescriptor::Type` enum values to the integer codes used
/// by `EvaluatePatchBasis`. Values match the C++ enum exactly:
///   QUADS=3, TRIANGLES=4, LOOP=5, REGULAR=6, GREGORY=7,
///   GREGORY_BOUNDARY=8, GREGORY_BASIS=9, GREGORY_TRIANGLE=10.
pub fn far_patch_type_to_osd_code(pt: PatchType) -> i32 {
    match pt {
        PatchType::Regular => patch_type::REGULAR,
        PatchType::Gregory => patch_type::GREGORY,
        PatchType::GregoryBoundary => patch_type::GREGORY_BOUNDARY,
        PatchType::GregoryBasis => patch_type::GREGORY_BASIS,
        PatchType::GregoryTriangle => patch_type::GREGORY_TRIANGLE,
        PatchType::Loop => patch_type::LOOP,
        PatchType::Quads => patch_type::QUADS,
        PatchType::Triangles => patch_type::TRIANGLES,
        // NonPatch, Points, Lines have no basis evaluation
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::far::patch_descriptor::PatchType;
    use crate::far::patch_param::PatchParam;

    #[test]
    fn regular_patch_weight_sum() {
        let pp = PatchParam::default();
        let mut w = vec![0.0f32; 16];
        let n = evaluate_far_patch_basis(
            PatchType::Regular,
            pp,
            0.5,
            0.5,
            &mut w,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 16);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 5e-4, "weight sum = {sum}");
    }

    #[test]
    fn linear_patch_weight_sum() {
        let pp = PatchParam::default();
        let mut w = vec![0.0f32; 4];
        let n = evaluate_far_patch_basis(
            PatchType::Quads,
            pp,
            0.5,
            0.5,
            &mut w,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(n, 4);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "weight sum = {sum}");
    }
}
