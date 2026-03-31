//! Port of testUsdSplines.py from OpenUSD
//! Tests Ts.Spline serialization, authoring, and value resolution.

mod common;

#[test]
#[ignore = "Needs Ts.Spline/Ts.Knot types + spline serialization"]
fn splines_basic() {
    common::setup();
    // C++ tests spline creation (Ts.Spline, Ts.Knot), serialization
    // to usda/usdc, round-trip verification, attribute value resolution
    // with spline-based time samples.
}
