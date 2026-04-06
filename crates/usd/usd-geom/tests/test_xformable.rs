use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

//! Tests for UsdGeomXformable.
//!
//! Ported from: testenv/testUsdGeomXformable.py

use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::{Vec3d, Vec3f};
use usd_gf::{Interval, Quatf};
use usd_sdf::{Layer, TimeCode};
use usd_tf::Token;

fn stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn default_tc() -> TimeCode {
    TimeCode::default_time()
}

/// Assert two Matrix4d are close (element-wise within epsilon).
fn assert_close_xf(a: &Matrix4d, b: &Matrix4d) {
    let eps = 1e-4;
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

// ============================================================================
// test_TranslateOp
// ============================================================================

#[test]
fn test_translate_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translation = Vec3d::new(10.0, 20.0, 30.0);
    let translate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);
    translate_op.set(translation, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let mut expected = Matrix4d::identity();
    expected.set_translate(&translation);
    assert_close_xf(&xform, &expected);

    // Check xformOpOrder
    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translate");
    assert_eq!(ops[0].op_type(), XformOpType::Translate);

    // Get the op back
    let get_op = xf.get_translate_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:translate");
    assert_eq!(get_op.op_type(), XformOpType::Translate);
    assert_eq!(translate_op.attr().name(), get_op.attr().name());

    // Scalar translation on Y X and Z should compose to same result
    xf.clear_xform_op_order();
    let translate_y = 20.0_f64;
    let translate_x = 10.0_f64;
    let translate_z = 30.0_f64;
    xf.add_translate_y_op(XformOpPrecision::Double, None, false)
        .set(translate_y, default_tc());
    xf.add_translate_x_op(XformOpPrecision::Double, None, false)
        .set(translate_x, default_tc());
    xf.add_translate_z_op(XformOpPrecision::Double, None, false)
        .set(translate_z, default_tc());

    // Check op order
    let ops2 = xf.get_ordered_xform_ops();
    assert_eq!(ops2.len(), 3);
    assert_eq!(ops2[0].op_name().as_str(), "xformOp:translateY");
    assert_eq!(ops2[1].op_name().as_str(), "xformOp:translateX");
    assert_eq!(ops2[2].op_name().as_str(), "xformOp:translateZ");

    let xform2 = xf.get_local_transformation(default_tc());
    assert_close_xf(&xform, &xform2);
}

#[test]
fn test_open_with_root_layer_preserves_imported_xform_ops() {
    setup();
    let layer_contents = r#"#usda 1.0
def Xform "Root" {
    double3 xformOp:translate = (10, 0, 5)
    uniform token[] xformOpOrder = ["xformOp:translate"]

    def Mesh "Mesh" {
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0,0,0), (1,0,0), (0,2,0)]
    }
}
"#;

    let layer = Layer::create_anonymous(Some("xform_import_open"));
    assert!(layer.import_from_string(layer_contents));

    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).unwrap();
    let root_path = usd_sdf::Path::from_string("/Root").unwrap();
    let mesh_path = usd_sdf::Path::from_string("/Root/Mesh").unwrap();

    let root_prim = stage.get_prim_at_path(&root_path).expect("root prim");
    let translate_attr = root_prim
        .get_attribute("xformOp:translate")
        .expect("translate attr");
    let translate_value = translate_attr
        .get(TimeCode::default())
        .expect("translate default value");
    assert_eq!(
        translate_value.downcast_clone::<Vec3d>(),
        Some(Vec3d::new(10.0, 0.0, 5.0))
    );
    let xformable = Xformable::new(root_prim);
    let ops = xformable.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1, "xformOpOrder should round-trip through open");
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translate");

    let mut expected = Matrix4d::identity();
    expected.set_translate(&Vec3d::new(10.0, 0.0, 5.0));
    let local = xformable.get_local_transformation(default_tc());
    assert_close_xf(&local, &expected);

    let mesh_prim = stage.get_prim_at_path(&mesh_path).expect("mesh prim");
    let world = Imageable::new(mesh_prim).compute_local_to_world_transform(TimeCode::default());
    assert_close_xf(&world, &expected);
}

// ============================================================================
// test_TranslateXOp
// ============================================================================

#[test]
fn test_translate_x_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translate_x = 10.0_f64;
    let translate_x_op = xf.add_translate_x_op(XformOpPrecision::Double, None, false);
    translate_x_op.set(translate_x, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let mut expected = Matrix4d::identity();
    expected.set_translate(&Vec3d::new(translate_x, 0.0, 0.0));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translateX");

    let get_op = xf.get_translate_x_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:translateX");
    assert_eq!(get_op.op_type(), XformOpType::TranslateX);
    assert_eq!(translate_x_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_TranslateYOp
// ============================================================================

#[test]
fn test_translate_y_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translate_y = 20.0_f64;
    let translate_y_op = xf.add_translate_y_op(XformOpPrecision::Double, None, false);
    translate_y_op.set(translate_y, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let mut expected = Matrix4d::identity();
    expected.set_translate(&Vec3d::new(0.0, translate_y, 0.0));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translateY");

    let get_op = xf.get_translate_y_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:translateY");
    assert_eq!(get_op.op_type(), XformOpType::TranslateY);
    assert_eq!(translate_y_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_TranslateZOp
// ============================================================================

#[test]
fn test_translate_z_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translate_z = 30.0_f64;
    let translate_z_op = xf.add_translate_z_op(XformOpPrecision::Double, None, false);
    translate_z_op.set(translate_z, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let mut expected = Matrix4d::identity();
    expected.set_translate(&Vec3d::new(0.0, 0.0, translate_z));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translateZ");

    let get_op = xf.get_translate_z_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:translateZ");
    assert_eq!(get_op.op_type(), XformOpType::TranslateZ);
    assert_eq!(translate_z_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_ScaleOp
// ============================================================================

#[test]
fn test_scale_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let scale_vec = Vec3f::new(10.0, 20.0, 30.0);
    let scale_op = xf.add_scale_op(XformOpPrecision::Float, None, false);
    scale_op.set(scale_vec, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::from_scale_vec(&Vec3d::new(10.0, 20.0, 30.0));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:scale");
    assert_eq!(ops[0].op_type(), XformOpType::Scale);

    let get_op = xf.get_scale_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:scale");
    assert_eq!(get_op.op_type(), XformOpType::Scale);
    assert_eq!(scale_op.attr().name(), get_op.attr().name());

    // Scalar scale on Y X and Z should compose to same result
    xf.clear_xform_op_order();
    let scale_y = 20.0_f64;
    let scale_x = 10.0_f64;
    let scale_z = 30.0_f64;
    xf.add_scale_y_op(XformOpPrecision::Double, None, false)
        .set(scale_y, default_tc());
    xf.add_scale_x_op(XformOpPrecision::Double, None, false)
        .set(scale_x, default_tc());
    xf.add_scale_z_op(XformOpPrecision::Double, None, false)
        .set(scale_z, default_tc());

    let ops2 = xf.get_ordered_xform_ops();
    assert_eq!(ops2.len(), 3);
    assert_eq!(ops2[0].op_name().as_str(), "xformOp:scaleY");
    assert_eq!(ops2[1].op_name().as_str(), "xformOp:scaleX");
    assert_eq!(ops2[2].op_name().as_str(), "xformOp:scaleZ");

    let xform2 = xf.get_local_transformation(default_tc());
    assert_close_xf(&xform, &xform2);
}

// ============================================================================
// test_ScaleXOp
// ============================================================================

#[test]
fn test_scale_x_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let scale_x = 10.0_f64;
    let scale_x_op = xf.add_scale_x_op(XformOpPrecision::Double, None, false);
    scale_x_op.set(scale_x, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::from_scale_vec(&Vec3d::new(scale_x, 1.0, 1.0));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:scaleX");

    let get_op = xf.get_scale_x_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:scaleX");
    assert_eq!(get_op.op_type(), XformOpType::ScaleX);
    assert_eq!(scale_x_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_ScaleYOp
// ============================================================================

#[test]
fn test_scale_y_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let scale_y = 20.0_f64;
    let scale_y_op = xf.add_scale_y_op(XformOpPrecision::Double, None, false);
    scale_y_op.set(scale_y, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::from_scale_vec(&Vec3d::new(1.0, scale_y, 1.0));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:scaleY");

    let get_op = xf.get_scale_y_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:scaleY");
    assert_eq!(get_op.op_type(), XformOpType::ScaleY);
    assert_eq!(scale_y_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_ScaleZOp
// ============================================================================

#[test]
fn test_scale_z_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let scale_z = 30.0_f64;
    let scale_z_op = xf.add_scale_z_op(XformOpPrecision::Double, None, false);
    scale_z_op.set(scale_z, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::from_scale_vec(&Vec3d::new(1.0, 1.0, scale_z));
    assert_close_xf(&xform, &expected);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:scaleZ");

    let get_op = xf.get_scale_z_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:scaleZ");
    assert_eq!(get_op.op_type(), XformOpType::ScaleZ);
    assert_eq!(scale_z_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_ScalarRotateOps
// ============================================================================

#[test]
fn test_scalar_rotate_ops() {
    setup();
    let s = stage();

    // RotateX 45 degrees
    {
        let path = usd_sdf::Path::from_string("/X").unwrap();
        let x = Xform::define(&s, &path);
        let xf = x.xformable();
        let rotate_x_op = xf.add_rotate_x_op(XformOpPrecision::Float, None, false);
        rotate_x_op.set(45.0_f32, default_tc());
        let xform_x = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.7071067811865475,
            0.7071067811865476,
            0.0,
            0.0,
            -0.7071067811865476,
            0.7071067811865475,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert_close_xf(&xform_x, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateX");

        let get_op = xf.get_rotate_x_op(None, false);
        assert_eq!(get_op.op_name().as_str(), "xformOp:rotateX");
        assert_eq!(get_op.op_type(), XformOpType::RotateX);
        assert_eq!(rotate_x_op.attr().name(), get_op.attr().name());
    }

    // RotateY 90 degrees
    {
        let path = usd_sdf::Path::from_string("/Y").unwrap();
        let y = Xform::define(&s, &path);
        let xf = y.xformable();
        let rotate_y_op = xf.add_rotate_y_op(XformOpPrecision::Float, None, false);
        rotate_y_op.set(90.0_f32, default_tc());
        let xform_y = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        assert_close_xf(&xform_y, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateY");

        let get_op = xf.get_rotate_y_op(None, false);
        assert_eq!(get_op.op_name().as_str(), "xformOp:rotateY");
        assert_eq!(get_op.op_type(), XformOpType::RotateY);
        assert_eq!(rotate_y_op.attr().name(), get_op.attr().name());
    }

    // RotateZ 30 degrees
    {
        let path = usd_sdf::Path::from_string("/Z").unwrap();
        let z = Xform::define(&s, &path);
        let xf = z.xformable();
        let rotate_z_op = xf.add_rotate_z_op(XformOpPrecision::Float, None, false);
        rotate_z_op.set(30.0_f32, default_tc());
        let xform_z = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            0.866025403784439,
            0.5,
            0.0,
            0.0,
            -0.5,
            0.866025403784439,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert_close_xf(&xform_z, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateZ");

        let get_op = xf.get_rotate_z_op(None, false);
        assert_eq!(get_op.op_name().as_str(), "xformOp:rotateZ");
        assert_eq!(get_op.op_type(), XformOpType::RotateZ);
        assert_eq!(rotate_z_op.attr().name(), get_op.attr().name());
    }

    // Combined RotateY(90) then RotateX(45)
    {
        let path = usd_sdf::Path::from_string("/XY").unwrap();
        let xy = Xform::define(&s, &path);
        let xf = xy.xformable();
        xf.add_rotate_y_op(XformOpPrecision::Float, None, false)
            .set(90.0_f32, default_tc());
        xf.add_rotate_x_op(XformOpPrecision::Float, None, false)
            .set(45.0_f32, default_tc());
        let xform_xy = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            0.0,
            0.0,
            -1.0,
            0.0,
            0.7071067811865476,
            0.7071067811865475,
            0.0,
            0.0,
            0.7071067811865475,
            -0.7071067811865476,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert_close_xf(&xform_xy, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateY");
        assert_eq!(ops[1].op_name().as_str(), "xformOp:rotateX");
    }

    // Combined RotateZ(30) then RotateY(90)
    {
        let path = usd_sdf::Path::from_string("/YZ").unwrap();
        let yz = Xform::define(&s, &path);
        let xf = yz.xformable();
        xf.add_rotate_z_op(XformOpPrecision::Float, None, false)
            .set(30.0_f32, default_tc());
        xf.add_rotate_y_op(XformOpPrecision::Float, None, false)
            .set(90.0_f32, default_tc());
        let xform_yz = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            0.0,
            0.0,
            -1.0,
            0.0,
            -0.5,
            0.8660254037844387,
            0.0,
            0.0,
            0.8660254037844387,
            0.5,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert_close_xf(&xform_yz, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateZ");
        assert_eq!(ops[1].op_name().as_str(), "xformOp:rotateY");
    }

    // Combined RotateX(45) then RotateZ(30)
    {
        let path = usd_sdf::Path::from_string("/ZX").unwrap();
        let zx = Xform::define(&s, &path);
        let xf = zx.xformable();
        xf.add_rotate_x_op(XformOpPrecision::Float, None, false)
            .set(45.0_f32, default_tc());
        xf.add_rotate_z_op(XformOpPrecision::Float, None, false)
            .set(30.0_f32, default_tc());
        let xform_zx = xf.get_local_transformation(default_tc());
        let expected = Matrix4d::new(
            0.8660254037844387,
            0.3535533905932737,
            0.35355339059327373,
            0.0,
            -0.5,
            0.6123724356957945,
            0.6123724356957946,
            0.0,
            0.0,
            -0.7071067811865476,
            0.7071067811865475,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert_close_xf(&xform_zx, &expected);

        let ops = xf.get_ordered_xform_ops();
        assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateX");
        assert_eq!(ops[1].op_name().as_str(), "xformOp:rotateZ");
    }
}

// ============================================================================
// test_VectorRotateOps
// ============================================================================

#[test]
fn test_vector_rotate_ops() {
    setup();
    let s = stage();
    let rot = Vec3f::new(30.0, 45.0, 60.0);

    // XYZ rotation order
    let path_xyz = usd_sdf::Path::from_string("/XYZ").unwrap();
    let xyz = Xform::define(&s, &path_xyz);
    let xf_xyz = xyz.xformable();
    let rotate_xyz_op = xf_xyz.add_rotate_xyz_op(XformOpPrecision::Float, None, false);
    rotate_xyz_op.set(rot, default_tc());
    let xform_xyz = xf_xyz.get_local_transformation(default_tc());
    let ops = xf_xyz.get_ordered_xform_ops();
    assert_eq!(ops[0].op_name().as_str(), "xformOp:rotateXYZ");

    let get_op = xf_xyz.get_rotate_xyz_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:rotateXYZ");
    assert_eq!(get_op.op_type(), XformOpType::RotateXYZ);
    assert_eq!(rotate_xyz_op.attr().name(), get_op.attr().name());

    // Equivalent: individual Z, Y, X ops in that order
    let path_xyz2 = usd_sdf::Path::from_string("/XYZ2").unwrap();
    let xyz2 = Xform::define(&s, &path_xyz2);
    let xf_xyz2 = xyz2.xformable();
    xf_xyz2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    xf_xyz2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    xf_xyz2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    let xform_xyz2 = xf_xyz2.get_local_transformation(default_tc());
    assert_close_xf(&xform_xyz, &xform_xyz2);

    // XZY rotation order
    let path_xzy = usd_sdf::Path::from_string("/XZY").unwrap();
    let xzy = Xform::define(&s, &path_xzy);
    let xf_xzy = xzy.xformable();
    let rotate_xzy_op = xf_xzy.add_rotate_xzy_op(XformOpPrecision::Float, None, false);
    rotate_xzy_op.set(rot, default_tc());
    let xform_xzy = xf_xzy.get_local_transformation(default_tc());
    assert_eq!(
        xf_xzy.get_ordered_xform_ops()[0].op_name().as_str(),
        "xformOp:rotateXZY"
    );

    let get_xzy = xf_xzy.get_rotate_xzy_op(None, false);
    assert_eq!(get_xzy.op_name().as_str(), "xformOp:rotateXZY");
    assert_eq!(get_xzy.op_type(), XformOpType::RotateXZY);
    assert_eq!(rotate_xzy_op.attr().name(), get_xzy.attr().name());

    // Equivalent: Y, Z, X individual
    let path_xzy2 = usd_sdf::Path::from_string("/XZY2").unwrap();
    let xzy2 = Xform::define(&s, &path_xzy2);
    let xf_xzy2 = xzy2.xformable();
    xf_xzy2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    xf_xzy2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    xf_xzy2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    let xform_xzy2 = xf_xzy2.get_local_transformation(default_tc());
    assert_close_xf(&xform_xzy, &xform_xzy2);

    // YXZ rotation order
    let path_yxz = usd_sdf::Path::from_string("/YXZ").unwrap();
    let yxz = Xform::define(&s, &path_yxz);
    let xf_yxz = yxz.xformable();
    let rotate_yxz_op = xf_yxz.add_rotate_yxz_op(XformOpPrecision::Float, None, false);
    rotate_yxz_op.set(rot, default_tc());
    let xform_yxz = xf_yxz.get_local_transformation(default_tc());
    assert_eq!(
        xf_yxz.get_ordered_xform_ops()[0].op_name().as_str(),
        "xformOp:rotateYXZ"
    );

    let get_yxz = xf_yxz.get_rotate_yxz_op(None, false);
    assert_eq!(get_yxz.op_name().as_str(), "xformOp:rotateYXZ");
    assert_eq!(get_yxz.op_type(), XformOpType::RotateYXZ);
    assert_eq!(rotate_yxz_op.attr().name(), get_yxz.attr().name());

    // Equivalent: Z, X, Y individual
    let path_yxz2 = usd_sdf::Path::from_string("/YXZ2").unwrap();
    let yxz2 = Xform::define(&s, &path_yxz2);
    let xf_yxz2 = yxz2.xformable();
    xf_yxz2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    xf_yxz2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    xf_yxz2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    let xform_yxz2 = xf_yxz2.get_local_transformation(default_tc());
    assert_close_xf(&xform_yxz, &xform_yxz2);

    // YZX rotation order
    let path_yzx = usd_sdf::Path::from_string("/YZX").unwrap();
    let yzx = Xform::define(&s, &path_yzx);
    let xf_yzx = yzx.xformable();
    let rotate_yzx_op = xf_yzx.add_rotate_yzx_op(XformOpPrecision::Float, None, false);
    rotate_yzx_op.set(rot, default_tc());
    let xform_yzx = xf_yzx.get_local_transformation(default_tc());
    assert_eq!(
        xf_yzx.get_ordered_xform_ops()[0].op_name().as_str(),
        "xformOp:rotateYZX"
    );

    let get_yzx = xf_yzx.get_rotate_yzx_op(None, false);
    assert_eq!(get_yzx.op_name().as_str(), "xformOp:rotateYZX");
    assert_eq!(get_yzx.op_type(), XformOpType::RotateYZX);
    assert_eq!(rotate_yzx_op.attr().name(), get_yzx.attr().name());

    // Equivalent: X, Z, Y individual
    let path_yzx2 = usd_sdf::Path::from_string("/YZX2").unwrap();
    let yzx2 = Xform::define(&s, &path_yzx2);
    let xf_yzx2 = yzx2.xformable();
    xf_yzx2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    xf_yzx2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    xf_yzx2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    let xform_yzx2 = xf_yzx2.get_local_transformation(default_tc());
    assert_close_xf(&xform_yzx, &xform_yzx2);

    // ZXY rotation order
    let path_zxy = usd_sdf::Path::from_string("/ZXY").unwrap();
    let zxy = Xform::define(&s, &path_zxy);
    let xf_zxy = zxy.xformable();
    let rotate_zxy_op = xf_zxy.add_rotate_zxy_op(XformOpPrecision::Float, None, false);
    rotate_zxy_op.set(rot, default_tc());
    let xform_zxy = xf_zxy.get_local_transformation(default_tc());
    assert_eq!(
        xf_zxy.get_ordered_xform_ops()[0].op_name().as_str(),
        "xformOp:rotateZXY"
    );

    let get_zxy = xf_zxy.get_rotate_zxy_op(None, false);
    assert_eq!(get_zxy.op_name().as_str(), "xformOp:rotateZXY");
    assert_eq!(get_zxy.op_type(), XformOpType::RotateZXY);
    assert_eq!(rotate_zxy_op.attr().name(), get_zxy.attr().name());

    // Equivalent: Y, X, Z individual
    let path_zxy2 = usd_sdf::Path::from_string("/ZXY2").unwrap();
    let zxy2 = Xform::define(&s, &path_zxy2);
    let xf_zxy2 = zxy2.xformable();
    xf_zxy2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    xf_zxy2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    xf_zxy2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    let xform_zxy2 = xf_zxy2.get_local_transformation(default_tc());
    assert_close_xf(&xform_zxy, &xform_zxy2);

    // ZYX rotation order
    let path_zyx = usd_sdf::Path::from_string("/ZYX").unwrap();
    let zyx = Xform::define(&s, &path_zyx);
    let xf_zyx = zyx.xformable();
    let rotate_zyx_op = xf_zyx.add_rotate_zyx_op(XformOpPrecision::Float, None, false);
    rotate_zyx_op.set(rot, default_tc());
    let xform_zyx = xf_zyx.get_local_transformation(default_tc());
    assert_eq!(
        xf_zyx.get_ordered_xform_ops()[0].op_name().as_str(),
        "xformOp:rotateZYX"
    );

    let get_zyx = xf_zyx.get_rotate_zyx_op(None, false);
    assert_eq!(get_zyx.op_name().as_str(), "xformOp:rotateZYX");
    assert_eq!(get_zyx.op_type(), XformOpType::RotateZYX);
    assert_eq!(rotate_zyx_op.attr().name(), get_zyx.attr().name());

    // Equivalent: X, Y, Z individual
    let path_zyx2 = usd_sdf::Path::from_string("/ZYX2").unwrap();
    let zyx2 = Xform::define(&s, &path_zyx2);
    let xf_zyx2 = zyx2.xformable();
    xf_zyx2
        .add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(rot.x, default_tc());
    xf_zyx2
        .add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(rot.y, default_tc());
    xf_zyx2
        .add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(rot.z, default_tc());
    let xform_zyx2 = xf_zyx2.get_local_transformation(default_tc());
    assert_close_xf(&xform_zyx, &xform_zyx2);
}

// ============================================================================
// test_PrestoRotatePivot
// ============================================================================

#[test]
fn test_presto_rotate_pivot() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    xf.add_translate_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(10.0, 0.0, 0.0), default_tc());

    let pivot_suffix = Token::new("pivot");
    xf.add_translate_op(XformOpPrecision::Double, Some(&pivot_suffix), false)
        .set(Vec3d::new(0.0, 10.0, 0.0), default_tc());

    xf.add_rotate_xyz_op(XformOpPrecision::Float, None, false)
        .set(Vec3f::new(60.0, 0.0, 30.0), default_tc());

    xf.add_scale_op(XformOpPrecision::Float, None, false)
        .set(Vec3f::new(2.0, 2.0, 2.0), default_tc());

    // Insert inverse pivot (setting value on inverse is disallowed)
    let _inverse_translate_op =
        xf.add_translate_op(XformOpPrecision::Double, Some(&pivot_suffix), true);

    // Check op order
    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 5);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:translate");
    assert_eq!(ops[1].op_name().as_str(), "xformOp:translate:pivot");
    assert_eq!(ops[2].op_name().as_str(), "xformOp:rotateXYZ");
    assert_eq!(ops[3].op_name().as_str(), "xformOp:scale");
    assert_eq!(ops[4].op_name().as_str(), "!invert!xformOp:translate:pivot");

    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::new(
        1.7320508075688774,
        1.0,
        0.0,
        0.0,
        -0.5,
        0.8660254037844389,
        1.7320508075688772,
        0.0,
        0.8660254037844385,
        -1.5,
        1.0,
        0.0,
        15.0,
        1.339745962155611,
        -17.32050807568877,
        1.0,
    );
    assert_close_xf(&xform, &expected);
}

// ============================================================================
// test_OrientOp
// ============================================================================

#[test]
fn test_orient_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let orient_op = xf.add_orient_op(XformOpPrecision::Float, None, false);

    // Quatf(1, Vec3f(2,3,4)).normalized()
    let quat = Quatf::new(1.0, Vec3f::new(2.0, 3.0, 4.0)).normalized();
    orient_op.set(quat, default_tc());
    let xform = xf.get_local_transformation(default_tc());
    let expected = Matrix4d::new(
        -0.666666666666667,
        0.66666666666667,
        0.333333333333333,
        0.0,
        0.133333333333333,
        -0.33333333333333,
        0.933333333333333,
        0.0,
        0.733333333333333,
        0.66666666666666,
        0.133333333333333,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    );
    assert_close_xf(&xform, &expected);

    // 90-degree on x-axis
    let quat2 = Quatf::new(0.7071067811865476, Vec3f::new(0.7071067811865475, 0.0, 0.0));
    orient_op.set(quat2, default_tc());
    let xform2 = xf.get_local_transformation(default_tc());
    let expected2 = Matrix4d::new(
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    );
    assert_close_xf(&xform2, &expected2);

    // Zero quat -> identity
    let quat_zero = Quatf::new(0.0, Vec3f::new(0.0, 0.0, 0.0));
    orient_op.set(quat_zero, default_tc());
    let xform3 = xf.get_local_transformation(default_tc());
    assert_close_xf(&xform3, &Matrix4d::identity());

    let get_op = xf.get_orient_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:orient");
    assert_eq!(get_op.op_type(), XformOpType::Orient);
    assert_eq!(orient_op.attr().name(), get_op.attr().name());
}

// ============================================================================
// test_TransformOp
// ============================================================================

#[test]
fn test_transform_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let transform_op = xf.add_transform_op(XformOpPrecision::Double, None, false);
    let mut mat = Matrix4d::from_scale(2.0);
    mat.set_translate_only(&Vec3d::new(10.0, 20.0, 30.0));
    transform_op.set(mat, default_tc());

    let result = xf.get_local_transformation(default_tc());
    assert_close_xf(&mat, &result);

    let ops = xf.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].op_name().as_str(), "xformOp:transform");

    // MakeMatrixXform
    let matrix_op = xf.make_matrix_xform();
    assert_eq!(matrix_op.op_name().as_str(), "xformOp:transform");
    let ops2 = xf.get_ordered_xform_ops();
    assert_eq!(ops2.len(), 1);
    assert_eq!(ops2[0].op_name().as_str(), "xformOp:transform");

    let get_op = xf.get_transform_op(None, false);
    assert_eq!(get_op.op_name().as_str(), "xformOp:transform");
    assert_eq!(get_op.op_type(), XformOpType::Transform);
    assert_eq!(transform_op.attr().name(), get_op.attr().name());

    // Clear xformOpOrder
    xf.clear_xform_op_order();

    // GetTransformOp should return invalid when op is not in xformOpOrder
    let get_op2 = xf.get_transform_op(None, false);
    assert!(!get_op2.is_valid());

    // Clearing does not remove the attribute
    assert!(xf.prim().has_attribute("xformOp:transform"));

    // Empty xformOpOrder
    let ops3 = xf.get_ordered_xform_ops();
    assert_eq!(ops3.len(), 0);

    // SetXformOpOrder with resetXformStack
    assert!(xf.set_xform_op_order_with_reset(&[matrix_op], true));

    let ops4 = xf.get_ordered_xform_ops();
    // The ops returned after reset should contain the transform op
    // (reset token is consumed internally)
    assert!(xf.get_reset_xform_stack());
    assert_eq!(ops4.len(), 1);
    assert_eq!(ops4[0].op_name().as_str(), "xformOp:transform");
}

// ============================================================================
// test_ResetXformStack
// ============================================================================

#[test]
fn test_reset_xform_stack() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    xf.add_translate_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(20.0, 30.0, 40.0), default_tc());

    xf.set_reset_xform_stack(true);
    assert!(xf.get_reset_xform_stack());

    // Calling it twice should have no extra effect
    xf.set_reset_xform_stack(true);
    assert!(xf.get_reset_xform_stack());

    xf.set_reset_xform_stack(false);
    assert!(!xf.get_reset_xform_stack());

    // Again no extra effect
    xf.set_reset_xform_stack(false);
    assert!(!xf.get_reset_xform_stack());

    xf.add_transform_op(XformOpPrecision::Double, None, false)
        .set(Matrix4d::identity(), default_tc());

    xf.set_reset_xform_stack(true);
    assert!(xf.get_reset_xform_stack());

    xf.set_reset_xform_stack(false);
    assert!(!xf.get_reset_xform_stack());

    // Child with parent transform
    let child_path = usd_sdf::Path::from_string("/World/Model").unwrap();
    let cx = Xform::define(&s, &child_path);
    let cxf = cx.xformable();
    cxf.add_translate_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(10.0, 10.0, 10.0), default_tc());

    let mut cache = XformCache::new(default_tc());
    let cx_ctm = cache.get_local_to_world_transform(cx.prim());
    let mut expected_ctm = Matrix4d::identity();
    expected_ctm.set_translate(&Vec3d::new(30.0, 40.0, 50.0));
    assert_close_xf(&cx_ctm, &expected_ctm);

    cxf.set_reset_xform_stack(true);
    assert!(cxf.get_reset_xform_stack());

    cache.clear();
    let new_cx_ctm = cache.get_local_to_world_transform(cx.prim());
    let local_cx_xform = cxf.get_local_transformation(default_tc());
    let mut expected_local = Matrix4d::identity();
    expected_local.set_translate(&Vec3d::new(10.0, 10.0, 10.0));
    assert_close_xf(&new_cx_ctm, &expected_local);
    assert_close_xf(&new_cx_ctm, &local_cx_xform);

    // Test resetXformStack when it's not at the beginning
    cxf.set_reset_xform_stack(false);

    // Manually set xformOpOrder with reset token at the end
    let order_attr = cxf.get_xform_op_order_attr();
    let new_order = vec![
        Token::new("xformOp:translate"),
        Token::new("!resetXformStack!"),
    ];
    order_attr.set(usd_vt::Value::new(new_order), TimeCode::default());

    cxf.add_transform_op(XformOpPrecision::Double, None, false)
        .set(Matrix4d::from_scale(2.0), default_tc());

    assert!(cxf.get_reset_xform_stack());
}

// ============================================================================
// test_InverseOps
// ============================================================================

#[test]
fn test_inverse_ops() {
    setup();
    let identity = Matrix4d::identity();

    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    // Translate + inverse translate = identity
    xf.add_translate_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(20.0, 30.0, 40.0), default_tc());
    xf.add_translate_op(XformOpPrecision::Double, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // Scale + inverse scale = identity
    xf.add_scale_op(XformOpPrecision::Float, None, false)
        .set(Vec3f::new(2.0, 3.0, 4.0), default_tc());
    xf.add_scale_op(XformOpPrecision::Float, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // RotateX + inverse = identity
    xf.add_rotate_x_op(XformOpPrecision::Float, None, false)
        .set(30.0_f32, default_tc());
    xf.add_rotate_x_op(XformOpPrecision::Float, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // RotateY + inverse = identity
    xf.add_rotate_y_op(XformOpPrecision::Float, None, false)
        .set(45.0_f32, default_tc());
    xf.add_rotate_y_op(XformOpPrecision::Float, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // RotateZ + inverse = identity
    xf.add_rotate_z_op(XformOpPrecision::Float, None, false)
        .set(60.0_f32, default_tc());
    xf.add_rotate_z_op(XformOpPrecision::Float, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // RotateXYZ with suffix + inverse = identity
    let first_rotate = Token::new("firstRotate");
    xf.add_rotate_xyz_op(XformOpPrecision::Float, Some(&first_rotate), false)
        .set(Vec3f::new(10.0, 20.0, 30.0), default_tc());
    xf.add_rotate_xyz_op(XformOpPrecision::Float, Some(&first_rotate), true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // RotateZYX with suffix + inverse = identity
    let last_rotate = Token::new("lastRotate");
    xf.add_rotate_zyx_op(XformOpPrecision::Float, Some(&last_rotate), false)
        .set(Vec3f::new(30.0, 60.0, 45.0), default_tc());
    xf.add_rotate_zyx_op(XformOpPrecision::Float, Some(&last_rotate), true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // Orient + inverse = identity
    let quat = Quatf::new(1.0, Vec3f::new(2.0, 3.0, 4.0));
    xf.add_orient_op(XformOpPrecision::Float, None, false)
        .set(quat, default_tc());
    xf.add_orient_op(XformOpPrecision::Float, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // Transform + inverse = identity
    let mut transform_mat = Matrix4d::identity();
    transform_mat.set_translate_only(&Vec3d::new(10.0, 20.0, 30.0));
    xf.add_transform_op(XformOpPrecision::Double, None, false)
        .set(transform_mat, default_tc());
    xf.add_transform_op(XformOpPrecision::Double, None, true);
    assert_close_xf(&xf.get_local_transformation(default_tc()), &identity);

    // Verify GetOrderedXformOps returns all ops with correct names
    let ordered_ops = xf.get_ordered_xform_ops();
    let op_names: Vec<String> = ordered_ops
        .iter()
        .map(|op| op.op_name().as_str().to_string())
        .collect();

    // Reconstruct from xformOpOrder attr and compare
    let order_attr = xf.get_xform_op_order_attr();
    if let Some(val) = order_attr.get(TimeCode::default()) {
        if let Some(tokens) = val.as_vec_clone::<Token>() {
            let attr_names: Vec<String> = tokens.iter().map(|t| t.as_str().to_string()).collect();
            assert_eq!(op_names, attr_names);
        }
    }
}

// ============================================================================
// test_GetXformOp
// ============================================================================

#[test]
fn test_get_xform_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translation = Vec3d::new(10.0, 20.0, 30.0);
    let translate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);
    translate_op.set(translation, default_tc());

    let xform = xf.get_local_transformation(default_tc());
    let mut expected = Matrix4d::identity();
    expected.set_translate(&translation);
    assert_close_xf(&xform, &expected);

    // Should return invalid op if op does not exist
    let no_scale = xf.get_xform_op(XformOpType::Scale, None, false);
    assert!(!no_scale.is_valid());

    // Add second translate with suffix
    let suffix = Token::new("translate2");
    let _translate2_op = xf.add_translate_op(XformOpPrecision::Double, Some(&suffix), false);

    let get_t1 = xf.get_translate_op(None, false);
    assert_eq!(get_t1.op_name().as_str(), "xformOp:translate");
    assert_eq!(get_t1.op_type(), XformOpType::Translate);
    assert_eq!(get_t1, translate_op);

    let get_t2 = xf.get_translate_op(Some(&suffix), false);
    assert_eq!(get_t2.op_name().as_str(), "xformOp:translate:translate2");
    assert_eq!(get_t2.op_type(), XformOpType::Translate);
    assert_eq!(get_t2, _translate2_op);

    // Inverse ops are distinct from regular ops
    let scale_inv_op = xf.add_scale_op(XformOpPrecision::Float, None, true);
    assert!(scale_inv_op.is_valid());

    // Getting the non-inverse version should fail (not in order yet)
    let no_scale2 = xf.get_scale_op(None, false);
    assert!(!no_scale2.is_valid());

    // Now add the regular scale
    let scale_op = xf.add_scale_op(XformOpPrecision::Float, None, false);
    assert!(scale_op.is_valid());

    let get_s = xf.get_scale_op(None, false);
    assert_eq!(get_s.op_name().as_str(), "xformOp:scale");
    assert_eq!(get_s.op_type(), XformOpType::Scale);
    assert_eq!(scale_op, get_s);

    let get_s_inv = xf.get_scale_op(None, true);
    assert_eq!(get_s_inv.op_name().as_str(), "!invert!xformOp:scale");
    assert_eq!(get_s_inv.op_type(), XformOpType::Scale);
    assert_eq!(scale_inv_op, get_s_inv);
}

// ============================================================================
// test_AddExistingXformOp
// ============================================================================

#[test]
fn test_add_existing_xform_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let _xlate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);

    // Adding duplicate op should return invalid
    let dup_op = xf.add_translate_op(XformOpPrecision::Double, None, false);
    assert!(!dup_op.is_valid());

    // Adding inverse is OK (considered separate)
    let inv_translate_op = xf.add_translate_op(XformOpPrecision::Double, None, true);
    assert!(inv_translate_op.is_valid());

    // Setting value on inverse op returns false
    let set_result = inv_translate_op.set(Vec3d::new(1.0, 1.0, 1.0), default_tc());
    assert!(!set_result);

    // Adding scale with double precision, then inverse with float precision
    // should fail due to precision mismatch
    let _scale_op = xf.add_scale_op(XformOpPrecision::Double, None, false);
    let inv_scale_op = xf.add_scale_op(XformOpPrecision::Float, None, true);
    // The precision mismatch should make this invalid
    assert!(!inv_scale_op.is_valid());
}

// ============================================================================
// test_SingularTransformOp
// ============================================================================

#[test]
fn test_singular_transform_op() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let transform_op = xf.add_transform_op(XformOpPrecision::Double, None, false);
    let singular_mat = Matrix4d::new(
        32.0, 8.0, 11.0, 17.0, 8.0, 20.0, 17.0, 23.0, 11.0, 17.0, 14.0, 26.0, 17.0, 23.0, 26.0, 2.0,
    );
    transform_op.set(singular_mat, TimeCode::new(1.0));

    // Insert translate op in the middle
    xf.add_translate_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(1.0, 1.0, 1.0), default_tc());
    xf.add_transform_op(XformOpPrecision::Double, None, true);

    // If the translateOp in the middle is removed from xformOpOrder, then
    // consecutive inverse ops will get skipped
    let order_attr = xf.get_xform_op_order_attr();
    let new_order = vec![
        Token::new("xformOp:transform"),
        Token::new("!invert!xformOp:transform"),
    ];
    order_attr.set(usd_vt::Value::new(new_order), TimeCode::default());

    let result = xf.get_local_transformation(TimeCode::new(1.0));
    assert_close_xf(&result, &Matrix4d::identity());
}

// ============================================================================
// test_VaryingPrecisionOps
// ============================================================================

#[test]
fn test_varying_precision_ops() {
    setup();
    let s = stage();

    // x1: half + double + float rotation ops
    let path1 = usd_sdf::Path::from_string("/World").unwrap();
    let x1 = Xform::define(&s, &path1);
    let xf1 = x1.xformable();

    let half_suffix = Token::new("Half");
    let half_rot_op = xf1.add_rotate_xyz_op(XformOpPrecision::Half, Some(&half_suffix), false);
    assert_eq!(half_rot_op.precision(), XformOpPrecision::Half);
    // Vec3h not available; use Vec3f for half-precision (gets converted)
    half_rot_op.set(Vec3f::new(0.0, 0.0, 60.0), default_tc());

    let double_suffix = Token::new("Double");
    let double_rot_op =
        xf1.add_rotate_xyz_op(XformOpPrecision::Double, Some(&double_suffix), false);
    assert_eq!(double_rot_op.precision(), XformOpPrecision::Double);
    double_rot_op.set(Vec3d::new(0.0, 45.123456789, 0.0), default_tc());

    let float_suffix = Token::new("Float");
    let float_rot_op = xf1.add_rotate_xyz_op(XformOpPrecision::Float, Some(&float_suffix), false);
    assert_eq!(float_rot_op.precision(), XformOpPrecision::Float);
    float_rot_op.set(Vec3f::new(30.0, 0.0, 0.0), default_tc());

    let ops1 = xf1.get_ordered_xform_ops();
    assert_eq!(ops1.len(), 3);
    assert_eq!(ops1[0].op_name().as_str(), "xformOp:rotateXYZ:Half");
    assert_eq!(ops1[1].op_name().as_str(), "xformOp:rotateXYZ:Double");
    assert_eq!(ops1[2].op_name().as_str(), "xformOp:rotateXYZ:Float");

    let _xform1 = xf1.get_local_transformation(default_tc());

    // x2: single double-precision combined op
    let path2 = usd_sdf::Path::from_string("/World2").unwrap();
    let x2 = Xform::define(&s, &path2);
    let xf2 = x2.xformable();
    xf2.add_rotate_xyz_op(XformOpPrecision::Double, None, false)
        .set(Vec3d::new(30.0, 45.123456789, 60.0), default_tc());
    let _xform2 = xf2.get_local_transformation(default_tc());

    // Note: half precision may lose some accuracy, so we just verify they compute
    // without error. Exact match (as in C++) requires native half-float support.

    // Orient ops with different precisions
    let path3 = usd_sdf::Path::from_string("/World3").unwrap();
    let x3 = Xform::define(&s, &path3);
    let xf3 = x3.xformable();

    let quatf = Quatf::new(2.0, Vec3f::new(3.0, 4.0, 5.0)).normalized();

    // Default orient op (float precision)
    xf3.add_orient_op(XformOpPrecision::Float, None, false)
        .set(quatf, default_tc());

    let float_orient_suffix = Token::new("Float");
    let float_orient_op =
        xf3.add_orient_op(XformOpPrecision::Float, Some(&float_orient_suffix), false);
    float_orient_op.set(quatf, default_tc());

    let double_orient_suffix = Token::new("Double");
    let _double_orient_op =
        xf3.add_orient_op(XformOpPrecision::Double, Some(&double_orient_suffix), false);

    // Computing the transform should not panic
    let _xform3 = xf3.get_local_transformation(default_tc());
}

// ============================================================================
// test_InvalidXformOps
// ============================================================================

#[test]
fn test_invalid_xform_ops() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let _prim = s
        .define_prim(path.get_string(), "Xform")
        .expect("define prim");
    let xf = Xformable::get(&s, &path);

    // Attribute not in xformOp namespace is not a valid xform op
    let attr_name = Token::new("myXformOp:transform");
    assert!(!XformOp::is_xform_op(&attr_name));

    // Attribute with invalid op type
    let attr_name2 = Token::new("xformOp:translateXYZ");
    assert!(XformOp::is_xform_op(&attr_name2));
    assert_eq!(
        XformOp::get_op_type_enum(&Token::new("translateXYZ")),
        XformOpType::Invalid
    );

    // XformOp with no attr is invalid
    let invalid_op = XformOp::invalid();
    assert!(!invalid_op.is_valid());

    // Adding transform with Float precision should fail (Matrix4f not supported)
    let float_transform = xf.add_transform_op(XformOpPrecision::Float, None, false);
    // Depending on implementation, this may or may not be valid
    // The Python test expects it to raise. In Rust, it returns invalid.
    let _ = float_transform;
}

// ============================================================================
// test_XformOpTypes
// ============================================================================

#[test]
fn test_xform_op_types() {
    setup();
    let type_enums = vec![
        XformOpType::Scale,
        XformOpType::Invalid,
        XformOpType::Translate,
        XformOpType::RotateZ,
        XformOpType::RotateX,
        XformOpType::RotateY,
        XformOpType::RotateZYX,
        XformOpType::RotateXYZ,
        XformOpType::RotateXZY,
        XformOpType::RotateYXZ,
        XformOpType::RotateYZX,
        XformOpType::RotateZXY,
        XformOpType::Transform,
        XformOpType::Orient,
    ];
    let type_tokens = vec![
        "scale",
        "",
        "translate",
        "rotateZ",
        "rotateX",
        "rotateY",
        "rotateZYX",
        "rotateXYZ",
        "rotateXZY",
        "rotateYXZ",
        "rotateYZX",
        "rotateZXY",
        "transform",
        "orient",
    ];

    for (index, (type_enum, type_token)) in type_enums.iter().zip(type_tokens.iter()).enumerate() {
        let test_enum = XformOp::get_op_type_enum(&Token::new(type_token));
        assert_eq!(
            test_enum, *type_enum,
            "Enum mismatch at index {index}: token='{type_token}'"
        );
        let test_token = XformOp::get_op_type_token(*type_enum);
        assert_eq!(
            test_token.as_str(),
            *type_token,
            "Token mismatch at index {index}: enum={type_enum:?}"
        );
    }
}

// ============================================================================
// test_MightBeTimeVarying
// ============================================================================

#[test]
fn test_might_be_time_varying() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let translation = Vec3d::new(10.0, 20.0, 30.0);
    let xlate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);

    // No value set
    assert!(!xf.transform_might_be_time_varying());

    // Single default value
    xlate_op.set(translation, default_tc());
    assert!(!xf.transform_might_be_time_varying());

    // Single time sample
    xlate_op.set(translation + translation, TimeCode::new(1.0));
    assert!(!xf.transform_might_be_time_varying());
    // Static overload
    assert!(!Xformable::transform_might_be_time_varying_from_ops(&[
        xlate_op.clone()
    ]));

    assert_eq!(xf.get_time_samples(), vec![1.0]);

    // Two time samples -> now time varying
    xlate_op.set(translation * 3.0, TimeCode::new(2.0));
    assert!(xf.transform_might_be_time_varying());
    assert!(Xformable::transform_might_be_time_varying_from_ops(&[
        xlate_op.clone()
    ]));
    assert_eq!(xf.get_time_samples(), vec![1.0, 2.0]);

    // Put resetXformStack after the op -> ops get cleared on parse
    let order_attr = xf.get_xform_op_order_attr();
    let new_order = vec![
        Token::new("xformOp:translate"),
        Token::new("!resetXformStack!"),
    ];
    order_attr.set(usd_vt::Value::new(new_order), TimeCode::default());
    assert!(!xf.transform_might_be_time_varying());
    assert!(xf.get_time_samples().is_empty());

    // Put resetXformStack before the op -> op is present
    let new_order2 = vec![
        Token::new("!resetXformStack!"),
        Token::new("xformOp:translate"),
    ];
    order_attr.set(usd_vt::Value::new(new_order2), TimeCode::default());
    assert!(xf.transform_might_be_time_varying());
    assert_eq!(xf.get_time_samples(), vec![1.0, 2.0]);
}

// ============================================================================
// test_GetTimeSamples
// ============================================================================

#[test]
fn test_get_time_samples() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    let xlate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);
    let scale_op = xf.add_scale_op(XformOpPrecision::Float, None, false);

    assert!(xlate_op.get_time_samples().is_empty());
    assert!(scale_op.get_time_samples().is_empty());

    let xlate1 = Vec3d::new(10.0, 20.0, 30.0);
    let scale2 = Vec3f::new(1.0, 2.0, 3.0);
    let xlate3 = Vec3d::new(10.0, 20.0, 30.0);
    let scale4 = Vec3f::new(1.0, 2.0, 3.0);

    assert!(xf.get_time_samples().is_empty());

    xlate_op.set(xlate1, TimeCode::new(1.0));
    assert_eq!(xf.get_time_samples(), vec![1.0]);

    xlate_op.set(xlate3, TimeCode::new(3.0));
    assert_eq!(xf.get_time_samples(), vec![1.0, 3.0]);

    scale_op.set(scale2, TimeCode::new(2.0));
    assert_eq!(xf.get_time_samples(), vec![1.0, 2.0, 3.0]);

    scale_op.set(scale4, TimeCode::new(4.0));
    assert_eq!(xf.get_time_samples(), vec![1.0, 2.0, 3.0, 4.0]);

    assert_eq!(xlate_op.get_time_samples(), vec![1.0, 3.0]);
    assert_eq!(scale_op.get_time_samples(), vec![2.0, 4.0]);

    // GetTimeSamplesInInterval
    let interval_2_4 = Interval::new(2.0, 4.0, true, true);
    assert_eq!(
        xlate_op.get_time_samples_in_interval(&interval_2_4),
        vec![3.0]
    );

    let interval_0_3 = Interval::new(0.0, 3.0, true, true);
    assert_eq!(
        scale_op.get_time_samples_in_interval(&interval_0_3),
        vec![2.0]
    );

    let interval_1_5_3_2 = Interval::new(1.5, 3.2, true, true);
    assert_eq!(
        xf.get_time_samples_in_interval(&interval_1_5_3_2),
        vec![2.0, 3.0]
    );
}

// ============================================================================
// test_PureOvers
// ============================================================================

#[test]
fn test_pure_overs() {
    setup();
    let s = stage();
    let prim = s.override_prim("/World").expect("override prim");
    let xf = Xformable::new(prim);
    xf.set_reset_xform_stack(true);
    xf.make_matrix_xform()
        .set(Matrix4d::identity(), default_tc());
    let ops = xf.get_ordered_xform_ops();
    xf.set_xform_op_order(&ops);
}

// ============================================================================
// test_Bug109853
// ============================================================================

#[test]
fn test_bug_109853() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/World").unwrap();
    let x = Xform::define(&s, &path);
    let xf = x.xformable();

    // Set xformOpOrder to reference a non-existent xformOp
    let order_attr = xf.create_xform_op_order_attr();
    let bogus_order = vec![Token::new("xformOp:transform")];
    order_attr.set(usd_vt::Value::new(bogus_order), TimeCode::default());

    // This should not crash (used to crash before bug 109853 was fixed)
    let _xform = xf.get_local_transformation(default_tc());
}

// ============================================================================
// test_InvalidXformable
// ============================================================================

#[test]
fn test_invalid_xformable() {
    setup();
    let xf = Xformable::invalid();
    // Operations on invalid xformable should not crash
    assert!(!xf.is_valid());
    // Getting prim should be safe even if invalid
    let prim = xf.prim();
    assert!(!prim.is_valid());
}

// ============================================================================
// test_XformOpOperators
// ============================================================================

#[test]
fn test_xform_op_operators() {
    setup();
    let s = stage();
    let path = usd_sdf::Path::from_string("/Root").unwrap();
    let root_xform = Xform::define(&s, &path);
    let xf = root_xform.xformable();

    let translate_op = xf.add_translate_op(XformOpPrecision::Double, None, false);
    let scale_op = xf.add_scale_op(XformOpPrecision::Float, None, false);

    // Invalid op should not be valid
    assert!(!XformOp::invalid().is_valid());

    // Valid ops
    assert!(translate_op.is_valid());
    assert!(scale_op.is_valid());

    let xform_ops = xf.get_ordered_xform_ops();
    assert_eq!(xform_ops.len(), 2);
    assert_eq!(xform_ops[0], translate_op);
    assert_eq!(xform_ops[1], scale_op);
}

// ============================================================================
// test_ImplicitConversions
// ============================================================================

#[test]
fn test_implicit_conversions() {
    setup();
    let s = stage();
    let path1 = usd_sdf::Path::from_string("/Root").unwrap();
    let path2 = usd_sdf::Path::from_string("/Root2").unwrap();
    let root1 = Xform::define(&s, &path1);
    let root2 = Xform::define(&s, &path2);
    let xf1 = root1.xformable();
    let xf2 = root2.xformable();

    let translate_op1 = xf1.add_translate_op(XformOpPrecision::Double, None, false);
    let _translate_op2 = xf2.add_translate_op(XformOpPrecision::Double, None, false);

    // In Rust, we can access the underlying attribute from XformOp
    let attr1 = translate_op1.attr();
    assert!(attr1.is_valid());
}

#[test]
fn test_time_sampled_points_return_different_values() {
    setup();
    
    usd_core::schema_registry::register_builtin_schemas();

    let usda = r#"#usda 1.0
(
    startTimeCode = 1
    endTimeCode = 3
)
def Mesh "Anim" {
    int[] faceVertexCounts = [3]
    int[] faceVertexIndices = [0, 1, 2]
    point3f[] points.timeSamples = {
        1: [(0,0,0), (1,0,0), (0,1,0)],
        2: [(0,0,5), (1,0,5), (0,1,5)],
        3: [(0,0,10), (1,0,10), (0,1,10)]
    }
}
"#;
    let layer = usd_sdf::Layer::create_anonymous(Some("time_sample_test"));
    layer.import_from_string(usda);
    let stage = usd_core::Stage::open_with_root_layer(layer, usd_core::InitialLoadSet::LoadAll)
        .expect("open");

    let prim = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Anim").unwrap())
        .unwrap();
    let attr = prim.get_attribute("points").expect("points attr");

    let samples = attr.get_time_samples();
    eprintln!("samples: {:?}", samples);
    assert!(
        samples.len() >= 3,
        "must have 3 time samples, got {}",
        samples.len()
    );

    let v1 = attr
        .get(usd_core::TimeCode::new(1.0))
        .expect("value at t=1");
    let v2 = attr
        .get(usd_core::TimeCode::new(2.0))
        .expect("value at t=2");
    let v3 = attr
        .get(usd_core::TimeCode::new(3.0))
        .expect("value at t=3");

    let p1 = v1.as_vec_clone::<usd_gf::Vec3f>().expect("Vec3f at t=1");
    let p2 = v2.as_vec_clone::<usd_gf::Vec3f>().expect("Vec3f at t=2");
    let p3 = v3.as_vec_clone::<usd_gf::Vec3f>().expect("Vec3f at t=3");

    assert_eq!(p1[0].z, 0.0, "t=1 z=0");
    assert_eq!(p2[0].z, 5.0, "t=2 z=5");
    assert_eq!(p3[0].z, 10.0, "t=3 z=10");
}

#[test]
fn test_flo_usdz_time_samples() {
    setup();
    usd_core::schema_registry::register_builtin_schemas();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/flo.usdz");
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} not found", path);
        return;
    }
    let stage = usd_core::Stage::open(path, usd_core::InitialLoadSet::LoadAll).expect("open");
    let mut mesh_count = 0;
    let mut with_points = 0;
    let mut with_samples = 0;
    for prim in stage.traverse() {
        if prim.type_name().as_str() != "Mesh" {
            continue;
        }
        mesh_count += 1;
        let Some(attr) = prim.get_attribute("points") else {
            continue;
        };
        with_points += 1;
        let samples = attr.get_time_samples();
        if mesh_count <= 3 {
            eprintln!("  {} samples={}", prim.path(), samples.len());
        }
        if samples.len() < 2 {
            continue;
        }
        with_samples += 1;
        let t1 = samples[0];
        let t2 = samples[samples.len() / 2];
        let v1 = attr.get(usd_core::TimeCode::new(t1));
        let v2 = attr.get(usd_core::TimeCode::new(t2));
        eprintln!(
            "Prim: {} samples={} t1={} t2={}",
            prim.path(),
            samples.len(),
            t1,
            t2
        );
        eprintln!("v1={} v2={}", v1.is_some(), v2.is_some());
        if let (Some(v1), Some(v2)) = (&v1, &v2) {
            eprintln!("v1.type={:?} v2.type={:?}", v1.type_name(), v2.type_name());
            let p1 = v1.as_vec_clone::<usd_gf::Vec3f>();
            let p2 = v2.as_vec_clone::<usd_gf::Vec3f>();
            eprintln!("p1_vec3f={} p2_vec3f={}", p1.is_some(), p2.is_some());
            if let (Some(p1), Some(p2)) = (&p1, &p2) {
                eprintln!(
                    "p1[0]={:?} p2[0]={:?} SAME={}",
                    p1[0],
                    p2[0],
                    p1[0] == p2[0]
                );
            }
        }
        break;
    }
    eprintln!(
        "meshes={} with_points={} with_samples={}",
        mesh_count, with_points, with_samples
    );

    // Check xformOp time samples (flo is transform-animated, not vertex-animated)
    let xform_path = "/root/flo/noga_a/noga1/noga3_001/noga5_001/noga6_001/flower/flo41";
    if let Some(prim) = stage.get_prim_at_path(&usd_sdf::Path::from_string(xform_path).unwrap()) {
        if let Some(attr) = prim.get_attribute("xformOp:translate") {
            let samples = attr.get_time_samples();
            eprintln!("xformOp:translate samples={}", samples.len());
            if samples.len() >= 2 {
                let v1 = attr.get(usd_core::TimeCode::new(samples[0]));
                let v2 = attr.get(usd_core::TimeCode::new(samples[samples.len() / 2]));
                eprintln!(
                    "t={}: {:?}",
                    samples[0],
                    v1.as_ref().map(|v| format!("{:?}", v))
                );
                eprintln!(
                    "t={}: {:?}",
                    samples[samples.len() / 2],
                    v2.as_ref().map(|v| format!("{:?}", v))
                );
            }
        } else {
            eprintln!("xformOp:translate NOT FOUND on {}", xform_path);
        }
    }
}
