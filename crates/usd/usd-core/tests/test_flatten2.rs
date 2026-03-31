//! Port of testUsdFlatten2.py from OpenUSD
//! CLI tool test — opens a layer (with optional session), exports flattened result.

mod common;

#[test]
#[ignore = "CLI argparse tool — not a unit test, needs disk files"]
fn flatten2() {
    common::setup();
    // C++ is a CLI script that takes a layer path + optional session layer,
    // opens stage, prints ExportToString. Not a standard unit test.
}
