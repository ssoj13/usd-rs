//! Python bindings for usd-rs — mirrors the `pxr` Python package from OpenUSD.
//!
//! All runtime API is exposed from the single native module `pxr._usd` (`.pyd` / `.so`);
//! the small `pxr/__init__.py` in the wheel only re-exports names from `pxr._usd`.
//!
//! Parity process and deviation register (repo root): `md/PYTHON_API_PARITY.md`,
//! `md/PYTHON_API_DEVIATIONS.md`, `md/PYTHON_API_WORK.md`. C++ reference tree:
//! `C:\projects\projects.rust.cg\usd-refs\OpenUSD` (see `STRUCTURE.md`).
//!
//! `pxr.Sdr.shaderParserTestUtils` is implemented in Rust (`sdr_shader_parser_test_utils.rs`), not as
//! an embedded upstream `.py` — see **G4 / §21** in `md/PYTHON_API_DEVIATIONS.md`.
//!
//! Module hierarchy matches C++ OpenUSD:
//!   pxr.Tf, pxr.Gf, pxr.Vt, pxr.Trace, pxr.Work, pxr.Sdf, pxr.Pcp, pxr.Ar, pxr.Usd,
//!   pxr.UsdGeom, pxr.UsdShade, pxr.UsdLux, pxr.UsdSkel, pxr.Sdr, pxr.UsdUtils, pxr.Cli, ...

// CamelCase method names are intentional — mirrors C++ OpenUSD Python API exactly.
#![allow(non_snake_case)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

mod ar;
mod constants_group;
mod cli;
mod geom;
mod gf;
mod kind;
mod lux;
mod pcp;
mod plug;
mod sdf;
mod sdr;
mod sdr_shader_parser_test_utils;
mod shade;
mod skel;
mod tf;
mod trace;
mod ts;
mod usd;
mod utils;
mod vt;
mod work;
mod xform_img_delegate;

use pyo3::prelude::*;

/// Root `pxr._usd` module — native extension backing the `pxr` package.
#[pymodule]
fn _usd(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Base modules
    register_sub(py, m, "Tf", tf::register)?;
    register_sub(py, m, "Gf", gf::register)?;
    register_sub(py, m, "Vt", vt::register)?;
    register_sub(py, m, "Trace", trace::register)?;
    register_sub(py, m, "Work", work::register)?;

    // Core USD modules
    register_sub(py, m, "Ar", ar::register)?;
    register_sub(py, m, "Plug", plug::register)?;
    register_sub(py, m, "Kind", kind::register)?;
    register_sub(py, m, "Sdf", sdf::register)?;
    register_sub(py, m, "Pcp", pcp::register)?;
    register_sub(py, m, "Ts", ts::register)?;
    register_sub(py, m, "Usd", usd::register)?;

    // Schema modules
    register_sub(py, m, "UsdGeom", geom::register)?;
    register_sub(py, m, "UsdShade", shade::register)?;
    register_sub(py, m, "UsdLux", lux::register)?;
    register_sub(py, m, "UsdSkel", skel::register)?;

    // Utilities / SDR (entire surface lives in this extension — no separate pxr/*.py logic)
    register_sub(py, m, "UsdUtils", utils::register)?;
    register_sub(py, m, "Sdr", sdr::register)?;

    // CLI tools as Python functions
    register_sub(py, m, "Cli", cli::register)?;

    Ok(())
}

fn register_sub(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    name: &str,
    f: fn(Python<'_>, &Bound<'_, PyModule>) -> PyResult<()>,
) -> PyResult<()> {
    let sub = PyModule::new(py, name)?;
    f(py, &sub)?;
    parent.add_submodule(&sub)?;
    // Register in sys.modules so `from pxr.Gf import *` works
    let full_name = format!("pxr.{name}");
    py.import("sys")?
        .getattr("modules")?
        .set_item(&full_name, &sub)?;
    Ok(())
}
