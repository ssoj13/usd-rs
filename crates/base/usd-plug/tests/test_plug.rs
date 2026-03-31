//! Port of pxr/base/plug/testPlug.py → Rust integration tests.
//!
//! Tests plugin registration, type hierarchy, metadata access,
//! dependency loading, error handling, and notice callbacks.
//!
//! NOTE: PlugRegistry is a process-wide singleton. Tests share state.
//! Each test uses unique plugin names to avoid cross-test interference.
//! Tests that depend on specific registration state use dedicated fixtures.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use usd_plug::{PlugPlugin, PlugRegistry};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

// Helper: register fixtures and return the registry + new plugins
fn register_fixture(name: &str) -> (&'static PlugRegistry, Vec<Arc<PlugPlugin>>) {
    let dir = fixtures_dir().join(name);
    let registry = PlugRegistry::get_instance();
    let new_plugins = registry.register_plugins(&dir.to_string_lossy());
    (registry, new_plugins)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_registration — port of TestPlug.test_Registration
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_registration_dso_plugins() {
    let (registry, new_plugins) = register_fixture("dso_plugins");

    // Verify plugins were discovered
    if new_plugins.is_empty() {
        // Already registered by a prior test — just verify lookup works
        assert!(registry.get_plugin_with_name("TestPlugDso1").is_some());
        return;
    }

    let names: HashSet<String> = new_plugins
        .iter()
        .map(|p| p.get_name().to_string())
        .collect();

    // All 6 plugins from the combined plugInfo.json
    assert!(names.contains("TestPlugDso1"), "missing TestPlugDso1");
    assert!(names.contains("TestPlugDso2"), "missing TestPlugDso2");
    assert!(names.contains("TestPlugDso3"), "missing TestPlugDso3");
    assert!(names.contains("TestPlugModule1"), "missing TestPlugModule1");
    assert!(names.contains("TestPlugModule2"), "missing TestPlugModule2");
    assert!(names.contains("TestPlugModule3"), "missing TestPlugModule3");
}

#[test]
fn test_registration_type_hierarchy() {
    let (registry, _) = register_fixture("dso_plugins");

    // _TestPlugBase<1> should have derived types: TestPlugDerived1,
    // TestPlugModule1.TestPlugPythonDerived1
    let base1_derived = registry.get_all_derived_types("_TestPlugBase<1>");
    assert!(
        base1_derived.contains(&"TestPlugDerived1".to_string()),
        "TestPlugDerived1 must be derived from _TestPlugBase<1>"
    );
    assert!(
        base1_derived.contains(&"TestPlugModule1.TestPlugPythonDerived1".to_string()),
        "TestPlugModule1.TestPlugPythonDerived1 must be derived from _TestPlugBase<1>"
    );

    // _TestPlugBase<2> should have: TestPlugDerived2, TestPlugModule2.TestPlugPythonDerived2
    let base2_derived = registry.get_all_derived_types("_TestPlugBase<2>");
    assert!(
        base2_derived.contains(&"TestPlugDerived2".to_string()),
        "TestPlugDerived2 must be derived from _TestPlugBase<2>"
    );
    assert!(
        base2_derived.contains(&"TestPlugModule2.TestPlugPythonDerived2".to_string()),
        "TestPlugModule2.TestPlugPythonDerived2 must be derived from _TestPlugBase<2>"
    );
}

#[test]
fn test_registration_tftype_hierarchy() {
    let (_registry, _) = register_fixture("dso_plugins");

    // TfType must know about declared types and their bases
    let tf_derived1 = usd_tf::TfType::find_by_name("TestPlugDerived1");
    assert!(
        !tf_derived1.is_unknown(),
        "TestPlugDerived1 TfType must exist"
    );

    let tf_base1 = usd_tf::TfType::find_by_name("_TestPlugBase<1>");
    assert!(!tf_base1.is_unknown(), "_TestPlugBase<1> TfType must exist");
    assert!(
        tf_derived1.is_a(tf_base1),
        "TestPlugDerived1 must is_a _TestPlugBase<1>"
    );

    // TestPlugDerived3_3 -> _TestPlugBase<3>
    let tf_d33 = usd_tf::TfType::find_by_name("TestPlugDerived3_3");
    let tf_b3 = usd_tf::TfType::find_by_name("_TestPlugBase<3>");
    assert!(!tf_d33.is_unknown());
    assert!(!tf_b3.is_unknown());
    assert!(tf_d33.is_a(tf_b3));
}

#[test]
fn test_registration_all_plugins_count() {
    let (registry, _) = register_fixture("dso_plugins");

    let all = registry.get_all_plugins();
    // At minimum we have 6 from dso_plugins + possibly others from other tests
    assert!(
        all.len() >= 6,
        "expected at least 6 plugins, got {}",
        all.len()
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_declares_type — port of test_ManufacturingCppDerivedClasses type checks
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_declares_type_direct_and_subclass() {
    let (registry, _) = register_fixture("dso_plugins");

    let pd1 = registry
        .get_plugin_with_name("TestPlugDso1")
        .expect("TestPlugDso1 must be registered");

    // Direct declaration
    assert!(
        pd1.declares_type("TestPlugDerived1", false),
        "TestPlugDerived1 must be directly declared by TestPlugDso1"
    );

    // Base is NOT directly declared
    assert!(
        !pd1.declares_type("_TestPlugBase<1>", false),
        "_TestPlugBase<1> must NOT be directly declared"
    );

    // With subclass search: base IS found via TestPlugDerived1's bases
    assert!(
        pd1.declares_type("_TestPlugBase<1>", true),
        "_TestPlugBase<1> must be found with include_subclasses=true"
    );
}

#[test]
fn test_plugin_for_type_lookup() {
    let (registry, _) = register_fixture("dso_plugins");

    let pd1 = registry.get_plugin_for_type("TestPlugDerived1");
    assert!(
        pd1.is_some(),
        "get_plugin_for_type must find TestPlugDerived1"
    );
    assert_eq!(pd1.unwrap().get_name(), "TestPlugDso1");

    let pd2 = registry.get_plugin_for_type("TestPlugDerived2");
    assert!(pd2.is_some());
    assert_eq!(pd2.unwrap().get_name(), "TestPlugDso2");

    let ppd1 = registry.get_plugin_for_type("TestPlugModule1.TestPlugPythonDerived1");
    assert!(ppd1.is_some());
    assert_eq!(ppd1.unwrap().get_name(), "TestPlugModule1");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_metadata_access — port of TestPlug.test_MetadataAccess
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_metadata_access() {
    let (registry, _) = register_fixture("unloadable");

    let plugin = registry
        .get_plugin_with_name("TestPlugDsoUnloadable")
        .expect("TestPlugDsoUnloadable must be registered");

    let metadata = plugin.get_metadata();
    assert!(metadata.contains_key("Types"), "Info must have Types key");
    assert!(
        metadata
            .get("Types")
            .and_then(|v| v.as_object())
            .unwrap()
            .contains_key("TestPlugUnloadable"),
        "Types must contain TestPlugUnloadable"
    );

    // Per-type metadata
    let md = plugin
        .get_metadata_for_type("TestPlugUnloadable")
        .expect("type metadata must exist");

    // get_metadata_for_type must match the sub-dict directly
    let md_via_registry = registry
        .get_data_from_plugin_metadata("TestPlugUnloadable", "description")
        .and_then(|v| v.as_string().map(|s| s.to_string()));
    assert_eq!(md_via_registry.as_deref(), Some("unloadable plugin"));

    // "bases" = ["_TestPlugBase<1>"]
    let bases = md
        .get("bases")
        .and_then(|v| v.as_array())
        .expect("bases must be array");
    assert_eq!(bases.len(), 1);
    assert_eq!(bases[0].as_string(), Some("_TestPlugBase<1>"));

    // "description" = "unloadable plugin"
    assert_eq!(
        md.get("description").and_then(|v| v.as_string()),
        Some("unloadable plugin")
    );

    // "notLoadable" = true
    assert_eq!(md.get("notLoadable").and_then(|v| v.as_bool()), Some(true));

    // "vectorInt" = [1, 2, 3]
    let vec_int = md
        .get("vectorInt")
        .and_then(|v| v.as_array())
        .expect("vectorInt must be array");
    assert_eq!(vec_int.len(), 3);
    assert_eq!(vec_int[0].as_i64(), Some(1));
    assert_eq!(vec_int[1].as_i64(), Some(2));
    assert_eq!(vec_int[2].as_i64(), Some(3));

    // "vectorString" = ["f", "l", "o"]
    let vec_str = md
        .get("vectorString")
        .and_then(|v| v.as_array())
        .expect("vectorString must be array");
    assert_eq!(vec_str.len(), 3);
    assert_eq!(vec_str[0].as_string(), Some("f"));
    assert_eq!(vec_str[1].as_string(), Some("l"));
    assert_eq!(vec_str[2].as_string(), Some("o"));

    // "vectorDouble" = [1.1, 2.2, 3.3]
    let vec_dbl = md
        .get("vectorDouble")
        .and_then(|v| v.as_array())
        .expect("vectorDouble must be array");
    assert_eq!(vec_dbl.len(), 3);
    let d0 = vec_dbl[0].as_f64().expect("must be f64");
    let d1 = vec_dbl[1].as_f64().expect("must be f64");
    let d2 = vec_dbl[2].as_f64().expect("must be f64");
    assert!((d0 - 1.1).abs() < 1e-9);
    assert!((d1 - 2.2).abs() < 1e-9);
    assert!((d2 - 3.3).abs() < 1e-9);

    // "Int" = 4711
    let int_val = md.get("Int").expect("Int field must exist");
    assert_eq!(int_val.as_i64(), Some(4711));

    // "Double" = 0.815
    let dbl_val = md.get("Double").expect("Double field must exist");
    let dbl = dbl_val.as_f64().expect("Double must be f64");
    assert!((dbl - 0.815).abs() < 1e-6);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_loading_dependencies — port of TestPlug.test_LoadingPluginDependencies
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_loading_plugin_dependencies() {
    let (registry, _) = register_fixture("dependencies");

    // Verify dependency chain is registered
    let dep_derived = registry
        .get_plugin_with_name("TestDepDerived")
        .expect("TestDepDerived must be registered");
    let dep_module = registry
        .get_plugin_with_name("TestDepModule")
        .expect("TestDepModule must be registered");

    // Resource plugins are always loaded, so the dependency mechanism
    // is validated via the type hierarchy instead.
    assert!(dep_derived.is_loaded());
    assert!(dep_module.is_loaded());

    // Verify type hierarchy: DepDerived3 -> DepBase3 <- DepModule.DepPythonDerived3
    let base3_derived = registry.get_all_derived_types("DepBase3");
    assert!(base3_derived.contains(&"DepDerived3".to_string()));
    assert!(base3_derived.contains(&"DepModule.DepPythonDerived3".to_string()));

    // Verify dependency metadata is present
    let deps = dep_derived.get_dependencies();
    assert!(
        deps.contains_key("DepBase3"),
        "PluginDependencies must contain DepBase3"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_error_cases — port of TestPlug.test_ErrorCases
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_error_nonexistent_path() {
    let registry = PlugRegistry::get_instance();
    let result = registry.register_plugins("/nowhere/that/exists/at/all");
    assert!(result.is_empty(), "nonexistent path must yield no plugins");
}

#[test]
fn test_error_empty_plugin_info() {
    // plugInfo.json contains "2+2" (invalid JSON) — should not crash
    let (_, new_plugins) = register_fixture("empty_info");
    assert!(
        new_plugins.is_empty(),
        "invalid plugInfo.json must yield no plugins"
    );
}

#[test]
fn test_error_incomplete_plugin() {
    // Directory has no plugInfo.json — should not crash
    let (_, new_plugins) = register_fixture("incomplete");
    assert!(
        new_plugins.is_empty(),
        "directory without plugInfo.json must yield no plugins"
    );
}

#[test]
fn test_error_unknown_type() {
    let registry = PlugRegistry::get_instance();
    let result = registry.get_plugin_for_type("CompletelyBogusType99999");
    assert!(result.is_none(), "unknown type must return None");
}

#[test]
#[should_panic(expected = "No plugin found for type")]
fn test_error_demand_unknown_type() {
    let registry = PlugRegistry::get_instance();
    registry.demand_plugin_for_type("AbsolutelyNonExistentType12345");
}

#[test]
fn test_error_re_registration_is_noop() {
    let registry = PlugRegistry::get_instance();
    let dir = fixtures_dir().join("dso_plugins");

    // First registration
    let first = registry.register_plugins(&dir.to_string_lossy());

    // Second registration of the same path — must return empty (no-op)
    let second = registry.register_plugins(&dir.to_string_lossy());
    assert!(
        second.is_empty(),
        "re-registering the same path must be a no-op"
    );

    // All plugins from the first batch must still be retrievable
    if !first.is_empty() {
        let all = registry.get_all_plugins();
        for plugin in &first {
            assert!(
                all.iter().any(|p| p.get_name() == plugin.get_name()),
                "plugin '{}' must still be in registry after re-registration",
                plugin.get_name()
            );
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_dependency_error_cases — port of bad dep tests from testPlug.py
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_dep_bad_base_registered() {
    // Plugin with PluginDependencies referencing an unknown base type
    // must still register (metadata is valid), but load() would fail
    let (registry, _) = register_fixture("dep_bad_base");
    let plugin = registry.get_plugin_with_name("TestDepBadBase");
    assert!(
        plugin.is_some(),
        "plugin with bad base dep must still register"
    );

    let deps = plugin.unwrap().get_dependencies();
    assert!(deps.contains_key("UnknownBase"));
}

#[test]
fn test_dep_bad_dep_registered() {
    let (registry, _) = register_fixture("dep_bad_dep");
    let plugin = registry.get_plugin_with_name("TestDepBadDep");
    assert!(plugin.is_some());
}

#[test]
fn test_dep_bad_dep2_registered() {
    let (registry, _) = register_fixture("dep_bad_dep2");
    let plugin = registry.get_plugin_with_name("TestDepBadDep2");
    assert!(plugin.is_some());
}

#[test]
fn test_dep_bad_load_registered() {
    let (registry, _) = register_fixture("dep_bad_load");
    let plugin = registry.get_plugin_with_name("TestDepBadLoad");
    assert!(plugin.is_some());
}

#[test]
fn test_dep_cycle_registered() {
    let (registry, _) = register_fixture("dep_cycle");
    let plugin = registry.get_plugin_with_name("TestDepCycle");
    assert!(
        plugin.is_some(),
        "plugin with cycle dep must still register"
    );

    // Verify the cyclic dependency metadata is present
    let deps = plugin.unwrap().get_dependencies();
    assert!(deps.contains_key("_TestPlugBase<3>"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_notice_callbacks — port of NoticeListener from testPlug.py
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_notice_callback_fires() {
    let received = Arc::new(AtomicBool::new(false));
    let received_count = Arc::new(AtomicUsize::new(0));
    let plugin_names: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    {
        let received = received.clone();
        let received_count = received_count.clone();
        let plugin_names = plugin_names.clone();
        usd_plug::on_did_register_plugins(move |plugins| {
            for p in plugins {
                if p.get_name() == "TestNoticeCallbackPlugin" {
                    received.store(true, Ordering::SeqCst);
                }
                plugin_names.lock().unwrap().push(p.get_name().to_string());
            }
            received_count.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Create a temp plugin to trigger the notice
    let dir = std::env::temp_dir().join("usd_plug_test_notice_cb");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(
        dir.join("plugInfo.json"),
        r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "TestNoticeCallbackPlugin",
                "Info": {
                    "Types": {
                        "NoticeTestType": { "bases": [] }
                    }
                }
            }]
        }"#,
    )
    .expect("write plugInfo");

    let registry = PlugRegistry::get_instance();
    let new_plugins = registry.register_plugins(&dir.to_string_lossy());

    std::fs::remove_dir_all(&dir).ok();

    if !new_plugins.is_empty() {
        assert!(
            received.load(Ordering::SeqCst),
            "notice callback must fire for TestNoticeCallbackPlugin"
        );
        assert!(
            received_count.load(Ordering::SeqCst) >= 1,
            "notice must be received at least once"
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_alias_declaration — port of alias handling from TestPlugModule3
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_alias_declaration() {
    let (registry, _) = register_fixture("dso_plugins");

    // TestPlugModule3.TestPlugPythonDerived3_3 has alias: {"TfRefBase": "PluginDerived3_3"}
    let plugin = registry
        .get_plugin_with_name("TestPlugModule3")
        .expect("TestPlugModule3 must be registered");

    let md = plugin
        .get_metadata_for_type("TestPlugModule3.TestPlugPythonDerived3_3")
        .expect("type metadata must exist");

    let alias = md
        .get("alias")
        .and_then(|v| v.as_object())
        .expect("alias must be an object");
    assert_eq!(
        alias.get("TfRefBase").and_then(|v| v.as_string()),
        Some("PluginDerived3_3")
    );

    // TfType alias must be registered
    let tf_base = usd_tf::TfType::find_by_name("TfRefBase");
    if !tf_base.is_unknown() {
        let aliases = tf_base.get_aliases_for_derived(usd_tf::TfType::find_by_name(
            "TestPlugModule3.TestPlugPythonDerived3_3",
        ));
        assert!(
            aliases.contains(&"PluginDerived3_3".to_string()),
            "TfType alias 'PluginDerived3_3' must be registered"
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_resource_plugin_properties — verify resource plugin semantics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_resource_plugin_always_loaded() {
    let (registry, _) = register_fixture("dso_plugins");

    for plugin in registry.get_all_plugins() {
        if plugin.is_resource() {
            assert!(
                plugin.is_loaded(),
                "resource plugin '{}' must always report is_loaded=true",
                plugin.get_name()
            );
        }
    }
}

#[test]
fn test_resource_plugin_load_is_noop() {
    let (registry, _) = register_fixture("dso_plugins");

    let plugin = registry
        .get_plugin_with_name("TestPlugDso1")
        .expect("must exist");
    assert!(plugin.is_resource());
    assert!(plugin.is_loaded());

    // load() on resource plugin must succeed without error
    assert!(plugin.load().is_ok());
    assert!(plugin.is_loaded());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_get_string_from_plugin_metadata — shorthand accessor
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_get_string_from_plugin_metadata() {
    let (registry, _) = register_fixture("unloadable");

    let desc = registry.get_string_from_plugin_metadata("TestPlugUnloadable", "description");
    assert_eq!(desc.as_deref(), Some("unloadable plugin"));

    // Non-existent type
    let missing = registry.get_string_from_plugin_metadata("NoSuchType999", "description");
    assert!(missing.is_none());

    // Non-existent key
    let bad_key = registry.get_string_from_plugin_metadata("TestPlugUnloadable", "nonExistentKey");
    assert!(bad_key.is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_find_derived_type_by_name — hierarchical lookups
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_find_derived_type_by_name() {
    let (registry, _) = register_fixture("dso_plugins");

    // TestPlugDerived1 is a direct child of _TestPlugBase<1>
    let found = registry.find_derived_type_by_name("_TestPlugBase<1>", "TestPlugDerived1");
    assert_eq!(found, Some("TestPlugDerived1".to_string()));

    // _TestPlugBase<1> is NOT a child of itself
    let not_found = registry.find_derived_type_by_name("_TestPlugBase<1>", "_TestPlugBase<1>");
    assert!(not_found.is_none());
}

#[test]
fn test_get_directly_derived_types() {
    let (registry, _) = register_fixture("dso_plugins");

    let direct = registry.get_directly_derived_types("_TestPlugBase<3>");
    assert!(
        direct.contains(&"TestPlugDerived3_3".to_string()),
        "TestPlugDerived3_3 must be direct child of _TestPlugBase<3>"
    );
    assert!(
        direct.contains(&"TestPlugModule3.TestPlugPythonDerived3_3".to_string()),
        "TestPlugModule3.TestPlugPythonDerived3_3 must be direct child of _TestPlugBase<3>"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_declared_types — per-plugin type listing
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_get_declared_types() {
    let (registry, _) = register_fixture("dso_plugins");

    let dso3 = registry
        .get_plugin_with_name("TestPlugDso3")
        .expect("must exist");
    let types = dso3.get_declared_types();
    assert!(types.contains(&"TestPlugDerived3_3".to_string()));
    assert!(types.contains(&"TestPlugDerived3_4".to_string()));
    assert_eq!(types.len(), 2);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_find_plugin_resource — resource path construction
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_find_plugin_resource_nonexistent() {
    let (registry, _) = register_fixture("dso_plugins");

    let plugin = registry
        .get_plugin_with_name("TestPlugDso1")
        .expect("must exist");

    // Verify=true with a non-existent relative path returns empty
    let result = plugin.find_plugin_resource("does_not_exist.txt", true);
    assert!(
        result.is_empty(),
        "non-existent resource with verify=true must return empty"
    );

    // Verify=false returns the constructed path regardless
    let result = plugin.find_plugin_resource("some/file.txt", false);
    assert!(!result.is_empty());
}

#[test]
fn test_find_plugin_resource_free_function() {
    // Free function with None plugin returns empty
    let result = usd_plug::find_plugin_resource(None, "test.txt", false);
    assert!(result.is_empty());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_library_plugin_load_failure — non-existent DSO path
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_library_plugin_load_nonexistent() {
    use usd_js::JsObject;
    use usd_plug::plugin::{PlugPlugin, PluginType};

    let plugin = PlugPlugin::new(
        "/nonexistent/libfake.dll".to_string(),
        "fakeLibPluginTestPlug".to_string(),
        "".to_string(),
        JsObject::new(),
        PluginType::Library,
    );
    assert!(!plugin.is_loaded());

    let result = plugin.load();
    assert!(result.is_err(), "loading non-existent library must fail");
    assert!(!plugin.is_loaded());
}

#[test]
fn test_library_plugin_empty_path_static_link() {
    use usd_js::JsObject;
    use usd_plug::plugin::{PlugPlugin, PluginType};

    // Empty path = monolithic/static build — treated as success
    let plugin = PlugPlugin::new(
        "".to_string(),
        "staticLinkPluginTestPlug".to_string(),
        "".to_string(),
        JsObject::new(),
        PluginType::Library,
    );
    assert!(!plugin.is_loaded());

    let result = plugin.load();
    assert!(result.is_ok(), "empty path (static link) must succeed");
    assert!(plugin.is_loaded());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// test_cycle_detection — direct cycle detection in load_with_dependents
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_cycle_detection_in_load() {
    use std::collections::HashSet as StdHashSet;
    use usd_js::JsObject;
    use usd_plug::plugin::{PlugPlugin, PluginType};

    let plugin = PlugPlugin::new(
        "".to_string(),
        "cycleTestPlugTestPlug".to_string(),
        "".to_string(),
        JsObject::new(),
        PluginType::Library,
    );

    let mut seen = StdHashSet::new();
    seen.insert("cycleTestPlugTestPlug".to_string());

    let result = plugin.load_with_dependents(&mut seen);
    assert!(result.is_err());
    assert!(
        result.unwrap_err().contains("cyclic"),
        "error must mention cyclic dependency"
    );
}
