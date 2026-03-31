//! Port of testUsdAppliedAPISchemas.py from OpenUSD pxr/usd/usd/testenv/
//! ~40 tests covering single/multi apply API schemas, nesting, auto-apply.
//! Requires plugin registration (resources/).

mod common;

#[test]
#[ignore = "Needs test plugin registration (testUsdAppliedAPISchemas resources/)"]
fn applied_api_schemas_placeholder() {
    common::setup();
    // C++ registers test plugin with SingleApplyAPI, MultiApplyAPI,
    // NestedInnerSingleApplyAPI, etc. Tests Apply(), CanApply(), HasAPI(),
    // GetAppliedSchemas(), built-in API schemas, auto-apply, etc.
    // 203KB test file — requires full schema infrastructure.
}
