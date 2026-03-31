//! Port of testUsdCrateRelocates.py from OpenUSD
//! Tests reading/writing relocates in crate (.usdc) files.

mod common;

#[test]
#[ignore = "Needs .usdc test files with relocates metadata"]
fn crate_relocates_read_write() {
    common::setup();
    // C++ opens crate files, verifies layer relocates field,
    // tests round-trip of relocates through usdc save/reload.
}
