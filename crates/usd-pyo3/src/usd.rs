//! pxr.Usd — Core USD Python bindings
//!
//! Drop-in replacement for the C++ OpenUSD `pxr.Usd` Python module.
//! Wraps usd-core Rust types with PyO3.

use pyo3::prelude::*;
use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError, PyValueError};
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

#[allow(deprecated)]
fn value_to_py(py: Python<'_>, val: &Value) -> PyObject {
    use pyo3::IntoPy;
    // Try common types; fall back to string repr
    if let Some(v) = val.downcast_clone::<bool>() {
        return v.into_py(py);
    }
    if let Some(v) = val.downcast_clone::<i32>() {
        return v.into_py(py);
    }
    if let Some(v) = val.downcast_clone::<i64>() {
        return v.into_py(py);
    }
    if let Some(v) = val.downcast_clone::<f32>() {
        return (v as f64).into_py(py);
    }
    if let Some(v) = val.downcast_clone::<f64>() {
        return v.into_py(py);
    }
    if let Some(v) = val.downcast_clone::<String>() {
        return v.into_py(py);
    }
    if let Some(v) = val.downcast_clone::<Token>() {
        return v.as_str().to_string().into_py(py);
    }
    if let Some(v) = val.downcast_clone::<Vec<f32>>() {
        return PyList::new(py, v).map(|l| l.into()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<f64>>() {
        return PyList::new(py, v).map(|l| l.into()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<i32>>() {
        return PyList::new(py, v).map(|l| l.into()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<Vec<String>>() {
        return PyList::new(py, v).map(|l| l.into()).unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<glam::Vec3>() {
        return PyTuple::new(py, [v.x as f64, v.y as f64, v.z as f64])
            .map(|t| t.into())
            .unwrap_or_else(|_| py.None());
    }
    if let Some(v) = val.downcast_clone::<glam::Vec2>() {
        return PyTuple::new(py, [v.x as f64, v.y as f64])
            .map(|t| t.into())
            .unwrap_or_else(|_| py.None());
    }
    if val.is_empty() {
        return py.None();
    }
    // Fallback: debug string
    format!("{val:?}").into_py(py)
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
#[pyclass(name = "TimeCode", module = "pxr.Usd")]
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
#[pyclass(name = "StagePopulationMask", module = "pxr.Usd")]
#[derive(Clone)]
pub struct PyStagePopulationMask {
    inner: StagePopulationMask,
}

#[pymethods]
impl PyStagePopulationMask {
    #[new]
    fn new() -> Self {
        Self { inner: StagePopulationMask::new() }
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

    fn __repr__(&self) -> String {
        let paths: Vec<_> = self.inner.get_paths().iter().map(|p| p.as_str().to_string()).collect();
        format!("Usd.StagePopulationMask([{}])", paths.join(", "))
    }
}

// ============================================================================
// UsdEditTarget
// ============================================================================

/// Specifies which layer should receive edits on a stage.
///
/// Matches C++ `UsdEditTarget`.
#[pyclass(name = "EditTarget", module = "pxr.Usd")]
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
#[pyclass(name = "EditContext", module = "pxr.Usd")]
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
#[pyclass(name = "PrimRange", module = "pxr.Usd")]
pub struct PyPrimRange {
    prims: Vec<PyObject>,
    index: usize,
}

#[pymethods]
impl PyPrimRange {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> Option<PyObject> {
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
    #[allow(deprecated)]
    fn from_prims(py: Python<'_>, prims: Vec<Prim>, stage_arc: Arc<Stage>) -> Self {
        use pyo3::IntoPy;
        let objs = prims
            .into_iter()
            .map(|p| {
                let py_prim = PyPrim::from_prim(p, stage_arc.clone());
                py_prim.into_py(py)
            })
            .collect();
        Self { prims: objs, index: 0 }
    }
}

// ============================================================================
// UsdObject base helpers (shared metadata logic)
// ============================================================================

fn prim_get_metadata(py: Python<'_>, prim: &Prim, key: &str) -> PyResult<PyObject> {
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

fn attr_get_metadata(py: Python<'_>, attr: &Attribute, key: &str) -> PyResult<PyObject> {
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
#[pyclass(name = "Prim", module = "pxr.Usd")]
#[derive(Clone)]
pub struct PyPrim {
    inner: Prim,
    // Keep the stage alive as long as this Python object exists.
    _stage: Arc<Stage>,
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

    // -- Metadata ----------------------------------------------------------

    #[allow(non_snake_case)]
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<PyObject> {
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
    fn GetAllMetadata(&self, py: Python<'_>) -> PyResult<PyObject> {
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
        Ok(dict.into())
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

#[pyclass(name = "VariantSets", module = "pxr.Usd")]
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
}

#[pyclass(name = "VariantSet", module = "pxr.Usd")]
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
}

// ============================================================================
// UsdAttribute
// ============================================================================

/// A typed, time-varying attribute on a prim.
///
/// Matches C++ `UsdAttribute`.
#[pyclass(name = "Attribute", module = "pxr.Usd")]
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
    #[allow(non_snake_case)]
    fn Get(&self, py: Python<'_>, time: Option<&Bound<'_, PyAny>>) -> PyResult<PyObject> {
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
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<PyObject> {
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
#[pyclass(name = "Relationship", module = "pxr.Usd")]
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
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<PyObject> {
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
#[pyclass(name = "SchemaBase", module = "pxr.Usd")]
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
#[pyclass(name = "Stage", module = "pxr.Usd")]
pub struct PyStage {
    inner: Arc<Stage>,
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
    #[staticmethod]
    #[pyo3(signature = (identifier = None, load = "LoadAll"))]
    #[allow(non_snake_case)]
    fn CreateInMemory(identifier: Option<&str>, load: &str) -> PyResult<Self> {
        let load_set = parse_load(load)?;
        let stage = match identifier {
            Some(id) => Stage::create_in_memory_with_identifier(id, load_set),
            None => Stage::create_in_memory(load_set),
        }
        .map_err(to_py_err)?;
        Ok(Self { inner: stage })
    }

    /// Open an existing stage from a file.
    #[staticmethod]
    #[pyo3(signature = (file_path, load = "LoadAll"))]
    #[allow(non_snake_case)]
    fn Open(file_path: &str, load: &str) -> PyResult<Self> {
        let load_set = parse_load(load)?;
        Stage::open(file_path, load_set)
            .map(|s| Self { inner: s })
            .map_err(to_py_err)
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
    fn GetRootLayer(&self) -> PyResult<PyObject> {
        // Return layer identifier string as proxy (full Layer wrapping deferred to Sdf module)
        Err(PyNotImplementedError::new_err("GetRootLayer: use stage.GetRootLayerIdentifier() for now"))
    }

    #[allow(non_snake_case)]
    fn GetRootLayerIdentifier(&self) -> String {
        self.inner.get_root_layer().identifier().to_string()
    }

    #[allow(non_snake_case)]
    fn GetSessionLayer(&self) -> Option<String> {
        self.inner.get_session_layer().map(|l| l.identifier().to_string())
    }

    #[allow(non_snake_case)]
    fn GetLayerStack(&self, py: Python<'_>) -> PyObject {
        let layers: Vec<String> = self.inner.layer_stack().iter().map(|l| l.identifier().to_string()).collect();
        PyList::new(py, layers).map(|l| l.into()).unwrap_or_else(|_| py.None())
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
    fn GetMetadata(&self, py: Python<'_>, key: &str) -> PyResult<PyObject> {
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
    fn GetGlobalVariantFallbacks(py: Python<'_>) -> PyObject {
        let fb = Stage::get_global_variant_fallbacks();
        let dict = PyDict::new(py);
        for (k, v) in fb {
            let vals: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
            let _ = dict.set_item(k.as_str(), vals);
        }
        dict.into()
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

    /// Flatten the stage into a single layer and return its string content.
    #[pyo3(signature = (add_source_file_comment = true))]
    #[allow(non_snake_case)]
    fn Flatten(&self, add_source_file_comment: bool) -> PyResult<String> {
        self.inner
            .flatten(add_source_file_comment)
            .map_err(to_py_err)
            .and_then(|layer| {
                layer.export_to_string().map_err(to_py_err)
            })
    }

    // -- Repr --------------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("Usd.Stage({})", self.inner.get_root_layer().identifier().to_string())
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
// UsdNotice stubs
// ============================================================================

/// Notice types for USD change notification.
///
/// Matches C++ `UsdNotice`.
#[pyclass(name = "Notice", module = "pxr.Usd")]
pub struct PyNotice;

#[pymethods]
impl PyNotice {
    #[staticmethod]
    #[allow(non_snake_case)]
    fn ObjectsChanged() -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "UsdNotice.ObjectsChanged requires a notice listener system",
        ))
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn StageContentsChanged() -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "UsdNotice.StageContentsChanged requires a notice listener system",
        ))
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    fn StageEditTargetChanged() -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "UsdNotice.StageEditTargetChanged requires a notice listener system",
        ))
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

    // Stage
    m.add_class::<PyStage>()?;

    // Notice stubs
    m.add_class::<PyNotice>()?;

    // Module-level constants
    add_constants(m)?;

    let _ = py;
    Ok(())
}
