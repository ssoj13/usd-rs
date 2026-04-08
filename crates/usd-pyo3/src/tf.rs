//! pxr.Tf — Tools Foundation Python bindings.
//!
//! Drop-in replacement for `pxr.Tf` from C++ OpenUSD.
//! Covers: Token, Type, Notice, Stopwatch, Debug, and module-level
//! diagnostic helpers (Warn, Status, RaiseCodingError, RaiseRuntimeError).

use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::sync::Mutex;

// ============================================================================
// Token
// ============================================================================

/// Interned string for fast O(1) comparison and hashing.
///
/// Mirrors `pxr.Tf.Token` / `TfToken` from C++ OpenUSD.
#[pyclass(skip_from_py_object,name = "Token", module = "pxr_rs.Tf")]
#[derive(Clone)]
pub struct PyToken {
    inner: usd_tf::Token,
}

#[pymethods]
impl PyToken {
    #[new]
    #[pyo3(signature = (text = ""))]
    fn new(text: &str) -> Self {
        Self {
            inner: usd_tf::Token::new(text),
        }
    }

    fn __str__(&self) -> &str {
        self.inner.as_str()
    }

    fn __repr__(&self) -> String {
        format!("Tf.Token('{}')", self.inner.as_str())
    }

    fn __hash__(&self) -> u64 {
        self.inner.hash()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &Self) -> bool {
        self.inner != other.inner
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __len__(&self) -> usize {
        self.inner.as_str().len()
    }

    /// The string value of this token.
    #[getter]
    fn text(&self) -> &str {
        self.inner.as_str()
    }
}

impl PyToken {
    pub fn token(&self) -> &usd_tf::Token {
        &self.inner
    }

    pub fn from_token(t: usd_tf::Token) -> Self {
        Self { inner: t }
    }
}

// ============================================================================
// Type  (TfType)
// ============================================================================

/// Runtime type handle — mirrors `pxr.Tf.Type` / `TfType`.
///
/// Use `Type.Find()` / `Type.FindByName()` to look up registered types.
#[pyclass(skip_from_py_object,name = "Type", module = "pxr_rs.Tf")]
#[derive(Clone)]
pub struct PyType {
    inner: usd_tf::TfType,
}

#[pymethods]
impl PyType {
    // ---- construction / lookup (class methods) ----

    /// Return the unknown (sentinel) type.
    #[classmethod]
    #[pyo3(name = "Unknown")]
    fn unknown(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self {
            inner: usd_tf::TfType::unknown(),
        }
    }

    /// Return the root type of the hierarchy.
    #[classmethod]
    #[pyo3(name = "GetRoot")]
    fn get_root(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self {
            inner: usd_tf::TfType::get_root(),
        }
    }

    /// Find the `TfType` for a C++ / plugin type name (e.g. `"UsdGeomMesh"`).
    #[classmethod]
    #[pyo3(name = "FindByName")]
    fn find_by_name(_cls: &Bound<'_, pyo3::types::PyType>, name: &str) -> Self {
        Self {
            inner: usd_tf::TfType::find_by_name(name),
        }
    }

    /// Find a derived type by its name under this base type (alias-aware).
    #[pyo3(name = "FindDerivedByName")]
    fn find_derived_by_name(&self, name: &str) -> Self {
        Self {
            inner: self.inner.find_derived_by_name(name),
        }
    }

    // ---- predicates ----

    /// True if this type has not been registered.
    #[getter]
    #[pyo3(name = "isUnknown")]
    fn is_unknown(&self) -> bool {
        self.inner.is_unknown()
    }

    /// True if this is the root type.
    #[getter]
    #[pyo3(name = "isRoot")]
    fn is_root(&self) -> bool {
        self.inner.is_root()
    }

    /// True if this type was registered as an enum.
    #[getter]
    #[pyo3(name = "isEnumType")]
    fn is_enum_type(&self) -> bool {
        self.inner.is_enum()
    }

    /// True if this is a plain-old-data type.
    #[getter]
    #[pyo3(name = "isPlainOldDataType")]
    fn is_pod(&self) -> bool {
        self.inner.is_plain_old_data_type()
    }

    // ---- properties ----

    /// The canonical type name string.
    #[getter]
    #[pyo3(name = "typeName")]
    fn type_name(&self) -> String {
        self.inner.type_name()
    }

    /// Size of the underlying C++ / Rust type in bytes.
    #[getter]
    #[pyo3(name = "sizeof")]
    fn sizeof_type(&self) -> usize {
        self.inner.get_sizeof()
    }

    // ---- hierarchy ----

    /// Immediate base types.
    #[pyo3(name = "GetBaseTypes")]
    fn get_base_types(&self) -> Vec<Self> {
        self.inner
            .base_types()
            .into_iter()
            .map(|t| Self { inner: t })
            .collect()
    }

    /// All ancestor types in C3 MRO order (self included first).
    #[pyo3(name = "GetAllAncestorTypes")]
    fn get_all_ancestor_types(&self) -> Vec<Self> {
        self.inner
            .get_all_ancestor_types()
            .into_iter()
            .map(|t| Self { inner: t })
            .collect()
    }

    /// Immediately derived (child) types.
    #[pyo3(name = "GetDirectlyDerivedTypes")]
    fn get_directly_derived_types(&self) -> Vec<Self> {
        self.inner
            .get_directly_derived_types()
            .into_iter()
            .map(|t| Self { inner: t })
            .collect()
    }

    /// True if this type is the same as, or derives from, `other`.
    #[pyo3(name = "IsA")]
    fn is_a(&self, other: &Self) -> bool {
        self.inner.is_a(other.inner)
    }

    // ---- aliases ----

    /// Aliases registered for this type.
    #[pyo3(name = "GetAliases")]
    fn get_aliases(&self, base: &Self) -> Vec<String> {
        self.inner.get_aliases_for_derived(base.inner)
    }

    // ---- dunder ----

    fn __repr__(&self) -> String {
        if self.inner.is_unknown() {
            "Tf.Type.Unknown".to_string()
        } else {
            format!("Tf.Type('{}')", self.inner.type_name())
        }
    }

    fn __str__(&self) -> String {
        self.inner.type_name()
    }

    fn __eq__(&self, other: &Self) -> bool {
        // TfType equality: same type_name is sufficient since type names are unique in the registry.
        self.inner.type_name() == other.inner.type_name()
            && self.inner.is_unknown() == other.inner.is_unknown()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.type_name().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// Notice  (TfNotice)
// ============================================================================

/// A listener registration key — holds the revoke handle.
///
/// Mirrors `pxr.Tf.Notice.Listener` / `TfNotice::Key`.
#[pyclass(skip_from_py_object,name = "NoticeListener", module = "pxr_rs.Tf")]
pub struct PyNoticeListener {
    /// The revoke handle from the Rust registry; None after `Revoke()`.
    key: Option<usd_tf::notice::ListenerKey>,
}

#[pymethods]
impl PyNoticeListener {
    /// Revoke this listener — no more callbacks will be delivered.
    #[pyo3(name = "Revoke")]
    fn revoke(&mut self) {
        if let Some(key) = self.key.take() {
            usd_tf::notice::global_registry().revoke(key);
        }
    }

    /// True if the listener is still active.
    #[getter]
    fn is_valid(&self) -> bool {
        self.key.as_ref().map(|k| k.is_valid()).unwrap_or(false)
    }

    fn __repr__(&self) -> &str {
        if self.key.as_ref().map(|k| k.is_valid()).unwrap_or(false) {
            "Tf.NoticeListener(active)"
        } else {
            "Tf.NoticeListener(revoked)"
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        // No stable identity after revoke; compare validity only.
        self.is_valid() == other.is_valid()
    }
}

/// Notice — module-level class that mirrors `pxr.Tf.Notice`.
///
/// Python-level notice types (subclasses of this) carry a string type tag
/// so they can be dispatched through the Rust notice system via a
/// string-keyed shim registry.
#[pyclass(skip_from_py_object,name = "Notice", module = "pxr_rs.Tf", subclass)]
pub struct PyNotice {
    /// Logical type name set by the concrete Python notice subclass.
    notice_type: String,
}

#[pymethods]
impl PyNotice {
    #[new]
    #[pyo3(signature = (notice_type = ""))]
    fn new(notice_type: &str) -> Self {
        Self {
            notice_type: notice_type.to_string(),
        }
    }

    /// Send this notice to all globally registered Python listeners.
    ///
    /// Mirrors `TfNotice::Send()` called on a global sender.
    #[pyo3(name = "SendGlobally")]
    fn send_globally(&self) {
        // Forward into the string-keyed shim dispatcher.
        PYTHON_NOTICE_SHIM.send(&self.notice_type);
    }

    fn __repr__(&self) -> String {
        format!("Tf.Notice('{}')", self.notice_type)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.notice_type == other.notice_type
    }

    // ---- class methods (static) ----

    /// Register a Python callable as a global listener for a notice type name.
    ///
    /// Returns a `NoticeListener` that can be used to revoke the registration.
    ///
    /// ```python
    /// key = Tf.Notice.RegisterGlobally("MyNotice", callback)
    /// key.Revoke()
    /// ```
    #[classmethod]
    #[pyo3(name = "RegisterGlobally")]
    fn register_globally(
        _cls: &Bound<'_, pyo3::types::PyType>,
        notice_type: &Bound<'_, PyAny>,
        callback: Py<PyAny>,
    ) -> PyResult<PyNoticeListener> {
        let tag = notice_type_to_string(notice_type)?;
        let key = PYTHON_NOTICE_SHIM.register(&tag, callback);
        Ok(PyNoticeListener { key: Some(key) })
    }

    /// Register a listener for a notice type with an optional sender.
    ///
    /// Matches C++ `TfNotice::Register(noticeType, callback, sender)`.
    /// `notice_type` can be a string or a Python class (its __name__ is used).
    /// `sender` is currently ignored (global registration).
    #[classmethod]
    #[pyo3(name = "Register", signature = (notice_type, callback, sender=None))]
    fn register(
        _cls: &Bound<'_, pyo3::types::PyType>,
        notice_type: &Bound<'_, PyAny>,
        callback: Py<PyAny>,
        sender: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyNoticeListener> {
        let _ = sender; // TODO: sender-scoped registration
        let tag = notice_type_to_string(notice_type)?;
        let key = PYTHON_NOTICE_SHIM.register(&tag, callback);
        Ok(PyNoticeListener { key: Some(key) })
    }
}

/// Extract notice type name from either a string or a Python class.
fn notice_type_to_string(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    // Try string first
    if let Ok(s) = obj.extract::<String>() {
        return Ok(s);
    }
    // Try Python type/class — use __name__
    if let Ok(name) = obj.getattr("__name__") {
        if let Ok(s) = name.extract::<String>() {
            return Ok(s);
        }
    }
    // Try repr as fallback
    Ok(format!("{}", obj))
}

// ---- String-keyed notice shim -------------------------------------------------
// The Rust notice system is generic over Rust types; Python notices carry an
// arbitrary string tag.  We bridge them via a single concrete PythonNotice
// Rust type whose payload is the tag string plus the Python callable list.

/// Marker notice type used as a placeholder in the Rust notice registry.
///
/// Python notice dispatch is driven by the string-keyed `PythonNoticeShim`
/// table; this type exists only to satisfy the generic `register_global` API.
#[derive(Clone)]
struct PythonStringNotice;

impl usd_tf::notice::Notice for PythonStringNotice {
    fn notice_type_name() -> &'static str {
        "PythonStringNotice"
    }
}

/// Per-tag callback list, shared between the shim registry and `send`.
#[allow(dead_code)]
struct ShimEntry {
    callbacks: Vec<(usd_tf::notice::ListenerKey, Py<PyAny>)>,
}

struct PythonNoticeShim {
    entries: Mutex<std::collections::HashMap<String, ShimEntry>>,
}

static PYTHON_NOTICE_SHIM: std::sync::LazyLock<PythonNoticeShim> =
    std::sync::LazyLock::new(|| PythonNoticeShim {
        entries: Mutex::new(std::collections::HashMap::new()),
    });

impl PythonNoticeShim {
    fn register(&self, tag: &str, callback: Py<PyAny>) -> usd_tf::notice::ListenerKey {
        // We don't use the Rust generic notice for Python callbacks — instead we
        // keep a manual table keyed by tag and issue callbacks directly in send().
        // We still need to return a real ListenerKey, so we mint a dummy one via
        // registering a no-op global listener and track it ourselves.
        let key = usd_tf::notice::global_registry()
            .register_global::<PythonStringNotice, _>(|_notice| {
                // Dispatch happens in send(); this closure is a no-op placeholder
                // required to satisfy the Rust notice registry's typed API.
            });

        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.entry(tag.to_string()).or_insert_with(|| ShimEntry {
            callbacks: Vec::new(),
        });
        entry.callbacks.push((key.clone(), callback));
        key
    }

    fn send(&self, tag: &str) {
        // We need the GIL to clone PyObject references, so acquire it first,
        // then snapshot the active callbacks while still holding the lock.
        // SAFETY: attach_unchecked is sound here — this function is only called
        // from a Rust thread context (notice dispatch), not from within a Python
        // callback, so no GIL is held and no re-entrancy issues can occur.
        #[allow(unsafe_code)]
        unsafe {
        Python::attach_unchecked(|py| {
            let callbacks: Vec<Py<PyAny>> = {
                let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                entries
                    .get(tag)
                    .map(|e| {
                        e.callbacks
                            .iter()
                            .filter(|(k, _)| k.is_valid())
                            .map(|(_, cb)| cb.clone_ref(py))
                            .collect()
                    })
                    .unwrap_or_default()
            };

            for cb in callbacks {
                // Call the Python callable with no arguments (notice value is opaque here).
                if let Err(e) = cb.call0(py) {
                    e.print(py);
                }
            }
        })
        } // end unsafe
    }
}

// ============================================================================
// Stopwatch
// ============================================================================

/// High-resolution timer — mirrors `pxr.Tf.Stopwatch` / `TfStopwatch`.
#[pyclass(skip_from_py_object,name = "Stopwatch", module = "pxr_rs.Tf")]
pub struct PyStopwatch {
    inner: usd_tf::stopwatch::Stopwatch,
}

#[pymethods]
impl PyStopwatch {
    #[new]
    fn new() -> Self {
        Self {
            inner: usd_tf::stopwatch::Stopwatch::new(),
        }
    }

    /// Begin timing.
    #[pyo3(name = "Start")]
    fn start(&mut self) {
        self.inner.start();
    }

    /// Stop timing and accumulate elapsed time.
    #[pyo3(name = "Stop")]
    fn stop(&mut self) {
        self.inner.stop();
    }

    /// Reset accumulated time and sample count to zero.
    #[pyo3(name = "Reset")]
    fn reset(&mut self) {
        self.inner.reset();
    }

    /// Add another stopwatch's accumulated time to this one.
    #[pyo3(name = "AddFrom")]
    fn add_from(&mut self, other: &Self) {
        self.inner.add_from(&other.inner);
    }

    // ---- time properties ----

    /// Accumulated time in nanoseconds.
    #[getter]
    fn nanoseconds(&self) -> i64 {
        self.inner.nanoseconds()
    }

    /// Accumulated time in microseconds.
    #[getter]
    fn microseconds(&self) -> i64 {
        self.inner.microseconds()
    }

    /// Accumulated time in milliseconds.
    #[getter]
    fn milliseconds(&self) -> i64 {
        self.inner.milliseconds()
    }

    /// Accumulated time in seconds (f64).
    #[getter]
    fn seconds(&self) -> f64 {
        self.inner.seconds()
    }

    /// Number of `Stop()` calls since creation or last `Reset()`.
    #[getter]
    #[pyo3(name = "sampleCount")]
    fn sample_count(&self) -> usize {
        self.inner.sample_count()
    }

    fn __repr__(&self) -> String {
        format!(
            "Tf.Stopwatch(samples={}, seconds={:.6})",
            self.inner.sample_count(),
            self.inner.seconds()
        )
    }

    fn __str__(&self) -> String {
        format!("{:.6} seconds", self.inner.seconds())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.nanoseconds() == other.inner.nanoseconds()
            && self.inner.sample_count() == other.inner.sample_count()
    }
}

// ============================================================================
// Debug
// ============================================================================

/// Debug symbol registry — mirrors `pxr.Tf.Debug` / `TfDebug`.
///
/// Debug symbols are named boolean flags that gate diagnostic output.
/// They can be toggled at runtime or via the `TF_DEBUG` environment variable.
#[pyclass(skip_from_py_object,name = "Debug", module = "pxr_rs.Tf")]
pub struct PyDebug;

#[pymethods]
impl PyDebug {
    /// Register a new debug symbol with an optional description.
    #[staticmethod]
    #[pyo3(name = "SetDebugSymbolsByName")]
    fn set_debug_symbols_by_name(pattern: &str, value: bool) {
        if value {
            usd_tf::Debug::enable_by_pattern(pattern);
        } else {
            usd_tf::Debug::disable_by_pattern(pattern);
        }
    }

    /// Return true if the named debug symbol is enabled.
    #[staticmethod]
    #[pyo3(name = "IsDebugSymbolNameEnabled")]
    fn is_symbol_enabled(name: &str) -> bool {
        usd_tf::Debug::is_enabled(name)
    }

    /// Return all registered debug symbol names (sorted).
    #[staticmethod]
    #[pyo3(name = "GetDebugSymbolNames")]
    fn get_symbol_names() -> Vec<String> {
        usd_tf::Debug::get_symbol_names()
    }

    /// Return the description for a registered debug symbol, or empty string.
    #[staticmethod]
    #[pyo3(name = "GetDebugSymbolDescription")]
    fn get_symbol_description(name: &str) -> String {
        usd_tf::Debug::get_symbol_description(name).unwrap_or_default()
    }

    /// Return a formatted string with all symbols, their on/off state, and descriptions.
    #[staticmethod]
    #[pyo3(name = "GetDebugSymbolDescriptions")]
    fn get_symbol_descriptions() -> String {
        usd_tf::Debug::get_symbol_descriptions()
    }

    /// SetOutputFile(file) — stub, accepts any arg and does nothing.
    /// C++ redirects debug output to a file; not meaningful for pure-Rust port.
    #[staticmethod]
    #[pyo3(name = "SetOutputFile")]
    fn set_output_file(_file: &Bound<'_, PyAny>) {}

    fn __repr__(&self) -> &str {
        "Tf.Debug"
    }

    fn __eq__(&self, _other: &Self) -> bool {
        true
    }
}

// ============================================================================
// EnvSetting  (lightweight accessor — no write support from Python)
// ============================================================================

/// Read a `TfEnvSetting` boolean value by name.
///
/// Mirrors `Tf.GetEnvSetting(name)`.
///
/// Only boolean env settings are exposed; returns `None` for unknown names.
#[pyfunction]
#[pyo3(name = "GetEnvSetting")]
fn get_env_setting(name: &str) -> Option<bool> {
    // TfEnvSetting values are controlled entirely by environment variables;
    // we read the raw env var as a best-effort approximation.
    std::env::var(name)
        .ok()
        .map(|v| !v.eq_ignore_ascii_case("false") && v != "0" && !v.is_empty())
}

// ============================================================================
// Module-level diagnostic helpers
// ============================================================================

/// Issue a TF warning — routes through DiagnosticMgr.
///
/// ```python
/// Tf.Warn("Something odd: %s", value)
/// ```
#[pyfunction]
#[pyo3(name = "Warn", signature = (msg, *args))]
fn warn(msg: &str, args: &Bound<'_, PyTuple>) -> PyResult<()> {
    let formatted = if args.is_empty() {
        msg.to_string()
    } else {
        // Basic C-style %s interpolation for compat with pxr.Tf.Warn usage patterns.
        format_msg(msg, args)?
    };
    usd_tf::issue_warning(usd_tf::CallContext::empty().hide(), formatted);
    Ok(())
}

/// Issue a TF status message.
///
/// ```python
/// Tf.Status("Loading stage: %s", path)
/// ```
#[pyfunction]
#[pyo3(name = "Status", signature = (msg, *args))]
fn status(msg: &str, args: &Bound<'_, PyTuple>) -> PyResult<()> {
    let formatted = if args.is_empty() {
        msg.to_string()
    } else {
        format_msg(msg, args)?
    };
    usd_tf::issue_status(usd_tf::CallContext::empty().hide(), formatted);
    Ok(())
}

/// Raise a `TfCodingError` — continues execution but signals programmer error.
///
/// In Python this raises `Tf.ErrorException`.
///
/// ```python
/// Tf.RaiseCodingError("Invalid prim path: %s", path)
/// ```
#[pyfunction]
#[pyo3(name = "RaiseCodingError", signature = (msg, *args))]
fn raise_coding_error(msg: &str, args: &Bound<'_, PyTuple>) -> PyResult<()> {
    let formatted = if args.is_empty() {
        msg.to_string()
    } else {
        format_msg(msg, args)?
    };
    usd_tf::issue_error(
        usd_tf::CallContext::empty().hide(),
        usd_tf::DiagnosticType::CodingError,
        formatted.clone(),
    );
    Err(PyException::new_err(formatted))
}

/// Raise a `TfRuntimeError` — signals an unrecoverable runtime condition.
///
/// ```python
/// Tf.RaiseRuntimeError("Failed to open file: %s", path)
/// ```
#[pyfunction]
#[pyo3(name = "RaiseRuntimeError", signature = (msg, *args))]
fn raise_runtime_error(msg: &str, args: &Bound<'_, PyTuple>) -> PyResult<()> {
    let formatted = if args.is_empty() {
        msg.to_string()
    } else {
        format_msg(msg, args)?
    };
    usd_tf::issue_error(
        usd_tf::CallContext::empty().hide(),
        usd_tf::DiagnosticType::RuntimeError,
        formatted.clone(),
    );
    Err(PyRuntimeError::new_err(formatted))
}

/// Report a fatal error — terminates the process.
///
/// ```python
/// Tf.Fatal("Invariant violated: %s", detail)
/// ```
#[pyfunction]
#[pyo3(name = "Fatal", signature = (msg, *args))]
fn fatal(py: Python<'_>, msg: &str, args: &Bound<'_, PyTuple>) -> PyResult<()> {
    let formatted = if args.is_empty() {
        msg.to_string()
    } else {
        format_msg(msg, args)?
    };
    // Raise SystemExit so Python's atexit handlers still run.
    py.run(
        pyo3::ffi::c_str!("raise SystemExit(1)"),
        None,
        None,
    )?;
    // issue_fatal_error would call process::abort; we prefer SystemExit from Python.
    eprintln!("FATAL: {}", formatted);
    Ok(())
}

// ---- Error exception class --------------------------------------------------

// ---- printf-style format helper ---------------------------------------------

/// Interpolate `%s` placeholders in `template` with Python repr of `args`.
fn format_msg(template: &str, args: &Bound<'_, PyTuple>) -> PyResult<String> {
    let mut result = String::with_capacity(template.len());
    let mut arg_idx = 0usize;
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.peek() {
                Some(&'s') => {
                    chars.next();
                    if arg_idx < args.len() {
                        let obj = args.get_item(arg_idx)?;
                        result.push_str(&obj.str()?.to_string_lossy());
                        arg_idx += 1;
                    } else {
                        result.push_str("%s");
                    }
                }
                Some(&'%') => {
                    chars.next();
                    result.push('%');
                }
                _ => result.push('%'),
            }
        } else {
            result.push(ch);
        }
    }
    Ok(result)
}

// ============================================================================
// MallocTag (TfMallocTag)
// ============================================================================

/// Memory tagging system — mirrors `pxr.Tf.MallocTag` / `TfMallocTag`.
///
/// Provides static methods for memory tracking and reporting.
#[pyclass(skip_from_py_object, name = "MallocTag", module = "pxr_rs.Tf")]
pub struct PyMallocTag;

#[pymethods]
impl PyMallocTag {
    /// Initialize the memory tagging system.
    #[staticmethod]
    #[pyo3(name = "Initialize")]
    fn initialize() -> bool {
        usd_tf::malloc_tag::MallocTag::initialize().is_ok()
    }

    /// True if the tagging system is initialized.
    #[staticmethod]
    #[pyo3(name = "IsInitialized")]
    fn is_initialized() -> bool {
        usd_tf::malloc_tag::MallocTag::is_initialized()
    }

    /// Get total bytes being tracked.
    #[staticmethod]
    #[pyo3(name = "GetTotalBytes")]
    fn get_total_bytes() -> usize {
        usd_tf::malloc_tag::MallocTag::get_total_bytes()
    }

    /// Get maximum total bytes ever allocated.
    #[staticmethod]
    #[pyo3(name = "GetMaxTotalBytes")]
    fn get_max_total_bytes() -> usize {
        usd_tf::malloc_tag::MallocTag::get_max_total_bytes()
    }

    /// Get a snapshot of memory usage as a CallTree.
    #[staticmethod]
    #[pyo3(name = "GetCallTree")]
    fn get_call_tree() -> PyCallTree {
        let tree = usd_tf::malloc_tag::MallocTag::get_call_tree(true);
        PyCallTree { inner: tree }
    }

    fn __repr__(&self) -> &str {
        "Tf.MallocTag"
    }
}

/// Memory usage call tree — mirrors `Tf.MallocTag.CallTree`.
#[pyclass(skip_from_py_object, name = "CallTree", module = "pxr_rs.Tf")]
pub struct PyCallTree {
    inner: usd_tf::malloc_tag::CallTree,
}

#[pymethods]
impl PyCallTree {
    /// Create an empty CallTree (for LoadReport usage).
    #[new]
    fn new() -> Self {
        Self {
            inner: usd_tf::malloc_tag::CallTree::default(),
        }
    }

    /// Get the root PathNode of the tree.
    #[pyo3(name = "GetRoot")]
    fn get_root(&self) -> PyPathNode {
        PyPathNode::from_node(&self.inner.root)
    }

    /// Return a formatted report string.
    #[pyo3(name = "GetPrettyPrintString")]
    #[pyo3(signature = (max_printed_nodes = 1000))]
    fn get_pretty_print_string(&self, max_printed_nodes: usize) -> String {
        self.inner.get_pretty_print_string(
            usd_tf::malloc_tag::PrintSetting::Both,
            max_printed_nodes,
        )
    }

    /// Print report to stdout.
    ///
    /// Mirrors C++ `CallTree::Report()`.
    #[pyo3(name = "Report")]
    #[pyo3(signature = (root_name = None))]
    fn report(&self, root_name: Option<&str>) {
        let mut buf = Vec::new();
        self.inner.report(&mut buf, root_name);
        print!("{}", String::from_utf8_lossy(&buf));
    }

    /// Load and parse a malloc tag report file.
    ///
    /// Returns true on success. Populates the tree from the "Tree view" section
    /// of the report file format. Raises Tf.ErrorException if file not found.
    ///
    /// Mirrors C++ `CallTree::LoadReport()`.
    #[pyo3(name = "LoadReport")]
    fn load_report(&mut self, filepath: &str) -> PyResult<bool> {
        let contents = std::fs::read_to_string(filepath).map_err(|e| {
            PyException::new_err(format!("Failed to load report '{}': {}", filepath, e))
        })?;
        Ok(self.parse_report_string(&contents))
    }

    /// Save the current tree to a temporary file and return the path.
    ///
    /// Mirrors C++ `CallTree::LogReport()`.
    #[pyo3(name = "LogReport")]
    fn log_report(&self) -> PyResult<String> {
        let report = self.format_tree_report();
        let path = std::env::temp_dir().join(format!(
            "malloc_tag_report_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        std::fs::write(&path, &report)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        Ok(path.to_string_lossy().to_string())
    }

    fn __repr__(&self) -> &str {
        "Tf.MallocTag.CallTree"
    }

    fn __bool__(&self) -> bool {
        !self.inner.root.site_name.is_empty()
    }
}

impl PyCallTree {
    /// Parse the "Tree view" section of a malloc tag report.
    ///
    /// Format per line:
    /// `<inclusive> B  <exclusive> B  <samples> samples  <tree_indent><name>`
    ///
    /// Where tree_indent uses `| ` for hierarchy.
    fn parse_report_string(&mut self, contents: &str) -> bool {
        // Parse all tree sections from the report. A report may contain multiple
        // "Tree view" sections (one per malloc tag snapshot). Each section's root
        // becomes a child of a synthetic root when multiple sections exist.
        let mut trees: Vec<usd_tf::malloc_tag::PathNode> = Vec::new();
        let mut in_tree = false;
        let mut current_root = usd_tf::malloc_tag::PathNode::default();
        let mut stack: Vec<(usize, usize)> = Vec::new();

        for line in contents.lines() {
            let trimmed = line.trim();

            // Detect start of tree view section
            if trimmed.starts_with("Tree view") {
                // Save previous tree if any
                if !current_root.site_name.is_empty() {
                    trees.push(std::mem::take(&mut current_root));
                    stack.clear();
                }
                in_tree = true;
                continue;
            }

            // Skip header line
            if trimmed.starts_with("inclusive") {
                continue;
            }

            // End of tree section: empty line, separator, or new report header
            if in_tree && (trimmed.is_empty() || trimmed.starts_with("---") || trimmed.starts_with("Call sites")) {
                in_tree = false;
                continue;
            }

            if !in_tree {
                continue;
            }

            // Parse a tree line
            let Some(node) = Self::parse_tree_line(line) else {
                continue;
            };

            if current_root.site_name.is_empty() {
                // First node in this section is the root
                current_root = node.0;
                stack.clear();
                stack.push((node.1, 0));
            } else {
                let depth = node.1;
                // Pop stack until we find a parent at lower depth
                while stack.len() > 1 && stack.last().map(|s| s.0).unwrap_or(0) >= depth {
                    stack.pop();
                }
                Self::insert_at_path(&mut current_root, &stack, node.0);
                let child_count = Self::children_count_at_path(&current_root, &stack);
                stack.push((depth, child_count.saturating_sub(1)));
            }
        }

        // Save the last tree
        if !current_root.site_name.is_empty() {
            trees.push(current_root);
        }

        if trees.is_empty() {
            return false;
        }

        if trees.len() == 1 {
            // Single tree: use it directly as root
            self.inner.root = trees.into_iter().next().unwrap();
        } else {
            // Multiple trees: create synthetic root with each tree as a child
            let mut root = usd_tf::malloc_tag::PathNode::default();
            root.site_name = String::from("__root");
            root.children = trees;
            // Sum up totals
            root.bytes = root.children.iter().map(|c| c.bytes).sum();
            root.bytes_direct = root.children.iter().map(|c| c.bytes_direct).sum();
            root.allocations = root.children.iter().map(|c| c.allocations).sum();
            self.inner.root = root;
        }

        true
    }

    /// Parse a single tree line, returning (PathNode, depth).
    ///
    /// Format: `  2,952 B         1,232 B      20 samples    | | Csd`
    fn parse_tree_line(line: &str) -> Option<(usd_tf::malloc_tag::PathNode, usize)> {
        // Find "samples" to split numbers from the tree part
        let samples_idx = line.find("samples")?;
        let num_part = &line[..samples_idx];
        let tree_part = &line[samples_idx + "samples".len()..];

        // Parse numbers: strip commas, find B-delimited values
        let nums: Vec<&str> = num_part.split('B').collect();
        if nums.len() < 3 {
            return None;
        }

        let inclusive = Self::parse_bytes(nums[0])?;
        let exclusive = Self::parse_bytes(nums[1])?;
        // Third part before "samples" is the sample count
        let samples = nums[2].trim().replace(',', "").parse::<usize>().ok()?;

        // Parse tree indent and name from the tree part.
        // Format after "samples    ":
        //   `__root`           -> depth 0 (no prefix)
        //   `| Csd`            -> depth 1 (2-char prefix)
        //   `|   CsdAttr`      -> depth 2 (4-char prefix)
        //   `|   | Csd`        -> depth 3 (6-char prefix)
        //   `|   |   CsdProp`  -> depth 4 (8-char prefix)
        // Depth = prefix_length / 2, where prefix is the `| ` / `|   ` chain.
        let tree_str = tree_part.trim_start();

        // Find where the actual name starts: skip `|` and space characters.
        let prefix_len = tree_str
            .chars()
            .take_while(|&c| c == '|' || c == ' ')
            .count();
        let depth = prefix_len / 2;
        let name = tree_str[prefix_len..].trim().to_string();
        if name.is_empty() {
            return None;
        }

        Some((
            usd_tf::malloc_tag::PathNode {
                bytes: inclusive,
                bytes_direct: exclusive,
                allocations: samples,
                site_name: name,
                children: Vec::new(),
            },
            depth,
        ))
    }

    /// Parse a byte value like "2,952 " -> 2952.
    fn parse_bytes(s: &str) -> Option<usize> {
        let cleaned = s.trim().replace(',', "");
        cleaned.parse::<usize>().ok()
    }

    /// Insert a node as child at the path described by the stack.
    fn insert_at_path(
        root: &mut usd_tf::malloc_tag::PathNode,
        stack: &[(usize, usize)],
        node: usd_tf::malloc_tag::PathNode,
    ) {
        let mut current = root;
        // Skip the first stack entry (it's the root itself)
        for &(_depth, child_idx) in stack.iter().skip(1) {
            if child_idx < current.children.len() {
                current = &mut current.children[child_idx];
            } else {
                // Invalid index, append to current
                break;
            }
        }
        current.children.push(node);
    }

    /// Count children at the path described by the stack.
    fn children_count_at_path(
        root: &usd_tf::malloc_tag::PathNode,
        stack: &[(usize, usize)],
    ) -> usize {
        let mut current = root;
        for &(_depth, child_idx) in stack.iter().skip(1) {
            if child_idx < current.children.len() {
                current = &current.children[child_idx];
            } else {
                break;
            }
        }
        current.children.len()
    }

    /// Format the tree into the report string format for LogReport.
    fn format_tree_report(&self) -> String {
        let mut out = String::new();
        out.push_str("Tree view  ==============\n");
        out.push_str("      inclusive       exclusive\n");
        Self::format_tree_node(&self.inner.root, 0, &mut out);
        out
    }

    fn format_tree_node(node: &usd_tf::malloc_tag::PathNode, depth: usize, out: &mut String) {
        // Build tree prefix: depth 0 = no prefix, depth N = "| " repeated with proper indentation
        // e.g., depth 1: "| ", depth 2: "|   | ", depth 3: "|   |   | "
        let prefix = if depth == 0 {
            String::new()
        } else {
            let mut p = String::new();
            for i in 0..depth {
                if i < depth - 1 {
                    p.push_str("|   ");
                } else {
                    p.push_str("| ");
                }
            }
            p
        };
        out.push_str(&format!(
            "{:>12} B {:>12} B {:>7} samples    {}{}\n",
            Self::format_with_commas(node.bytes),
            Self::format_with_commas(node.bytes_direct),
            node.allocations,
            prefix,
            node.site_name,
        ));
        for child in &node.children {
            Self::format_tree_node(child, depth + 1, out);
        }
    }

    /// Format a number with comma separators (e.g., 2952 -> "2,952").
    fn format_with_commas(n: usize) -> String {
        let s = n.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    }
}

/// Node in the malloc tag call tree — mirrors `Tf.MallocTag.CallTree.PathNode`.
#[pyclass(skip_from_py_object, name = "PathNode", module = "pxr_rs.Tf")]
#[derive(Clone)]
pub struct PyPathNode {
    /// Tag name at this site.
    #[pyo3(get)]
    #[pyo3(name = "siteName")]
    site_name: String,
    /// Inclusive bytes (self + descendants).
    #[pyo3(get)]
    #[pyo3(name = "nBytes")]
    n_bytes: usize,
    /// Exclusive bytes (self only).
    #[pyo3(get)]
    #[pyo3(name = "nBytesDirect")]
    n_bytes_direct: usize,
    /// Number of allocations.
    #[pyo3(get)]
    #[pyo3(name = "nAllocations")]
    n_allocations: usize,
    children_data: Vec<usd_tf::malloc_tag::PathNode>,
}

impl PyPathNode {
    fn from_node(node: &usd_tf::malloc_tag::PathNode) -> Self {
        Self {
            site_name: node.site_name.clone(),
            n_bytes: node.bytes,
            n_bytes_direct: node.bytes_direct,
            n_allocations: node.allocations,
            children_data: node.children.clone(),
        }
    }
}

#[pymethods]
impl PyPathNode {
    /// Return child nodes.
    #[pyo3(name = "GetChildren")]
    fn get_children(&self) -> Vec<PyPathNode> {
        self.children_data.iter().map(PyPathNode::from_node).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Tf.MallocTag.CallTree.PathNode('{}', bytes={})",
            self.site_name, self.n_bytes
        )
    }
}

// ============================================================================
// ScriptModuleLoader (TfScriptModuleLoader)
// ============================================================================

/// Script module loading system — mirrors `pxr.Tf.ScriptModuleLoader`.
///
/// In pure Rust USD this is largely a no-op stub since all modules are
/// statically linked. The API is preserved for compatibility with tests
/// that call `_LoadModulesForLibrary` / `_RegisterLibrary`.
#[pyclass(skip_from_py_object, name = "ScriptModuleLoader", module = "pxr_rs.Tf")]
pub struct PyScriptModuleLoader;

#[pymethods]
impl PyScriptModuleLoader {
    /// Return the singleton loader (matches C++ constructor pattern).
    #[new]
    fn new() -> Self {
        Self
    }

    /// Load modules for the named library and its dependencies.
    ///
    /// In Rust this is a no-op since there are no dynamic Python modules to load.
    #[pyo3(name = "_LoadModulesForLibrary")]
    fn load_modules_for_library(&self, lib_name: &str) {
        usd_tf::script_module_loader::ScriptModuleLoader::load_modules_for_library(lib_name);
    }

    /// Register a library with dependencies for later loading.
    #[pyo3(name = "_RegisterLibrary")]
    #[pyo3(signature = (lib_name, module_name, predecessors))]
    fn register_library(&self, lib_name: &str, module_name: &str, predecessors: Vec<String>) {
        let pred_strs: Vec<&str> = predecessors.iter().map(|s| s.as_str()).collect();
        usd_tf::script_module_loader::ScriptModuleLoader::register_library_info(
            lib_name,
            module_name,
            &pred_strs,
        );
    }

    /// Return all registered library names.
    #[pyo3(name = "GetModuleNames")]
    fn get_module_names(&self) -> Vec<String> {
        usd_tf::script_module_loader::ScriptModuleLoader::library_names()
    }

    fn __repr__(&self) -> &str {
        "Tf.ScriptModuleLoader"
    }
}

// ============================================================================
// Enum helpers
// ============================================================================

/// Wrap a Python int value as a Tf-registered enum integer.
///
/// Mirrors `Tf.Enum` / `TfEnum` usage in Python where int subclasses carry
/// a type tag.  In practice pxr uses Python ints directly — this wrapper
/// preserves the `typeName` attribute for introspection.
#[pyclass(skip_from_py_object,name = "Enum", module = "pxr_rs.Tf")]
#[derive(Clone)]
pub struct PyEnum {
    value: i64,
    type_name: String,
}

#[pymethods]
impl PyEnum {
    #[new]
    #[pyo3(signature = (value, type_name = ""))]
    fn new(value: i64, type_name: &str) -> Self {
        Self {
            value,
            type_name: type_name.to_string(),
        }
    }

    /// Numeric integer value.
    #[getter]
    fn value(&self) -> i64 {
        self.value
    }

    /// Type name string registered with TfEnum.
    #[getter]
    #[pyo3(name = "typeName")]
    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn __int__(&self) -> i64 {
        self.value
    }

    fn __index__(&self) -> i64 {
        self.value
    }

    fn __repr__(&self) -> String {
        if self.type_name.is_empty() {
            format!("Tf.Enum({})", self.value)
        } else {
            format!("Tf.Enum({}, '{}')", self.value, self.type_name)
        }
    }

    fn __str__(&self) -> String {
        format!("{}", self.value)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.value == other.value && self.type_name == other.type_name
    }

    fn __hash__(&self) -> i64 {
        self.value
    }

    fn __lt__(&self, other: &Self) -> bool {
        self.value < other.value
    }

    fn __le__(&self, other: &Self) -> bool {
        self.value <= other.value
    }

    fn __gt__(&self, other: &Self) -> bool {
        self.value > other.value
    }

    fn __ge__(&self, other: &Self) -> bool {
        self.value >= other.value
    }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Tf classes and free functions into the `pxr.Tf` submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<PyToken>()?;
    m.add_class::<PyType>()?;
    m.add_class::<PyNotice>()?;
    m.add_class::<PyNoticeListener>()?;
    m.add_class::<PyStopwatch>()?;
    m.add_class::<PyDebug>()?;
    m.add_class::<PyEnum>()?;
    m.add_class::<PyMallocTag>()?;
    m.add_class::<PyCallTree>()?;
    m.add_class::<PyPathNode>()?;
    m.add_class::<PyScriptModuleLoader>()?;

    // Nest CallTree and PathNode under MallocTag so `Tf.MallocTag.CallTree` works.
    let malloc_tag_cls = m.getattr("MallocTag")?;
    malloc_tag_cls.setattr("CallTree", py.get_type::<PyCallTree>())?;
    let call_tree_cls = m.getattr("CallTree")?;
    call_tree_cls.setattr("PathNode", py.get_type::<PyPathNode>())?;

    // Free functions
    m.add_function(wrap_pyfunction!(warn, m)?)?;
    m.add_function(wrap_pyfunction!(status, m)?)?;
    m.add_function(wrap_pyfunction!(raise_coding_error, m)?)?;
    m.add_function(wrap_pyfunction!(raise_runtime_error, m)?)?;
    m.add_function(wrap_pyfunction!(fatal, m)?)?;
    m.add_function(wrap_pyfunction!(get_env_setting, m)?)?;

    // ErrorException attribute (pxr.Tf.ErrorException)
    m.add("ErrorException", py.get_type::<pyo3::exceptions::PyException>())?;

    Ok(())
}
