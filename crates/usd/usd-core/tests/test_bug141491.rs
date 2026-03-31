//! Port of testUsdBug141491.py from OpenUSD pxr/usd/usd/testenv/
//! 1 test: TestBug141491 — deactivate ancestor of instance, then edit;
//! regression test for crash at stage teardown.

mod common;

#[test]
#[ignore = "Needs root.usda test asset + instancing (deactivate ancestor + edit)"]
fn bug_141491() {
    common::setup();
    // C++ opens root.usda, verifies instance prim, deactivates ancestor,
    // adds inherit arc to referenced prim. Prior to fix this would crash
    // at stage teardown due to orphaned prototype prim.
}
