//! Port of testUsdGeomNoPlugLoad.py
//!
//! Tests that geom schema type names work for prim definition without loading
//! plugins. The original C++ test relies on generatedSchema.usda for builtin
//! property discovery and fallback values. Our Rust implementation tests:
//! - Type name recognition via define_prim
//! - IsA type hierarchy checks
//! - Schema API attribute creation
//! - Attribute access after authoring

use usd_core::{InitialLoadSet, Stage};
use usd_tf::Token;

// ============================================================================
// test_scope_type_name
// ============================================================================

#[test]
fn test_scope_type_name() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    // Define a Scope prim by type name string
    let scope = stage
        .define_prim("/scope", "Scope")
        .expect("Failed to define Scope prim");

    assert!(scope.is_valid(), "Scope prim should be valid");
    assert_eq!(scope.type_name().as_str(), "Scope");

    // IsA(Typed) should work via the type hierarchy
    assert!(scope.is_a(&Token::new("Typed")), "Scope should IsA Typed");
}

// ============================================================================
// test_cube_type_and_schema_attrs
// ============================================================================

#[test]
fn test_cube_type_and_schema_attrs() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let cube_prim = stage
        .define_prim("/cube", "Cube")
        .expect("Failed to define Cube prim");

    assert!(cube_prim.is_valid(), "Cube prim should be valid");
    assert_eq!(cube_prim.type_name().as_str(), "Cube");

    // Cube should be identifiable as Typed
    assert!(
        cube_prim.is_a(&Token::new("Typed")),
        "Cube should IsA Typed"
    );

    // Access the "size" attribute via the typed Cube schema
    let cube = usd_geom::Cube::new(cube_prim.clone());
    assert!(cube.is_valid(), "Cube schema should be valid");

    // Before creating, authored property names should be empty (or not contain "size")
    let authored_before: Vec<String> = cube_prim
        .get_authored_property_names()
        .iter()
        .map(|t| t.as_str().to_string())
        .collect();
    assert!(
        !authored_before.contains(&"size".to_string()),
        "size should NOT be in authored property names before create"
    );

    // Create the size attr via schema API
    let size_attr = cube.create_size_attr(None, false);
    assert!(
        size_attr.is_valid(),
        "size attr should be valid after create"
    );

    // After creating, the attribute should be accessible from the prim
    let fetched_attr = cube_prim
        .get_attribute("size")
        .expect("size attribute should exist after create");
    assert!(fetched_attr.is_valid());

    // Now it IS authored (create_size_attr authored the spec)
    let authored_after: Vec<String> = cube_prim
        .get_authored_property_names()
        .iter()
        .map(|t| t.as_str().to_string())
        .collect();
    assert!(
        authored_after.contains(&"size".to_string()),
        "size should be in authored property names after create: {authored_after:?}"
    );
}

// ============================================================================
// test_cube_size_roundtrip
// ============================================================================

#[test]
fn test_cube_size_roundtrip() {
    // Test that we can set and get the size attribute via the Cube schema
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let cube_prim = stage
        .define_prim("/cube", "Cube")
        .expect("Failed to define Cube prim");

    let cube = usd_geom::Cube::new(cube_prim);
    assert!(cube.is_valid());

    // Set size = 4.0 via schema API
    let size_attr = cube.create_size_attr(None, false);
    size_attr.set(
        usd_vt::Value::from(4.0_f64),
        usd_sdf::TimeCode::default_time(),
    );

    // Read it back
    let size = cube.get_size(usd_sdf::TimeCode::default_time());
    assert_eq!(
        size,
        Some(4.0),
        "Cube::get_size should return authored value 4.0"
    );

    // has_authored_value should be true
    assert!(
        size_attr.has_authored_value(),
        "size should be authored after set"
    );
}

// ============================================================================
// test_multiple_schema_type_names
// ============================================================================

#[test]
fn test_multiple_schema_type_names() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    // All standard UsdGeom schema type names should be recognized
    let type_names = [
        ("Scope", "/test_scope"),
        ("Xform", "/test_xform"),
        ("Mesh", "/test_mesh"),
        ("Sphere", "/test_sphere"),
        ("Cube", "/test_cube"),
        ("Cylinder", "/test_cylinder"),
        ("Cone", "/test_cone"),
        ("Capsule", "/test_capsule"),
        ("Camera", "/test_camera"),
        ("BasisCurves", "/test_basis_curves"),
        ("Points", "/test_points"),
        ("PointInstancer", "/test_point_instancer"),
        ("NurbsCurves", "/test_nurbs_curves"),
        ("NurbsPatch", "/test_nurbs_patch"),
    ];

    for (type_name, path) in &type_names {
        let prim = stage
            .define_prim(path.to_string(), type_name.to_string())
            .unwrap_or_else(|_| panic!("Failed to define {type_name} at {path}"));
        assert!(
            prim.is_valid(),
            "{type_name} prim at {path} should be valid"
        );
        assert_eq!(
            prim.type_name().as_str(),
            *type_name,
            "type name should match for {path}"
        );
        // All geom types should IsA Typed
        assert!(
            prim.is_a(&Token::new("Typed")),
            "{type_name} should IsA Typed"
        );
    }
}
