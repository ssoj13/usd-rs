//! Tests for EditTarget and EditContext.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdEditTarget.py
//!
//! Skipped:
//!   - test_PathTranslationAndValueResolution (PrimIndex node access)
//!   - test_StageEditTargetSessionSublayer (external test data)
//!   - test_StageEditTargetInstancing2 (PrimIndex node access)
//!   - test_BugRegressionTest_VariantEditTargetCrash (variant edit target + namespace edit)

mod common;

use std::sync::Arc;
use usd_core::{EditContext, EditTarget, InitialLoadSet, Stage};
use usd_sdf::{Layer, Path, TimeCode};

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// test_StageEditTargetAPI — set/get_edit_target(), target layers
//
// Tests basic EditTarget switching between root and sublayer, plus
// EditContext RAII scope for temporary target changes.
// ============================================================================

#[test]
fn edit_target_stage_api() {
    common::setup();

    // Create root and sub layers
    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let sub_layer = Layer::create_anonymous(Some("sub.usda"));

    // Set up root layer with a prim /Foo and an attribute
    root_layer.import_from_string(
        r#"#usda 1.0

def "Foo" {
    string myAttr = "from_root"
}
"#,
    );

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Default edit target should be the root layer
    let edit_target = stage.get_edit_target();
    assert!(
        edit_target.is_valid(),
        "default edit target should be valid"
    );
    let target_layer = edit_target.layer().expect("edit target has layer");
    assert!(
        Arc::ptr_eq(target_layer, &root_layer),
        "default edit target should be root layer"
    );

    // Switch to sub layer
    let sub_target = EditTarget::for_local_layer(Arc::clone(&sub_layer));
    stage.set_edit_target(sub_target);
    let current = stage.get_edit_target();
    assert!(current.is_valid());

    // Switch back to root
    stage.set_edit_target(EditTarget::for_local_layer(Arc::clone(&root_layer)));
    let current = stage.get_edit_target();
    assert!(
        Arc::ptr_eq(current.layer().unwrap(), &root_layer),
        "should be back to root layer"
    );
}

// ============================================================================
// EditContext scope test — temporary edit target, restored on drop
// ============================================================================

#[test]
fn edit_target_context_scope() {
    common::setup();

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let sub_layer = Layer::create_anonymous(Some("sub.usda"));

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");
    stage.define_prim("/Foo", "").expect("define /Foo");

    // Verify we start on root layer
    assert!(Arc::ptr_eq(
        stage.get_edit_target().layer().unwrap(),
        &root_layer
    ));

    // Use EditContext to temporarily switch to sub layer
    {
        let _ctx = EditContext::new_with_target(
            Arc::clone(&stage),
            EditTarget::for_local_layer(Arc::clone(&sub_layer)),
        );

        // Inside context, target should be sub layer
        let inner_target = stage.get_edit_target();
        assert!(inner_target.is_valid());
        // Author something on the sub layer target
        let foo = stage.get_prim_at_path(&p("/Foo")).expect("get /Foo");
        let attr = foo
            .create_attribute("sub_attr", &common::vtn("string"), false, None)
            .expect("create sub_attr");
        attr.set("hello_from_sub".to_string(), TimeCode::default_time());
    }

    // After context drop, target should be restored to root
    assert!(Arc::ptr_eq(
        stage.get_edit_target().layer().unwrap(),
        &root_layer
    ));
}

// ============================================================================
// test_InvalidStage — EditTarget on invalid stage
// ============================================================================

#[test]
fn edit_target_invalid() {
    common::setup();

    // Invalid edit target
    let target = EditTarget::invalid();
    assert!(!target.is_valid());
    assert!(target.is_null());
    assert!(target.layer().is_none());
}

// ============================================================================
// EditTarget path mapping — map_to_spec_path
// ============================================================================

#[test]
fn edit_target_path_mapping() {
    common::setup();

    let layer = Layer::create_anonymous(Some("test.usda"));

    // Local layer target — no remapping
    let local_target = EditTarget::for_local_layer(Arc::clone(&layer));
    let path = p("/World/Cube");
    assert_eq!(local_target.map_to_spec_path(&path), path);

    // Target with path mapping — /World -> /Root
    let mapped_target = EditTarget::for_layer_at_path(Arc::clone(&layer), p("/Root"), p("/World"));
    let mapped = mapped_target.map_to_spec_path(&p("/World/Cube"));
    assert_eq!(mapped.get_string(), "/Root/Cube");

    // Path not under mapping target — returned as-is
    let unmapped = mapped_target.map_to_spec_path(&p("/Other/Thing"));
    assert_eq!(unmapped.get_string(), "/Other/Thing");
}

// ============================================================================
// EditTarget compose_over
// ============================================================================

#[test]
fn edit_target_compose_over() {
    common::setup();

    let layer_a = Layer::create_anonymous(Some("a.usda"));
    let layer_b = Layer::create_anonymous(Some("b.usda"));

    // compose_over: self takes precedence for layer
    let target_a = EditTarget::for_local_layer(Arc::clone(&layer_a));
    let target_b = EditTarget::for_local_layer(Arc::clone(&layer_b));

    let composed = target_a.compose_over(&target_b);
    assert!(Arc::ptr_eq(composed.layer().unwrap(), &layer_a));

    // If self has no layer, use weaker's
    let invalid = EditTarget::invalid();
    let composed2 = invalid.compose_over(&target_b);
    assert!(Arc::ptr_eq(composed2.layer().unwrap(), &layer_b));
}

// ============================================================================
// get_edit_target_for_local_layer
// ============================================================================

#[test]
fn edit_target_for_local_layer() {
    common::setup();

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    let target = stage.get_edit_target_for_local_layer(&root_layer);
    assert!(target.is_valid());
    assert!(Arc::ptr_eq(target.layer().unwrap(), &root_layer));
}
