//! pxr.UsdUtils — native surface (backed by `usd-utils` crate).
//! Merged at import time with pure-Python helpers under `pxr.UsdUtils`.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::ffi::CString;
use usd_core::time_code::TimeCode;
use usd_core::stage_cache::StageCache as UsdStageCache;
use usd_utils::time_code_range::TimeCodeRange as Utcr;
use usd_utils::StageCache as UtilsStageCache;

use crate::usd::{PyStageCache, PyTimeCode};

// ---------------------------------------------------------------------------
// TimeCodeRange
// ---------------------------------------------------------------------------

#[pyclass(name = "TimeCodeRange", module = "pxr.UsdUtils")]
pub struct PyTimeCodeRange {
    inner: Utcr,
}

#[pymethods]
impl PyTimeCodeRange {
    #[new]
    #[pyo3(signature = (a=None, b=None, c=None))]
    fn new(a: Option<f64>, b: Option<f64>, c: Option<f64>) -> Self {
        let inner = match (a, b, c) {
            (None, None, None) => Utcr::default(),
            (Some(t), None, None) => Utcr::new_single(TimeCode::new(t)),
            (Some(s), Some(e), None) => Utcr::new(TimeCode::new(s), TimeCode::new(e)),
            (Some(s), Some(e), Some(stride)) => {
                Utcr::new_with_stride(TimeCode::new(s), TimeCode::new(e), stride)
            }
            _ => Utcr::default(),
        };
        Self { inner }
    }

    #[classmethod]
    #[pyo3(name = "CreateFromFrameSpec")]
    fn create_from_frame_spec(_cls: &Bound<'_, pyo3::types::PyType>, spec: &str) -> Self {
        Self {
            inner: Utcr::from_frame_spec(spec),
        }
    }

    #[getter]
    fn frameSpec(&self) -> String {
        if self.inner.is_empty() {
            return "NONE".to_string();
        }
        self.inner.to_string()
    }

    #[getter]
    fn startTimeCode(&self) -> f64 {
        self.inner.get_start_time_code().value()
    }

    #[getter]
    fn endTimeCode(&self) -> f64 {
        self.inner.get_end_time_code().value()
    }

    #[getter]
    fn stride(&self) -> f64 {
        self.inner.get_stride()
    }

    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_empty() {
            "UsdUtils.TimeCodeRange()".to_string()
        } else {
            format!(
                "UsdUtils.TimeCodeRange.CreateFromFrameSpec('{}')",
                self.frameSpec()
            )
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __iter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let mut items = Vec::new();
        for tc in slf.inner.iter() {
            let pytc = Py::new(py, PyTimeCode::from_usd_core(tc))?;
            items.push(pytc.into_any());
        }
        let list = pyo3::types::PyList::new(py, items)?;
        list
            .call_method0("__iter__")
            .map(|iter_bound| iter_bound.into_any().unbind())
    }
}

// ---------------------------------------------------------------------------
// StageCache (UsdUtils singleton — underlying `usd_utils::StageCache`)
// ---------------------------------------------------------------------------

#[pyclass(name = "StageCache", module = "pxr.UsdUtils")]
pub struct PyUsdUtilsStageCache {
    _inner: std::sync::Arc<UtilsStageCache>,
}

impl PyUsdUtilsStageCache {
    pub(crate) fn usd_cache_arc(&self) -> std::sync::Arc<UsdStageCache> {
        self._inner.usd_cache_arc()
    }
}

#[pymethods]
impl PyUsdUtilsStageCache {
    #[classmethod]
    #[pyo3(name = "Get")]
    fn get(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self {
            _inner: UtilsStageCache::get(),
        }
    }

    /// Same underlying data as [`Usd.StageCache`] for the process singleton (for `Usd.StageCacheContext`).
    #[pyo3(name = "GetUsdStageCache")]
    fn get_usd_stage_cache(&self, py: Python<'_>) -> PyResult<Py<PyStageCache>> {
        Py::new(py, PyStageCache::from_arc(self._inner.usd_cache_arc()))
    }

    fn __repr__(&self) -> &'static str {
        "UsdUtils.StageCache"
    }
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimeCodeRange>()?;
    m.add_class::<PyUsdUtilsStageCache>()?;

    // UsdUtils.TimeCodeRange.Tokens — OpenUSD exposes nested class with string constants.
    let dict = PyDict::new(py);
    let code = CString::new(
        r"
class _TcrTokens:
    EmptyTimeCodeRange = 'NONE'
    RangeSeparator = ':'
    StrideSeparator = 'x'
",
    )
    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    py.run(&code, None, Some(&dict))?;
    let tokens = dict
        .get_item("_TcrTokens")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Tokens class"))?;
    let tcr = m.getattr("TimeCodeRange")?;
    tcr.setattr("Tokens", tokens)?;

    Ok(())
}
