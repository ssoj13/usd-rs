//! Port of testUsdExternalAssetDependencies.py from OpenUSD
//! Tests external asset dependency tracking with procedural file format plugin.

mod common;

#[test]
#[ignore = "Needs procedural file format plugin registration + disk files"]
fn external_asset_dependencies() {
    common::setup();
    // C++ registers TestUsdProceduralExternalAssetsFileFormatPlugin,
    // opens procedural test file, verifies GetExternalAssetDependencies,
    // tests stage reload with external deps.
}
