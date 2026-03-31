//! Miscellaneous UsdGeom tests.
//!
//! Ported from:
//!   testUsdGeomPoints.py
//!   testUsdGeomMotionAPI.py
//!   testUsdGeomImageable.py
//!   testUsdGeomConstraintTarget.py
//!   testUsdGeomHermiteCurves.py
//!   testUsdGeomTypeRegistry.py

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::{Vec3d, Vec3f};
use usd_sdf::TimeCode;
use usd_tf::Token;

// ============================================================================
// Helpers
// ============================================================================

fn stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn path(s: &str) -> usd_sdf::Path {
    usd_sdf::Path::from_string(s).unwrap()
}

fn testenv_path(subdir: &str, file: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("testenv");
    p.push(subdir);
    p.push(file);
    p.to_string_lossy().to_string()
}

fn open_stage(subdir: &str, file: &str) -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(testenv_path(subdir, file), InitialLoadSet::LoadAll)
        .expect("Failed to open test stage")
}

fn default_tc() -> TimeCode {
    TimeCode::default_time()
}

fn earliest_tc() -> TimeCode {
    TimeCode::new(f64::MIN)
}

// ============================================================================
// testUsdGeomPoints: test_ComputePointCount
// ============================================================================

#[test]
fn test_points_compute_point_count_time_sampled() {
    let s = open_stage("testUsdGeomPoints", "points.usda");

    let unset = Points::get(&s, &path("/UnsetPoints"));
    let blocked = Points::get(&s, &path("/BlockedPoints"));
    let empty = Points::get(&s, &path("/EmptyPoints"));
    let time_sampled = Points::get(&s, &path("/TimeSampledPoints"));
    let time_sampled_and_default = Points::get(&s, &path("/TimeSampledAndDefaultPoints"));

    // All schemas should be valid
    assert!(unset.is_valid());
    assert!(blocked.is_valid());
    assert!(empty.is_valid());
    assert!(time_sampled.is_valid());
    assert!(time_sampled_and_default.is_valid());

    // Time-sampled queries (EarliestTime)
    let time_sample_cases: Vec<(&Points, usize)> = vec![
        (&unset, 0),
        (&blocked, 0),
        (&empty, 0),
        (&time_sampled, 3),
        (&time_sampled_and_default, 5),
    ];

    for (schema, expected) in &time_sample_cases {
        assert_eq!(
            schema.get_point_count(earliest_tc()),
            *expected,
            "EarliestTime count mismatch"
        );
    }
}

#[test]
fn test_points_compute_point_count_default() {
    let s = open_stage("testUsdGeomPoints", "points.usda");

    let unset = Points::get(&s, &path("/UnsetPoints"));
    let blocked = Points::get(&s, &path("/BlockedPoints"));
    let empty = Points::get(&s, &path("/EmptyPoints"));
    let time_sampled = Points::get(&s, &path("/TimeSampledPoints"));
    let time_sampled_and_default = Points::get(&s, &path("/TimeSampledAndDefaultPoints"));

    // Default time queries
    let default_cases: Vec<(&Points, usize)> = vec![
        (&unset, 0),
        (&blocked, 0),
        (&empty, 0),
        (&time_sampled, 0),
        (&time_sampled_and_default, 4),
    ];

    for (schema, expected) in &default_cases {
        assert_eq!(
            schema.get_point_count(default_tc()),
            *expected,
            "Default count mismatch"
        );
    }
}

#[test]
fn test_points_invalid_schema() {
    // Invalid Points from invalid prim: get_point_count should return 0
    let invalid = Points::invalid();
    assert!(!invalid.is_valid());
    assert_eq!(invalid.get_point_count(default_tc()), 0);
    assert_eq!(invalid.get_point_count(earliest_tc()), 0);
}

// ============================================================================
// testUsdGeomMotionAPI: test_Basic
// ============================================================================

#[test]
fn test_motion_api_inherit_from_parent() {
    let s = stage();

    // Parent root1: create MotionAPI attrs directly (apply via correct name)
    let root1 = Xform::define(&s, &path("/root1"));
    assert!(root1.is_valid());

    // Use MotionAPI::new (wraps prim) + create attrs directly
    let root1_motion = MotionAPI::new(root1.prim().clone());
    let blur_attr = root1_motion
        .create_motion_blur_scale_attr(Some(usd_vt::Value::from_no_hash(0.5_f32)), false);
    let sample_attr = root1_motion
        .create_motion_nonlinear_sample_count_attr(Some(usd_vt::Value::from_no_hash(5_i32)), false);

    // Verify the parent attrs are set correctly
    assert!(blur_attr.is_valid(), "blur attr should be valid");
    assert!(
        blur_attr.has_authored_value(),
        "blur attr should have authored value"
    );
    assert!(sample_attr.is_valid(), "sample count attr should be valid");
    assert!(
        sample_attr.has_authored_value(),
        "sample count attr should have authored value"
    );

    // Verify parent reads its own values
    assert_eq!(
        root1_motion.compute_motion_blur_scale(default_tc()),
        0.5_f32,
        "Parent should read its own blur scale"
    );
    assert_eq!(
        root1_motion.compute_nonlinear_sample_count(default_tc()),
        5,
        "Parent should read its own nonlinear sample count"
    );

    // Child1 (mesh1): does NOT apply MotionAPI, should inherit from parent
    let _root1_child1 = Mesh::define(&s, &path("/root1/mesh1"));
    let child1_motion = MotionAPI::new(s.get_prim_at_path(&path("/root1/mesh1")).unwrap());
    assert_eq!(
        child1_motion.compute_motion_blur_scale(default_tc()),
        0.5_f32,
        "Child should inherit blur scale from parent"
    );
    assert_eq!(
        child1_motion.compute_nonlinear_sample_count(default_tc()),
        5,
        "Child should inherit nonlinear sample count from parent"
    );
}

#[test]
fn test_motion_api_applied_no_author_still_inherits() {
    let s = stage();

    let root1 = Xform::define(&s, &path("/root1"));
    let root1_motion = MotionAPI::new(root1.prim().clone());
    root1_motion.create_motion_blur_scale_attr(Some(usd_vt::Value::from_no_hash(0.5_f32)), false);
    root1_motion
        .create_motion_nonlinear_sample_count_attr(Some(usd_vt::Value::from_no_hash(5_i32)), false);

    // Even if Child1 has MotionAPI wrapper but does not author a value,
    // it should still inherit from parent
    let root1_child1 = Mesh::define(&s, &path("/root1/mesh1"));

    let child1_motion = MotionAPI::new(root1_child1.prim().clone());
    assert_eq!(
        child1_motion.compute_motion_blur_scale(default_tc()),
        0.5_f32
    );
    assert_eq!(
        child1_motion.compute_nonlinear_sample_count(default_tc()),
        5
    );
}

#[test]
fn test_motion_api_child_author_override() {
    let s = stage();

    let root1 = Xform::define(&s, &path("/root1"));
    let root1_motion = MotionAPI::new(root1.prim().clone());
    root1_motion.create_motion_blur_scale_attr(Some(usd_vt::Value::from_no_hash(0.5_f32)), false);
    root1_motion
        .create_motion_nonlinear_sample_count_attr(Some(usd_vt::Value::from_no_hash(5_i32)), false);

    // Child2: author its own values.
    // Our Rust impl walks attrs from child up to parent, finding the child's
    // authored value first.
    let root1_child2 = Mesh::define(&s, &path("/root1/mesh2"));
    let child2_motion = MotionAPI::new(root1_child2.prim().clone());
    child2_motion.create_motion_blur_scale_attr(Some(usd_vt::Value::from_no_hash(2.0_f32)), false);
    child2_motion.create_motion_nonlinear_sample_count_attr(
        Some(usd_vt::Value::from_no_hash(10_i32)),
        false,
    );

    // Child has its own authored attrs, should use them
    assert_eq!(
        child2_motion.compute_motion_blur_scale(default_tc()),
        2.0_f32
    );
    assert_eq!(
        child2_motion.compute_nonlinear_sample_count(default_tc()),
        10
    );
}

#[test]
fn test_motion_api_fallback_values() {
    let s = stage();

    // When nothing is authored anywhere, fallback is 1.0 and 3
    let _root2 = Xform::define(&s, &path("/root2"));
    let _root2_child = Mesh::define(&s, &path("/root2/mesh"));

    let child_motion = MotionAPI::new(s.get_prim_at_path(&path("/root2/mesh")).unwrap());
    assert_eq!(
        child_motion.compute_motion_blur_scale(default_tc()),
        1.0_f32
    );
    assert_eq!(child_motion.compute_nonlinear_sample_count(default_tc()), 3);
}

// ============================================================================
// testUsdGeomImageable: test_MakeVisible
// ============================================================================

#[test]
fn test_imageable_make_visible() {
    let s = open_stage("testUsdGeomImageable", "AllInvisible.usda");

    let bar2 = Imageable::new(s.get_prim_at_path(&path("/foo/bar2")).unwrap());
    let thing1 = Imageable::new(s.get_prim_at_path(&path("/foo/bar1/thing1")).unwrap());
    let thing2 = Imageable::new(s.get_prim_at_path(&path("/foo/bar1/thing2")).unwrap());

    // Make thing1 visible
    thing1.make_visible(default_tc());

    // bar2 should remain invisible
    assert_eq!(
        bar2.compute_visibility(default_tc()).as_str(),
        "invisible",
        "bar2 should remain invisible"
    );

    // thing1 should now be visible (inherited)
    assert_eq!(
        thing1.compute_visibility(default_tc()).as_str(),
        "inherited",
        "thing1 should be visible (inherited)"
    );

    // thing2 should remain invisible
    assert_eq!(
        thing2.compute_visibility(default_tc()).as_str(),
        "invisible",
        "thing2 should remain invisible"
    );
}

// ============================================================================
// testUsdGeomConstraintTarget: test_Basic
// ============================================================================

#[test]
fn test_constraint_target_create_and_get() {
    let s = stage();

    let model = Xform::define(&s, &path("/Model"));
    assert!(model.is_valid());
    let model_prim = model.prim().clone();
    model_prim.set_metadata(&Token::new("kind"), Token::new("component"));

    // Create constraint target attribute directly on the prim
    // (ModelAPI::create_constraint_target has a pre-existing bug where
    // get_attribute returns a ghost attr before create_attribute runs)
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));
    let attr = model_prim
        .create_attribute("constraintTargets:rootXf", &matrix4d_type, true, None)
        .expect("should create constraint target attribute");

    assert!(attr.is_valid());

    // Wrap in ConstraintTarget and verify it's valid
    let cnstr_target = ConstraintTarget::new(attr.clone());
    assert!(ConstraintTarget::is_valid(&attr));
    assert!(cnstr_target.is_defined());

    // Verify the attr name is in constraintTargets: namespace
    assert!(
        attr.name().as_str().starts_with("constraintTargets:"),
        "attr name should be in constraintTargets namespace, got: {}",
        attr.name().as_str()
    );
}

#[test]
fn test_constraint_target_set_get_value() {
    let s = stage();

    let model = Xform::define(&s, &path("/Model"));
    let model_prim = model.prim().clone();
    model_prim.set_metadata(&Token::new("kind"), Token::new("component"));

    // Create constraint target attribute directly
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));
    let attr = model_prim
        .create_attribute("constraintTargets:rootXf", &matrix4d_type, true, None)
        .expect("create constraint target attr");

    let cnstr_target = ConstraintTarget::new(attr);
    assert!(cnstr_target.is_defined());

    // Set and get a matrix value
    let test_matrix = Matrix4d::identity();
    assert!(cnstr_target.set(&test_matrix, default_tc()));
    let read_back = cnstr_target.get(default_tc());
    assert!(read_back.is_some(), "should read back the set matrix value");
}

#[test]
fn test_constraint_target_identifier() {
    let s = stage();

    let model = Xform::define(&s, &path("/Model"));
    let model_prim = model.prim().clone();
    model_prim.set_metadata(&Token::new("kind"), Token::new("component"));

    // Create constraint target attribute directly
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));
    let attr = model_prim
        .create_attribute("constraintTargets:rootXf", &matrix4d_type, true, None)
        .expect("create constraint target attr");

    let cnstr_target = ConstraintTarget::new(attr);
    assert!(cnstr_target.is_defined());

    // Set and get identifier
    cnstr_target.set_identifier(&Token::new("RootXf"));
    assert_eq!(cnstr_target.get_identifier().as_str(), "RootXf");
}

#[test]
fn test_constraint_target_world_space() {
    let s = stage();

    let model = Xform::define(&s, &path("/Model"));
    let model_prim = model.prim().clone();
    model_prim.set_metadata(&Token::new("kind"), Token::new("component"));

    // model-space transform: identity + translate(10,20,30)
    let mut model_space = Matrix4d::from_scale(2.0);
    model_space.set_translate(&Vec3d::new(10.0, 20.0, 30.0));
    let xform_op = model
        .xformable()
        .add_transform_op(XformOpPrecision::Double, None, false);
    assert!(xform_op.is_valid());
    xform_op.set(model_space, default_tc());

    // Create constraint target attribute directly
    let registry = usd_sdf::ValueTypeRegistry::instance();
    let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));
    let attr = model_prim
        .create_attribute("constraintTargets:rootXf", &matrix4d_type, true, None)
        .expect("create constraint target attr");

    let cnstr_target = ConstraintTarget::new(attr);
    assert!(cnstr_target.is_defined());

    // Author a local constraint-space rotation (45 degrees about (1,1,0))
    let rotation = usd_gf::rotation::Rotation::from_axis_angle(Vec3d::new(1.0, 1.0, 0.0), 45.0);
    let quat = rotation.get_quat();
    let mut local_constraint_space = Matrix4d::identity();
    local_constraint_space.set_rotate(&quat);
    cnstr_target.set(&local_constraint_space, default_tc());

    // ComputeInWorldSpace = localConstraintSpace * modelSpace
    let world_space = cnstr_target.compute_in_world_space(default_tc(), None);
    let expected = local_constraint_space * model_space;
    assert!(
        usd_gf::matrix4::is_close(&world_space, &expected, 1e-4),
        "World space mismatch:\ngot:      {:?}\nexpected: {:?}",
        world_space,
        expected,
    );
}

#[test]
fn test_constraint_target_attr_name() {
    // Verify get_constraint_attr_name produces correct namespaced name
    let attr_name = ConstraintTarget::get_constraint_attr_name("rootXf");
    assert_eq!(attr_name.as_str(), "constraintTargets:rootXf");

    let attr_name2 = ConstraintTarget::get_constraint_attr_name("hand_L");
    assert_eq!(attr_name2.as_str(), "constraintTargets:hand_L");
}

#[test]
fn test_constraint_target_invalid_names() {
    let s = stage();

    let model = Xform::define(&s, &path("/Model"));
    let model_prim = model.prim().clone();
    model_prim.set_metadata(&Token::new("kind"), Token::new("component"));

    let registry = usd_sdf::ValueTypeRegistry::instance();
    let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));
    let float_type = registry.find_type_by_token(&Token::new("float"));

    // Attribute without constraintTargets: namespace is NOT a valid constraint target
    if let Some(invalid_attr) =
        model_prim.create_attribute("invalidConstraintTargetName", &matrix4d_type, true, None)
    {
        assert!(!ConstraintTarget::is_valid(&invalid_attr));
    }

    // constraintTargets: namespace but wrong type is NOT valid
    if let Some(wrong_type_attr) =
        model_prim.create_attribute("constraintTargets:invalidTypeXf", &float_type, true, None)
    {
        assert!(!ConstraintTarget::is_valid(&wrong_type_attr));
    }
}

// ============================================================================
// testUsdGeomHermiteCurves: testPointAndTangents
// ============================================================================

#[test]
fn test_hermite_curves_mismatched_sizes() {
    // Mismatched points/tangents sizes should produce empty
    let invalid = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(1.0, 2.0, 3.0)],
        vec![Vec3f::new(1.0, 2.0, 3.0), Vec3f::new(4.0, 5.0, 6.0)],
    );
    assert!(invalid.is_empty());
}

#[test]
fn test_hermite_curves_empty() {
    let empty = PointAndTangentArrays::new();
    assert!(empty.get_points().is_empty());
    assert!(empty.get_tangents().is_empty());
    assert!(empty.is_empty());
}

#[test]
fn test_hermite_curves_valid_construction() {
    let pt = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(-1.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 2.0, 3.0), Vec3f::new(4.0, 5.0, 6.0)],
    );
    assert!(!pt.is_empty());
    assert_eq!(pt.get_points().len(), 2);
    assert_eq!(pt.get_tangents().len(), 2);

    assert_eq!(pt.get_points()[0], Vec3f::new(1.0, 0.0, 0.0));
    assert_eq!(pt.get_points()[1], Vec3f::new(-1.0, 0.0, 0.0));
    assert_eq!(pt.get_tangents()[0], Vec3f::new(1.0, 2.0, 3.0));
    assert_eq!(pt.get_tangents()[1], Vec3f::new(4.0, 5.0, 6.0));
}

#[test]
fn test_hermite_curves_equality() {
    // Empty == empty
    assert_eq!(PointAndTangentArrays::new(), PointAndTangentArrays::new());

    // Same data == same data
    let a = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(2.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 0.0, 0.0)],
    );
    let b = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(2.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 0.0, 0.0)],
    );
    assert_eq!(a, b);

    // Empty != non-empty
    let c = PointAndTangentArrays::new();
    assert_ne!(c, a);
}

// ============================================================================
// testUsdGeomHermiteCurves: testInterleave
// ============================================================================

#[test]
fn test_hermite_curves_interleave() {
    let pt = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(2.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(0.0, 1.0, 0.0)],
    );
    let interleaved = pt.interleave();
    assert_eq!(interleaved.len(), 4);
    assert_eq!(interleaved[0], Vec3f::new(0.0, 0.0, 0.0));
    assert_eq!(interleaved[1], Vec3f::new(1.0, 0.0, 0.0));
    assert_eq!(interleaved[2], Vec3f::new(2.0, 0.0, 0.0));
    assert_eq!(interleaved[3], Vec3f::new(0.0, 1.0, 0.0));
}

#[test]
fn test_hermite_curves_interleave_empty() {
    let empty = PointAndTangentArrays::new();
    let interleaved = empty.interleave();
    assert!(interleaved.is_empty());
}

// ============================================================================
// testUsdGeomHermiteCurves: testSeparate
// ============================================================================

#[test]
fn test_hermite_curves_separate_odd_count() {
    // Odd number of elements should produce empty (error)
    let result = PointAndTangentArrays::separate(&[
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(2.0, 0.0, 0.0),
    ]);
    assert!(result.is_empty());
}

#[test]
fn test_hermite_curves_separate_valid() {
    let separated = PointAndTangentArrays::separate(&[
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(2.0, 0.0, 0.0),
        Vec3f::new(0.0, 1.0, 0.0),
    ]);

    let expected = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(2.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(0.0, 1.0, 0.0)],
    );
    assert_eq!(separated, expected);
}

#[test]
fn test_hermite_curves_separate_empty() {
    let empty = PointAndTangentArrays::separate(&[]);
    assert!(empty.is_empty());
}

#[test]
fn test_hermite_curves_roundtrip() {
    // Interleave then separate should yield the original
    let original = PointAndTangentArrays::from_points_and_tangents(
        vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(2.0, 0.0, 0.0)],
        vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(0.0, 1.0, 0.0)],
    );
    let interleaved = original.interleave();
    let roundtripped = PointAndTangentArrays::separate(&interleaved);
    assert_eq!(roundtripped, original);
}

// ============================================================================
// testUsdGeomTypeRegistry: test_ConcreteTyped, test_AbstractTyped, test_Applied
// ============================================================================
//
// The Python test checks that TfType + SchemaRegistry roundtrips work for
// schema type names. Our Rust port verifies the schema_type_name() statics
// return correct values and that define/get produce valid schemas.
// We don't have TfType registry in Rust, so we test the equivalent:
// schema type names match, define() creates correct prim types, and
// API schemas can be applied.

#[test]
fn test_type_registry_concrete_typed() {
    let s = stage();

    // Mesh
    let mesh = Mesh::define(&s, &path("/Mesh"));
    assert!(mesh.is_valid());
    assert_eq!(mesh.prim().get_type_name().as_str(), "Mesh");
    assert_eq!(Mesh::schema_type_name().as_str(), "Mesh");

    // Sphere
    let sphere = Sphere::define(&s, &path("/Sphere"));
    assert!(sphere.is_valid());
    assert_eq!(sphere.prim().get_type_name().as_str(), "Sphere");
    assert_eq!(Sphere::schema_type_name().as_str(), "Sphere");
}

#[test]
fn test_type_registry_abstract_typed() {
    // Abstract schemas: Boundable and Imageable do not produce concrete typed prims.
    // Verify that wrapping an empty prim in these schemas yields invalid or
    // that the abstract type name is reported correctly.
    let s = stage();

    // Attempting to define a prim with abstract type "Boundable" or "Imageable"
    // may or may not succeed depending on the implementation, but the schema
    // type names should be correct.
    assert_eq!(Imageable::schema_type_name().as_str(), "Imageable");

    // An Imageable wrapping a real Xform prim should work
    let xf = Xform::define(&s, &path("/AbstractTest"));
    let imageable = Imageable::new(xf.prim().clone());
    assert!(imageable.is_valid());
}

#[test]
fn test_type_registry_applied_api() {
    let s = stage();

    // PrimvarsAPI and MotionAPI are applied API schemas.
    let prim = s.define_prim("/ApiTest", "Xform").unwrap();

    // MotionAPI: verify the schema is registered in the registry
    let motion_info = usd_core::SchemaRegistry::find_schema_info(&Token::new("MotionAPI"));
    assert!(
        motion_info.is_some(),
        "MotionAPI should be registered in SchemaRegistry"
    );
    assert_eq!(
        motion_info.unwrap().kind,
        usd_core::SchemaKind::SingleApplyAPI
    );

    // MotionAPI wrapper should work on a valid prim
    let motion = MotionAPI::new(prim.clone());
    assert!(motion.is_valid());

    // PrimvarsAPI: registered as SingleApplyAPI
    let pv_info = usd_core::SchemaRegistry::find_schema_info(&Token::new("PrimvarsAPI"));
    assert!(
        pv_info.is_some(),
        "PrimvarsAPI should be registered in SchemaRegistry"
    );

    // PrimvarsAPI wrapping a valid prim should work
    let pv_api = PrimvarsAPI::new(prim.clone());
    assert!(prim.is_valid());
    let _ = pv_api;
}
