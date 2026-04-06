//! Stub — will be filled by agent

use pyo3::prelude::*;

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = m;
    Ok(())
}
