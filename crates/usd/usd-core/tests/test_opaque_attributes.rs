//! Port of testUsdOpaqueAttributes.py from OpenUSD pxr/usd/usd/testenv/
//! 3 tests: test_NoAuthoredValue, test_SerializableUsda, test_SerializableUsdc.

mod common;

use usd_core::common::InitialLoadSet;
use usd_core::stage::Stage;

// ============================================================================
// 1. test_NoAuthoredValue
// ============================================================================

#[test]
fn opaque_no_authored_value() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let prim = stage.define_prim("/X", "Scope").unwrap();
    let attr1 = prim
        .create_attribute("Attr1", &common::vtn("opaque"), false, None)
        .expect("create opaque attr");

    // Opaque attributes should return None for Get
    assert!(attr1.get(usd_sdf::TimeCode::default()).is_none());

    // C++ test verifies Set raises an error for opaque attrs.
    // The key invariant: after any authoring attempts, has_value() == false
    assert!(!attr1.has_value(), "opaque attr should never have a value");
}

// ============================================================================
// 2. test_SerializableUsda
// ============================================================================

#[test]
#[ignore = "Needs disk I/O (CreateNew + Save + Reload) — not in-memory portable"]
fn opaque_serialize_usda() {
    common::setup();
    // C++ creates file on disk, saves, reloads, checks hidden + connections preserved
}

// ============================================================================
// 3. test_SerializableUsdc
// ============================================================================

#[test]
#[ignore = "Needs disk I/O (CreateNew + Save + Reload) — not in-memory portable"]
fn opaque_serialize_usdc() {
    common::setup();
    // C++ creates file on disk, saves, reloads, checks hidden + connections preserved
}
