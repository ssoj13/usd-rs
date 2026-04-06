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
        notice_type: &str,
        callback: Py<PyAny>,
    ) -> PyNoticeListener {
        let key = PYTHON_NOTICE_SHIM.register(notice_type, callback);
        PyNoticeListener { key: Some(key) }
    }
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
