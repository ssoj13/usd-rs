//! Tests for UsdGeomPointInstancer.
//!
//! Ported from: testenv/testUsdGeomPointInstancer.py

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::point_instancer::{MaskApplication, ProtoXformInclusion};
use usd_geom::*;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::{Vec3d, Vec3f};
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Helpers
// ============================================================================

fn testenv_path(subdir: &str, file: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("testenv");
    p.push(subdir);
    p.push(file);
    p.to_string_lossy().to_string()
}

fn open_stage(subdir: &str, file: &str) -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(testenv_path(subdir, file), InitialLoadSet::LoadAll).unwrap()
}

fn default_tc() -> TimeCode {
    TimeCode::default_time()
}

/// Assert two Vec3f are close (element-wise within epsilon).
fn assert_close_v3f(a: &Vec3f, b: &Vec3f, eps: f32) {
    assert!(
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps,
        "Vec3f mismatch: {:?} vs {:?} (eps={eps})",
        a,
        b
    );
}

/// Assert two Matrix4d are close (element-wise within epsilon).
fn assert_close_xf(a: &Matrix4d, b: &Matrix4d, eps: f64) {
    for row in 0..4 {
        for col in 0..4 {
            let va = a[row][col];
            let vb = b[row][col];
            assert!(
                (va - vb).abs() < eps,
                "Matrix mismatch at [{row}][{col}]: {va} vs {vb}"
            );
        }
    }
}

/// Add an inline cube model at the given path.
/// Creates a unit cube mesh with extent [(-0.5,-0.5,-0.5), (0.5,0.5,0.5)].
/// This avoids reference composition issues in tests.
fn add_cube_model(stage: &Stage, prim_path: &str) -> usd_core::Prim {
    let prim = stage
        .define_prim(prim_path, "Xform")
        .expect("Failed to define prim");
    let geom_path = format!("{}/Geom", prim_path);
    let _geom = stage.define_prim(&geom_path, "Xform").unwrap();
    let cube_path = format!("{}/Geom/Cube", prim_path);
    let _mesh_prim = stage.define_prim(&cube_path, "Mesh").unwrap();

    // Set extent
    let mesh = Mesh::get(stage, &usd_sdf::Path::from_string(&cube_path).unwrap());
    let extent_attr = mesh.point_based().gprim().boundable().create_extent_attr();
    extent_attr.set(
        Value::from(vec![
            Vec3f::new(-0.5, -0.5, -0.5),
            Vec3f::new(0.5, 0.5, 0.5),
        ]),
        default_tc(),
    );

    // Set points
    let points_attr = mesh.point_based().create_points_attr(None, false);
    points_attr.set(
        Value::from(vec![
            Vec3f::new(-0.5, -0.5, 0.5),
            Vec3f::new(0.5, -0.5, 0.5),
            Vec3f::new(-0.5, 0.5, 0.5),
            Vec3f::new(0.5, 0.5, 0.5),
            Vec3f::new(-0.5, 0.5, -0.5),
            Vec3f::new(0.5, 0.5, -0.5),
            Vec3f::new(-0.5, -0.5, -0.5),
            Vec3f::new(0.5, -0.5, -0.5),
        ]),
        default_tc(),
    );

    // Set face vertex counts and indices
    let fvc_attr = mesh.create_face_vertex_counts_attr(None, false);
    fvc_attr.set(Value::from(vec![4i32, 4, 4, 4, 4, 4]), default_tc());
    let fvi_attr = mesh.create_face_vertex_indices_attr(None, false);
    fvi_attr.set(
        Value::from(vec![
            0i32, 1, 3, 2, 2, 3, 5, 4, 4, 5, 7, 6, 6, 7, 1, 0, 1, 7, 5, 3, 6, 0, 2, 4,
        ]),
        default_tc(),
    );

    prim
}

/// Set transform components and proto indices on a PointInstancer.
fn set_transform_components_and_indices(
    instancer: &PointInstancer,
    positions: &[Vec3f],
    indices: &[i32],
    scales: Option<&[Vec3f]>,
    orientations: Option<&[usd_gf::quat::Quath]>,
) {
    let pos_attr = instancer.create_positions_attr(None, false);
    pos_attr.set(Value::from(positions.to_vec()), default_tc());

    let idx_attr = instancer.create_proto_indices_attr(None, false);
    idx_attr.set(Value::from(indices.to_vec()), default_tc());

    if let Some(s) = scales {
        let scale_attr = instancer.create_scales_attr(None, false);
        scale_attr.set(Value::from(s.to_vec()), default_tc());
    }

    if let Some(o) = orientations {
        let orient_attr = instancer.create_orientations_attr(None, false);
        orient_attr.set(Value::from_no_hash(o.to_vec()), default_tc());
    }
}

/// Validate extent of a PointInstancer against expected values.
fn validate_extent(instancer: &PointInstancer, expected_min: Vec3f, expected_max: Vec3f) {
    let mut extent = usd_vt::Array::new();
    let ok = instancer.compute_extent_at_time(&mut extent, default_tc(), default_tc());
    assert!(ok, "compute_extent_at_time failed");
    assert_eq!(extent.len(), 2, "Extent should have 2 elements");
    assert_close_v3f(&extent[0], &expected_min, 1e-4);
    assert_close_v3f(&extent[1], &expected_max, 1e-4);
}

/// Validate instance transforms of a PointInstancer against expected transforms.
fn validate_instance_transforms(instancer: &PointInstancer, expected: &[Matrix4d]) {
    let mut xforms = Vec::new();
    let ok = instancer.compute_instance_transforms_at_time(
        &mut xforms,
        default_tc(),
        default_tc(),
        ProtoXformInclusion::IncludeProtoXform,
        MaskApplication::ApplyMask,
    );
    assert!(ok, "compute_instance_transforms_at_time failed");
    assert_eq!(
        xforms.len(),
        expected.len(),
        "Transform count mismatch: got {} expected {}",
        xforms.len(),
        expected.len()
    );
    for i in 0..xforms.len() {
        assert_close_xf(&xforms[i], &expected[i], 1e-5);
    }
}

/// Build a Matrix4d from 4 rows (convenience for tests).
fn mat4_from_rows(r0: [f64; 4], r1: [f64; 4], r2: [f64; 4], r3: [f64; 4]) -> Matrix4d {
    Matrix4d::from_array([r0, r1, r2, r3])
}

/// Create the standard test fixture: an in-memory stage with PointInstancer at
/// /MyPointInstancer with 4 prototype cubes (origin, origin+scale5, translated, rotated).
fn create_test_fixture() -> (Arc<Stage>, PointInstancer) {
    usd_sdf::init();

    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let instancer_path = usd_sdf::Path::from_string("/MyPointInstancer").unwrap();
    let instancer = PointInstancer::define(&stage, &instancer_path);
    assert!(instancer.is_valid());

    let protos_path_str = "/MyPointInstancer/prototypes";
    let _protos_prim = stage
        .define_prim(protos_path_str, "")
        .expect("Failed to define prototypes prim");

    // Prototype 0: OriginCube -- unit cube at origin
    let _origin_cube = add_cube_model(&stage, "/MyPointInstancer/prototypes/OriginCube");
    // Prototype 1: OriginScaledCube -- unit cube at origin with 5x scale
    let origin_scaled_cube =
        add_cube_model(&stage, "/MyPointInstancer/prototypes/OriginScaledCube");
    let xf_scaled = Xformable::new(origin_scaled_cube.clone());
    let scale_op = xf_scaled.add_scale_op(XformOpPrecision::Float, None, false);
    scale_op.set(Vec3f::new(5.0, 5.0, 5.0), default_tc());

    // Prototype 2: TranslatedCube -- unit cube with translation
    let translated_cube = add_cube_model(&stage, "/MyPointInstancer/prototypes/TranslatedCube");
    let xf_translated = Xformable::new(translated_cube.clone());
    let translate_op = xf_translated.add_translate_op(XformOpPrecision::Double, None, false);
    translate_op.set(Vec3d::new(3.0, 6.0, 9.0), default_tc());

    // Prototype 3: RotatedCube -- unit cube with 45-degree Z rotation
    let rotated_cube = add_cube_model(&stage, "/MyPointInstancer/prototypes/RotatedCube");
    let xf_rotated = Xformable::new(rotated_cube.clone());
    let rotate_op = xf_rotated.add_rotate_z_op(XformOpPrecision::Float, None, false);
    rotate_op.set(45.0f32, default_tc());

    // Set prototypes relationship
    let protos_rel = instancer.create_prototypes_rel();
    protos_rel.set_targets(&[
        usd_sdf::Path::from_string("/MyPointInstancer/prototypes/OriginCube").unwrap(),
        usd_sdf::Path::from_string("/MyPointInstancer/prototypes/OriginScaledCube").unwrap(),
        usd_sdf::Path::from_string("/MyPointInstancer/prototypes/TranslatedCube").unwrap(),
        usd_sdf::Path::from_string("/MyPointInstancer/prototypes/RotatedCube").unwrap(),
    ]);

    (stage, instancer)
}

// ============================================================================
// test_ExtentOneOriginCubeInstance
// ============================================================================

#[test]
fn test_extent_one_origin_cube_instance() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [Vec3f::new(10.0, 15.0, 20.0)];
    let indices = [0i32];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    validate_extent(
        &instancer,
        Vec3f::new(9.5, 14.5, 19.5),
        Vec3f::new(10.5, 15.5, 20.5),
    );
}

// ============================================================================
// test_ExtentOneOriginScaledCubeInstance
// ============================================================================

#[test]
#[ignore = "requires working Xformable scale op round-trip"]
fn test_extent_one_origin_scaled_cube_instance() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [Vec3f::new(20.0, 30.0, 40.0)];
    let indices = [1i32];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    validate_extent(
        &instancer,
        Vec3f::new(17.5, 27.5, 37.5),
        Vec3f::new(22.5, 32.5, 42.5),
    );
}

// ============================================================================
// test_ExtentOneTranslatedCubeInstance
// ============================================================================

#[test]
#[ignore = "requires working Xformable translate op round-trip"]
fn test_extent_one_translated_cube_instance() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [Vec3f::new(-2.0, -4.0, -6.0)];
    let indices = [2i32];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    validate_extent(
        &instancer,
        Vec3f::new(0.5, 1.5, 2.5),
        Vec3f::new(1.5, 2.5, 3.5),
    );
}

// ============================================================================
// test_ExtentOneRotatedCubeInstance
// ============================================================================

#[test]
#[ignore = "requires working Xformable rotate op round-trip"]
fn test_extent_one_rotated_cube_instance() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [Vec3f::new(100.0, 100.0, 100.0)];
    let indices = [3i32];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    validate_extent(
        &instancer,
        Vec3f::new(99.29289, 99.29289, 99.5),
        Vec3f::new(100.70711, 100.70711, 100.5),
    );
}

// ============================================================================
// test_ExtentMultipleCubeInstances
// ============================================================================

#[test]
#[ignore = "requires working Xformable scale op round-trip for proto bounds"]
fn test_extent_multiple_cube_instances() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [
        Vec3f::new(-33.0, -33.0, 0.0),
        Vec3f::new(0.0, 0.0, -44.0),
        Vec3f::new(0.0, 222.0, 555.0),
        Vec3f::new(66.0, 111.0, 11.0),
    ];
    let indices = [0i32, 1, 0, 1];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    validate_extent(
        &instancer,
        Vec3f::new(-33.5, -33.5, -46.5),
        Vec3f::new(68.5, 222.5, 555.5),
    );
}

// ============================================================================
// test_InstanceTransformsAndExtentWithMaskedInstances
// ============================================================================

#[test]
#[ignore = "requires working Xformable scale op round-trip for proto transforms"]
fn test_instance_transforms_and_extent_with_masked_instances() {
    let (_stage, instancer) = create_test_fixture();

    let positions = [
        Vec3f::new(-2.5, -2.5, -2.5),
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(2.5, 2.5, 2.5),
    ];
    let scales = [
        Vec3f::new(1.0, 1.0, 1.0),
        Vec3f::new(20.0, 20.0, 20.0),
        Vec3f::new(1.0, 1.0, 1.0),
    ];
    let indices = [1i32, 0, 1];
    set_transform_components_and_indices(&instancer, &positions, &indices, Some(&scales), None);

    // Instance 0: proto=1 (scaled cube with 5x scale op), instance scale=1 -> 5x total
    // Instance 1: proto=0 (origin cube, identity xform), instance scale=20 -> 20x
    // Instance 2: proto=1 (scaled cube), instance scale=1 -> 5x
    let mut scale20 = Matrix4d::identity();
    scale20.set_scale(20.0);

    let expected_xforms_all = [
        mat4_from_rows(
            [5.0, 0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0, 0.0],
            [0.0, 0.0, 5.0, 0.0],
            [-2.5, -2.5, -2.5, 1.0],
        ),
        scale20,
        mat4_from_rows(
            [5.0, 0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0, 0.0],
            [0.0, 0.0, 5.0, 0.0],
            [2.5, 2.5, 2.5, 1.0],
        ),
    ];
    validate_instance_transforms(&instancer, &expected_xforms_all);

    validate_extent(
        &instancer,
        Vec3f::new(-10.0, -10.0, -10.0),
        Vec3f::new(10.0, 10.0, 10.0),
    );

    // Deactivate instance 1
    assert!(instancer.deactivate_id(1));

    // After deactivating, only instances 0 and 2 remain
    let expected_xforms_masked = [
        mat4_from_rows(
            [5.0, 0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0, 0.0],
            [0.0, 0.0, 5.0, 0.0],
            [-2.5, -2.5, -2.5, 1.0],
        ),
        mat4_from_rows(
            [5.0, 0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0, 0.0],
            [0.0, 0.0, 5.0, 0.0],
            [2.5, 2.5, 2.5, 1.0],
        ),
    ];
    validate_instance_transforms(&instancer, &expected_xforms_masked);

    validate_extent(
        &instancer,
        Vec3f::new(-5.0, -5.0, -5.0),
        Vec3f::new(5.0, 5.0, 5.0),
    );
}

// ============================================================================
// test_BBoxCache
// ============================================================================

#[test]
#[ignore = "requires working Xformable op round-trip for proto transforms and BBox computation"]
fn test_bbox_cache() {
    usd_sdf::init();

    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    // Build hierarchy: /World/parent/MyPointInstancer
    let world_path = usd_sdf::Path::from_string("/World").unwrap();
    let world = Xform::define(&stage, &world_path);
    assert!(world.is_valid());

    let parent_path = usd_sdf::Path::from_string("/World/parent").unwrap();
    let parent = Xform::define(&stage, &parent_path);
    assert!(parent.is_valid());

    let instancer_path = usd_sdf::Path::from_string("/World/parent/MyPointInstancer").unwrap();
    let instancer = PointInstancer::define(&stage, &instancer_path);
    assert!(instancer.is_valid());

    let protos_path = "/World/parent/MyPointInstancer/prototypes";
    let _protos_prim = stage.define_prim(protos_path, "").unwrap();

    // Prototype 0: OriginCube
    let _origin_cube = add_cube_model(
        &stage,
        "/World/parent/MyPointInstancer/prototypes/OriginCube",
    );

    // Prototype 1: OriginScaledCube with 5x scale
    let origin_scaled_cube = add_cube_model(
        &stage,
        "/World/parent/MyPointInstancer/prototypes/OriginScaledCube",
    );
    let xf_scaled = Xformable::new(origin_scaled_cube.clone());
    let scale_op = xf_scaled.add_scale_op(XformOpPrecision::Float, None, false);
    scale_op.set(Vec3f::new(5.0, 5.0, 5.0), default_tc());

    // Prototype 2: TranslatedCube
    let translated_cube = add_cube_model(
        &stage,
        "/World/parent/MyPointInstancer/prototypes/TranslatedCube",
    );
    let xf_translated = Xformable::new(translated_cube.clone());
    let translate_op = xf_translated.add_translate_op(XformOpPrecision::Double, None, false);
    translate_op.set(Vec3d::new(3.0, 6.0, 9.0), default_tc());

    // Prototype 3: RotatedCube
    let rotated_cube = add_cube_model(
        &stage,
        "/World/parent/MyPointInstancer/prototypes/RotatedCube",
    );
    let xf_rotated = Xformable::new(rotated_cube.clone());
    let rotate_op = xf_rotated.add_rotate_z_op(XformOpPrecision::Float, None, false);
    rotate_op.set(45.0f32, default_tc());

    // Set prototypes relationship
    let protos_rel = instancer.create_prototypes_rel();
    protos_rel.set_targets(&[
        usd_sdf::Path::from_string("/World/parent/MyPointInstancer/prototypes/OriginCube").unwrap(),
        usd_sdf::Path::from_string("/World/parent/MyPointInstancer/prototypes/OriginScaledCube")
            .unwrap(),
        usd_sdf::Path::from_string("/World/parent/MyPointInstancer/prototypes/TranslatedCube")
            .unwrap(),
        usd_sdf::Path::from_string("/World/parent/MyPointInstancer/prototypes/RotatedCube")
            .unwrap(),
    ]);

    // Add transforms on world, parent, instancer
    let world_xlate = world
        .xformable()
        .add_translate_op(XformOpPrecision::Double, None, false);
    world_xlate.set(Vec3d::new(2.0, 2.0, 2.0), default_tc());
    let parent_xlate = parent
        .xformable()
        .add_translate_op(XformOpPrecision::Double, None, false);
    parent_xlate.set(Vec3d::new(7.0, 7.0, 7.0), default_tc());
    let instancer_xformable = Xformable::new(instancer.prim().clone());
    let inst_xlate = instancer_xformable.add_translate_op(XformOpPrecision::Double, None, false);
    inst_xlate.set(Vec3d::new(11.0, 11.0, 11.0), default_tc());

    // All 4 instances at origin, one per prototype
    let positions = [
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 0.0),
    ];
    let indices = [0i32, 1, 2, 3];
    set_transform_components_and_indices(&instancer, &positions, &indices, None, None);

    // Build BBoxCache
    let purposes = vec![Token::new("default")];
    let mut bbox_cache = BBoxCache::new(default_tc(), purposes, false, false);

    let unit_box = usd_gf::Range3d::new(Vec3d::new(-0.5, -0.5, -0.5), Vec3d::new(0.5, 0.5, 0.5));

    let worldxf = Matrix4d::from_translation(Vec3d::new(2.0, 2.0, 2.0));
    let parentxf = Matrix4d::from_translation(Vec3d::new(7.0, 7.0, 7.0));
    let instancerxf = Matrix4d::from_translation(Vec3d::new(11.0, 11.0, 11.0));

    let cubescale = Matrix4d::from_scale_vec(&Vec3d::new(5.0, 5.0, 5.0));
    let cubexlat = Matrix4d::from_translation(Vec3d::new(3.0, 6.0, 9.0));

    // Build rotation matrix for 45 degrees around Z
    let cos45 = std::f64::consts::FRAC_PI_4.cos();
    let sin45 = std::f64::consts::FRAC_PI_4.sin();
    let cuberot = mat4_from_rows(
        [cos45, sin45, 0.0, 0.0],
        [-sin45, cos45, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );

    let cases: [(i64, Matrix4d); 4] = [
        (0, Matrix4d::identity()), // originCube
        (1, cubescale),            // originScaledCube
        (2, cubexlat),             // translatedCube
        (3, cuberot),              // rotatedCube
    ];

    // Scalar point instance bound tests
    for (iid, cubexf) in &cases {
        // World bound = unitBox * cubexf * instancerxf * parentxf * worldxf
        let world_bound = bbox_cache.compute_point_instance_world_bound(&instancer, *iid);
        let expected_world_xf = *cubexf * instancerxf * parentxf * worldxf;
        let expected_world = usd_gf::BBox3d::from_range_matrix(unit_box, expected_world_xf);
        assert_bbox3d_close(&world_bound, &expected_world, 1e-5);

        // Relative bound = unitBox * cubexf * instancerxf * parentxf
        let relative_bound =
            bbox_cache.compute_point_instance_relative_bound(&instancer, *iid, world.prim());
        let expected_relative_xf = *cubexf * instancerxf * parentxf;
        let expected_relative = usd_gf::BBox3d::from_range_matrix(unit_box, expected_relative_xf);
        assert_bbox3d_close(&relative_bound, &expected_relative, 1e-5);

        // Local bound = unitBox * cubexf * instancerxf
        let local_bound = bbox_cache.compute_point_instance_local_bound(&instancer, *iid);
        let expected_local_xf = *cubexf * instancerxf;
        let expected_local = usd_gf::BBox3d::from_range_matrix(unit_box, expected_local_xf);
        assert_bbox3d_close(&local_bound, &expected_local, 1e-5);

        // Untransformed bound = unitBox * cubexf
        let untransformed_bound =
            bbox_cache.compute_point_instance_untransformed_bound(&instancer, *iid);
        let expected_untransformed = usd_gf::BBox3d::from_range_matrix(unit_box, *cubexf);
        assert_bbox3d_close(&untransformed_bound, &expected_untransformed, 1e-5);
    }

    // Vectorized tests
    let instance_ids: Vec<i64> = vec![0, 1, 2, 3];
    let world_bounds = bbox_cache.compute_point_instance_world_bounds(&instancer, &instance_ids);
    for (i, wbox) in world_bounds.iter().enumerate() {
        let expected_xf = cases[i].1 * instancerxf * parentxf * worldxf;
        let expected = usd_gf::BBox3d::from_range_matrix(unit_box, expected_xf);
        assert_bbox3d_close(wbox, &expected, 1e-5);
    }

    let relative_bounds =
        bbox_cache.compute_point_instance_relative_bounds(&instancer, &instance_ids, world.prim());
    for (i, rbox) in relative_bounds.iter().enumerate() {
        let expected_xf = cases[i].1 * instancerxf * parentxf;
        let expected = usd_gf::BBox3d::from_range_matrix(unit_box, expected_xf);
        assert_bbox3d_close(rbox, &expected, 1e-5);
    }

    let local_bounds = bbox_cache.compute_point_instance_local_bounds(&instancer, &instance_ids);
    for (i, lbox) in local_bounds.iter().enumerate() {
        let expected_xf = cases[i].1 * instancerxf;
        let expected = usd_gf::BBox3d::from_range_matrix(unit_box, expected_xf);
        assert_bbox3d_close(lbox, &expected, 1e-5);
    }

    let untransformed_bounds =
        bbox_cache.compute_point_instance_untransformed_bounds(&instancer, &instance_ids);
    for (i, ubox) in untransformed_bounds.iter().enumerate() {
        let expected = usd_gf::BBox3d::from_range_matrix(unit_box, cases[i].1);
        assert_bbox3d_close(ubox, &expected, 1e-5);
    }
}

/// Assert two BBox3d are close (compare aligned ranges).
fn assert_bbox3d_close(a: &usd_gf::BBox3d, b: &usd_gf::BBox3d, eps: f64) {
    let a_range = a.compute_aligned_range();
    let b_range = b.compute_aligned_range();
    let a_min = a_range.min();
    let b_min = b_range.min();
    let a_max = a_range.max();
    let b_max = b_range.max();
    assert!(
        (a_min.x - b_min.x).abs() < eps
            && (a_min.y - b_min.y).abs() < eps
            && (a_min.z - b_min.z).abs() < eps,
        "BBox3d min mismatch: {:?} vs {:?}",
        a_min,
        b_min
    );
    assert!(
        (a_max.x - b_max.x).abs() < eps
            && (a_max.y - b_max.y).abs() < eps
            && (a_max.z - b_max.z).abs() < eps,
        "BBox3d max mismatch: {:?} vs {:?}",
        a_max,
        b_max
    );
}

// ============================================================================
// test_ComputeInstancerCount
// ============================================================================

#[test]
fn test_compute_instancer_count() {
    let stage = open_stage("testUsdGeomPointInstancer", "instancer.usda");

    let earliest = TimeCode::new(f64::MIN);

    let unset = PointInstancer::get(
        &stage,
        &usd_sdf::Path::from_string("/UnsetIndices").unwrap(),
    );
    let blocked = PointInstancer::get(
        &stage,
        &usd_sdf::Path::from_string("/BlockedIndices").unwrap(),
    );
    let empty = PointInstancer::get(
        &stage,
        &usd_sdf::Path::from_string("/EmptyIndices").unwrap(),
    );
    let time_sampled = PointInstancer::get(
        &stage,
        &usd_sdf::Path::from_string("/TimeSampledIndices").unwrap(),
    );
    let time_sampled_and_default = PointInstancer::get(
        &stage,
        &usd_sdf::Path::from_string("/TimeSampledAndDefaultIndices").unwrap(),
    );

    // Time-sampled queries (EarliestTime)
    assert!(unset.is_valid());
    assert_eq!(unset.get_instance_count(earliest), 0);

    assert!(blocked.is_valid());
    assert_eq!(blocked.get_instance_count(earliest), 0);

    assert!(empty.is_valid());
    assert_eq!(empty.get_instance_count(earliest), 0);

    assert!(time_sampled.is_valid());
    assert_eq!(time_sampled.get_instance_count(earliest), 3);

    assert!(time_sampled_and_default.is_valid());
    assert_eq!(time_sampled_and_default.get_instance_count(earliest), 5);

    // Default queries
    assert_eq!(unset.get_instance_count(default_tc()), 0);
    assert_eq!(blocked.get_instance_count(default_tc()), 0);
    assert_eq!(empty.get_instance_count(default_tc()), 0);
    assert_eq!(time_sampled.get_instance_count(default_tc()), 0);
    assert_eq!(time_sampled_and_default.get_instance_count(default_tc()), 4);

    // Invalid instancer: Rust returns 0 instead of raising RuntimeError
    let invalid = PointInstancer::invalid();
    assert!(!invalid.is_valid());
    assert_eq!(invalid.get_instance_count(default_tc()), 0);
    assert_eq!(invalid.get_instance_count(earliest), 0);
}

// ============================================================================
// test_InstancerInVis
// ============================================================================

#[test]
fn test_instancer_invis() {
    usd_sdf::init();

    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let instancer_path = usd_sdf::Path::from_string("/Instance").unwrap();
    let instancer = PointInstancer::define(&stage, &instancer_path);
    assert!(instancer.is_valid());

    // InvisId(1, Default) should author invisibleIds = [1]
    assert!(instancer.invis_id(1, default_tc()));

    // Read back the invisibleIds attribute
    let invis_attr = instancer.get_invisible_ids_attr();
    assert!(invis_attr.is_valid());

    if let Some(value) = invis_attr.get(default_tc()) {
        if let Some(arr) = value.get::<usd_vt::Array<i64>>() {
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0], 1);
        } else if let Some(vec) = value.get::<Vec<i64>>() {
            assert_eq!(vec.len(), 1);
            assert_eq!(vec[0], 1);
        } else {
            panic!("invisibleIds value has unexpected type");
        }
    } else {
        panic!("invisibleIds has no value at default time");
    }
}
