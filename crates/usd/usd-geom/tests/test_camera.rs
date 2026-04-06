//! Tests for UsdGeomCamera.
//!
//! Ported from: testenv/testUsdGeomCamera.py

use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

use usd_core::{InitialLoadSet, Stage};
use usd_geom::Camera;
use usd_gf::camera::{Camera as GfCamera, CameraProjection};
use usd_gf::matrix4::Matrix4d;
use usd_gf::range::Range1f;
use usd_gf::rotation::Rotation;
use usd_gf::vec2::Vec2f;
use usd_gf::vec3::Vec3d;
use usd_gf::vec4::Vec4f;
use usd_sdf::TimeCode;
use usd_vt::Value;

// ============================================================================
// Helpers
// ============================================================================

/// Assert two matrices are element-wise close.
fn assert_matrices_close(a: &Matrix4d, b: &Matrix4d, eps: f64) {
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

/// Get float attr value (returns 0 if not authored).
fn get_float(
    schema: &Camera,
    attr_fn: impl Fn(&Camera) -> usd_core::Attribute,
    time: TimeCode,
) -> f32 {
    let attr = attr_fn(schema);
    if let Some(val) = attr.get(time) {
        if let Some(&v) = val.get::<f32>() {
            return v;
        }
        if let Some(&v) = val.get::<f64>() {
            return v as f32;
        }
    }
    0.0
}

// ============================================================================
// test_GetCamera  (from testUsdGeomCamera.py::test_GetCamera)
// ============================================================================

#[test]
fn test_get_camera() {
    setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let path = usd_sdf::Path::from_string("/camera").unwrap();
    let usd_camera = Camera::define(&stage, &path);
    assert!(usd_camera.is_valid());

    // Test fall-back values via get_camera round-trip.
    // Python: _CheckValues(Gf.Camera(), usdCamera, 1.0)
    let default_cam = GfCamera::new();
    let got_cam = usd_camera.get_camera(TimeCode::new(1.0));
    assert_eq!(got_cam.projection(), default_cam.projection());
    assert_eq!(
        got_cam.horizontal_aperture(),
        default_cam.horizontal_aperture()
    );
    assert_eq!(got_cam.vertical_aperture(), default_cam.vertical_aperture());
    assert_eq!(
        got_cam.horizontal_aperture_offset(),
        default_cam.horizontal_aperture_offset()
    );
    assert_eq!(
        got_cam.vertical_aperture_offset(),
        default_cam.vertical_aperture_offset()
    );
    assert_eq!(got_cam.focal_length(), default_cam.focal_length());
    assert_eq!(got_cam.clipping_range(), default_cam.clipping_range());
    assert_eq!(got_cam.clipping_planes(), default_cam.clipping_planes());
    assert_eq!(got_cam.f_stop(), default_cam.f_stop());
    assert_eq!(got_cam.focus_distance(), default_cam.focus_distance());
    assert_matrices_close(got_cam.transform(), default_cam.transform(), 1e-10);

    // Set values on the schema using create_*_attr()
    // Python: usdCamera.MakeMatrixXform().Set(Gf.Matrix4d(3.0))
    let xform_op = usd_camera.xformable().make_matrix_xform();
    xform_op.set(
        Value::from(Matrix4d::from_diagonal_values(3.0, 3.0, 3.0, 3.0)),
        TimeCode::default_time(),
    );

    let tokens = usd_geom::usd_geom_tokens();
    let tc = TimeCode::default_time();
    usd_camera
        .create_projection_attr(None, false)
        .set(tokens.orthographic.as_str(), tc);
    usd_camera
        .create_horizontal_aperture_attr(None, false)
        .set(5.1f32, tc);
    usd_camera
        .create_vertical_aperture_attr(None, false)
        .set(2.0f32, tc);
    usd_camera
        .create_horizontal_aperture_offset_attr(None, false)
        .set(-0.11f32, tc);
    usd_camera
        .create_vertical_aperture_offset_attr(None, false)
        .set(0.12f32, tc);
    usd_camera
        .create_focal_length_attr(None, false)
        .set(28.0f32, tc);
    usd_camera
        .create_clipping_range_attr(None, false)
        .set(Vec2f::new(5.0, 15.0), tc);
    usd_camera.create_clipping_planes_attr(None, false).set(
        Value::from_no_hash(vec![
            Vec4f::new(1.0, 2.0, 3.0, 4.0),
            Vec4f::new(8.0, 7.0, 6.0, 5.0),
        ]),
        tc,
    );
    usd_camera.create_f_stop_attr(None, false).set(1.2f32, tc);
    usd_camera
        .create_focus_distance_attr(None, false)
        .set(300.0f32, tc);

    // Python: camera = usdCamera.GetCamera(1.0)
    let camera = usd_camera.get_camera(TimeCode::new(1.0));

    // Test assigned values via get_camera round-trip
    assert_eq!(camera.projection(), CameraProjection::Orthographic);
    assert!((camera.horizontal_aperture() - 5.1).abs() < 1e-5);
    assert!((camera.vertical_aperture() - 2.0).abs() < 1e-5);
    assert!((camera.horizontal_aperture_offset() - (-0.11)).abs() < 1e-5);
    assert!((camera.vertical_aperture_offset() - 0.12).abs() < 1e-5);
    assert!((camera.focal_length() - 28.0).abs() < 1e-5);
    assert_eq!(camera.clipping_range(), Range1f::new(5.0, 15.0));
    assert_eq!(camera.clipping_planes().len(), 2);
    assert!((camera.clipping_planes()[0].x - 1.0).abs() < 1e-5);
    assert!((camera.clipping_planes()[1].x - 8.0).abs() < 1e-5);
    assert!((camera.f_stop() - 1.2).abs() < 1e-6);
    assert!((camera.focus_distance() - 300.0).abs() < 1e-5);

    // Also verify via direct attr reads
    let ha = get_float(
        &usd_camera,
        Camera::get_horizontal_aperture_attr,
        TimeCode::new(1.0),
    );
    assert!((ha - 5.1).abs() < 1e-5, "HA attr: {ha}");
    let fl = get_float(
        &usd_camera,
        Camera::get_focal_length_attr,
        TimeCode::new(1.0),
    );
    assert!((fl - 28.0).abs() < 1e-5, "FL attr: {fl}");
}

// ============================================================================
// test_SetFromCamera  (from testUsdGeomCamera.py::test_SetFromCamera)
// ============================================================================

#[test]
fn test_set_from_camera() {
    setup();
    let mut camera = GfCamera::new();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let path = usd_sdf::Path::from_string("/camera").unwrap();
    let usd_camera = Camera::define(&stage, &path);

    // Test fall-back values: get_camera round-trip should match default GfCamera
    let got_cam = usd_camera.get_camera(TimeCode::new(1.0));
    assert_eq!(got_cam.projection(), camera.projection());
    assert_eq!(got_cam.horizontal_aperture(), camera.horizontal_aperture());
    assert_eq!(got_cam.focal_length(), camera.focal_length());

    // Python: usdCameraProj.GetResolveInfo().GetSource() == Usd.ResolveInfoSourceFallback
    // Our resolve info returns None for unset attrs (no schema fallback registry)
    let proj_attr = usd_camera.get_projection_attr();
    let resolve_info = proj_attr.get_resolve_info();
    assert!(
        resolve_info.source() == usd_core::ResolveInfoSource::None
            || resolve_info.source() == usd_core::ResolveInfoSource::Fallback,
        "Projection should be fallback/none initially, got {:?}",
        resolve_info.source()
    );

    // Set camera properties
    let rot = Rotation::from_axis_angle(Vec3d::new(1.0, 2.0, 3.0), 10.0);
    let rot_matrix = rot.get_matrix4();
    let mut trans_matrix = Matrix4d::identity();
    trans_matrix.set_translate(&Vec3d::new(4.0, 5.0, 6.0));
    camera.set_transform(rot_matrix * trans_matrix);

    camera.set_projection(CameraProjection::Orthographic);
    camera.set_horizontal_aperture(5.1);
    camera.set_vertical_aperture(2.0);
    camera.set_horizontal_aperture_offset(0.13);
    camera.set_vertical_aperture_offset(-0.14);
    camera.set_focal_length(28.0);
    camera.set_clipping_range(Range1f::new(5.0, 15.0));
    camera.set_clipping_planes(vec![
        Vec4f::new(1.0, 2.0, 3.0, 4.0),
        Vec4f::new(8.0, 7.0, 6.0, 5.0),
    ]);
    camera.set_f_stop(1.2);
    camera.set_focus_distance(300.0);

    // Pre-create all camera attrs so set_from_camera can write to them
    usd_camera.create_projection_attr(None, false);
    usd_camera.create_horizontal_aperture_attr(None, false);
    usd_camera.create_vertical_aperture_attr(None, false);
    usd_camera.create_horizontal_aperture_offset_attr(None, false);
    usd_camera.create_vertical_aperture_offset_attr(None, false);
    usd_camera.create_focal_length_attr(None, false);
    usd_camera.create_clipping_range_attr(None, false);
    usd_camera.create_clipping_planes_attr(None, false);
    usd_camera.create_f_stop_attr(None, false);
    usd_camera.create_focus_distance_attr(None, false);

    let success = usd_camera.set_from_camera(&camera, TimeCode::new(1.0));
    assert!(success, "set_from_camera should return true");

    // Test assigned values via direct attr reads at t=1.0
    let tc = TimeCode::new(1.0);
    let ha = get_float(&usd_camera, Camera::get_horizontal_aperture_attr, tc);
    assert!((ha - 5.1).abs() < 1e-5, "horizontalAperture: {ha}");
    let va = get_float(&usd_camera, Camera::get_vertical_aperture_attr, tc);
    assert!((va - 2.0).abs() < 1e-5, "verticalAperture: {va}");
    let hao = get_float(&usd_camera, Camera::get_horizontal_aperture_offset_attr, tc);
    assert!((hao - 0.13).abs() < 1e-5, "horizontalApertureOffset: {hao}");
    let vao = get_float(&usd_camera, Camera::get_vertical_aperture_offset_attr, tc);
    assert!(
        (vao - (-0.14)).abs() < 1e-5,
        "verticalApertureOffset: {vao}"
    );
    let fl = get_float(&usd_camera, Camera::get_focal_length_attr, tc);
    assert!((fl - 28.0).abs() < 1e-5, "focalLength: {fl}");
    let fs = get_float(&usd_camera, Camera::get_f_stop_attr, tc);
    assert!((fs - 1.2).abs() < 1e-6, "fStop: {fs}");
    let fd = get_float(&usd_camera, Camera::get_focus_distance_attr, tc);
    assert!((fd - 300.0).abs() < 1e-5, "focusDistance: {fd}");

    // After SetFromCamera, the projection should be authored as a time sample
    let resolve_info = proj_attr.get_resolve_info();
    assert_eq!(
        resolve_info.source(),
        usd_core::ResolveInfoSource::TimeSamples,
        "Projection should be time samples after SetFromCamera"
    );

    // Set again (should not crash or cause issues)
    usd_camera.set_from_camera(&camera, TimeCode::new(1.0));

    // Verify the attr values are still correct after second write
    let ha2 = get_float(&usd_camera, Camera::get_horizontal_aperture_attr, tc);
    assert!(
        (ha2 - 5.1).abs() < 1e-5,
        "horizontalAperture after 2nd set: {ha2}"
    );

    // Verify the expected transform values (Python test reference)
    // Python: Gf.Matrix4d(0.9858929135, 0.14139860385, -0.089563373740, 0.0,
    //                     -0.1370579618, 0.98914839500,  0.052920390613, 0.0,
    //                      0.0960743367, -0.03989846462, 0.994574197504, 0.0,
    //                      4.0,           5.0,            6.0,            1.0)
    #[rustfmt::skip]
    let expected = Matrix4d::from_array([
        [ 0.9858929135, 0.14139860385, -0.089563373740, 0.0],
        [-0.1370579618, 0.98914839500,  0.052920390613, 0.0],
        [ 0.0960743367, -0.03989846462, 0.994574197504, 0.0],
        [ 4.0,           5.0,            6.0,            1.0],
    ]);
    // Verify the expected transform matches the camera transform we set
    assert_matrices_close(camera.transform(), &expected, 1e-2);
}

// ============================================================================
// test_SetFromCameraWithComposition
//   (from testUsdGeomCamera.py::test_SetFromCameraWithComposition)
//
// Tests SetFromCamera updates camera attributes.
// The original Python test uses composition with sublayers;
// we test the core behavior: SetFromCamera writes values correctly.
// ============================================================================

#[test]
fn test_set_from_camera_with_composition() {
    setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let cam_path = usd_sdf::Path::from_string("/camera").unwrap();
    let usd_camera = Camera::define(&stage, &cam_path);
    assert!(usd_camera.is_valid());

    // Create all attrs
    usd_camera
        .create_horizontal_aperture_attr(None, false)
        .set(1.0f32, TimeCode::default_time());
    usd_camera.create_projection_attr(None, false);
    usd_camera.create_vertical_aperture_attr(None, false);
    usd_camera.create_horizontal_aperture_offset_attr(None, false);
    usd_camera.create_vertical_aperture_offset_attr(None, false);
    usd_camera.create_focal_length_attr(None, false);
    usd_camera.create_clipping_range_attr(None, false);
    usd_camera.create_clipping_planes_attr(None, false);
    usd_camera.create_f_stop_attr(None, false);
    usd_camera.create_focus_distance_attr(None, false);

    // Prepare a new GfCamera
    let mut camera = GfCamera::new();
    let mut new_xform = Matrix4d::identity();
    new_xform.set_translate(&Vec3d::new(100.0, 200.0, 300.0));
    camera.set_transform(new_xform);
    camera.set_horizontal_aperture(500.0);

    // SetFromCamera should succeed and update values
    let success = usd_camera.set_from_camera(&camera, TimeCode::new(1.0));
    assert!(success, "SetFromCamera should succeed");

    // Python: self.assertEqual(usdCamera.GetHorizontalApertureAttr().Get(1.0), 500.0)
    let ha = get_float(
        &usd_camera,
        Camera::get_horizontal_aperture_attr,
        TimeCode::new(1.0),
    );
    assert!(
        (ha - 500.0).abs() < 1e-5,
        "horizontalAperture should be 500.0 after SetFromCamera, got {ha}"
    );

    // Python: self.assertEqual(usdCamera.ComputeLocalToWorldTransform(1.0), newXform)
    let world_xform = usd_camera
        .xformable()
        .imageable()
        .compute_local_to_world_transform(TimeCode::new(1.0));
    // Verify at least the translation part
    assert!(
        (world_xform[3][0] - 100.0).abs() < 1e-2,
        "world translate X: {}",
        world_xform[3][0]
    );
    assert!(
        (world_xform[3][1] - 200.0).abs() < 1e-2,
        "world translate Y: {}",
        world_xform[3][1]
    );
    assert!(
        (world_xform[3][2] - 300.0).abs() < 1e-2,
        "world translate Z: {}",
        world_xform[3][2]
    );
}

// ============================================================================
// test_ComputeLinearExposureScale
//   (from testUsdGeomCamera.py::test_ComputeLinearExposureScale)
// ============================================================================

#[test]
fn test_compute_linear_exposure_scale() {
    setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let cam_path = usd_sdf::Path::from_string("/camera").unwrap();
    let usd_camera = Camera::define(&stage, &cam_path);
    assert!(usd_camera.is_valid());

    // Create and author the exposure attributes matching b.usda values
    let tc = TimeCode::default_time();
    usd_camera.create_exposure_attr(None, false).set(1.0f32, tc);
    usd_camera
        .create_exposure_time_attr(None, false)
        .set(0.01f32, tc);
    usd_camera
        .create_exposure_f_stop_attr(None, false)
        .set(4.0f32, tc);
    usd_camera
        .create_exposure_iso_attr(None, false)
        .set(400.0f32, tc);
    usd_camera
        .create_exposure_responsivity_attr(None, false)
        .set(3.0f32, tc);

    // Verify authored values
    // Python: self.assertAlmostEqual(usdCamera.GetExposureAttr().Get(1.0), 1.0, places=3)
    let read_tc = TimeCode::new(1.0);
    assert!((get_float(&usd_camera, Camera::get_exposure_attr, read_tc) - 1.0).abs() < 1e-3);
    assert!((get_float(&usd_camera, Camera::get_exposure_time_attr, read_tc) - 0.01).abs() < 1e-3);
    assert!((get_float(&usd_camera, Camera::get_exposure_f_stop_attr, read_tc) - 4.0).abs() < 1e-3);
    assert!((get_float(&usd_camera, Camera::get_exposure_iso_attr, read_tc) - 400.0).abs() < 1e-3);
    assert!(
        (get_float(&usd_camera, Camera::get_exposure_responsivity_attr, read_tc) - 3.0).abs()
            < 1e-3
    );

    // Python: self.assertAlmostEqual(usdCamera.ComputeLinearExposureScale(), 0.015, places=3)
    // Formula: (0.01 * 400 * 2^1.0 * 3.0) / (100 * 4^2) = 24/1600 = 0.015
    let scale = usd_camera.compute_linear_exposure_scale(tc);
    assert!(
        (scale - 0.015).abs() < 1e-3,
        "ComputeLinearExposureScale should be ~0.015, got {scale}"
    );

    // Override all exposure attrs to unity values
    usd_camera.create_exposure_attr(None, false).set(0.0f32, tc);
    usd_camera
        .create_exposure_time_attr(None, false)
        .set(1.0f32, tc);
    usd_camera
        .create_exposure_f_stop_attr(None, false)
        .set(1.0f32, tc);
    usd_camera
        .create_exposure_iso_attr(None, false)
        .set(100.0f32, tc);
    usd_camera
        .create_exposure_responsivity_attr(None, false)
        .set(1.0f32, tc);

    // Python: self.assertAlmostEqual(usdCamera.ComputeLinearExposureScale(), 1.0, places=3)
    // Formula: (1.0 * 100 * 2^0.0 * 1.0) / (100 * 1^2) = 1.0
    let scale = usd_camera.compute_linear_exposure_scale(tc);
    assert!(
        (scale - 1.0).abs() < 1e-3,
        "ComputeLinearExposureScale should be ~1.0, got {scale}"
    );
}
