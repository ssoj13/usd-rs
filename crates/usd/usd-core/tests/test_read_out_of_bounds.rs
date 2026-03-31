//! Port of testUsdReadOutOfBounds.py from OpenUSD pxr/usd/usd/testenv/
//! 1 test: test_ReadOutOfBounds — opening a corrupt .usd file should error, not crash.

mod common;

#[test]
#[ignore = "Needs corrupt.usd test asset"]
fn read_out_of_bounds() {
    common::setup();
    // C++ opens "corrupt.usd", expects TfErrorException (not crash/UB).
}
