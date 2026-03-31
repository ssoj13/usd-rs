// Port of testUsdClipsAPI — value clips API subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdClipsAPI

mod common;

use usd_core::clips_api::{ClipsAPI, ClipsAPIInfoKeys, ClipsAPISetNames};
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Model", "Xform").expect("define /Model");
    stage
}

// ============================================================================
// ClipsAPIInfoKeys token values
// ============================================================================

#[test]
fn clips_api_info_keys() {
    // Verify well-known token names
    assert_eq!(ClipsAPIInfoKeys::active().as_str(), "active");
    assert_eq!(ClipsAPIInfoKeys::asset_paths().as_str(), "assetPaths");
    assert_eq!(
        ClipsAPIInfoKeys::manifest_asset_path().as_str(),
        "manifestAssetPath"
    );
    assert_eq!(ClipsAPIInfoKeys::prim_path().as_str(), "primPath");
    assert_eq!(ClipsAPIInfoKeys::times().as_str(), "times");
    assert_eq!(
        ClipsAPIInfoKeys::template_asset_path().as_str(),
        "templateAssetPath"
    );
    assert_eq!(
        ClipsAPIInfoKeys::template_start_time().as_str(),
        "templateStartTime"
    );
    assert_eq!(
        ClipsAPIInfoKeys::template_end_time().as_str(),
        "templateEndTime"
    );
    assert_eq!(
        ClipsAPIInfoKeys::template_stride().as_str(),
        "templateStride"
    );
    assert_eq!(
        ClipsAPIInfoKeys::template_active_offset().as_str(),
        "templateActiveOffset"
    );
    assert_eq!(
        ClipsAPIInfoKeys::interpolate_missing_clip_values().as_str(),
        "interpolateMissingClipValues"
    );
}

#[test]
fn clips_sets_name_default() {
    assert_eq!(ClipsAPISetNames::default_().as_str(), "default");
}

// ============================================================================
// ClipsAPI construction
// ============================================================================

#[test]
fn clips_api_new() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    assert!(api.is_valid());
}

#[test]
fn clips_api_invalid() {
    let api = ClipsAPI::invalid();
    assert!(!api.is_valid());
}

#[test]
fn clips_api_get_from_prim() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::get_from_prim(&prim);
    assert!(api.is_valid());
    assert_eq!(api.path().get_string(), "/Model");
}

#[test]
fn clips_api_get() {
    let stage = setup_stage();
    let path = Path::from_string("/Model").expect("p");
    let api = ClipsAPI::get(&stage, &path);
    assert!(api.is_valid());
}

#[test]
fn clips_api_schema_type_name() {
    let name = ClipsAPI::schema_type_name();
    assert!(!name.is_empty());
}

#[test]
fn clips_api_schema_attribute_names() {
    let names = ClipsAPI::get_schema_attribute_names(false);
    // ClipsAPI doesn't define schema attributes, so this may be empty
    let _ = names;
}

// ============================================================================
// Clips metadata
// ============================================================================

#[test]
fn clips_api_get_clips_empty() {
    // Without authoring, clips metadata should be None
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    let clips = api.get_clips();
    // No clips authored
    assert!(clips.is_none() || clips.as_ref().map(|d| d.is_empty()).unwrap_or(true));
}

#[test]
fn clips_api_get_clip_sets_empty() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    let sets = api.get_clip_sets();
    // No clip sets authored
    assert!(sets.is_none());
}

#[test]
fn clips_api_get_clip_asset_paths_empty() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    let paths = api.get_clip_asset_paths_default();
    assert!(paths.is_none());
}

#[test]
fn clips_api_compute_clip_asset_paths_empty() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Model").expect("p"))
        .expect("prim");
    let api = ClipsAPI::new(prim);
    let paths = api.compute_clip_asset_paths_default();
    assert!(paths.is_empty());
}
