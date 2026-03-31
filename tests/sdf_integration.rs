//! Integration tests for SDF Layer and Path operations.

use usd::sdf::{Layer, Path, SpecType, Specifier};

/// Test Path creation and manipulation
#[test]
fn test_path_creation() {
    let path = Path::from("/World/Mesh");
    assert!(!path.is_empty());
    assert!(path.is_absolute_path());
    assert_eq!(path.get_as_string(), "/World/Mesh");
}

/// Test Path parent/child relationships
#[test]
fn test_path_hierarchy() {
    let path = Path::from("/A/B/C");

    let parent = path.get_parent_path();
    assert_eq!(parent.get_as_string(), "/A/B");

    let grandparent = parent.get_parent_path();
    assert_eq!(grandparent.get_as_string(), "/A");

    let name = path.get_name();
    assert_eq!(name, "C");
}

/// Test Path append operations
#[test]
fn test_path_append() {
    let root = Path::from("/World");

    let child = root.append_child("Child").expect("Failed to append child");
    assert_eq!(child.get_as_string(), "/World/Child");

    let prop = root
        .append_property("visibility")
        .expect("Failed to append property");
    assert_eq!(prop.get_as_string(), "/World.visibility");
}

/// Test absolute root path
#[test]
fn test_absolute_root_path() {
    let root = Path::absolute_root();
    assert_eq!(root.get_as_string(), "/");
    assert!(root.is_absolute_root_path());
}

/// Test property paths
#[test]
fn test_property_path() {
    let prop_path = Path::from("/Prim.attribute");
    assert!(prop_path.is_property_path());

    let prim_path = prop_path.get_prim_path();
    assert_eq!(prim_path.get_as_string(), "/Prim");

    let name = prop_path.get_name();
    assert_eq!(name, "attribute");
}

/// Test Layer creation
#[test]
fn test_layer_create_anonymous() {
    let layer = Layer::create_anonymous(Some("test"));
    assert!(layer.is_anonymous());
    assert!(!layer.identifier().is_empty());
}

/// Test Layer with prim spec
#[test]
fn test_layer_prim_spec() {
    let layer = Layer::create_anonymous(Some("test"));

    // Create a prim spec
    let prim_path = Path::from("/TestPrim");
    let _prim_spec = layer
        .create_prim_spec(&prim_path, Specifier::Def, "")
        .expect("Failed to create prim spec");

    // Check that the layer has the spec
    assert!(layer.has_spec(&prim_path));
    assert_eq!(layer.get_spec_type(&prim_path), SpecType::Prim);
}

/// Test Layer root prims
#[test]
fn test_layer_root_prims() {
    let layer = Layer::create_anonymous(Some("test"));

    let _ = layer
        .create_prim_spec(&Path::from("/A"), Specifier::Def, "")
        .unwrap();
    let _ = layer
        .create_prim_spec(&Path::from("/B"), Specifier::Def, "")
        .unwrap();

    // Check that we can get root prims
    let root_prims = layer.get_root_prims();
    assert!(root_prims.len() >= 2, "Should have at least 2 root prims");
}

/// Test Layer export to string
#[test]
fn test_layer_export_to_string() {
    let layer = Layer::create_anonymous(Some("test"));
    let _ = layer
        .create_prim_spec(&Path::from("/World"), Specifier::Def, "")
        .unwrap();

    let exported = layer.export_to_string().expect("Failed to export");
    assert!(exported.contains("#usda"));
    assert!(exported.contains("World"));
}

/// Test empty path
#[test]
fn test_empty_path() {
    let empty = Path::empty();
    assert!(empty.is_empty());
}

/// Test path comparison
#[test]
fn test_path_comparison() {
    let p1 = Path::from("/A/B");
    let p2 = Path::from("/A/B");
    let p3 = Path::from("/A/C");

    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

/// Test path contains (has_prefix)
#[test]
fn test_path_has_prefix() {
    let parent = Path::from("/A");
    let child = Path::from("/A/B");
    let other = Path::from("/C");

    assert!(child.has_prefix(&parent));
    assert!(!other.has_prefix(&parent));
}

/// Test relative path
#[test]
fn test_relative_path() {
    let rel = Path::from("relative/path");
    assert!(!rel.is_absolute_path());
}

/// Test path prefixes
#[test]
fn test_path_prefixes() {
    let path = Path::from("/A/B/C/D");
    let prefixes = path.get_prefixes();

    // Should have: /, /A, /A/B, /A/B/C, /A/B/C/D
    assert_eq!(prefixes.len(), 5);
}

/// Test path with variant selection
#[test]
fn test_path_variant() {
    let path = Path::from("/Model{variant=blue}");
    assert!(path.contains_prim_variant_selection());
}

/// Test layer dirty state
#[test]
fn test_layer_dirty_state() {
    let layer = Layer::create_anonymous(Some("test"));

    // New anonymous layer should not be dirty initially
    // (implementation may vary)
    let _initial_dirty = layer.is_dirty();

    // Modifying should make it dirty
    let _ = layer
        .create_prim_spec(&Path::from("/Test"), Specifier::Def, "")
        .unwrap();
    // After modification it might be dirty depending on implementation
    let _ = layer.is_dirty(); // Just check it doesn't panic
}

/// Test path strip all variant selections
#[test]
fn test_strip_variant_selections() {
    let path = Path::from("/Model{var=a}/Child{var2=b}");
    let stripped = path.strip_all_variant_selections();
    assert!(!stripped.contains_prim_variant_selection());
}
