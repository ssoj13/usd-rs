//! Tests for PrimFlagsPredicate.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdPrimFlagsPredicate.py

mod common;

use usd_core::common::InitialLoadSet;
use usd_core::{Stage, prim_flags};
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// testSimpleParentChild — absent child of a defined parent
// ============================================================================

#[test]
fn prim_flags_simple_parent_child() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Parent", "").expect("define /Parent");
    stage
        .override_prim("/Parent/Child")
        .expect("override /Parent/Child");

    let pred = prim_flags::default_predicate().into_predicate();

    // Parent (defined via DefinePrim) should pass default predicate
    let parent = stage.get_prim_at_path(&p("/Parent")).expect("/Parent");
    assert!(
        pred.matches(parent.flags()),
        "defined /Parent should pass default predicate"
    );

    // Child (override only, not defined) should NOT pass default predicate
    let child = stage
        .get_prim_at_path(&p("/Parent/Child"))
        .expect("/Parent/Child");
    assert!(
        !pred.matches(child.flags()),
        "over-only /Parent/Child should NOT pass default predicate"
    );
}

// ============================================================================
// Additional: active/inactive prim
// ============================================================================

#[test]
fn prim_flags_active_inactive() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let active_prim = stage.define_prim("/Active", "").expect("define /Active");
    let inactive_prim = stage
        .define_prim("/Inactive", "")
        .expect("define /Inactive");

    // Deactivate one prim
    inactive_prim.set_active(false);

    let pred = prim_flags::default_predicate().into_predicate();

    assert!(
        pred.matches(active_prim.flags()),
        "active prim should pass default predicate"
    );
    assert!(
        !pred.matches(inactive_prim.flags()),
        "inactive prim should NOT pass default predicate"
    );
}

// ============================================================================
// get_children filters by default predicate
// ============================================================================

#[test]
fn prim_flags_get_children_filters() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "").expect("define /Root");
    stage
        .define_prim("/Root/Defined", "")
        .expect("define /Root/Defined");
    stage
        .override_prim("/Root/Override")
        .expect("override /Root/Override");

    let root = stage.get_prim_at_path(&p("/Root")).expect("/Root");

    // get_children uses default predicate — should only include defined children
    let children = root.get_children();
    let child_names: Vec<String> = children
        .iter()
        .map(|c| c.get_name().as_str().to_string())
        .collect();
    assert!(
        child_names.contains(&"Defined".to_string()),
        "Defined should be in children"
    );
    assert!(
        !child_names.contains(&"Override".to_string()),
        "Override-only prim should NOT be in get_children()"
    );

    // get_all_children includes all children regardless of predicate
    let all_children = root.get_all_children();
    let all_names: Vec<String> = all_children
        .iter()
        .map(|c| c.get_name().as_str().to_string())
        .collect();
    assert!(
        all_names.contains(&"Defined".to_string()),
        "Defined should be in all_children"
    );
    assert!(
        all_names.contains(&"Override".to_string()),
        "Override should be in all_children"
    );
}
