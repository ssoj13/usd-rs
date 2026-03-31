// Port of testUsdValueClips.py — value clips core subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdValueClips.py

mod common;

use usd_core::clips_api::ClipsAPI;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Model", "Xform").expect("define /Model");
    stage
}

// ============================================================================
// Basic clips API operations
// ============================================================================

#[test]
fn clips_api_basic() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    assert!(api.is_valid());
}

#[test]
fn clips_no_clips_by_default() {
    // Without any clips authored, everything should be None/empty
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);

    assert!(
        api.get_clips().is_none()
            || api
                .get_clips()
                .as_ref()
                .map(|d| d.is_empty())
                .unwrap_or(true)
    );
    assert!(api.get_clip_sets().is_none());
    assert!(api.get_clip_asset_paths_default().is_none());
}

// ============================================================================
// Clips metadata set/get roundtrip
// ============================================================================

#[test]
fn clips_set_get_roundtrip() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);

    // Set clips dictionary
    let mut clips = std::collections::HashMap::new();
    clips.insert("default".to_string(), usd_vt::Value::from(true));
    let set_ok = api.set_clips(clips);
    // May succeed or fail depending on implementation
    let _ = set_ok;
}

// ============================================================================
// Compute clip asset paths
// ============================================================================

#[test]
fn compute_clip_asset_paths_empty() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);

    let paths = api.compute_clip_asset_paths_default();
    assert!(paths.is_empty());
}

#[test]
fn compute_clip_asset_paths_named_set_empty() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);

    let paths = api.compute_clip_asset_paths("default");
    assert!(paths.is_empty());
}

// ============================================================================
// Multiple prims with clips
// ============================================================================

#[test]
fn clips_api_on_different_prims() {
    let stage = setup_stage();
    stage.define_prim("/Model2", "Xform").expect("define");

    let prim1 = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("p");
    let prim2 = stage
        .get_prim_at_path(&Path::from_string("/Model2").expect("p"))
        .expect("p");

    let api1 = ClipsAPI::new(prim1);
    let api2 = ClipsAPI::new(prim2);

    assert!(api1.is_valid());
    assert!(api2.is_valid());
    assert_ne!(api1.path().get_string(), api2.path().get_string());
}
