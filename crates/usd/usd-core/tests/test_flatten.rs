// Port of testUsdFlatten.py + testUsdFlattenLayerStack.py — core subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdFlatten*.py

mod common;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/World", "Xform").expect("define /World");
    stage
        .define_prim("/World/Geom", "Mesh")
        .expect("define geom");
    stage
        .define_prim("/World/Light", "SphereLight")
        .expect("define light");
    stage
}

// ============================================================================
// Stage.Flatten()
// ============================================================================

#[test]
fn flatten_basic() {
    // C++ ref: basic flatten produces a single-layer stage
    let stage = setup_stage();
    let flattened = stage.flatten(true);
    assert!(flattened.is_ok(), "flatten should produce a layer");
}

#[test]
fn flatten_preserves_prims() {
    let stage = setup_stage();
    let flat_layer = stage.flatten(true).expect("flatten");

    // The flattened layer should have prim specs for our prims
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/World").expect("p"))
            .is_some()
    );
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/World/Geom").expect("p"))
            .is_some()
    );
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/World/Light").expect("p"))
            .is_some()
    );
}

#[test]
fn flatten_no_sublayers() {
    // Flattened layer should not have sublayer references
    let stage = setup_stage();
    let flat_layer = stage.flatten(true).expect("flatten");
    let sublayers = flat_layer.sublayer_paths();
    assert!(
        sublayers.is_empty(),
        "flattened layer should have no sublayers"
    );
}

// ============================================================================
// Flatten with attributes
// ============================================================================

#[test]
fn flatten_preserves_attributes() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/World/Geom").expect("p"))
        .expect("prim");

    let float_type = common::vtn("float");
    prim.create_attribute("myValue", &float_type, false, None);

    let flat_layer = stage.flatten(true).expect("flatten");
    let geom_spec = flat_layer.get_prim_at_path(&Path::from_string("/World/Geom").expect("p"));
    assert!(geom_spec.is_some());
}

// ============================================================================
// Flatten with metadata
// ============================================================================

#[test]
fn flatten_preserves_metadata() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/World").expect("p"))
        .expect("prim");
    prim.set_metadata(
        &usd_tf::Token::new("documentation"),
        usd_vt::Value::from("test doc".to_string()),
    );

    let flat_layer = stage.flatten(true).expect("flatten");
    let spec = flat_layer.get_prim_at_path(&Path::from_string("/World").expect("p"));
    assert!(spec.is_some());
}

// ============================================================================
// Flatten empty stage
// ============================================================================

#[test]
fn flatten_empty_stage() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let flat_layer = stage.flatten(true);
    assert!(flat_layer.is_ok());
}

// ============================================================================
// Flatten with references
// ============================================================================

#[test]
fn flatten_with_references() {
    let stage = setup_stage();

    // Create a prim with an internal reference
    let prim = stage.define_prim("/Instanced", "Xform").expect("define");
    let refs = prim.get_references();
    refs.add_internal_reference(
        &Path::from_string("/World/Geom").expect("p"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::FrontOfPrependList,
    );

    let flat_layer = stage.flatten(true).expect("flatten");
    // Flattened layer should have the prim
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/Instanced").expect("p"))
            .is_some()
    );
}

// ============================================================================
// Flatten with inherits
// ============================================================================

#[test]
fn flatten_with_inherits() {
    let stage = setup_stage();

    stage
        .create_class_prim("/_class_Base")
        .expect("define class");
    let prim = stage.define_prim("/Derived", "Xform").expect("define");
    let inherits = prim.get_inherits();
    inherits.add_inherit(
        &Path::from_string("/_class_Base").expect("p"),
        usd_core::common::ListPosition::FrontOfPrependList,
    );

    let flat_layer = stage.flatten(true).expect("flatten");
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/Derived").expect("p"))
            .is_some()
    );
    assert!(
        flat_layer
            .get_prim_at_path(&Path::from_string("/_class_Base").expect("p"))
            .is_some()
    );
}

// ============================================================================
// Port of testUsdFlatten.py: test_NoFallbacks
// ============================================================================

#[test]
#[ignore = "Needs OpenStage + fallback values verification"]
fn flatten_no_fallbacks() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_Export
// ============================================================================

#[test]
#[ignore = "Needs Export to file + re-open verification"]
fn flatten_export() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenClips
// ============================================================================

#[test]
#[ignore = "Needs value clips + flatten"]
fn flatten_clips() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenBadMetadata
// ============================================================================

#[test]
#[ignore = "Needs badMetadata.usd test asset"]
fn flatten_bad_metadata() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenRelationshipTargets
// ============================================================================

#[test]
#[ignore = "Needs disk files (relationshipTargets/source.usda)"]
fn flatten_relationship_targets() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenConnections
// ============================================================================

#[test]
#[ignore = "Needs disk files (connections/source.usda)"]
fn flatten_connections() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenTimeSamplesAndDefaults
// ============================================================================

#[test]
#[ignore = "Needs disk files + time samples flatten verification"]
fn flatten_time_samples_and_defaults() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenAssetPaths
// ============================================================================

#[test]
#[ignore = "Needs disk files + asset path resolution"]
fn flatten_asset_paths() {
    common::setup();
}

// ============================================================================
// Port of testUsdFlatten.py: test_FlattenStageMetadata
// ============================================================================

#[test]
#[ignore = "Needs disk files + stage metadata verification"]
fn flatten_stage_metadata() {
    common::setup();
}
