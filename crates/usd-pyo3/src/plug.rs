//! pxr.Plug — Plugin Registry Python bindings.
//!
//! Drop-in replacement for `pxr.Plug` from C++ OpenUSD.
//! Covers: PlugRegistry (singleton accessor).

use pyo3::prelude::*;

use usd_plug::PlugRegistry;
use usd_tf;

use crate::tf::PyType;

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
#[pyclass(skip_from_py_object, name = "Registry", module = "pxr.Plug")]
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
        reg.get_plugin_with_name(name)
            .map(|p| p.get_name().to_string())
    }

    /// Resolve a registered plugin implementation type by name (see `plugInfo.json` / TfType map).
    ///
    /// Returns [`Tf.Type`] registered for `name`, or **unknown** if not found (truth-tested in tests).
    #[pyo3(name = "FindTypeByName")]
    fn find_type_by_name(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyType>>> {
        let reg = PlugRegistry::get_instance();
        if reg.find_type_by_name(name).is_none() {
            return Ok(None);
        }
        Ok(Some(Py::new(
            py,
            PyType {
                inner: usd_tf::TfType::find_by_name(name),
            },
        )?))
    }

    fn __repr__(&self) -> &str {
        "Plug.Registry"
    }
}

// ============================================================================
// _TestPlugBase — test infrastructure for plugin cross-language inheritance
// ============================================================================

/// Base class for Plug test modules (mirrors C++ `_TestPlugBase<N>`).
/// Python test modules derive from these to test plugin discovery.
macro_rules! test_plug_base {
    ($name:ident, $py_name:literal) => {
        #[pyclass(subclass, name = $py_name, module = "pxr.Plug")]
        pub struct $name;

        #[pymethods]
        impl $name {
            #[new]
            fn new() -> Self {
                Self
            }

            #[pyo3(name = "GetTypeName")]
            fn get_type_name(&self) -> String {
                $py_name.to_owned()
            }
        }
    };
}

test_plug_base!(PyTestPlugBase1, "_TestPlugBase1");
test_plug_base!(PyTestPlugBase2, "_TestPlugBase2");
test_plug_base!(PyTestPlugBase3, "_TestPlugBase3");
test_plug_base!(PyTestPlugBase4, "_TestPlugBase4");

// ============================================================================
// Plugin — wraps a discovered plugin handle
// ============================================================================

/// Wraps a single discovered plugin (mirrors `pxr.Plug.Plugin`).
#[pyclass(name = "Plugin", module = "pxr.Plug")]
pub struct PyPlugin {
    name: String,
}

#[pymethods]
impl PyPlugin {
    #[pyo3(name = "GetName")]
    fn get_name(&self) -> &str {
        &self.name
    }

    fn __repr__(&self) -> String {
        format!("Plug.Plugin('{}')", self.name)
    }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Plug classes into the `pxr.Plug` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;
    m.add_class::<PyPlugRegistry>()?;
    m.add_class::<PyTestPlugBase1>()?;
    m.add_class::<PyTestPlugBase2>()?;
    m.add_class::<PyTestPlugBase3>()?;
    m.add_class::<PyTestPlugBase4>()?;
    m.add_class::<PyPlugin>()?;
    Ok(())
}
