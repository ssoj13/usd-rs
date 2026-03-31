//! Port of testUsdFlattenLayerStack.py from OpenUSD
//! Tests UsdUtilsFlattenLayerStack with various composition scenarios.

mod common;

#[test]
#[ignore = "Needs root.usda + sublayers + Usd.FlattenLayerStack API"]
fn flatten_layer_stack_basic() {
    common::setup();
    // C++ opens root.usda with sublayers, calls Usd.FlattenLayerStack,
    // verifies composition arcs, metadata, opinions all preserved correctly.
}
