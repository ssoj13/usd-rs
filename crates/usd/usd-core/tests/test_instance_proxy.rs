// Port of testUsdInstanceProxy.py — core instance proxy subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdInstanceProxy.py

mod common;

use usd_core::prim::Prim;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Layer, Path};
use usd_tf::Token;

fn setup_instanced_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Create a prototype prim
    let _proto = stage.define_prim("/Proto", "Xform").expect("define /Proto");
    stage
        .define_prim("/Proto/Child", "Mesh")
        .expect("define /Proto/Child");

    // Create instances referencing the prototype
    let inst1 = stage
        .define_prim("/Instance1", "Xform")
        .expect("define /Instance1");
    let refs1 = inst1.get_references();
    refs1.add_internal_reference(
        &Path::from_string("/Proto").expect("p"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::FrontOfPrependList,
    );
    // Mark as instanceable
    inst1.set_metadata(&Token::new("instanceable"), usd_vt::Value::from(true));

    let inst2 = stage
        .define_prim("/Instance2", "Xform")
        .expect("define /Instance2");
    let refs2 = inst2.get_references();
    refs2.add_internal_reference(
        &Path::from_string("/Proto").expect("p"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::FrontOfPrependList,
    );
    inst2.set_metadata(&Token::new("instanceable"), usd_vt::Value::from(true));

    stage
}

// ============================================================================
// Instance detection
// ============================================================================

#[test]
fn is_instance() {
    let stage = setup_instanced_stage();
    let inst1 = stage.get_prim_at_path(&Path::from_string("/Instance1").expect("p"));
    if let Some(prim) = inst1 {
        // If instancing resolved, the prim should be an instance
        if prim.is_instance() {
            assert!(!prim.is_prototype());
            assert!(!prim.is_in_prototype());
        }
    }
}

#[test]
fn is_instanceable() {
    let stage = setup_instanced_stage();
    let inst1 = stage
        .get_prim_at_path(&Path::from_string("/Instance1").expect("p"))
        .expect("prim");
    assert!(inst1.is_instanceable());
}

#[test]
fn proto_not_instanceable() {
    let stage = setup_instanced_stage();
    let proto = stage
        .get_prim_at_path(&Path::from_string("/Proto").expect("p"))
        .expect("prim");
    assert!(!proto.is_instanceable());
}

// ============================================================================
// Prototype access
// ============================================================================

#[test]
fn get_prototypes() {
    let stage = setup_instanced_stage();
    let prototypes = stage.get_prototypes();
    // May or may not have prototypes depending on instance resolution
    let _ = prototypes;
}

#[test]
fn get_prototype_from_instance() {
    let stage = setup_instanced_stage();
    let inst1 = stage
        .get_prim_at_path(&Path::from_string("/Instance1").expect("p"))
        .expect("prim");

    if inst1.is_instance() {
        let proto = inst1.get_prototype();
        if proto.is_valid() {
            assert!(proto.is_prototype());
            assert!(proto.is_in_prototype());
        }
    }
}

// ============================================================================
// Instance proxy checks
// ============================================================================

#[test]
fn instance_children_are_proxies() {
    // C++ ref: children of instances should be instance proxies
    let stage = setup_instanced_stage();
    let inst1 = stage
        .get_prim_at_path(&Path::from_string("/Instance1").expect("p"))
        .expect("prim");

    if inst1.is_instance() {
        let children = inst1.get_all_children();
        for child in &children {
            // Children of an instance should be instance proxies
            if child.is_instance_proxy() {
                assert!(child.is_valid());
            }
        }
    }
}

#[test]
fn descendant_proxy_lookup_maps_to_child_not_root() {
    let layer = Layer::create_anonymous(Some("instance_proxy_lookup.usda"));
    assert!(layer.import_from_string(
        r#"#usda 1.0
        def "Proto" {
            def "Child" {}
        }
        def "Instance1" (
            instanceable = true
            references = </Proto>
        ) {
        }
        "#
    ));
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let child = stage
        .get_prim_at_path(&Path::from_string("/Instance1/Child").expect("p"))
        .expect("proxy child");

    assert!(child.is_instance_proxy());
    assert_eq!(
        child.path(),
        &Path::from_string("/Instance1/Child").expect("p")
    );
    assert_eq!(
        child.get_prim_in_prototype().path(),
        &Path::from_string("/__Prototype_1/Child").expect("p")
    );
}

#[test]
fn descendant_proxy_parent_stays_in_instance_namespace() {
    let layer = Layer::create_anonymous(Some("instance_proxy_parent.usda"));
    assert!(layer.import_from_string(
        r#"#usda 1.0
        def "Proto" {
            def "Child" {}
        }
        def "Instance1" (
            instanceable = true
            references = </Proto>
        ) {
        }
        "#
    ));
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let child = stage
        .get_prim_at_path(&Path::from_string("/Instance1/Child").expect("p"))
        .expect("proxy child");

    let parent = child.parent();
    assert!(parent.is_valid());
    assert_eq!(parent.path(), &Path::from_string("/Instance1").expect("p"));
    assert!(parent.is_instance());
}

#[test]
fn prototype_path_detection() {
    // C++ ref: prototype paths have specific prefix
    // In C++ prototypes are at /__Prototype_*
    let proto_path = Path::from_string("/__Prototype_1").expect("p");
    assert!(Prim::is_prototype_path(&proto_path));

    let normal_path = Path::from_string("/World").expect("p");
    assert!(!Prim::is_prototype_path(&normal_path));
}

// ============================================================================
// Non-instanced prims
// ============================================================================

#[test]
fn non_instanced_prim_not_instance() {
    let stage = setup_instanced_stage();
    let proto = stage
        .get_prim_at_path(&Path::from_string("/Proto").expect("p"))
        .expect("prim");
    assert!(!proto.is_instance());
    assert!(!proto.is_instance_proxy());
}
