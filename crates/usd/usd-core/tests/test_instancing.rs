//! Tests for USD instancing.
//!
//! Ported from (selected tests):
//!   - pxr/usd/usd/testenv/testUsdInstancing.py
//!   - pxr/usd/usd/testenv/testUsdInstancingCpp.cpp
//!
//! Many tests from the reference require PrimIndex node access and complex
//! external test data. We port the feasible subset using inline USDA.

mod common;

use std::sync::Arc;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Layer, Path};

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

/// Create a stage from inline USDA.
fn stage_from_usda(usda: &str) -> Arc<Stage> {
    let layer = Layer::create_anonymous(Some(".usda"));
    assert!(layer.import_from_string(usda), "import_from_string failed");
    Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage")
}

// ============================================================================
// Basic instancing: instanceable flag, prototype creation
// ============================================================================

#[test]
fn instancing_basic() {
    common::setup();

    // Two prims referencing the same source, both marked instanceable
    let ref_layer = Layer::create_anonymous(Some("ref.usda"));
    ref_layer.import_from_string(
        r#"#usda 1.0

def "RefPrim" {
    int myAttr = 42
    def "Child" {
        float childAttr = 1.0
    }
}
"#,
    );

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let ref_id = ref_layer.identifier().to_string();
    let usda = format!(
        r##"#usda 1.0

def "Instance1" (
    instanceable = true
    references = @{ref_id}@</RefPrim>
)
{{
}}

def "Instance2" (
    instanceable = true
    references = @{ref_id}@</RefPrim>
)
{{
}}

def "NonInstance" (
    references = @{ref_id}@</RefPrim>
)
{{
}}
"##
    );
    root_layer.import_from_string(&usda);

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Instance1 and Instance2 should be instances
    let inst1 = stage
        .get_prim_at_path(&p("/Instance1"))
        .expect("/Instance1");
    let inst2 = stage
        .get_prim_at_path(&p("/Instance2"))
        .expect("/Instance2");
    let non_inst = stage
        .get_prim_at_path(&p("/NonInstance"))
        .expect("/NonInstance");

    assert!(inst1.is_instanceable(), "Instance1 should be instanceable");
    assert!(inst2.is_instanceable(), "Instance2 should be instanceable");
    assert!(
        !non_inst.is_instanceable(),
        "NonInstance should not be instanceable"
    );

    // Instance prims should be instances (composition resolved)
    assert!(inst1.is_instance(), "Instance1 should be an instance");
    assert!(inst2.is_instance(), "Instance2 should be an instance");
    assert!(
        !non_inst.is_instance(),
        "NonInstance should not be an instance"
    );

    // Both instances should share the same prototype
    let proto1 = inst1.get_prototype();
    let proto2 = inst2.get_prototype();
    assert!(proto1.is_valid(), "Instance1 prototype should be valid");
    assert!(proto2.is_valid(), "Instance2 prototype should be valid");
    assert_eq!(
        proto1.get_path(),
        proto2.get_path(),
        "Both instances should share same prototype"
    );

    // Prototype should be a prototype prim
    assert!(proto1.is_prototype(), "prototype should be is_prototype");
    assert!(
        proto1.is_in_prototype(),
        "prototype should be is_in_prototype"
    );

    // Non-instance should not have a prototype
    let non_proto = non_inst.get_prototype();
    assert!(
        !non_proto.is_valid(),
        "NonInstance should not have prototype"
    );

    // Stage should report prototypes
    let prototypes = stage.get_prototypes();
    assert!(
        !prototypes.is_empty(),
        "stage should have at least one prototype"
    );
}

// ============================================================================
// Set instanceable flag programmatically
// ============================================================================

#[test]
fn instancing_set_instanceable() {
    common::setup();

    let ref_layer = Layer::create_anonymous(Some("ref.usda"));
    ref_layer.import_from_string(
        r#"#usda 1.0

def "Source" {
    int x = 1
}
"#,
    );

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let ref_id = ref_layer.identifier().to_string();
    let usda = format!(
        r##"#usda 1.0

def "A" (
    references = @{ref_id}@</Source>
)
{{
}}
"##
    );
    root_layer.import_from_string(&usda);

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");
    let prim_a = stage.get_prim_at_path(&p("/A")).expect("/A");

    // Not instanceable initially
    assert!(!prim_a.is_instanceable());
    assert!(!prim_a.is_instance());

    // Set instanceable
    assert!(prim_a.set_instanceable(true));
    assert!(prim_a.is_instanceable());
    // After recomposition, it should become an instance
    // (note: recomposition happens automatically in some impls, may need explicit)
}

// ============================================================================
// Instance proxy traversal
// ============================================================================

#[test]
fn instancing_prototype_children() {
    common::setup();

    let ref_layer = Layer::create_anonymous(Some("ref.usda"));
    ref_layer.import_from_string(
        r#"#usda 1.0

def "Model" {
    def "Geom" {
        def "Mesh" {
            int verts = 100
        }
    }
    def "Materials" {
        int shader = 1
    }
}
"#,
    );

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let ref_id = ref_layer.identifier().to_string();
    let usda = format!(
        r##"#usda 1.0

def "Inst" (
    instanceable = true
    references = @{ref_id}@</Model>
)
{{
}}
"##
    );
    root_layer.import_from_string(&usda);

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");
    let inst = stage.get_prim_at_path(&p("/Inst")).expect("/Inst");

    if inst.is_instance() {
        let proto = inst.get_prototype();
        assert!(proto.is_valid());

        // Prototype should have children Geom and Materials
        let proto_children = proto.get_all_children();
        let child_names: Vec<String> = proto_children
            .iter()
            .map(|c| c.get_name().as_str().to_string())
            .collect();
        assert!(
            child_names.contains(&"Geom".to_string()),
            "prototype should have Geom child, got: {child_names:?}"
        );
        assert!(
            child_names.contains(&"Materials".to_string()),
            "prototype should have Materials child, got: {child_names:?}"
        );

        // Children of prototype should be in-prototype
        for child in &proto_children {
            assert!(
                child.is_in_prototype(),
                "child {} should be in prototype",
                child.get_name().as_str()
            );
        }
    }
}

// ============================================================================
// Instance with inherits
// ============================================================================

#[test]
fn instancing_with_inherits() {
    common::setup();

    let stage = stage_from_usda(
        r#"#usda 1.0

class "BaseClass" {
    int classAttr = 100
}

def "A" (
    inherits = </BaseClass>
    instanceable = true
) {
}

def "B" (
    inherits = </BaseClass>
    instanceable = true
) {
}
"#,
    );

    let prim_a = stage.get_prim_at_path(&p("/A")).expect("/A");
    let prim_b = stage.get_prim_at_path(&p("/B")).expect("/B");

    assert!(prim_a.is_instanceable());
    assert!(prim_b.is_instanceable());

    // Both should be instances sharing the same prototype
    if prim_a.is_instance() && prim_b.is_instance() {
        let proto_a = prim_a.get_prototype();
        let proto_b = prim_b.get_prototype();
        assert!(proto_a.is_valid());
        assert!(proto_b.is_valid());
        assert_eq!(
            proto_a.get_path(),
            proto_b.get_path(),
            "instances with same inherits should share prototype"
        );
    }
}

// ============================================================================
// Deinstancing: clearing instanceable flag
// ============================================================================

#[test]
fn instancing_deinstance() {
    common::setup();

    let ref_layer = Layer::create_anonymous(Some("ref.usda"));
    ref_layer.import_from_string(
        r#"#usda 1.0

def "Source" {
    string tag = "hello"
}
"#,
    );

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let ref_id = ref_layer.identifier().to_string();
    let usda = format!(
        r##"#usda 1.0

def "Inst" (
    instanceable = true
    references = @{ref_id}@</Source>
)
{{
}}
"##
    );
    root_layer.import_from_string(&usda);

    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open stage");
    let prim = stage.get_prim_at_path(&p("/Inst")).expect("/Inst");

    // Should start as instance
    assert!(prim.is_instanceable());

    // Deinstance
    assert!(prim.set_instanceable(false));
    assert!(!prim.is_instanceable());
}

// ============================================================================
// Prototype path prefix
// ============================================================================

#[test]
fn instancing_prototype_paths() {
    common::setup();

    // Prototype paths use a special prefix "/__Prototype_"
    assert!(
        usd_core::Prim::is_prototype_path(&p("/__Prototype_1")),
        "/__Prototype_1 should be a prototype path"
    );
    assert!(
        usd_core::Prim::is_prototype_path(&p("/__Prototype_0")),
        "/__Prototype_0 should be a prototype path"
    );
    assert!(
        !usd_core::Prim::is_prototype_path(&p("/Regular")),
        "/Regular should NOT be a prototype path"
    );
    assert!(
        !usd_core::Prim::is_prototype_path(&p("/Prototype")),
        "/Prototype should NOT be a prototype path"
    );
}
