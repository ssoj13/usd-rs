//! Port of testUsdAttributeAssetPathVariableExpressions.py from OpenUSD
//! 2 tests: test_Get, test_GetArray — expression variable resolution in asset paths.

mod common;

#[test]
#[ignore = "Needs expressionVariables support in layer + AssetPath evaluation"]
fn asset_path_variable_expressions_get() {
    common::setup();
    // C++ creates in-memory stage with expressionVariables metadata,
    // sets asset attr with variable expressions like @`"./${NAME}.usda"`@,
    // verifies evaluatedPath resolves correctly.
}

#[test]
#[ignore = "Needs expressionVariables + asset[] array evaluation"]
fn asset_path_variable_expressions_get_array() {
    common::setup();
    // Same as above but with asset[] arrays and time samples.
}
