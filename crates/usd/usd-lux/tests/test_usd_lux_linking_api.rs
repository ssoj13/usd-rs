//! Integration tests for UsdLux light linking via CollectionAPI.
//!
//! Port of `pxr/usd/usdLux/testenv/testUsdLuxLinkingAPI.py`

use std::sync::Once;

use usd_core::{InitialLoadSet, Stage};
use usd_lux::{LightAPI, LightFilter, SphereLight};
use usd_sdf::{Layer, Path};

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

/// Helper to get the testenv directory for LinkingAPI tests.
fn testenv_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/testenv/testUsdLuxLinkingAPI", manifest)
}

// =============================================================================
// test_LinkageQueries
// Matches Python: test_LinkageQueries
// =============================================================================

#[test]
fn test_linkage_queries() {
    setup();
    let test_file = format!("{}/linking_example.usda", testenv_dir());
    let layer = Layer::find_or_open(&test_file).expect("failed to open linking_example.usda layer");
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll)
        .expect("failed to create stage");

    // Matches Python test_LinkageQueries exactly
    let test_cases = vec![
        ("/Lights/DefaultLinkage/include_all", "/Geom", true),
        ("/Lights/DefaultLinkage/include_all", "/Geom/a", true),
        (
            "/Lights/DefaultLinkage/include_all",
            "/Geom/a/sub_scope",
            true,
        ),
        ("/Lights/SimpleInclude/include_a", "/Geom", false),
        ("/Lights/SimpleInclude/include_a", "/Geom/a", true),
        ("/Lights/SimpleInclude/include_a", "/Geom/a/sub_scope", true),
        ("/Lights/SimpleInclude/include_a", "/Geom/b", false),
        ("/Lights/SimpleExclude/exclude_a", "/Geom", true),
        ("/Lights/SimpleExclude/exclude_a", "/Geom/a", false),
        (
            "/Lights/SimpleExclude/exclude_a",
            "/Geom/a/sub_scope",
            false,
        ),
        ("/Lights/SimpleExclude/exclude_a", "/Geom/b", true),
        ("/Lights/FaceSetLinking/include_faceSet_example", "/", false),
        (
            "/Lights/FaceSetLinking/include_faceSet_example",
            "/Geom",
            false,
        ),
        (
            "/Lights/FaceSetLinking/include_faceSet_example",
            "/Geom/meshWithFaceSet",
            false,
        ),
        (
            "/Lights/FaceSetLinking/include_faceSet_example",
            "/Geom/meshWithFaceSet/faceSet",
            true,
        ),
    ];

    for (light_path, test_path, expected) in &test_cases {
        let prim = stage
            .get_prim_at_path(&Path::from(*light_path))
            .unwrap_or_else(|| panic!("no prim at {}", light_path));
        let light = LightAPI::new(prim);
        let links = light.get_light_link_collection_api();
        let query = links.compute_membership_query();
        let actual = query.is_path_included(&Path::from(*test_path), None);
        assert_eq!(
            actual, *expected,
            "light={}, path={}: expected {}, got {}",
            light_path, test_path, expected, actual
        );
    }
}

// =============================================================================
// test_LinkageAuthoring
// Matches Python: test_LinkageAuthoring
// =============================================================================

#[test]
fn test_linkage_authoring() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    let _geom_scope = stage
        .define_prim("/Geom", "Scope")
        .expect("failed to define Geom");
    let _sphere = stage
        .define_prim("/Geom/Sphere", "Sphere")
        .expect("failed to define Sphere");
    let _light_scope = stage
        .define_prim("/Lights", "Scope")
        .expect("failed to define Lights");
    let light_1 = SphereLight::define(&stage, &Path::from("/Lights/light_1"))
        .expect("failed to define light_1");
    let light_1_links = LightAPI::new(light_1.get_prim().clone()).get_light_link_collection_api();

    // Schema default: link everything (includeRoot=true)
    let query = light_1_links.compute_membership_query();
    assert!(query.is_path_included(&Path::from("/Geom"), None));
    assert!(query.is_path_included(&Path::from("/Geom/Sphere"), None));
    assert!(query.is_path_included(&Path::from("/RandomOtherPath"), None));

    // Exclude /Geom
    light_1_links.exclude_path(&Path::from("/Geom"));
    let query = light_1_links.compute_membership_query();
    assert!(!query.is_path_included(&Path::from("/Geom"), None));
    assert!(!query.is_path_included(&Path::from("/Geom/Sphere"), None));
    assert!(query.is_path_included(&Path::from("/RandomOtherPath"), None));

    // Include /Geom/Sphere
    light_1_links.include_path(&Path::from("/Geom/Sphere"));
    let query = light_1_links.compute_membership_query();
    assert!(!query.is_path_included(&Path::from("/Geom"), None));
    assert!(query.is_path_included(&Path::from("/Geom/Sphere"), None));
    assert!(query.is_path_included(&Path::from("/Geom/Sphere/Child"), None));
    assert!(query.is_path_included(&Path::from("/RandomOtherPath"), None));
}

// =============================================================================
// test_FilterLinking
// Matches Python: test_FilterLinking
// =============================================================================

#[test]
fn test_filter_linking() {
    setup();
    let test_file = format!("{}/linking_example.usda", testenv_dir());
    let layer = Layer::find_or_open(&test_file).expect("failed to open linking_example.usda layer");
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll)
        .expect("failed to create stage");

    let light_path = Path::from("/Lights/FilterLinking/filter_exclude_a");
    let light_prim = stage
        .get_prim_at_path(&light_path)
        .expect("failed to get filter_exclude_a");
    let light = LightAPI::new(light_prim);

    let filters_rel = light.get_filters_rel().expect("no filters relationship");
    let filter_paths = filters_rel.get_forwarded_targets();
    assert_eq!(filter_paths.len(), 1);

    let filter_prim = stage
        .get_prim_at_path(&filter_paths[0])
        .expect("failed to get filter prim");
    let light_filter = LightFilter::new(filter_prim);
    assert_eq!(
        light_filter.get_path().as_str(),
        "/Lights/FilterLinking/filter"
    );

    // Verify filter linking: excludes /Geom/a, includes /Geom/b
    let links = light_filter.get_filter_link_collection_api();
    let query = links.compute_membership_query();
    assert!(!query.is_path_included(&Path::from("/Geom/a"), None));
    assert!(query.is_path_included(&Path::from("/Geom/b"), None));
}

// =============================================================================
// test_LightFilterCollectionName
// =============================================================================

#[test]
fn test_light_filter_collection_name() {
    assert_eq!(LightFilter::FILTER_LINK_COLLECTION_NAME, "filterLink");
    assert_eq!(
        LightFilter::get_filter_link_collection_name().as_str(),
        "filterLink"
    );
}
