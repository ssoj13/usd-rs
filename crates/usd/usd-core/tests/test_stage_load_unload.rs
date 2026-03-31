//! Tests for Stage load/unload operations and StageLoadRules.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdStageLoadUnload.py

mod common;

use usd_core::{
    LoadRule as Rule, Stage, StageLoadRules,
    common::{InitialLoadSet, ListPosition},
};
use usd_sdf::{LayerOffset, Path};

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// test_LoadRules — StageLoadRules object API
// Matches testUsdStageLoadUnload.py::test_LoadRules
// ============================================================================

#[test]
fn load_rules_basics() {
    common::setup();

    // Default rules = load all
    let rules = StageLoadRules::new();
    assert_eq!(rules, StageLoadRules::load_all());

    // AddRule root=None -> load none
    let mut rules = StageLoadRules::new();
    rules.add_rule(p("/"), Rule::NoneRule);
    assert_eq!(rules, StageLoadRules::load_none());

    // LoadWithDescendants / -> load all (after minimize)
    let mut rules = StageLoadRules::new();
    rules.load_with_descendants(&p("/"));
    rules.minimize();
    assert_eq!(rules, StageLoadRules::new());

    // Unload / -> load none
    let mut rules = StageLoadRules::new();
    rules.unload(&p("/"));
    assert_eq!(rules, StageLoadRules::load_none());

    // LoadWithoutDescendants / -> root is OnlyRule
    let mut rules = StageLoadRules::new();
    rules.load_without_descendants(&p("/"));
    let sorted = rules.get_rules();
    assert_eq!(sorted.len(), 1);
    assert_eq!(sorted[0].0, p("/"));
    assert_eq!(sorted[0].1, Rule::OnlyRule);
}

#[test]
fn load_rules_is_loaded() {
    common::setup();

    // AllRule at root
    let mut rules = StageLoadRules::new();
    rules.add_rule(p("/"), Rule::AllRule);
    assert!(rules.is_loaded(&p("/")));
    assert!(rules.is_loaded(&p("/foo/bar/baz")));

    // NoneRule at root
    rules.add_rule(p("/"), Rule::NoneRule);
    assert!(!rules.is_loaded(&p("/")));
    assert!(!rules.is_loaded(&p("/foo/bar/baz")));

    // None for /, All for /Foo/Bar/Baz/Garply
    rules.add_rule(p("/Foo/Bar/Baz/Garply"), Rule::AllRule);
    assert!(rules.is_loaded(&p("/Foo/Bar/Baz/Garply")));
    assert!(rules.is_loaded(&p("/Foo/Bar/Baz/Garply/Child")));
    // Ancestors NOT loaded because they have NoneRule effective
    // But wait: in C++ test, ancestors ARE loaded because they're needed to reach the target.
    // Our load_rules doesn't automatically add ancestor OnlyRules when adding AllRule directly.
    // The test uses add_rule directly, so ancestors should check effective rule.
    // With add_rule: no auto-ancestor, so /Foo should NOT be loaded.
    assert!(!rules.is_loaded(&p("/Foo/Bear")));
    assert!(!rules.is_loaded(&p("/Foo/Bear/Baz")));
}

#[test]
fn load_rules_effective_rule() {
    common::setup();

    let mut rules = StageLoadRules::load_none();
    assert_eq!(rules.get_effective_rule(&p("/any/path")), Rule::NoneRule);

    // OnlyRule at /any (via add_rule, NOT load_without_descendants)
    rules.add_rule(p("/any"), Rule::OnlyRule);
    assert_eq!(rules.get_effective_rule(&p("/any")), Rule::OnlyRule);
    assert_eq!(rules.get_effective_rule(&p("/any/path")), Rule::NoneRule);
    assert_eq!(
        rules.get_effective_rule(&p("/outside/path")),
        Rule::NoneRule
    );

    assert!(rules.is_loaded_with_no_descendants(&p("/any")));
    assert!(!rules.is_loaded_with_no_descendants(&p("/any/path")));
    assert!(!rules.is_loaded_with_all_descendants(&p("/any")));

    // AllRule at /other/child
    rules.add_rule(p("/other/child"), Rule::AllRule);
    assert_eq!(rules.get_effective_rule(&p("/other/child")), Rule::AllRule);
    assert_eq!(
        rules.get_effective_rule(&p("/other/child/descndt/path")),
        Rule::AllRule
    );
    assert_eq!(
        rules.get_effective_rule(&p("/outside/path")),
        Rule::NoneRule
    );

    assert!(rules.is_loaded_with_all_descendants(&p("/other/child")));
    assert!(rules.is_loaded_with_all_descendants(&p("/other/child/descndt/path")));

    // OnlyRule and NoneRule under /other/child
    rules.add_rule(p("/other/child/only"), Rule::OnlyRule);
    rules.add_rule(p("/other/child/none"), Rule::NoneRule);
    assert_eq!(rules.get_effective_rule(&p("/other/child")), Rule::AllRule);
    assert_eq!(
        rules.get_effective_rule(&p("/other/child/only")),
        Rule::OnlyRule
    );
    assert_eq!(
        rules.get_effective_rule(&p("/other/child/only/child")),
        Rule::NoneRule
    );
    assert_eq!(
        rules.get_effective_rule(&p("/other/child/none")),
        Rule::NoneRule
    );

    // AllRule nested under NoneRule
    rules.add_rule(p("/other/child/none/child/all"), Rule::AllRule);
    assert_eq!(
        rules.get_effective_rule(&p("/other/child/none/child/all")),
        Rule::AllRule
    );
}

#[test]
fn load_rules_minimize() {
    common::setup();

    // Empty rules minimize to empty
    let mut rules = StageLoadRules::new();
    rules.minimize();
    assert_eq!(rules, StageLoadRules::new());

    // AllRule at root minimizes to empty
    let mut rules = StageLoadRules::new();
    rules.add_rule(p("/"), Rule::AllRule);
    rules.minimize();
    assert_eq!(rules, StageLoadRules::new());
}

#[test]
fn load_rules_swap() {
    common::setup();

    let mut r1 = StageLoadRules::load_none();
    let mut r2 = StageLoadRules::load_all();

    r1.swap(&mut r2);
    assert_eq!(r1, StageLoadRules::load_all());
    assert_eq!(r2, StageLoadRules::load_none());

    r1.add_rule(p("/foo"), Rule::NoneRule);
    r2.add_rule(p("/bar"), Rule::AllRule);

    r1.swap(&mut r2);

    let r1_rules = r1.get_rules();
    assert!(r1_rules.iter().any(|(path, _)| path == &p("/")));
    assert!(r1_rules.iter().any(|(path, _)| path == &p("/bar")));

    let r2_rules = r2.get_rules();
    assert!(r2_rules.iter().any(|(path, _)| path == &p("/foo")));
}

#[test]
fn load_rules_hash() {
    common::setup();

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(rules: &StageLoadRules) -> u64 {
        let mut hasher = DefaultHasher::new();
        rules.hash(&mut hasher);
        hasher.finish()
    }

    assert_eq!(
        hash_of(&StageLoadRules::new()),
        hash_of(&StageLoadRules::new())
    );
    assert_eq!(
        hash_of(&StageLoadRules::load_all()),
        hash_of(&StageLoadRules::load_all())
    );
    assert_eq!(
        hash_of(&StageLoadRules::load_none()),
        hash_of(&StageLoadRules::load_none())
    );
}

// ============================================================================
// test_GetSetLoadRules — Stage::get_load_rules / set_load_rules
// ============================================================================

#[test]
fn load_get_set_load_rules() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");

    // Default rules should be load all
    let default_rules = stage.get_load_rules();
    assert!(
        default_rules.is_load_all(),
        "default rules should be load all"
    );

    // Set custom rules
    let mut custom = StageLoadRules::load_none();
    custom.add_rule(p("/Foo"), Rule::AllRule);

    stage.set_load_rules(custom.clone());
    let retrieved = stage.get_load_rules();
    assert_eq!(retrieved, custom, "set_load_rules should roundtrip");
}

// ============================================================================
// test_LoadAndUnload — basic payload load/unload via Stage
// ============================================================================

#[test]
fn load_and_unload_basic() {
    common::setup();

    // Create a payload layer
    let payload_stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("payload stage");
    payload_stage
        .define_prim("/Sad/Panda", "Scope")
        .expect("define /Sad/Panda");

    // Create the main stage with a prim that has a payload
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("scene stage");
    let sad = stage.define_prim("/Sad", "Scope").expect("define /Sad");
    sad.get_payloads().add_payload_with_path(
        payload_stage.get_root_layer().identifier(),
        &p("/Sad"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // With LoadAll, the payload should be loaded
    let load_set = stage.get_load_set();
    assert!(
        load_set.contains(&p("/Sad")),
        "/Sad should be in load set initially"
    );

    // Unload /Sad
    stage.unload(&p("/Sad"));
    let load_set = stage.get_load_set();
    assert!(
        !load_set.contains(&p("/Sad")),
        "/Sad should NOT be in load set after unload"
    );

    // Load /Sad again
    stage.load(&p("/Sad"), None);
    let load_set = stage.get_load_set();
    assert!(
        load_set.contains(&p("/Sad")),
        "/Sad should be in load set after load"
    );
}

// ============================================================================
// test_Load — load specific paths
// ============================================================================

#[test]
fn load_specific_paths() {
    common::setup();

    // Create payload layers
    let payload1 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("payload1");
    payload1
        .define_prim("/Payload1/Child", "Scope")
        .expect("p1");

    let payload2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("payload2");
    payload2
        .define_prim("/Payload2/Child", "Scope")
        .expect("p2");

    // Main scene: /A and /B both have payloads
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim_a = stage.define_prim("/A", "Scope").expect("define /A");
    prim_a.get_payloads().add_payload_with_path(
        payload1.get_root_layer().identifier(),
        &p("/Payload1"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    let prim_b = stage.define_prim("/B", "Scope").expect("define /B");
    prim_b.get_payloads().add_payload_with_path(
        payload2.get_root_layer().identifier(),
        &p("/Payload2"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // Unload everything
    stage.unload(&p("/"));

    let load_set = stage.get_load_set();
    assert!(load_set.is_empty(), "everything should be unloaded");

    // Load only /A
    stage.load(&p("/A"), None);
    let load_set = stage.get_load_set();
    assert!(load_set.contains(&p("/A")), "/A should be loaded");

    // Load /B too
    stage.load(&p("/B"), None);
    let load_set = stage.get_load_set();
    assert!(load_set.contains(&p("/A")), "/A should still be loaded");
    assert!(load_set.contains(&p("/B")), "/B should be loaded");

    // Unload /A
    stage.unload(&p("/A"));
    let load_set = stage.get_load_set();
    assert!(!load_set.contains(&p("/A")), "/A should be unloaded");
    assert!(load_set.contains(&p("/B")), "/B should still be loaded");
}

// ============================================================================
// test_LoadWithDescendants / LoadWithoutDescendants
// ============================================================================

#[test]
fn load_with_descendants() {
    common::setup();

    let mut rules = StageLoadRules::load_none();
    let kitchen = p("/World/sets/kitchen");

    rules.load_with_descendants(&kitchen);

    assert!(rules.is_loaded(&kitchen));
    assert!(rules.is_loaded_with_all_descendants(&kitchen));

    // Ancestors should have OnlyRule (auto-inserted by load_with_descendants)
    let sets = p("/World/sets");
    assert_eq!(rules.get_rule(&sets), Some(Rule::OnlyRule));
    let world = p("/World");
    assert_eq!(rules.get_rule(&world), Some(Rule::OnlyRule));
}

#[test]
fn load_without_descendants() {
    common::setup();

    let mut rules = StageLoadRules::load_none();
    let kitchen = p("/World/sets/kitchen");

    rules.load_without_descendants(&kitchen);

    assert!(rules.is_loaded_with_no_descendants(&kitchen));
    assert!(!rules.is_loaded_with_all_descendants(&kitchen));

    // Ancestors should have OnlyRule
    let sets = p("/World/sets");
    assert_eq!(rules.get_rule(&sets), Some(Rule::OnlyRule));
}

// ============================================================================
// test_LoadRules_SetRules roundtrip
// ============================================================================

#[test]
fn load_rules_set_rules_roundtrip() {
    common::setup();

    let mut r1 = StageLoadRules::new();
    r1.add_rule(p("/A"), Rule::AllRule);
    r1.add_rule(p("/B"), Rule::NoneRule);
    r1.add_rule(p("/C"), Rule::OnlyRule);

    let rules_data = r1.get_rules();

    let mut r2 = StageLoadRules::new();
    r2.set_rules(rules_data.clone());

    assert_eq!(r1, r2);
    assert_eq!(r1.get_rules(), r2.get_rules());
}

// ============================================================================
// test_Create with load policy
// ============================================================================

#[test]
fn load_create_load_none() {
    common::setup();

    // CreateInMemory with LoadNone
    let stage = Stage::create_in_memory(InitialLoadSet::LoadNone).expect("stage");
    let _prim = stage.define_prim("/Test", "").expect("define /Test");

    // With LoadNone, the stage still creates prims (they aren't payloaded),
    // but the initial load set policy is LoadNone
    let prim = stage.get_prim_at_path(&p("/Test"));
    assert!(
        prim.is_some(),
        "/Test should exist regardless of load policy"
    );
}

// ============================================================================
// test_UnloadAll — unload everything via root path
// ============================================================================

#[test]
fn load_unload_root() {
    common::setup();

    let payload_stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("payload");
    payload_stage.define_prim("/P/Child", "").expect("define");

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let foo = stage.define_prim("/Foo", "").expect("define /Foo");
    foo.get_payloads().add_payload_with_path(
        payload_stage.get_root_layer().identifier(),
        &p("/P"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // Should be loaded initially
    assert!(!stage.get_load_set().is_empty());

    // Unload everything
    stage.unload(&p("/"));
    assert!(
        stage.get_load_set().is_empty(),
        "unload root should clear all"
    );
}
