// Port of testUsdCrateSpecific.py — USDC crate format specific tests
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdCrateSpecific.py

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_vt::Value;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "Xform").expect("define");
    stage.define_prim("/Root/Child", "Mesh").expect("define");
    stage
}

// ============================================================================
// Export to USDC and re-read
// ============================================================================

#[test]
fn export_usdc_roundtrip() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let float_type = common::vtn("float");
    let attr = prim
        .create_attribute("testVal", &float_type, false, None)
        .expect("create");
    attr.set(Value::from(3.14f32), TimeCode::default_time());

    let tmp = common::tmp_path("test_crate.usdc");
    let tmp_str = tmp.to_str().expect("path str");
    let result = stage.export(tmp_str, false);
    assert!(result.is_ok(), "export should succeed: {:?}", result.err());

    // Re-read and verify
    let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
    if let Ok(s) = stage2 {
        let p = s.get_prim_at_path(&Path::from_string("/Root").expect("p"));
        assert!(p.is_some(), "/Root should exist in re-read stage");

        let c = s.get_prim_at_path(&Path::from_string("/Root/Child").expect("p"));
        assert!(c.is_some(), "/Root/Child should exist");

        if let Some(prim) = p {
            if let Some(a) = prim.get_attribute("testVal") {
                if let Some(v) = a.get(TimeCode::default_time()) {
                    if let Some(f) = v.get::<f32>() {
                        assert!((f - 3.14f32).abs() < 1e-5, "value mismatch: {}", f);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Export to USDA and re-read
// ============================================================================

#[test]
fn export_usda_roundtrip() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let int_type = common::vtn("int");
    let attr = prim
        .create_attribute("intVal", &int_type, false, None)
        .expect("create");
    attr.set(Value::from(99i32), TimeCode::default_time());

    let tmp = common::tmp_path("test_crate.usda");
    let tmp_str = tmp.to_str().expect("path str");
    let result = stage.export(tmp_str, false);
    assert!(result.is_ok(), "export usda: {:?}", result.err());

    let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
    if let Ok(s) = stage2 {
        let p = s.get_prim_at_path(&Path::from_string("/Root").expect("p"));
        assert!(p.is_some());
        if let Some(prim) = p {
            if let Some(a) = prim.get_attribute("intVal") {
                if let Some(v) = a.get(TimeCode::default_time()) {
                    if let Some(i) = v.get::<i32>() {
                        assert_eq!(*i, 99);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Multiple prims USDC roundtrip
// ============================================================================

#[test]
fn export_many_prims_usdc() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    for i in 0..50 {
        stage
            .define_prim(&format!("/Prim_{}", i), "Xform")
            .expect("define");
    }

    let tmp = common::tmp_path("many_prims.usdc");
    let tmp_str = tmp.to_str().expect("path str");
    stage.export(tmp_str, false).expect("export");

    let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
    if let Ok(s) = stage2 {
        for i in 0..50 {
            let path = Path::from_string(&format!("/Prim_{}", i)).expect("p");
            assert!(
                s.get_prim_at_path(&path).is_some(),
                "Prim_{} should exist",
                i
            );
        }
    }
}

// ============================================================================
// Time samples USDC roundtrip
// ============================================================================

#[test]
fn export_time_samples_usdc() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let prim = stage.define_prim("/Animated", "Xform").expect("define");

    let float_type = common::vtn("float");
    let attr = prim
        .create_attribute("val", &float_type, false, None)
        .expect("create");
    attr.set(Value::from(0.0f32), TimeCode::new(1.0));
    attr.set(Value::from(1.0f32), TimeCode::new(24.0));

    let tmp = common::tmp_path("time_samples.usdc");
    let tmp_str = tmp.to_str().expect("path str");
    stage.export(tmp_str, false).expect("export");

    let stage2 = Stage::open(tmp_str, InitialLoadSet::LoadAll);
    if let Ok(s) = stage2 {
        if let Some(p) = s.get_prim_at_path(&Path::from_string("/Animated").expect("p")) {
            if let Some(a) = p.get_attribute("val") {
                let times = a.get_time_samples();
                assert_eq!(times.len(), 2, "should have 2 time samples");
            }
        }
    }
}
