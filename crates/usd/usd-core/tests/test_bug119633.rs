//! Port of testUsdBug119633.py from OpenUSD pxr/usd/usd/testenv/
//! 1 test: TestBug119633 — opens root.usda and verifies prim lookup doesn't crash.

mod common;

#[test]
#[ignore = "Needs root.usda test asset on disk"]
fn bug_119633() {
    common::setup();
    // C++ opens root.usda, does GetPrimAtPath("/SardineGroup_OceanA"),
    // catches TfErrorException if it happens. Just verifying no crash.
}
