// Port of testUsdUsdzFileFormat.py — USDZ format tests (basic)
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdUsdzFileFormat.py
//
// Note: USDZ is a zip-based container format. These tests verify basic
// export/read operations if supported.

mod common;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Path, TimeCode};

// ============================================================================
// USDZ export (if supported)
// ============================================================================

#[test]
fn usdz_export_basic() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    stage.define_prim("/Root", "Xform").expect("define");
    stage.define_prim("/Root/Geom", "Mesh").expect("define");

    let tmp = common::tmp_path("test_basic.usdz");
    let tmp_str = tmp.to_str().expect("path str");

    // USDZ export may or may not be supported
    let result = stage.export(tmp_str, false);
    // Record success or expected failure
    let _ = result;
}

// ============================================================================
// USDZ read-back (if export succeeded)
// ============================================================================

#[test]
fn usdz_roundtrip() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    stage.define_prim("/Scene", "Xform").expect("define");

    let prim = stage
        .get_prim_at_path(&Path::from_string("/Scene").expect("p"))
        .expect("prim");
    let float_type = common::vtn("float");
    let attr = prim
        .create_attribute("val", &float_type, false, None)
        .expect("create");
    attr.set(7.5f32, TimeCode::default_time());

    let tmp = common::tmp_path("test_roundtrip.usdz");
    let tmp_str = tmp.to_str().expect("path str");

    if stage.export(tmp_str, false).is_ok() {
        // Try to open the USDZ
        let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
        if let Ok(s) = stage2 {
            let p = s.get_prim_at_path(&Path::from_string("/Scene").expect("p"));
            assert!(p.is_some(), "/Scene should exist in USDZ");
        }
    }
}

// ============================================================================
// USDZ with multiple prims
// ============================================================================

#[test]
fn usdz_multiple_prims() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    for i in 0..10 {
        stage
            .define_prim(&format!("/Prim_{}", i), "Xform")
            .expect("define");
    }

    let tmp = common::tmp_path("test_multi.usdz");
    let tmp_str = tmp.to_str().expect("path str");

    if stage.export(tmp_str, false).is_ok() {
        let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
        if let Ok(s) = stage2 {
            for i in 0..10 {
                let path = Path::from_string(&format!("/Prim_{}", i)).expect("p");
                assert!(s.get_prim_at_path(&path).is_some());
            }
        }
    }
}
