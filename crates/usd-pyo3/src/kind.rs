//! pxr.Kind — Model Kind Registry Python bindings.
//!
//! Drop-in replacement for `pxr.Kind` from C++ OpenUSD.
//! Covers: KindRegistry (singleton accessor), KindTokens.

use pyo3::prelude::*;

use usd_kind::registry;
use usd_kind::tokens::KindTokens;
use usd_tf::Token;

// ============================================================================
// KindTokens
// ============================================================================

/// Static tokens for built-in USD model kinds.
///
/// Mirrors `pxr.Kind.Tokens` / `KindTokens`.
#[pyclass(name = "Tokens", module = "pxr_rs.Kind")]
pub struct PyKindTokens;

#[pymethods]
impl PyKindTokens {
    #[classattr]
    fn model() -> &'static str {
        KindTokens::get_instance().model.as_str()
    }

    #[classattr]
    fn component() -> &'static str {
        KindTokens::get_instance().component.as_str()
    }

    #[classattr]
    fn group() -> &'static str {
        KindTokens::get_instance().group.as_str()
    }

    #[classattr]
    fn assembly() -> &'static str {
        KindTokens::get_instance().assembly.as_str()
    }

    #[classattr]
    fn subcomponent() -> &'static str {
        KindTokens::get_instance().subcomponent.as_str()
    }

    fn __repr__(&self) -> &str {
        "Kind.Tokens"
    }
}

// ============================================================================
// KindRegistry
// ============================================================================

/// Singleton registry for model kind information.
///
/// Access via `KindRegistry.GetInstance()` or the module-level convenience
/// functions `HasKind`, `GetBaseKind`, `IsA`, etc.
///
/// Mirrors `pxr.Kind.Registry` / `KindRegistry`.
#[pyclass(name = "Registry", module = "pxr_rs.Kind")]
pub struct PyKindRegistry;

#[pymethods]
impl PyKindRegistry {
    /// Return the singleton KindRegistry instance.
    #[classmethod]
    #[pyo3(name = "GetInstance")]
    fn get_instance(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self
    }

    /// Return true if `kind` is known to the registry.
    ///
    /// ```python
    /// Kind.Registry.HasKind("model")  # True
    /// ```
    #[pyo3(name = "HasKind")]
    fn has_kind(&self, kind: &str) -> bool {
        registry::has_kind(&Token::new(kind))
    }

    /// Return the base kind of `kind`, or empty string if it has none.
    ///
    /// ```python
    /// Kind.Registry.GetBaseKind("assembly")  # "group"
    /// Kind.Registry.GetBaseKind("model")     # ""
    /// ```
    #[pyo3(name = "GetBaseKind")]
    fn get_base_kind(&self, kind: &str) -> String {
        registry::get_base_kind(&Token::new(kind)).as_str().to_string()
    }

    /// Return an unordered list of all registered kind strings.
    ///
    /// ```python
    /// kinds = Kind.Registry.GetAllKinds()
    /// ```
    #[pyo3(name = "GetAllKinds")]
    fn get_all_kinds(&self) -> Vec<String> {
        registry::get_all_kinds()
            .into_iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    /// True if `derived_kind` is the same as, or derives from, `base_kind`.
    ///
    /// ```python
    /// Kind.Registry.IsA("assembly", "model")  # True
    /// Kind.Registry.IsA("component", "group") # False
    /// ```
    #[pyo3(name = "IsA")]
    fn is_a(&self, derived_kind: &str, base_kind: &str) -> bool {
        registry::is_a(&Token::new(derived_kind), &Token::new(base_kind))
    }

    /// True if `kind` is or derives from "model".
    #[pyo3(name = "IsModel")]
    fn is_model(&self, kind: &str) -> bool {
        registry::is_model_kind(&Token::new(kind))
    }

    /// True if `kind` is or derives from "group".
    #[pyo3(name = "IsGroup")]
    fn is_group(&self, kind: &str) -> bool {
        registry::is_group_kind(&Token::new(kind))
    }

    /// True if `kind` is or derives from "assembly".
    #[pyo3(name = "IsAssembly")]
    fn is_assembly(&self, kind: &str) -> bool {
        registry::is_assembly_kind(&Token::new(kind))
    }

    /// True if `kind` is or derives from "component".
    #[pyo3(name = "IsComponent")]
    fn is_component(&self, kind: &str) -> bool {
        registry::is_component_kind(&Token::new(kind))
    }

    /// True if `kind` is or derives from "subcomponent".
    #[pyo3(name = "IsSubComponent")]
    fn is_sub_component(&self, kind: &str) -> bool {
        registry::is_subcomponent_kind(&Token::new(kind))
    }

    fn __repr__(&self) -> &str {
        "Kind.Registry"
    }
}

// ============================================================================
// Module-level convenience functions
// ============================================================================

/// True if `kind` is known to the registry.
///
/// Mirrors `pxr.Kind.Registry.HasKind(kind)` at module level.
#[pyfunction]
#[pyo3(name = "HasKind")]
fn has_kind(kind: &str) -> bool {
    registry::has_kind(&Token::new(kind))
}

/// Return the base kind of `kind`, or empty string if none.
///
/// Mirrors `pxr.Kind.Registry.GetBaseKind(kind)` at module level.
#[pyfunction]
#[pyo3(name = "GetBaseKind")]
fn get_base_kind(kind: &str) -> String {
    registry::get_base_kind(&Token::new(kind)).as_str().to_string()
}

/// Return all registered kind strings.
///
/// Mirrors `pxr.Kind.Registry.GetAllKinds()` at module level.
#[pyfunction]
#[pyo3(name = "GetAllKinds")]
fn get_all_kinds() -> Vec<String> {
    registry::get_all_kinds()
        .into_iter()
        .map(|t| t.as_str().to_string())
        .collect()
}

/// True if `derived_kind` IsA `base_kind` (same or derives from).
///
/// Mirrors `pxr.Kind.Registry.IsA()` at module level.
#[pyfunction]
#[pyo3(name = "IsA")]
fn is_a(derived_kind: &str, base_kind: &str) -> bool {
    registry::is_a(&Token::new(derived_kind), &Token::new(base_kind))
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Kind classes and free functions into the `pxr.Kind` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;

    // Classes
    m.add_class::<PyKindTokens>()?;
    m.add_class::<PyKindRegistry>()?;

    // Free convenience functions
    m.add_function(wrap_pyfunction!(has_kind, m)?)?;
    m.add_function(wrap_pyfunction!(get_base_kind, m)?)?;
    m.add_function(wrap_pyfunction!(get_all_kinds, m)?)?;
    m.add_function(wrap_pyfunction!(is_a, m)?)?;

    // Module-level token constants (matches pxr.Kind.Tokens.model, etc.)
    let tokens = KindTokens::get_instance();
    m.add("model", tokens.model.as_str())?;
    m.add("component", tokens.component.as_str())?;
    m.add("group", tokens.group.as_str())?;
    m.add("assembly", tokens.assembly.as_str())?;
    m.add("subcomponent", tokens.subcomponent.as_str())?;

    Ok(())
}
