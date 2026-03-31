//! Tests for PrimRange traversal.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdPrimRange.py (11 methods)

mod common;

use usd_core::{
    ModelAPI, Prim, PrimRange, Stage,
    common::{InitialLoadSet, ListPosition},
    prim_flags::{
        PrimFlagsPredicate, USD_PRIM_IS_ABSTRACT, USD_PRIM_IS_ACTIVE, USD_PRIM_IS_DEFINED,
        USD_PRIM_IS_GROUP, USD_PRIM_IS_INSTANCE, USD_PRIM_IS_LOADED, USD_PRIM_IS_MODEL,
    },
};
use usd_sdf::{LayerOffset, Path};
use usd_tf::Token;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

fn prim_paths(prims: &[Prim]) -> Vec<String> {
    prims
        .iter()
        .map(|pr| pr.get_path().get_string().to_string())
        .collect()
}

// ============================================================================
// test_PrimIsDefined
// ============================================================================

#[test]
fn prim_range_is_defined() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let pseudo_root = stage.pseudo_root();
    let _foo = stage.define_prim("/Foo", "Mesh").expect("define /Foo");
    let _bar = stage.override_prim("/Bar").expect("override /Bar");
    let _faz = stage
        .define_prim("/Foo/Faz", "Mesh")
        .expect("define /Foo/Faz");
    let _baz = stage
        .define_prim("/Bar/Baz", "Mesh")
        .expect("define /Bar/Baz");

    // Default PrimRange from pseudo_root should skip undefined prims (/Bar is override)
    let range = PrimRange::from_prim(&pseudo_root);
    let prims: Vec<Prim> = range.into_iter().collect();
    let paths = prim_paths(&prims);
    assert!(
        paths.contains(&"/".to_string()),
        "should include pseudo_root"
    );
    assert!(paths.contains(&"/Foo".to_string()), "should include /Foo");
    assert!(
        paths.contains(&"/Foo/Faz".to_string()),
        "should include /Foo/Faz"
    );

    // With explicit IsDefined predicate
    let defined_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_DEFINED);
    let range2 = PrimRange::from_prim_with_predicate(&pseudo_root, defined_pred);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    let paths2 = prim_paths(&prims2);
    assert!(paths2.contains(&"/Foo".to_string()));
    assert!(paths2.contains(&"/Foo/Faz".to_string()));

    // With negated IsDefined — should return undefined prims
    let bar = stage.get_prim_at_path(&p("/Bar")).expect("/Bar");
    let not_defined_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_DEFINED.not());
    let range3 = PrimRange::from_prim_with_predicate(&bar, not_defined_pred);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    let paths3 = prim_paths(&prims3);
    assert!(
        paths3.contains(&"/Bar".to_string()),
        "/Bar should be in ~IsDefined range"
    );
    assert!(
        paths3.contains(&"/Bar/Baz".to_string()),
        "/Bar/Baz should be in ~IsDefined range"
    );
}

// ============================================================================
// test_PrimIsActive
// ============================================================================

#[test]
fn prim_range_is_active() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let foo = stage.define_prim("/Foo", "Mesh").expect("define /Foo");
    let bar = stage
        .define_prim("/Foo/Bar", "Mesh")
        .expect("define /Foo/Bar");
    let baz = stage
        .define_prim("/Foo/Bar/Baz", "Mesh")
        .expect("define /Foo/Bar/Baz");
    baz.set_active(false);

    // Active predicate — should not traverse into inactive prims
    let active_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_ACTIVE);
    let range = PrimRange::from_prim_with_predicate(&foo, active_pred);
    let prims: Vec<Prim> = range.into_iter().collect();
    let paths = prim_paths(&prims);
    assert_eq!(
        paths,
        vec!["/Foo", "/Foo/Bar"],
        "active traversal from /Foo"
    );

    // Predicate is checked on iteration root too
    let range2 = PrimRange::from_prim_with_predicate(&baz, active_pred);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    assert!(prims2.is_empty(), "inactive root should yield empty range");

    // Negated: only inactive prims
    let not_active_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_ACTIVE.not());
    let range3 = PrimRange::from_prim_with_predicate(&baz, not_active_pred);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    let paths3 = prim_paths(&prims3);
    assert_eq!(
        paths3,
        vec!["/Foo/Bar/Baz"],
        "negated active gives inactive baz"
    );

    // Active from /Foo/Bar — /Foo/Bar/Baz is inactive, should be excluded
    let range4 = PrimRange::from_prim_with_predicate(&bar, active_pred);
    let prims4: Vec<Prim> = range4.into_iter().collect();
    let paths4 = prim_paths(&prims4);
    assert_eq!(
        paths4,
        vec!["/Foo/Bar"],
        "active from /Bar excludes inactive /Baz"
    );
}

// ============================================================================
// test_PrimIsAbstract
// ============================================================================

#[test]
fn prim_range_abstract() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let group = stage.define_prim("/Group", "Xform").expect("define /Group");
    let class_prim = stage
        .create_class_prim("/class_Model")
        .expect("create class");

    // Abstract predicate — group is not abstract
    let abstract_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_ABSTRACT);
    let range1 = PrimRange::from_prim_with_predicate(&group, abstract_pred);
    let prims1: Vec<Prim> = range1.into_iter().collect();
    assert!(prims1.is_empty(), "group is not abstract");

    // Negated abstract — group should appear
    let not_abstract_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_ABSTRACT.not());
    let range2 = PrimRange::from_prim_with_predicate(&group, not_abstract_pred);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    let paths2 = prim_paths(&prims2);
    assert_eq!(paths2, vec!["/Group"], "not-abstract includes /Group");

    // Abstract predicate — class_prim is abstract
    let range3 = PrimRange::from_prim_with_predicate(&class_prim, abstract_pred);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    let paths3 = prim_paths(&prims3);
    assert_eq!(paths3, vec!["/class_Model"], "abstract includes class prim");

    // Negated abstract — class_prim should not appear
    let range4 = PrimRange::from_prim_with_predicate(&class_prim, not_abstract_pred);
    let prims4: Vec<Prim> = range4.into_iter().collect();
    assert!(prims4.is_empty(), "not-abstract excludes class prim");
}

// ============================================================================
// test_PrimIsLoaded (payload-based)
// ============================================================================

#[test]
fn prim_range_loaded() {
    common::setup();

    let payload_stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("payload stage");
    let _payload_prim = payload_stage
        .define_prim("/Payload", "Scope")
        .expect("define /Payload");

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("scene stage");
    let foo = stage.define_prim("/Foo", "").expect("define /Foo");
    foo.get_payloads().add_payload_with_path(
        payload_stage.get_root_layer().identifier(),
        &p("/Payload"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // Load set should include /Foo
    let load_set = stage.get_load_set();
    assert!(load_set.contains(&p("/Foo")), "/Foo should be in load set");

    // Unload /Foo
    stage.unload(&p("/Foo"));

    // ~IsLoaded should include unloaded /Foo
    let not_loaded = PrimFlagsPredicate::from_term(USD_PRIM_IS_LOADED.not());
    let range1 = PrimRange::from_prim_with_predicate(&foo, not_loaded);
    let prims1: Vec<Prim> = range1.into_iter().collect();
    let paths1 = prim_paths(&prims1);
    assert!(
        paths1.contains(&"/Foo".to_string()),
        "unloaded /Foo in ~IsLoaded range"
    );

    // IsLoaded should NOT include unloaded /Foo
    let loaded = PrimFlagsPredicate::from_term(USD_PRIM_IS_LOADED);
    let range2 = PrimRange::from_prim_with_predicate(&foo, loaded);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    assert!(
        prims2.is_empty(),
        "unloaded /Foo should not be in IsLoaded range"
    );

    // Reload /Foo
    stage.load(&p("/Foo"), None);

    // ~IsLoaded should NOT include loaded /Foo
    let range3 = PrimRange::from_prim_with_predicate(&foo, not_loaded);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    assert!(
        prims3.is_empty(),
        "loaded /Foo should not be in ~IsLoaded range"
    );

    // IsLoaded should include loaded /Foo
    let range4 = PrimRange::from_prim_with_predicate(&foo, loaded);
    let prims4: Vec<Prim> = range4.into_iter().collect();
    let paths4 = prim_paths(&prims4);
    assert!(
        paths4.contains(&"/Foo".to_string()),
        "loaded /Foo in IsLoaded range"
    );
}

// ============================================================================
// test_PrimIsModelOrGroup
// ============================================================================

#[test]
fn prim_range_model_group() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let group = stage.define_prim("/Group", "Xform").expect("define /Group");
    ModelAPI::new(group.clone()).set_kind(&Token::new("group"));
    let model = stage
        .define_prim("/Group/Model", "")
        .expect("define /Group/Model");
    ModelAPI::new(model.clone()).set_kind(&Token::new("model"));
    let _mesh = stage
        .define_prim("/Group/Model/Sbdv", "Mesh")
        .expect("define mesh");

    // IsModel should include group and model (groups are models in USD)
    let model_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_MODEL);
    let range1 = PrimRange::from_prim_with_predicate(&group, model_pred);
    let prims1: Vec<Prim> = range1.into_iter().collect();
    let paths1 = prim_paths(&prims1);
    assert!(paths1.contains(&"/Group".to_string()), "group is a model");
    assert!(
        paths1.contains(&"/Group/Model".to_string()),
        "model is a model"
    );

    // IsGroup should only include group
    let group_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_GROUP);
    let range2 = PrimRange::from_prim_with_predicate(&group, group_pred);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    let paths2 = prim_paths(&prims2);
    assert!(paths2.contains(&"/Group".to_string()), "group is a group");

    // ~IsModel from group should give empty (group is a model, root fails)
    let not_model_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_MODEL.not());
    let range3 = PrimRange::from_prim_with_predicate(&group, not_model_pred);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    assert!(prims3.is_empty(), "~IsModel from model-root gives empty");

    // ~IsModel from mesh should include the mesh
    let mesh = stage
        .get_prim_at_path(&p("/Group/Model/Sbdv"))
        .expect("mesh");
    let range4 = PrimRange::from_prim_with_predicate(&mesh, not_model_pred);
    let prims4: Vec<Prim> = range4.into_iter().collect();
    let paths4 = prim_paths(&prims4);
    assert!(
        paths4.contains(&"/Group/Model/Sbdv".to_string()),
        "mesh is not a model"
    );
}

// ============================================================================
// test_PrimIsInstanceOrPrototypeOrRoot
// ============================================================================

#[test]
fn prim_range_instance_proto() {
    common::setup();

    let ref_stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("ref stage");
    ref_stage
        .define_prim("/Ref/Child", "")
        .expect("define /Ref/Child");

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("scene stage");
    let _root = stage.define_prim("/Root", "").expect("define /Root");

    let instance = stage
        .define_prim("/Root/Instance", "")
        .expect("define instance");
    instance.get_references().add_reference_with_path(
        ref_stage.get_root_layer().identifier(),
        &p("/Ref"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );
    instance.set_instanceable(true);

    let non_instance = stage
        .define_prim("/Root/NonInstance", "")
        .expect("define non-instance");
    non_instance.get_references().add_reference_with_path(
        ref_stage.get_root_layer().identifier(),
        &p("/Ref"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // IsInstance on instance should pass
    let instance_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_INSTANCE);
    let range1 = PrimRange::from_prim_with_predicate(&instance, instance_pred);
    let prims1: Vec<Prim> = range1.into_iter().collect();
    let paths1 = prim_paths(&prims1);
    assert!(
        paths1.contains(&"/Root/Instance".to_string()),
        "instance passes IsInstance"
    );

    // ~IsInstance on instance should be empty (root fails predicate)
    let not_instance_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_INSTANCE.not());
    let range2 = PrimRange::from_prim_with_predicate(&instance, not_instance_pred);
    let prims2: Vec<Prim> = range2.into_iter().collect();
    assert!(prims2.is_empty(), "instance fails ~IsInstance");

    // IsInstance on non_instance should be empty
    let range3 = PrimRange::from_prim_with_predicate(&non_instance, instance_pred);
    let prims3: Vec<Prim> = range3.into_iter().collect();
    assert!(prims3.is_empty(), "non-instance fails IsInstance");

    // ~IsInstance on non_instance should include it and descendants
    let range4 = PrimRange::from_prim_with_predicate(&non_instance, not_instance_pred);
    let prims4: Vec<Prim> = range4.into_iter().collect();
    let paths4 = prim_paths(&prims4);
    assert!(paths4.contains(&"/Root/NonInstance".to_string()));
}

// ============================================================================
// test_StageTraverse
// ============================================================================

#[test]
fn prim_range_stage_traverse() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let _foo = stage.define_prim("/Foo", "Mesh").expect("define /Foo");
    let _bar = stage.override_prim("/Bar").expect("override /Bar");
    let _faz = stage
        .define_prim("/Foo/Faz", "Mesh")
        .expect("define /Foo/Faz");
    let _baz = stage
        .define_prim("/Bar/Baz", "Mesh")
        .expect("define /Bar/Baz");

    // Default traverse should not hit undefined prims
    let default_traverse: Vec<Prim> = stage.traverse().into_iter().collect();
    let default_paths = prim_paths(&default_traverse);
    assert!(
        default_paths.contains(&"/Foo".to_string()),
        "traverse includes /Foo"
    );
    assert!(
        default_paths.contains(&"/Foo/Faz".to_string()),
        "traverse includes /Foo/Faz"
    );
    assert!(
        !default_paths.contains(&"/Bar".to_string()),
        "traverse excludes /Bar (override)"
    );

    // TraverseAll should include everything
    let all_traverse: Vec<Prim> = stage.traverse_all().into_iter().collect();
    let all_paths = prim_paths(&all_traverse);
    assert!(all_paths.contains(&"/Foo".to_string()));
    assert!(all_paths.contains(&"/Foo/Faz".to_string()));
    assert!(
        all_paths.contains(&"/Bar".to_string()),
        "traverse_all includes /Bar"
    );
    assert!(
        all_paths.contains(&"/Bar/Baz".to_string()),
        "traverse_all includes /Bar/Baz"
    );

    // Traverse with ~IsDefined — only undefined prims
    let not_defined_pred = PrimFlagsPredicate::from_term(USD_PRIM_IS_DEFINED.not());
    let undef_traverse: Vec<Prim> = stage
        .traverse_with_predicate(not_defined_pred)
        .into_iter()
        .collect();
    let undef_paths = prim_paths(&undef_traverse);
    assert!(
        undef_paths.contains(&"/Bar".to_string()),
        "~defined traverse includes /Bar"
    );
    assert!(
        undef_paths.contains(&"/Bar/Baz".to_string()),
        "~defined traverse includes /Bar/Baz"
    );
    assert!(
        !undef_paths.contains(&"/Foo".to_string()),
        "~defined traverse excludes /Foo"
    );
}

// ============================================================================
// test_WithInstancing — default traversal skips prototypes
// ============================================================================

#[test]
fn prim_range_instancing() {
    common::setup();

    let ref_stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("ref stage");
    ref_stage
        .define_prim("/Ref/Child", "")
        .expect("define /Ref/Child");

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("scene stage");
    let _root = stage.define_prim("/Root", "").expect("define /Root");

    let instance = stage
        .define_prim("/Root/Instance", "")
        .expect("define instance");
    instance.get_references().add_reference_with_path(
        ref_stage.get_root_layer().identifier(),
        &p("/Ref"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );
    instance.set_instanceable(true);

    let _non_instance = stage
        .define_prim("/Root/NonInstance", "")
        .expect("define non-instance");
    _non_instance.get_references().add_reference_with_path(
        ref_stage.get_root_layer().identifier(),
        &p("/Ref"),
        LayerOffset::default(),
        ListPosition::BackOfAppendList,
    );

    // Default stage traversal should include instance but not prototype subtrees
    let stage_prims: Vec<Prim> = stage.traverse().into_iter().collect();
    let stage_paths = prim_paths(&stage_prims);
    assert!(stage_paths.contains(&"/Root".to_string()));
    assert!(stage_paths.contains(&"/Root/Instance".to_string()));
    assert!(stage_paths.contains(&"/Root/NonInstance".to_string()));

    // If there are prototypes, we can traverse them explicitly
    let prototypes = stage.get_prototypes();
    if !prototypes.is_empty() {
        let proto = &prototypes[0];
        let proto_range = PrimRange::from_prim(proto);
        let proto_prims: Vec<Prim> = proto_range.into_iter().collect();
        assert!(
            !proto_prims.is_empty(),
            "prototype traversal should return prims"
        );
    }
}

// ============================================================================
// test_RoundTrip — PrimRange collect roundtrip
// ============================================================================

#[test]
fn prim_range_roundtrip() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    stage.define_prim("/foo", "").expect("define /foo");
    stage.define_prim("/bar", "").expect("define /bar");
    stage.define_prim("/baz", "").expect("define /baz");

    let pseudo_root = stage.pseudo_root();
    let range = PrimRange::from_prim(&pseudo_root);
    let prims: Vec<Prim> = range.into_iter().collect();
    // pseudo_root + 3 defined prims = 4
    assert_eq!(prims.len(), 4, "pseudo_root + 3 prims");

    // AllPrims should also work
    let all_range = PrimRange::all_prims(&pseudo_root);
    let all_prims: Vec<Prim> = all_range.into_iter().collect();
    assert!(all_prims.len() >= 4, "all_prims should have at least 4");
}

// ============================================================================
// test_EmptyRange
// ============================================================================

#[test]
fn prim_range_empty() {
    common::setup();

    let range = PrimRange::new();
    assert!(range.is_empty(), "default range should be empty");

    let prims: Vec<Prim> = range.into_iter().collect();
    assert!(prims.is_empty(), "empty range should yield no prims");
}

// ============================================================================
// test_AllPrims — includes all specifier types
// ============================================================================

#[test]
fn prim_range_all_prims() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let _def = stage.define_prim("/Defined", "").expect("define");
    let _over = stage.override_prim("/Override").expect("override");

    let pseudo_root = stage.pseudo_root();

    // AllPrims includes everything
    let all_range = PrimRange::all_prims(&pseudo_root);
    let prims: Vec<Prim> = all_range.into_iter().collect();
    let paths = prim_paths(&prims);
    assert!(
        paths.contains(&"/Defined".to_string()),
        "AllPrims includes defined"
    );
    assert!(
        paths.contains(&"/Override".to_string()),
        "AllPrims includes override"
    );
}

// ============================================================================
// test_PrimRange_Front_and_IncrementBegin
// ============================================================================

#[test]
fn prim_range_front_and_increment() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    stage.define_prim("/A", "").expect("define /A");
    stage.define_prim("/B", "").expect("define /B");
    stage.define_prim("/C", "").expect("define /C");

    let pseudo_root = stage.pseudo_root();
    let mut range = PrimRange::from_prim(&pseudo_root);

    // front() returns the first prim
    let front = range.front();
    assert!(front.is_some(), "front should return a prim");
    assert_eq!(
        front.unwrap().get_path().get_string(),
        "/",
        "front is pseudo_root"
    );

    // increment_begin() advances the start
    range.increment_begin();
    let front2 = range.front();
    assert!(front2.is_some());
    assert_eq!(
        front2.unwrap().get_path().get_string(),
        "/A",
        "after increment, front is /A"
    );
}
