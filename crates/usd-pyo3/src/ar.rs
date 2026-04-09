//! pxr.Ar — Asset Resolver Python bindings.
//!
//! Drop-in replacement for `pxr.Ar` from C++ OpenUSD.
//! Covers: ArResolver, ArResolvedPath, ArResolverContext, ArDefaultResolverContext,
//! ArDefaultResolver, ArAssetInfo, ArTimestamp, ArNotice, ArAsset,
//! ResolverScopedCache, ResolverContextBinder, package utils.

use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use usd_ar::resolver::{DefaultResolver, get_resolver, set_preferred_resolver};
use usd_ar::{
    Asset, AssetInfo, DefaultResolverContext, ResolvedPath, ResolverContext,
    ResolverContextBinder as RustBinder, ResolverScopedCache as RustScopedCache, Timestamp,
};

// ============================================================================
// ArResolvedPath
// ============================================================================

/// A resolved asset path — the physical location after resolution.
///
/// Mirrors `pxr.Ar.ResolvedPath` / `ArResolvedPath`.
#[pyclass(skip_from_py_object, name = "ResolvedPath", module = "pxr.Ar")]
#[derive(Clone)]
pub struct PyResolvedPath {
    inner: ResolvedPath,
}

#[pymethods]
impl PyResolvedPath {
    /// Create a ResolvedPath from a path string.
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
        if self.inner.is_empty() {
            "Ar.ResolvedPath()".to_string()
        } else {
            format!("Ar.ResolvedPath('{}')", self.inner.as_str())
        }
    }

    fn __str__(&self) -> &str {
        self.inner.as_str()
    }

    /// Rich comparison: supports ==, !=, <, <=, >, >= with both ResolvedPath and str.
    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: pyo3::basic::CompareOp) -> PyResult<bool> {
        let other_str = if let Ok(rp) = other.extract::<PyRef<'_, PyResolvedPath>>() {
            rp.inner.as_str().to_string()
        } else if let Ok(s) = other.extract::<String>() {
            s
        } else {
            return match op {
                pyo3::basic::CompareOp::Eq => Ok(false),
                pyo3::basic::CompareOp::Ne => Ok(true),
                _ => Err(PyTypeError::new_err("unsupported operand type")),
            };
        };
        let self_str = self.inner.as_str();
        Ok(match op {
            pyo3::basic::CompareOp::Eq => self_str == other_str,
            pyo3::basic::CompareOp::Ne => self_str != other_str,
            pyo3::basic::CompareOp::Lt => self_str < other_str.as_str(),
            pyo3::basic::CompareOp::Le => self_str <= other_str.as_str(),
            pyo3::basic::CompareOp::Gt => self_str > other_str.as_str(),
            pyo3::basic::CompareOp::Ge => self_str >= other_str.as_str(),
        })
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
// ArDefaultResolverContext
// ============================================================================

/// Default resolver context with search paths.
///
/// Mirrors `pxr.Ar.DefaultResolverContext` / `ArDefaultResolverContext`.
#[pyclass(from_py_object, name = "DefaultResolverContext", module = "pxr.Ar")]
#[derive(Clone)]
pub struct PyDefaultResolverContext {
    inner: DefaultResolverContext,
}

#[pymethods]
impl PyDefaultResolverContext {
    /// Create a DefaultResolverContext with optional search paths.
    #[new]
    #[pyo3(signature = (search_paths = None))]
    fn new(search_paths: Option<Vec<String>>) -> Self {
        let ctx = match search_paths {
            Some(paths) => DefaultResolverContext::new(paths),
            None => DefaultResolverContext::empty(),
        };
        Self { inner: ctx }
    }

    /// Return the search paths.
    #[pyo3(name = "GetSearchPath")]
    fn get_search_path(&self) -> Vec<String> {
        self.inner.search_paths().to_vec()
    }

    fn __repr__(&self) -> String {
        let paths = self.inner.search_paths();
        if paths.is_empty() {
            "Ar.DefaultResolverContext()".to_string()
        } else {
            format!("Ar.DefaultResolverContext({})", format!("{:?}", paths))
        }
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut h);
        h.finish()
    }

    fn __bool__(&self) -> bool {
        !self.inner.search_paths().is_empty()
    }
}

// ============================================================================
// ArResolverContext
// ============================================================================

/// Provides additional data to the resolver for use during resolution.
///
/// Mirrors `pxr.Ar.ResolverContext` / `ArResolverContext`.
///
/// Can hold one or more context objects (e.g. DefaultResolverContext).
#[pyclass(from_py_object, name = "ResolverContext", module = "pxr.Ar")]
#[derive(Clone)]
pub struct PyResolverContext {
    /// The wrapped context objects (currently only DefaultResolverContext).
    context_objs: Vec<PyDefaultResolverContext>,
}

impl PyResolverContext {
    /// Build a Rust `ResolverContext` from our stored objects.
    pub(crate) fn to_rust_context(&self) -> ResolverContext {
        if self.context_objs.is_empty() {
            ResolverContext::new()
        } else {
            // Combine all context objects
            let mut ctx = ResolverContext::new();
            for obj in &self.context_objs {
                ctx.add(obj.inner.clone());
            }
            ctx
        }
    }

    /// Build from a single DefaultResolverContext.
    fn from_default_ctx(ctx: PyDefaultResolverContext) -> Self {
        Self {
            context_objs: vec![ctx],
        }
    }

    /// Build from a Rust [`ResolverContext`] (e.g. `Usd.Stage.GetPathResolverContext`).
    pub(crate) fn from_ar_resolver_context(
        py: Python<'_>,
        ctx: &ResolverContext,
    ) -> PyResult<Py<Self>> {
        let mut context_objs = Vec::new();
        if let Some(d) = ctx.get::<DefaultResolverContext>() {
            context_objs.push(PyDefaultResolverContext { inner: d.clone() });
        }
        Py::new(py, Self { context_objs })
    }
}

#[pymethods]
impl PyResolverContext {
    /// Create a ResolverContext.
    ///
    /// Accepts: None, empty tuple/list, a DefaultResolverContext, or a
    /// tuple/list of DefaultResolverContext objects.
    #[new]
    #[pyo3(signature = (arg = None))]
    fn new(arg: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(arg) = arg else {
            return Ok(Self {
                context_objs: vec![],
            });
        };

        // None -> empty context
        if arg.is_none() {
            return Ok(Self {
                context_objs: vec![],
            });
        }

        // Single DefaultResolverContext
        if let Ok(ctx) = arg.extract::<PyDefaultResolverContext>() {
            return Ok(Self::from_default_ctx(ctx));
        }

        // Tuple or list of context objects
        if let Ok(seq) = arg.cast::<PyTuple>() {
            return Self::from_sequence(seq.iter());
        }
        if let Ok(seq) = arg.cast::<PyList>() {
            return Self::from_sequence(seq.iter());
        }

        Err(PyTypeError::new_err(
            "Expected None, DefaultResolverContext, or tuple/list of context objects",
        ))
    }

    /// True if this context contains no data.
    #[pyo3(name = "IsEmpty")]
    fn is_empty(&self) -> bool {
        self.context_objs.is_empty()
    }

    /// Return the list of context objects.
    #[pyo3(name = "Get")]
    fn get(&self) -> Vec<PyDefaultResolverContext> {
        self.context_objs.clone()
    }

    fn __repr__(&self) -> String {
        if self.context_objs.is_empty() {
            "Ar.ResolverContext()".to_string()
        } else {
            let parts: Vec<String> = self.context_objs.iter().map(|c| c.__repr__()).collect();
            format!("Ar.ResolverContext({})", parts.join(", "))
        }
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.context_objs == other.context_objs
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.context_objs != other.context_objs
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.context_objs.hash(&mut h);
        h.finish()
    }

    fn __bool__(&self) -> bool {
        !self.is_empty()
    }
}

impl PyResolverContext {
    /// Parse a sequence of items into context objects.
    fn from_sequence<'py>(iter: impl Iterator<Item = Bound<'py, PyAny>>) -> PyResult<Self> {
        let mut objs = Vec::new();
        for item in iter {
            if let Ok(ctx) = item.extract::<PyDefaultResolverContext>() {
                objs.push(ctx);
            } else {
                return Err(PyTypeError::new_err(
                    "Expected DefaultResolverContext object in sequence",
                ));
            }
        }
        Ok(Self { context_objs: objs })
    }
}

impl PartialEq for PyDefaultResolverContext {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl std::hash::Hash for PyDefaultResolverContext {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

// ============================================================================
// ArAssetInfo
// ============================================================================

/// Metadata about a resolved asset.
///
/// Mirrors `pxr.Ar.AssetInfo` / `ArAssetInfo`.
#[pyclass(skip_from_py_object, name = "AssetInfo", module = "pxr.Ar")]
#[derive(Clone)]
pub struct PyAssetInfo {
    inner: AssetInfo,
}

#[pymethods]
impl PyAssetInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: AssetInfo::new(),
        }
    }

    /// Asset version string (empty string if not set).
    #[getter]
    fn version(&self) -> String {
        self.inner.version.clone().unwrap_or_default()
    }

    #[setter]
    fn set_version(&mut self, v: String) {
        if v.is_empty() {
            self.inner.version = None;
        } else {
            self.inner.version = Some(v);
        }
    }

    /// Asset name string (empty string if not set).
    #[getter]
    #[pyo3(name = "assetName")]
    fn asset_name(&self) -> String {
        self.inner.asset_name.clone().unwrap_or_default()
    }

    #[setter]
    #[pyo3(name = "assetName")]
    fn set_asset_name(&mut self, v: String) {
        if v.is_empty() {
            self.inner.asset_name = None;
        } else {
            self.inner.asset_name = Some(v);
        }
    }

    /// Repository path (empty string if not set).
    #[getter]
    #[pyo3(name = "repoPath")]
    fn repo_path(&self) -> String {
        self.inner.repo_path.clone().unwrap_or_default()
    }

    #[setter]
    #[pyo3(name = "repoPath")]
    fn set_repo_path(&mut self, v: String) {
        if v.is_empty() {
            self.inner.repo_path = None;
        } else {
            self.inner.repo_path = Some(v);
        }
    }

    /// Resolver-specific info (any Python value, or None).
    #[getter]
    #[pyo3(name = "resolverInfo")]
    fn resolver_info(&self, py: Python<'_>) -> Py<PyAny> {
        // We store it as a generic Python-compatible value.
        // For now, return None (AssetInfo::resolver_info is Option<Value>).
        match &self.inner.resolver_info {
            Some(v) => {
                // Try to convert common Value types back to Python
                if let Some(i) = v.get::<i64>() {
                    i.into_pyobject(py)
                        .expect("into_pyobject")
                        .into_any()
                        .unbind()
                } else if let Some(s) = v.get::<String>() {
                    s.into_pyobject(py)
                        .expect("into_pyobject")
                        .into_any()
                        .unbind()
                } else if let Some(f) = v.get::<f64>() {
                    f.into_pyobject(py)
                        .expect("into_pyobject")
                        .into_any()
                        .unbind()
                } else if let Some(b) = v.get::<bool>() {
                    let val: bool = *b;
                    val.into_pyobject(py)
                        .expect("into_pyobject")
                        .to_owned()
                        .into_any()
                        .unbind()
                } else {
                    py.None()
                }
            }
            None => py.None(),
        }
    }

    #[setter]
    #[pyo3(name = "resolverInfo")]
    fn set_resolver_info(&mut self, v: &Bound<'_, PyAny>) -> PyResult<()> {
        if v.is_none() {
            self.inner.resolver_info = None;
        } else if let Ok(i) = v.extract::<i64>() {
            self.inner.resolver_info = Some(usd_vt::Value::from(i));
        } else if let Ok(s) = v.extract::<String>() {
            self.inner.resolver_info = Some(usd_vt::Value::from(s));
        } else if let Ok(f) = v.extract::<f64>() {
            self.inner.resolver_info = Some(usd_vt::Value::from(f));
        } else if let Ok(b) = v.extract::<bool>() {
            self.inner.resolver_info = Some(usd_vt::Value::from(b));
        } else {
            self.inner.resolver_info = None;
        }
        Ok(())
    }

    /// True if this AssetInfo has no data.
    #[pyo3(name = "IsEmpty")]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!(
            "Ar.AssetInfo(name={:?}, version={:?})",
            self.inner.asset_name, self.inner.version
        )
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

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// ArTimestamp
// ============================================================================

/// A timestamp for an asset (Unix time, or invalid/NaN).
///
/// Mirrors `pxr.Ar.Timestamp` / `ArTimestamp`.
#[pyclass(skip_from_py_object, name = "Timestamp", module = "pxr.Ar")]
#[derive(Clone, Copy)]
pub struct PyTimestamp {
    inner: Timestamp,
}

#[pymethods]
impl PyTimestamp {
    /// Create a Timestamp from a Unix time value (seconds since epoch).
    /// Called with no argument creates an invalid timestamp.
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

    /// Return the Unix time value.
    /// Raises Tf.ErrorException if the timestamp is invalid.
    #[pyo3(name = "GetTime")]
    fn get_time(&self) -> PyResult<f64> {
        if self.inner.is_valid() {
            Ok(self.inner.try_get_time().unwrap_or(f64::NAN))
        } else {
            // C++ raises TF_CODING_ERROR which maps to Tf.ErrorException (PyException)
            Err(pyo3::exceptions::PyException::new_err(
                "Cannot get time from invalid timestamp",
            ))
        }
    }

    fn __repr__(&self) -> String {
        if self.inner.is_valid() {
            // Always show as float with at least one decimal place
            let t = self.inner.try_get_time().unwrap_or(f64::NAN);
            // Use {:?} to get f64 debug repr which always includes decimal point
            let s = format!("{t:?}");
            // Ensure it has a decimal point (f64 debug should always have one)
            if s.contains('.') || s.contains('e') || s.contains('E') {
                format!("Ar.Timestamp({s})")
            } else {
                format!("Ar.Timestamp({s}.0)")
            }
        } else {
            "Ar.Timestamp()".to_string()
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
// ArAsset (Python wrapper for reading resolved assets)
// ============================================================================

/// A resolved asset that can be read.
///
/// Mirrors `pxr.Ar.Asset` — supports context manager protocol.
#[pyclass(skip_from_py_object, name = "Asset", module = "pxr.Ar")]
pub struct PyAsset {
    asset: Option<Arc<dyn Asset>>,
}

#[pymethods]
impl PyAsset {
    /// Return the total size in bytes.
    #[pyo3(name = "GetSize")]
    fn get_size(&self) -> PyResult<usize> {
        let asset = self
            .asset
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Unable to access invalid asset"))?;
        Ok(asset.size())
    }

    /// Return the entire contents as bytes.
    #[pyo3(name = "GetBuffer")]
    fn get_buffer(&self) -> PyResult<Vec<u8>> {
        let asset = self
            .asset
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Unable to access invalid asset"))?;
        match asset.get_buffer() {
            Some(buf) => Ok(buf.to_vec()),
            None => Ok(Vec::new()),
        }
    }

    /// Read `count` bytes starting at `offset`.
    ///
    /// If offset is beyond the asset, raises ValueError.
    /// If count exceeds available bytes, returns what's available.
    #[pyo3(name = "Read")]
    fn read(&self, count: usize, offset: usize) -> PyResult<Vec<u8>> {
        let asset = self
            .asset
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Unable to access invalid asset"))?;
        let size = asset.size();
        if offset > size {
            return Err(PyValueError::new_err("Invalid read offset"));
        }
        let available = size - offset;
        let to_read = count.min(available);
        let mut buffer = vec![0u8; to_read];
        let bytes_read = asset.read(&mut buffer, offset);
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// True if the asset is valid.
    fn __bool__(&self) -> bool {
        self.asset.is_some()
    }

    /// Context manager enter.
    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.asset.is_none() {
            return Err(PyRuntimeError::new_err("Unable to access invalid asset"));
        }
        Ok(slf)
    }

    /// Context manager exit — releases the asset.
    #[pyo3(signature = (_exc_type = None, _exc_val = None, _exc_tb = None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> bool {
        self.asset = None;
        false
    }

    fn __repr__(&self) -> String {
        if let Some(ref asset) = self.asset {
            format!("Ar.Asset(size={})", asset.size())
        } else {
            "Ar.Asset(invalid)".to_string()
        }
    }
}

// ============================================================================
// ArDefaultResolver (class-level static methods)
// ============================================================================

/// Default filesystem-based asset resolver.
///
/// Mirrors `pxr.Ar.DefaultResolver` / `ArDefaultResolver`.
#[pyclass(skip_from_py_object, name = "DefaultResolver", module = "pxr.Ar")]
pub struct PyDefaultResolver;

#[pymethods]
impl PyDefaultResolver {
    /// Set the default search paths for asset resolution.
    ///
    /// This is a static method equivalent to `Ar.DefaultResolver.SetDefaultSearchPath(paths)`.
    #[staticmethod]
    #[pyo3(name = "SetDefaultSearchPath")]
    fn set_default_search_path(search_path: Vec<String>) {
        DefaultResolver::set_default_search_path_static(search_path);
    }

    fn __repr__(&self) -> &str {
        "Ar.DefaultResolver"
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
#[pyclass(skip_from_py_object, name = "Resolver", module = "pxr.Ar")]
pub struct PyResolver;

#[pymethods]
impl PyResolver {
    /// Resolve an asset path to its physical location.
    #[pyo3(name = "Resolve")]
    fn resolve(&self, asset_path: &str) -> PyResolvedPath {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        PyResolvedPath::from_inner(resolver.resolve(asset_path))
    }

    /// Resolve a path for a new (possibly non-existent) asset.
    #[pyo3(name = "ResolveForNewAsset")]
    fn resolve_for_new_asset(&self, asset_path: &str) -> PyResolvedPath {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        PyResolvedPath::from_inner(resolver.resolve_for_new_asset(asset_path))
    }

    /// Create an identifier for the given asset path.
    #[pyo3(name = "CreateIdentifier")]
    #[pyo3(signature = (asset_path, anchor_path = None))]
    fn create_identifier(&self, asset_path: &str, anchor_path: Option<&PyResolvedPath>) -> String {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        resolver.create_identifier(asset_path, anchor_path.map(|p| p.inner()))
    }

    /// Create an identifier for a new asset at the given path.
    #[pyo3(name = "CreateIdentifierForNewAsset")]
    #[pyo3(signature = (asset_path, anchor_path = None))]
    fn create_identifier_for_new_asset(
        &self,
        asset_path: &str,
        anchor_path: Option<&PyResolvedPath>,
    ) -> String {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        resolver.create_identifier_for_new_asset(asset_path, anchor_path.map(|p| p.inner()))
    }

    /// Create a default context for this resolver.
    #[pyo3(name = "CreateDefaultContext")]
    fn create_default_context(&self) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let ctx = resolver.create_default_context();
        rust_context_to_py(&ctx)
    }

    /// Create a default context for the given asset.
    #[pyo3(name = "CreateDefaultContextForAsset")]
    fn create_default_context_for_asset(&self, asset_path: &str) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let ctx = resolver.create_default_context_for_asset(asset_path);
        rust_context_to_py(&ctx)
    }

    /// Create a context from a context string (colon-separated search paths).
    ///
    /// With two arguments: `CreateContextFromString(uri_scheme, context_str)`.
    #[pyo3(name = "CreateContextFromString")]
    #[pyo3(signature = (context_str_or_scheme, context_str = None))]
    fn create_context_from_string(
        &self,
        context_str_or_scheme: &str,
        context_str: Option<&str>,
    ) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");

        let ctx = match context_str {
            Some(cs) => {
                // Two-arg form: (uri_scheme, context_str)
                resolver.create_context_from_string_with_scheme(context_str_or_scheme, cs)
            }
            None => {
                // One-arg form: (context_str)
                resolver.create_context_from_string(context_str_or_scheme)
            }
        };
        rust_context_to_py(&ctx)
    }

    /// Create a context from multiple (uri_scheme, context_str) pairs.
    #[pyo3(name = "CreateContextFromStrings")]
    fn create_context_from_strings(
        &self,
        context_strings: Vec<(String, String)>,
    ) -> PyResolverContext {
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let ctx = resolver.create_context_from_strings(&context_strings);
        rust_context_to_py(&ctx)
    }

    /// Return the file extension for the given asset path.
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

    /// Open an asset at the given resolved path for reading.
    ///
    /// Returns None if the asset doesn't exist or is a directory.
    #[pyo3(name = "OpenAsset")]
    #[pyo3(signature = (resolved_path = None, **_kwargs))]
    fn open_asset(
        &self,
        resolved_path: Option<&PyResolvedPath>,
        _kwargs: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> Option<PyAsset> {
        // Also check kwargs for "resolvedPath" keyword arg
        let rp = if let Some(rp) = resolved_path {
            rp
        } else if let Some(kwargs) = _kwargs {
            if let Ok(Some(val)) = kwargs.get_item("resolvedPath") {
                if let Ok(rp) = val.extract::<PyRef<'_, PyResolvedPath>>() {
                    // Use the extracted value - need to handle lifetime
                    let inner = rp.inner().clone();
                    let lock = get_resolver();
                    let resolver = lock.read().expect("resolver rwlock poisoned");
                    // Check: don't open directories
                    let path = std::path::Path::new(inner.as_str());
                    if path.is_dir() {
                        return None;
                    }
                    let asset = resolver.open_asset(&inner)?;
                    return Some(PyAsset { asset: Some(asset) });
                }
                return None;
            } else {
                return None;
            }
        } else {
            return None;
        };

        let inner = rp.inner();
        // Check: don't open directories
        let path = std::path::Path::new(inner.as_str());
        if path.is_dir() {
            return None;
        }
        let lock = get_resolver();
        let resolver = lock.read().expect("resolver rwlock poisoned");
        let asset = resolver.open_asset(inner)?;
        Some(PyAsset { asset: Some(asset) })
    }

    fn __repr__(&self) -> &str {
        "Ar.Resolver"
    }
}

// ============================================================================
// _PyResolverContextBinder (context manager)
// ============================================================================

/// Context manager that binds an ArResolverContext for the duration of a block.
///
/// Mirrors `pxr.Ar.ResolverContextBinder`.
#[pyclass(skip_from_py_object, name = "ResolverContextBinder", module = "pxr.Ar")]
pub struct PyResolverContextBinder {
    context: PyResolverContext,
    binder: Option<RustBinder>,
}

#[pymethods]
impl PyResolverContextBinder {
    #[new]
    fn new(context: &Bound<'_, PyAny>) -> PyResult<Self> {
        // Accept either PyResolverContext or PyDefaultResolverContext
        let py_ctx = if let Ok(ctx) = context.extract::<PyResolverContext>() {
            ctx
        } else if let Ok(drc) = context.extract::<PyDefaultResolverContext>() {
            PyResolverContext::from_default_ctx(drc)
        } else {
            return Err(PyTypeError::new_err(
                "Expected ResolverContext or DefaultResolverContext",
            ));
        };
        Ok(Self {
            context: py_ctx,
            binder: None,
        })
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        let rust_ctx = slf.context.to_rust_context();
        slf.binder = Some(RustBinder::new(rust_ctx));
        slf
    }

    #[pyo3(signature = (_exc_type = None, _exc_val = None, _exc_tb = None))]
    fn __exit__(
        mut slf: PyRefMut<'_, Self>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> bool {
        // Drop the binder to unbind the context
        slf.binder = None;
        false
    }

    fn __repr__(&self) -> String {
        format!("Ar.ResolverContextBinder(active={})", self.binder.is_some())
    }
}

// ============================================================================
// ResolverScopedCache (context manager)
// ============================================================================

/// Context manager for resolver caching scope.
///
/// Mirrors `pxr.Ar.ResolverScopedCache`.
#[pyclass(skip_from_py_object, name = "ResolverScopedCache", module = "pxr.Ar")]
pub struct PyResolverScopedCache {
    cache: Option<RustScopedCache>,
}

#[pymethods]
impl PyResolverScopedCache {
    #[new]
    fn new() -> Self {
        Self { cache: None }
    }

    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.cache = Some(RustScopedCache::new());
        slf
    }

    #[pyo3(signature = (_exc_type = None, _exc_val = None, _exc_tb = None))]
    fn __exit__(
        mut slf: PyRefMut<'_, Self>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> bool {
        slf.cache = None;
        false
    }

    fn __repr__(&self) -> String {
        format!("Ar.ResolverScopedCache(active={})", self.cache.is_some())
    }
}

// ============================================================================
// ArNotice
// ============================================================================

/// Notice namespace class for asset resolver notices.
#[pyclass(skip_from_py_object, name = "Notice", module = "pxr.Ar")]
pub struct PyArNotice;

#[pymethods]
impl PyArNotice {
    fn __repr__(&self) -> &str {
        "Ar.Notice"
    }
}

/// Notice sent when the resolver's state changes.
#[pyclass(skip_from_py_object, name = "ResolverChanged", module = "pxr.Ar")]
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
    fn affects_context(&self, _context: &Bound<'_, PyAny>) -> bool {
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
#[pyfunction]
#[pyo3(name = "GetResolver")]
fn get_resolver_py() -> PyResolver {
    PyResolver
}

/// Return the underlying resolver (same as GetResolver for our implementation).
#[pyfunction]
#[pyo3(name = "GetUnderlyingResolver")]
fn get_underlying_resolver_py() -> PyDefaultResolver {
    // Force resolver initialization
    let _ = get_resolver();
    PyDefaultResolver
}

/// Set the preferred resolver type by name.
#[pyfunction]
#[pyo3(name = "SetPreferredResolver")]
fn set_preferred_resolver_py(type_name: &str) {
    let _ = set_preferred_resolver(type_name);
}

/// Return all registered URI schemes.
#[pyfunction]
#[pyo3(name = "GetRegisteredURISchemes")]
fn get_registered_uri_schemes() -> Vec<String> {
    usd_ar::resolver::get_registered_uri_schemes()
}

/// Return the list of available resolver TfTypes.
#[pyfunction]
#[pyo3(name = "GetAvailableResolvers")]
fn get_available_resolvers_py() -> Vec<String> {
    usd_ar::resolver::get_available_resolvers()
        .iter()
        .map(|t| t.type_name().to_string())
        .collect()
}

// ============================================================================
// Package utils (free functions)
// ============================================================================

/// True if the given path is a package-relative path.
#[pyfunction]
#[pyo3(name = "IsPackageRelativePath")]
fn is_package_relative_path_py(path: &str) -> bool {
    usd_ar::is_package_relative_path(path)
}

/// Join a list of paths into a package-relative path.
///
/// Accepts either a list of strings (2+ elements) or two separate string args.
#[pyfunction]
#[pyo3(name = "JoinPackageRelativePath")]
#[pyo3(signature = (paths_or_first, second = None))]
fn join_package_relative_path_py(
    paths_or_first: &Bound<'_, PyAny>,
    second: Option<&str>,
) -> PyResult<String> {
    if let Some(s2) = second {
        // Two-arg form: JoinPackageRelativePath(pkg, packaged)
        let s1: String = paths_or_first.extract()?;
        Ok(usd_ar::join_package_relative_path_pair(&s1, s2))
    } else if let Ok(paths) = paths_or_first.extract::<Vec<String>>() {
        // List form: JoinPackageRelativePath(["a.pack", "b.file"])
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        Ok(usd_ar::join_package_relative_path(&refs))
    } else {
        Err(PyTypeError::new_err(
            "Expected list of strings or (str, str)",
        ))
    }
}

/// Split a package-relative path at the innermost nesting level.
///
/// Returns (package_path, packaged_path) tuple.
#[pyfunction]
#[pyo3(name = "SplitPackageRelativePathInner")]
fn split_package_relative_path_inner_py(path: &str) -> (String, String) {
    usd_ar::split_package_relative_path_inner(path)
}

/// Split a package-relative path at the outermost nesting level.
///
/// Returns (package_path, packaged_path) tuple.
#[pyfunction]
#[pyo3(name = "SplitPackageRelativePathOuter")]
fn split_package_relative_path_outer_py(path: &str) -> (String, String) {
    usd_ar::split_package_relative_path_outer(path)
}

/// Test helper: implicit conversion of context objects.
/// Returns the ResolverContext equivalent of the argument.
#[pyfunction]
#[pyo3(name = "_TestImplicitConversion")]
fn test_implicit_conversion(arg: &Bound<'_, PyAny>) -> PyResult<PyResolverContext> {
    // Mirrors C++ _TestImplicitConversion: accepts same args as ResolverContext ctor
    PyResolverContext::new(Some(arg))
}

// ============================================================================
// Helpers
// ============================================================================

/// Convert a Rust ResolverContext to a Python PyResolverContext.
fn rust_context_to_py(ctx: &ResolverContext) -> PyResolverContext {
    let mut objs = Vec::new();
    if let Some(drc) = ctx.get::<DefaultResolverContext>() {
        objs.push(PyDefaultResolverContext { inner: drc.clone() });
    }
    PyResolverContext { context_objs: objs }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Ar classes and free functions into the `pxr.Ar` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = py;

    m.add_class::<PyResolvedPath>()?;
    m.add_class::<PyDefaultResolverContext>()?;
    m.add_class::<PyResolverContext>()?;
    m.add_class::<PyAssetInfo>()?;
    m.add_class::<PyTimestamp>()?;
    m.add_class::<PyResolver>()?;
    m.add_class::<PyDefaultResolver>()?;
    m.add_class::<PyAsset>()?;
    m.add_class::<PyResolverContextBinder>()?;
    m.add_class::<PyResolverScopedCache>()?;
    m.add_class::<PyArNotice>()?;
    m.add_class::<PyResolverChanged>()?;

    m.add_function(wrap_pyfunction!(get_resolver_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_underlying_resolver_py, m)?)?;
    m.add_function(wrap_pyfunction!(set_preferred_resolver_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_registered_uri_schemes, m)?)?;
    m.add_function(wrap_pyfunction!(get_available_resolvers_py, m)?)?;

    // Package utils
    m.add_function(wrap_pyfunction!(is_package_relative_path_py, m)?)?;
    m.add_function(wrap_pyfunction!(join_package_relative_path_py, m)?)?;
    m.add_function(wrap_pyfunction!(split_package_relative_path_inner_py, m)?)?;
    m.add_function(wrap_pyfunction!(split_package_relative_path_outer_py, m)?)?;

    // Test helpers
    m.add_function(wrap_pyfunction!(test_implicit_conversion, m)?)?;

    // Nest ResolverChanged under Notice
    let notice_cls = m.getattr("Notice")?;
    notice_cls.setattr("ResolverChanged", py.get_type::<PyResolverChanged>())?;

    Ok(())
}
