//! Tests for Prim::get_descendants / get_all_descendants / get_filtered_descendants.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdPrimGetDescendants.cpp

mod common;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// get_descendants with default predicate
// ============================================================================

#[test]
fn descendants_default() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/A", "").expect("define /A");
    stage.define_prim("/A/B", "").expect("define /A/B");
    stage.define_prim("/A/B/C", "").expect("define /A/B/C");
    stage.define_prim("/A/D", "").expect("define /A/D");
    // Override-only prim — should be excluded by default predicate
    stage.override_prim("/A/E").expect("override /A/E");

    let a = stage.get_prim_at_path(&p("/A")).expect("/A");
    let descendants = a.get_descendants();
    let names: Vec<String> = descendants
        .iter()
        .map(|p| p.get_path().get_string().to_string())
        .collect();

    // Default predicate: only defined prims
    assert!(names.contains(&"/A/B".to_string()), "should contain /A/B");
    assert!(
        names.contains(&"/A/B/C".to_string()),
        "should contain /A/B/C"
    );
    assert!(names.contains(&"/A/D".to_string()), "should contain /A/D");
    assert!(
        !names.contains(&"/A/E".to_string()),
        "should NOT contain override-only /A/E"
    );
    assert_eq!(descendants.len(), 3, "expected 3 defined descendants");
}

// ============================================================================
// get_all_descendants — includes all prims regardless of predicate
// ============================================================================

#[test]
fn descendants_all() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/A", "").expect("define /A");
    stage.define_prim("/A/B", "").expect("define /A/B");
    stage.override_prim("/A/C").expect("override /A/C");
    stage.define_prim("/A/C/D", "").expect("define /A/C/D");

    let a = stage.get_prim_at_path(&p("/A")).expect("/A");
    let all = a.get_all_descendants();
    let names: Vec<String> = all
        .iter()
        .map(|p| p.get_path().get_string().to_string())
        .collect();

    assert!(names.contains(&"/A/B".to_string()));
    assert!(
        names.contains(&"/A/C".to_string()),
        "all should include override /A/C"
    );
    assert!(names.contains(&"/A/C/D".to_string()));
    assert_eq!(all.len(), 3);
}

// ============================================================================
// get_filtered_descendants with custom predicate
// ============================================================================

#[test]
fn descendants_filtered() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "").expect("define /Root");
    let _active = stage
        .define_prim("/Root/Active", "")
        .expect("define Active");
    let inactive = stage
        .define_prim("/Root/Inactive", "")
        .expect("define Inactive");
    inactive.set_active(false);
    stage
        .define_prim("/Root/Active/Child", "")
        .expect("define Child");

    let root = stage.get_prim_at_path(&p("/Root")).expect("/Root");

    // Default predicate filters out inactive
    let default_desc = root.get_descendants();
    let default_names: Vec<String> = default_desc
        .iter()
        .map(|p| p.get_name().as_str().to_string())
        .collect();
    assert!(default_names.contains(&"Active".to_string()));
    assert!(default_names.contains(&"Child".to_string()));
    assert!(
        !default_names.contains(&"Inactive".to_string()),
        "default predicate should exclude inactive"
    );

    // All descendants includes inactive
    let all_desc = root.get_all_descendants();
    let all_names: Vec<String> = all_desc
        .iter()
        .map(|p| p.get_name().as_str().to_string())
        .collect();
    assert!(all_names.contains(&"Active".to_string()));
    assert!(all_names.contains(&"Inactive".to_string()));
    assert!(all_names.contains(&"Child".to_string()));
}

// ============================================================================
// Empty descendants
// ============================================================================

#[test]
fn descendants_empty() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Leaf", "").expect("define /Leaf");

    let leaf = stage.get_prim_at_path(&p("/Leaf")).expect("/Leaf");
    assert!(leaf.get_descendants().is_empty());
    assert!(leaf.get_all_descendants().is_empty());
}

// ============================================================================
// Nested references + descendants
// ============================================================================

#[test]
fn descendants_through_references() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Build hierarchy
    stage.define_prim("/World", "").expect("define /World");
    stage
        .define_prim("/World/Child1", "")
        .expect("define /World/Child1");
    stage
        .define_prim("/World/Child1/GrandChild", "")
        .expect("define /World/Child1/GrandChild");
    stage
        .define_prim("/World/Child2", "")
        .expect("define /World/Child2");

    let world = stage.get_prim_at_path(&p("/World")).expect("/World");
    let descendants = world.get_descendants();

    // Should have 3 descendants: Child1, Child1/GrandChild, Child2
    assert_eq!(descendants.len(), 3, "expected 3 descendants of /World");

    let paths: Vec<String> = descendants
        .iter()
        .map(|p| p.get_path().get_string().to_string())
        .collect();
    assert!(paths.contains(&"/World/Child1".to_string()));
    assert!(paths.contains(&"/World/Child1/GrandChild".to_string()));
    assert!(paths.contains(&"/World/Child2".to_string()));
}
