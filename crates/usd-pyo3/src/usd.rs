//! pxr.Usd — Core USD Python bindings
//!
//! Drop-in replacement for the C++ OpenUSD `pxr.Usd` Python module.
//! Wraps usd-core Rust types with PyO3.

use pyo3::prelude::*;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::{PyDict, PyList, PyTuple};
use std::sync::Arc;

use usd_core::{
    stage::Stage,
    prim::Prim,
    attribute::Attribute,
    relationship::Relationship,
    edit_target::EditTarget,
    edit_context::EditContext,
    population_mask::StagePopulationMask,
    common::InitialLoadSet,
    time_code::TimeCode,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Helper: convert Rust Error to Python RuntimeError
// ============================================================================

fn to_py_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn path_from_str(s: &str) -> PyResult<Path> {
    Path::from_string(s).ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {s}")))
}

fn value_to_py(py: Python<'_>, val: &Value) -> Py<PyAny> {
    // Try common types; fall back to string repr
    if let Some(v) = val.downcast_clone::<bool>() {
        return v.into_pyobject(py).expect("ok").to_owned().into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<i32>() {
        return v.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<i64>() {
        return v.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<f32>() {
        return (v as f64).into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<f64>() {
        return v.into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<String>() {
        return v.as_str().into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<Token>() {
        return v.as_str().to_string().into_pyobject(py).expect("ok").into_any().unbind();
    }
    if let Some(v) = val.downcast_clone::<Vec<f32>>() {
        return PyList::new(py, v).map(|l| l.into_any().unbind()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<f64>>() {
        return PyList::new(py, v).map(|l| l.into_any().unbind()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<i32>>() {
        return PyList::new(py, v).map(|l| l.into_any().unbind()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<String>>() {
        return PyList::new(py, v).map(|l| l.into_any().unbind()).unwrap_or_else(|_| py.None());
    }
    // GfVec types → PyVec (proper Gf.Vec3d etc.)
    if let Some(v) = val.downcast_clone::<usd_gf::Vec3d>() {
        return Py::new(py, crate::gf::vec::PyVec3d(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    if let Some(v) = val.downcast_clone::<usd_gf::Vec3f>() {
        return Py::new(py, crate::gf::vec::PyVec3f(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    if let Some(v) = val.downcast_clone::<usd_gf::Vec2d>() {
        return Py::new(py, crate::gf::vec::PyVec2d(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    if let Some(v) = val.downcast_clone::<usd_gf::Vec2f>() {
        return Py::new(py, crate::gf::vec::PyVec2f(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    if let Some(v) = val.downcast_clone::<usd_gf::Vec4d>() {
        return Py::new(py, crate::gf::vec::PyVec4d(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    if let Some(v) = val.downcast_clone::<usd_gf::Vec4f>() {
        return Py::new(py, crate::gf::vec::PyVec4f(v))
            .map(|p| p.into_any())
            .unwrap_or_else(|_| py.None().into_bound(py).unbind());
    }
    // Legacy glam types (fallback)
    if let Some(v) = val.downcast_clone::<glam::Vec3>() {
        return PyTuple::new(py, [v.x as f64, v.y as f64, v.z as f64])
            .map(|t| t.into_any().unbind())
            .unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<glam::Vec2>() {
        return PyTuple::new(py, [v.x as f64, v.y as f64])
            .map(|t| t.into_any().unbind())
            .unwrap_or_else(|_| py.None());
    }
    if val.is_empty() {
        return py.None();
    }
    // Fallback: debug string
    format!("{val:?}").into_pyobject(py).expect("ok").into_any().unbind()
}

fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Ok(v) = obj.extract::<bool>() {
        return Ok(Value::from(v));
    }
    if let Ok(v) = obj.extract::<i64>() {
        return Ok(Value::from(v as i32));
    }
    if let Ok(v) = obj.extract::<f64>() {
        return Ok(Value::from(v));
    }
    if let Ok(v) = obj.extract::<String>() {
        return Ok(Value::from(v));
    }
    if let Ok(v) = obj.extract::<Vec<f64>>() {
        return Ok(Value::from_no_hash(v));
    }
    if let Ok(v) = obj.extract::<Vec<i64>>() {
        let vi: Vec<i32> = v.into_iter().map(|x| x as i32).collect();
        return Ok(Value::from(vi));
    }
    if let Ok(v) = obj.extract::<Vec<String>>() {
        return Ok(Value::from(v));
    }
    Err(PyValueError::new_err(format!(
        "Cannot convert Python object of type '{}' to VtValue",
        obj.get_type().name()?
    )))
}

// ============================================================================
// UsdTimeCode
// ============================================================================

/// Represents a USD time code value.
///
/// Matches C++ `UsdTimeCode`.
#[pyclass(skip_from_py_object,name = "TimeCode", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyTimeCode {
    inner: TimeCode,
}

#[pymethods]
impl PyTimeCode {
    #[new]
    #[pyo3(signature = (t = 0.0))]
    fn new(t: f64) -> Self {
        Self { inner: TimeCode::new(t) }
    }

    /// UsdTimeCode representing the default time.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn Default() -> Self {
        Self { inner: TimeCode::default() }
    }

    /// UsdTimeCode representing the earliest possible time.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn EarliestTime() -> Self {
        Self { inner: TimeCode::earliest_time() }
    }

    /// UsdTimeCode representing the pre-time sentinel.
    ///
    /// Matches C++ `UsdTimeCode::PreTime()`.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn PreTime() -> Self {
        // Pre-time is just before the earliest time — use a very small value
        Self { inner: TimeCode::new(f64::MIN) }
    }

    /// Returns a safe step value to advance time codes.
    ///
    /// Matches C++ `UsdTimeCode::SafeStep()`.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn SafeStep() -> f64 {
        // USD C++ returns 1.0 / (2^10) but we use the same constant.
        // Actually it's `std::numeric_limits<double>::epsilon() * stage.timeCodesPerSecond`
        // but the static version just returns a small epsilon.
        1.0 / 1024.0
    }

    #[getter]
    #[allow(non_snake_case)]
    fn IsDefault(&self) -> bool {
        self.inner.is_default()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn IsEarliestTime(&self) -> bool {
        self.inner.is_earliest_time()
    }

    #[getter]
    #[allow(non_snake_case)]
    fn IsNumeric(&self) -> bool {
        self.inner.is_numeric()
    }

    #[allow(non_snake_case)]
    fn GetValue(&self) -> f64 {
        if self.inner.is_default() { f64::NAN } else { self.inner.value() }
    }

    fn __float__(&self) -> f64 {
        if self.inner.is_default() { f64::NAN } else { self.inner.value() }
    }

    fn __repr__(&self) -> String {
        if self.inner.is_default() {
            "Usd.TimeCode.Default()".to_string()
        } else if self.inner.is_earliest_time() {
            "Usd.TimeCode.EarliestTime()".to_string()
        } else {
            format!("Usd.TimeCode({})", self.inner.value())
        }
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &PyTimeCode) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &PyTimeCode) -> bool {
        self.inner != other.inner
    }

    fn __lt__(&self, other: &PyTimeCode) -> bool {
        self.inner < other.inner
    }

    fn __le__(&self, other: &PyTimeCode) -> bool {
        self.inner <= other.inner
    }

    fn __gt__(&self, other: &PyTimeCode) -> bool {
        self.inner > other.inner
    }

    fn __ge__(&self, other: &PyTimeCode) -> bool {
        self.inner >= other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut h);
        h.finish()
    }

    fn __add__(&self, rhs: f64) -> Self {
        // Use __float__ conversion for raw numeric value
        let v = self.__float__();
        Self { inner: TimeCode::new(v + rhs) }
    }

    fn __radd__(&self, lhs: f64) -> Self {
        let v = self.__float__();
        Self { inner: TimeCode::new(lhs + v) }
    }

    fn __sub__(&self, rhs: f64) -> Self {
        let v = self.__float__();
        Self { inner: TimeCode::new(v - rhs) }
    }

    fn __mul__(&self, rhs: f64) -> Self {
        let v = self.__float__();
        Self { inner: TimeCode::new(v * rhs) }
    }

    fn __truediv__(&self, rhs: f64) -> PyResult<Self> {
        if rhs == 0.0 {
            return Err(PyValueError::new_err("division by zero"));
        }
        let v = self.__float__();
        Ok(Self { inner: TimeCode::new(v / rhs) })
    }
}

impl PyTimeCode {
    fn to_time_code(&self) -> TimeCode {
        self.inner
    }
}

fn tc_from_py(obj: &Bound<'_, PyAny>) -> PyResult<TimeCode> {
    if let Ok(tc) = obj.extract::<PyRef<PyTimeCode>>() {
        return Ok(tc.to_time_code());
    }
    if let Ok(v) = obj.extract::<f64>() {
        return Ok(TimeCode::new(v));
    }
    Err(PyValueError::new_err("Expected Usd.TimeCode or float"))
}

/// Convert a `usd_core::TimeCode` to `usd_sdf::TimeCode` (= `usd_vt::TimeCode`).
///
/// These are two separate types: USD core has default/earliest/pre sentinels,
/// while SDF TimeCode is a thin f64 wrapper. Default → NaN, earliest → f64::MIN.
fn core_tc_to_sdf(tc: TimeCode) -> usd_sdf::TimeCode {
    if tc.is_default() {
        usd_sdf::TimeCode::DEFAULT
    } else {
        usd_sdf::TimeCode::new(tc.value())
    }
}

// ============================================================================
// UsdStagePopulationMask
// ============================================================================

/// Controls which prims are populated on a stage.
///
/// Matches C++ `UsdStagePopulationMask`.
#[pyclass(skip_from_py_object,name = "StagePopulationMask", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyStagePopulationMask {
    inner: StagePopulationMask,
}

#[pymethods]
impl PyStagePopulationMask {
    #[new]
    #[pyo3(signature = (*paths))]
    fn new(paths: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let mut mask = StagePopulationMask::new();
        for item in paths.iter() {
            if let Ok(s) = item.extract::<String>() {
                if let Some(p) = Path::from_string(&s) {
                    mask.add(p);
                }
            } else if let Ok(list) = item.extract::<Vec<String>>() {
                for s in &list {
                    if let Some(p) = Path::from_string(s) {
                        mask.add(p);
                    }
                }
            }
        }
        Ok(Self { inner: mask })
    }

    /// Returns a mask that includes everything.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn All() -> Self {
        Self { inner: StagePopulationMask::all() }
    }

    #[allow(non_snake_case)]
    fn Add(&mut self, path: &str) -> PyResult<()> {
        let p = path_from_str(path)?;
        self.inner.add(p);
        Ok(())
    }

    #[allow(non_snake_case)]
    fn Includes(&self, path: &str) -> PyResult<bool> {
        let p = path_from_str(path)?;
        Ok(self.inner.includes(&p))
    }

    #[allow(non_snake_case)]
    fn IsEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    #[allow(non_snake_case)]
    fn GetPaths(&self) -> Vec<String> {
        self.inner.get_paths().iter().map(|p| p.as_str().to_string()).collect()
    }

    /// Returns the union of two masks.
    #[allow(non_snake_case)]
    fn Union(&self, other: &PyStagePopulationMask) -> Self {
        let mut result = self.inner.clone();
        for path in other.inner.get_paths() {
            result.add(path.clone());
        }
        Self { inner: result }
    }

    /// Returns the intersection of two masks.
    #[allow(non_snake_case)]
    fn Intersection(&self, other: &PyStagePopulationMask) -> Self {
        let my_paths: std::collections::HashSet<String> =
            self.inner.get_paths().iter().map(|p| p.as_str().to_string()).collect();
        let mut result = StagePopulationMask::new();
        for path in other.inner.get_paths() {
            if my_paths.contains(path.as_str()) {
                result.add(path.clone());
            }
        }
        Self { inner: result }
    }

    /// Returns the child names that are included at the given path.
    #[allow(non_snake_case)]
    fn GetIncludedChildNames(&self, path: &str) -> PyResult<(bool, Vec<String>)> {
        let p = path_from_str(path)?;
        let mut child_names = Vec::new();
        let all_included = self.inner.get_included_child_names(&p, &mut child_names);
        let names: Vec<String> = child_names.iter().map(|t| t.as_str().to_string()).collect();
        Ok((all_included, names))
    }

    fn __repr__(&self) -> String {
        let paths: Vec<_> = self.inner.get_paths().iter().map(|p| p.as_str().to_string()).collect();
        format!("Usd.StagePopulationMask([{}])", paths.join(", "))
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __eq__(&self, other: &PyStagePopulationMask) -> bool {
        self.inner == other.inner
    }
}

// ============================================================================
// UsdEditTarget
// ============================================================================

/// Specifies which layer should receive edits on a stage.
///
/// Matches C++ `UsdEditTarget`.
#[pyclass(skip_from_py_object,name = "EditTarget", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyEditTarget {
    inner: EditTarget,
}

#[pymethods]
impl PyEditTarget {
    #[new]
    fn new() -> Self {
        Self { inner: EditTarget::invalid() }
    }

    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Return the layer this edit target directs edits to.
    #[allow(non_snake_case)]
    fn GetLayer(&self) -> Option<crate::sdf::PyLayer> {
        self.inner.layer().map(|l| crate::sdf::PyLayer::from_layer_arc(l.clone()))
    }

    fn __repr__(&self) -> String {
        if self.inner.is_valid() {
            format!(
                "Usd.EditTarget(layer='{}')",
                self.inner.layer().map(|l| l.identifier().to_string()).unwrap_or_default()
            )
        } else {
            "Usd.EditTarget()".to_string()
        }
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ============================================================================
// UsdEditContext  (Python context manager)
// ============================================================================

/// RAII helper that temporarily changes a stage's edit target.
///
/// Matches C++ `UsdEditContext`. Use as `with Usd.EditContext(stage, target):`.
#[pyclass(skip_from_py_object,name = "EditContext", module = "pxr_rs.Usd")]
pub struct PyEditContext {
    inner: Option<EditContext>,
}

#[pymethods]
impl PyEditContext {
    #[new]
    fn new(stage: &PyStage, target: &PyEditTarget) -> Self {
        let ctx = EditContext::new_with_target(stage.inner.clone(), target.inner.clone());
        Self { inner: Some(ctx) }
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc_val: &Bound<'_, PyAny>,
        _exc_tb: &Bound<'_, PyAny>,
    ) -> bool {
        // Drop restores the original edit target via EditContext::drop()
        self.inner = None;
        false
    }
}

// ============================================================================
// UsdPrimRange  (iterator)
// ============================================================================

/// Iterator over a range of prims on a stage.
///
/// Matches C++ `UsdPrimRange`.
#[pyclass(skip_from_py_object,name = "PrimRange", module = "pxr_rs.Usd")]
pub struct PyPrimRange {
    prims: Vec<Py<PyAny>>,
    index: usize,
}

#[pymethods]
impl PyPrimRange {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> Option<Py<PyAny>> {
        if self.index < self.prims.len() {
            let item = self.prims[self.index].clone_ref(py);
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn __len__(&self) -> usize {
        self.prims.len()
    }
}

impl PyPrimRange {
    fn from_prims(py: Python<'_>, prims: Vec<Prim>, stage_arc: Arc<Stage>) -> Self {
        let objs = prims
            .into_iter()
            .map(|p| {
                let py_prim = PyPrim::from_prim(p, stage_arc.clone());
                py_prim.into_pyobject(py).expect("ok").into_any().unbind()
            })
            .collect();
        Self { prims: objs, index: 0 }
    }
}

// ============================================================================
// UsdObject base helpers (shared metadata logic)
// ============================================================================

fn prim_get_metadata(py: Python<'_>, prim: &Prim, key: &str) -> PyResult<Py<PyAny>> {
    let token = Token::new(key);
    // Route through stage for proper composition, returning raw VtValue.
    let val_opt = prim.stage()
        .and_then(|s| s.get_metadata_for_object(prim.path(), &token));
    match val_opt {
        Some(v) => Ok(value_to_py(py, &v)),
        None => Ok(py.None()),
    }
}

fn prim_set_metadata(prim: &Prim, key: &str, obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let token = Token::new(key);
    let val = py_to_value(obj)?;
    Ok(prim.set_metadata(&token, val))
}

fn attr_get_metadata(py: Python<'_>, attr: &Attribute, key: &str) -> PyResult<Py<PyAny>> {
    let token = Token::new(key);
    match attr.get_metadata(&token) {
        Some(v) => Ok(value_to_py(py, &v)),
        None => Ok(py.None()),
    }
}

// ============================================================================
// UsdPrim
// ============================================================================

/// A composed prim on a stage.
///
/// Matches C++ `UsdPrim`.
#[pyclass(skip_from_py_object,name = "Prim", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyPrim {
    pub(crate) inner: Prim,
    // Keep the stage alive as long as this Python object exists.
    pub(crate) _stage: Arc<Stage>,
}

impl PyPrim {
    fn from_prim(prim: Prim, stage: Arc<Stage>) -> Self {
        Self { inner: prim, _stage: stage }
    }
}

#[pymethods]
impl PyPrim {
    // -- Validity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    // -- Identity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetName(&self) -> String {
        self.inner.get_name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetPath(&self) -> String {
        self.inner.get_path().as_str().to_string()
    }

    /// Returns the prim path (same as GetPath).
    #[allow(non_snake_case)]
    fn GetPrimPath(&self) -> String {
        self.inner.get_path().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetTypeName(&self) -> String {
        self.inner.get_type_name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetSpecifier(&self) -> String {
        match self.inner.specifier() {
            usd_sdf::Specifier::Def => "def".to_string(),
            usd_sdf::Specifier::Over => "over".to_string(),
            usd_sdf::Specifier::Class => "class".to_string(),
        }
    }

    // -- Flags -------------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsActive(&self) -> bool {
        self.inner.is_active()
    }

    #[allow(non_snake_case)]
    fn IsLoaded(&self) -> bool {
        self.inner.is_loaded()
    }

    #[allow(non_snake_case)]
    fn IsModel(&self) -> bool {
        self.inner.is_model()
    }

    #[allow(non_snake_case)]
    fn IsGroup(&self) -> bool {
        self.inner.is_group()
    }

    #[allow(non_snake_case)]
    fn IsAbstract(&self) -> bool {
        self.inner.is_abstract()
    }

    #[allow(non_snake_case)]
    fn IsDefined(&self) -> bool {
        self.inner.is_defined()
    }

    #[allow(non_snake_case)]
    fn HasPayload(&self) -> bool {
        self.inner.has_payload()
    }

    // -- Schema / type checks ---------------------------------------------

    #[allow(non_snake_case)]
    fn IsA(&self, schema_type: &str) -> bool {
        let token = Token::new(schema_type);
        self.inner.is_a(&token)
    }

    #[allow(non_snake_case)]
    fn HasAPI(&self, api_name: &str) -> bool {
        let token = Token::new(api_name);
        self.inner.has_api(&token)
    }

    #[allow(non_snake_case)]
    fn CanApplyAPI(&self, api_name: &str) -> bool {
        // Delegates to schema registry. Return true if not already applied.
        let token = Token::new(api_name);
        !self.inner.has_api(&token)
    }

    #[allow(non_snake_case)]
    fn ApplyAPI(&self, api_name: &str) -> bool {
        let token = Token::new(api_name);
        self.inner.apply_api(&token)
    }

    #[allow(non_snake_case)]
    fn RemoveAPI(&self, api_name: &str) -> bool {
        let token = Token::new(api_name);
        self.inner.remove_api(&token)
    }

    #[allow(non_snake_case)]
    fn GetAppliedSchemas(&self) -> Vec<String> {
        self.inner
            .get_applied_schemas()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    // -- Instancing --------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsInstance(&self) -> bool {
        self.inner.is_instance()
    }

    #[allow(non_snake_case)]
    fn IsInstanceProxy(&self) -> bool {
        self.inner.is_instance_proxy()
    }

    #[allow(non_snake_case)]
    fn IsPrototype(&self) -> bool {
        self.inner.is_prototype()
    }

    #[allow(non_snake_case)]
    fn IsInPrototype(&self) -> bool {
        self.inner.is_in_prototype()
    }

    #[allow(non_snake_case)]
    fn GetPrototype(&self) -> Option<PyPrim> {
        let proto = self.inner.get_prototype();
        if proto.is_valid() {
            Some(PyPrim::from_prim(proto, self._stage.clone()))
        } else {
            None
        }
    }

    #[allow(non_snake_case)]
    fn GetInstances(&self) -> Vec<PyPrim> {
        self.inner
            .get_instances()
            .into_iter()
            .map(|p| PyPrim::from_prim(p, self._stage.clone()))
            .collect()
    }

    // -- Hierarchy ---------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetParent(&self) -> Option<PyPrim> {
        let parent = self.inner.parent();
        if parent.is_valid() {
            Some(PyPrim::from_prim(parent, self._stage.clone()))
        } else {
            None
        }
    }

    #[allow(non_snake_case)]
    fn GetChildren(&self) -> Vec<PyPrim> {
        self.inner
            .get_children()
            .into_iter()
            .map(|p| PyPrim::from_prim(p, self._stage.clone()))
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetAllChildren(&self) -> Vec<PyPrim> {
        self.inner
            .get_all_children()
            .into_iter()
            .map(|p| PyPrim::from_prim(p, self._stage.clone()))
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetChildrenNames(&self) -> Vec<String> {
        self.inner
            .get_children()
            .iter()
            .map(|p| p.get_name().as_str().to_string())
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetChild(&self, name: &str) -> Option<PyPrim> {
        let child_path = self.inner.path().append_child(name)?;
        let stage = self.inner.stage()?;
        let prim = stage.get_prim_at_path(&child_path)?;
        Some(PyPrim::from_prim(prim, self._stage.clone()))
    }

    #[allow(non_snake_case)]
    fn GetNextSibling(&self) -> Option<PyPrim> {
        let next = self.inner.get_next_sibling();
        if next.is_valid() {
            Some(PyPrim::from_prim(next, self._stage.clone()))
        } else {
            None
        }
    }

    // -- Properties --------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetPropertyNames(&self) -> Vec<String> {
        self.inner
            .get_property_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetAuthoredPropertyNames(&self) -> Vec<String> {
        // Filter property names to those with authored values — check via attribute or relationship
        self.inner
            .get_property_names()
            .iter()
            .filter(|name| {
                let name_str = name.as_str();
                if let Some(attr) = self.inner.get_attribute(name_str) {
                    return attr.has_authored_value() || attr.as_property().is_authored();
                }
                if let Some(rel) = self.inner.get_relationship(name_str) {
                    return rel.as_property().is_authored();
                }
                false
            })
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetAttribute(&self, name: &str) -> Option<PyAttribute> {
        let attr = self.inner.get_attribute(name)?;
        Some(PyAttribute { inner: attr, _stage: self._stage.clone() })
    }

    #[allow(non_snake_case)]
    fn GetAttributes(&self) -> Vec<PyAttribute> {
        self.inner
            .get_attributes()
            .into_iter()
            .map(|a| PyAttribute { inner: a, _stage: self._stage.clone() })
            .collect()
    }

    #[allow(non_snake_case)]
    fn HasAttribute(&self, name: &str) -> bool {
        self.inner.has_attribute(name)
    }

    #[allow(non_snake_case)]
    fn GetRelationship(&self, name: &str) -> Option<PyRelationship> {
        let rel = self.inner.get_relationship(name)?;
        Some(PyRelationship { inner: rel, _stage: self._stage.clone() })
    }

    #[allow(non_snake_case)]
    fn GetRelationships(&self) -> Vec<PyRelationship> {
        self.inner
            .get_relationships()
            .into_iter()
            .map(|r| PyRelationship { inner: r, _stage: self._stage.clone() })
            .collect()
    }

    #[allow(non_snake_case)]
    fn HasRelationship(&self, name: &str) -> bool {
        self.inner.has_relationship(name)
    }

    // -- Authoring ---------------------------------------------------------

    /// Create an attribute on this prim.
    ///
    /// Matches C++ `UsdPrim::CreateAttribute`.
    #[pyo3(signature = (name, type_name, custom = true, variability = "varying"))]
    #[allow(non_snake_case)]
    fn CreateAttribute(
        &self,
        name: &str,
        type_name: &crate::sdf::PyValueTypeName,
        custom: bool,
        variability: &str,
    ) -> PyResult<Option<PyAttribute>> {
        let var = match variability {
            "uniform" | "Uniform" => Some(usd_core::attribute::Variability::Uniform),
            _ => Some(usd_core::attribute::Variability::Varying),
        };
        Ok(self.inner.create_attribute(name, &type_name.inner(), custom, var)
            .map(|a| PyAttribute { inner: a, _stage: self._stage.clone() }))
    }

    /// Create a relationship on this prim.
    ///
    /// Matches C++ `UsdPrim::CreateRelationship`.
    #[pyo3(signature = (name, custom = true))]
    #[allow(non_snake_case)]
    fn CreateRelationship(&self, name: &str, custom: bool) -> Option<PyRelationship> {
        self.inner.create_relationship(name, custom)
            .map(|r| PyRelationship { inner: r, _stage: self._stage.clone() })
    }

    // -- Composition arcs --------------------------------------------------

    /// Return a References proxy for editing reference arcs.
    #[allow(non_snake_case)]
    fn GetReferences(&self) -> PyReferences {
        PyReferences {
            prim: self.inner.clone(),
            _stage: self._stage.clone(),
        }
    }

    /// Return a Payloads proxy for editing payload arcs.
    #[allow(non_snake_case)]
    fn GetPayloads(&self) -> PyPayloads {
        PyPayloads {
            prim: self.inner.clone(),
            _stage: self._stage.clone(),
        }
    }

    /// Return an Inherits proxy for editing inherit arcs.
    #[allow(non_snake_case)]
    fn GetInherits(&self) -> PyInherits {
        PyInherits {
            prim: self.inner.clone(),
            _stage: self._stage.clone(),
        }
    }

    /// Return a Specializes proxy for editing specialize arcs.
    #[allow(non_snake_case)]
    fn GetSpecializes(&self) -> PySpecializes {
        PySpecializes {
            prim: self.inner.clone(),
            _stage: self._stage.clone(),
        }
    }

    #[allow(non_snake_case)]
    fn HasAuthoredReferences(&self) -> bool {
        self.inner.has_authored_references()
    }

    #[allow(non_snake_case)]
    fn HasAuthoredPayloads(&self) -> bool {
        self.inner.has_payload()
    }

    #[allow(non_snake_case)]
    fn HasAuthoredInherits(&self) -> bool {
        self.inner.has_authored_metadata(&Token::new("inheritPaths"))
    }

    #[allow(non_snake_case)]
    fn HasAuthoredSpecializes(&self) -> bool {
        self.inner.has_authored_metadata(&Token::new("specializes"))
    }

    #[allow(non_snake_case)]
    fn HasAuthoredTypeName(&self) -> bool {
        self.inner.has_authored_type_name()
    }

    #[allow(non_snake_case)]
    fn SetInstanceable(&self, instanceable: bool) -> bool {
        self.inner.set_instanceable(instanceable)
    }

    #[allow(non_snake_case)]
    fn IsInstanceable(&self) -> bool {
        self.inner.has_authored_metadata(&Token::new("instanceable"))
            && self.inner.get_metadata::<bool>(&Token::new("instanceable"))
                .unwrap_or(false)
    }

    #[allow(non_snake_case)]
    fn ClearInstanceable(&self) -> bool {
        self.inner.clear_metadata(&Token::new("instanceable"))
    }

    #[allow(non_snake_case)]
    fn HasAuthoredInstanceable(&self) -> bool {
        self.inner.has_authored_metadata(&Token::new("instanceable"))
    }

    /// Set custom data dictionary on this prim.
    #[allow(non_snake_case)]
    fn SetCustomData(&self, data: &Bound<'_, PyDict>) -> PyResult<bool> {
        let val = py_to_value(&data.as_any())?;
        Ok(self.inner.set_metadata(&Token::new("customData"), val))
    }

    /// Set a single key in the custom data dictionary.
    #[allow(non_snake_case)]
    fn SetCustomDataByKey(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let val = py_to_value(value)?;
        let meta_key = format!("customData:{key}");
        Ok(self.inner.set_metadata(&Token::new(&meta_key), val))
    }

    /// Get custom data dictionary.
    #[allow(non_snake_case)]
    fn GetCustomData(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        prim_get_metadata(py, &self.inner, "customData")
    }

    /// Static method: check if path is a prototype path.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsPrototypePath(path: &str) -> bool {
        path.starts_with("/__Prototype_")
    }

    #[allow(non_snake_case)]
    fn SetActive(&self, active: bool) -> bool {
        self.inner.set_active(active)
    }

    #[allow(non_snake_case)]
    fn SetTypeName(&self, type_name: &str) -> bool {
        self.inner.set_type_name(type_name)
    }

    #[allow(non_snake_case)]
    fn SetSpecifier(&self, specifier: &str) -> PyResult<bool> {
        let spec = match specifier {
            "def" | "Def" => usd_sdf::Specifier::Def,
            "over" | "Over" => usd_sdf::Specifier::Over,
            "class" | "Class" => usd_sdf::Specifier::Class,
            _ => return Err(PyValueError::new_err(format!("Unknown specifier: {specifier}"))),
        };
        Ok(self.inner.set_specifier(spec))
    }

    // -- Kind --------------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetKind(&self) -> String {
        let key = Token::new("kind");
        self.inner.stage()
            .and_then(|s| s.get_metadata_for_object(self.inner.path(), &key))
            .and_then(|v| v.downcast_clone::<Token>())
            .map(|t| t.as_str().to_string())
            .unwrap_or_default()
    }

    #[allow(non_snake_case)]
    fn SetKind(&self, kind: &str) -> bool {
        self.inner.set_metadata(&Token::new("kind"), Value::from(Token::new(kind)))
    }

    // -- Variant sets ------------------------------------------------------

    #[allow(non_snake_case)]
    fn HasVariantSets(&self) -> bool {
        self.inner.has_variant_sets()
    }

    #[allow(non_snake_case)]
    fn GetVariantSets(&self) -> PyVariantSets {
        PyVariantSets {
            prim: self.inner.clone(),
            _stage: self._stage.clone(),
        }
    }

    /// Convenience method: return a single VariantSet by name.
    ///
    /// Equivalent to `prim.GetVariantSets().GetVariantSet(name)`.
    #[allow(non_snake_case)]
    fn GetVariantSet(&self, name: &str) -> PyVariantSet {
        PyVariantSet {
            prim: self.inner.clone(),
            name: name.to_string(),
            _stage: self._stage.clone(),
        }
    }

    // -- Metadata ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        prim_get_metadata(py, &self.inner, key)
    }

    #[allow(non_snake_case)]
    fn SetMetadata(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        prim_set_metadata(&self.inner, key, value)
    }

    #[allow(non_snake_case)]
    fn HasMetadata(&self, key: &str) -> bool {
        let token = Token::new(key);
        self.inner.stage()
            .and_then(|s| s.get_metadata_for_object(self.inner.path(), &token))
            .is_some()
    }

    #[allow(non_snake_case)]
    fn ClearMetadata(&self, key: &str) -> bool {
        self.inner.clear_metadata(&Token::new(key))
    }

    #[allow(non_snake_case)]
    fn GetAllMetadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        // Collect authored metadata from the strongest layer for known schema keys.
        // C++ enumerates registered fields; we approximate with common authored fields.
        let dict = PyDict::new(py);
        let common_keys = [
            "kind", "active", "instanceable", "hidden", "apiSchemas",
            "assetInfo", "customData", "doc", "comment", "typeName",
            "clips", "payload", "references", "inherits", "specializes",
        ];
        for key_str in common_keys {
            let key = Token::new(key_str);
            if let Some(val) = self.inner.stage()
                .and_then(|s| s.get_metadata_for_object(self.inner.path(), &key))
            {
                dict.set_item(key_str, value_to_py(py, &val))?;
            }
        }
        Ok(dict.into_any().unbind())
    }

    #[allow(non_snake_case)]
    fn HasAuthoredMetadata(&self, key: &str) -> bool {
        self.inner.has_authored_metadata(&Token::new(key))
    }

    // -- Stage reference ---------------------------------------------------

    #[allow(non_snake_case)]
    fn GetStage(&self) -> PyStage {
        PyStage { inner: self._stage.clone() }
    }

    // -- Description -------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("Usd.Prim({})", self.inner.get_path().as_str())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &PyPrim) -> bool {
        self.inner.path() == other.inner.path()
    }

    fn __ne__(&self, other: &PyPrim) -> bool {
        self.inner.path() != other.inner.path()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// UsdVariantSets (lightweight helper returned by prim.GetVariantSets())
// ============================================================================

#[pyclass(skip_from_py_object,name = "VariantSets", module = "pxr_rs.Usd")]
pub struct PyVariantSets {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyVariantSets {
    #[allow(non_snake_case)]
    fn GetNames(&self) -> Vec<String> {
        self.prim.get_variant_sets().get_names()
    }

    #[allow(non_snake_case)]
    fn HasVariantSet(&self, name: &str) -> bool {
        self.prim.get_variant_sets().get_names().iter().any(|n| n == name)
    }

    #[allow(non_snake_case)]
    fn GetVariantSet(&self, name: &str) -> PyVariantSet {
        PyVariantSet {
            prim: self.prim.clone(),
            name: name.to_string(),
            _stage: self._stage.clone(),
        }
    }

    #[allow(non_snake_case)]
    fn GetVariantSelection(&self, variant_set_name: &str) -> String {
        self.prim.get_variant_sets().get_variant_selection(variant_set_name)
    }

    /// Add a new variant set to this prim.
    ///
    /// Matches C++ `UsdVariantSets::AddVariantSet`.
    #[pyo3(signature = (name, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddVariantSet(&self, name: &str, position: &str) -> PyVariantSet {
        let pos = parse_list_position(position);
        self.prim.get_variant_sets().add_variant_set(name, pos);
        PyVariantSet {
            prim: self.prim.clone(),
            name: name.to_string(),
            _stage: self._stage.clone(),
        }
    }

    #[allow(non_snake_case)]
    fn SetSelection(&self, variant_set_name: &str, variant_name: &str) -> bool {
        self.prim.get_variant_sets().get_variant_set(variant_set_name)
            .set_variant_selection(variant_name)
    }
}

#[pyclass(skip_from_py_object,name = "VariantSet", module = "pxr_rs.Usd")]
pub struct PyVariantSet {
    prim: Prim,
    name: String,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyVariantSet {
    #[allow(non_snake_case)]
    fn GetVariantNames(&self) -> Vec<String> {
        self.prim.get_variant_sets().get_variant_set(&self.name).get_variant_names()
    }

    #[allow(non_snake_case)]
    fn GetVariantSelection(&self) -> String {
        self.prim.get_variant_sets().get_variant_set(&self.name).get_variant_selection()
    }

    #[allow(non_snake_case)]
    fn SetVariantSelection(&self, variant: &str) -> bool {
        self.prim.get_variant_sets().get_variant_set(&self.name).set_variant_selection(variant)
    }

    #[allow(non_snake_case)]
    fn ClearVariantSelection(&self) -> bool {
        self.prim.get_variant_sets().get_variant_set(&self.name).clear_variant_selection()
    }

    /// Add a variant name to this variant set.
    ///
    /// Matches C++ `UsdVariantSet::AddVariant`.
    #[pyo3(signature = (variant_name, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddVariant(&self, variant_name: &str, position: &str) -> bool {
        let pos = parse_list_position(position);
        self.prim.get_variant_sets().get_variant_set(&self.name).add_variant(variant_name, pos)
    }

    /// Get the name of this variant set.
    #[allow(non_snake_case)]
    fn GetName(&self) -> &str {
        &self.name
    }

    fn __repr__(&self) -> String {
        format!("Usd.VariantSet('{}' on {})", self.name, self.prim.get_path().as_str())
    }

    fn __bool__(&self) -> bool {
        true
    }
}

// ============================================================================
// List position helper
// ============================================================================

fn parse_list_position(s: &str) -> usd_core::common::ListPosition {
    match s {
        "FrontOfPrependList" => usd_core::common::ListPosition::FrontOfPrependList,
        "BackOfPrependList" => usd_core::common::ListPosition::BackOfPrependList,
        "FrontOfAppendList" => usd_core::common::ListPosition::FrontOfAppendList,
        "BackOfAppendList" => usd_core::common::ListPosition::BackOfAppendList,
        _ => usd_core::common::ListPosition::BackOfPrependList,
    }
}

// ============================================================================
// UsdReferences — proxy for editing reference arcs
// ============================================================================

#[pyclass(skip_from_py_object, name = "References", module = "pxr_rs.Usd")]
pub struct PyReferences {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyReferences {
    /// Add a reference to an external layer (by identifier) and optional prim path.
    #[pyo3(signature = (asset_path = "", prim_path = "", layer_offset = None, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddReference(
        &self,
        asset_path: &str,
        prim_path: &str,
        layer_offset: Option<&crate::sdf::PyLayerOffset>,
        position: &str,
    ) -> PyResult<bool> {
        let refs = self.prim.get_references();
        let pos = parse_list_position(position);
        let offset = layer_offset.map(|lo| lo.to_layer_offset()).unwrap_or_default();

        if prim_path.is_empty() && !asset_path.is_empty() {
            Ok(refs.add_reference_to_default_prim(asset_path, offset, pos))
        } else if !asset_path.is_empty() || !prim_path.is_empty() {
            let p = if prim_path.is_empty() {
                Path::empty()
            } else {
                path_from_str(prim_path)?
            };
            Ok(refs.add_reference_with_path(asset_path, &p, offset, pos))
        } else {
            // Both empty — add empty reference (internal)
            let reference = usd_sdf::Reference::default();
            Ok(refs.add_reference(&reference, pos))
        }
    }

    /// Add an internal reference to a prim path in the same stage.
    #[pyo3(signature = (prim_path, layer_offset = None, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddInternalReference(
        &self,
        prim_path: &str,
        layer_offset: Option<&crate::sdf::PyLayerOffset>,
        position: &str,
    ) -> PyResult<bool> {
        let refs = self.prim.get_references();
        let pos = parse_list_position(position);
        let offset = layer_offset.map(|lo| lo.to_layer_offset()).unwrap_or_default();
        let p = path_from_str(prim_path)?;
        Ok(refs.add_internal_reference(&p, offset, pos))
    }

    #[allow(non_snake_case)]
    fn ClearReferences(&self) -> bool {
        self.prim.get_references().clear_references()
    }

    #[allow(non_snake_case)]
    fn SetReferences(&self) -> bool {
        self.prim.get_references().set_references(Vec::new())
    }

    #[allow(non_snake_case)]
    fn GetPrim(&self) -> PyPrim {
        PyPrim::from_prim(self.prim.clone(), self._stage.clone())
    }

    fn __repr__(&self) -> String {
        format!("Usd.References({})", self.prim.get_path().as_str())
    }

    fn __bool__(&self) -> bool {
        true
    }
}

// ============================================================================
// UsdPayloads — proxy for editing payload arcs
// ============================================================================

#[pyclass(skip_from_py_object, name = "Payloads", module = "pxr_rs.Usd")]
pub struct PyPayloads {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyPayloads {
    /// Add a payload to an external layer (by identifier) and optional prim path.
    #[pyo3(signature = (asset_path = "", prim_path = "", layer_offset = None, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddPayload(
        &self,
        asset_path: &str,
        prim_path: &str,
        layer_offset: Option<&crate::sdf::PyLayerOffset>,
        position: &str,
    ) -> PyResult<bool> {
        let payloads = self.prim.get_payloads();
        let pos = parse_list_position(position);
        let offset = layer_offset.map(|lo| lo.to_layer_offset()).unwrap_or_default();

        if prim_path.is_empty() && !asset_path.is_empty() {
            Ok(payloads.add_payload_to_default_prim(asset_path, offset, pos))
        } else if !asset_path.is_empty() || !prim_path.is_empty() {
            let p = if prim_path.is_empty() {
                Path::empty()
            } else {
                path_from_str(prim_path)?
            };
            Ok(payloads.add_payload_with_path(asset_path, &p, offset, pos))
        } else {
            let payload = usd_sdf::Payload::default();
            Ok(payloads.add_payload(&payload, pos))
        }
    }

    #[pyo3(signature = (prim_path, layer_offset = None, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddInternalPayload(
        &self,
        prim_path: &str,
        layer_offset: Option<&crate::sdf::PyLayerOffset>,
        position: &str,
    ) -> PyResult<bool> {
        let payloads = self.prim.get_payloads();
        let pos = parse_list_position(position);
        let offset = layer_offset.map(|lo| lo.to_layer_offset()).unwrap_or_default();
        let p = path_from_str(prim_path)?;
        Ok(payloads.add_internal_payload(&p, offset, pos))
    }

    #[allow(non_snake_case)]
    fn ClearPayloads(&self) -> bool {
        self.prim.get_payloads().clear_payloads()
    }

    #[allow(non_snake_case)]
    fn SetPayloads(&self) -> bool {
        self.prim.get_payloads().set_payloads(Vec::new())
    }

    #[allow(non_snake_case)]
    fn GetPrim(&self) -> PyPrim {
        PyPrim::from_prim(self.prim.clone(), self._stage.clone())
    }

    fn __repr__(&self) -> String {
        format!("Usd.Payloads({})", self.prim.get_path().as_str())
    }

    fn __bool__(&self) -> bool {
        true
    }
}

// ============================================================================
// UsdInherits — proxy for editing inherit arcs
// ============================================================================

#[pyclass(skip_from_py_object, name = "Inherits", module = "pxr_rs.Usd")]
pub struct PyInherits {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyInherits {
    #[pyo3(signature = (prim_path, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddInherit(&self, prim_path: &str, position: &str) -> PyResult<bool> {
        let p = path_from_str(prim_path)?;
        let pos = parse_list_position(position);
        Ok(self.prim.get_inherits().add_inherit(&p, pos))
    }

    #[allow(non_snake_case)]
    fn RemoveInherit(&self, prim_path: &str) -> PyResult<bool> {
        let p = path_from_str(prim_path)?;
        Ok(self.prim.get_inherits().remove_inherit(&p))
    }

    #[allow(non_snake_case)]
    fn ClearInherits(&self) -> bool {
        self.prim.get_inherits().clear_inherits()
    }

    #[allow(non_snake_case)]
    fn SetInherits(&self, paths: Vec<String>) -> PyResult<bool> {
        let items: PyResult<Vec<Path>> = paths.iter().map(|s| path_from_str(s)).collect();
        Ok(self.prim.get_inherits().set_inherits(items?))
    }

    #[allow(non_snake_case)]
    fn GetAllDirectInherits(&self) -> Vec<String> {
        self.prim.get_inherits()
            .get_all_direct_inherits()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect()
    }

    #[allow(non_snake_case)]
    fn GetPrim(&self) -> PyPrim {
        PyPrim::from_prim(self.prim.clone(), self._stage.clone())
    }

    fn __repr__(&self) -> String {
        format!("Usd.Inherits({})", self.prim.get_path().as_str())
    }

    fn __bool__(&self) -> bool {
        true
    }
}

// ============================================================================
// UsdSpecializes — proxy for editing specialize arcs
// ============================================================================

#[pyclass(skip_from_py_object, name = "Specializes", module = "pxr_rs.Usd")]
pub struct PySpecializes {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PySpecializes {
    #[pyo3(signature = (prim_path, position = "BackOfPrependList"))]
    #[allow(non_snake_case)]
    fn AddSpecialize(&self, prim_path: &str, position: &str) -> PyResult<bool> {
        let p = path_from_str(prim_path)?;
        let pos = parse_list_position(position);
        Ok(self.prim.get_specializes().add_specialize(&p, pos))
    }

    #[allow(non_snake_case)]
    fn RemoveSpecialize(&self, prim_path: &str) -> PyResult<bool> {
        let p = path_from_str(prim_path)?;
        Ok(self.prim.get_specializes().remove_specialize(&p))
    }

    #[allow(non_snake_case)]
    fn ClearSpecializes(&self) -> bool {
        self.prim.get_specializes().clear_specializes()
    }

    #[allow(non_snake_case)]
    fn SetSpecializes(&self, paths: Vec<String>) -> PyResult<bool> {
        let items: PyResult<Vec<Path>> = paths.iter().map(|s| path_from_str(s)).collect();
        Ok(self.prim.get_specializes().set_specializes(items?))
    }

    #[allow(non_snake_case)]
    fn GetPrim(&self) -> PyPrim {
        PyPrim::from_prim(self.prim.clone(), self._stage.clone())
    }

    fn __repr__(&self) -> String {
        format!("Usd.Specializes({})", self.prim.get_path().as_str())
    }

    fn __bool__(&self) -> bool {
        true
    }
}

// ============================================================================
// UsdAttribute
// ============================================================================

/// A typed, time-varying attribute on a prim.
///
/// Matches C++ `UsdAttribute`.
#[pyclass(skip_from_py_object,name = "Attribute", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyAttribute {
    inner: Attribute,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyAttribute {
    // -- Validity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    // -- Identity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetPath(&self) -> String {
        self.inner.path().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetName(&self) -> String {
        self.inner.name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetBaseName(&self) -> String {
        self.inner.as_property().base_name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetNamespace(&self) -> String {
        self.inner.as_property().namespace().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn SplitName(&self) -> Vec<String> {
        self.inner.as_property().split_name().iter().map(|t| t.as_str().to_string()).collect()
    }

    #[allow(non_snake_case)]
    fn GetPrimPath(&self) -> String {
        self.inner.prim_path().as_str().to_string()
    }

    // -- Type info ---------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetTypeName(&self) -> String {
        self.inner.get_type_name().as_token().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetVariability(&self) -> String {
        match self.inner.variability() {
            usd_core::attribute::Variability::Varying => "varying".to_string(),
            usd_core::attribute::Variability::Uniform => "uniform".to_string(),
        }
    }

    #[allow(non_snake_case)]
    fn GetRoleName(&self) -> String {
        self.inner.get_role_name().as_str().to_string()
    }

    // -- Value access ------------------------------------------------------

    /// Get the attribute value at the given time code.
    ///
    /// Returns None if there is no value.
    #[pyo3(signature = (time = None))]
    #[allow(non_snake_case)]
    fn Get(&self, py: Python<'_>, time: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        let tc = match time {
            Some(t) => tc_from_py(t)?,
            None => TimeCode::default(),
        };
        match self.inner.get(tc) {
            Some(v) => Ok(value_to_py(py, &v)),
            None => Ok(py.None()),
        }
    }

    /// Set the attribute value at the given time code.
    #[pyo3(signature = (value, time = None))]
    #[allow(non_snake_case)]
    fn Set(&self, value: &Bound<'_, PyAny>, time: Option<&Bound<'_, PyAny>>) -> PyResult<bool> {
        let tc = match time {
            Some(t) => core_tc_to_sdf(tc_from_py(t)?),
            None => usd_sdf::TimeCode::DEFAULT,
        };
        let val = py_to_value(value)?;
        Ok(self.inner.set(val, tc))
    }

    /// Clear the authored value at the given time.
    #[allow(non_snake_case)]
    fn Clear(&self, time: Option<&Bound<'_, PyAny>>) -> PyResult<bool> {
        let tc = match time {
            Some(t) => core_tc_to_sdf(tc_from_py(t)?),
            None => usd_sdf::TimeCode::DEFAULT,
        };
        Ok(self.inner.clear(tc))
    }

    /// Clear the default value.
    #[allow(non_snake_case)]
    fn ClearDefault(&self) -> bool {
        self.inner.clear(usd_sdf::TimeCode::DEFAULT)
    }

    /// Clear the value at the given time.
    #[allow(non_snake_case)]
    fn ClearAtTime(&self, time: &Bound<'_, PyAny>) -> PyResult<bool> {
        let tc = core_tc_to_sdf(tc_from_py(time)?);
        Ok(self.inner.clear(tc))
    }

    /// Block the attribute value (author a ValueBlock).
    #[allow(non_snake_case)]
    fn Block(&self) -> bool {
        self.inner.block()
    }

    // -- Value queries -----------------------------------------------------

    #[allow(non_snake_case)]
    fn HasValue(&self) -> bool {
        self.inner.has_value()
    }

    #[allow(non_snake_case)]
    fn HasAuthoredValue(&self) -> bool {
        self.inner.has_authored_value()
    }

    #[allow(non_snake_case)]
    fn HasFallbackValue(&self) -> bool {
        self.inner.has_fallback_value()
    }

    // -- Time samples ------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetTimeSamples(&self) -> Vec<f64> {
        self.inner.get_time_samples()
    }

    #[allow(non_snake_case)]
    fn GetTimeSamplesInInterval(&self, lo: f64, hi: f64) -> Vec<f64> {
        self.inner.get_time_samples_in_interval(lo, hi)
    }

    #[allow(non_snake_case)]
    fn GetBracketingTimeSamples(&self, desired: f64) -> (f64, f64, bool) {
        match self.inner.get_bracketing_time_samples(desired) {
            Some((lo, hi)) => (lo, hi, true),
            None => (0.0, 0.0, false),
        }
    }

    #[allow(non_snake_case)]
    fn ValueMightBeTimeVarying(&self) -> bool {
        self.inner.value_might_be_time_varying()
    }

    #[allow(non_snake_case)]
    fn GetNumTimeSamples(&self) -> usize {
        self.inner.get_time_samples().len()
    }

    // -- Connections -------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetConnections(&self) -> Vec<String> {
        self.inner
            .get_connections()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect()
    }

    #[allow(non_snake_case)]
    fn AddConnection(&self, source: &str) -> PyResult<bool> {
        let p = path_from_str(source)?;
        Ok(self.inner.add_connection(&p))
    }

    #[allow(non_snake_case)]
    fn RemoveConnection(&self, source: &str) -> PyResult<bool> {
        let p = path_from_str(source)?;
        Ok(self.inner.remove_connection(&p))
    }

    #[allow(non_snake_case)]
    fn SetConnections(&self, sources: Vec<String>) -> PyResult<bool> {
        let paths: PyResult<Vec<Path>> = sources.iter().map(|s| path_from_str(s)).collect();
        Ok(self.inner.set_connections(paths?))
    }

    #[allow(non_snake_case)]
    fn ClearConnections(&self) -> bool {
        self.inner.clear_connections()
    }

    #[allow(non_snake_case)]
    fn HasConnections(&self) -> bool {
        !self.inner.get_connections().is_empty()
    }

    // -- Color space -------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetColorSpace(&self) -> String {
        self.inner.get_color_space().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn SetColorSpace(&self, color_space: &str) {
        let token = Token::new(color_space);
        self.inner.set_color_space(&token);
    }

    #[allow(non_snake_case)]
    fn HasColorSpace(&self) -> bool {
        !self.inner.get_color_space().is_empty()
    }

    #[allow(non_snake_case)]
    fn ClearColorSpace(&self) {
        self.inner.clear_color_space();
    }

    // -- Custom flag -------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsCustom(&self) -> bool {
        self.inner.as_property().is_custom()
    }

    #[allow(non_snake_case)]
    fn SetCustom(&self, custom: bool) -> bool {
        self.inner.as_property().set_custom(custom)
    }

    // -- Metadata ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        attr_get_metadata(py, &self.inner, key)
    }

    #[allow(non_snake_case)]
    fn SetMetadata(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let token = Token::new(key);
        let val = py_to_value(value)?;
        Ok(self.inner.set_metadata(&token, val))
    }

    #[allow(non_snake_case)]
    fn HasMetadata(&self, key: &str) -> bool {
        self.inner.get_metadata(&Token::new(key)).is_some()
    }

    #[allow(non_snake_case)]
    fn ClearMetadata(&self, key: &str) -> bool {
        self.inner.clear_metadata(&Token::new(key))
    }

    // -- Display -----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetDisplayGroup(&self) -> String {
        self.inner
            .get_metadata(&Token::new("displayGroup"))
            .and_then(|v| v.downcast_clone::<String>())
            .unwrap_or_default()
    }

    #[allow(non_snake_case)]
    fn SetDisplayGroup(&self, group: &str) -> bool {
        self.inner.set_metadata(&Token::new("displayGroup"), Value::from(group.to_string()))
    }

    #[allow(non_snake_case)]
    fn GetDisplayName(&self) -> String {
        self.inner
            .get_metadata(&Token::new("displayName"))
            .and_then(|v| v.downcast_clone::<String>())
            .unwrap_or_default()
    }

    #[allow(non_snake_case)]
    fn SetDisplayName(&self, name: &str) -> bool {
        self.inner.set_metadata(&Token::new("displayName"), Value::from(name.to_string()))
    }

    // -- Authoring status --------------------------------------------------

    #[allow(non_snake_case)]
    fn IsAuthored(&self) -> bool {
        self.inner.as_property().is_authored()
    }

    #[allow(non_snake_case)]
    fn IsDefined(&self) -> bool {
        self.inner.is_valid()
    }

    // -- Repr --------------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("Usd.Attribute({})", self.inner.path().as_str())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &PyAttribute) -> bool {
        self.inner.path() == other.inner.path()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// UsdRelationship
// ============================================================================

/// A property that holds paths to other prims/properties.
///
/// Matches C++ `UsdRelationship`.
#[pyclass(skip_from_py_object,name = "Relationship", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyRelationship {
    inner: Relationship,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PyRelationship {
    // -- Validity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.inner.is_valid()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }

    // -- Identity ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetPath(&self) -> String {
        self.inner.path().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetName(&self) -> String {
        self.inner.name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetBaseName(&self) -> String {
        self.inner.as_property().base_name().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetNamespace(&self) -> String {
        self.inner.as_property().namespace().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn GetPrimPath(&self) -> String {
        self.inner.prim_path().as_str().to_string()
    }

    // -- Targets -----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetTargets(&self) -> Vec<String> {
        self.inner.get_targets().iter().map(|p| p.as_str().to_string()).collect()
    }

    #[allow(non_snake_case)]
    fn GetForwardedTargets(&self) -> Vec<String> {
        self.inner.get_forwarded_targets().iter().map(|p| p.as_str().to_string()).collect()
    }

    #[allow(non_snake_case)]
    fn AddTarget(&self, target: &str) -> PyResult<bool> {
        let p = path_from_str(target)?;
        Ok(self.inner.add_target(&p))
    }

    #[allow(non_snake_case)]
    fn RemoveTarget(&self, target: &str) -> PyResult<bool> {
        let p = path_from_str(target)?;
        Ok(self.inner.remove_target(&p))
    }

    #[allow(non_snake_case)]
    fn SetTargets(&self, targets: Vec<String>) -> PyResult<bool> {
        let paths: PyResult<Vec<Path>> = targets.iter().map(|s| path_from_str(s)).collect();
        Ok(self.inner.set_targets(&paths?))
    }

    #[allow(non_snake_case)]
    fn ClearTargets(&self, _remove_spec: bool) -> bool {
        // C++ ClearTargets(bool removeSpec): we delegate to clear_targets which removes spec
        self.inner.clear_targets()
    }

    #[allow(non_snake_case)]
    fn HasTargets(&self) -> bool {
        !self.inner.get_targets().is_empty()
    }

    // -- Custom flag -------------------------------------------------------

    #[allow(non_snake_case)]
    fn IsCustom(&self) -> bool {
        self.inner.as_property().is_custom()
    }

    #[allow(non_snake_case)]
    fn SetCustom(&self, custom: bool) -> bool {
        self.inner.as_property().set_custom(custom)
    }

    // -- Authoring status --------------------------------------------------

    #[allow(non_snake_case)]
    fn IsAuthored(&self) -> bool {
        self.inner.as_property().is_authored()
    }

    #[allow(non_snake_case)]
    fn IsDefined(&self) -> bool {
        self.inner.is_valid()
    }

    // -- Metadata ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        let token = Token::new(key);
        match self.inner.as_property().get_metadata(&token) {
            Some(v) => Ok(value_to_py(py, &v)),
            None => Ok(py.None()),
        }
    }

    #[allow(non_snake_case)]
    fn SetMetadata(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let token = Token::new(key);
        let val = py_to_value(value)?;
        Ok(self.inner.as_property().set_metadata(&token, val))
    }

    #[allow(non_snake_case)]
    fn HasMetadata(&self, key: &str) -> bool {
        self.inner.as_property().get_metadata(&Token::new(key)).is_some()
    }

    #[allow(non_snake_case)]
    fn ClearMetadata(&self, key: &str) -> bool {
        self.inner.as_property().clear_metadata(&Token::new(key))
    }

    // -- Display -----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetDisplayGroup(&self) -> String {
        self.inner.as_property()
            .get_metadata(&Token::new("displayGroup"))
            .and_then(|v| v.downcast_clone::<String>())
            .unwrap_or_default()
    }

    #[allow(non_snake_case)]
    fn SetDisplayGroup(&self, group: &str) -> bool {
        self.inner.as_property().set_metadata(
            &Token::new("displayGroup"),
            Value::from(group.to_string()),
        )
    }

    // -- Repr --------------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("Usd.Relationship({})", self.inner.path().as_str())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }

    fn __eq__(&self, other: &PyRelationship) -> bool {
        self.inner.path() == other.inner.path()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.inner.path().as_str().hash(&mut h);
        h.finish()
    }
}

// ============================================================================
// UsdSchemaBase
// ============================================================================

/// Base class for all USD schema wrappers.
///
/// Matches C++ `UsdSchemaBase`.
#[pyclass(skip_from_py_object,name = "SchemaBase", module = "pxr_rs.Usd")]
pub struct PySchemaBase {
    prim: Prim,
    _stage: Arc<Stage>,
}

#[pymethods]
impl PySchemaBase {
    #[allow(non_snake_case)]
    fn GetPrim(&self) -> PyPrim {
        PyPrim::from_prim(self.prim.clone(), self._stage.clone())
    }

    #[allow(non_snake_case)]
    fn GetPath(&self) -> String {
        self.prim.get_path().as_str().to_string()
    }

    #[allow(non_snake_case)]
    fn IsValid(&self) -> bool {
        self.prim.is_valid()
    }
}

// ============================================================================
// UsdStage
// ============================================================================

/// The outermost container for scene description.
///
/// Matches C++ `UsdStage`.
#[pyclass(skip_from_py_object,name = "Stage", module = "pxr_rs.Usd")]
pub struct PyStage {
    pub(crate) inner: Arc<Stage>,
}

#[pymethods]
impl PyStage {
    // -- Factory methods (static) -----------------------------------------

    /// Create a new stage backed by a new file at `identifier`.
    #[staticmethod]
    #[pyo3(signature = (identifier, load = "LoadAll"))]
    #[allow(non_snake_case)]
    fn CreateNew(identifier: &str, load: &str) -> PyResult<Self> {
        let load_set = parse_load(load)?;
        Stage::create_new(identifier, load_set)
            .map(|s| Self { inner: s })
            .map_err(to_py_err)
    }

    /// Create a new stage with an anonymous (in-memory) root layer.
    ///
    /// Accepts optional `identifier` and optional `sessionLayer` (keyword).
    /// `sessionLayer` can be a `Sdf.Layer` or `None`.
    #[staticmethod]
    #[pyo3(signature = (identifier = None, load = "LoadAll", **kwargs))]
    #[allow(non_snake_case)]
    fn CreateInMemory(
        identifier: Option<&str>,
        load: &str,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let load_set = parse_load(load)?;

        // Extract sessionLayer from kwargs if provided
        let session_layer: Option<Arc<usd_sdf::Layer>> = if let Some(kw) = kwargs {
            if let Some(val) = kw.get_item("sessionLayer")? {
                if val.is_none() {
                    None
                } else {
                    let py_layer: crate::sdf::PyLayer = val.extract()?;
                    Some(py_layer.layer().clone())
                }
            } else {
                None
            }
        } else {
            None
        };

        let stage = match (identifier, session_layer) {
            (Some(id), Some(sess)) => {
                Stage::create_in_memory_with_session(id, sess, load_set)
            }
            (Some(id), None) => Stage::create_in_memory_with_identifier(id, load_set),
            (None, _) => Stage::create_in_memory(load_set),
        }
        .map_err(to_py_err)?;
        Ok(Self { inner: stage })
    }

    /// Open an existing stage from a file path or a `Sdf.Layer`.
    ///
    /// Accepts `Usd.Stage.Open(str)`, `Usd.Stage.Open(Sdf.Layer)`,
    /// and optional `sessionLayer` keyword.
    #[staticmethod]
    #[pyo3(signature = (file_path, load = "LoadAll", **kwargs))]
    #[allow(non_snake_case)]
    fn Open(
        file_path: &Bound<'_, PyAny>,
        load: &str,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let load_set = parse_load(load)?;

        // Extract optional sessionLayer from kwargs
        let session_layer: Option<Arc<usd_sdf::Layer>> = if let Some(kw) = kwargs {
            if let Some(val) = kw.get_item("sessionLayer")? {
                if val.is_none() {
                    None
                } else {
                    let py_layer: crate::sdf::PyLayer = val.extract()?;
                    Some(py_layer.layer().clone())
                }
            } else {
                None
            }
        } else {
            None
        };

        // Accept either a string path or a Layer object
        if let Ok(py_layer) = file_path.extract::<crate::sdf::PyLayer>() {
            let root_layer = py_layer.layer().clone();
            match session_layer {
                Some(sess) => Stage::open_with_root_and_session_layer(root_layer, sess, load_set),
                None => Stage::open_with_root_layer(root_layer, load_set),
            }
            .map(|s| Self { inner: s })
            .map_err(to_py_err)
        } else if let Ok(path_str) = file_path.extract::<String>() {
            // session_layer not usable with file path open (would need layer lookup)
            Stage::open(&path_str, load_set)
                .map(|s| Self { inner: s })
                .map_err(to_py_err)
        } else {
            Err(PyValueError::new_err(
                "Stage.Open() expects a file path (str) or an Sdf.Layer",
            ))
        }
    }

    /// Open an existing stage with a population mask.
    #[staticmethod]
    #[pyo3(signature = (file_path, population_mask, load = "LoadAll"))]
    #[allow(non_snake_case)]
    fn OpenMasked(
        file_path: &str,
        population_mask: &PyStagePopulationMask,
        load: &str,
    ) -> PyResult<Self> {
        let load_set = parse_load(load)?;
        Stage::open_masked(file_path, population_mask.inner.clone(), load_set)
            .map(|s| Self { inner: s })
            .map_err(to_py_err)
    }

    /// Returns true if the given file can be opened as a USD stage.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsSupportedFile(file_path: &str) -> bool {
        Stage::is_supported_file(file_path)
    }

    // -- Class-level load policy constants (mirrors C++ Usd.Stage.LoadAll etc.)
    // These return string sentinels consumed by parse_load().

    #[classattr]
    #[allow(non_snake_case)]
    fn LoadAll() -> &'static str { "LoadAll" }

    #[classattr]
    #[allow(non_snake_case)]
    fn LoadNone() -> &'static str { "LoadNone" }

    // -- Prim access -------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetPrimAtPath(&self, path: &str) -> PyResult<Option<PyPrim>> {
        let p = path_from_str(path)?;
        Ok(self.inner.get_prim_at_path(&p).map(|prim| {
            PyPrim::from_prim(prim, self.inner.clone())
        }))
    }

    #[allow(non_snake_case)]
    fn GetDefaultPrim(&self) -> Option<PyPrim> {
        let prim = self.inner.get_default_prim();
        if prim.is_valid() {
            Some(PyPrim::from_prim(prim, self.inner.clone()))
        } else {
            None
        }
    }

    #[allow(non_snake_case)]
    fn SetDefaultPrim(&self, prim: &PyPrim) -> bool {
        self.inner.set_default_prim(&prim.inner)
    }

    #[allow(non_snake_case)]
    fn HasDefaultPrim(&self) -> bool {
        self.inner.get_default_prim().is_valid()
    }

    #[allow(non_snake_case)]
    fn ClearDefaultPrim(&self) {
        self.inner.clear_default_prim();
    }

    #[allow(non_snake_case)]
    fn GetPseudoRoot(&self) -> PyPrim {
        PyPrim::from_prim(self.inner.get_pseudo_root(), self.inner.clone())
    }

    // -- Authoring ---------------------------------------------------------

    #[pyo3(signature = (path, type_name = ""))]
    #[allow(non_snake_case)]
    fn DefinePrim(&self, path: &str, type_name: &str) -> PyResult<PyPrim> {
        self.inner
            .define_prim(path, type_name)
            .map(|p| PyPrim::from_prim(p, self.inner.clone()))
            .map_err(to_py_err)
    }

    #[allow(non_snake_case)]
    fn OverridePrim(&self, path: &str) -> PyResult<PyPrim> {
        self.inner
            .override_prim(path)
            .map(|p| PyPrim::from_prim(p, self.inner.clone()))
            .map_err(to_py_err)
    }

    #[allow(non_snake_case)]
    fn RemovePrim(&self, path: &str) -> PyResult<bool> {
        let p = path_from_str(path)?;
        Ok(self.inner.remove_prim(&p))
    }

    // -- Traversal ---------------------------------------------------------

    #[allow(non_snake_case)]
    fn Traverse(&self, py: Python<'_>) -> PyPrimRange {
        let range = self.inner.traverse();
        let prims: Vec<Prim> = range.into_iter().collect();
        PyPrimRange::from_prims(py, prims, self.inner.clone())
    }

    #[allow(non_snake_case)]
    fn TraverseAll(&self, py: Python<'_>) -> PyPrimRange {
        let range = self.inner.traverse_all();
        let prims: Vec<Prim> = range.into_iter().collect();
        PyPrimRange::from_prims(py, prims, self.inner.clone())
    }

    // -- Layers ------------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetRootLayer(&self) -> crate::sdf::PyLayer {
        crate::sdf::PyLayer::from_layer_arc(self.inner.get_root_layer().clone())
    }

    #[allow(non_snake_case)]
    fn GetRootLayerIdentifier(&self) -> String {
        self.inner.get_root_layer().identifier().to_string()
    }

    /// Returns true if the given layer is part of the local layer stack.
    #[allow(non_snake_case)]
    fn HasLocalLayer(&self, layer: &crate::sdf::PyLayer) -> bool {
        let target_id = layer.layer().identifier().to_string();
        self.inner.layer_stack().iter().any(|l| l.identifier() == target_id)
    }

    #[allow(non_snake_case)]
    fn GetSessionLayer(&self) -> Option<crate::sdf::PyLayer> {
        self.inner.get_session_layer().map(|l| crate::sdf::PyLayer::from_layer_arc(l))
    }

    #[pyo3(signature = (includeSessionLayers = true))]
    #[allow(non_snake_case)]
    fn GetLayerStack(&self, py: Python<'_>, includeSessionLayers: bool) -> Py<PyAny> {
        let layers: Vec<crate::sdf::PyLayer> = self.inner.layer_stack()
            .iter()
            .filter(|l| includeSessionLayers || !l.is_anonymous())
            .map(|l| crate::sdf::PyLayer::from_layer_arc(l.clone()))
            .collect();
        PyList::new(py, layers).map(|l| l.into_any().unbind()).unwrap_or_else(|_| py.None())
    }

    #[allow(non_snake_case)]
    fn MuteLayer(&self, layer_identifier: &str) {
        self.inner.mute_layer(layer_identifier);
    }

    #[allow(non_snake_case)]
    fn UnmuteLayer(&self, layer_identifier: &str) {
        self.inner.unmute_layer(layer_identifier);
    }

    #[allow(non_snake_case)]
    fn GetMutedLayers(&self) -> Vec<String> {
        self.inner.get_muted_layers().into_iter().collect()
    }

    #[allow(non_snake_case)]
    fn IsLayerMuted(&self, layer_identifier: &str) -> bool {
        self.inner.is_layer_muted(layer_identifier)
    }

    // -- Edit target -------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetEditTarget(&self) -> PyEditTarget {
        PyEditTarget { inner: self.inner.get_edit_target() }
    }

    #[allow(non_snake_case)]
    fn SetEditTarget(&self, target: &PyEditTarget) {
        self.inner.set_edit_target(target.inner.clone());
    }

    // -- Time --------------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetStartTimeCode(&self) -> f64 {
        self.inner.get_start_time_code()
    }

    #[allow(non_snake_case)]
    fn SetStartTimeCode(&self, time_code: f64) {
        self.inner.set_start_time_code(time_code);
    }

    #[allow(non_snake_case)]
    fn GetEndTimeCode(&self) -> f64 {
        self.inner.get_end_time_code()
    }

    #[allow(non_snake_case)]
    fn SetEndTimeCode(&self, time_code: f64) {
        self.inner.set_end_time_code(time_code);
    }

    #[allow(non_snake_case)]
    fn GetTimeCodesPerSecond(&self) -> f64 {
        self.inner.get_time_codes_per_second()
    }

    #[allow(non_snake_case)]
    fn SetTimeCodesPerSecond(&self, tcps: f64) {
        self.inner.set_time_codes_per_second(tcps);
    }

    #[allow(non_snake_case)]
    fn GetFramesPerSecond(&self) -> f64 {
        self.inner.get_frames_per_second()
    }

    #[allow(non_snake_case)]
    fn SetFramesPerSecond(&self, fps: f64) {
        self.inner.set_frames_per_second(fps);
    }

    // -- I/O ---------------------------------------------------------------

    /// Save all dirty, non-anonymous layers to disk.
    #[allow(non_snake_case)]
    fn Save(&self) -> PyResult<()> {
        self.inner.save().map(|_| ()).map_err(to_py_err)
    }

    /// Export the stage to a file.
    #[pyo3(signature = (file_path, add_source_file_comment = true))]
    #[allow(non_snake_case)]
    fn Export(&self, file_path: &str, add_source_file_comment: bool) -> PyResult<bool> {
        self.inner.export(file_path, add_source_file_comment).map_err(to_py_err)
    }

    /// Export the stage to a string.
    #[pyo3(signature = (add_source_file_comment = true))]
    #[allow(non_snake_case)]
    fn ExportToString(&self, add_source_file_comment: bool) -> PyResult<String> {
        self.inner.export_to_string(add_source_file_comment).map_err(to_py_err)
    }

    /// Reload all layers in the layer stack from disk.
    #[allow(non_snake_case)]
    fn Reload(&self) -> PyResult<()> {
        self.inner.reload().map_err(to_py_err)
    }

    /// Save all session layers.
    #[allow(non_snake_case)]
    fn SaveSessionLayers(&self) -> PyResult<()> {
        self.inner.save_session_layers().map_err(to_py_err)
    }

    // -- Loading -----------------------------------------------------------

    #[pyo3(signature = (path, policy = "LoadWithDescendants"))]
    #[allow(non_snake_case)]
    fn Load(&self, path: &str, policy: &str) -> PyResult<PyPrim> {
        let p = path_from_str(path)?;
        let pol = parse_load_policy(policy)?;
        let prim = self.inner.load(&p, Some(pol));
        Ok(PyPrim::from_prim(prim, self.inner.clone()))
    }

    #[allow(non_snake_case)]
    fn Unload(&self, path: &str) -> PyResult<()> {
        let p = path_from_str(path)?;
        self.inner.unload(&p);
        Ok(())
    }

    #[allow(non_snake_case)]
    fn GetPrototypes(&self) -> Vec<PyPrim> {
        self.inner
            .get_prototypes()
            .into_iter()
            .map(|p| PyPrim::from_prim(p, self.inner.clone()))
            .collect()
    }

    // -- Metadata ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        let token = Token::new(key);
        match self.inner.get_metadata(&token) {
            Some(v) => Ok(value_to_py(py, &v)),
            None => Ok(py.None()),
        }
    }

    #[allow(non_snake_case)]
    fn SetMetadata(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let token = Token::new(key);
        let val = py_to_value(value)?;
        Ok(self.inner.set_metadata(&token, val))
    }

    #[allow(non_snake_case)]
    fn HasMetadata(&self, key: &str) -> bool {
        self.inner.has_metadata(&Token::new(key))
    }

    #[allow(non_snake_case)]
    fn ClearMetadata(&self, key: &str) -> bool {
        self.inner.clear_metadata(&Token::new(key))
    }

    // -- Global variant fallbacks -----------------------------------------

    #[staticmethod]
    #[allow(non_snake_case)]
    fn GetGlobalVariantFallbacks(py: Python<'_>) -> Py<PyAny> {
        let fb = Stage::get_global_variant_fallbacks();
        let dict = PyDict::new(py);
        for (k, v) in fb {
            let vals: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
            let _ = dict.set_item(k.as_str(), vals);
        }
        dict.into_any().unbind()
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn SetGlobalVariantFallbacks(fallbacks: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut map = std::collections::HashMap::new();
        for (k, v) in fallbacks {
            let key: String = k.extract()?;
            let vals: Vec<String> = v.extract()?;
            let tokens: Vec<Token> = vals.iter().map(|s| Token::new(s)).collect();
            map.insert(Token::new(&key), tokens);
        }
        Stage::set_global_variant_fallbacks(&map);
        Ok(())
    }

    // -- Flatten -----------------------------------------------------------

    // -- CreateClassPrim ---------------------------------------------------

    /// Create a class prim at the given path.
    ///
    /// Matches C++ `UsdStage::CreateClassPrim`.
    #[allow(non_snake_case)]
    fn CreateClassPrim(&self, path: &str) -> PyResult<PyPrim> {
        self.inner
            .create_class_prim(path)
            .map(|p| PyPrim::from_prim(p, self.inner.clone()))
            .map_err(to_py_err)
    }

    // -- GetAttributeAtPath / GetRelationshipAtPath / GetPropertyAtPath ----

    /// Get an attribute by its full path (e.g. `/Foo.bar`).
    #[allow(non_snake_case)]
    fn GetAttributeAtPath(&self, path: &str) -> PyResult<Option<PyAttribute>> {
        let p = path_from_str(path)?;
        Ok(self.inner.get_attribute_at_path(&p)
            .map(|a| PyAttribute { inner: a, _stage: self.inner.clone() }))
    }

    /// Get a relationship by its full path.
    #[allow(non_snake_case)]
    fn GetRelationshipAtPath(&self, path: &str) -> PyResult<Option<PyRelationship>> {
        let p = path_from_str(path)?;
        Ok(self.inner.get_relationship_at_path(&p)
            .map(|r| PyRelationship { inner: r, _stage: self.inner.clone() }))
    }

    // -- GetUsedLayers ----------------------------------------------------

    /// Returns all layers used by this stage (including clip layers).
    #[pyo3(signature = (include_clip_layers = true))]
    #[allow(non_snake_case)]
    fn GetUsedLayers(&self, include_clip_layers: bool) -> Vec<crate::sdf::PyLayer> {
        self.inner
            .get_used_layers(include_clip_layers)
            .into_iter()
            .map(|l| crate::sdf::PyLayer::from_layer_arc(l))
            .collect()
    }

    // -- GetCompositionErrors ---------------------------------------------

    /// Returns composition errors accumulated during stage load.
    #[allow(non_snake_case)]
    fn GetCompositionErrors(&self) -> Vec<String> {
        self.inner.get_composition_errors()
    }

    // -- MuteAndUnmuteLayers ----------------------------------------------

    /// Simultaneously mute and unmute layers.
    #[allow(non_snake_case)]
    fn MuteAndUnmuteLayers(&self, mute_layers: Vec<String>, unmute_layers: Vec<String>) {
        self.inner.mute_and_unmute_layers(&mute_layers, &unmute_layers);
    }

    // -- GetEditTargetForLocalLayer ----------------------------------------

    /// Return an edit target that directs to the given local layer.
    #[allow(non_snake_case)]
    fn GetEditTargetForLocalLayer(&self, layer: &crate::sdf::PyLayer) -> PyEditTarget {
        let et = self.inner.get_edit_target_for_local_layer(layer.layer());
        PyEditTarget { inner: et }
    }

    // -- GetPopulationMask / SetPopulationMask ----------------------------

    #[allow(non_snake_case)]
    fn GetPopulationMask(&self) -> PyStagePopulationMask {
        PyStagePopulationMask {
            inner: self.inner.get_population_mask().unwrap_or_else(StagePopulationMask::new),
        }
    }

    #[allow(non_snake_case)]
    fn SetPopulationMask(&self, mask: &PyStagePopulationMask) {
        self.inner.set_population_mask(Some(mask.inner.clone()));
    }

    // -- Flatten -----------------------------------------------------------

    /// Flatten the stage into a single layer and return it as a Layer.
    ///
    /// Matches C++ `UsdStage::Flatten`.
    #[pyo3(signature = (add_source_file_comment = true))]
    #[allow(non_snake_case)]
    fn Flatten(&self, add_source_file_comment: bool) -> PyResult<crate::sdf::PyLayer> {
        self.inner
            .flatten(add_source_file_comment)
            .map(|layer| crate::sdf::PyLayer::from_layer_arc(layer))
            .map_err(to_py_err)
    }

    /// Flatten the stage to a string representation.
    #[pyo3(signature = (add_source_file_comment = true))]
    #[allow(non_snake_case)]
    fn FlattenToString(&self, add_source_file_comment: bool) -> PyResult<String> {
        self.inner
            .flatten(add_source_file_comment)
            .map_err(to_py_err)
            .and_then(|layer| layer.export_to_string().map_err(to_py_err))
    }

    // -- Repr --------------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("Usd.Stage.Open('{}')", self.inner.get_root_layer().identifier())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

// ============================================================================
// Load policy helpers
// ============================================================================

fn parse_load(s: &str) -> PyResult<InitialLoadSet> {
    match s {
        "LoadAll" | "load_all" => Ok(InitialLoadSet::LoadAll),
        "LoadNone" | "load_none" => Ok(InitialLoadSet::LoadNone),
        _ => Err(PyValueError::new_err(format!(
            "Unknown load parameter '{s}': use 'LoadAll' or 'LoadNone'"
        ))),
    }
}

fn parse_load_policy(s: &str) -> PyResult<usd_core::common::LoadPolicy> {
    match s {
        "LoadWithDescendants" | "load_with_descendants" => {
            Ok(usd_core::common::LoadPolicy::LoadWithDescendants)
        }
        "LoadWithoutDescendants" | "load_without_descendants" => {
            Ok(usd_core::common::LoadPolicy::LoadWithoutDescendants)
        }
        _ => Err(PyValueError::new_err(format!(
            "Unknown load policy '{s}': use 'LoadWithDescendants' or 'LoadWithoutDescendants'"
        ))),
    }
}

// ============================================================================
// PrimResyncType enum — mirrors C++ UsdNotice::ObjectsChanged::PrimResyncType
// ============================================================================

/// Classification of prim resync operations.
///
/// Matches C++ `UsdNotice::ObjectsChanged::PrimResyncType`.
#[pyclass(skip_from_py_object, name = "PrimResyncType", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyPrimResyncType {
    value: i32,
}

#[pymethods]
impl PyPrimResyncType {
    // Enum values as class attributes — accessible as PrimResyncType.Delete etc.
    #[classattr]
    #[allow(non_snake_case)]
    fn RenameSource() -> Self { Self { value: 0 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn RenameDestination() -> Self { Self { value: 1 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn ReparentSource() -> Self { Self { value: 2 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn ReparentDestination() -> Self { Self { value: 3 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn RenameAndReparentSource() -> Self { Self { value: 4 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn RenameAndReparentDestination() -> Self { Self { value: 5 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn Delete() -> Self { Self { value: 6 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn UnchangedPrimStack() -> Self { Self { value: 7 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn Other() -> Self { Self { value: 8 } }
    #[classattr]
    #[allow(non_snake_case)]
    fn Invalid() -> Self { Self { value: 9 } }

    fn __repr__(&self) -> String {
        let name = match self.value {
            0 => "RenameSource",
            1 => "RenameDestination",
            2 => "ReparentSource",
            3 => "ReparentDestination",
            4 => "RenameAndReparentSource",
            5 => "RenameAndReparentDestination",
            6 => "Delete",
            7 => "UnchangedPrimStack",
            8 => "Other",
            _ => "Invalid",
        };
        format!("Usd.Notice.ObjectsChanged.PrimResyncType.{name}")
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.value == other.value
    }

    fn __hash__(&self) -> u64 {
        self.value as u64
    }
}

// ============================================================================
// ObjectsChanged notice — accessible as Usd.Notice.ObjectsChanged
// ============================================================================

/// Stub for `UsdNotice::ObjectsChanged`.
///
/// Provides `PrimResyncType` enum and the notice type for listener registration.
#[pyclass(skip_from_py_object, name = "ObjectsChanged", module = "pxr_rs.Usd")]
pub struct PyObjectsChanged;

#[pymethods]
impl PyObjectsChanged {
    fn __repr__(&self) -> &str {
        "Usd.Notice.ObjectsChanged"
    }
}

// ============================================================================
// StageContentsChanged / StageEditTargetChanged notice stubs
// ============================================================================

/// Stub for `UsdNotice::StageContentsChanged`.
#[pyclass(skip_from_py_object, name = "StageContentsChanged", module = "pxr_rs.Usd")]
pub struct PyStageContentsChanged;

/// Stub for `UsdNotice::StageEditTargetChanged`.
#[pyclass(skip_from_py_object, name = "StageEditTargetChanged", module = "pxr_rs.Usd")]
pub struct PyStageEditTargetChanged;

// ============================================================================
// UsdNotice — container exposing notice types
// ============================================================================

/// Notice types for USD change notification.
///
/// Matches C++ `UsdNotice`. Sub-types are class attributes.
#[pyclass(skip_from_py_object, name = "Notice", module = "pxr_rs.Usd")]
pub struct PyNotice;

/// Manually register `ObjectsChanged` etc. as class-level attributes on Notice.
/// Called from the module register function with access to `py`.
fn register_notice_attrs(py: Python<'_>) -> PyResult<()> {
    let notice_type = py.get_type::<PyNotice>();

    // ObjectsChanged class with PrimResyncType enum values
    let oc_type = py.get_type::<PyObjectsChanged>();
    // Attach PrimResyncType class itself so tests can do
    //   Usd.Notice.ObjectsChanged.PrimResyncType (gets the type)
    //   and compare instances like PrimResyncType.Delete == PrimResyncType.Delete
    let prt_type = py.get_type::<PyPrimResyncType>();
    oc_type.setattr("PrimResyncType", prt_type)?;
    notice_type.setattr("ObjectsChanged", oc_type)?;

    // Other notice types as class attrs
    let sc_type = py.get_type::<PyStageContentsChanged>();
    notice_type.setattr("StageContentsChanged", sc_type)?;
    let set_type = py.get_type::<PyStageEditTargetChanged>();
    notice_type.setattr("StageEditTargetChanged", set_type)?;

    Ok(())
}

// ============================================================================
// SchemaRegistry — mirrors C++ UsdSchemaRegistry
// ============================================================================

/// Version policy for schema family queries.
///
/// Matches C++ `UsdSchemaRegistry::VersionPolicy`.
#[pyclass(skip_from_py_object, name = "VersionPolicy", module = "pxr_rs.Usd")]
#[derive(Clone)]
pub struct PyVersionPolicy {
    value: i32,
}

#[pymethods]
impl PyVersionPolicy {
    #[classattr] #[allow(non_snake_case)]
    fn All() -> Self { Self { value: 0 } }
    #[classattr] #[allow(non_snake_case)]
    fn GreaterThan() -> Self { Self { value: 1 } }
    #[classattr] #[allow(non_snake_case)]
    fn GreaterThanOrEqual() -> Self { Self { value: 2 } }
    #[classattr] #[allow(non_snake_case)]
    fn LessThan() -> Self { Self { value: 3 } }
    #[classattr] #[allow(non_snake_case)]
    fn LessThanOrEqual() -> Self { Self { value: 4 } }

    fn __repr__(&self) -> String {
        let name = match self.value {
            0 => "All", 1 => "GreaterThan", 2 => "GreaterThanOrEqual",
            3 => "LessThan", _ => "LessThanOrEqual",
        };
        format!("Usd.SchemaRegistry.VersionPolicy.{name}")
    }

    fn __eq__(&self, other: &Self) -> bool { self.value == other.value }
    fn __hash__(&self) -> u64 { self.value as u64 }
}

/// Singleton registry of schema types and definitions.
///
/// Matches C++ `UsdSchemaRegistry`.
#[pyclass(skip_from_py_object, name = "SchemaRegistry", module = "pxr_rs.Usd")]
pub struct PySchemaRegistry;

#[pymethods]
impl PySchemaRegistry {
    /// Parse a schema family name and version from an identifier string.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn ParseSchemaFamilyAndVersionFromIdentifier(
        identifier: &str,
    ) -> (String, u32) {
        // Simple parsing: if ends with _N, split off N as version
        if let Some(idx) = identifier.rfind('_') {
            if let Ok(ver) = identifier[idx + 1..].parse::<u32>() {
                return (identifier[..idx].to_string(), ver);
            }
        }
        (identifier.to_string(), 0)
    }

    /// Check if a schema family name is allowed.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsAllowedSchemaFamily(_family: &str) -> bool {
        true
    }

    /// Check if a schema identifier is allowed.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn IsAllowedSchemaIdentifier(_identifier: &str) -> bool {
        true
    }

    /// Build a schema identifier from family + version.
    #[staticmethod]
    #[allow(non_snake_case)]
    fn MakeSchemaIdentifierForFamilyAndVersion(
        family: &str,
        version: u32,
    ) -> String {
        if version == 0 {
            family.to_string()
        } else {
            format!("{family}_{version}")
        }
    }

    /// Find schema info by type or identifier. Returns None (stub).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn FindSchemaInfo(_id: &Bound<'_, PyAny>, _version: Option<u32>) -> Option<String> {
        None
    }

    /// Find all schema infos in a family. Returns empty list (stub).
    #[staticmethod]
    #[pyo3(signature = (family, version = 0, policy = None))]
    #[allow(non_snake_case)]
    fn FindSchemaInfosInFamily(
        family: &str,
        version: u32,
        policy: Option<&PyVersionPolicy>,
    ) -> Vec<String> {
        let _ = (family, version, policy);
        Vec::new()
    }

    fn __repr__(&self) -> &str {
        "Usd.SchemaRegistry"
    }
}

/// Register SchemaRegistry class attrs (VersionPolicy).
fn register_schema_registry_attrs(py: Python<'_>) -> PyResult<()> {
    let sr_type = py.get_type::<PySchemaRegistry>();
    let vp_type = py.get_type::<PyVersionPolicy>();
    sr_type.setattr("VersionPolicy", vp_type)?;
    Ok(())
}

// ============================================================================
// ColorSpaceHashCache — stub for subclassing in Python tests
// ============================================================================

/// Base class for color space caching.
///
/// Matches C++ `UsdColorSpaceHashCache`. Subclassable in Python.
#[pyclass(subclass, skip_from_py_object, name = "ColorSpaceHashCache", module = "pxr_rs.Usd")]
pub struct PyColorSpaceHashCache;

#[pymethods]
impl PyColorSpaceHashCache {
    #[new]
    fn new() -> Self { Self }

    /// Look up cached color space name for the given path.
    /// Returns None by default — override in Python subclass.
    #[allow(non_snake_case)]
    fn Find(&self, _path: &str) -> Option<String> {
        None
    }

    fn __repr__(&self) -> &str {
        "Usd.ColorSpaceHashCache()"
    }
}

// ============================================================================
// Module constants
// ============================================================================

/// Sentinel TimeCode values exposed as module-level constants to match C++.
#[allow(non_snake_case)]
fn add_constants(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("LoadAll", "LoadAll")?;
    m.add("LoadNone", "LoadNone")?;
    m.add("LoadWithDescendants", "LoadWithDescendants")?;
    m.add("LoadWithoutDescendants", "LoadWithoutDescendants")?;
    Ok(())
}

// ============================================================================
// Registration
// ============================================================================

/// Register all Usd submodule classes.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Core types
    m.add_class::<PyTimeCode>()?;
    m.add_class::<PyStagePopulationMask>()?;
    m.add_class::<PyEditTarget>()?;
    m.add_class::<PyEditContext>()?;
    m.add_class::<PyPrimRange>()?;

    // Schema base
    m.add_class::<PySchemaBase>()?;

    // Scene graph
    m.add_class::<PyPrim>()?;
    m.add_class::<PyAttribute>()?;
    m.add_class::<PyRelationship>()?;
    m.add_class::<PyVariantSets>()?;
    m.add_class::<PyVariantSet>()?;

    // Composition arc proxies
    m.add_class::<PyReferences>()?;
    m.add_class::<PyPayloads>()?;
    m.add_class::<PyInherits>()?;
    m.add_class::<PySpecializes>()?;

    // Stage
    m.add_class::<PyStage>()?;

    // Notice types
    m.add_class::<PyPrimResyncType>()?;
    m.add_class::<PyObjectsChanged>()?;
    m.add_class::<PyStageContentsChanged>()?;
    m.add_class::<PyStageEditTargetChanged>()?;
    m.add_class::<PyNotice>()?;
    // Wire up Notice.ObjectsChanged.PrimResyncType class hierarchy
    register_notice_attrs(py)?;

    // SchemaRegistry
    m.add_class::<PyVersionPolicy>()?;
    m.add_class::<PySchemaRegistry>()?;
    m.add("SchemaRegistry", py.get_type::<PySchemaRegistry>())?;
    register_schema_registry_attrs(py)?;

    // ColorSpaceHashCache
    m.add_class::<PyColorSpaceHashCache>()?;

    // Module-level constants
    add_constants(m)?;

    Ok(())
}
