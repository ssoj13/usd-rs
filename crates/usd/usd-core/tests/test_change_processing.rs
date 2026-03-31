//! Port of testUsdChangeProcessing.py from OpenUSD pxr/usd/usd/testenv/
//! 6 tests: RenamingSpec, ChangeInsignificantSublayer,
//! AddSublayerWithCycle, UnmuteWithCycle,
//! SublayerOperationProcessingApiSchema, SublayerOperationProcessingActive.

mod common;

use usd_core::common::InitialLoadSet;
use usd_core::edit_context::EditContext;
use usd_core::edit_target::EditTarget;
use usd_core::stage::Stage;
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}

// ============================================================================
// 1. RenamingSpec
// ============================================================================

#[test]
fn change_processing_renaming_spec() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let layer = stage.get_root_layer();

    stage.define_prim("/parent", "").unwrap();
    stage.define_prim("/parent/child", "").unwrap();

    assert!(stage.get_prim_at_path(&p("/parent")).is_some());
    assert!(stage.get_prim_at_path(&p("/parent/child")).is_some());

    // Rename /parent -> /parent_renamed at the Sdf layer level
    let mut prim_spec = layer.get_prim_at_path(&p("/parent")).expect("prim spec");
    assert!(prim_spec.set_name("parent_renamed", true));

    // Stage should reflect the rename
    assert!(stage.get_prim_at_path(&p("/parent_renamed")).is_some());
    assert!(
        stage
            .get_prim_at_path(&p("/parent_renamed/child"))
            .is_some()
    );
}

// ============================================================================
// 2. ChangeInsignificantSublayer
// ============================================================================

#[test]
fn change_processing_insignificant_sublayer() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let layer = stage.get_root_layer();

    let insignificant = usd_sdf::Layer::create_anonymous(Some(".usda"));
    assert!(insignificant.is_empty());

    // Add as sublayer
    let mut paths = layer.sublayer_paths();
    paths.push(insignificant.identifier().to_string());
    layer.set_sublayer_paths(&paths);

    // Edit in the insignificant sublayer
    let edit_target = EditTarget::for_local_layer(insignificant.clone());
    let _ctx = EditContext::new_with_target(stage.clone(), edit_target);

    let prim = stage.define_prim("/Foo", "").unwrap();
    assert!(prim.is_valid());
}

// ============================================================================
// 3. AddSublayerWithCycle
// ============================================================================

#[test]
#[ignore = "Needs CreateNew (disk files) + Pcp::ErrorSublayerCycle detection"]
fn change_processing_add_sublayer_with_cycle() {
    common::setup();
    // C++ creates 3 files on disk (root.usda, a.usda, b.usda) and creates
    // a sublayer cycle: root->b->a->b. Verifies GetCompositionErrors returns
    // Pcp.ErrorSublayerCycle for each stage.
}

// ============================================================================
// 4. UnmuteWithCycle
// ============================================================================

#[test]
#[ignore = "Needs CreateNew (disk) + MuteLayer/UnmuteLayer + Pcp::ErrorSublayerCycle"]
fn change_processing_unmute_with_cycle() {
    common::setup();
    // C++ creates files, mutes layer b, builds cycle while muted,
    // then unmutes and verifies the cycle error appears.
}

// ============================================================================
// 5. SublayerOperationProcessingApiSchema
// ============================================================================

#[test]
#[ignore = "Needs CreateNew (disk) + HasAPI + ColorSpaceAPI.Apply"]
fn change_processing_sublayer_api_schema() {
    common::setup();
    // C++ creates two stages on disk, applies ColorSpaceAPI to sublayer,
    // adds as sublayer, verifies HasAPI propagates.
}

// ============================================================================
// 6. SublayerOperationProcessingActive
// ============================================================================

#[test]
#[ignore = "Needs CreateNew (disk) + SetActive propagation across sublayers"]
fn change_processing_sublayer_active() {
    common::setup();
    // C++ creates two stages on disk, sets prim inactive in sublayer,
    // adds as sublayer, verifies IsActive() is false.
}
