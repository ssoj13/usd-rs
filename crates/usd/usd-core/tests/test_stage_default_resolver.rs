//! Port of testUsdStageDefaultResolver.py from OpenUSD
//! Tests stage reload with ArDefaultResolver search path changes.

mod common;

#[test]
#[ignore = "Needs Ar.DefaultResolver.SetDefaultSearchPath + CreateNew (disk files)"]
fn stage_default_resolver_reload() {
    common::setup();
    // C++ creates test files in dirA/ and dirB/, sets up references,
    // changes ArDefaultResolver search path, verifies stage picks up
    // the correct file through resolver changed notice.
}
