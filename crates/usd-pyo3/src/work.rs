//! pxr.Work — Threading / concurrency Python bindings.
//!
//! Drop-in replacement for `pxr.Work` from C++ OpenUSD.
//! Covers: SetMaximumConcurrencyLimit, SetConcurrencyLimit,
//! GetConcurrencyLimit, GetPhysicalConcurrencyLimit.

use pyo3::prelude::*;

/// Set thread pool to maximum hardware concurrency.
///
/// Mirrors `pxr.Work.SetMaximumConcurrencyLimit()`.
#[pyfunction]
#[pyo3(name = "SetMaximumConcurrencyLimit")]
fn set_maximum_concurrency_limit() {
    usd_work::set_maximum_concurrency_limit();
}

/// Set the concurrency limit to the given value.
///
/// Mirrors `pxr.Work.SetConcurrencyLimit(n)`.
#[pyfunction]
#[pyo3(name = "SetConcurrencyLimit")]
fn set_concurrency_limit(n: usize) {
    usd_work::set_concurrency_limit(n);
}

/// Return the current concurrency limit.
///
/// Mirrors `pxr.Work.GetConcurrencyLimit()`.
#[pyfunction]
#[pyo3(name = "GetConcurrencyLimit")]
fn get_concurrency_limit() -> usize {
    usd_work::get_concurrency_limit()
}

/// Return the physical concurrency limit (number of hardware threads).
///
/// Mirrors `pxr.Work.GetPhysicalConcurrencyLimit()`.
#[pyfunction]
#[pyo3(name = "GetPhysicalConcurrencyLimit")]
fn get_physical_concurrency_limit() -> usize {
    usd_work::get_physical_concurrency_limit()
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Work functions into the `pxr.Work` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;
    m.add_function(wrap_pyfunction!(set_maximum_concurrency_limit, m)?)?;
    m.add_function(wrap_pyfunction!(set_concurrency_limit, m)?)?;
    m.add_function(wrap_pyfunction!(get_concurrency_limit, m)?)?;
    m.add_function(wrap_pyfunction!(get_physical_concurrency_limit, m)?)?;
    Ok(())
}
