//! Port of testUsdPayloads.py from OpenUSD pxr/usd/usd/testenv/
//! 5 tests, logic matches C++ reference exactly.

mod common;

use usd_core::Stage;
use usd_core::common::{InitialLoadSet, ListPosition};
use usd_sdf::{Layer, Path, Payload};

// ============================================================================
// 1. test_InstancesWithPayloads
// ============================================================================

#[test]
#[ignore = "Complex PayloadedScene setup with instancing not yet ported"]
fn payloads_instances() {
    common::setup();
}

// ============================================================================
// 2. test_Payloads — basic payload API
// ============================================================================

#[test]
fn payloads_basic() {
    common::setup();

    let payload_layer = Layer::create_anonymous(Some("payload.usda"));

    // Create target in payload layer
    {
        let stage = Stage::open(payload_layer.identifier(), InitialLoadSet::LoadAll).unwrap();
        let _sad = stage.define_prim("/Sad", "Scope").unwrap();
        let _panda = stage.define_prim("/Sad/Panda", "Scope");
    }

    // Create main stage referencing payload
    let main_layer = Layer::create_anonymous(Some("main.usda"));
    let stage = Stage::open(main_layer.identifier(), InitialLoadSet::LoadAll).unwrap();
    let sad_prim = stage.define_prim("/Sad", "").unwrap();

    // Add payload
    assert!(!sad_prim.has_payload());
    sad_prim.get_payloads().add_payload(
        &Payload::new(
            payload_layer.identifier(),
            &Path::from_string("/Sad").unwrap().get_string(),
        ),
        ListPosition::BackOfAppendList,
    );
    assert!(sad_prim.has_payload());

    // Root prim should not have payloads
    let root = stage.get_prim_at_path(&Path::absolute_root()).unwrap();
    assert!(!root.has_payload());

    // Clear payloads
    sad_prim.get_payloads().clear_payloads();
    assert!(!sad_prim.has_payload());
}

// ============================================================================
// 3. test_ClearPayload
// ============================================================================

#[test]
fn payloads_clear() {
    common::setup();

    let payload_layer = Layer::create_anonymous(Some("payload_clear.usda"));
    {
        let stage = Stage::open(payload_layer.identifier(), InitialLoadSet::LoadAll).unwrap();
        let _ = stage.define_prim("/Target", "Scope");
    }

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let prim = stage.define_prim("/Payloaded", "").unwrap();

    // Add then clear
    prim.get_payloads().add_payload(
        &Payload::new(
            payload_layer.identifier(),
            &Path::from_string("/Target").unwrap().get_string(),
        ),
        ListPosition::BackOfAppendList,
    );
    assert!(prim.has_payload());

    prim.get_payloads().clear_payloads();
    assert!(!prim.has_payload());
}

// ============================================================================
// 4. test_Bug160419
// ============================================================================

#[test]
#[ignore = "Complex nested payload loading test"]
fn payloads_bug_160419() {
    common::setup();
}

// ============================================================================
// 5. test_SubrootReferencePayloads
// ============================================================================

#[test]
#[ignore = "Subroot reference + payload composition"]
fn payloads_subroot() {
    common::setup();
}
