//! pxr.Trace — performance tracing (usd-trace).
//!
//! Parity target: OpenUSD `pxr/base/trace` Python API used by `testTrace.py`.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use usd_trace::AggregateNode;
use usd_trace::{Collector, Reporter};

static GLOBAL_REPORTER: OnceLock<Py<PyReporter>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

#[pyclass(name = "Collector", module = "pxr.Trace")]
pub struct PyCollector;

#[pymethods]
impl PyCollector {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.cast::<PyCollector>().is_ok()
    }

    fn __hash__(&self) -> isize {
        0x5452_4143u32 as isize
    }

    #[getter]
    fn enabled(&self) -> bool {
        Collector::is_enabled()
    }

    #[setter]
    fn set_enabled(&self, value: bool) {
        Collector::get_instance().set_enabled(value);
    }

    #[pyo3(name = "BeginEvent")]
    fn begin_event(&self, key: &str) -> u64 {
        Collector::get_instance().begin_event(key)
    }

    #[pyo3(name = "EndEvent")]
    fn end_event(&self, key: &str) -> u64 {
        Collector::get_instance().end_event(key)
    }

    #[pyo3(name = "Clear")]
    fn clear(&self) {
        Collector::get_instance().clear();
    }
}

// ---------------------------------------------------------------------------
// Aggregate tree node (Python tests expect `.key`, times, `.children`, …)
// ---------------------------------------------------------------------------

#[pyclass(name = "EventNode", module = "pxr.Trace")]
pub struct PyEventNode {
    inner: Arc<AggregateNode>,
    expanded: AtomicBool,
}

impl PyEventNode {
    fn from_aggregate(py: Python<'_>, node: &Arc<AggregateNode>) -> PyResult<Py<PyEventNode>> {
        Py::new(
            py,
            PyEventNode {
                inner: Arc::clone(node),
                expanded: AtomicBool::new(node.is_expanded()),
            },
        )
    }
}

#[pymethods]
impl PyEventNode {
    #[getter]
    fn key(&self) -> String {
        self.inner.key().to_string()
    }

    #[getter]
    fn exclusiveTime(&self) -> f64 {
        self.inner.exclusive_time()
    }

    #[getter]
    fn inclusiveTime(&self) -> f64 {
        self.inner.inclusive_time()
    }

    #[getter]
    fn count(&self) -> u64 {
        self.inner.count()
    }

    #[getter]
    fn expanded(&self) -> bool {
        self.expanded.load(Ordering::Relaxed)
    }

    #[setter]
    fn set_expanded(&self, value: bool) {
        self.expanded.store(value, Ordering::Relaxed);
    }

    #[getter]
    fn children(&self, py: Python<'_>) -> PyResult<Vec<Py<PyEventNode>>> {
        let mut out = Vec::new();
        for c in self.inner.children() {
            out.push(PyEventNode::from_aggregate(py, c)?);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Reporter
// ---------------------------------------------------------------------------

#[pyclass(name = "Reporter", module = "pxr.Trace")]
pub struct PyReporter {
    inner: Mutex<Reporter>,
}

impl PyReporter {
    fn new_reporter(label: &str) -> Self {
        Self {
            inner: Mutex::new(Reporter::new(label)),
        }
    }
}

#[pymethods]
impl PyReporter {
    #[new]
    #[pyo3(signature = (label = ""))]
    fn ctor(label: &str) -> Self {
        Self::new_reporter(label)
    }

    #[pyo3(name = "GetLabel")]
    fn get_label(&self) -> String {
        self.inner
            .lock()
            .map_or_else(|_| String::new(), |g| g.get_label().to_string())
    }

    #[getter]
    fn shouldAdjustForOverheadAndNoise(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.should_adjust_for_overhead_and_noise())
            .unwrap_or(true)
    }

    #[setter]
    fn set_shouldAdjustForOverheadAndNoise(&self, value: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.set_should_adjust_for_overhead_and_noise(value);
        }
    }

    #[pyo3(name = "UpdateTraceTrees")]
    fn update_trace_trees(&self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Trace reporter mutex poisoned")
            })?
            .update_trace_trees();
        Ok(())
    }

    #[pyo3(name = "ClearTree")]
    fn clear_tree(&self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Trace reporter mutex poisoned")
            })?
            .clear_tree();
        Ok(())
    }

    #[getter]
    fn aggregateTreeRoot(&self, py: Python<'_>) -> PyResult<Option<Py<PyEventNode>>> {
        let guard = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Trace reporter mutex poisoned")
        })?;
        Ok(match guard.get_aggregate_tree_root() {
            Some(root) => Some(PyEventNode::from_aggregate(py, root)?),
            None => None,
        })
    }

    #[pyo3(name = "Report", signature = (iteration_count=None))]
    fn report(&self, py: Python<'_>, iteration_count: Option<u32>) -> PyResult<()> {
        let text = self
            .inner
            .lock()
            .map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Trace reporter mutex poisoned")
            })?
            .report(iteration_count.unwrap_or(0));
        py.import("builtins")?
            .getattr("print")?
            .call1((text,))?;
        Ok(())
    }

    #[pyo3(name = "ReportTimes")]
    fn report_times(&self, py: Python<'_>) -> PyResult<()> {
        let text = self
            .inner
            .lock()
            .map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Trace reporter mutex poisoned")
            })?
            .report_times();
        py.import("builtins")?
            .getattr("print")?
            .call1((text,))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Module-level helpers (OpenUSD test hooks)
// ---------------------------------------------------------------------------

#[pyfunction]
#[pyo3(name = "TestAuto")]
fn trace_test_auto() {
    let c = Collector::get_instance();
    c.begin_event("TestAuto");
    c.end_event("TestAuto");
}

#[pyfunction]
#[pyo3(name = "TestNesting")]
fn trace_test_nesting() {
    let c = Collector::get_instance();
    c.begin_event("TestNesting");
    c.begin_event("Inner");
    c.end_event("Inner");
    c.end_event("TestNesting");
}

static TEST_EVENT_NAME: Mutex<String> = Mutex::new(String::new());

#[pyfunction]
#[pyo3(name = "TestCreateEvents")]
fn trace_test_create_events() -> PyResult<()> {
    let mut g = TEST_EVENT_NAME
        .lock()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("mutex poisoned"))?;
    *g = "PYTHON_EVENT".to_string();
    Ok(())
}

#[pyfunction]
#[pyo3(name = "GetTestEventName")]
fn get_test_event_name() -> PyResult<String> {
    let g = TEST_EVENT_NAME
        .lock()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("mutex poisoned"))?;
    if g.is_empty() {
        Ok("PYTHON_EVENT".to_string())
    } else {
        Ok(g.clone())
    }
}

#[pyfunction]
#[pyo3(name = "GetElapsedSeconds")]
fn get_elapsed_seconds(begin: u64, end: u64) -> f64 {
    if end > begin {
        (end - begin) as f64 / 1_000_000_000.0
    } else {
        0.0
    }
}

#[pyfunction]
#[pyo3(name = "PythonGarbageCollectionCallback")]
fn python_gc_callback(_py: Python<'_>, phase: &str, _info: &Bound<'_, PyAny>) {
    let c = Collector::get_instance();
    let key = format!("Python Garbage Collection ({phase})");
    c.begin_event(&key);
    c.end_event(&key);
}

fn install_decorators(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let dict = PyDict::new(py);
    dict.set_item("Collector", m.getattr("Collector")?)?;
    let code = CString::new(
        r"
def TraceFunction(f):
    def wrapper(*a, **kw):
        c = Collector()
        name = getattr(f, '__name__', 'trace')
        c.BeginEvent(name)
        try:
            return f(*a, **kw)
        finally:
            c.EndEvent(name)
    try:
        wrapper.__name__ = getattr(f, '__name__', 'trace')
        wrapper.__doc__ = getattr(f, '__doc__', None)
    except Exception:
        pass
    return wrapper

def TraceMethod(f):
    return TraceFunction(f)
",
    )
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    py.run(&code, Some(&dict), Some(&dict))?;
    m.setattr("TraceFunction", dict.get_item("TraceFunction")?.expect("TraceFunction"))?;
    m.setattr("TraceMethod", dict.get_item("TraceMethod")?.expect("TraceMethod"))?;
    Ok(())
}

fn ensure_global_reporter(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    if GLOBAL_REPORTER.get().is_some() {
        return Ok(());
    }
    let rep = Py::new(py, PyReporter::new_reporter(""))?;
    let _ = GLOBAL_REPORTER.set(rep.clone_ref(py));
    let cls = m.getattr("Reporter")?;
    cls.setattr("globalReporter", rep.bind(py))?;
    m.setattr("_global_reporter_singleton", rep.bind(py))?;
    Ok(())
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCollector>()?;
    m.add_class::<PyReporter>()?;
    m.add_class::<PyEventNode>()?;

    m.add_function(wrap_pyfunction!(trace_test_auto, m)?)?;
    m.add_function(wrap_pyfunction!(trace_test_nesting, m)?)?;
    m.add_function(wrap_pyfunction!(trace_test_create_events, m)?)?;
    m.add_function(wrap_pyfunction!(get_test_event_name, m)?)?;
    m.add_function(wrap_pyfunction!(get_elapsed_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(python_gc_callback, m)?)?;

    install_decorators(py, m)?;
    ensure_global_reporter(py, m)?;
    Ok(())
}
