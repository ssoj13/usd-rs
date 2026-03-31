//! Tests for attribute time sample interpolation.
//! Ported from C++ testUsdAttributeInterpolationCpp.cpp.
//!
//! Pattern: create in-memory stage, author 2 time samples at t=0 and t=2,
//! verify interpolated value at t=1 under both Linear and Held modes.

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_core::interpolation::InterpolationType;
use usd_sdf::TimeCode;
use usd_vt::Value;

// ============================================================================
// Helpers
// ============================================================================

/// Create an in-memory stage and a test prim at /TestPrim.
fn make_stage() -> std::sync::Arc<Stage> {
    common::setup();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage")
}

/// Get prim, panicking if not found.
#[allow(dead_code)]
fn get_prim(stage: &Stage, path: &str) -> usd_core::Prim {
    stage
        .get_prim_at_path(&usd_sdf::Path::from_string(path).unwrap())
        .expect("prim exists")
}

/// Verify attribute value at given time matches expected.
fn verify<T: PartialEq + std::fmt::Debug + Clone + 'static>(
    attr: &usd_core::Attribute,
    time: f64,
    expected: &T,
) {
    let val = attr.get(TimeCode::new(time)).expect(&format!(
        "attr {} at t={} should have value",
        attr.path(),
        time
    ));
    let got = val.get::<T>().expect(&format!(
        "attr {} at t={}: wrong type, expected {}",
        attr.path(),
        time,
        std::any::type_name::<T>()
    ));
    assert_eq!(
        got,
        expected,
        "attr {} at t={}: got {:?}, expected {:?}",
        attr.path(),
        time,
        got,
        expected
    );
}

/// Verify f64 attribute value approximately matches expected.
fn verify_near(attr: &usd_core::Attribute, time: f64, expected: f64) {
    let val = attr.get(TimeCode::new(time)).expect("has value");
    let got = *val.get::<f64>().expect("is f64");
    assert!(
        (got - expected).abs() < 1e-6,
        "attr {} at t={}: got {}, expected {}",
        attr.path(),
        time,
        got,
        expected
    );
}

/// Verify f32 attribute value approximately matches expected.
fn verify_near_f32(attr: &usd_core::Attribute, time: f64, expected: f32) {
    let val = attr.get(TimeCode::new(time)).expect("has value");
    let got = *val.get::<f32>().expect("is f32");
    assert!(
        (got - expected).abs() < 1e-4,
        "attr {} at t={}: got {}, expected {}",
        attr.path(),
        time,
        got,
        expected
    );
}

// ============================================================================
// 1. Non-interpolatable scalar types — always held
// ============================================================================

#[test]
fn interp_bool_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testBool", &common::vtn("bool"), false, None)
        .unwrap();
    attr.set(Value::from(true), TimeCode::new(0.0));
    attr.set(Value::from(false), TimeCode::new(2.0));

    // Linear mode — bool does not interpolate
    stage.set_interpolation_type(InterpolationType::Linear);
    verify(&attr, 0.0, &true);
    verify(&attr, 1.0, &true); // held at lower
    verify(&attr, 2.0, &false);

    // Held mode
    stage.set_interpolation_type(InterpolationType::Held);
    verify(&attr, 0.0, &true);
    verify(&attr, 1.0, &true);
    verify(&attr, 2.0, &false);
}

#[test]
fn interp_string_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testString", &common::vtn("string"), false, None)
        .unwrap();
    attr.set(Value::from("s1".to_string()), TimeCode::new(0.0));
    attr.set(Value::from("s2".to_string()), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    verify(&attr, 0.0, &"s1".to_string());
    verify(&attr, 1.0, &"s1".to_string());
    verify(&attr, 2.0, &"s2".to_string());

    stage.set_interpolation_type(InterpolationType::Held);
    verify(&attr, 0.0, &"s1".to_string());
    verify(&attr, 1.0, &"s1".to_string());
    verify(&attr, 2.0, &"s2".to_string());
}

#[test]
fn interp_token_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testToken", &common::vtn("token"), false, None)
        .unwrap();
    attr.set(Value::from(usd_tf::Token::new("s1")), TimeCode::new(0.0));
    attr.set(Value::from(usd_tf::Token::new("s2")), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    verify(&attr, 0.0, &usd_tf::Token::new("s1"));
    verify(&attr, 1.0, &usd_tf::Token::new("s1"));
    verify(&attr, 2.0, &usd_tf::Token::new("s2"));
}

#[test]
fn interp_int_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testInt", &common::vtn("int"), false, None)
        .unwrap();
    attr.set(Value::from(0i32), TimeCode::new(0.0));
    attr.set(Value::from(2i32), TimeCode::new(2.0));

    // C++: int does NOT linearly interpolate — falls back to held
    stage.set_interpolation_type(InterpolationType::Linear);
    verify(&attr, 0.0, &0i32);
    verify(&attr, 1.0, &0i32);
    verify(&attr, 2.0, &2i32);

    stage.set_interpolation_type(InterpolationType::Held);
    verify(&attr, 0.0, &0i32);
    verify(&attr, 1.0, &0i32);
    verify(&attr, 2.0, &2i32);
}

// ============================================================================
// 2. Interpolatable scalar types — linear interpolation
// ============================================================================

#[test]
fn interp_double_linear() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDouble", &common::vtn("double"), false, None)
        .unwrap();
    attr.set(Value::from(0.0f64), TimeCode::new(0.0));
    attr.set(Value::from(2.0f64), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    verify_near(&attr, 0.0, 0.0);
    verify_near(&attr, 1.0, 1.0); // linearly interpolated
    verify_near(&attr, 2.0, 2.0);
}

#[test]
fn interp_double_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDouble", &common::vtn("double"), false, None)
        .unwrap();
    attr.set(Value::from(0.0f64), TimeCode::new(0.0));
    attr.set(Value::from(2.0f64), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Held);
    verify_near(&attr, 0.0, 0.0);
    verify_near(&attr, 1.0, 0.0); // held = lower value
    verify_near(&attr, 2.0, 2.0);
}

#[test]
fn interp_float_linear() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testFloat", &common::vtn("float"), false, None)
        .unwrap();
    attr.set(Value::from(0.0f32), TimeCode::new(0.0));
    attr.set(Value::from(2.0f32), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    verify_near_f32(&attr, 0.0, 0.0);
    verify_near_f32(&attr, 1.0, 1.0);
    verify_near_f32(&attr, 2.0, 2.0);
}

#[test]
fn interp_float_held() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testFloat", &common::vtn("float"), false, None)
        .unwrap();
    attr.set(Value::from(0.0f32), TimeCode::new(0.0));
    attr.set(Value::from(2.0f32), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Held);
    verify_near_f32(&attr, 0.0, 0.0);
    verify_near_f32(&attr, 1.0, 0.0);
    verify_near_f32(&attr, 2.0, 2.0);
}

// ============================================================================
// 3. Vec types — linear interpolation for float/double, held for int
// ============================================================================

#[test]
fn interp_vec3d_linear() {
    use usd_gf::Vec3d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testVec3d", &common::vtn("double3"), false, None)
        .unwrap();
    attr.set(Value::from(Vec3d::new(0.0, 0.0, 0.0)), TimeCode::new(0.0));
    attr.set(Value::from(Vec3d::new(2.0, 4.0, 6.0)), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let v = val.get::<Vec3d>().unwrap();
    assert!((v.x - 1.0).abs() < 1e-6);
    assert!((v.y - 2.0).abs() < 1e-6);
    assert!((v.z - 3.0).abs() < 1e-6);
}

#[test]
fn interp_vec3d_held() {
    use usd_gf::Vec3d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testVec3d", &common::vtn("double3"), false, None)
        .unwrap();
    attr.set(Value::from(Vec3d::new(0.0, 0.0, 0.0)), TimeCode::new(0.0));
    attr.set(Value::from(Vec3d::new(2.0, 4.0, 6.0)), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Held);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let v = val.get::<Vec3d>().unwrap();
    assert!((v.x - 0.0).abs() < 1e-6);
    assert!((v.y - 0.0).abs() < 1e-6);
    assert!((v.z - 0.0).abs() < 1e-6);
}

#[test]
fn interp_vec3f_linear() {
    use usd_gf::Vec3f;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testVec3f", &common::vtn("float3"), false, None)
        .unwrap();
    attr.set(Value::from(Vec3f::new(0.0, 0.0, 0.0)), TimeCode::new(0.0));
    attr.set(Value::from(Vec3f::new(2.0, 4.0, 6.0)), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let v = val.get::<Vec3f>().unwrap();
    assert!((v.x - 1.0).abs() < 1e-4);
    assert!((v.y - 2.0).abs() < 1e-4);
    assert!((v.z - 3.0).abs() < 1e-4);
}

#[test]
fn interp_vec4d_linear() {
    use usd_gf::Vec4d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testVec4d", &common::vtn("double4"), false, None)
        .unwrap();
    attr.set(
        Value::from(Vec4d::new(0.0, 0.0, 0.0, 0.0)),
        TimeCode::new(0.0),
    );
    attr.set(
        Value::from(Vec4d::new(2.0, 4.0, 6.0, 8.0)),
        TimeCode::new(2.0),
    );

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let v = val.get::<Vec4d>().unwrap();
    assert!((v.x - 1.0).abs() < 1e-6);
    assert!((v.y - 2.0).abs() < 1e-6);
    assert!((v.z - 3.0).abs() < 1e-6);
    assert!((v.w - 4.0).abs() < 1e-6);
}

#[test]
fn interp_vec2f_linear() {
    use usd_gf::Vec2f;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testVec2f", &common::vtn("float2"), false, None)
        .unwrap();
    attr.set(Value::from(Vec2f::new(0.0, 0.0)), TimeCode::new(0.0));
    attr.set(Value::from(Vec2f::new(2.0, 4.0)), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let v = val.get::<Vec2f>().unwrap();
    assert!((v.x - 1.0).abs() < 1e-4);
    assert!((v.y - 2.0).abs() < 1e-4);
}

// ============================================================================
// 4. Matrix types — element-wise linear interpolation
// ============================================================================

#[test]
fn interp_matrix4d_linear() {
    use usd_gf::Matrix4d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testMatrix4d", &common::vtn("matrix4d"), false, None)
        .unwrap();
    let lo = Matrix4d::from_array([[0.0; 4]; 4]);
    let hi = Matrix4d::from_array([
        [2.0, 4.0, 6.0, 8.0],
        [10.0, 12.0, 14.0, 16.0],
        [18.0, 20.0, 22.0, 24.0],
        [26.0, 28.0, 30.0, 32.0],
    ]);
    attr.set(Value::from(lo), TimeCode::new(0.0));
    attr.set(Value::from(hi), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let m = val.get::<Matrix4d>().unwrap();
    assert!((m[0][0] - 1.0).abs() < 1e-6, "[0][0]={}", m[0][0]);
    assert!((m[0][1] - 2.0).abs() < 1e-6, "[0][1]={}", m[0][1]);
    assert!((m[1][0] - 5.0).abs() < 1e-6, "[1][0]={}", m[1][0]);
    assert!((m[3][3] - 16.0).abs() < 1e-6, "[3][3]={}", m[3][3]);
}

#[test]
fn interp_matrix4d_held() {
    use usd_gf::Matrix4d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testMatrix4d", &common::vtn("matrix4d"), false, None)
        .unwrap();
    let lo = Matrix4d::from_array([[0.0; 4]; 4]);
    let hi = Matrix4d::from_array([[2.0; 4]; 4]);
    attr.set(Value::from(lo), TimeCode::new(0.0));
    attr.set(Value::from(hi), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Held);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let m = val.get::<Matrix4d>().unwrap();
    // Held: should return lower (all zeros)
    assert!((m[0][0] - 0.0).abs() < 1e-6);
    assert!((m[3][3] - 0.0).abs() < 1e-6);
}

#[test]
fn interp_matrix3d_linear() {
    use usd_gf::Matrix3d;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testMatrix3d", &common::vtn("matrix3d"), false, None)
        .unwrap();
    let lo = Matrix3d::from_array([[0.0; 3]; 3]);
    let hi = Matrix3d::from_array([[2.0, 4.0, 6.0], [8.0, 10.0, 12.0], [14.0, 16.0, 18.0]]);
    attr.set(Value::from(lo), TimeCode::new(0.0));
    attr.set(Value::from(hi), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let m = val.get::<Matrix3d>().unwrap();
    assert!((m[0][0] - 1.0).abs() < 1e-6);
    assert!((m[1][1] - 5.0).abs() < 1e-6);
    assert!((m[2][2] - 9.0).abs() < 1e-6);
}

// ============================================================================
// 5. Quaternion types — slerp interpolation
// ============================================================================

#[test]
fn interp_quatd_slerp() {
    use usd_gf::Quatd;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testQuatd", &common::vtn("quatd"), false, None)
        .unwrap();
    let identity = Quatd::from_components(1.0, 0.0, 0.0, 0.0);
    let rot180z = Quatd::from_components(0.0, 0.0, 0.0, 1.0);
    attr.set(Value::from(identity), TimeCode::new(0.0));
    attr.set(Value::from(rot180z), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    // At t=0 should be identity
    let v0 = attr.get(TimeCode::new(0.0)).unwrap();
    let q0 = v0.get::<Quatd>().unwrap();
    assert!((q0.real() - 1.0).abs() < 1e-6, "real at t=0: {}", q0.real());
    // At t=2 should be rot180z
    let v2 = attr.get(TimeCode::new(2.0)).unwrap();
    let q2 = v2.get::<Quatd>().unwrap();
    assert!((q2.real() - 0.0).abs() < 1e-6, "real at t=2: {}", q2.real());
    // At t=1 should be halfway slerp (~90 degrees about Z)
    let v1 = attr.get(TimeCode::new(1.0)).unwrap();
    let q1 = v1.get::<Quatd>().unwrap();
    // Halfway between identity and 180 about Z = 90 about Z
    // = cos(45deg) + sin(45deg)*k = (sqrt(2)/2, 0, 0, sqrt(2)/2)
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(
        (q1.real() - s).abs() < 1e-4,
        "real at t=1: {} expected ~{}",
        q1.real(),
        s
    );
}

#[test]
fn interp_quatd_held() {
    use usd_gf::Quatd;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testQuatd", &common::vtn("quatd"), false, None)
        .unwrap();
    let identity = Quatd::from_components(1.0, 0.0, 0.0, 0.0);
    let rot180z = Quatd::from_components(0.0, 0.0, 0.0, 1.0);
    attr.set(Value::from(identity), TimeCode::new(0.0));
    attr.set(Value::from(rot180z), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Held);
    let v1 = attr.get(TimeCode::new(1.0)).unwrap();
    let q1 = v1.get::<Quatd>().unwrap();
    // Held: should return lower (identity)
    assert!(
        (q1.real() - 1.0).abs() < 1e-6,
        "real at t=1 held: {}",
        q1.real()
    );
}

// ============================================================================
// 6. Array types — element-wise interpolation
// ============================================================================

#[test]
fn interp_double_array_linear() {
    use usd_vt::Array;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDoubleArray", &common::vtn("double[]"), false, None)
        .unwrap();
    attr.set(
        Value::from_no_hash(Array::from(vec![0.0f64, 0.0])),
        TimeCode::new(0.0),
    );
    attr.set(
        Value::from_no_hash(Array::from(vec![2.0f64, 2.0])),
        TimeCode::new(2.0),
    );

    stage.set_interpolation_type(InterpolationType::Linear);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let arr = val.get::<Array<f64>>().unwrap();
    assert_eq!(arr.len(), 2);
    assert!((arr[0] - 1.0).abs() < 1e-6, "arr[0]={}", arr[0]);
    assert!((arr[1] - 1.0).abs() < 1e-6, "arr[1]={}", arr[1]);
}

#[test]
fn interp_double_array_held() {
    use usd_vt::Array;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDoubleArray", &common::vtn("double[]"), false, None)
        .unwrap();
    attr.set(
        Value::from_no_hash(Array::from(vec![0.0f64, 0.0])),
        TimeCode::new(0.0),
    );
    attr.set(
        Value::from_no_hash(Array::from(vec![2.0f64, 2.0])),
        TimeCode::new(2.0),
    );

    stage.set_interpolation_type(InterpolationType::Held);
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let arr = val.get::<Array<f64>>().unwrap();
    assert_eq!(arr.len(), 2);
    assert!((arr[0] - 0.0).abs() < 1e-6);
    assert!((arr[1] - 0.0).abs() < 1e-6);
}

// ============================================================================
// 7. Per-stage interpolation type setting
// ============================================================================

#[test]
fn interp_type_setting() {
    let stage = make_stage();

    // Default is Linear
    assert_eq!(stage.interpolation_type(), InterpolationType::Linear);

    stage.set_interpolation_type(InterpolationType::Held);
    assert_eq!(stage.interpolation_type(), InterpolationType::Held);

    stage.set_interpolation_type(InterpolationType::Linear);
    assert_eq!(stage.interpolation_type(), InterpolationType::Linear);
}

// ============================================================================
// 8. Mismatched array shapes — held fallback (C++ TestInterpolationWithMismatchedShapes)
// ============================================================================

#[test]
fn interp_mismatched_shapes() {
    use usd_vt::Array;
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testAttr", &common::vtn("double[]"), false, None)
        .unwrap();

    // 5 elements at t=0, 3 elements at t=2
    attr.set(
        Value::from_no_hash(Array::from(vec![1.0f64; 5])),
        TimeCode::new(0.0),
    );
    attr.set(
        Value::from_no_hash(Array::from(vec![3.0f64; 3])),
        TimeCode::new(2.0),
    );

    stage.set_interpolation_type(InterpolationType::Linear);
    // C++: mismatched shapes -> held (returns lower value = 5 elements of 1.0)
    let val = attr.get(TimeCode::new(1.0)).unwrap();
    let arr = val.get::<Array<f64>>().unwrap();
    assert_eq!(
        arr.len(),
        5,
        "mismatched: should return lower array (5 elems)"
    );
    assert!((arr[0] - 1.0).abs() < 1e-6);
}

// ============================================================================
// 9. Boundary conditions: before first / after last sample
// ============================================================================

#[test]
fn interp_boundary_clamp() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDouble", &common::vtn("double"), false, None)
        .unwrap();
    attr.set(Value::from(10.0f64), TimeCode::new(1.0));
    attr.set(Value::from(20.0f64), TimeCode::new(3.0));

    stage.set_interpolation_type(InterpolationType::Linear);

    // Before first sample: clamp to first
    verify_near(&attr, 0.0, 10.0);
    verify_near(&attr, -5.0, 10.0);

    // After last sample: clamp to last
    verify_near(&attr, 4.0, 20.0);
    verify_near(&attr, 100.0, 20.0);

    // At exact sample times
    verify_near(&attr, 1.0, 10.0);
    verify_near(&attr, 3.0, 20.0);

    // Midpoint
    verify_near(&attr, 2.0, 15.0);
}

// ============================================================================
// 10. Exact sample times return exact values
// ============================================================================

#[test]
fn interp_exact_sample_times() {
    let stage = make_stage();
    let prim = stage.override_prim("/TestPrim").unwrap();
    let attr = prim
        .create_attribute("testDouble", &common::vtn("double"), false, None)
        .unwrap();
    attr.set(Value::from(0.0f64), TimeCode::new(0.0));
    attr.set(Value::from(2.0f64), TimeCode::new(2.0));

    stage.set_interpolation_type(InterpolationType::Linear);
    verify_near(&attr, 0.0, 0.0);
    verify_near(&attr, 2.0, 2.0);

    stage.set_interpolation_type(InterpolationType::Held);
    verify_near(&attr, 0.0, 0.0);
    verify_near(&attr, 2.0, 2.0);
}
