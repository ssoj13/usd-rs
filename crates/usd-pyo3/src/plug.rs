//! pxr.Plug — Plugin Registry Python bindings.
//!
//! Drop-in replacement for `pxr.Plug` from C++ OpenUSD.
//! Covers: PlugRegistry (singleton accessor).

use pyo3::prelude::*;

use usd_plug::PlugRegistry;

// ============================================================================
// Registry (PlugRegistry)
// ============================================================================

/// Singleton registry for plugins discovered via `plugInfo.json`.
///
/// Mirrors `pxr.Plug.Registry` / `PlugRegistry`.
///
/// ```python
/// reg = Plug.Registry()
/// reg.RegisterPlugins("/path/to/plugins")
/// ```
#[pyclass(skip_from_py_object, name = "Registry", module = "pxr_rs.Plug")]
pub struct PyPlugRegistry;

#[pymethods]
impl PyPlugRegistry {
    /// Create a handle to the singleton PlugRegistry.
    #[new]
    fn new() -> Self {
        Self
    }

    /// Register all plugins discovered at the given path(s).
    ///
    /// Accepts a single path string or a list of path strings.
    /// Returns a list of newly registered plugin names.
    #[pyo3(name = "RegisterPlugins")]
    fn register_plugins(&self, path: &str) -> Vec<String> {
        let reg = PlugRegistry::get_instance();
        reg.register_plugins(path)
            .into_iter()
            .map(|p| p.get_name().to_string())
            .collect()
    }

    /// Return all registered plugin names.
    #[pyo3(name = "GetAllPlugins")]
    fn get_all_plugins(&self) -> Vec<String> {
        let reg = PlugRegistry::get_instance();
        reg.get_all_plugins()
            .into_iter()
            .map(|p| p.get_name().to_string())
            .collect()
    }

    /// Return a plugin by name, or None if not found.
    #[pyo3(name = "GetPluginWithName")]
    fn get_plugin_with_name(&self, name: &str) -> Option<String> {
        let reg = PlugRegistry::get_instance();
        reg.get_plugin_with_name(name).map(|p| p.get_name().to_string())
    }

    fn __repr__(&self) -> &str {
        "Plug.Registry"
    }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Plug classes into the `pxr.Plug` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;
    m.add_class::<PyPlugRegistry>()?;
    Ok(())
}
