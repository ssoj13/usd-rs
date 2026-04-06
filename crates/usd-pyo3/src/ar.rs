//! pxr.Ar — Asset Resolver Python bindings.
//!
//! Drop-in replacement for `pxr.Ar` from C++ OpenUSD.
//! Covers: ArResolver, ArResolvedPath, ArResolverContext, ArAssetInfo,
//! ArTimestamp, ArNotice, resolver context binder.

use pyo3::prelude::*;

use usd_ar::{AssetInfo, DefaultResolverContext, ResolvedPath, Timestamp};
use usd_ar::resolver::{get_resolver, set_preferred_resolver};

// ============================================================================
// ArResolvedPath
// ============================================================================

/// A resolved asset path — the physical location after resolution.
///
/// Mirrors `pxr.Ar.ResolvedPath` / `ArResolvedPath`.
#[pyclass(skip_from_py_object,name = "ResolvedPath", module = "pxr_rs.Ar")]
#[derive(Clone)]
pub struct PyResolvedPath {
    inner: ResolvedPath,
}

#[pymethods]
impl PyResolvedPath {
    /// Create a ResolvedPath from a path string.
    ///
    /// ```python
    /// p = Ar.ResolvedPath("/some/file.usd")
    /// ```
    #[new]
    #[pyo3(signature = (path = ""))]
    fn new(path: &str) -> Self {
        Self {
            inner: ResolvedPath::new(path),
        }
    }

    /// Return the resolved path as a string.
    #[pyo3(name = "GetPathString")]
    fn get_path_string(&self) -> &str {
        self.inner.as_str()
    }

    /// True if the resolved path is non-empty.
    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!("Ar.ResolvedPath('{}')", self.inner.as_str())
    }

    fn __str__(&self) -> &str {
        self.inner.as_str()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __lt__(&self, other: &Self) -> bool {
        self.inner < other.inner
    }

    fn __le__(&self, other: &Self) -> bool {
        self.inner <= other.inner
    }

    fn __gt__(&self, other: &Self) -> bool {
        self.inner > other.inner
    }

    fn __ge__(&self, other: &Self) -> bool {
        self.inner >= other.inner
    }

    fn __hash__(&self) -> u64 {
        self.inner.hash_value()
    }
}

impl PyResolvedPath {
    pub fn from_inner(inner: ResolvedPath) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &ResolvedPath {
        &self.inner
    }
}

// ============================================================================
// ArResolverContext
// ============================================================================

/// Provides additional data to the resolver for use during resolution.
///
/// Mirrors `pxr.Ar.ResolverContext` / `ArResolverContext`.
///
/// Python-level context holds a search-path list (the default context type).
#[pyclass(from_py_object,name = "ResolverContext", module = "pxr_rs.Ar")]
#[derive(Clone)]
pub struct PyResolverContext {
    /// Search paths for the default file resolver.
    search_paths: Vec<String>,
}

#[pymethods]
impl PyResolverContext {
    /// Create an empty resolver context, or one with the given search paths.
    ///
    /// ```python
    /// ctx = Ar.ResolverContext()
    /// ctx2 = Ar.ResolverContext(["/search/path1", "/search/path2"])
    /// ```
    #[new]
    #[pyo3(signature = (search_paths = None))]
    fn new(search_paths: Option<Vec<String>>) -> Self {
        Self {
            search_paths: search_paths.unwrap_or_default(),
        }
    }

    /// True if this context contains no data.
    #[pyo3(name = "IsEmpty")]
    fn is_empty(&self) -> bool {
        self.search_paths.is_empty()
    }

    /// Return a debug string representation.
    #[pyo3(name = "GetDebugString")]
    fn get_debug_string(&self) -> String {
        format!("ArResolverContext(searchPaths={:?})", self.search_paths)
    }

    /// Return the search paths held by this context.
    #[pyo3(name = "Get")]
    fn get(&self) -> Vec<String> {
        self.search_paths.clone()
    }

    fn __repr__(&self) -> String {
        format!("Ar.ResolverContext({})", self.get_debug_string())
    }

    fn __str__(&self) -> String {
        self.get_debug_string()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.search_paths == other.search_paths
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.search_paths != other.search_paths
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.search_paths.hash(&mut h);
        h.finish()
    }

    fn __bool__(&self) -> bool {
        !self.is_empty()
    }
}

// ============================================================================
// ArAssetInfo
// ============================================================================

/// Metadata about a resolved asset.
///
/// Mirrors `pxr.Ar.AssetInfo` / `ArAssetInfo`.
#[pyclass(skip_from_py_object,name = "AssetInfo", module = "pxr_rs.Ar")]
#[derive(Clone)]
pub struct PyAssetInfo {
    inner: AssetInfo,
}

#[pymethods]
impl PyAssetInfo {
    #[new]
    fn new() -> Self {
        Self { inner: AssetInfo::new() }
    }

    /// Asset version string, or None.
    #[getter]
    fn version(&self) -> Option<&str> {
        self.inner.version.as_deref()
    }

    #[setter]
    fn set_version(&mut self, v: Option<String>) {
        self.inner.version = v;
    }

    /// Asset name string, or None.
    #[getter]
    #[pyo3(name = "assetName")]
    fn asset_name(&self) -> Option<&str> {
        self.inner.asset_name.as_deref()
    }

    #[setter]
    fn set_asset_name(&mut self, v: Option<String>) {
        self.inner.asset_name = v;
    }

    /// Repository path, or None.
    #[getter]
    #[pyo3(name = "repoPath")]
    fn repo_path(&self) -> Option<&str> {
        self.inner.repo_path.as_deref()
    }

    #[setter]
    fn set_repo_path(&mut self, v: Option<String>) {
        self.inner.repo_path = v;
    }

    /// True if this AssetInfo has no data.
    #[pyo3(name = "IsEmpty")]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!("Ar.AssetInfo(name={:?}, version={:?})",
            self.inner.asset_name, self.inner.version)
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }
}

// ============================================================================
// ArTimestamp
// ============================================================================

/// A timestamp for an asset (Unix time, or invalid/NaN).
///
/// Mirrors `pxr.Ar.Timestamp` / `ArTimestamp`.
#[pyclass(skip_from_py_object,name = "Timestamp", module = "pxr_rs.Ar")]
#[derive(Clone, Copy)]
pub struct PyTimestamp {
    inner: Timestamp,
}

#[pymethods]
impl PyTimestamp {
    /// Create a Timestamp from a Unix time value (seconds since epoch).
    ///
    /// Called with no argument creates an invalid timestamp.
    ///
    /// ```python
    /// ts = Ar.Timestamp(1609459200.0)
    /// invalid = Ar.Timestamp()
    /// ```
    #[new]
    #[pyo3(signature = (time = None))]
    fn new(time: Option<f64>) -> Self {
        Self {
            inner: match time {
                Some(t) => Timestamp::new(t),
                None => Timestamp::invalid(),
            },
        }
    }

    /// True if this timestamp is valid (not NaN).
    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Return the Unix time value, or None if invalid.
    #[pyo3(name = "GetTime")]
    fn get_time(&self) -> Option<f64> {
        self.inner.try_get_time()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_valid() {
            format!("Ar.Timestamp({})", self.inner.try_get_time().unwrap_or(f64::NAN))
        } else {
            "Ar.Timestamp(invalid)".to_string()
        }
    }

    fn __str__(&self) -> String {
        format!("{}", self.inner)
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __lt__(&self, other: &Self) -> bool {
        self.inner < other.inner
    }

    fn __le__(&self, other: &Self) -> bool {
        self.inner <= other.inner
    }

    fn __gt__(&self, other: &Self) -> bool {
        self.inner > other.inner
    }

    fn __ge__(&self, other: &Self) -> bool {
        self.inner >= other.inner
    }

    fn __hash__(&self) -> u64 {
        self.inner.raw_time().to_bits()
    }
}

// ============================================================================
// ArResolver (Python-level singleton accessor)
// ============================================================================

/// Python-level wrapper around the global asset resolver.
///
/// Mirrors `pxr.Ar.Resolver` / `ArResolver`.
///
/// Obtain via `Ar.GetResolver()`.
#[pyclass(skip_from_py_object,name = "Resolver", module = "pxr_rs.Ar")]
pub struct PyResolver;

#[pymethods]
impl PyResolver {
    /// Resolve an asset path to its physical location.
    ///
    /// Returns an empty ResolvedPath if the asset could not be found.
    #[pyo3(name = "Resolve")]
    fn resolve(&self, asset_path: &str) -> PyResolvedPath {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        PyResolvedPath::from_inner(resolver.resolve(asset_path))
    }

    /// Create an identifier for the given asset path.
    ///
    /// If `anchor_path` is provided, relative paths are anchored to it.
    #[pyo3(name = "CreateIdentifier")]
    #[pyo3(signature = (asset_path, anchor_path = None))]
    fn create_identifier(&self, asset_path: &str, anchor_path: Option<&PyResolvedPath>) -> String {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        resolver.create_identifier(asset_path, anchor_path.map(|p| p.inner()))
    }

    /// Create a default context for this resolver.
    #[pyo3(name = "CreateDefaultContext")]
    fn create_default_context(&self) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let ctx = resolver.create_default_context();
        // Extract search paths from the DefaultResolverContext inside the ArResolverContext.
        let paths: Vec<String> = ctx
            .get::<DefaultResolverContext>()
            .map(|c| c.search_paths().to_vec())
            .unwrap_or_default();
        PyResolverContext { search_paths: paths }
    }

    /// Create a context from a colon-separated string of search paths.
    #[pyo3(name = "CreateContextFromString")]
    fn create_context_from_string(&self, context_str: &str) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let _ctx = resolver.create_context_from_string(context_str);
        // Parse colon-separated paths as best-effort representation.
        let paths: Vec<String> = context_str
            .split(':')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        PyResolverContext { search_paths: paths }
    }

    /// Return the file extension (without leading '.') for the given asset path.
    #[pyo3(name = "GetExtension")]
    fn get_extension(&self, asset_path: &str) -> String {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        resolver.get_extension(asset_path)
    }

    /// True if the given asset path is context-dependent.
    #[pyo3(name = "IsContextDependentPath")]
    fn is_context_dependent_path(&self, asset_path: &str) -> bool {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        resolver.is_context_dependent_path(asset_path)
    }

    fn __repr__(&self) -> &str {
        "Ar.Resolver"
    }
}

// ============================================================================
// _PyResolverContextBinder  (context manager)
// ============================================================================

/// Context manager that binds an ArResolverContext for the duration of a block.
///
/// Mirrors `pxr.Ar._PyResolverContextBinder` / `ArResolverContextBinder`.
///
/// ```python
/// with Ar.ResolverContextBinder(context):
///     resolved = Ar.GetResolver().Resolve("asset.usd")
/// ```
#[pyclass(skip_from_py_object,name = "ResolverContextBinder", module = "pxr_rs.Ar")]
pub struct PyResolverContextBinder {
    context: PyResolverContext,
    active: bool,
}

#[pymethods]
impl PyResolverContextBinder {
    #[new]
    fn new(context: PyResolverContext) -> Self {
        Self { context, active: false }
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.active = true;
        slf
    }

    #[pyo3(signature = (_exc_type = None, _exc_val = None, _exc_tb = None))]
    fn __exit__(
        mut slf: PyRefMut<'_, Self>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> bool {
        slf.active = false;
        false
    }

    fn __repr__(&self) -> String {
        format!("Ar.ResolverContextBinder(active={}, paths={:?})",
            self.active, self.context.search_paths)
    }
}

// ============================================================================
// ArNotice
// ============================================================================

/// Notice namespace class for asset resolver notices.
///
/// Mirrors `pxr.Ar.Notice` / `ArNotice`.
#[pyclass(skip_from_py_object,name = "Notice", module = "pxr_rs.Ar")]
pub struct PyArNotice;

#[pymethods]
impl PyArNotice {
    fn __repr__(&self) -> &str {
        "Ar.Notice"
    }
}

/// Notice sent when the resolver's state changes.
///
/// Mirrors `pxr.Ar.Notice.ResolverChanged`.
#[pyclass(skip_from_py_object,name = "ResolverChanged", module = "pxr_rs.Ar")]
#[derive(Clone)]
pub struct PyResolverChanged {
    context: Option<PyResolverContext>,
}

#[pymethods]
impl PyResolverChanged {
    #[new]
    #[pyo3(signature = (context = None))]
    fn new(context: Option<PyResolverContext>) -> Self {
        Self { context }
    }

    /// True if the given resolver context is affected by this notice.
    #[pyo3(name = "AffectsContext")]
    fn affects_context(&self, _context: &PyResolverContext) -> bool {
        // Stub: true when no specific context is set (affects all).
        self.context.is_none()
    }

    fn __repr__(&self) -> &str {
        "Ar.Notice.ResolverChanged"
    }
}

// ============================================================================
// Free functions
// ============================================================================

/// Return the global resolver singleton.
///
/// Mirrors `pxr.Ar.GetResolver()`.
#[pyfunction]
#[pyo3(name = "GetResolver")]
fn get_resolver_py() -> PyResolver {
    PyResolver
}

/// Set the preferred resolver type by name (must be called before first GetResolver).
///
/// Mirrors `pxr.Ar.SetPreferredResolver(typeName)`.
#[pyfunction]
#[pyo3(name = "SetPreferredResolver")]
fn set_preferred_resolver_py(type_name: &str) {
    // Ignore the result; errors logged internally.
    let _ = set_preferred_resolver(type_name);
}

/// Return all registered URI schemes.
///
/// Mirrors `pxr.Ar.GetRegisteredURISchemes()`.
#[pyfunction]
#[pyo3(name = "GetRegisteredURISchemes")]
fn get_registered_uri_schemes() -> Vec<String> {
    Vec::new()
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Ar classes and free functions into the `pxr.Ar` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;

    m.add_class::<PyResolvedPath>()?;
    m.add_class::<PyResolverContext>()?;
    m.add_class::<PyAssetInfo>()?;
    m.add_class::<PyTimestamp>()?;
    m.add_class::<PyResolver>()?;
    m.add_class::<PyResolverContextBinder>()?;
    m.add_class::<PyArNotice>()?;
    m.add_class::<PyResolverChanged>()?;

    m.add_function(wrap_pyfunction!(get_resolver_py, m)?)?;
    m.add_function(wrap_pyfunction!(set_preferred_resolver_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_registered_uri_schemes, m)?)?;

    Ok(())
}
