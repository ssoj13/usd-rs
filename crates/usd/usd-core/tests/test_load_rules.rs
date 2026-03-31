//! Tests for StageLoadRules.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdStageLoadUnload.py (load rules subset)

mod common;

use usd_core::load_rules::StageLoadRules;
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// Basic LoadRules API
// ============================================================================

#[test]
fn load_rules_load_all() {
    common::setup();

    let rules = StageLoadRules::load_all();

    // Everything should be loaded
    assert!(rules.is_loaded(&p("/")));
    assert!(rules.is_loaded(&p("/World")));
    assert!(rules.is_loaded(&p("/World/Model")));
}

#[test]
fn load_rules_load_none() {
    common::setup();

    let rules = StageLoadRules::load_none();

    // Nothing should be loaded
    assert!(!rules.is_loaded(&p("/")));
    assert!(!rules.is_loaded(&p("/World")));
}

#[test]
fn load_rules_load_with_descendants() {
    common::setup();

    let mut rules = StageLoadRules::load_none();

    // Load /World with descendants
    rules.load_with_descendants(&p("/World"));

    assert!(rules.is_loaded(&p("/World")));
    assert!(rules.is_loaded(&p("/World/Model")));
    assert!(rules.is_loaded(&p("/World/Model/Geom")));
    assert!(
        !rules.is_loaded(&p("/Other")),
        "paths outside /World should not be loaded"
    );

    assert!(
        rules.is_loaded_with_all_descendants(&p("/World")),
        "/World should be loaded with all descendants"
    );
}

#[test]
fn load_rules_load_without_descendants() {
    common::setup();

    let mut rules = StageLoadRules::load_none();

    // Load /World without descendants
    rules.load_without_descendants(&p("/World"));

    assert!(rules.is_loaded(&p("/World")));
    assert!(
        !rules.is_loaded(&p("/World/Child")),
        "descendants should NOT be loaded"
    );
    assert!(
        rules.is_loaded_with_no_descendants(&p("/World")),
        "/World should be loaded-no-descendants"
    );
}

#[test]
fn load_rules_unload() {
    common::setup();

    let mut rules = StageLoadRules::load_all();

    // Unload /World
    rules.unload(&p("/World"));

    assert!(!rules.is_loaded(&p("/World")));
    assert!(!rules.is_loaded(&p("/World/Child")));
    // Root should still be loaded
    assert!(rules.is_loaded(&p("/")));
    assert!(rules.is_loaded(&p("/Other")));
}

#[test]
fn load_rules_selective() {
    common::setup();

    let mut rules = StageLoadRules::load_none();

    // Load multiple specific paths
    rules.load_with_descendants(&p("/A"));
    rules.load_with_descendants(&p("/B"));

    assert!(rules.is_loaded(&p("/A")));
    assert!(rules.is_loaded(&p("/A/Child")));
    assert!(rules.is_loaded(&p("/B")));
    assert!(rules.is_loaded(&p("/B/Child")));
    assert!(!rules.is_loaded(&p("/C")));
}

#[test]
fn load_rules_unload_subtree() {
    common::setup();

    let mut rules = StageLoadRules::load_all();

    // Unload a subtree
    rules.unload(&p("/World/Heavy"));

    assert!(rules.is_loaded(&p("/World")));
    assert!(rules.is_loaded(&p("/World/Light")));
    assert!(!rules.is_loaded(&p("/World/Heavy")));
    assert!(!rules.is_loaded(&p("/World/Heavy/Child")));
}
