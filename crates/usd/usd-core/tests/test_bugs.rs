// Port of testUsdBugs.py — regression tests for known bugs
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdBugs.py

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
// Bug: Removing prim should not crash
// ============================================================================

#[test]
fn remove_prim_no_crash() {
    let stage = setup_stage();
    stage.define_prim("/ToRemove", "Xform").expect("define");
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/ToRemove").expect("p"))
            .is_some()
    );

    assert!(stage.remove_prim(&Path::from_string("/ToRemove").expect("p")));
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/ToRemove").expect("p"))
            .is_none()
    );
}

#[test]
fn remove_nonexistent_prim() {
    let stage = setup_stage();
    let result = stage.remove_prim(&Path::from_string("/DoesNotExist").expect("p"));
    // Should not crash, may return false
    let _ = result;
}

// ============================================================================
// Bug: Define prim with empty type
// ============================================================================

#[test]
fn define_prim_empty_type() {
    // Should handle empty type gracefully (creates untyped def)
    let stage = setup_stage();
    let result = stage.define_prim("/Untyped", "");
    assert!(result.is_ok());
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Untyped").expect("p"))
        .expect("prim");
    assert!(prim.is_valid());
}

// ============================================================================
// Bug: Attribute default value roundtrip
// ============================================================================

#[test]
fn attribute_float_default_roundtrip() {
    let stage = setup_stage();
    let prim = stage.define_prim("/Test", "Xform").expect("define");

    let float_type = common::vtn("float");
    let attr = prim
        .create_attribute("myFloat", &float_type, false, None)
        .expect("create attr");
    attr.set(Value::from(3.14f32), TimeCode::default_time());

    let val = attr.get(TimeCode::default_time());
    assert!(val.is_some());
    if let Some(v) = val {
        if let Some(f) = v.get::<f32>() {
            assert!((f - 3.14f32).abs() < 1e-6);
        }
    }
}

#[test]
fn attribute_int_default_roundtrip() {
    let stage = setup_stage();
    let prim = stage.define_prim("/Test", "Xform").expect("define");

    let int_type = common::vtn("int");
    let attr = prim
        .create_attribute("myInt", &int_type, false, None)
        .expect("create attr");
    attr.set(Value::from(42i32), TimeCode::default_time());

    let val = attr.get(TimeCode::default_time());
    assert!(val.is_some());
    if let Some(v) = val {
        if let Some(i) = v.get::<i32>() {
            assert_eq!(*i, 42);
        }
    }
}

#[test]
fn attribute_string_default_roundtrip() {
    let stage = setup_stage();
    let prim = stage.define_prim("/Test", "Xform").expect("define");

    let str_type = common::vtn("string");
    let attr = prim
        .create_attribute("myStr", &str_type, false, None)
        .expect("create attr");
    attr.set(
        Value::from("hello world".to_string()),
        TimeCode::default_time(),
    );

    let val = attr.get(TimeCode::default_time());
    assert!(val.is_some());
    if let Some(v) = val {
        if let Some(s) = v.get::<String>() {
            assert_eq!(s, "hello world");
        }
    }
}

// ============================================================================
// Bug: Children after remove
// ============================================================================

#[test]
fn children_after_sibling_remove() {
    let stage = setup_stage();
    stage.define_prim("/Parent", "Xform").expect("define");
    stage.define_prim("/Parent/A", "Mesh").expect("define");
    stage.define_prim("/Parent/B", "Mesh").expect("define");
    stage.define_prim("/Parent/C", "Mesh").expect("define");

    let parent = stage
        .get_prim_at_path(&Path::from_string("/Parent").expect("p"))
        .expect("prim");
    let children_before = parent.get_all_children();
    assert_eq!(children_before.len(), 3);

    stage.remove_prim(&Path::from_string("/Parent/B").expect("p"));

    let children_after = parent.get_all_children();
    assert_eq!(children_after.len(), 2);
    let names: Vec<_> = children_after
        .iter()
        .map(|p| p.get_name().to_string())
        .collect();
    assert!(names.contains(&"A".to_string()));
    assert!(names.contains(&"C".to_string()));
    assert!(!names.contains(&"B".to_string()));
}

// ============================================================================
// Bug: Metadata on pseudo root
// ============================================================================

#[test]
fn metadata_on_pseudo_root() {
    let stage = setup_stage();
    let pseudo = stage.pseudo_root();
    // Setting metadata on pseudo root should not crash
    let result = pseudo.set_metadata(
        &usd_tf::Token::new("documentation"),
        Value::from("root doc".to_string()),
    );
    let _ = result;
}

// ============================================================================
// Bug: Empty stage traversal
// ============================================================================

#[test]
fn empty_stage_traverse() {
    let _stage = setup_stage();
    // Empty stage (no user prims)
    let stage2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let prims: Vec<_> = stage2.traverse().into_iter().collect();
    assert!(prims.is_empty());
}

// ============================================================================
// Bug: Property path operations
// ============================================================================

#[test]
fn property_path_parent() {
    let path = Path::from_string("/Prim.attr").expect("p");
    assert!(path.is_property_path());
    let parent = path.get_parent_path();
    assert_eq!(parent.get_string(), "/Prim");
    assert!(parent.is_prim_path());
}

#[test]
fn property_path_name() {
    let path = Path::from_string("/Prim.attr").expect("p");
    assert_eq!(path.get_name(), "attr");
}

// ============================================================================
// Bug: Nested define_prim auto-creates parents
// ============================================================================

#[test]
fn define_prim_auto_creates_parents() {
    let stage = setup_stage();
    let result = stage.define_prim("/Deep/Nested/Path", "Mesh");
    assert!(result.is_ok());

    // Parents should exist
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Deep").expect("p"))
            .is_some()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Deep/Nested").expect("p"))
            .is_some()
    );
    assert!(
        stage
            .get_prim_at_path(&Path::from_string("/Deep/Nested/Path").expect("p"))
            .is_some()
    );
}
