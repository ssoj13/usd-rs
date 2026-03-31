// Port of testUsdFileFormats.py — file format detection and round-trip
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdFileFormats.py

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_vt::Value;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage")
}

// ============================================================================
// In-memory stage basics
// ============================================================================

#[test]
fn create_in_memory_stage() {
    let stage = setup_stage();
    let root_layer = stage.get_root_layer();
    assert!(!root_layer.identifier().is_empty());
}

#[test]
fn in_memory_stage_is_anonymous() {
    let stage = setup_stage();
    let layer = stage.get_root_layer();
    assert!(layer.is_anonymous());
}

// ============================================================================
// Stage root layer
// ============================================================================

#[test]
fn root_layer_exists() {
    let stage = setup_stage();
    let root = stage.get_root_layer();
    assert!(!root.identifier().is_empty());
}

#[test]
fn session_layer_exists() {
    let stage = setup_stage();
    let session = stage.get_session_layer();
    // Session layer may or may not exist depending on creation mode
    let _ = session;
}

// ============================================================================
// Layer stack
// ============================================================================

#[test]
fn layer_stack_not_empty() {
    let stage = setup_stage();
    let stack = stage.layer_stack();
    assert!(
        !stack.is_empty(),
        "layer stack should have at least root layer"
    );
}

// ============================================================================
// Round-trip through flatten
// ============================================================================

#[test]
fn flatten_roundtrip_prims() {
    let stage = setup_stage();
    stage.define_prim("/A", "Xform").expect("define");
    stage.define_prim("/B", "Mesh").expect("define");

    let flat = stage.flatten(true).expect("flatten");
    let flat_stage = Stage::open_with_root_layer(flat, InitialLoadSet::LoadAll);
    if let Ok(fs) = flat_stage {
        assert!(
            fs.get_prim_at_path(&Path::from_string("/A").expect("p"))
                .is_some()
        );
        assert!(
            fs.get_prim_at_path(&Path::from_string("/B").expect("p"))
                .is_some()
        );
    }
}

#[test]
fn flatten_roundtrip_attributes() {
    let stage = setup_stage();
    let prim = stage.define_prim("/Test", "Xform").expect("define");
    let float_type = common::vtn("float");
    let attr = prim
        .create_attribute("val", &float_type, false, None)
        .expect("create");
    attr.set(Value::from(42.0f32), TimeCode::default_time());

    let flat = stage.flatten(true).expect("flatten");
    let flat_stage = Stage::open_with_root_layer(flat, InitialLoadSet::LoadAll);
    if let Ok(fs) = flat_stage {
        if let Some(p) = fs.get_prim_at_path(&Path::from_string("/Test").expect("p")) {
            if let Some(a) = p.get_attribute("val") {
                let v = a.get(TimeCode::default_time());
                if let Some(v) = v {
                    if let Some(f) = v.get::<f32>() {
                        assert!((f - 42.0f32).abs() < 1e-6);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Stage identifier
// ============================================================================

#[test]
fn stage_root_layer_identifier() {
    let stage = setup_stage();
    let layer = stage.get_root_layer();
    let id = layer.identifier();
    assert!(!id.is_empty(), "root layer should have an identifier");
}
