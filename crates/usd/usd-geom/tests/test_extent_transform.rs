//! Port of testUsdGeomExtentTransform.py
//!
//! Tests extent computation with and without transform matrices
//! for all Boundable geometry types.

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::matrix4::Matrix4d;
use usd_gf::rotation::Rotation;
use usd_gf::vec3::{Vec3d, Vec3f};
use usd_sdf::TimeCode;

const EXTENT_TOLERANCE: f32 = 0.00001;

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

/// Build the STRANGE_TRANSFORM from the Python test:
/// Gf.Matrix4d(Gf.Rotation(Gf.Vec3d(1,1,1), 45.0), Gf.Vec3d(1,2,3))
///   * Gf.Matrix4d(Gf.Vec4d(1,1,0.5,1))
fn strange_transform() -> Matrix4d {
    // Rotation 45 degrees around (1,1,1) with translation (1,2,3)
    let rot = Rotation::from_axis_angle(Vec3d::new(1.0, 1.0, 1.0), 45.0);
    let mut rot_trans = rot.get_matrix4();
    rot_trans.set_translate_only(&Vec3d::new(1.0, 2.0, 3.0));

    // Diagonal scale matrix (1, 1, 0.5, 1)
    let scale = Matrix4d::from_diagonal_values(1.0, 1.0, 0.5, 1.0);

    rot_trans * scale
}

fn vec3f_close(a: &Vec3f, b: &Vec3f, tol: f32) -> bool {
    (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol && (a.z - b.z).abs() < tol
}

fn assert_extents_equal(e1: &[Vec3f; 2], e2: &[Vec3f; 2]) {
    assert!(
        vec3f_close(&e1[0], &e2[0], EXTENT_TOLERANCE),
        "min mismatch: {:?} vs {:?}",
        e1[0],
        e2[0]
    );
    assert!(
        vec3f_close(&e1[1], &e2[1], EXTENT_TOLERANCE),
        "max mismatch: {:?} vs {:?}",
        e1[1],
        e2[1]
    );
}

/// Compute extent via the plugin registry (matches Python ComputeExtentFromPlugins).
fn verify_extent(boundable: &Boundable, expected: &[Vec3f; 2], transform: Option<&Matrix4d>) {
    let computed = compute_extent_from_plugins(boundable, TimeCode::default_time(), transform)
        .expect("compute_extent_from_plugins returned None");
    assert_extents_equal(&computed, expected);
}

// ============================================================================
// test_Sphere
// ============================================================================

#[test]
fn test_sphere() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let path = usd_sdf::Path::from_string("/Foo").expect("bad path");
    let sphere = Sphere::define(&stage, &path);
    assert!(sphere.is_valid());

    // radius=2
    sphere
        .create_radius_attr(None, false)
        .set(usd_vt::Value::from(2.0_f64), TimeCode::default_time());

    let boundable = Boundable::new(sphere.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        None,
    );

    // Identity transform
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -2.242468870104182,
                -1.2424688701041822,
                -0.12123443505209108,
            ),
            Vec3f::new(4.242468870104182, 5.242468870104181, 3.121234435052091),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Cube
// ============================================================================

#[test]
fn test_cube() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let path = usd_sdf::Path::from_string("/Foo").expect("bad path");
    let cube = Cube::define(&stage, &path);
    assert!(cube.is_valid());

    // size=4
    cube.create_size_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());

    let boundable = Boundable::new(cube.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -2.242468870104182,
                -1.2424688701041822,
                -0.12123443505209108,
            ),
            Vec3f::new(4.242468870104182, 5.242468870104181, 3.121234435052091),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Cylinder
// ============================================================================

#[test]
fn test_cylinder() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let path = usd_sdf::Path::from_string("/Foo").expect("bad path");

    // -- cylinder: height=4, radius=2, axis=X --
    let cylinder = Cylinder::define(&stage, &path);
    assert!(cylinder.is_valid());
    cylinder
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    cylinder
        .create_radius_attr(None, false)
        .set(usd_vt::Value::from(2.0_f64), TimeCode::default_time());
    cylinder.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("X")),
        TimeCode::default_time(),
    );

    let boundable = Boundable::new(cylinder.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -2.242468870104182,
                -1.2424688701041822,
                -0.12123443505209108,
            ),
            Vec3f::new(4.242468870104182, 5.242468870104181, 3.121234435052091),
        ],
        Some(&xform),
    );

    // -- longCylinder: height=6, radius=2, axis=X (reuse same prim) --
    cylinder
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(6.0_f64), TimeCode::default_time());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -2.0, -2.0), Vec3f::new(3.0, 2.0, 2.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -2.0, -2.0), Vec3f::new(3.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -3.0472067242285474,
                -1.7483482335058629,
                -0.27654304381511374,
            ),
            Vec3f::new(5.0472067242285465, 5.7483482335058635, 3.2765430438151135),
        ],
        Some(&xform),
    );

    // -- wideCylinderZ: height=4, radius=3, axis=Z --
    cylinder
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    cylinder
        .create_radius_attr(None, false)
        .set(usd_vt::Value::from(3.0_f64), TimeCode::default_time());
    cylinder.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("Z")),
        TimeCode::default_time(),
    );

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -2.0), Vec3f::new(3.0, 3.0, 2.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -2.0), Vec3f::new(3.0, 3.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-3.3578239417545923, -2.553086087630228, -0.5294827255159541),
            Vec3f::new(5.357823941754592, 6.553086087630229, 3.529482725515954),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Cone
// ============================================================================

#[test]
fn test_cone() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let path = usd_sdf::Path::from_string("/Foo").expect("bad path");

    // -- cone: height=4, radius=2, axis=X --
    let cone = Cone::define(&stage, &path);
    assert!(cone.is_valid());
    cone.create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    cone.create_radius_attr(None, false)
        .set(usd_vt::Value::from(2.0_f64), TimeCode::default_time());
    cone.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("X")),
        TimeCode::default_time(),
    );

    let boundable = Boundable::new(cone.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -2.242468870104182,
                -1.2424688701041822,
                -0.12123443505209108,
            ),
            Vec3f::new(4.242468870104182, 5.242468870104181, 3.121234435052091),
        ],
        Some(&xform),
    );

    // -- longCone: height=6, radius=2, axis=X --
    cone.create_height_attr(None, false)
        .set(usd_vt::Value::from(6.0_f64), TimeCode::default_time());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -2.0, -2.0), Vec3f::new(3.0, 2.0, 2.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -2.0, -2.0), Vec3f::new(3.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(
                -3.0472067242285474,
                -1.7483482335058629,
                -0.27654304381511374,
            ),
            Vec3f::new(5.0472067242285465, 5.7483482335058635, 3.2765430438151135),
        ],
        Some(&xform),
    );

    // -- wideConeZ: height=4, radius=3, axis=Z --
    cone.create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    cone.create_radius_attr(None, false)
        .set(usd_vt::Value::from(3.0_f64), TimeCode::default_time());
    cone.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("Z")),
        TimeCode::default_time(),
    );

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -2.0), Vec3f::new(3.0, 3.0, 2.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -2.0), Vec3f::new(3.0, 3.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-3.3578239417545923, -2.553086087630228, -0.5294827255159541),
            Vec3f::new(5.357823941754592, 6.553086087630229, 3.529482725515954),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Capsule
// ============================================================================

#[test]
fn test_capsule() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");
    let path = usd_sdf::Path::from_string("/Foo").expect("bad path");

    // -- capsule: height=4, radius=2, axis=X --
    let capsule = Capsule::define(&stage, &path);
    assert!(capsule.is_valid());
    capsule
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    capsule
        .create_radius_attr(None, false)
        .set(usd_vt::Value::from(2.0_f64), TimeCode::default_time());
    capsule.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("X")),
        TimeCode::default_time(),
    );

    let boundable = Boundable::new(capsule.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-4.0, -2.0, -2.0), Vec3f::new(4.0, 2.0, 2.0)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-4.0, -2.0, -2.0), Vec3f::new(4.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-3.851944578352912, -2.254227596907543, -0.4318516525781366),
            Vec3f::new(5.851944578352912, 6.254227596907542, 3.4318516525781364),
        ],
        Some(&xform),
    );

    // -- longCapsule: height=6, radius=2, axis=X --
    capsule
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(6.0_f64), TimeCode::default_time());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-5.0, -2.0, -2.0), Vec3f::new(5.0, 2.0, 2.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-5.0, -2.0, -2.0), Vec3f::new(5.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-4.656682432477277, -2.760106960309224, -0.5871602613411594),
            Vec3f::new(6.656682432477277, 6.760106960309223, 3.5871602613411593),
        ],
        Some(&xform),
    );

    // -- wideCapsuleZ: height=4, radius=3, axis=Z --
    capsule
        .create_height_attr(None, false)
        .set(usd_vt::Value::from(4.0_f64), TimeCode::default_time());
    capsule
        .create_radius_attr(None, false)
        .set(usd_vt::Value::from(3.0_f64), TimeCode::default_time());
    capsule.create_axis_attr(None, false).set(
        usd_vt::Value::from(usd_tf::Token::new("Z")),
        TimeCode::default_time(),
    );

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -5.0), Vec3f::new(3.0, 3.0, 5.0)],
        None,
    );

    // Identity
    verify_extent(
        &boundable,
        &[Vec3f::new(-3.0, -3.0, -5.0), Vec3f::new(3.0, 3.0, 5.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-4.875462031959634, -3.4849377402083643, -1.7365895067025015),
            Vec3f::new(6.875462031959634, 7.484937740208365, 4.736589506702502),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_PointInstancer
// ============================================================================

#[test]
fn test_point_instancer() {
    let stage = open_stage("testUsdGeomExtentTransform", "testPointInstancer.usda");

    let path = usd_sdf::Path::from_string("/Instancer").expect("bad path");
    let pi = PointInstancer::get(&stage, &path);
    assert!(pi.is_valid());

    let boundable = Boundable::new(pi.prim().clone());

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-1.0, -1.0, -1.0), Vec3f::new(3.5, 3.5, 3.5)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-1.0, -1.0, -1.0), Vec3f::new(3.5, 3.5, 3.5)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-1.3977774381637573, 0.3787655532360077, 0.689382791519165),
            Vec3f::new(5.12123441696167, 6.12123441696167, 3.560617208480835),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_PointBased
// ============================================================================

#[test]
fn test_point_based() {
    let stage = open_stage("testUsdGeomExtentTransform", "testPointBased.usda");

    let path = usd_sdf::Path::from_string("/Points").expect("bad path");
    let prim = stage.get_prim_at_path(&path).expect("prim not found");
    let boundable = Boundable::new(prim);

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(3.0, 2.0, 2.0)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(3.0, 2.0, 2.0)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(0.3787655532360077, 2.0, 1.5),
            Vec3f::new(4.4259724617004395, 3.609475612640381, 2.0058794021606445),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Points
// ============================================================================

#[test]
fn test_points() {
    let stage = open_stage("testUsdGeomExtentTransform", "testPoints.usda");

    let path = usd_sdf::Path::from_string("/Points").expect("bad path");
    let prim = stage.get_prim_at_path(&path).expect("prim not found");
    let boundable = Boundable::new(prim);

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-0.5, -0.5, -0.5), Vec3f::new(3.25, 2.0, 2.25)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-0.5, -0.5, -0.5), Vec3f::new(3.25, 2.0, 2.25)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(0.18938279151916504, 1.189382791519165, 1.0946913957595825),
            Vec3f::new(4.8312811851501465, 3.609475612640381, 2.041466236114502),
        ],
        Some(&xform),
    );
}

// ============================================================================
// test_Curves
// ============================================================================

#[test]
fn test_curves() {
    let stage = open_stage("testUsdGeomExtentTransform", "testCurves.usda");

    let path = usd_sdf::Path::from_string("/Curves").expect("bad path");
    let prim = stage.get_prim_at_path(&path).expect("prim not found");
    let boundable = Boundable::new(prim);

    // No transform
    verify_extent(
        &boundable,
        &[Vec3f::new(-0.5, -0.5, -0.5), Vec3f::new(3.5, 2.5, 2.5)],
        None,
    );

    // Identity
    let identity = Matrix4d::identity();
    verify_extent(
        &boundable,
        &[Vec3f::new(-0.5, -0.5, -0.5), Vec3f::new(3.5, 2.5, 2.5)],
        Some(&identity),
    );

    // STRANGE_TRANSFORM
    let xform = strange_transform();
    verify_extent(
        &boundable,
        &[
            Vec3f::new(-0.43185165524482727, 1.189382791519165, 1.0946913957595825),
            Vec3f::new(5.236589431762695, 4.420092582702637, 2.4111881256103516),
        ],
        Some(&xform),
    );
}
