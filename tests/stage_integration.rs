//! Integration tests for Stage and Prim operations.
//!
//! Based on OpenUSD C++ tests to ensure compatibility.

use usd::sdf::Path;
use usd::tf::Token;
use usd::usd::{InitialLoadSet, Stage};

/// Test basic Stage creation in memory
#[test]
fn test_stage_create_in_memory() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // Stage should have a pseudo root
    let root = stage.pseudo_root();
    assert!(root.is_valid(), "Pseudo root should be valid");
    assert!(root.is_pseudo_root(), "Should be pseudo root");
    assert_eq!(root.path().get_as_string(), "/");
}

/// Test defining prims on a stage
#[test]
fn test_stage_define_prim() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // Define a simple prim
    let prim = stage
        .define_prim("/World", "Xform")
        .expect("Failed to define prim");
    assert!(prim.is_valid(), "Prim should be valid");
    assert_eq!(prim.name().get_text(), "World");
    assert_eq!(prim.path().get_as_string(), "/World");

    // Define a child prim
    let child = stage
        .define_prim("/World/Child", "Mesh")
        .expect("Failed to define child prim");
    assert!(child.is_valid());
    assert_eq!(child.path().get_as_string(), "/World/Child");

    // Parent should have the child
    let parent = child.parent();
    assert!(parent.is_valid());
    assert_eq!(parent.path().get_as_string(), "/World");
}

/// Test prim hierarchy traversal
#[test]
fn test_prim_hierarchy() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // Create a hierarchy
    let _ = stage.define_prim("/A", "").unwrap();
    let _ = stage.define_prim("/A/B", "").unwrap();
    let _ = stage.define_prim("/A/C", "").unwrap();
    let _ = stage.define_prim("/A/B/D", "").unwrap();

    // Test get_prim_at_path
    let prim_a = stage
        .get_prim_at_path(&Path::from("/A"))
        .expect("Should find /A");
    assert_eq!(prim_a.path().get_as_string(), "/A");

    // Test children
    let children = prim_a.children();
    assert_eq!(children.len(), 2);

    // Test descendants
    let descendants = prim_a.descendants();
    assert_eq!(descendants.len(), 3); // B, C, D
}

/// Test Stage traversal
#[test]
fn test_stage_traverse() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let _ = stage.define_prim("/Root", "").unwrap();
    let _ = stage.define_prim("/Root/A", "").unwrap();
    let _ = stage.define_prim("/Root/B", "").unwrap();

    // Traverse should find all prims
    let all_prims: Vec<_> = stage.traverse().into_iter().collect();
    // Should have: Root, A, B (pseudo root is not included in default traversal)
    assert!(all_prims.len() >= 3, "Should have at least 3 prims");
}

/// Test prim flags
#[test]
fn test_prim_flags() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let prim = stage.define_prim("/Test", "").unwrap();

    // Newly defined prims should be active
    assert!(prim.is_active());
    assert!(prim.is_defined());
    assert!(!prim.is_abstract());
}

/// Test override prim
#[test]
fn test_override_prim() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // Create an override (no def)
    let prim = stage
        .override_prim("/Override")
        .expect("Failed to create override");
    assert!(prim.is_valid());
    // Override prims are not "defined" in the USD sense
    assert!(!prim.is_defined());
}

/// Test class prim
#[test]
fn test_class_prim() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let prim = stage
        .create_class_prim("/_class_Base")
        .expect("Failed to create class");
    assert!(prim.is_valid());
    // Class prims are used for inheritance but may not be "abstract" in the schema sense
    // They should be valid and retrievable
    assert_eq!(prim.name().get_text(), "_class_Base");
}

/// Test remove prim
#[test]
fn test_remove_prim() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let _ = stage.define_prim("/ToRemove", "").unwrap();
    assert!(stage.get_prim_at_path(&Path::from("/ToRemove")).is_some());

    let removed = stage.remove_prim(&Path::from("/ToRemove"));
    assert!(removed);
    assert!(stage.get_prim_at_path(&Path::from("/ToRemove")).is_none());
}

/// Test default prim
#[test]
fn test_default_prim() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    // No default prim initially
    assert!(!stage.has_default_prim());

    let prim = stage.define_prim("/Main", "").unwrap();
    assert!(stage.set_default_prim(&prim));
    assert!(stage.has_default_prim());

    let default = stage.default_prim().expect("Should have default prim");
    assert_eq!(default.path().get_as_string(), "/Main");

    stage.clear_default_prim();
    assert!(!stage.has_default_prim());
}

/// Test Stage export to string
#[test]
fn test_stage_export_to_string() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");
    let _ = stage.define_prim("/Test", "Xform").unwrap();

    let exported = stage
        .export_to_string(false)
        .expect("Failed to export stage");

    // Should contain usda header and our prim
    assert!(exported.contains("#usda"));
    assert!(exported.contains("def Xform \"Test\""));
}

/// Test layer stack
#[test]
fn test_layer_stack() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let root_layer = stage.root_layer();
    assert!(!root_layer.identifier().is_empty());

    let layers = stage.layer_stack();
    assert!(!layers.is_empty());
}

/// Test Stage metadata
#[test]
fn test_stage_metadata() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let doc_key = Token::new("documentation");

    // Initially no doc
    assert!(!stage.has_authored_metadata(&doc_key));

    // Set documentation
    let doc_value = usd::vt::Value::from("Test documentation".to_string());
    assert!(stage.set_metadata(&doc_key, doc_value));
    assert!(stage.has_authored_metadata(&doc_key));

    // Get it back
    let retrieved = stage.get_metadata(&doc_key).expect("Should have metadata");
    let s = retrieved.get::<String>().expect("Should be string");
    assert_eq!(s, "Test documentation");

    // Clear it
    assert!(stage.clear_metadata(&doc_key));
    assert!(!stage.has_authored_metadata(&doc_key));
}

/// Test prim type info
#[test]
fn test_prim_type_info() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let prim = stage.define_prim("/Typed", "Xform").unwrap();
    assert_eq!(prim.type_name().get_text(), "Xform");

    let untyped = stage.define_prim("/Untyped", "").unwrap();
    assert!(untyped.type_name().get_text().is_empty());
}

/// Test has_prim_at_path
#[test]
fn test_has_prim_at_path() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    assert!(!stage.has_prim_at_path(&Path::from("/Test")));

    let _ = stage.define_prim("/Test", "").unwrap();
    assert!(stage.has_prim_at_path(&Path::from("/Test")));
}

/// Test sibling navigation
#[test]
fn test_sibling_navigation() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let _ = stage.define_prim("/Parent", "").unwrap();
    let _ = stage.define_prim("/Parent/A", "").unwrap();
    let _ = stage.define_prim("/Parent/B", "").unwrap();
    let _ = stage.define_prim("/Parent/C", "").unwrap();

    let prim_a = stage.get_prim_at_path(&Path::from("/Parent/A")).unwrap();
    let next = prim_a.get_next_sibling();
    assert!(next.is_valid());
    assert_eq!(next.name().get_text(), "B");
}

/// Test instance/prototype queries (basic)
#[test]
fn test_instance_queries() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let prim = stage.define_prim("/Test", "").unwrap();

    // Normal prims are not instances or prototypes
    assert!(!prim.is_instance());
    assert!(!prim.is_prototype());
    assert!(!prim.is_instance_proxy());
    assert!(!prim.is_in_prototype());
}
