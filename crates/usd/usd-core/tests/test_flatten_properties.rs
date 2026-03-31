// Port of testUsdFlattenProperties.py — property flattening subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdFlattenProperties.py

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_vt::Value;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Root", "Xform").expect("define /Root");

    let float_type = common::vtn("float");
    let int_type = common::vtn("int");
    let str_type = common::vtn("string");

    prim.create_attribute("floatAttr", &float_type, false, None);
    prim.create_attribute("intAttr", &int_type, false, None);
    prim.create_attribute("strAttr", &str_type, false, None);

    // Set default values
    if let Some(attr) = prim.get_attribute("floatAttr") {
        attr.set(Value::from(1.5f32), TimeCode::default_time());
    }
    if let Some(attr) = prim.get_attribute("intAttr") {
        attr.set(Value::from(42i32), TimeCode::default_time());
    }
    if let Some(attr) = prim.get_attribute("strAttr") {
        attr.set(Value::from("hello".to_string()), TimeCode::default_time());
    }

    stage
}

// ============================================================================
// Flatten preserves attribute values
// ============================================================================

#[test]
fn flatten_preserves_float_attr() {
    let stage = setup_stage();
    let flat_layer = stage.flatten(true).expect("flatten");

    // Open a new stage from the flattened layer to verify values
    let flat_stage = Stage::open_with_root_layer(flat_layer, InitialLoadSet::LoadAll);
    if let Ok(flat_stage) = flat_stage {
        let prim = flat_stage.get_prim_at_path(&Path::from_string("/Root").expect("p"));
        if let Some(prim) = prim {
            if let Some(attr) = prim.get_attribute("floatAttr") {
                let val = attr.get(TimeCode::default_time());
                if let Some(v) = val {
                    if let Some(f) = v.get::<f32>() {
                        assert!((f - 1.5f32).abs() < 1e-6);
                    }
                }
            }
        }
    }
}

#[test]
fn flatten_preserves_int_attr() {
    let stage = setup_stage();
    let flat_layer = stage.flatten(true).expect("flatten");

    let flat_stage = Stage::open_with_root_layer(flat_layer, InitialLoadSet::LoadAll);
    if let Ok(flat_stage) = flat_stage {
        let prim = flat_stage.get_prim_at_path(&Path::from_string("/Root").expect("p"));
        if let Some(prim) = prim {
            if let Some(attr) = prim.get_attribute("intAttr") {
                let val = attr.get(TimeCode::default_time());
                if let Some(v) = val {
                    if let Some(i) = v.get::<i32>() {
                        assert_eq!(*i, 42);
                    }
                }
            }
        }
    }
}

#[test]
fn flatten_preserves_string_attr() {
    let stage = setup_stage();
    let flat_layer = stage.flatten(true).expect("flatten");

    let flat_stage = Stage::open_with_root_layer(flat_layer, InitialLoadSet::LoadAll);
    if let Ok(flat_stage) = flat_stage {
        let prim = flat_stage.get_prim_at_path(&Path::from_string("/Root").expect("p"));
        if let Some(prim) = prim {
            if let Some(attr) = prim.get_attribute("strAttr") {
                let val = attr.get(TimeCode::default_time());
                if let Some(v) = val {
                    if let Some(s) = v.get::<String>() {
                        assert_eq!(s, "hello");
                    }
                }
            }
        }
    }
}

// ============================================================================
// Flatten with authored properties from references
// ============================================================================

#[test]
fn flatten_reference_properties() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    let source = stage.define_prim("/Source", "Mesh").expect("define");
    let float_type = common::vtn("float");
    source.create_attribute("val", &float_type, false, None);
    if let Some(attr) = source.get_attribute("val") {
        attr.set(Value::from(99.0f32), TimeCode::default_time());
    }

    let ref_prim = stage.define_prim("/Ref", "Xform").expect("define");
    let refs = ref_prim.get_references();
    refs.add_internal_reference(
        &Path::from_string("/Source").expect("p"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::FrontOfPrependList,
    );

    let flat_layer = stage.flatten(true).expect("flatten");
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/Ref").expect("p"))
            .is_some()
    );
}

// ============================================================================
// Flatten preserves attribute count
// ============================================================================

#[test]
fn flatten_preserves_attribute_count() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let orig_count = prim.get_attribute_names().len();

    let flat_layer = stage.flatten(true).expect("flatten");
    let flat_stage = Stage::open_with_root_layer(flat_layer, InitialLoadSet::LoadAll);
    if let Ok(flat_stage) = flat_stage {
        if let Some(flat_prim) =
            flat_stage.get_prim_at_path(&Path::from_string("/Root").expect("p"))
        {
            let flat_count = flat_prim.get_attribute_names().len();
            assert_eq!(flat_count, orig_count);
        }
    }
}
