// Port of testUsdPrimCompositionQuery.py — core subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdPrimCompositionQuery.py

mod common;

use usd_core::Stage;
use usd_core::common::{InitialLoadSet, ListPosition};
use usd_core::prim_composition_query::{
    ArcTypeFilter, Filter, HasSpecsFilter, PrimCompositionQuery,
};
use usd_pcp::ArcType;
use usd_sdf::{LayerOffset, Path};

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "Xform").expect("define /Root");
    stage
        .define_prim("/Root/Child", "Mesh")
        .expect("define /Root/Child");
    stage
}

// ============================================================================
// Filter basics
// ============================================================================

#[test]
fn filter_arc_type_matching() {
    let filter = Filter::default();
    // Default filter should match all arc types
    assert!(filter.arc_type_filter.matches(ArcType::Root));
    assert!(filter.arc_type_filter.matches(ArcType::Reference));
    assert!(filter.arc_type_filter.matches(ArcType::Inherit));
    assert!(filter.arc_type_filter.matches(ArcType::Variant));
    assert!(filter.arc_type_filter.matches(ArcType::Specialize));
    assert!(filter.arc_type_filter.matches(ArcType::Payload));
}

// ============================================================================
// Query on simple stage (root arc only)
// ============================================================================

#[test]
fn query_simple_prim() {
    // A prim defined in-memory has only the root composition arc
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    // At minimum we expect a root arc
    assert!(!arcs.is_empty(), "should have at least root arc");

    // The first arc should be the root
    let root_arc = &arcs[0];
    assert_eq!(root_arc.arc_type(), ArcType::Root);
    assert!(!root_arc.is_implicit());
    assert!(!root_arc.is_ancestral());
    assert!(root_arc.has_specs());
    assert!(root_arc.is_introduced_in_root_layer_stack());
    assert!(root_arc.is_introduced_in_root_layer_prim_spec());
}

#[test]
fn query_with_references() {
    // Stage with a reference arc
    let stage = setup_stage();

    // Add a reference from /Ref to /Root
    let prim_ref = stage.define_prim("/Ref", "Xform").expect("define /Ref");
    let refs = prim_ref.get_references();
    refs.add_internal_reference(
        &Path::from_string("/Root").expect("p"),
        LayerOffset::default(),
        ListPosition::FrontOfPrependList,
    );

    let query = PrimCompositionQuery::get_direct_references(prim_ref);
    let arcs = query.get_composition_arcs();

    // Should have reference arcs
    let ref_arcs: Vec<_> = arcs
        .iter()
        .filter(|a| a.arc_type() == ArcType::Reference)
        .collect();
    assert!(!ref_arcs.is_empty(), "should have reference arcs");
}

#[test]
fn query_with_inherits() {
    let stage = setup_stage();

    // Create a class and inherit from it
    stage
        .create_class_prim("/_class_Base")
        .expect("define class");
    let prim = stage
        .define_prim("/Derived", "Xform")
        .expect("define /Derived");
    let inherits = prim.get_inherits();
    inherits.add_inherit(
        &Path::from_string("/_class_Base").expect("p"),
        ListPosition::FrontOfPrependList,
    );

    let query = PrimCompositionQuery::get_direct_inherits(prim);
    let arcs = query.get_composition_arcs();

    let inherit_arcs: Vec<_> = arcs
        .iter()
        .filter(|a| a.arc_type() == ArcType::Inherit)
        .collect();
    assert!(!inherit_arcs.is_empty(), "should have inherit arcs");
}

#[test]
fn query_filter_by_arc_type() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    // Filter for only reference arcs — a simple in-memory prim has none
    let mut filter = Filter::default();
    filter.arc_type_filter = ArcTypeFilter::Reference;
    let query = PrimCompositionQuery::new(prim.clone(), filter);
    let arcs = query.get_composition_arcs();

    let ref_arcs: Vec<_> = arcs
        .iter()
        .filter(|a| a.arc_type() == ArcType::Reference)
        .collect();
    // Should be empty — no references on this prim
    assert!(ref_arcs.is_empty());
}

#[test]
fn query_set_filter() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let mut query = PrimCompositionQuery::new(prim, Filter::default());
    let _original = query.get_filter();

    let mut new_filter = Filter::default();
    new_filter.has_specs_filter = HasSpecsFilter::HasSpecs;
    query.set_filter(new_filter);

    // Query should now use the new filter
    let arcs = query.get_composition_arcs();
    for arc in &arcs {
        assert!(arc.has_specs(), "filtered to HasSpecs only");
    }
}

// ============================================================================
// Arc accessors
// ============================================================================

#[test]
fn arc_target_prim_path() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    if !arcs.is_empty() {
        let root_arc = &arcs[0];
        let target_path = root_arc.target_prim_path();
        assert_eq!(target_path.get_string(), "/Root");
    }
}

#[test]
fn arc_introducing_prim_path() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    // Root arc introducing path is typically empty
    if !arcs.is_empty() {
        let _intro_path = arcs[0].introducing_prim_path();
        // Just verify it doesn't panic
    }
}
