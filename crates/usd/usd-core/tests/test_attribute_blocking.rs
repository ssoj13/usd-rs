//! Tests for attribute value blocking.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdAttributeBlocking.py
//!   - pxr/usd/usd/testenv/testUsdAttributeBlocking.cpp

mod common;

use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_core::common::InitialLoadSet;
use usd_core::resolve_info::ResolveInfoSource;
use usd_core::stage::Stage;
use usd_sdf::{TimeCode, ValueBlock};
use usd_vt::Value;
use usd_vt::spline::{SplineExtrapolation, SplineInterpMode, SplineKnot, SplineValue};

const TIME_SAMPLE_BEGIN: usize = 101;
const TIME_SAMPLE_END: usize = 120;
const DEFAULT_VALUE: f64 = 4.0;

// ============================================================================
// Stage setup helpers (matches C++ _GenerateStage and Python CreateTestAssets)
// ============================================================================

/// Create a stage with a prim "/Sphere" that has:
///   - "size" attribute with default value 1.0
///   - "points" attribute with time samples [101..120)
///   - "/SphereOver" override prim referencing /Sphere with "size" blocked
fn generate_stage() -> (Arc<Stage>, Attribute, Attribute, Attribute) {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");

    let prim = stage.define_prim("/Sphere", "").expect("define_prim");

    let def_attr = prim
        .create_attribute("size", &common::vtn("double"), true, None)
        .expect("create size attr");
    def_attr.set(Value::from(1.0_f64), TimeCode::default_time());

    let sample_attr = prim
        .create_attribute("points", &common::vtn("double"), false, None)
        .expect("create points attr");
    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        let t = i as f64;
        sample_attr.set(Value::from(t), TimeCode::new(t));
    }

    // Create an over prim with internal reference and blocked attr
    let local_ref_prim = stage.override_prim("/SphereOver").expect("override_prim");
    local_ref_prim.get_references().add_internal_reference(
        &usd_sdf::Path::from("/Sphere"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::default(),
    );
    let local_ref_attr = local_ref_prim
        .create_attribute("size", &common::vtn("double"), true, None)
        .expect("create local ref size attr");
    local_ref_attr.block();

    (stage, def_attr, sample_attr, local_ref_attr)
}

/// Create a stage with animation block test layers.
/// Matches C++ _GenerateStageForAnimationBlock / Python CreateTestAssetsForAnimationBlock.
fn generate_stage_for_animation_block() -> Arc<Stage> {
    common::setup();

    // Weakest layer
    let weaker_layer = usd_sdf::Layer::create_anonymous(Some("animationBlocks_weaker"));
    assert!(weaker_layer.import_from_string(
        r#"#usda 1.0
over "Human"
{
    int c = 1
    double d = 2.0
}
"#,
    ));

    // Weak middle layer
    let weak_layer = usd_sdf::Layer::create_anonymous(Some("animationBlocks_weak"));
    assert!(weak_layer.import_from_string(
        r#"#usda 1.0
over "Human"
{
    int a = AnimationBlock
    int a.timeSamples = {
        1: 5,
        2: 18,
    }

    double b.spline = {
        1: 5; post held,
        2: 18; post held,
    }

    int c.timeSamples = {
        0: 456,
        1: 789
    }

    double d.spline = {
        1: 5; post held,
        2: 18; post held,
    }
}
"#,
    ));

    // Strongest layer
    let strong_layer = usd_sdf::Layer::create_anonymous(Some("animationBlocks_strong"));
    assert!(strong_layer.import_from_string(
        r#"#usda 1.0
def Xform "Human"
{
    double b = AnimationBlock
    double b.spline = {
        1: 10; post held,
        2: 20; post held,
    }

    double d = AnimationBlock

    double e = AnimationBlock
}
"#,
    ));

    // Root layer sublayering all three
    let root_layer = usd_sdf::Layer::create_anonymous(Some("test_anim_block"));
    root_layer.set_sublayer_paths(&[
        strong_layer.identifier().to_string(),
        weak_layer.identifier().to_string(),
        weaker_layer.identifier().to_string(),
    ]);

    let stage =
        Stage::open_with_root_layer(root_layer, InitialLoadSet::LoadAll).expect("open stage");

    // BlockAnimation on "c"
    let attr_c = stage
        .get_attribute_at_path(&usd_sdf::Path::from("/Human.c"))
        .expect("attr c");
    attr_c.block_animation();

    stage
}

/// Create an in-memory stage with a single spline-valued double attribute.
fn generate_stage_for_spline() -> (Arc<Stage>, Attribute) {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create stage");
    let prim = stage.define_prim("/Sphere", "").expect("define_prim");
    let spline_attr = prim
        .create_attribute("points", &common::vtn("double"), false, None)
        .expect("create spline attr");

    let mut spline = SplineValue::default();
    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        let sample = i as f64;
        let mut knot = SplineKnot::new(sample, sample);
        knot.set_next_interpolation(SplineInterpMode::Held);
        spline.set_knot(knot);
    }
    assert!(spline_attr.set_spline(&spline));

    (stage, spline_attr)
}

// ============================================================================
// Python tests
// ============================================================================

/// TestBlock: block() clears all time samples; get() returns None.
/// Matches Python TestBlock.
#[test]
fn attr_blocking_block_clears_time_samples() {
    let (_stage, _def_attr, sample_attr, _local_ref) = generate_stage();

    assert!(sample_attr.get_num_time_samples() > 0);
    assert!(!sample_attr.get_resolve_info().value_is_blocked());

    sample_attr.block();

    assert_eq!(sample_attr.get_num_time_samples(), 0);
    assert!(sample_attr.get_resolve_info().value_is_blocked());

    // get() at default should return None (blocked)
    assert!(sample_attr.get(TimeCode::default_time()).is_none());

    // get() at every time sample should return None
    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        assert!(
            sample_attr.get(TimeCode::new(i as f64)).is_none(),
            "Expected None at time {i}"
        );
    }
}

/// TestIndividualTimeSampleBlocking: set ValueBlock at individual times.
/// Matches Python TestIndividualTimeSampleBlocking.
#[test]
fn attr_blocking_individual_time_samples() {
    let (_stage, _def_attr, sample_attr, _local_ref) = generate_stage();

    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        let t = i as f64;
        sample_attr.set(Value::new(ValueBlock), TimeCode::new(t));
        assert!(
            sample_attr.get(TimeCode::new(t)).is_none(),
            "Expected None at blocked time {t}"
        );
        // Individual time sample blocking is NOT whole-value blocking
        assert!(!sample_attr.get_resolve_info().value_is_blocked());
    }
}

/// TestDefaultValueBlocking: block default value with ValueBlock.
/// Matches Python TestDefaultValueBlocking.
#[test]
fn attr_blocking_default_value() {
    let (_stage, def_attr, _sample_attr, _local_ref) = generate_stage();

    // Initially has a value
    assert!(def_attr.get(TimeCode::default_time()).is_some());
    assert!(!def_attr.get_resolve_info().value_is_blocked());

    // Block with ValueBlock
    def_attr.set(Value::new(ValueBlock), TimeCode::default_time());

    // Now get() returns None and value is blocked
    assert!(def_attr.get(TimeCode::default_time()).is_none());
    assert!(def_attr.get_resolve_info().value_is_blocked());
}

// ============================================================================
// C++ tests
// ============================================================================

/// Blocks through local references: an over prim referencing /Sphere with
/// attr.Block() should have its value blocked.
/// Matches C++ _CheckDefaultBlocked(localRefAttr) and _CheckDefaultNotBlocked(defAttr).
#[test]
fn attr_blocking_through_local_references() {
    let (_stage, def_attr, _sample_attr, local_ref_attr) = generate_stage();

    // localRefAttr is blocked
    assert!(local_ref_attr.get(TimeCode::default_time()).is_none());
    assert!(!local_ref_attr.has_value());
    assert!(!local_ref_attr.has_authored_value());
    let info = local_ref_attr.get_resolve_info();
    assert!(info.has_authored_value_opinion());

    // defAttr still has its value
    let val = def_attr.get(TimeCode::default_time());
    assert!(val.is_some());
    let v: f64 = val.as_ref().and_then(|v| v.downcast_clone()).expect("f64");
    assert!((v - 1.0).abs() < 1e-9);
    assert!(def_attr.has_value());
    assert!(def_attr.has_authored_value());
}

/// Block default, unblock by setting a new value, block again via Block().
/// Matches C++ main() default value blocking sequence.
#[test]
fn attr_blocking_default_set_and_unset() {
    let (_stage, def_attr, _sample_attr, _local_ref) = generate_stage();

    // Block with typed ValueBlock
    def_attr.set(Value::new(ValueBlock), TimeCode::default_time());
    assert!(def_attr.get(TimeCode::default_time()).is_none());
    assert!(!def_attr.has_value());
    assert!(!def_attr.has_authored_value());
    assert!(def_attr.get_resolve_info().has_authored_value_opinion());

    // Restore value
    def_attr.set(Value::from(DEFAULT_VALUE), TimeCode::default_time());
    let val: f64 = def_attr
        .get(TimeCode::default_time())
        .and_then(|v| v.downcast_clone())
        .expect("restored");
    assert!((val - DEFAULT_VALUE).abs() < 1e-9);
    assert!(def_attr.has_value());
    assert!(def_attr.has_authored_value());

    // Block with untyped VtValue(block)
    def_attr.set(Value::new(ValueBlock), TimeCode::default_time());
    assert!(def_attr.get(TimeCode::default_time()).is_none());

    // Restore again
    def_attr.set(Value::from(DEFAULT_VALUE), TimeCode::default_time());
    let val2: f64 = def_attr
        .get(TimeCode::default_time())
        .and_then(|v| v.downcast_clone())
        .expect("restored2");
    assert!((val2 - DEFAULT_VALUE).abs() < 1e-9);

    // Block via Block() method
    def_attr.block();
    assert!(def_attr.get(TimeCode::default_time()).is_none());
    assert!(!def_attr.has_value());
    assert!(!def_attr.has_authored_value());
}

/// Typed time sample blocking: set ValueBlock at each time, verify blocked,
/// check bracketing time samples still report correctly.
/// Matches C++ "Testing typed time sample operations".
#[test]
fn attr_blocking_typed_time_sample_ops() {
    let (_stage, _def_attr, sample_attr, _local_ref) = generate_stage();

    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        let t = i as f64;

        // Verify sample exists
        let val_before = sample_attr.get(TimeCode::new(t));
        assert!(val_before.is_some(), "Expected value at time {t}");

        // Get bracketing before block
        let bracket_pre = sample_attr.get_bracketing_time_samples(t);

        // Block it
        sample_attr.set(Value::new(ValueBlock), TimeCode::new(t));

        // Verify blocked
        assert!(
            sample_attr.get(TimeCode::new(t)).is_none(),
            "Expected None at blocked time {t}"
        );

        // Bracketing should still report the same times
        let bracket_post = sample_attr.get_bracketing_time_samples(t);
        assert_eq!(
            bracket_pre, bracket_post,
            "Brackets changed after block at {t}"
        );
    }
}

/// Block() clears both default and time samples entirely.
/// Matches C++ "sampleAttr.Block()" section after typed time sample ops.
#[test]
fn attr_blocking_block_clears_all() {
    let (_stage, _def_attr, sample_attr, _local_ref) = generate_stage();

    // Reset with time samples
    assert!(sample_attr.get_num_time_samples() > 0);

    sample_attr.block();

    // Default is blocked
    assert!(sample_attr.get(TimeCode::default_time()).is_none());
    assert_eq!(sample_attr.get_num_time_samples(), 0);

    // Every time sample should be gone
    for i in TIME_SAMPLE_BEGIN..TIME_SAMPLE_END {
        assert!(
            sample_attr.get(TimeCode::new(i as f64)).is_none(),
            "Expected None at time {i}"
        );
    }
}

/// Interleaved blocked/unblocked time samples.
/// Matches C++ "Test attribute blocking behavior in between blocked/unblocked times".
#[test]
fn attr_blocking_interleaved() {
    let (_stage, _def_attr, sample_attr, _local_ref) = generate_stage();

    // Block every other sample
    for i in (TIME_SAMPLE_BEGIN..TIME_SAMPLE_END).step_by(2) {
        let t = i as f64;
        sample_attr.set(Value::new(ValueBlock), TimeCode::new(t));

        // The blocked sample should return None
        assert!(
            sample_attr.get(TimeCode::new(t)).is_none(),
            "Expected None at blocked time {t}"
        );

        // Check half-step and next sample
        if (i + 1) < TIME_SAMPLE_END {
            let half = t + 0.5;
            // Half-step between blocked and next: should be blocked
            // (held interpolation from blocked value)
            assert!(
                sample_attr.get(TimeCode::new(half)).is_none(),
                "Expected None at half-step {half}"
            );

            // Next (unblocked) sample should still have value
            let next = t + 1.0;
            let val = sample_attr.get(TimeCode::new(next));
            assert!(val.is_some(), "Expected value at unblocked time {next}");
        }
    }
}

// ============================================================================
// Animation Block tests
// ============================================================================

/// Test AnimationBlock semantics across sublayers.
/// Matches C++ _CheckAnimationBlock / Python TestAnimationBlock.
///
/// Attr "a": animation block in default (weaker), time samples stronger => time samples shine through
/// Attr "b": animation block in default, spline stronger => spline shines through
/// Attr "c": animation block authored via block_animation() => blocks time samples, default shines through
/// Attr "d": animation block in strongest layer => blocks spline, default shines through
/// Attr "e": only animation block, no other values => source is None
#[test]
fn attr_blocking_animation_block() {
    let stage = generate_stage_for_animation_block();
    let prim = stage
        .get_prim_at_path(&usd_sdf::Path::from("/Human"))
        .expect("prim");

    // Attr "a": strongest has time samples (not blocked by animation block)
    // Animation block in same layer is weaker than time samples
    let attr_a = prim.get_attribute("a").expect("attr a");
    let info_a = attr_a.get_resolve_info();
    assert_eq!(info_a.source(), ResolveInfoSource::TimeSamples);
    // Default should be None (animation block)
    assert!(attr_a.get(TimeCode::default_time()).is_none());
    // Time samples shine through
    let val_a = attr_a.get_typed::<i32>(TimeCode::new(1.0));
    assert_eq!(val_a, Some(5));

    // Attr "b": strongest spline is not blocked by weaker animation block.
    let attr_b = prim.get_attribute("b").expect("attr b");
    let info_b = attr_b.get_resolve_info();
    assert_eq!(info_b.source(), ResolveInfoSource::Spline);
    assert!(attr_b.get(TimeCode::default_time()).is_none());
    let val_b_t1 = attr_b.get_typed::<f64>(TimeCode::new(1.0));
    assert_eq!(val_b_t1, Some(10.0));

    // Attr "c": animation block via block_animation() => blocks time samples
    // Default value (1) from weaker layer shines through
    let attr_c = prim.get_attribute("c").expect("attr c");
    let info_c = attr_c.get_resolve_info();
    assert_eq!(info_c.source(), ResolveInfoSource::Default);
    let val_c_default = attr_c.get_typed::<i32>(TimeCode::default_time());
    assert_eq!(val_c_default, Some(1));
    // Time samples are blocked, default shines through at all times
    let val_c_t1 = attr_c.get_typed::<i32>(TimeCode::new(1.0));
    assert_eq!(val_c_t1, Some(1));

    // Attr "d": AnimationBlock in strongest layer blocks spline from weaker
    // Default value (2.0) from weakest layer shines through
    let attr_d = prim.get_attribute("d").expect("attr d");
    let info_d = attr_d.get_resolve_info();
    assert_eq!(info_d.source(), ResolveInfoSource::Default);
    let val_d_default = attr_d.get_typed::<f64>(TimeCode::default_time());
    assert_eq!(val_d_default, Some(2.0));
    let val_d_t1 = attr_d.get_typed::<f64>(TimeCode::new(1.0));
    assert_eq!(val_d_t1, Some(2.0));

    // Attr "e": only has AnimationBlock, no other values anywhere
    let attr_e = prim.get_attribute("e").expect("attr e");
    let info_e = attr_e.get_resolve_info();
    assert_eq!(info_e.source(), ResolveInfoSource::None);
    assert!(attr_e.get(TimeCode::default_time()).is_none());
}

// ============================================================================
// Spline blocking tests — requires TsSpline support
// ============================================================================

/// Spline value blocking: extrapolation blocking, interpolation blocking, empty spline.
/// Matches C++ _CheckSplineBlocking / Python TestSplineValueBlocking.
#[test]
fn attr_blocking_spline_value_blocking() {
    let (_stage, spline_attr) = generate_stage_for_spline();

    let t0 = TIME_SAMPLE_BEGIN as f64;
    let t1 = TIME_SAMPLE_END as f64;

    for t in (0..((TIME_SAMPLE_END - TIME_SAMPLE_BEGIN) * 2)).map(|i| t0 + (i as f64) * 0.5) {
        assert!(
            spline_attr.get_typed::<f64>(TimeCode::new(t)).is_some(),
            "expected spline value at {t}"
        );
    }

    let mut spline = spline_attr.get_spline().expect("spline");
    spline.set_pre_extrapolation(SplineExtrapolation::ValueBlock);
    spline.set_post_extrapolation(SplineExtrapolation::ValueBlock);

    assert!(
        spline_attr
            .get_typed::<f64>(TimeCode::new(t0 - 1.0))
            .is_some()
    );
    assert!(
        spline_attr
            .get_typed::<f64>(TimeCode::new(t1 + 1.0))
            .is_some()
    );

    assert!(spline_attr.set_spline(&spline));

    assert!(
        spline_attr
            .get_typed::<f64>(TimeCode::new(t0 - 1.0))
            .is_none()
    );
    assert!(
        spline_attr
            .get_typed::<f64>(TimeCode::new(t1 + 1.0))
            .is_none()
    );

    for i in (TIME_SAMPLE_BEGIN..TIME_SAMPLE_END).step_by(2) {
        let mut knot = spline.get_knot(i as f64).expect("knot");
        knot.set_next_interpolation(SplineInterpMode::ValueBlock);
        spline.set_knot(knot);
    }

    assert!(spline_attr.set_spline(&spline));

    for i in (TIME_SAMPLE_BEGIN..TIME_SAMPLE_END).step_by(2) {
        let t = i as f64;
        assert!(spline_attr.get_typed::<f64>(TimeCode::new(t)).is_none());
        assert!(
            spline_attr
                .get_typed::<f64>(TimeCode::new(t + 0.5))
                .is_none()
        );
    }

    for i in ((TIME_SAMPLE_BEGIN + 1)..TIME_SAMPLE_END).step_by(2) {
        let t = i as f64;
        assert!(spline_attr.get_typed::<f64>(TimeCode::new(t)).is_some());
        if t + 0.5 < t1 {
            assert!(
                spline_attr
                    .get_typed::<f64>(TimeCode::new(t + 0.5))
                    .is_some()
            );
        }
    }

    assert!(!spline_attr.get_resolve_info().value_is_blocked());

    assert!(spline_attr.set_spline(&SplineValue::default()));
    for t in
        (0..(((TIME_SAMPLE_END - TIME_SAMPLE_BEGIN) + 2) * 2)).map(|i| t0 - 1.0 + (i as f64) * 0.5)
    {
        assert!(spline_attr.get_typed::<f64>(TimeCode::new(t)).is_none());
    }
}

/// Spline-based animation block tests (attr b with spline in sublayers).
/// Matches C++ and Python animation block for attr "b" (spline source).
#[test]
fn attr_blocking_spline_animation_block() {
    let stage = generate_stage_for_animation_block();
    let prim = stage
        .get_prim_at_path(&usd_sdf::Path::from("/Human"))
        .expect("prim");

    let attr_b = prim.get_attribute("b").expect("attr b");
    assert_eq!(
        attr_b.get_resolve_info().source(),
        ResolveInfoSource::Spline
    );
    assert!(attr_b.get(TimeCode::default_time()).is_none());
    assert_eq!(attr_b.get_typed::<f64>(TimeCode::new(1.0)), Some(10.0));
}
