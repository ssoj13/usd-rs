//! Tests for attribute array edit operations.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdAttributeArrayEdits.cpp

mod common;

use usd_core::{EditContext, EditTarget, InitialLoadSet, Stage};
use usd_sdf::TimeCode;
use usd_vt::{Array, ArrayEditBuilder, Value};

// ============================================================================
// TestBasics — from C++ TestBasics()
// ============================================================================

/// Port of C++ TestBasics():
/// 1. Create in-memory stage with int array attribute
/// 2. Set [3, 2, 1] on root layer
/// 3. Build ArrayEdit: prepend(0), append(9)
/// 4. Author edit to session layer via EditContext
/// 5. Get composed value — expect [0, 3, 2, 1, 9]
#[test]
fn test_basics() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    let prim = stage
        .define_prim("/TestBasics", "")
        .expect("failed to define prim");

    let attr = prim
        .create_attribute("attr", &common::vtn("int[]"), false, None)
        .expect("failed to create attribute");

    // Initial: no value authored
    let initial = attr.get(TimeCode::default());
    assert!(initial.is_none(), "expected no initial value");

    // Set [3, 2, 1] on root layer (default edit target)
    let iarray = Array::from(vec![3i32, 2, 1]);
    assert!(attr.set(iarray, TimeCode::default()));

    // Verify the set value
    let val = attr
        .get(TimeCode::default())
        .expect("expected value after set");
    let arr = val.get::<Array<i32>>().expect("expected Array<i32>");
    assert_eq!(arr.as_slice(), &[3, 2, 1]);

    // Build ArrayEdit: prepend(0), append(9)
    let mut builder = ArrayEditBuilder::<i32>::new();
    builder.prepend(0);
    builder.append(9);
    let zero_nine = builder.build();

    // Author the edit to the session layer
    {
        let session = stage.get_session_layer().expect("expected session layer");
        let _ctx =
            EditContext::new_with_target(stage.clone(), EditTarget::for_local_layer(session));
        assert!(attr.set(Value::from(zero_nine), TimeCode::default()));
    }

    // Composed value: session edit over root array = [0, 3, 2, 1, 9]
    let composed = attr
        .get(TimeCode::default())
        .expect("expected composed value");
    let composed_arr = composed
        .get::<Array<i32>>()
        .expect("expected Array<i32> after composition");
    assert_eq!(
        composed_arr.as_slice(),
        &[0, 3, 2, 1, 9],
        "ArrayEdit prepend(0) + append(9) over [3,2,1] should give [0,3,2,1,9]"
    );
}
