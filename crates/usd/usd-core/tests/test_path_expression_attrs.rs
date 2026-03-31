//! Port of testUsdPathExpressionAttrs.py from OpenUSD
//! 3 tests: BasicResolution, BasicPathTranslation, BasicPathTranslation2.

mod common;

#[test]
#[ignore = "Needs Sdf.PathExpression type + composition of expression attrs"]
fn path_expression_attrs_basic_resolution() {
    common::setup();
    // C++ creates superlayer + sublayer with PathExpression attrs,
    // verifies composition produces correct composed expression.
}

#[test]
#[ignore = "Needs PathExpression + reference path translation"]
fn path_expression_attrs_basic_path_translation() {
    common::setup();
    // C++ creates reference with PathExpression attr, verifies path
    // translation across the reference arc.
}

#[test]
#[ignore = "Needs PathExpression + instance/prototype path mapping"]
fn path_expression_attrs_path_translation2() {
    common::setup();
    // C++ tests `//` leading expressions translate across references
    // and prototype-to-instance path mapping.
}
