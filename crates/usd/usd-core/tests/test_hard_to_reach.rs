// Port of testUsdHardToReach.cpp — edge cases and hard-to-reach code paths
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdHardToReach.cpp

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_core::prim::Prim;
use usd_sdf::Path;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "Xform").expect("define /Root");
    stage
        .define_prim("/Root/A", "Mesh")
        .expect("define /Root/A");
    stage
        .define_prim("/Root/B", "Scope")
        .expect("define /Root/B");
    stage
}

// ============================================================================
// Invalid prim operations
// ============================================================================

#[test]
fn invalid_prim_is_not_valid() {
    let prim = Prim::invalid();
    assert!(!prim.is_valid());
    assert!(!prim.is_active());
    assert!(!prim.is_defined());
    assert!(!prim.is_instance());
    assert!(!prim.is_instance_proxy());
    assert!(!prim.is_prototype());
}

#[test]
fn get_prim_at_empty_path() {
    let stage = setup_stage();
    let empty = Path::empty();
    let prim = stage.get_prim_at_path(&empty);
    assert!(prim.is_none());
}

#[test]
fn get_prim_at_nonexistent_path() {
    let stage = setup_stage();
    let path = Path::from_string("/Nonexistent/Deep/Path").expect("p");
    assert!(stage.get_prim_at_path(&path).is_none());
}

// ============================================================================
// Pseudo root
// ============================================================================

#[test]
fn pseudo_root_is_valid() {
    let stage = setup_stage();
    let pseudo = stage.pseudo_root();
    assert!(pseudo.is_valid());
    assert_eq!(pseudo.path().get_string(), "/");
}

#[test]
fn pseudo_root_children() {
    let stage = setup_stage();
    let pseudo = stage.pseudo_root();
    let children = pseudo.get_all_children();
    // Should have /Root as child
    assert!(!children.is_empty());
}

// ============================================================================
// Prim type name
// ============================================================================

#[test]
fn prim_type_name() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root/A").expect("p"))
        .expect("prim");
    assert_eq!(prim.get_type_name().as_str(), "Mesh");
}

#[test]
fn prim_type_name_scope() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root/B").expect("p"))
        .expect("prim");
    assert_eq!(prim.get_type_name().as_str(), "Scope");
}

// ============================================================================
// Define/override prim edge cases
// ============================================================================

#[test]
fn define_prim_at_existing_path() {
    // Redefining at an existing path should succeed (update type if different)
    let stage = setup_stage();
    let result = stage.define_prim("/Root", "Mesh");
    assert!(result.is_ok());
}

#[test]
fn override_prim() {
    let stage = setup_stage();
    let result = stage.override_prim("/NewOver");
    assert!(result.is_ok());
    let prim = stage
        .get_prim_at_path(&Path::from_string("/NewOver").expect("p"))
        .expect("prim");
    assert!(prim.is_valid());
}

#[test]
fn create_class_prim() {
    let stage = setup_stage();
    let result = stage.create_class_prim("/_class_Test");
    assert!(result.is_ok());
    let prim = stage
        .get_prim_at_path(&Path::from_string("/_class_Test").expect("p"))
        .expect("prim");
    assert!(prim.is_valid());
    assert!(prim.is_abstract());
}

// ============================================================================
// Prim active/inactive
// ============================================================================

#[test]
fn set_prim_inactive() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root/A").expect("p"))
        .expect("prim");
    assert!(prim.is_active());

    prim.set_active(false);
    // After deactivation, the prim should still be accessible but inactive
    let prim2 = stage.get_prim_at_path(&Path::from_string("/Root/A").expect("p"));
    if let Some(p) = prim2 {
        assert!(!p.is_active());
    }
}

// ============================================================================
// Multiple stages independence
// ============================================================================

#[test]
fn stages_are_independent() {
    common::setup();
    let stage1 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("s1");
    let stage2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("s2");

    stage1
        .define_prim("/OnlyInStage1", "Xform")
        .expect("define");
    stage2
        .define_prim("/OnlyInStage2", "Xform")
        .expect("define");

    assert!(
        stage1
            .get_prim_at_path(&Path::from_string("/OnlyInStage1").expect("p"))
            .is_some()
    );
    assert!(
        stage1
            .get_prim_at_path(&Path::from_string("/OnlyInStage2").expect("p"))
            .is_none()
    );

    assert!(
        stage2
            .get_prim_at_path(&Path::from_string("/OnlyInStage2").expect("p"))
            .is_some()
    );
    assert!(
        stage2
            .get_prim_at_path(&Path::from_string("/OnlyInStage1").expect("p"))
            .is_none()
    );
}

// ============================================================================
// Path edge cases
// ============================================================================

#[test]
fn path_from_string_empty() {
    let path = Path::from_string("");
    // Empty string should fail or return empty path
    assert!(path.is_none() || path.as_ref().map(|p| p.is_empty()).unwrap_or(true));
}

#[test]
fn deeply_nested_prim() {
    let stage = setup_stage();
    // Create a deeply nested hierarchy
    let mut path = String::from("/Deep");
    stage.define_prim(&path, "Xform").expect("define");
    for i in 0..10 {
        path.push_str(&format!("/Level_{}", i));
        stage.define_prim(&path, "Xform").expect("define nested");
    }
    let deep_path = Path::from_string(&path).expect("p");
    assert!(stage.get_prim_at_path(&deep_path).is_some());
}
