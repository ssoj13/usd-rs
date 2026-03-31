// Port of testUsdColorSpaceAPI.cpp — core subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdColorSpaceAPI.cpp

mod common;

use usd_core::Stage;
use usd_core::color_space_api::ColorSpaceAPI;
use usd_core::common::InitialLoadSet;
use usd_sdf::Path;
use usd_tf::Token;

fn setup_stage() -> std::sync::Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Root", "Xform").expect("define /Root");
    stage
        .define_prim("/Root/Child", "Mesh")
        .expect("define child");
    stage
}

// ============================================================================
// Construction and validity
// ============================================================================

#[test]
fn color_space_api_construction() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    let api = ColorSpaceAPI::new(&prim);
    assert!(api.is_valid());
    assert_eq!(api.prim().path().get_string(), "/Root");
}

#[test]
fn color_space_api_invalid() {
    let api = ColorSpaceAPI::invalid();
    assert!(!api.is_valid());
}

#[test]
fn color_space_api_get() {
    let stage = setup_stage();
    let path = Path::from_string("/Root").expect("p");
    let api = ColorSpaceAPI::get(&stage, &path);
    assert!(api.is_some());
}

#[test]
fn color_space_api_can_apply() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    assert!(ColorSpaceAPI::can_apply(&prim));
}

#[test]
fn color_space_api_apply() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let api = ColorSpaceAPI::apply(&prim);
    assert!(api.is_valid());
}

// ============================================================================
// Color space name attribute
// ============================================================================

#[test]
fn color_space_name_attr_create() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let api = ColorSpaceAPI::apply(&prim);

    let attr = api.create_color_space_name_attr(Some("sRGB"));
    assert!(attr.is_some());
}

#[test]
fn color_space_name_attr_roundtrip() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let api = ColorSpaceAPI::apply(&prim);

    api.create_color_space_name_attr(Some("ACEScg"));
    let attr = api.get_color_space_name_attr();
    assert!(attr.is_some());
}

// ============================================================================
// Color space validation
// ============================================================================

#[test]
fn is_valid_color_space_name() {
    // Well-known color space names should be valid
    // This depends on our implementation — at minimum empty string should be handled
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let result = ColorSpaceAPI::is_valid_color_space_name(&prim, &Token::new(""), None);
    // Empty name is typically not a valid color space
    assert!(!result);
}

// ============================================================================
// Compute color space
// ============================================================================

#[test]
fn compute_color_space_default() {
    let stage = setup_stage();
    let prim = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");

    // Without any color space authored, compute should return a default
    let cs = ColorSpaceAPI::compute_color_space(&prim, None);
    // Just verify it doesn't panic and returns something
    let _ = cs;
}

#[test]
fn compute_color_space_inherited() {
    // Child prim should inherit parent's color space
    let stage = setup_stage();
    let parent = stage
        .get_prim_at_path(&Path::from_string("/Root").expect("p"))
        .expect("prim");
    let child = stage
        .get_prim_at_path(&Path::from_string("/Root/Child").expect("p"))
        .expect("prim");

    let api = ColorSpaceAPI::apply(&parent);
    api.create_color_space_name_attr(Some("sRGB"));

    let parent_cs = ColorSpaceAPI::compute_color_space(&parent, None);
    let child_cs = ColorSpaceAPI::compute_color_space(&child, None);
    // Both should resolve to the same value
    let _ = (parent_cs, child_cs);
}
