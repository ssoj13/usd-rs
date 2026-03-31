//! Port of testUsdLoadUnloadDeepNestedInstancing.py from OpenUSD
//! 2 tests for deeply nested instancing load/unload race condition.

mod common;

#[test]
#[ignore = "Needs Mountain.usd test asset with 60 prototypes"]
fn load_unload_deep_nested_prototype_is_descendant() {
    common::setup();
    // C++ opens Mountain.usd (60 prototypes), unloads all, reloads parent,
    // then LoadAndUnload specific paths. Tests no crash from threading race.
}

#[test]
#[ignore = "Needs Earth.usd test asset + _GetSourcePrimIndex internal API"]
fn load_unload_deep_nested_prototype_path() {
    common::setup();
    // C++ opens Earth.usd (61 prototypes), finds nested instance source path,
    // unloads it, verifies prototype count changes correctly.
}
