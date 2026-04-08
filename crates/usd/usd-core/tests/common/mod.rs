//! Shared test helpers for usd-core test suite.
//! Ported from OpenUSD pxr/usd/usd/testenv/ test infrastructure.

use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize SDF file format plugins. Must be called once before any test.
pub fn setup() {
    INIT.call_once(|| {
        usd_sdf::init();
    });
}

/// Create a temporary file path with a unique name.
#[allow(dead_code)]
pub fn tmp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "usd_test_{}_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("t"),
        name
    ))
}

/// Path under `pxr/usd/usd/testenv/<subpath>` (requires `OPENUSD_SRC_ROOT`).
#[allow(dead_code)]
pub fn testenv_path(subpath: &str) -> PathBuf {
    openusd_test_path::pxr_usd_module_testenv("usd", subpath)
}

/// Float comparison epsilon.
pub const EPSILON: f64 = 1e-5;

/// Assert two f64 values are within EPSILON.
#[allow(dead_code)]
pub fn assert_near(label: &str, a: f64, b: f64) {
    assert!(
        (a - b).abs() < EPSILON,
        "{label}: expected {a} ~= {b} (diff = {})",
        (a - b).abs()
    );
}

/// Helper: create ValueTypeName from SDF type string.
#[allow(dead_code)]
pub fn vtn(type_str: &str) -> usd_sdf::value_type_name::ValueTypeName {
    usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type(type_str)
}
