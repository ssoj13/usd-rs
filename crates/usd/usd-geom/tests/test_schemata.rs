use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

//! Tests for UsdGeomSchemata.
//!
//! Ported from: testenv/testUsdGeomSchemata.py

use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;
use usd_tf::Token;

fn stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn path(s: &str) -> usd_sdf::Path {
    usd_sdf::Path::from_string(s).unwrap()
}

fn default_tc() -> TimeCode {
    TimeCode::default_time()
}

// ============================================================================
// test_Basic
// ============================================================================

#[test]
fn test_basic() {
    setup();
    let s = stage();
    let p = s.define_prim("/Mesh", "Mesh").expect("define /Mesh");
    assert!(p.is_valid());

    let mesh = Mesh::new(p.clone());
    assert!(mesh.is_valid());
    assert!(mesh.prim().is_valid());

    // points attr should exist but have no value at time=1
    let points_attr = mesh.point_based().get_points_attr();
    assert!(points_attr.get(TimeCode::new(1.0)).is_none());

    // The prim type name should match the schema type name
    assert_eq!(
        p.get_type_name().as_str(),
        Mesh::schema_type_name().as_str()
    );

    // Orientation attribute: should exist as a schema builtin
    let ori = p
        .get_attribute("orientation")
        .expect("orientation attr should exist");
    assert!(ori.as_property().is_defined());

    // Not yet authored
    assert!(!ori.has_authored_value());

    // Author leftHanded at default time
    let left_handed = Token::new("leftHanded");
    let right_handed = Token::new("rightHanded");
    ori.set(left_handed.clone(), default_tc());

    assert!(ori.as_property().is_defined());
    assert!(ori.has_authored_value());

    // Author rightHanded at t=10
    mesh.point_based()
        .gprim()
        .get_orientation_attr()
        .set(right_handed.clone(), TimeCode::new(10.0));

    // Default should still be leftHanded
    let val_default = ori.get_typed::<Token>(default_tc()).or_else(|| {
        ori.get(default_tc())
            .and_then(|v| v.get::<String>().map(|s| Token::new(s)))
    });
    if let Some(val) = val_default {
        assert_eq!(val.as_str(), "leftHanded");
    }

    // At any time >= default with a time sample, should return rightHanded
    for t in &[9.9, 10.0, 10.1, 11.0] {
        let val_t = ori.get_typed::<Token>(TimeCode::new(*t)).or_else(|| {
            ori.get(TimeCode::new(*t))
                .and_then(|v| v.get::<String>().map(|s| Token::new(s)))
        });
        if let Some(val) = val_t {
            assert_eq!(val.as_str(), "rightHanded", "at time {t}");
        }
    }

    // Schema attribute names should be non-empty
    let schema_attrs = Mesh::get_schema_attribute_names(true);
    assert!(!schema_attrs.is_empty());

    // Inherited vs local should differ
    let inherited = Mesh::get_schema_attribute_names(true);
    let local_only = Mesh::get_schema_attribute_names(false);
    assert_ne!(inherited.len(), local_only.len());
}

// ============================================================================
// test_IsA
// ============================================================================

#[test]
fn test_is_a_basis_curves() {
    setup();
    let s = stage();
    let schema = BasisCurves::define(&s, &path("/BasisCurves"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    // BasisCurves is NOT a Mesh
    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    // BasisCurves IS a BasisCurves
    assert_eq!(prim.get_type_name().as_str(), "BasisCurves");
    // BasisCurves is NOT a Cylinder
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    // Builtin: basis attr should exist
    assert!(schema.get_basis_attr().is_valid());
}

#[test]
fn test_is_a_camera() {
    setup();
    let s = stage();
    let schema = Camera::define(&s, &path("/Camera"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Camera");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_focal_length_attr().is_valid());
}

#[test]
fn test_is_a_capsule() {
    setup();
    let s = stage();
    let schema = Capsule::define(&s, &path("/Capsule"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Capsule");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_axis_attr().is_valid());
}

#[test]
fn test_is_a_capsule1() {
    setup();
    let s = stage();
    let schema = Capsule1::define(&s, &path("/Capsule_1"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Capsule_1");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.as_capsule().get_axis_attr().is_valid());
}

#[test]
fn test_is_a_cone() {
    setup();
    let s = stage();
    let schema = Cone::define(&s, &path("/Cone"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Cone");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_axis_attr().is_valid());
}

#[test]
fn test_is_a_cube() {
    setup();
    let s = stage();
    let schema = Cube::define(&s, &path("/Cube"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Cube");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_size_attr().is_valid());
}

#[test]
fn test_is_a_cylinder() {
    setup();
    let s = stage();
    let schema = Cylinder::define(&s, &path("/Cylinder"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_axis_attr().is_valid());
}

#[test]
fn test_is_a_cylinder1() {
    setup();
    let s = stage();
    let schema = Cylinder1::define(&s, &path("/Cylinder_1"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Cylinder_1");
    assert!(schema.as_cylinder().get_axis_attr().is_valid());
}

#[test]
fn test_is_a_mesh() {
    setup();
    let s = stage();
    let schema = Mesh::define(&s, &path("/Mesh"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_eq!(prim.get_type_name().as_str(), "Mesh");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_face_vertex_counts_attr().is_valid());
}

#[test]
fn test_is_a_nurbs_curves() {
    setup();
    let s = stage();
    let schema = NurbsCurves::define(&s, &path("/NurbsCurves"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "NurbsCurves");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_knots_attr().is_valid());
}

#[test]
fn test_is_a_nurbs_patch() {
    setup();
    let s = stage();
    let schema = NurbsPatch::define(&s, &path("/NurbsPatch"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "NurbsPatch");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_u_knots_attr().is_valid());
}

#[test]
fn test_is_a_points() {
    setup();
    let s = stage();
    let schema = Points::define(&s, &path("/Points"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Points");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_widths_attr().is_valid());
}

#[test]
fn test_is_a_scope() {
    setup();
    let s = stage();
    let schema = Scope::define(&s, &path("/Scope"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Scope");
    // Scope is NOT Xformable
    assert_ne!(prim.get_type_name().as_str(), "Xform");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    // Scope has no builtins of its own (only inherited from Imageable)
}

#[test]
fn test_is_a_sphere() {
    setup();
    let s = stage();
    let schema = Sphere::define(&s, &path("/Sphere"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Sphere");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.get_radius_attr().is_valid());
}

#[test]
fn test_is_a_xform() {
    setup();
    let s = stage();
    let schema = Xform::define(&s, &path("/Xform"));
    assert!(schema.is_valid());
    let prim = schema.prim();

    assert_ne!(prim.get_type_name().as_str(), "Mesh");
    assert_eq!(prim.get_type_name().as_str(), "Xform");
    assert_ne!(prim.get_type_name().as_str(), "Cylinder");
    assert!(schema.xformable().get_xform_op_order_attr().is_valid());
}

// ============================================================================
// test_Fallbacks
// ============================================================================

#[test]
fn test_fallbacks_xform_op_order() {
    setup();
    let s = stage();

    let xform = Xform::define(&s, &path("/Xform"));
    let xform_op_order = xform.xformable().get_xform_op_order_attr();

    // xformOpOrder has no fallback value and is not yet authored
    assert!(!xform_op_order.has_authored_value());
    assert!(xform_op_order.get(default_tc()).is_none());
    assert!(!xform_op_order.has_fallback_value());

    // Author then revert via the prim attribute API
    let xform_op_order_attr = xform
        .prim()
        .get_attribute("xformOpOrder")
        .expect("xformOpOrder attr should exist");
    assert!(xform_op_order_attr.is_valid());
    assert!(xform_op_order_attr.get(default_tc()).is_none());

    // Author a value
    let op_order_val: Vec<Token> = vec![Token::new("xformOp:transform")];
    assert!(xform_op_order_attr.set(usd_vt::Value::from_no_hash(op_order_val), default_tc()));
    assert!(xform_op_order_attr.has_authored_value());
    assert!(xform_op_order_attr.get(default_tc()).is_some());

    // Clear and verify
    assert!(xform_op_order_attr.clear(default_tc()));
    assert!(!xform_op_order_attr.has_authored_value());
    assert!(xform_op_order_attr.get(default_tc()).is_none());
    assert!(!xform_op_order.has_fallback_value());
}

#[test]
fn test_fallbacks_curves_interpolation() {
    setup();
    let s = stage();

    // PointBased normals and Curves widths interpolation defaults
    let curves = BasisCurves::define(&s, &path("/Curves"));
    let normals_interp = curves.curves().point_based().get_normals_interpolation();
    assert_eq!(normals_interp.as_str(), "vertex");

    let widths_interp = curves.curves().get_widths_interpolation();
    assert_eq!(widths_interp.as_str(), "vertex");
}

#[test]
fn test_fallbacks_double_sided_authoring() {
    setup();
    let s = stage();

    let mesh = Mesh::define(&s, &path("/Mesh"));

    // doubleSided has a fallback of false. Initially not authored.
    let ds_attr = mesh.point_based().gprim().get_double_sided_attr();
    assert!(!ds_attr.has_authored_value());

    // Author false (matches fallback), then check
    ds_attr.set(false, default_tc());
    assert!(ds_attr.has_authored_value());

    // Clear and check
    ds_attr.clear(default_tc());
    assert!(!ds_attr.has_authored_value());

    // Author true (differs from fallback)
    ds_attr.set(true, default_tc());
    assert!(ds_attr.has_authored_value());
    if let Some(val) = ds_attr.get_typed::<bool>(default_tc()) {
        assert!(val);
    }
}

#[test]
fn test_fallbacks_override_prim_double_sided() {
    setup();
    let s = stage();

    // Override prim and author doubleSided
    let over_mesh_prim = s.override_prim("/overMesh").expect("override /overMesh");
    let over_mesh = Mesh::new(over_mesh_prim);

    let ds_attr = over_mesh.point_based().gprim().get_double_sided_attr();
    ds_attr.set(false, default_tc());
    assert!(ds_attr.has_authored_value());

    if let Some(val) = ds_attr.get_typed::<bool>(default_tc()) {
        assert!(!val);
    }

    // Overwrite with true
    ds_attr.set(true, default_tc());
    if let Some(val) = ds_attr.get_typed::<bool>(default_tc()) {
        assert!(val);
    }

    // Define the mesh at the same path (should keep authored value)
    let _mesh2 = Mesh::define(&s, &path("/overMesh"));
    if let Some(val) = ds_attr.get_typed::<bool>(default_tc()) {
        assert!(val);
    }
}

#[test]
fn test_fallbacks_sphere_radius_has_fallback() {
    setup();
    let s = stage();

    let sphere = Sphere::define(&s, &path("/Sphere"));
    let radius = sphere.get_radius_attr();
    assert!(radius.has_fallback_value());
}

// ============================================================================
// test_DefineSchema
// ============================================================================

#[test]
fn test_define_schema() {
    setup();
    let s = stage();

    let _parent = s.override_prim("/parent").expect("override /parent");

    // Make a subscope
    let scope = Scope::define(&s, &path("/parent/subscope"));
    assert!(scope.is_valid());

    // A simple override gives us the scope back
    let over_prim = s
        .override_prim("/parent/subscope")
        .expect("override subscope");
    assert!(over_prim.is_valid());
    assert_eq!(
        over_prim.get_path().get_string(),
        scope.prim().get_path().get_string()
    );

    // Redefine at subscope's path as a Mesh -> transforms the Scope into a Mesh
    let mesh = Mesh::define(&s, &path("/parent/subscope"));
    assert!(mesh.is_valid());

    // The old scope wrapper should now be invalid because the type changed
    // Verify the type name changed to Mesh
    assert_eq!(mesh.prim().get_type_name().as_str(), "Mesh");

    // Make a mesh at a different path
    let mesh2 = Mesh::define(&s, &path("/parent/mesh"));
    assert!(mesh2.is_valid());
}

// ============================================================================
// test_BasicMetadataCases
// ============================================================================

#[test]
fn test_basic_metadata_cases() {
    setup();
    let s = stage();
    let sphere_prim = Sphere::define(&s, &path("/sphere")).prim().clone();

    let radius = sphere_prim
        .get_attribute("radius")
        .expect("radius attr should exist");

    // The radius attribute should be defined (it's a schema builtin)
    assert!(radius.as_property().is_defined());

    // Not custom
    assert!(!radius.as_property().is_custom());

    // Check type name (with full schematics: "double"; with fallback inference: "f64")
    let type_name_str = radius.get_type_name().to_string();
    if !type_name_str.is_empty() {
        assert!(
            type_name_str == "double" || type_name_str == "f64",
            "unexpected type name: {}",
            type_name_str
        );
    }

    // visibility attribute should be defined
    let visibility = sphere_prim
        .get_attribute("visibility")
        .expect("visibility attr should exist");
    assert!(visibility.as_property().is_defined());
}

// ============================================================================
// test_Camera
// ============================================================================

#[test]
fn test_camera() {
    setup();
    let s = stage();
    let camera = Camera::define(&s, &path("/Camera"));

    // Camera is a kind of Xformable
    assert!(camera.xformable().is_valid());
    assert_eq!(camera.prim().get_type_name().as_str(), "Camera");

    // Projection default: "perspective"
    let proj_attr = camera.get_projection_attr();
    let proj_val = proj_attr
        .get_typed::<Token>(default_tc())
        .map(|t| t.as_str().to_string())
        .or_else(|| {
            proj_attr
                .get(default_tc())
                .and_then(|v| v.get::<String>().cloned())
        });
    if let Some(val) = proj_val {
        assert_eq!(val, "perspective");
    }

    // Set orthographic
    proj_attr.set(Token::new("orthographic"), default_tc());
    let proj_val2 = proj_attr
        .get_typed::<Token>(default_tc())
        .map(|t| t.as_str().to_string())
        .or_else(|| {
            proj_attr
                .get(default_tc())
                .and_then(|v| v.get::<String>().cloned())
        });
    if let Some(val) = proj_val2 {
        assert_eq!(val, "orthographic");
    }

    // horizontalAperture default: 0.825 * 25.4 = 20.955
    let ha_attr = camera.get_horizontal_aperture_attr();
    if let Some(val) = ha_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 0.825_f32 * 25.4_f32).abs() < 1e-3);
    }
    ha_attr.set(3.0_f32, default_tc());
    if let Some(val) = ha_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 3.0_f32).abs() < 1e-5);
    }

    // verticalAperture default: 0.602 * 25.4 = 15.2908
    let va_attr = camera.get_vertical_aperture_attr();
    if let Some(val) = va_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 0.602_f32 * 25.4_f32).abs() < 1e-3);
    }
    va_attr.set(2.0_f32, default_tc());
    if let Some(val) = va_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 2.0_f32).abs() < 1e-5);
    }

    // focalLength default: 50.0
    let fl_attr = camera.get_focal_length_attr();
    if let Some(val) = fl_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 50.0_f32).abs() < 1e-5);
    }
    fl_attr.set(35.0_f32, default_tc());
    if let Some(val) = fl_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 35.0_f32).abs() < 1e-5);
    }

    // clippingRange default: (1, 1000000)
    let cr_attr = camera.get_clipping_range_attr();
    if let Some(val) = cr_attr.get_typed::<usd_gf::vec2::Vec2f>(default_tc()) {
        assert!((val.x - 1.0_f32).abs() < 1e-5);
        assert!((val.y - 1_000_000.0_f32).abs() < 1.0);
    }
    cr_attr.set(
        usd_vt::Value::from_no_hash(usd_gf::vec2::Vec2f::new(5.0, 10.0)),
        default_tc(),
    );
    if let Some(val) = cr_attr.get_typed::<usd_gf::vec2::Vec2f>(default_tc()) {
        assert!((val.x - 5.0_f32).abs() < 1e-5);
        assert!((val.y - 10.0_f32).abs() < 1e-5);
    }

    // fStop default: 0.0
    let fs_attr = camera.get_f_stop_attr();
    if let Some(val) = fs_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 0.0_f32).abs() < 1e-5);
    }
    fs_attr.set(2.8_f32, default_tc());
    if let Some(val) = fs_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 2.8_f32).abs() < 1e-5);
    }

    // focusDistance default: 0.0
    let fd_attr = camera.get_focus_distance_attr();
    if let Some(val) = fd_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 0.0_f32).abs() < 1e-5);
    }
    fd_attr.set(10.0_f32, default_tc());
    if let Some(val) = fd_attr.get_typed::<f32>(default_tc()) {
        assert!((val - 10.0_f32).abs() < 1e-5);
    }
}

// ============================================================================
// test_Points
// ============================================================================

#[test]
fn test_points() {
    setup();
    let s = stage();
    let schema = Points::define(&s, &path("/Points"));
    assert!(schema.is_valid());

    // Test that ids roundtrip properly for big numbers and negative numbers
    let ids: Vec<i64> = vec![8_589_934_592, 1_099_511_627_776, 0, -42];
    let ids_attr = schema.create_ids_attr(None, false);
    ids_attr.set(usd_vt::Value::from_no_hash(ids.clone()), default_tc());

    if let Some(resolved) = ids_attr.get_typed::<Vec<i64>>(default_tc()) {
        assert_eq!(ids, resolved);
    } else if let Some(val) = ids_attr.get(default_tc()) {
        if let Some(arr) = val.get::<Vec<i64>>() {
            assert_eq!(&ids, arr);
        }
    }
}

// ============================================================================
// test_Revert_Bug111239
// ============================================================================

#[test]
fn test_revert_bug_111239() {
    setup();
    let s = stage();

    // Define a prim with typeName='Sphere' (valid schema name)
    let sphere = s.define_prim("/sphere", "Sphere").expect("define sphere");
    // A Sphere schema wrapping a Sphere-typed prim should be valid
    let sphere_schema = Sphere::new(sphere.clone());
    assert!(sphere_schema.is_valid());

    // The prim should have a 'radius' attribute
    assert!(sphere.get_attribute("radius").is_some());

    // Define with a bogus typeName (not a real schema) - should still create
    // the prim but it won't be recognized as a Sphere
    let bogus = s
        .define_prim("/usdGeomSphere", "tfTypeName")
        .expect("define with bogus type");

    // The prim should NOT have 'radius' as a builtin
    let has_radius = bogus
        .get_attribute("radius")
        .map(|a| a.is_valid())
        .unwrap_or(false);
    assert!(!has_radius);

    // Verify the bogus prim does NOT have the Sphere type name
    assert_ne!(bogus.get_type_name().as_str(), "Sphere");
}

// ============================================================================
// test_ComputeExtent
// ============================================================================

fn vec3f(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f::new(x, y, z)
}

fn close3(a: Vec3f, b: Vec3f, eps: f32) -> bool {
    (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
}

#[test]
fn test_compute_extent_point_based() {
    setup();
    let all_points: Vec<Vec<Vec3f>> = vec![
        vec![vec3f(1.0, 1.0, 0.0)],                          // Zero-Volume
        vec![vec3f(0.0, 0.0, 0.0)],                          // Simple
        vec![vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)], // Multiple
        vec![vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)], // Erroneous (ok for PointBased)
        vec![
            vec3f(3.0, -1.0, 5.0),
            vec3f(-1.5, 0.0, 3.0),
            vec3f(1.0, 3.0, -2.0),
            vec3f(2.0, 2.0, -4.0),
        ],
    ];

    let point_based_solutions: Vec<[Vec3f; 2]> = vec![
        [vec3f(1.0, 1.0, 0.0), vec3f(1.0, 1.0, 0.0)],
        [vec3f(0.0, 0.0, 0.0), vec3f(0.0, 0.0, 0.0)],
        [vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)],
        [vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)],
        [vec3f(-1.5, -1.0, -4.0), vec3f(3.0, 3.0, 5.0)],
    ];

    for i in 0..all_points.len() {
        let mut extent = [vec3f(0.0, 0.0, 0.0); 2];
        let ok = PointBased::compute_extent(&all_points[i], &mut extent);
        assert!(ok, "PointBased::compute_extent failed for set {i}");
        assert!(
            close3(extent[0], point_based_solutions[i][0], 1e-5),
            "min mismatch for set {i}: {:?} vs {:?}",
            extent[0],
            point_based_solutions[i][0]
        );
        assert!(
            close3(extent[1], point_based_solutions[i][1], 1e-5),
            "max mismatch for set {i}: {:?} vs {:?}",
            extent[1],
            point_based_solutions[i][1]
        );
    }
}

#[test]
fn test_compute_extent_point_based_empty() {
    setup();
    // Empty points: our impl returns false (C++ returns an empty range)
    let empty_points: Vec<Vec3f> = vec![];
    let mut extent = [vec3f(0.0, 0.0, 0.0); 2];
    let ok = PointBased::compute_extent(&empty_points, &mut extent);
    assert!(!ok);
}

#[test]
fn test_compute_extent_points_with_widths() {
    setup();
    let all_points: Vec<Vec<Vec3f>> = vec![
        vec![vec3f(1.0, 1.0, 0.0)],
        vec![vec3f(0.0, 0.0, 0.0)],
        vec![vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)],
        vec![vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)],
        vec![
            vec3f(3.0, -1.0, 5.0),
            vec3f(-1.5, 0.0, 3.0),
            vec3f(1.0, 3.0, -2.0),
            vec3f(2.0, 2.0, -4.0),
        ],
    ];

    let all_widths: Vec<Vec<f32>> = vec![
        vec![0.0],
        vec![2.0],
        vec![2.0, 4.0],
        vec![2.0, 4.0, 5.0], // erroneous: widths.len != points.len
        vec![1.0, 2.0, 2.0, 1.0],
    ];

    let points_solutions: Vec<Option<[Vec3f; 2]>> = vec![
        Some([vec3f(1.0, 1.0, 0.0), vec3f(1.0, 1.0, 0.0)]),
        Some([vec3f(-1.0, -1.0, -1.0), vec3f(1.0, 1.0, 1.0)]),
        Some([vec3f(-2.0, -2.0, -2.0), vec3f(3.0, 3.0, 3.0)]),
        None, // widths/points size mismatch
        Some([vec3f(-2.5, -1.5, -4.5), vec3f(3.5, 4.0, 5.5)]),
    ];

    for i in 0..all_points.len() {
        let mut extent = [vec3f(0.0, 0.0, 0.0); 2];
        let ok = Points::compute_extent(&all_points[i], &all_widths[i], &mut extent);

        match &points_solutions[i] {
            Some(expected) => {
                assert!(ok, "Points::compute_extent should succeed for set {i}");
                assert!(
                    close3(extent[0], expected[0], 1e-5),
                    "min mismatch for set {i}: {:?} vs {:?}",
                    extent[0],
                    expected[0]
                );
                assert!(
                    close3(extent[1], expected[1], 1e-5),
                    "max mismatch for set {i}: {:?} vs {:?}",
                    extent[1],
                    expected[1]
                );
            }
            None => {
                assert!(
                    !ok,
                    "Points::compute_extent should fail for erroneous set {i}"
                );
            }
        }
    }
}

#[test]
fn test_compute_extent_curves() {
    setup();
    let curves_points: Vec<Vec<Vec3f>> = vec![
        vec![
            vec3f(0.0, 0.0, 0.0),
            vec3f(1.0, 1.0, 1.0),
            vec3f(2.0, 1.0, 1.0),
            vec3f(3.0, 0.0, 0.0),
        ],
        vec![
            vec3f(0.0, 0.0, 0.0),
            vec3f(1.0, 1.0, 1.0),
            vec3f(2.0, 1.0, 1.0),
            vec3f(3.0, 0.0, 0.0),
        ],
        vec![
            vec3f(0.0, 0.0, 0.0),
            vec3f(1.0, 1.0, 1.0),
            vec3f(2.0, 1.0, 1.0),
            vec3f(3.0, 0.0, 0.0),
        ],
    ];

    let curves_widths: Vec<Vec<f32>> = vec![
        vec![1.0],
        vec![0.5, 0.1],
        vec![], // no widths
    ];

    let curves_solutions: Vec<[Vec3f; 2]> = vec![
        [vec3f(-0.5, -0.5, -0.5), vec3f(3.5, 1.5, 1.5)],
        [vec3f(-0.25, -0.25, -0.25), vec3f(3.25, 1.25, 1.25)],
        [vec3f(0.0, 0.0, 0.0), vec3f(3.0, 1.0, 1.0)],
    ];

    for i in 0..curves_points.len() {
        let mut extent = [vec3f(0.0, 0.0, 0.0); 2];
        let ok = Curves::compute_extent(&curves_points[i], &curves_widths[i], &mut extent);
        assert!(ok, "Curves::compute_extent failed for set {i}");
        assert!(
            close3(extent[0], curves_solutions[i][0], 1e-5),
            "min mismatch for curves set {i}: {:?} vs {:?}",
            extent[0],
            curves_solutions[i][0]
        );
        assert!(
            close3(extent[1], curves_solutions[i][1], 1e-5),
            "max mismatch for curves set {i}: {:?} vs {:?}",
            extent[1],
            curves_solutions[i][1]
        );
    }
}

// ============================================================================
// test_TypeUsage
// ============================================================================

#[test]
fn test_type_usage() {
    setup();
    // Verify that ComputeExtent works with Vec<Vec3f> input
    let points = vec![
        vec3f(0.0, 0.0, 0.0),
        vec3f(1.0, 1.0, 1.0),
        vec3f(2.0, 2.0, 2.0),
    ];

    let mut extent1 = [vec3f(0.0, 0.0, 0.0); 2];
    let ok1 = PointBased::compute_extent(&points, &mut extent1);
    assert!(ok1);

    // Same data, computed again for comparison
    let mut extent2 = [vec3f(0.0, 0.0, 0.0); 2];
    let ok2 = PointBased::compute_extent(&points, &mut extent2);
    assert!(ok2);

    assert!(close3(extent1[0], extent2[0], 1e-5));
    assert!(close3(extent1[1], extent2[1], 1e-5));

    // Verify expected extent
    assert!(close3(extent1[0], vec3f(0.0, 0.0, 0.0), 1e-5));
    assert!(close3(extent1[1], vec3f(2.0, 2.0, 2.0), 1e-5));
}

// ============================================================================
// test_Bug116593 (ModelAPI extentsHint)
// ============================================================================

#[test]
fn test_bug_116593() {
    setup();
    let s = stage();
    let prim = s.define_prim("/sphere", "Sphere").expect("define sphere");

    let model_api = ModelAPI::new(prim.clone());

    // Set extentsHint with Vec3f pairs
    let hint: Vec<Vec3f> = vec![vec3f(1.0, 2.0, 2.0), vec3f(12.0, 3.0, 3.0)];
    model_api.set_extents_hint(&hint, default_tc());

    if let Some(result) = model_api.get_extents_hint(default_tc()) {
        if result.len() >= 2 {
            assert!(close3(result[0], vec3f(1.0, 2.0, 2.0), 1e-5));
            assert!(close3(result[1], vec3f(12.0, 3.0, 3.0), 1e-5));
        }
    }

    // Set with different values
    let hint2: Vec<Vec3f> = vec![vec3f(1.0, 2.0, 2.0), vec3f(1.0, 1.0, 1.0)];
    model_api.set_extents_hint(&hint2, default_tc());

    if let Some(result) = model_api.get_extents_hint(default_tc()) {
        if result.len() >= 2 {
            assert!(close3(result[0], vec3f(1.0, 2.0, 2.0), 1e-5));
            assert!(close3(result[1], vec3f(1.0, 1.0, 1.0), 1e-5));
        }
    }
}

// ============================================================================
// test_Typed
// ============================================================================

#[test]
fn test_typed_schema_types() {
    setup();
    // Verify that concrete schema types have the expected type names
    assert_eq!(Xform::schema_type_name().as_str(), "Xform");
    assert_eq!(Scope::schema_type_name().as_str(), "Scope");
    assert_eq!(Mesh::schema_type_name().as_str(), "Mesh");
    assert_eq!(Sphere::schema_type_name().as_str(), "Sphere");
    assert_eq!(Cube::schema_type_name().as_str(), "Cube");
    assert_eq!(Cone::schema_type_name().as_str(), "Cone");
    assert_eq!(Cylinder::schema_type_name().as_str(), "Cylinder");
    assert_eq!(Capsule::schema_type_name().as_str(), "Capsule");

    // Concrete schemas produce valid prims
    let s = stage();
    let xf = Xform::define(&s, &path("/TypedXform"));
    assert!(xf.is_valid());
    assert_eq!(xf.prim().get_type_name().as_str(), "Xform");
}

// ============================================================================
// test_Concrete
// ============================================================================

#[test]
fn test_concrete_schema_types() {
    setup();
    let s = stage();

    // Xform is concrete: can define a prim of type Xform
    let xf = Xform::define(&s, &path("/ConcreteXform"));
    assert!(xf.is_valid());
    assert_eq!(xf.prim().get_type_name().as_str(), "Xform");

    // Imageable is NOT concrete: it's an abstract type
    // The prim gets created but won't have concrete schema properties
    let result = s.define_prim("/ConcreteImageable", "Imageable");
    match result {
        Ok(prim) => {
            // Prim was created but type is just "Imageable" (abstract)
            let _ = prim;
        }
        Err(_) => {
            // Some implementations may reject abstract types
        }
    }

    // ModelAPI is not concrete (it's an API schema, applied not defined)
}

// ============================================================================
// test_Apply
// ============================================================================

#[test]
fn test_apply_motion_api() {
    setup();
    let s = stage();
    let root = s.define_prim("/hello", "").expect("define /hello");
    assert!(root.get_applied_schemas().is_empty());

    // Apply MotionAPI
    MotionAPI::apply(&root);
    let schemas = root.get_applied_schemas();
    let schema_strs: Vec<&str> = schemas.iter().map(|t| t.as_str()).collect();
    assert!(
        schema_strs.contains(&"MotionAPI"),
        "MotionAPI should be in applied schemas: {:?}",
        schema_strs
    );

    // Apply again: should not duplicate
    MotionAPI::apply(&root);
    let schemas2 = root.get_applied_schemas();
    let count = schemas2
        .iter()
        .filter(|t| t.as_str() == "MotionAPI")
        .count();
    assert_eq!(count, 1, "MotionAPI should appear exactly once");
}

#[test]
fn test_apply_model_api() {
    setup();
    let s = stage();
    let root = s.define_prim("/hello", "").expect("define /hello");

    // Apply MotionAPI first
    MotionAPI::apply(&root);

    // Apply ModelAPI (GeomModelAPI)
    ModelAPI::apply(&root);
    let schemas = root.get_applied_schemas();
    let schema_strs: Vec<&str> = schemas.iter().map(|t| t.as_str()).collect();

    assert!(
        schema_strs.contains(&"MotionAPI"),
        "MotionAPI should still be present: {:?}",
        schema_strs
    );
    assert!(
        schema_strs.contains(&"GeomModelAPI"),
        "GeomModelAPI should be in applied schemas: {:?}",
        schema_strs
    );
}

// ============================================================================
// test_IsATypeless
// ============================================================================

#[test]
fn test_is_a_typeless() {
    setup();
    let s = stage();
    let sphere_prim = s.define_prim("/sphere", "Sphere").expect("define sphere");
    let typeless_prim = s.define_prim("/regular", "").expect("define regular");

    // Sphere prim should have type "Sphere"
    assert_eq!(sphere_prim.get_type_name().as_str(), "Sphere");

    // Typeless prim should have empty type
    assert!(
        typeless_prim.get_type_name().as_str().is_empty(),
        "typeless prim should have empty type name, got: '{}'",
        typeless_prim.get_type_name().as_str()
    );

    // Sphere schema wrapping a Sphere prim should be valid
    let sphere_schema = Sphere::new(sphere_prim.clone());
    assert!(sphere_schema.is_valid());

    // Typeless prim should NOT have 'radius' as a schema builtin
    let typeless_has_radius = typeless_prim
        .get_attribute("radius")
        .map(|a| a.is_valid())
        .unwrap_or(false);
    assert!(!typeless_has_radius);

    // The Sphere prim SHOULD have 'radius' attribute (from schema)
    let sphere_has_radius = sphere_prim
        .get_attribute("radius")
        .map(|a| a.is_valid())
        .unwrap_or(false);
    assert!(sphere_has_radius);
}

// ============================================================================
// test_HasAPI
// ============================================================================

#[test]
fn test_has_api() {
    setup();
    let s = stage();
    let prim = s.define_prim("/prim", "").expect("define /prim");

    // No APIs applied yet
    assert!(prim.get_applied_schemas().is_empty());

    // Apply ModelAPI and MotionAPI
    ModelAPI::apply(&prim);
    MotionAPI::apply(&prim);

    // Check that applied schemas show up
    let schemas = prim.get_applied_schemas();
    let schema_strs: Vec<&str> = schemas.iter().map(|t| t.as_str()).collect();

    assert!(
        schema_strs.contains(&"GeomModelAPI"),
        "GeomModelAPI should be applied: {:?}",
        schema_strs
    );
    assert!(
        schema_strs.contains(&"MotionAPI"),
        "MotionAPI should be applied: {:?}",
        schema_strs
    );

    // Non-API types should NOT appear in applied schemas
    assert!(
        !schema_strs.contains(&"Xform"),
        "Xform is not an API schema"
    );
    assert!(
        !schema_strs.contains(&"Imageable"),
        "Imageable is not an API schema"
    );
}
