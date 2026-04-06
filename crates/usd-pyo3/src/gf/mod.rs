//! pxr.Gf — Graphics Foundation Python bindings.
//!
//! Complete drop-in replacement for pxr.Gf from C++ OpenUSD.
//! Covers all 51 types from the API inventory.

pub mod geo;
pub mod matrix;
pub mod quat;
pub mod vec;

use pyo3::prelude::*;

/// Register all Gf types into the pxr.Gf submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    vec::register(py, m)?;
    matrix::register(py, m)?;
    quat::register(py, m)?;
    geo::register(py, m)?;
    Ok(())
}
