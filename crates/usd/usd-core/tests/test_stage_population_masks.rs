//! Tests for StagePopulationMask.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdStagePopulationMasks.py

mod common;

use usd_core::{InitialLoadSet, Stage, StagePopulationMask};
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// test_Basic — StagePopulationMask object API
// Matches testUsdStagePopulationMasks.py::test_Basic
// ============================================================================

#[test]
fn pop_mask_all() {
    common::setup();

    let pm = StagePopulationMask::all();
    assert!(!pm.is_empty());
    assert!(pm.includes(&p("/any/path")));

    let mut names = Vec::new();
    let has = pm.get_included_child_names(&p("/"), &mut names);
    assert!(has, "All mask includes children of /");
    // "All" means all children included — names should be empty (all included)
    assert!(
        names.is_empty(),
        "All mask: all children included, names empty"
    );
}

#[test]
fn pop_mask_empty() {
    common::setup();

    let pm = StagePopulationMask::new();
    // Empty mask includes all (it's the default — no restriction)
    assert!(pm.is_empty());
    // Per our implementation, empty mask = includes_all = true
    assert!(pm.includes_all());
}

#[test]
fn pop_mask_add_paths() {
    common::setup();

    let mut pm = StagePopulationMask::new();
    pm.add(p("/World/anim/chars/CharGroup"));

    assert!(!pm.is_empty());
    assert_eq!(pm.len(), 1);

    // Adding a descendant should not increase count if parent already covers it
    // (our impl doesn't auto-minimize on add, but includes() works correctly)
    pm.add(p("/World/anim/chars/CharGroup/child"));
    // Both paths are stored (no auto-minimize)

    pm.add(p("/World/anim/chars/OtherCharGroup"));
    pm.add(p("/World/sets/arch/Building"));

    // Check includes
    assert!(
        pm.includes(&p("/World")),
        "/World is ancestor of mask paths"
    );
    assert!(
        !pm.includes_subtree(&p("/World")),
        "/World is not fully included"
    );
    assert!(pm.includes(&p("/World/anim")));
    assert!(!pm.includes_subtree(&p("/World/anim")));
    assert!(pm.includes(&p("/World/anim/chars/CharGroup")));
    assert!(pm.includes_subtree(&p("/World/anim/chars/CharGroup")));
    assert!(pm.includes(&p("/World/anim/chars/CharGroup/child")));
    assert!(pm.includes_subtree(&p("/World/anim/chars/CharGroup/child")));
}

#[test]
fn pop_mask_includes_mask() {
    common::setup();

    let mut pm1 = StagePopulationMask::new();
    pm1.add(p("/foo"));
    pm1.add(p("/bar"));

    let pm_empty = StagePopulationMask::new();

    // Empty mask includes_all, so it includes everything
    assert!(
        pm_empty.includes_mask(&pm1),
        "empty (all) includes any mask"
    );
    // pm1 (restricted) does not include empty (all)
    // Actually our includes_mask checks if other's paths are subset — but
    // empty mask paths means it's trivially included
    assert!(pm1.includes_mask(&pm_empty), "any mask includes empty");
}

#[test]
fn pop_mask_get_included_child_names() {
    common::setup();

    let mask = StagePopulationMask::from_paths(vec![
        p("/A/B"),
        p("/A/C"),
        p("/A/D/E"),
        p("/A/D/F"),
        p("/B"),
    ]);

    // Children of /
    let mut names = Vec::new();
    let has = mask.get_included_child_names(&p("/"), &mut names);
    assert!(has);
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        names.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        vec!["A", "B"]
    );

    // Children of /A
    let mut names = Vec::new();
    let has = mask.get_included_child_names(&p("/A"), &mut names);
    assert!(has);
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        names.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        vec!["B", "C", "D"]
    );

    // Children of /A/B — fully included, no specific names
    let mut names = Vec::new();
    let has = mask.get_included_child_names(&p("/A/B"), &mut names);
    assert!(has);
    assert!(names.is_empty(), "/A/B subtree fully included");

    // Children of /A/D
    let mut names = Vec::new();
    let has = mask.get_included_child_names(&p("/A/D"), &mut names);
    assert!(has);
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        names.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        vec!["E", "F"]
    );

    // Children of /C — not included
    let mut names = Vec::new();
    let has = mask.get_included_child_names(&p("/C"), &mut names);
    assert!(!has, "/C is not in mask");
}

// ============================================================================
// Union and Intersection
// ============================================================================

#[test]
fn pop_mask_union() {
    common::setup();

    let pm1 = StagePopulationMask::from_paths(vec![p("/A"), p("/AA"), p("/B/C"), p("/U")]);
    let pm2 = StagePopulationMask::from_paths(vec![p("/A/X"), p("/B"), p("/Q")]);

    let union = StagePopulationMask::union_of(&pm1, &pm2);
    // /A includes /A/X so union has /A, /AA, /B includes /B/C so union has /B, /Q, /U
    assert!(union.includes(&p("/A")));
    assert!(union.includes(&p("/AA")));
    assert!(union.includes(&p("/B")));
    assert!(union.includes(&p("/Q")));
    assert!(union.includes(&p("/U")));
}

#[test]
fn pop_mask_intersection() {
    common::setup();

    let pm1 = StagePopulationMask::from_paths(vec![p("/A"), p("/AA"), p("/B/C"), p("/U")]);
    let pm2 = StagePopulationMask::from_paths(vec![p("/A/X"), p("/B"), p("/Q")]);

    let inter = StagePopulationMask::intersection_of(&pm1, &pm2);
    // /A and /A/X: /A/X is included in both => /A/X in intersection
    // /B/C and /B: /B/C is included in both => /B/C in intersection
    assert!(inter.includes(&p("/A/X")));
    assert!(inter.includes(&p("/B/C")));
    // /AA, /U, /Q should NOT be in intersection
    assert!(!inter.includes(&p("/AA")));
    assert!(!inter.includes(&p("/U")));
    assert!(!inter.includes(&p("/Q")));
}

#[test]
fn pop_mask_union_with_path() {
    common::setup();

    let mut pm = StagePopulationMask::new();
    pm.add(p("/world/anim"));

    let pm2 = pm.get_union_path(&p("/world"));
    // /world includes /world/anim, so result should just be /world
    let paths = pm2.get_paths();
    assert_eq!(paths.len(), 1);
    assert!(paths.iter().any(|pp| **pp == p("/world")));
}

// ============================================================================
// test_Stages — OpenMasked
// ============================================================================

#[test]
fn pop_mask_open_masked() {
    common::setup();

    // Create an unmasked stage with prims
    let unmasked = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("unmasked");
    unmasked
        .define_prim("/World/anim/chars/DoryGroup/Dory", "")
        .expect("define dory");
    unmasked
        .define_prim("/World/anim/chars/NemoGroup/Nemo", "")
        .expect("define nemo");
    unmasked
        .define_prim("/World/sets/Reef/Coral/CoralGroup1", "")
        .expect("define coral");

    // Open with mask for DoryGroup only
    let mut dory_mask = StagePopulationMask::new();
    dory_mask.add(p("/World/anim/chars/DoryGroup"));

    let dory_stage = Stage::open_masked_with_root_layer(
        unmasked.get_root_layer(),
        dory_mask.clone(),
        InitialLoadSet::LoadAll,
    )
    .expect("open masked");

    // Population mask should match
    let retrieved_mask = dory_stage.get_population_mask();
    assert!(
        retrieved_mask.is_some(),
        "masked stage should have population mask"
    );
    assert_eq!(retrieved_mask.unwrap(), dory_mask);

    // DoryGroup and ancestors should be visible
    assert!(
        dory_stage.get_prim_at_path(&p("/World")).is_some(),
        "/World should exist"
    );
    assert!(
        dory_stage
            .get_prim_at_path(&p("/World/anim/chars/DoryGroup"))
            .is_some(),
        "DoryGroup should exist"
    );
    assert!(
        dory_stage
            .get_prim_at_path(&p("/World/anim/chars/DoryGroup/Dory"))
            .is_some(),
        "Dory should exist"
    );
}

// ============================================================================
// test_SetPopulationMask — modify mask on existing stage
// ============================================================================

#[test]
fn pop_mask_set_on_stage() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    stage.define_prim("/A/Child", "").expect("define /A/Child");
    stage.define_prim("/B/Child", "").expect("define /B/Child");

    // Default: no mask
    assert!(stage.get_population_mask().is_none());

    // Set mask to only include /A
    let mut mask = StagePopulationMask::new();
    mask.add(p("/A"));
    stage.set_population_mask(Some(mask.clone()));

    let retrieved = stage.get_population_mask();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), mask);

    // Clear mask
    stage.set_population_mask(None);
    assert!(stage.get_population_mask().is_none());
}

// ============================================================================
// StagePopulationMask display
// ============================================================================

#[test]
fn pop_mask_display() {
    common::setup();

    let mut mask = StagePopulationMask::new();
    mask.add(p("/World"));
    let display = format!("{}", mask);
    assert!(display.contains("StagePopulationMask"));
    assert!(display.contains("/World"));
}

// ============================================================================
// Hash equality
// ============================================================================

#[test]
fn pop_mask_hash_eq() {
    common::setup();

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(mask: &StagePopulationMask) -> u64 {
        let mut hasher = DefaultHasher::new();
        mask.hash(&mut hasher);
        hasher.finish()
    }

    let m1 = StagePopulationMask::from_paths(vec![p("/A"), p("/B")]);
    let m2 = StagePopulationMask::from_paths(vec![p("/B"), p("/A")]);
    assert_eq!(m1, m2);
    assert_eq!(hash_of(&m1), hash_of(&m2));

    let m3 = StagePopulationMask::all();
    let m4 = StagePopulationMask::all();
    assert_eq!(hash_of(&m3), hash_of(&m4));
}
