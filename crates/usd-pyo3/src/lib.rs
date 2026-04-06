//! Python bindings for usd-rs — mirrors the `pxr` Python package from OpenUSD.
//!
//! Module hierarchy matches C++ OpenUSD:
//!   pxr.Tf, pxr.Gf, pxr.Vt, pxr.Sdf, pxr.Pcp, pxr.Ar, pxr.Usd,
//!   pxr.UsdGeom, pxr.UsdShade, pxr.UsdLux, pxr.UsdSkel, ...

#![allow(clippy::useless_conversion)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

mod tf;
mod gf;
mod vt;
mod sdf;
mod pcp;
mod ar;
mod usd;
mod geom;
mod shade;
mod lux;
mod skel;
mod kind;
mod cli;

use pyo3::prelude::*;

/// Root `pxr._usd` module — native extension backing the `pxr` package.
#[pymodule]
fn _usd(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Base modules
    register_sub(py, m, "Tf", tf::register)?;
    register_sub(py, m, "Gf", gf::register)?;
    register_sub(py, m, "Vt", vt::register)?;

    // Core USD modules
    register_sub(py, m, "Ar", ar::register)?;
    register_sub(py, m, "Kind", kind::register)?;
    register_sub(py, m, "Sdf", sdf::register)?;
    register_sub(py, m, "Pcp", pcp::register)?;
    register_sub(py, m, "Usd", usd::register)?;

    // Schema modules
    register_sub(py, m, "UsdGeom", geom::register)?;
    register_sub(py, m, "UsdShade", shade::register)?;
    register_sub(py, m, "UsdLux", lux::register)?;
    register_sub(py, m, "UsdSkel", skel::register)?;

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
    Ok(())
}
