//! Resolve paths into an OpenUSD source tree for integration tests.
//!
//! Set **`OPENUSD_SRC_ROOT`** to the root of a clone of [OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD)
//! (the directory that contains the `pxr` folder). Test assets are read from
//! `pxr/.../testenv/...` only — they are not bundled in `usd-rs`.
//!
//! # Examples
//!
//! ```ignore
//! use openusd_test_path::pxr_usd_module_testenv;
//! let p = pxr_usd_module_testenv("usd", "testUsdPrims.testenv/test.usda");
//! ```

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Root of the OpenUSD checkout (contains `pxr/`).
///
/// # Panics
///
/// Panics if `OPENUSD_SRC_ROOT` is unset or empty. Tests should set the variable before running.
pub fn require_openusd_src_root() -> PathBuf {
    let Some(root) = std::env::var_os("OPENUSD_SRC_ROOT") else {
        panic!(
            "OPENUSD_SRC_ROOT must be set to the root of an OpenUSD source tree (directory containing `pxr`). \
             Example (PowerShell): $env:OPENUSD_SRC_ROOT = 'C:\\path\\to\\OpenUSD'"
        );
    };
    let p = PathBuf::from(root);
    if !p.is_dir() {
        panic!("OPENUSD_SRC_ROOT is not a directory: {}", p.display());
    }
    p
}

/// Append path segments under `pxr/`.
pub fn pxr_path(segments: &[&str]) -> PathBuf {
    let mut p = require_openusd_src_root();
    p.push("pxr");
    for s in segments {
        p.push(s);
    }
    p
}

/// Path under `pxr/usd/<module>/testenv/<relative>`.
///
/// `module` is the OpenUSD package directory name, e.g. `usd`, `pcp`, `sdf`, `usdGeom`, `usdLux`, `usdShade`.
pub fn pxr_usd_module_testenv(module: &str, relative: impl AsRef<Path>) -> PathBuf {
    let mut p = pxr_path(&["usd", module, "testenv"]);
    p.push(relative);
    p
}

/// `pxr/usdImaging/testenv/<relative>` (imaging tests live outside `pxr/usd/`).
pub fn pxr_usd_imaging_testenv(relative: impl AsRef<Path>) -> PathBuf {
    let mut p = pxr_path(&["usdImaging", "testenv"]);
    p.push(relative);
    p
}

/// PCP Museum fixtures: `pxr/usd/pcp/testenv/testPcpMuseum_<scenario>.testenv/<scenario>/<file>`.
///
/// Upstream OpenUSD does **not** use a single `museum/` directory; each scenario is a
/// `testPcpMuseum_<ScenarioName>.testenv` folder (see `testPcpCompositionResults.py`).
pub fn pxr_pcp_museum(scenario: &str, file: impl AsRef<Path>) -> PathBuf {
    let mut p = pxr_path(&[
        "usd",
        "pcp",
        "testenv",
        &format!("testPcpMuseum_{scenario}.testenv"),
        scenario,
    ]);
    p.push(file);
    p
}

/// Baseline next to Museum assets: `.../testPcpMuseum_<scenario>.testenv/baseline/compositionResults_<scenario>.txt`.
pub fn pxr_pcp_museum_baseline_composition_results(scenario: &str) -> PathBuf {
    let mut p = pxr_path(&[
        "usd",
        "pcp",
        "testenv",
        &format!("testPcpMuseum_{scenario}.testenv"),
        "baseline",
    ]);
    p.push(format!("compositionResults_{scenario}.txt"));
    p
}
