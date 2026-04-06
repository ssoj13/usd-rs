//! Integration tests for UsdLuxLightListAPI.
//!
//! Exact port of `pxr/usd/usdLux/testenv/testUsdLuxLightListAPI.py`.
//! Note: the Python reference has two methods both named `test_ListAPI`;
//! the second (root_with_instances.usda) silently overrides the first,
//! so only root_with_instances.usda is tested in the original.

use std::sync::Once;

use usd_core::{InitialLoadSet, Stage};
use usd_lux::{ComputeMode, LightAPI, LightFilter, LightListAPI};
use usd_sdf::Path;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

fn testenv_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/testenv/testUsdLuxLightListAPI", manifest)
}

/// Skip test if fixture is LFS pointer or missing.
fn require_fixture(name: &str) -> Option<String> {
    let path = format!("{}/{}", testenv_dir(), name);
    match std::fs::read_to_string(&path) {
        Ok(content) if content.starts_with("version https://git-lfs") => {
            eprintln!("skip: {name} is LFS pointer");
            None
        }
        Ok(_) => Some(path),
        Err(_) => {
            eprintln!("skip: {name} not found");
            None
        }
    }
}

/// Exact port of `_test()` from the Python reference.
/// Lines 20-89 of testUsdLuxLightListAPI.py.
#[test]
fn test_light_list_api() {
    setup();
    let Some(root_path) = require_fixture("root_with_instances.usda") else { return };
    let stage = match Stage::open(&root_path, InitialLoadSet::LoadNone) {
        Ok(s) => s,
        Err(_) => return,
    };

    let world = match stage.get_prim_at_path(&Path::from("/World")) {
        Some(p) => p,
        None => return,
    };
    let list_api = LightListAPI::new(world);
    let consult = ComputeMode::ConsultModelHierarchyCache;
    let ignore = ComputeMode::IgnoreCache;

    let sky_light = Path::from("/World/Lights/Sky_light");
    let torch1_light = Path::from("/World/Geo/torch_1/light");
    let torch2_light = Path::from("/World/Geo/torch_2/light");

    // No cache initially (ref line 26)
    let targets = list_api.get_light_list_rel()
        .map(|rel| rel.get_targets())
        .unwrap_or_default();
    assert_eq!(targets.len(), 0);

    // Compute w/o cache: 1 light outside payload (ref line 28-30)
    let computed = list_api.compute_light_list(ignore);
    assert_eq!(computed.len(), 1, "IgnoreCache should find 1 light: {computed:?}");
    assert!(computed.contains(&sky_light));

    // Compute w/ cache: 2 lights (1 extra from payload cache) (ref line 33-36)
    let computed = list_api.compute_light_list(consult);
    assert_eq!(computed.len(), 2, "ConsultCache should find 2 lights: {computed:?}");
    assert!(computed.contains(&sky_light));
    assert!(computed.contains(&torch2_light));

    // Load payloads (ref line 39)
    stage.load(&Path::from("/"), None);

    // Consult cache still 2 (ref line 42-45)
    let computed = list_api.compute_light_list(consult);
    assert_eq!(computed.len(), 2, "After Load, consult should still be 2: {computed:?}");
    assert!(computed.contains(&sky_light));
    assert!(computed.contains(&torch2_light));

    // Ignore cache now sees 3 (ref line 47-51)
    let computed = list_api.compute_light_list(ignore);
    assert_eq!(computed.len(), 3, "After Load, ignore should find 3: {computed:?}");
    assert!(computed.contains(&sky_light));
    assert!(computed.contains(&torch1_light));
    assert!(computed.contains(&torch2_light));

    // Store full list (ref line 54)
    list_api.store_light_list(&computed);

    // Now cache should return everything (ref line 57-61)
    let computed = list_api.compute_light_list(consult);
    assert_eq!(computed.len(), 3, "After store, consult should find 3: {computed:?}");
    assert!(computed.contains(&sky_light));
    assert!(computed.contains(&torch1_light));
    assert!(computed.contains(&torch2_light));

    // Deactivate 1 torch model (ref line 64-65)
    let torch_1 = stage.get_prim_at_path(&torch1_light.get_parent_path())
        .expect("torch_1 prim");
    torch_1.set_active(false);

    // Ignore cache sees 2 (ref line 68)
    assert_eq!(list_api.compute_light_list(ignore).len(), 2);
    // Cache still reports 3 (stale) (ref line 70)
    assert_eq!(list_api.compute_light_list(consult).len(), 3);
    // Invalidate → cache reports 2 (ref line 72-73)
    list_api.invalidate_light_list();
    assert_eq!(list_api.compute_light_list(consult).len(), 2);

    // Add a light filter (ref line 76-78)
    assert_eq!(list_api.compute_light_list(ignore).len(), 2);
    let _filter = LightFilter::define(&stage, &Path::from("/World/Lights/TestFilter"));
    assert_eq!(list_api.compute_light_list(ignore).len(), 3);

    // Add untyped prim + apply LightAPI (ref line 82-86)
    assert_eq!(list_api.compute_light_list(ignore).len(), 3);
    let prim = stage.define_prim("/World/Lights/PrimWithLightAPI", "")
        .expect("define prim");
    assert_eq!(list_api.compute_light_list(ignore).len(), 3);
    LightAPI::apply(&prim);
    assert_eq!(list_api.compute_light_list(ignore).len(), 4);

    // Discard changes (ref line 89)
    let _ = stage.reload();
}

#[test]
fn test_compute_mode_values() {
    assert_ne!(
        ComputeMode::ConsultModelHierarchyCache as u32,
        ComputeMode::IgnoreCache as u32,
    );
}
