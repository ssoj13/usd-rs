//! Tests for time offset composition through references and payloads.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdTimeOffsets.py

mod common;

use std::sync::Arc;
use usd_core::{
    Stage,
    common::{InitialLoadSet, ListPosition},
};
use usd_sdf::{Layer, LayerOffset, Path, TimeCode};

// ============================================================================
// Helpers
// ============================================================================

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

/// Bundled prim + offset for verification.
struct AdjustedPrim {
    stage: Arc<Stage>,
    prim_path: Path,
    layer_offset: LayerOffset,
}

/// Creates a source layer with float attr at </Foo.attr> with time samples at 1, 2, 10.
/// The value at each time equals the time itself: value(1)=1.0, value(2)=2.0, value(10)=10.0.
fn gen_test_layer() -> Arc<Layer> {
    let layer = Layer::create_anonymous(Some(".usda"));
    let stage = Stage::open_with_root_layer(Arc::clone(&layer), InitialLoadSet::LoadAll)
        .expect("open stage from anonymous layer");
    stage.override_prim("/Foo").expect("override /Foo");
    let foo = stage.get_prim_at_path(&p("/Foo")).expect("get /Foo");
    let attr = foo
        .create_attribute("attr", &common::vtn("float"), false, None)
        .expect("create attr");
    attr.set(1.0_f32, TimeCode::new(1.0));
    attr.set(2.0_f32, TimeCode::new(2.0));
    attr.set(10.0_f32, TimeCode::new(10.0));
    layer
}

/// Creates an override prim with a reference (or payload) to test_layer at </Foo>,
/// applying the given offset and scale.
fn make_prim(
    stage: &Arc<Stage>,
    test_layer: &Arc<Layer>,
    path: &str,
    offset: f64,
    scale: f64,
    match_path: bool,
    make_payload: bool,
) -> AdjustedPrim {
    stage
        .override_prim(path)
        .unwrap_or_else(|_| panic!("override {path}"));
    let prim = stage
        .get_prim_at_path(&p(path))
        .unwrap_or_else(|| panic!("get {path}"));
    let layer_offset = LayerOffset::new(offset, scale);

    let ref_path = if match_path { p(path) } else { p("/Foo") };

    if make_payload {
        assert!(
            prim.get_payloads().add_payload_with_path(
                test_layer.identifier(),
                &ref_path,
                layer_offset.clone(),
                ListPosition::FrontOfPrependList,
            ),
            "add_payload failed for {path}"
        );
    } else {
        assert!(
            prim.get_references().add_reference_with_path(
                test_layer.identifier(),
                &ref_path,
                layer_offset.clone(),
                ListPosition::FrontOfPrependList,
            ),
            "add_reference failed for {path}"
        );
    }

    AdjustedPrim {
        stage: Arc::clone(stage),
        prim_path: p(path),
        layer_offset,
    }
}

/// Verifies that time samples through the offset match expectations.
///
/// For each original time t in {1, 2, 10}, the value at offset*t should equal t.
/// GetTimeSamples should return the offset-transformed times.
/// GetBracketingTimeSamples should bracket correctly.
fn verify_offset(adj: &AdjustedPrim) {
    let prim = adj
        .stage
        .get_prim_at_path(&adj.prim_path)
        .unwrap_or_else(|| panic!("prim at {:?}", adj.prim_path));
    let attr = prim
        .get_attribute("attr")
        .unwrap_or_else(|| panic!("attr on {:?}", adj.prim_path));
    let offset = &adj.layer_offset;

    // value(offset * t) == t for each original sample time
    let mut expected_times = Vec::new();
    for &t in &[1.0_f64, 2.0, 10.0] {
        let stage_time = offset.apply(t);
        let val = *attr
            .get(TimeCode::new(stage_time))
            .unwrap_or_else(|| {
                panic!(
                    "get value at stage_time={stage_time} for {:?}",
                    adj.prim_path
                )
            })
            .get::<f32>()
            .unwrap_or_else(|| {
                panic!(
                    "value not f32 at stage_time={stage_time} for {:?}",
                    adj.prim_path
                )
            });
        assert_eq!(
            val, t as f32,
            "expected {t} at stage_time={stage_time} for {:?}",
            adj.prim_path
        );
        expected_times.push(stage_time);
    }

    // GetTimeSamples should match expected offset-transformed times
    let authored_times = attr.get_time_samples();
    assert_eq!(
        authored_times.len(),
        expected_times.len(),
        "time sample count mismatch for {:?}",
        adj.prim_path
    );
    for (i, time) in authored_times.iter().enumerate() {
        assert!(
            (expected_times[i] - time).abs() < 1e-5,
            "time[{i}] mismatch: expected {} got {time} for {:?}",
            expected_times[i],
            adj.prim_path
        );
    }

    // GetBracketingTimeSamples at exact sample times should return (t, t)
    for &t in &[1.0_f64, 2.0, 10.0] {
        let stage_time = offset.apply(t);
        let (lo, hi) = attr
            .get_bracketing_time_samples(stage_time)
            .expect(&format!("bracketing at {stage_time}"));
        assert!(
            (lo - stage_time).abs() < 1e-5 && (hi - stage_time).abs() < 1e-5,
            "bracketing({stage_time}) = ({lo},{hi}), expected ({stage_time},{stage_time}) for {:?}",
            adj.prim_path
        );
    }

    // Before first sample: bracketing should clamp to first
    let before = offset.apply(0.0);
    let first = offset.apply(1.0);
    let (lo, hi) = attr
        .get_bracketing_time_samples(before)
        .expect("bracketing before first");
    assert!(
        (lo - first).abs() < 1e-5 && (hi - first).abs() < 1e-5,
        "bracketing({before}) = ({lo},{hi}), expected ({first},{first})"
    );

    // Between samples: should bracket to neighbors
    for &(t_lo, t_hi) in &[(1.0, 2.0), (2.0, 10.0)] {
        let mid = (offset.apply(t_lo) + offset.apply(t_hi)) / 2.0;
        let (lo, hi) = attr
            .get_bracketing_time_samples(mid)
            .expect(&format!("bracketing mid {mid}"));
        assert!(
            (lo - offset.apply(t_lo)).abs() < 1e-5,
            "bracketing({mid}).lo = {lo}, expected {}",
            offset.apply(t_lo)
        );
        assert!(
            (hi - offset.apply(t_hi)).abs() < 1e-5,
            "bracketing({mid}).hi = {hi}, expected {}",
            offset.apply(t_hi)
        );
    }

    // After last sample: bracketing should clamp to last
    let after = offset.apply(11.0);
    let last = offset.apply(10.0);
    let (lo, hi) = attr
        .get_bracketing_time_samples(after)
        .expect("bracketing after last");
    assert!(
        (lo - last).abs() < 1e-5 && (hi - last).abs() < 1e-5,
        "bracketing({after}) = ({lo},{hi}), expected ({last},{last})"
    );
}

/// Builds reference offsets (or payload offsets if make_payloads=true).
/// Returns AdjustedPrim list for valid (positive-scale) offsets only.
fn build_reference_offsets(
    root_layer: &Arc<Layer>,
    test_layer: &Arc<Layer>,
    make_payloads: bool,
) -> Vec<AdjustedPrim> {
    let stage = Stage::open_with_root_layer(Arc::clone(root_layer), InitialLoadSet::LoadAll)
        .expect("open root stage");

    // Valid offset/scale cases
    let cases: Vec<(&str, f64, f64)> = vec![
        // Single offset or scale
        ("/Identity", 0.0, 1.0),
        ("/Offset_1", 1.0, 1.0),
        ("/Offset_neg1", -1.0, 1.0),
        ("/Offset_7", 7.0, 1.0),
        ("/Offset_neg7", -7.0, 1.0),
        ("/Scale_2", 0.0, 2.0),
        ("/Scale_1p5", 0.0, 1.5),
        ("/Scale_half", 0.0, 0.5),
        // Combined offset and scale
        ("/Scale_half_Offset_1", 1.0, 0.5),
        ("/Scale_half_Offset_neg1", -1.0, 0.5),
    ];

    let mut adj_prims = Vec::new();
    for (path, offset, scale) in &cases {
        adj_prims.push(make_prim(
            &stage,
            test_layer,
            path,
            *offset,
            *scale,
            false,
            make_payloads,
        ));
    }

    // Verify no composition errors for valid offsets
    let errors = stage.get_composition_errors();
    assert!(
        errors.is_empty(),
        "unexpected composition errors: {errors:?}"
    );

    // Negative scale cases should produce composition errors
    let neg_cases: Vec<(&str, f64, f64)> = vec![
        ("/Scale_negHalf", 0.0, -0.5),
        ("/Scale_negHalf_Offset_1", 1.0, -0.5),
        ("/Scale_negHalf_Offset_neg1", -1.0, -0.5),
    ];
    for (path, offset, scale) in &neg_cases {
        make_prim(
            &stage,
            test_layer,
            path,
            *offset,
            *scale,
            false,
            make_payloads,
        );
    }
    let errors = stage.get_composition_errors();
    assert_eq!(
        errors.len(),
        neg_cases.len(),
        "expected {} composition errors for negative scales, got {}: {errors:?}",
        neg_cases.len(),
        errors.len()
    );

    adj_prims
}

// ============================================================================
// test_ReferenceOffsets
// ============================================================================

#[test]
fn to_reference_offsets() {
    common::setup();

    let test_layer = gen_test_layer();
    let root_layer = Layer::create_anonymous(Some(".usda"));

    let adj_prims = build_reference_offsets(&root_layer, &test_layer, false);
    for adj in &adj_prims {
        verify_offset(adj);
    }
}

// ============================================================================
// test_PayloadOffsets
// ============================================================================

#[test]
fn to_payload_offsets() {
    common::setup();

    let test_layer = gen_test_layer();
    let root_layer = Layer::create_anonymous(Some(".usda"));

    let adj_prims = build_reference_offsets(&root_layer, &test_layer, true);
    for adj in &adj_prims {
        verify_offset(adj);
    }
}

// ============================================================================
// test_OffsetsAuthoring
//
// Tests authoring time samples through references and sublayers with offsets.
// The authored time values should be correctly inverse-transformed.
//
// This test requires EditTarget + EditContext + sublayer manipulation.
// ============================================================================

#[test]
fn to_offsets_authoring() {
    common::setup();

    let root_layer = Layer::create_anonymous(Some("root.usda"));
    let sub_layer = Layer::create_anonymous(Some("sub.usda"));
    let ref_layer = Layer::create_anonymous(Some("ref.usda"));
    let payload_layer = Layer::create_anonymous(Some("payload.usda"));

    let sub_offset = LayerOffset::new(4.0, 3.0);
    root_layer.set_sublayer_paths(&[sub_layer.identifier().to_string()]);
    root_layer.set_sublayer_offset(&sub_offset, 0);

    // Set up ref_layer with a /Bar prim
    {
        let ref_stage =
            Stage::open_with_root_layer(Arc::clone(&ref_layer), InitialLoadSet::LoadAll)
                .expect("open ref stage");
        ref_stage
            .override_prim("/Bar")
            .expect("override /Bar in ref");
    }

    // Set up payload_layer with a /Baz prim
    {
        let payload_stage =
            Stage::open_with_root_layer(Arc::clone(&payload_layer), InitialLoadSet::LoadAll)
                .expect("open payload stage");
        payload_stage
            .override_prim("/Baz")
            .expect("override /Baz in payload");
    }

    // Set up root_layer with /Foo that references /Bar in ref_layer
    let ref_offset = LayerOffset::new(1.0, 2.0);
    let authored_payload_offset = LayerOffset::new(-1.0, 1.0);
    {
        let root_stage =
            Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
                .expect("open root stage");
        root_stage.override_prim("/Foo").expect("override /Foo");
        let foo = root_stage.get_prim_at_path(&p("/Foo")).expect("get /Foo");
        // Add reference to /Bar in ref_layer with offset
        assert!(foo.get_references().add_reference_with_path(
            ref_layer.identifier(),
            &p("/Bar"),
            ref_offset.clone(),
            ListPosition::FrontOfPrependList,
        ));
        assert!(foo.get_payloads().add_payload_with_path(
            payload_layer.identifier(),
            &p("/Baz"),
            authored_payload_offset.clone(),
            ListPosition::FrontOfPrependList,
        ));
    }

    payload_layer.set_time_codes_per_second(48.0);
    let payload_offset = authored_payload_offset * LayerOffset::new(0.0, 24.0 / 48.0);

    // Open the composed stage
    let stage = Stage::open_with_root_layer(Arc::clone(&root_layer), InitialLoadSet::LoadAll)
        .expect("open composed stage");
    let foo = stage
        .get_prim_at_path(&p("/Foo"))
        .expect("get /Foo composed");

    let prim_index = foo.prim_index().expect("prim index for /Foo");
    let root_children = prim_index.root_node().children();
    let ref_node = root_children
        .iter()
        .find(|node| node.path() == p("/Bar"))
        .cloned()
        .expect("reference node for /Bar");
    let payload_node = root_children
        .iter()
        .find(|node| node.path() == p("/Baz"))
        .cloned()
        .expect("payload node for /Baz");

    // Author via EditTarget into ref_layer
    let ref_target = usd_core::EditTarget::for_layer_with_map_function(
        Arc::clone(&ref_layer),
        ref_node.map_to_root().evaluate(),
    );
    assert_eq!(ref_target.map_to_spec_path(&p("/Foo.attr")), p("/Bar.attr"));
    {
        let _ctx = usd_core::EditContext::new_with_target(Arc::clone(&stage), ref_target.clone());

        let attr = foo
            .create_attribute("attr", &common::vtn("double"), false, None)
            .expect("create /Foo.attr via ref edit target");
        attr.set(1.0_f64, TimeCode::new(2.0));
        let authored_times = ref_layer.list_time_samples_for_path(&p("/Bar.attr"));
        assert_eq!(
            authored_times.len(),
            1,
            "expected one authored sample in ref layer"
        );
        assert!(
            (authored_times[0] - ref_offset.inverse().apply(2.0)).abs() < 1e-8,
            "expected authored ref time {}, got {}",
            ref_offset.inverse().apply(2.0),
            authored_times[0]
        );
        let val = *attr
            .get(TimeCode::new(2.0))
            .expect("get /Foo.attr at t=2 through ref edit target")
            .get::<f64>()
            .expect("value not f64");
        assert_eq!(val, 1.0, "expected 1.0 at time=2.0, got {val}");
    }

    // Author via EditTarget into payload_layer
    let payload_target = usd_core::EditTarget::for_layer_with_map_function(
        Arc::clone(&payload_layer),
        payload_node.map_to_root().evaluate(),
    );
    assert_eq!(
        payload_target.map_to_spec_path(&p("/Foo.attrFromBaz")),
        p("/Baz.attrFromBaz")
    );
    {
        let _ctx = usd_core::EditContext::new_with_target(Arc::clone(&stage), payload_target);
        let attr = foo
            .create_attribute("attrFromBaz", &common::vtn("double"), false, None)
            .expect("create /Foo.attrFromBaz via payload edit target");
        attr.set(1.0_f64, TimeCode::new(2.0));
        let val = *attr
            .get(TimeCode::new(2.0))
            .expect("get /Foo.attrFromBaz at t=2")
            .get::<f64>()
            .expect("value not f64");
        assert_eq!(val, 1.0, "expected 1.0 at time=2.0, got {val}");

        let authored_times = payload_layer.list_time_samples_for_path(&p("/Baz.attrFromBaz"));
        assert_eq!(
            authored_times.len(),
            1,
            "expected one authored sample in payload layer"
        );
        assert!(
            (authored_times[0] - payload_offset.inverse().apply(2.0)).abs() < 1e-8,
            "expected authored payload time {}, got {}",
            payload_offset.inverse().apply(2.0),
            authored_times[0]
        );
    }

    // Author via EditTarget into sub_layer
    let sub_target = stage.get_edit_target_for_local_layer(&sub_layer);
    {
        let _ctx = usd_core::EditContext::new_with_target(Arc::clone(&stage), sub_target);
        let attr = foo
            .get_attribute("attr")
            .expect("get /Foo.attr for sublayer");
        attr.set(1.0_f64, TimeCode::new(2.0));
        let val = *attr
            .get(TimeCode::new(2.0))
            .expect("get /Foo.attr at t=2 through sublayer")
            .get::<f64>()
            .expect("value not f64");
        assert_eq!(val, 1.0, "expected 1.0 at time=2.0, got {val}");

        let authored_times = sub_layer.list_time_samples_for_path(&p("/Foo.attr"));
        assert_eq!(
            authored_times.len(),
            1,
            "expected one authored sample in sub layer"
        );
        assert!(
            (authored_times[0] - sub_offset.inverse().apply(2.0)).abs() < 1e-8,
            "expected authored sublayer time {}, got {}",
            sub_offset.inverse().apply(2.0),
            authored_times[0]
        );
    }
}

// ============================================================================
// test_LayerOffsetArithmetic
//
// Standalone test for LayerOffset operations used by offset composition.
// Not from reference — added for confidence in the building blocks.
// ============================================================================

#[test]
fn to_layer_offset_arithmetic() {
    common::setup();

    // Identity
    let id = LayerOffset::identity();
    assert_eq!(id.apply(5.0), 5.0);

    // Simple offset
    let off = LayerOffset::new(10.0, 1.0);
    assert_eq!(off.apply(1.0), 11.0);
    assert_eq!(off.apply(0.0), 10.0);

    // Simple scale
    let sc = LayerOffset::new(0.0, 2.0);
    assert_eq!(sc.apply(1.0), 2.0);
    assert_eq!(sc.apply(5.0), 10.0);

    // Combined offset + scale: t -> scale*t + offset
    let combined = LayerOffset::new(3.0, 2.0);
    assert_eq!(combined.apply(1.0), 5.0); // 2*1+3 = 5
    assert_eq!(combined.apply(0.0), 3.0); // 2*0+3 = 3

    // Composition: (a * b).apply(t) = a.apply(b.apply(t))
    let a = LayerOffset::new(1.0, 2.0); // t -> 2t+1
    let b = LayerOffset::new(3.0, 0.5); // t -> 0.5t+3
    let composed = a * b; // t -> 2*(0.5t+3)+1 = t+7
    assert!((composed.apply(0.0) - 7.0).abs() < 1e-10);
    assert!((composed.apply(1.0) - 8.0).abs() < 1e-10);
    assert!((composed.apply(10.0) - 17.0).abs() < 1e-10);

    // Inverse
    let off2 = LayerOffset::new(5.0, 2.0); // t -> 2t+5
    let inv = off2.inverse(); // t -> (t-5)/2
    assert!((inv.apply(5.0) - 0.0).abs() < 1e-10);
    assert!((inv.apply(7.0) - 1.0).abs() < 1e-10);
    assert!((inv.apply(25.0) - 10.0).abs() < 1e-10);

    // Roundtrip: offset * inverse = identity
    let roundtrip = off2 * inv;
    assert!((roundtrip.apply(42.0) - 42.0).abs() < 1e-10);

    // Mul with f64
    let off3 = LayerOffset::new(1.0, 2.0);
    let result = off3 * 3.0; // 2*3+1 = 7
    assert_eq!(result, 7.0);

    // Mul with TimeCode
    let tc = TimeCode::new(3.0);
    let off4 = LayerOffset::new(1.0, 2.0);
    let result_tc = off4 * tc; // 2*3+1 = 7
    assert_eq!(result_tc.value(), 7.0);
}
