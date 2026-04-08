//! Feature Adaptive Representation -- public subdivision API.
//!
//! Core types: TopologyRefiner, PatchTable, StencilTable, PrimvarRefiner.

pub mod bilinear_patch_builder;
pub mod catmark_patch_builder;
pub mod error;
pub mod loop_patch_builder;
pub mod patch_basis;
pub mod patch_builder;
pub mod patch_descriptor;
pub mod patch_map;
pub mod patch_param;
pub mod patch_table;
pub mod patch_table_factory;
pub mod primvar_refiner;
pub mod ptex_indices;
pub mod sparse_matrix;
pub mod stencil_builder;
pub mod stencil_table;
pub mod stencil_table_factory;
pub mod topology_descriptor;
pub mod topology_level;
pub mod topology_refiner;
pub mod topology_refiner_factory;
pub mod types;

pub use error::{ErrorType, far_error, far_warning, set_error_callback, set_warning_callback};
pub use patch_descriptor::{PatchDescriptor, PatchType};
pub use patch_map::PatchMap;
pub use patch_param::PatchParam;
pub use patch_table::{PatchHandle, PatchTable};
pub use patch_table_factory::{EndCapType, Options as PatchTableFactoryOptions, PatchTableFactory};
pub use ptex_indices::PtexIndices;
pub use stencil_table::{LimitStencilTable, StencilTable};
pub use topology_descriptor::{FVarChannel, TopologyDescriptor};
pub use topology_level::TopologyLevel;
pub use topology_refiner::{AdaptiveOptions, TopologyRefiner, UniformOptions};
pub use topology_refiner_factory::{
    FactoryOptions, TopologyDescriptorFactory, TopologyRefinerFactory,
};
pub use types::*;

// ---------------------------------------------------------------------------
// Patch basis evaluation helpers — thin wrappers that accept Far types.
// ---------------------------------------------------------------------------

use crate::osd::patch_basis::{OsdPatchParam, evaluate_patch_basis as osd_eval_f32};

/// Evaluate patch basis weights (f32).  Accepts `PatchType` and `PatchParam`
/// from the Far namespace, converting them to OSD types internally.
pub fn evaluate_patch_basis(
    ptype: PatchType,
    param: PatchParam,
    s: f32,
    t: f32,
    wp: Option<&mut [f32]>,
    wds: Option<&mut [f32]>,
    wdt: Option<&mut [f32]>,
    wdss: Option<&mut [f32]>,
    wdst: Option<&mut [f32]>,
    wdtt: Option<&mut [f32]>,
) -> i32 {
    let osd_param = OsdPatchParam::new(param.field0, param.field1, 0.0);
    let patch_type_id = ptype as i32;
    let mut dummy_wp = [0.0f32; 20];
    let wp_slice = wp.unwrap_or(&mut dummy_wp);
    osd_eval_f32(
        patch_type_id,
        &osd_param,
        s,
        t,
        wp_slice,
        wds,
        wdt,
        wdss,
        wdst,
        wdtt,
    )
}

/// Evaluate patch basis weights (f64).  Internally casts to f32 and back since
/// the basis functions are precision-agnostic polynomials.
pub fn evaluate_patch_basis_f64(
    ptype: PatchType,
    param: PatchParam,
    s: f64,
    t: f64,
    mut wp: Option<&mut [f64]>,
    mut wds: Option<&mut [f64]>,
    mut wdt: Option<&mut [f64]>,
    mut wdss: Option<&mut [f64]>,
    mut wdst: Option<&mut [f64]>,
    mut wdtt: Option<&mut [f64]>,
) -> i32 {
    let osd_param = OsdPatchParam::new(param.field0, param.field1, 0.0);
    let patch_type_id = ptype as i32;

    // Maximum control point count across all patch types (GregoryBasis = 20).
    const MAX_PTS: usize = 20;

    // Allocate scratch buffers for all optional derivatives.
    let mut wp32 = [0.0f32; MAX_PTS];
    let mut wds32 = [0.0f32; MAX_PTS];
    let mut wdt32 = [0.0f32; MAX_PTS];
    let mut wdss32 = [0.0f32; MAX_PTS];
    let mut wdst32 = [0.0f32; MAX_PTS];
    let mut wdtt32 = [0.0f32; MAX_PTS];

    let npts = osd_eval_f32(
        patch_type_id,
        &osd_param,
        s as f32,
        t as f32,
        &mut wp32,
        if wds.is_some() {
            Some(&mut wds32)
        } else {
            None
        },
        if wdt.is_some() {
            Some(&mut wdt32)
        } else {
            None
        },
        if wdss.is_some() {
            Some(&mut wdss32)
        } else {
            None
        },
        if wdst.is_some() {
            Some(&mut wdst32)
        } else {
            None
        },
        if wdtt.is_some() {
            Some(&mut wdtt32)
        } else {
            None
        },
    ) as usize;

    // Write back results into the caller's f64 slices.
    if let Some(ref mut out) = wp {
        for (a, &b) in out[..npts].iter_mut().zip(wp32[..npts].iter()) {
            *a = b as f64;
        }
    }
    if let Some(ref mut out) = wds {
        for (a, &b) in out[..npts].iter_mut().zip(wds32[..npts].iter()) {
            *a = b as f64;
        }
    }
    if let Some(ref mut out) = wdt {
        for (a, &b) in out[..npts].iter_mut().zip(wdt32[..npts].iter()) {
            *a = b as f64;
        }
    }
    if let Some(ref mut out) = wdss {
        for (a, &b) in out[..npts].iter_mut().zip(wdss32[..npts].iter()) {
            *a = b as f64;
        }
    }
    if let Some(ref mut out) = wdst {
        for (a, &b) in out[..npts].iter_mut().zip(wdst32[..npts].iter()) {
            *a = b as f64;
        }
    }
    if let Some(ref mut out) = wdtt {
        for (a, &b) in out[..npts].iter_mut().zip(wdtt32[..npts].iter()) {
            *a = b as f64;
        }
    }

    npts as i32
}
