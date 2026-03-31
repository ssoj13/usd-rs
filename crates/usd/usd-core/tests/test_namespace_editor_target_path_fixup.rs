//! Port of testUsdNamespaceEditorTargetPathFixup.py from OpenUSD
//! Tests that namespace edits correctly fix up connection/target paths.

mod common;

#[test]
#[ignore = "Needs full NamespaceEditor + connection/target path fixup (123KB test)"]
fn namespace_editor_target_path_fixup() {
    common::setup();
    // C++ tests that when prims/properties are renamed/moved via NamespaceEditor,
    // all connectionPaths and target fields in the layer are correctly updated.
}
