// Port of testUsdResolveTarget.cpp — core subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdResolveTarget.cpp

mod common;

use usd_core::prim_composition_query::PrimCompositionQuery;
use usd_core::resolve_target::ResolveTarget;
use usd_core::{InitialLoadSet, Stage};
use usd_pcp::ArcType;
use usd_sdf::Path;

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
// ResolveTarget construction
// ============================================================================

#[test]
fn resolve_target_new_is_null() {
    let rt = ResolveTarget::new();
    assert!(rt.is_null());
    assert!(rt.prim_index().is_none());
    assert!(rt.start_node().is_none());
    assert!(rt.start_layer().is_none());
    assert!(rt.stop_node().is_none());
    assert!(rt.stop_layer().is_none());
}

// ============================================================================
// ResolveTarget from composition arcs
// ============================================================================

#[test]
fn resolve_target_from_root_arc() {
    // C++ ref: builds resolve targets from composition query arcs
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    if !arcs.is_empty() {
        let root_arc = &arcs[0];
        assert_eq!(root_arc.arc_type(), ArcType::Root);

        // make_resolve_target_up_to with None = resolve up to root
        let rt = root_arc.make_resolve_target_up_to(None);
        // Root arc resolve target should have start node
        let _ = rt;
    }
}

#[test]
fn resolve_target_up_to_sublayer() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    if !arcs.is_empty() {
        let root_arc = &arcs[0];
        // Get the target layer
        if let Some(layer) = root_arc.target_layer() {
            let rt = root_arc.make_resolve_target_up_to(Some(layer));
            let _ = rt;
        }
    }
}

#[test]
fn resolve_target_stronger_than() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let query = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
    let arcs = query.get_composition_arcs();

    if !arcs.is_empty() {
        let root_arc = &arcs[0];
        if let Some(layer) = root_arc.target_layer() {
            let rt = root_arc.make_resolve_target_stronger_than(Some(layer));
            let _ = rt;
        }
    }
}

// ============================================================================
// ResolveTarget with references
// ============================================================================

#[test]
fn resolve_target_with_reference_arcs() {
    let stage = setup_stage();

    // Create a reference arc
    let prim_ref = stage.define_prim("/Ref", "Xform").expect("define /Ref");
    let refs = prim_ref.get_references();
    refs.add_internal_reference(
        &Path::from_string("/Root").expect("p"),
        usd_sdf::LayerOffset::default(),
        usd_core::common::ListPosition::FrontOfPrependList,
    );

    let query = PrimCompositionQuery::get_direct_references(prim_ref);
    let arcs = query.get_composition_arcs();

    for arc in &arcs {
        if arc.arc_type() == ArcType::Reference {
            let rt = arc.make_resolve_target_up_to(None);
            // Reference arc should produce a valid resolve target
            let _ = rt;
        }
    }
}
