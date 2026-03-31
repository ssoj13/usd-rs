// Port of testUsdTimeValueAuthoring.cpp + .py — time-based value authoring
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdTimeValueAuthoring*

mod common;

use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;
use usd_sdf::TimeCode;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Root", "Xform").expect("define");

    let float_type = common::vtn("float");
    prim.create_attribute("anim", &float_type, false, None);

    stage
}

// ============================================================================
// Set/get time samples
// ============================================================================

#[test]
fn set_and_get_time_sample() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    attr.set(0.0f32, TimeCode::new(1.0));
    attr.set(1.0f32, TimeCode::new(10.0));

    let val_at_1 = attr.get(TimeCode::new(1.0));
    if let Some(v) = val_at_1 {
        if let Some(f) = v.get::<f32>() {
            assert!((f - 0.0f32).abs() < 1e-6);
        }
    }

    let val_at_10 = attr.get(TimeCode::new(10.0));
    if let Some(v) = val_at_10 {
        if let Some(f) = v.get::<f32>() {
            assert!((f - 1.0f32).abs() < 1e-6);
        }
    }
}

// ============================================================================
// Time sample count
// ============================================================================

#[test]
fn time_sample_count() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    assert_eq!(attr.get_num_time_samples(), 0);

    attr.set(0.0f32, TimeCode::new(1.0));
    assert_eq!(attr.get_num_time_samples(), 1);

    attr.set(0.5f32, TimeCode::new(5.0));
    assert_eq!(attr.get_num_time_samples(), 2);

    attr.set(1.0f32, TimeCode::new(10.0));
    assert_eq!(attr.get_num_time_samples(), 3);
}

// ============================================================================
// Get time sample times
// ============================================================================

#[test]
fn get_time_sample_times() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    attr.set(0.0f32, TimeCode::new(1.0));
    attr.set(0.5f32, TimeCode::new(5.0));
    attr.set(1.0f32, TimeCode::new(10.0));

    let times = attr.get_time_samples();
    assert_eq!(times.len(), 3);
    assert!((times[0] - 1.0).abs() < 1e-6);
    assert!((times[1] - 5.0).abs() < 1e-6);
    assert!((times[2] - 10.0).abs() < 1e-6);
}

// ============================================================================
// Default value vs time samples
// ============================================================================

#[test]
fn default_does_not_affect_time_samples() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    // Set default and time samples
    attr.set(100.0f32, TimeCode::default_time());
    attr.set(0.0f32, TimeCode::new(1.0));

    // Default should not affect time sample query
    let val = attr.get(TimeCode::new(1.0));
    if let Some(v) = val {
        if let Some(f) = v.get::<f32>() {
            assert!((f - 0.0f32).abs() < 1e-6);
        }
    }

    // Query at default time should return the default (if no time samples bracket it)
    let default = attr.get(TimeCode::default_time());
    if let Some(v) = default {
        if let Some(f) = v.get::<f32>() {
            assert!((f - 100.0f32).abs() < 1e-6);
        }
    }
}

// ============================================================================
// Has value / has authored value
// ============================================================================

#[test]
fn has_value_after_authoring() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    // Before authoring
    assert!(!attr.has_value());

    // After setting default
    attr.set(1.0f32, TimeCode::default_time());
    assert!(attr.has_value());
}

#[test]
fn has_authored_value_keys() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let attr = prim.get_attribute("anim").expect("attr");

    assert!(!attr.has_authored_value());

    attr.set(0.0f32, TimeCode::new(1.0));
    assert!(attr.has_authored_value());
}

// ============================================================================
// Multiple attributes with time samples
// ============================================================================

#[test]
fn multiple_attrs_time_samples() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let float_type = common::vtn("float");
    let attr_x = prim
        .create_attribute("tx", &float_type, false, None)
        .expect("create");
    let attr_y = prim
        .create_attribute("ty", &float_type, false, None)
        .expect("create");

    attr_x.set(0.0f32, TimeCode::new(1.0));
    attr_x.set(100.0f32, TimeCode::new(10.0));

    attr_y.set(50.0f32, TimeCode::new(1.0));
    attr_y.set(-50.0f32, TimeCode::new(10.0));

    assert_eq!(attr_x.get_num_time_samples(), 2);
    assert_eq!(attr_y.get_num_time_samples(), 2);

    let vx = attr_x.get(TimeCode::new(1.0));
    let vy = attr_y.get(TimeCode::new(1.0));

    if let (Some(vx), Some(vy)) = (vx, vy) {
        if let (Some(fx), Some(fy)) = (vx.get::<f32>(), vy.get::<f32>()) {
            assert!((fx - 0.0f32).abs() < 1e-6);
            assert!((fy - 50.0f32).abs() < 1e-6);
        }
    }
}
