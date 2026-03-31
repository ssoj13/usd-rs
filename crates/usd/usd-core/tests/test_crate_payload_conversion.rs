//! Port of testUsdCratePayloadConversionFromVersion07.py from OpenUSD
//! Tests crate file version 0.7 payload list conversion during read.

mod common;

#[test]
#[ignore = "Needs .usdc test files with version 0.7 payload format"]
fn crate_payload_conversion_from_v07() {
    common::setup();
    // C++ opens .usdc files from version 0.7 format, verifies payload lists
    // are correctly converted to the current internal representation.
}
