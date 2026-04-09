//! `pxr.Sdr.shaderParserTestUtils` — test helpers (Rust, PyO3), no `.py` shim.

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Registers `pxr.Sdr.shaderParserTestUtils` with functions ported from OpenUSD tests.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "shaderParserTestUtils")?;
    // Populated in follow-up: TestBasicNode, TestShaderSpecificNode, TestShaderPropertiesNode
    // re-exported here so `from pxr.Sdr import shaderParserTestUtils` resolves to native code.
    parent.add_submodule(&m)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("pxr.Sdr.shaderParserTestUtils", &m)?;
    Ok(())
}
