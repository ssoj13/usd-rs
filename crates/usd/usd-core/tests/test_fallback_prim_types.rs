//! Port of testUsdFallbackPrimTypes.py from OpenUSD
//! Tests fallback prim type metadata and change processing.

mod common;

#[test]
#[ignore = "Needs test plugin + Tf.Notice + fallback prim type metadata"]
fn fallback_prim_types() {
    common::setup();
    // C++ registers test plugin, tests fallback prim type names in layer
    // metadata, verifies change processing when metadata changes,
    // and correct prim type resolution with/without fallbacks.
}
