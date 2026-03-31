//! Tests for USD data format handling.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdDataFormats.py (core subset)
//!
//! Tests basic USDA layer creation, import, export, and round-trip.

mod common;

use std::sync::Arc;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Layer, Path, TimeCode};

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// USDA round-trip: create in memory, export as string, re-import
// ============================================================================

#[test]
fn data_formats_usda_roundtrip() {
    common::setup();

    let layer = Layer::create_anonymous(Some(".usda"));
    let stage = Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Author some data
    stage.define_prim("/World", "Xform").expect("define World");
    stage
        .define_prim("/World/Cube", "Mesh")
        .expect("define Cube");

    let cube = stage.get_prim_at_path(&p("/World/Cube")).expect("get Cube");
    let attr = cube
        .create_attribute("size", &common::vtn("double"), false, None)
        .expect("create size");
    attr.set(2.0_f64, TimeCode::default_time());

    // Export to USDA string
    let usda_string = layer.export_to_string().expect("export to string");
    assert!(
        !usda_string.is_empty(),
        "exported USDA string should not be empty"
    );
    assert!(
        usda_string.contains("World"),
        "exported string should contain World"
    );
    assert!(
        usda_string.contains("Cube"),
        "exported string should contain Cube"
    );

    // Re-import into a fresh layer
    let layer2 = Layer::create_anonymous(Some(".usda"));
    assert!(
        layer2.import_from_string(&usda_string),
        "import_from_string should succeed"
    );

    let stage2 = Stage::open_with_root_layer(Arc::clone(&layer2), InitialLoadSet::LoadAll)
        .expect("open re-imported stage");

    // Verify round-tripped data
    let cube2 = stage2
        .get_prim_at_path(&p("/World/Cube"))
        .expect("get Cube after roundtrip");
    assert_eq!(cube2.get_type_name().as_str(), "Mesh");
    let attr2 = cube2.get_attribute("size").expect("get size attr");
    let val: f64 = attr2
        .get(TimeCode::default_time())
        .and_then(|v| v.get::<f64>().copied())
        .expect("get size value");
    assert_eq!(val, 2.0);
}

// ============================================================================
// Anonymous layer identifier
// ============================================================================

#[test]
fn data_formats_anonymous_layer_id() {
    common::setup();

    let layer = Layer::create_anonymous(Some("test.usda"));
    let id = layer.identifier();

    // Anonymous layer identifiers start with "anon:" prefix
    assert!(
        !id.is_empty(),
        "anonymous layer should have non-empty identifier"
    );
}

// ============================================================================
// Empty layer export
// ============================================================================

#[test]
fn data_formats_empty_layer() {
    common::setup();

    let layer = Layer::create_anonymous(Some(".usda"));
    let usda = layer.export_to_string().expect("export to string");

    // Even an empty layer should produce valid USDA (at minimum the header)
    assert!(
        !usda.is_empty(),
        "empty layer should still export something"
    );
}

// ============================================================================
// Multiple prims with attributes
// ============================================================================

#[test]
fn data_formats_multiple_prims() {
    common::setup();

    let layer = Layer::create_anonymous(Some(".usda"));
    let stage = Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Create several prims with attributes of different types
    stage.define_prim("/A", "").expect("define /A");
    stage.define_prim("/B", "").expect("define /B");
    stage.define_prim("/C", "").expect("define /C");

    let a = stage.get_prim_at_path(&p("/A")).expect("get A");
    let b = stage.get_prim_at_path(&p("/B")).expect("get B");

    let int_attr = a
        .create_attribute("myInt", &common::vtn("int"), false, None)
        .expect("create int attr");
    int_attr.set(42_i32, TimeCode::default_time());

    let str_attr = b
        .create_attribute("myString", &common::vtn("string"), false, None)
        .expect("create string attr");
    str_attr.set("hello".to_string(), TimeCode::default_time());

    // Export and verify
    let usda = layer.export_to_string().expect("export to string");
    assert!(usda.contains("myInt"), "should contain myInt");
    assert!(usda.contains("myString"), "should contain myString");
    assert!(usda.contains("42"), "should contain value 42");
    assert!(usda.contains("hello"), "should contain value hello");
}
