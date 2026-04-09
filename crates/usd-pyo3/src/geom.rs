//! pxr.UsdGeom — Python bindings for the USD Geometry module.
//!
//! Drop-in replacement for `pxr.UsdGeom` from C++ OpenUSD.
//! All 38 schema classes, plus BBoxCache, XformCache, Primvar, XformOp, Tokens, Metrics.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyListMethods};
use pyo3::{PyRef, exceptions::PyException};
use usd_sdf::{Path, TimeCode};
use usd_tf::{TfType, Token};

// Qualified imports to avoid name collisions with #[pyfunction] names.
use usd_geom::metrics;
use usd_geom::{
    BBoxCache, BasisCurves, Boundable, Camera, Capsule, Capsule1, Cone, Cube, Curves, Cylinder,
    Cylinder1, Gprim, HermiteCurves, Imageable, Mesh, ModelAPI, MotionAPI, NurbsCurves, NurbsPatch,
    Plane, PointAndTangentArrays, PointBased, PointInstancer, Points, Primvar, PrimvarsAPI,
    RotationOrder, SHARPNESS_INFINITE, Scope, Sphere, Subset, TetMesh, VisibilityAPI, Xform,
    XformCache, XformCommonAPI, XformOp, XformOpPrecision, XformOpType, Xformable,
};
use usd_vt::Array as VtArray;

// ============================================================================
// Helpers
// ============================================================================

fn tc(t: Option<f64>) -> TimeCode {
    match t {
        Some(v) => TimeCode::new(v),
        None => TimeCode::default(),
    }
}

/// Points from `list` of 3-tuples / `Gf.Vec3f` / `Vt.Vec3fArray` — used by `PointBased.ComputeExtent` and extents hints.
fn f32_vec_from_py(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Vec<f32>> {
    if let Ok(a) = obj.extract::<PyRef<crate::vt::PyFloatArray>>() {
        return Ok(a.inner.as_slice().to_vec());
    }
    let list = obj.cast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(item.extract::<f64>()? as f32);
    }
    Ok(out)
}

fn vec3f_vec_from_points_arg(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Vec<usd_gf::Vec3f>> {
    if let Ok(a) = obj.extract::<PyRef<crate::vt::PyVec3fArray>>() {
        return Ok(a.inner.as_slice().iter().copied().collect());
    }
    let list = obj.cast::<PyList>()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        if let Ok(v) = item.extract::<PyRef<crate::gf::vec::PyVec3f>>() {
            out.push(v.0);
            continue;
        }
        let (x, y, z): (f64, f64, f64) = item.extract()?;
        out.push(usd_gf::Vec3f::new(x as f32, y as f32, z as f32));
    }
    Ok(out)
}

/// `Usd.TimeCode` / `float` / `None` → `Sdf.TimeCode` for xform APIs (matches pxr `GetLocalTransformation` time arg).
fn tc_from_py_opt(time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<TimeCode> {
    match time {
        None => Ok(TimeCode::default()),
        Some(o) => crate::usd::tc_from_py_sdf(o),
    }
}

fn parse_path(s: &str) -> PyResult<Path> {
    Path::from_string(s).ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{s}'")))
}

/// `Get` / `Define` path argument: `str` or `Sdf.Path` (pxr parity).
fn parse_path_py(path: &Bound<'_, pyo3::PyAny>) -> PyResult<Path> {
    if let Ok(s) = path.extract::<String>() {
        return parse_path(&s);
    }
    if let Ok(p) = path.extract::<pyo3::PyRef<'_, crate::sdf::PyPath>>() {
        return Ok(p.inner.clone());
    }
    Err(PyValueError::new_err("Path must be str or Sdf.Path"))
}

/// Maps to pxr `Tf.ErrorException` (see `pxr.Tf` registration).
fn raise_tf_coding_error_py(msg: impl Into<String>) -> PyErr {
    let msg = msg.into();
    usd_tf::issue_error(
        usd_tf::CallContext::empty().hide(),
        usd_tf::DiagnosticType::CodingError,
        msg.clone(),
    );
    PyException::new_err(msg)
}

fn raise_tf_runtime_error_py(msg: impl Into<String>) -> PyErr {
    let msg = msg.into();
    usd_tf::issue_error(
        usd_tf::CallContext::empty().hide(),
        usd_tf::DiagnosticType::RuntimeError,
        msg.clone(),
    );
    PyException::new_err(msg)
}

fn mat4_to_flat(m: &usd_gf::Matrix4d) -> Vec<f64> {
    // Matrix4 stores data as [[T; 4]; 4] in row-major order.
    let mut out = Vec::with_capacity(16);
    for row in 0..4 {
        for col in 0..4 {
            out.push(m.row(row)[col]);
        }
    }
    out
}

fn parse_xform_op_type(s: &str) -> PyResult<XformOpType> {
    Ok(match s {
        "translate" => XformOpType::Translate,
        "translateX" => XformOpType::TranslateX,
        "translateY" => XformOpType::TranslateY,
        "translateZ" => XformOpType::TranslateZ,
        "scale" => XformOpType::Scale,
        "scaleX" => XformOpType::ScaleX,
        "scaleY" => XformOpType::ScaleY,
        "scaleZ" => XformOpType::ScaleZ,
        "rotateX" => XformOpType::RotateX,
        "rotateY" => XformOpType::RotateY,
        "rotateZ" => XformOpType::RotateZ,
        "rotateXYZ" => XformOpType::RotateXYZ,
        "rotateXZY" => XformOpType::RotateXZY,
        "rotateYXZ" => XformOpType::RotateYXZ,
        "rotateYZX" => XformOpType::RotateYZX,
        "rotateZXY" => XformOpType::RotateZXY,
        "rotateZYX" => XformOpType::RotateZYX,
        "orient" => XformOpType::Orient,
        "transform" => XformOpType::Transform,
        _ => return Err(PyValueError::new_err(format!("Unknown XformOpType: '{s}'"))),
    })
}

fn parse_xform_precision(s: &str) -> PyResult<XformOpPrecision> {
    Ok(match s {
        "double" => XformOpPrecision::Double,
        "float" => XformOpPrecision::Float,
        "half" => XformOpPrecision::Half,
        _ => {
            return Err(PyValueError::new_err(format!(
                "Unknown XformOpPrecision: '{s}'"
            )));
        }
    })
}

fn parse_rotation_order(s: &str) -> PyResult<RotationOrder> {
    Ok(match s {
        "XYZ" => RotationOrder::XYZ,
        "XZY" => RotationOrder::XZY,
        "YXZ" => RotationOrder::YXZ,
        "YZX" => RotationOrder::YZX,
        "ZXY" => RotationOrder::ZXY,
        "ZYX" => RotationOrder::ZYX,
        _ => {
            return Err(PyValueError::new_err(format!(
                "Unknown RotationOrder: '{s}'"
            )));
        }
    })
}

// Re-use core wrappers from usd module — PyO3 needs a single pyclass per type.
use crate::usd::{PyAttribute, PyPrim, PyStage};

/// Extract a Prim from a PyPrim or any schema wrapper with GetPrim().
/// Mirrors C++ implicit Prim conversion from schema types.
/// Wrap add_xform_op result: raise RuntimeError if the op already exists (invalid).
fn check_xform_op(op: XformOp) -> PyResult<PyXformOp> {
    if op.is_valid() {
        Ok(PyXformOp(op))
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Failed to add xform op (already exists or invalid)",
        ))
    }
}

fn extract_prim(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<usd_core::Prim> {
    // Try direct PyPrim first
    if let Ok(p) = obj.cast_exact::<PyPrim>() {
        return Ok(p.borrow().inner.clone());
    }
    // Try calling GetPrim() on schema wrappers (Mesh, Xform, etc.)
    if let Ok(prim_obj) = obj.call_method0("GetPrim") {
        if let Ok(p) = prim_obj.cast_exact::<PyPrim>() {
            return Ok(p.borrow().inner.clone());
        }
    }
    Err(PyValueError::new_err(
        "Expected a Prim or schema object with GetPrim()",
    ))
}

#[inline]
fn imageable_for_subset_prim(prim: &usd_core::Prim) -> Imageable {
    Imageable::new(prim.clone())
}

include!("impl_xform_img_macros.rs");

// ============================================================================
// Tokens
// ============================================================================

#[pyclass(name = "Tokens", module = "pxr.UsdGeom")]
pub struct PyTokens;

#[pymethods]
impl PyTokens {
    #[classattr]
    fn visibility() -> &'static str {
        "visibility"
    }
    #[classattr]
    fn purpose() -> &'static str {
        "purpose"
    }
    #[classattr]
    fn proxy_prim() -> &'static str {
        "proxyPrim"
    }
    #[classattr]
    fn inherited() -> &'static str {
        "inherited"
    }
    #[classattr]
    fn invisible() -> &'static str {
        "invisible"
    }
    #[classattr]
    fn default_() -> &'static str {
        "default"
    }
    #[classattr]
    fn render() -> &'static str {
        "render"
    }
    #[classattr]
    fn proxy() -> &'static str {
        "proxy"
    }
    #[classattr]
    fn guide() -> &'static str {
        "guide"
    }
    #[classattr]
    fn extent() -> &'static str {
        "extent"
    }
    #[classattr]
    fn double_sided() -> &'static str {
        "doubleSided"
    }
    #[classattr]
    fn orientation() -> &'static str {
        "orientation"
    }
    #[classattr]
    fn right_handed() -> &'static str {
        "rightHanded"
    }
    #[classattr]
    fn left_handed() -> &'static str {
        "leftHanded"
    }
    /// pxr naming (`UsdGeom.Tokens.leftHanded`).
    #[classattr]
    #[pyo3(name = "leftHanded")]
    fn token_left_handed_px() -> &'static str {
        "leftHanded"
    }
    /// pxr naming (`UsdGeom.Tokens.rightHanded`).
    #[classattr]
    #[pyo3(name = "rightHanded")]
    fn token_right_handed_px() -> &'static str {
        "rightHanded"
    }
    #[classattr]
    fn points() -> &'static str {
        "points"
    }
    #[classattr]
    fn velocities() -> &'static str {
        "velocities"
    }
    #[classattr]
    fn normals() -> &'static str {
        "normals"
    }
    #[classattr]
    fn face_vertex_indices() -> &'static str {
        "faceVertexIndices"
    }
    #[classattr]
    fn face_vertex_counts() -> &'static str {
        "faceVertexCounts"
    }
    #[classattr]
    fn subdivision_scheme() -> &'static str {
        "subdivisionScheme"
    }
    #[classattr]
    fn interpolate_boundary() -> &'static str {
        "interpolateBoundary"
    }
    #[classattr]
    fn face_varying_linear_interpolation() -> &'static str {
        "faceVaryingLinearInterpolation"
    }
    #[classattr]
    fn crease_indices() -> &'static str {
        "creaseIndices"
    }
    #[classattr]
    fn crease_lengths() -> &'static str {
        "creaseLengths"
    }
    #[classattr]
    fn crease_sharpnesses() -> &'static str {
        "creaseSharpnesses"
    }
    #[classattr]
    fn corner_indices() -> &'static str {
        "cornerIndices"
    }
    #[classattr]
    fn corner_sharpnesses() -> &'static str {
        "cornerSharpnesses"
    }
    #[classattr]
    fn hole_indices() -> &'static str {
        "holeIndices"
    }
    #[classattr]
    fn xform_op_order() -> &'static str {
        "xformOpOrder"
    }
    /// pxr naming (`UsdGeom.Tokens.xformOpOrder`).
    #[classattr]
    #[pyo3(name = "xformOpOrder")]
    fn xform_op_order_px() -> &'static str {
        "xformOpOrder"
    }
    /// Cylinder family token (`UsdGeom.Tokens.Capsule`).
    #[classattr]
    #[pyo3(name = "Capsule")]
    fn token_capsule() -> &'static str {
        "Capsule"
    }
    #[classattr]
    #[pyo3(name = "Cylinder")]
    fn token_cylinder() -> &'static str {
        "Cylinder"
    }
    #[classattr]
    fn interpolation() -> &'static str {
        "interpolation"
    }
    #[classattr]
    fn constant() -> &'static str {
        "constant"
    }
    #[classattr]
    fn uniform() -> &'static str {
        "uniform"
    }
    #[classattr]
    fn vertex() -> &'static str {
        "vertex"
    }
    #[classattr]
    fn varying() -> &'static str {
        "varying"
    }
    #[classattr]
    fn face_varying() -> &'static str {
        "faceVarying"
    }
    #[classattr]
    fn catmull_clark() -> &'static str {
        "catmullClark"
    }
    #[classattr]
    fn loop_() -> &'static str {
        "loop"
    }
    #[classattr]
    fn bilinear() -> &'static str {
        "bilinear"
    }
    #[classattr]
    fn none() -> &'static str {
        "none"
    }
    #[classattr]
    fn up_axis() -> &'static str {
        "upAxis"
    }
    #[classattr]
    fn meters_per_unit() -> &'static str {
        "metersPerUnit"
    }
    #[classattr]
    fn radius() -> &'static str {
        "radius"
    }
    #[classattr]
    fn height() -> &'static str {
        "height"
    }
    #[classattr]
    fn size() -> &'static str {
        "size"
    }
    #[classattr]
    fn axis() -> &'static str {
        "axis"
    }
    #[classattr]
    fn x() -> &'static str {
        "X"
    }
    #[classattr]
    fn y() -> &'static str {
        "Y"
    }
    #[classattr]
    fn z() -> &'static str {
        "Z"
    }
    #[classattr]
    fn display_color() -> &'static str {
        "primvars:displayColor"
    }
    #[classattr]
    fn display_opacity() -> &'static str {
        "primvars:displayOpacity"
    }
    #[classattr]
    fn draw_mode() -> &'static str {
        "model:drawMode"
    }
    #[classattr]
    fn cards() -> &'static str {
        "cards"
    }
    #[classattr]
    fn bounds() -> &'static str {
        "bounds"
    }
    #[classattr]
    fn origin() -> &'static str {
        "origin"
    }
    /// Camera projection token values (`UsdGeomCamera` schema).
    #[classattr]
    fn perspective() -> &'static str {
        "perspective"
    }
    #[classattr]
    fn orthographic() -> &'static str {
        "orthographic"
    }
}

/// `UsdGeom.XformOpTypes` — special xform op name tokens (matches pxr `UsdGeomXformOpTypes`).
#[pyclass(name = "XformOpTypes", module = "pxr.UsdGeom")]
pub struct PyXformOpTypes;

#[pymethods]
impl PyXformOpTypes {
    #[classattr]
    #[pyo3(name = "resetXformStack")]
    fn reset_xform_stack() -> &'static str {
        "!resetXformStack!"
    }
}

// ============================================================================
// XformOp
// ============================================================================

#[pyclass(name = "XformOp", module = "pxr.UsdGeom")]
pub struct PyXformOp(pub XformOp);

#[pymethods]
impl PyXformOp {
    #[pyo3(name = "GetOpType")]
    pub fn get_op_type(&self) -> String {
        format!("{}", self.0.op_type())
    }
    #[pyo3(name = "GetAttr")]
    pub fn get_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.attr().clone())
    }
    #[pyo3(name = "GetName")]
    pub fn get_name(&self) -> String {
        self.0.op_name().as_str().to_owned()
    }
    #[pyo3(name = "GetBaseName")]
    pub fn get_base_name(&self) -> String {
        self.0.op_name().as_str().to_owned()
    }
    #[pyo3(name = "IsInverseOp")]
    pub fn is_inverse_op(&self) -> bool {
        self.0.is_inverse_op()
    }
    #[pyo3(name = "IsDefined")]
    pub fn is_defined(&self) -> bool {
        self.0.is_valid()
    }

    /// Set the value of this xform op. Delegates to Attribute.Set().
    #[pyo3(name = "Set", signature = (value, time=None))]
    pub fn set(
        &self,
        py: pyo3::Python<'_>,
        value: &Bound<'_, pyo3::PyAny>,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<bool> {
        // Wrap inner attr as PyAttribute and call its Set method
        let py_attr = PyAttribute::from_attr(self.0.attr().clone());
        let bound = pyo3::Bound::new(py, py_attr)?;
        let args = match time {
            Some(t) => pyo3::types::PyTuple::new(py, &[value.clone(), t.clone()])?,
            None => pyo3::types::PyTuple::new(py, &[value.clone()])?,
        };
        let result = bound.call_method1("Set", args)?;
        result.extract::<bool>()
    }

    /// Get the value of this xform op at a time.
    #[pyo3(name = "Get", signature = (time=None))]
    pub fn get_value(
        &self,
        py: pyo3::Python<'_>,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<pyo3::Py<pyo3::PyAny>> {
        let py_attr = PyAttribute::from_attr(self.0.attr().clone());
        let bound = pyo3::Bound::new(py, py_attr)?;
        let result = match time {
            Some(t) => bound.call_method1("Get", (t,))?,
            None => bound.call_method0("Get")?,
        };
        Ok(result.unbind())
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __repr__(&self) -> String {
        format!("UsdGeom.XformOp('{}')", self.0.op_name())
    }
    pub fn __eq__(&self, other: &PyXformOp) -> bool {
        self.0.op_name() == other.0.op_name()
    }
    pub fn __ne__(&self, other: &PyXformOp) -> bool {
        self.0.op_name() != other.0.op_name()
    }
}

// ============================================================================
// Primvar
// ============================================================================

#[pyclass(name = "Primvar", module = "pxr.UsdGeom")]
pub struct PyPrimvar(pub Primvar);

#[pymethods]
impl PyPrimvar {
    #[pyo3(name = "GetInterpolation")]
    pub fn get_interpolation(&self) -> String {
        self.0.get_interpolation().as_str().to_owned()
    }
    #[pyo3(name = "SetInterpolation")]
    pub fn set_interpolation(&self, interp: &str) -> bool {
        self.0.set_interpolation(&Token::new(interp))
    }
    #[pyo3(name = "HasValue")]
    pub fn has_value(&self) -> bool {
        self.0.has_value()
    }
    #[pyo3(name = "HasAuthoredValue")]
    pub fn has_authored_value(&self) -> bool {
        self.0.has_authored_value()
    }
    #[pyo3(name = "IsIndexed")]
    pub fn is_indexed(&self) -> bool {
        self.0.is_indexed()
    }
    #[pyo3(name = "GetAttr")]
    pub fn get_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_attr().clone())
    }
    #[pyo3(name = "GetName")]
    pub fn get_name(&self) -> String {
        self.0.get_attr().name().as_str().to_owned()
    }
    #[pyo3(name = "GetPrimvarName")]
    pub fn get_primvar_name(&self) -> String {
        self.0.get_primvar_name().as_str().to_owned()
    }
    #[pyo3(name = "GetTypeName")]
    pub fn get_type_name(&self) -> String {
        self.0.get_type_name().to_string()
    }
    #[pyo3(name = "GetElementSize")]
    pub fn get_element_size(&self) -> i32 {
        self.0.get_element_size()
    }
    #[pyo3(name = "SetElementSize")]
    pub fn set_element_size(&self, size: i32) -> bool {
        self.0.set_element_size(size)
    }
    #[pyo3(name = "IsDefined")]
    pub fn is_defined(&self) -> bool {
        self.0.is_valid()
    }
    #[pyo3(name = "IsPrimvar")]
    #[staticmethod]
    pub fn is_primvar(attr: &PyAttribute) -> bool {
        Primvar::is_primvar(&attr.inner)
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __repr__(&self) -> String {
        format!("UsdGeom.Primvar('{}')", self.0.get_attr().name())
    }
    pub fn __eq__(&self, other: &PyPrimvar) -> bool {
        self.0.get_attr().path() == other.0.get_attr().path()
    }
    pub fn __ne__(&self, other: &PyPrimvar) -> bool {
        self.0.get_attr().path() != other.0.get_attr().path()
    }
}

// ============================================================================
// BBoxCache
// ============================================================================

#[pyclass(name = "BBoxCache", module = "pxr.UsdGeom")]
pub struct PyBBoxCache(pub BBoxCache);

#[pymethods]
impl PyBBoxCache {
    #[new]
    #[pyo3(signature = (time, included_purposes, use_extents_hint = false, ignore_visibility = false))]
    pub fn new(
        time: f64,
        included_purposes: Vec<String>,
        use_extents_hint: bool,
        ignore_visibility: bool,
    ) -> Self {
        let purposes: Vec<Token> = included_purposes.iter().map(|s| Token::new(s)).collect();
        Self(BBoxCache::new(
            TimeCode::new(time),
            purposes,
            use_extents_hint,
            ignore_visibility,
        ))
    }

    #[pyo3(name = "ComputeWorldBound")]
    pub fn compute_world_bound(&mut self, prim: &PyPrim) -> crate::gf::geo::PyBBox3d {
        crate::gf::geo::PyBBox3d(self.0.compute_world_bound(&prim.inner))
    }

    #[pyo3(name = "ComputeLocalBound")]
    pub fn compute_local_bound(&mut self, prim: &PyPrim) -> crate::gf::geo::PyBBox3d {
        crate::gf::geo::PyBBox3d(self.0.compute_local_bound(&prim.inner))
    }

    #[pyo3(name = "SetTime")]
    pub fn set_time(&mut self, time: f64) {
        self.0.set_time(TimeCode::new(time));
    }

    #[pyo3(name = "GetTime")]
    pub fn get_time(&self) -> f64 {
        self.0.get_time().value()
    }

    #[pyo3(name = "GetUseExtentsHint")]
    pub fn get_use_extents_hint(&self) -> bool {
        self.0.get_use_extents_hint()
    }

    #[pyo3(name = "Clear")]
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn __repr__(&self) -> &'static str {
        "UsdGeom.BBoxCache"
    }
}

// ============================================================================
// XformCache
// ============================================================================

#[pyclass(name = "XformCache", module = "pxr.UsdGeom")]
pub struct PyXformCache(pub XformCache);

#[pymethods]
impl PyXformCache {
    #[new]
    #[pyo3(signature = (time = None))]
    pub fn new(time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let tc = match time {
            None => usd_sdf::TimeCode::default(),
            Some(o) => crate::usd::tc_from_py_sdf(o)?,
        };
        Ok(Self(XformCache::new(tc)))
    }

    #[pyo3(name = "GetLocalToWorldTransform")]
    pub fn get_local_to_world_transform(&mut self, prim: &PyPrim) -> crate::gf::matrix::PyMatrix4d {
        crate::gf::matrix::PyMatrix4d(self.0.get_local_to_world_transform(&prim.inner))
    }

    #[pyo3(name = "GetParentToWorldTransform")]
    pub fn get_parent_to_world_transform(
        &mut self,
        prim: &PyPrim,
    ) -> crate::gf::matrix::PyMatrix4d {
        crate::gf::matrix::PyMatrix4d(self.0.get_parent_to_world_transform(&prim.inner))
    }

    #[pyo3(name = "GetLocalTransformation")]
    pub fn get_local_transformation(
        &mut self,
        prim: &PyPrim,
    ) -> (crate::gf::matrix::PyMatrix4d, bool) {
        let (m, resets) = self.0.get_local_transformation(&prim.inner);
        (crate::gf::matrix::PyMatrix4d(m), resets)
    }

    #[pyo3(name = "ComputeRelativeTransform")]
    pub fn compute_relative_transform(
        &mut self,
        prim: &PyPrim,
        ancestor: &PyPrim,
    ) -> (crate::gf::matrix::PyMatrix4d, bool) {
        let (m, resets) = self
            .0
            .compute_relative_transform(&prim.inner, &ancestor.inner);
        (crate::gf::matrix::PyMatrix4d(m), resets)
    }

    #[pyo3(name = "SetTime")]
    pub fn set_time(&mut self, time: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        self.0.set_time(crate::usd::tc_from_py_sdf(time)?);
        Ok(())
    }

    #[pyo3(name = "GetTime")]
    pub fn get_time(&self) -> crate::usd::PyTimeCode {
        let sdf = self.0.get_time();
        crate::usd::PyTimeCode::from_usd_core(usd_core::time_code::TimeCode::from_sdf_time_code(
            &sdf,
        ))
    }

    #[pyo3(name = "Swap")]
    pub fn swap(&mut self, other: &mut PyXformCache) {
        self.0.swap(&mut other.0);
    }

    #[pyo3(name = "Clear")]
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn __repr__(&self) -> &'static str {
        "UsdGeom.XformCache"
    }
}

// ============================================================================
// Imageable
// ============================================================================

#[pyclass(name = "Imageable", module = "pxr.UsdGeom")]
pub struct PyImageable(pub Imageable);

#[pymethods]
impl PyImageable {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Imageable::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at '{}'", p.get_string())))?;
        Ok(Self(Imageable::new(prim)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }
    #[pyo3(name = "GetVisibilityAttr")]
    pub fn get_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_visibility_attr())
    }
    #[pyo3(name = "CreateVisibilityAttr")]
    pub fn create_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_visibility_attr())
    }
    #[pyo3(name = "GetPurposeAttr")]
    pub fn get_purpose_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_purpose_attr())
    }
    #[pyo3(name = "CreatePurposeAttr")]
    pub fn create_purpose_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_purpose_attr())
    }

    #[pyo3(name = "ComputeVisibility", signature = (time=None))]
    pub fn compute_visibility(&self, time: Option<f64>) -> String {
        self.0.compute_visibility(tc(time)).as_str().to_owned()
    }

    #[pyo3(name = "ComputePurpose")]
    pub fn compute_purpose(&self) -> String {
        self.0.compute_purpose().as_str().to_owned()
    }

    #[pyo3(name = "MakeVisible", signature = (time=None))]
    pub fn make_visible(&self, time: Option<f64>) {
        self.0.make_visible(tc(time));
    }
    #[pyo3(name = "MakeInvisible", signature = (time=None))]
    pub fn make_invisible(&self, time: Option<f64>) {
        self.0.make_invisible(tc(time));
    }

    #[pyo3(name = "ComputeWorldBound")]
    pub fn compute_world_bound(&self, time: f64, purpose: &str) -> crate::gf::geo::PyBBox3d {
        let mut cache =
            BBoxCache::new(TimeCode::new(time), vec![Token::new(purpose)], false, false);
        crate::gf::geo::PyBBox3d(cache.compute_world_bound(self.0.prim()))
    }

    #[pyo3(name = "ComputeLocalBound")]
    pub fn compute_local_bound(&self, time: f64, purpose: &str) -> crate::gf::geo::PyBBox3d {
        let mut cache =
            BBoxCache::new(TimeCode::new(time), vec![Token::new(purpose)], false, false);
        crate::gf::geo::PyBBox3d(cache.compute_local_bound(self.0.prim()))
    }

    #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
    pub fn compute_local_to_world_transform(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        let mut cache = XformCache::new(t);
        Ok(crate::gf::matrix::PyMatrix4d(
            cache.get_local_to_world_transform(self.0.prim()),
        ))
    }

    #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
    pub fn compute_parent_to_world_transform(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        let mut cache = XformCache::new(t);
        Ok(crate::gf::matrix::PyMatrix4d(
            cache.get_parent_to_world_transform(self.0.prim()),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "GetOrderedPurposeTokens")]
    pub fn get_ordered_purpose_tokens() -> Vec<String> {
        Imageable::get_ordered_purpose_tokens()
            .iter()
            .map(|t| t.as_str().to_owned())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames")]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Imageable::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "IsValid")]
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Imageable('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Imageable(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// Xformable
// ============================================================================

#[pyclass(name = "Xformable", module = "pxr.UsdGeom")]
pub struct PyXformable(pub Xformable);

#[pymethods]
impl PyXformable {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Xformable::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at '{}'", p.get_string())))?;
        Ok(Self(Xformable::new(prim)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }
    #[pyo3(name = "GetXformOpOrderAttr")]
    pub fn get_xform_op_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_xform_op_order_attr())
    }
    #[pyo3(name = "CreateXformOpOrderAttr")]
    pub fn create_xform_op_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_xform_op_order_attr())
    }

    #[pyo3(name = "AddTranslateOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_translate_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.add_xform_op(
            XformOpType::Translate,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }

    #[pyo3(name = "AddRotateXYZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_rotate_xyz_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.add_xform_op(
            XformOpType::RotateXYZ,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }

    #[pyo3(name = "AddScaleOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_scale_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .add_xform_op(XformOpType::Scale, prec, tok.as_ref(), is_inverse_op),
        )
    }

    #[pyo3(name = "AddTransformOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_transform_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.add_xform_op(
            XformOpType::Transform,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }

    #[pyo3(name = "AddOrientOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_orient_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .add_xform_op(XformOpType::Orient, prec, tok.as_ref(), is_inverse_op),
        )
    }

    #[pyo3(name = "AddRotateXOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_rotate_x_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .add_xform_op(XformOpType::RotateX, prec, tok.as_ref(), is_inverse_op),
        )
    }

    #[pyo3(name = "AddRotateYOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_rotate_y_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .add_xform_op(XformOpType::RotateY, prec, tok.as_ref(), is_inverse_op),
        )
    }

    #[pyo3(name = "AddRotateZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_rotate_z_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .add_xform_op(XformOpType::RotateZ, prec, tok.as_ref(), is_inverse_op),
        )
    }

    #[pyo3(name = "AddXformOp", signature = (op_type, precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_xform_op(
        &self,
        op_type: &str,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let op_t = parse_xform_op_type(op_type)?;
        let prec = parse_xform_precision(precision)?;
        let tok = suffix.map(Token::new);
        Ok(PyXformOp(self.0.add_xform_op(
            op_t,
            prec,
            tok.as_ref(),
            is_inverse_op,
        )))
    }

    #[pyo3(name = "GetOrderedXformOps")]
    pub fn get_ordered_xform_ops(&self) -> Vec<PyXformOp> {
        self.0
            .get_ordered_xform_ops()
            .into_iter()
            .map(PyXformOp)
            .collect()
    }

    /// Matches C++ `UsdGeomXformable::GetLocalTransformation(UsdTimeCode) -> GfMatrix4d`.
    #[pyo3(name = "GetLocalTransformation", signature = (time=None))]
    pub fn get_local_transformation(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        Ok(crate::gf::matrix::PyMatrix4d(
            self.0.get_local_transformation(t),
        ))
    }

    #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
    pub fn compute_local_to_world_transform(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        let mut cache = XformCache::new(t);
        Ok(crate::gf::matrix::PyMatrix4d(
            cache.get_local_to_world_transform(self.0.imageable().prim()),
        ))
    }

    #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
    pub fn compute_parent_to_world_transform(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        let mut cache = XformCache::new(t);
        Ok(crate::gf::matrix::PyMatrix4d(
            cache.get_parent_to_world_transform(self.0.imageable().prim()),
        ))
    }

    #[pyo3(name = "TransformMightBeTimeVarying")]
    pub fn transform_might_be_time_varying(&self) -> bool {
        self.0.transform_might_be_time_varying()
    }

    #[pyo3(name = "MakeMatrixXform")]
    pub fn make_matrix_xform(&self) -> PyXformOp {
        PyXformOp(self.0.make_matrix_xform())
    }
    #[pyo3(name = "ClearXformOpOrder")]
    pub fn clear_xform_op_order(&self) -> bool {
        self.0.clear_xform_op_order()
    }
    #[pyo3(name = "GetResetXformStack")]
    pub fn get_reset_xform_stack(&self) -> bool {
        self.0.get_reset_xform_stack()
    }
    #[pyo3(name = "SetResetXformStack")]
    pub fn set_reset_xform_stack(&self, reset: bool) -> bool {
        self.0.set_reset_xform_stack(reset)
    }

    // --- Imageable (C++ `class_<UsdGeomXformable, bases<UsdGeomImageable>>`) ---
    #[pyo3(name = "GetVisibilityAttr")]
    pub fn get_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.imageable().get_visibility_attr())
    }
    #[pyo3(name = "CreateVisibilityAttr")]
    pub fn create_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.imageable().create_visibility_attr())
    }
    #[pyo3(name = "GetPurposeAttr")]
    pub fn get_purpose_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.imageable().get_purpose_attr())
    }
    #[pyo3(name = "CreatePurposeAttr")]
    pub fn create_purpose_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.imageable().create_purpose_attr())
    }
    #[pyo3(name = "ComputeVisibility", signature = (time=None))]
    pub fn compute_visibility(&self, time: Option<f64>) -> String {
        self.0
            .imageable()
            .compute_visibility(tc(time))
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "ComputePurpose")]
    pub fn compute_purpose(&self) -> String {
        self.0.imageable().compute_purpose().as_str().to_owned()
    }
    #[pyo3(name = "MakeVisible", signature = (time=None))]
    pub fn make_visible(&self, time: Option<f64>) {
        self.0.imageable().make_visible(tc(time));
    }
    #[pyo3(name = "MakeInvisible", signature = (time=None))]
    pub fn make_invisible(&self, time: Option<f64>) {
        self.0.imageable().make_invisible(tc(time));
    }
    #[pyo3(name = "ComputeWorldBound")]
    pub fn compute_world_bound(&self, time: f64, purpose: &str) -> crate::gf::geo::PyBBox3d {
        let mut cache =
            BBoxCache::new(TimeCode::new(time), vec![Token::new(purpose)], false, false);
        crate::gf::geo::PyBBox3d(cache.compute_world_bound(self.0.imageable().prim()))
    }
    #[pyo3(name = "ComputeLocalBound")]
    pub fn compute_local_bound(&self, time: f64, purpose: &str) -> crate::gf::geo::PyBBox3d {
        let mut cache =
            BBoxCache::new(TimeCode::new(time), vec![Token::new(purpose)], false, false);
        crate::gf::geo::PyBBox3d(cache.compute_local_bound(self.0.imageable().prim()))
    }

    #[staticmethod]
    #[pyo3(name = "GetOrderedPurposeTokens")]
    pub fn get_ordered_purpose_tokens() -> Vec<String> {
        Imageable::get_ordered_purpose_tokens()
            .iter()
            .map(|t| t.as_str().to_owned())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames")]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Xformable::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[pyo3(name = "IsValid")]
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Xformable('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Xformable(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// Xform
// ============================================================================

#[pyclass(name = "Xform", module = "pxr.UsdGeom")]
pub struct PyXform(pub Xform);

#[pymethods]
impl PyXform {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Xform::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Xform::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Xform::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Xform('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Xform(<invalid>)".to_owned()
        }
    }

    // --- Xformable methods delegated through xformable() ---
    #[pyo3(name = "GetXformOpOrderAttr")]
    pub fn get_xform_op_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.xformable().get_xform_op_order_attr())
    }
    #[pyo3(name = "CreateXformOpOrderAttr")]
    pub fn create_xform_op_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.xformable().create_xform_op_order_attr())
    }

    #[pyo3(name = "AddTranslateOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_translate_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.xformable().add_xform_op(
            XformOpType::Translate,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }
    #[pyo3(name = "AddRotateXYZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_rotate_xyz_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.xformable().add_xform_op(
            XformOpType::RotateXYZ,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }
    #[pyo3(name = "AddScaleOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
    pub fn add_scale_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.xformable().add_xform_op(
            XformOpType::Scale,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }
    #[pyo3(name = "AddTransformOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_transform_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.xformable().add_xform_op(
            XformOpType::Transform,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }
    #[pyo3(name = "AddOrientOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_orient_op(
        &self,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
        let tok = suffix.map(Token::new);
        check_xform_op(self.0.xformable().add_xform_op(
            XformOpType::Orient,
            prec,
            tok.as_ref(),
            is_inverse_op,
        ))
    }
    #[pyo3(name = "AddXformOp", signature = (op_type, precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
    pub fn add_xform_op(
        &self,
        op_type: &str,
        precision: &str,
        suffix: Option<&str>,
        is_inverse_op: bool,
    ) -> PyResult<PyXformOp> {
        let op_t = parse_xform_op_type(op_type)?;
        let prec = parse_xform_precision(precision)?;
        let tok = suffix.map(Token::new);
        check_xform_op(
            self.0
                .xformable()
                .add_xform_op(op_t, prec, tok.as_ref(), is_inverse_op),
        )
    }
    #[pyo3(name = "GetOrderedXformOps")]
    pub fn get_ordered_xform_ops(&self) -> Vec<PyXformOp> {
        self.0
            .xformable()
            .get_ordered_xform_ops()
            .into_iter()
            .map(PyXformOp)
            .collect()
    }
    #[pyo3(name = "GetLocalTransformation", signature = (time=None))]
    pub fn get_local_transformation(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<crate::gf::matrix::PyMatrix4d> {
        let t = tc_from_py_opt(time)?;
        Ok(crate::gf::matrix::PyMatrix4d(
            self.0.xformable().get_local_transformation(t),
        ))
    }
    #[pyo3(name = "TransformMightBeTimeVarying")]
    pub fn transform_might_be_time_varying(&self) -> bool {
        self.0.xformable().transform_might_be_time_varying()
    }
    #[pyo3(name = "MakeMatrixXform")]
    pub fn make_matrix_xform(&self) -> PyXformOp {
        PyXformOp(self.0.xformable().make_matrix_xform())
    }
    #[pyo3(name = "ClearXformOpOrder")]
    pub fn clear_xform_op_order(&self) -> bool {
        self.0.xformable().clear_xform_op_order()
    }
    #[pyo3(name = "GetResetXformStack")]
    pub fn get_reset_xform_stack(&self) -> bool {
        self.0.xformable().get_reset_xform_stack()
    }
    #[pyo3(name = "SetResetXformStack")]
    pub fn set_reset_xform_stack(&self, reset: bool) -> bool {
        self.0.xformable().set_reset_xform_stack(reset)
    }

    // --- Imageable methods ---
    #[pyo3(name = "GetVisibilityAttr")]
    pub fn get_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.xformable().imageable().get_visibility_attr())
    }
    #[pyo3(name = "GetPurposeAttr")]
    pub fn get_purpose_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.xformable().imageable().get_purpose_attr())
    }
    #[pyo3(name = "ComputeVisibility", signature = (time=None))]
    pub fn compute_visibility(&self, time: Option<f64>) -> String {
        self.0
            .xformable()
            .imageable()
            .compute_visibility(tc(time))
            .as_str()
            .to_owned()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Xform"
    }
}

// ============================================================================
// Boundable
// ============================================================================

#[pyclass(name = "Boundable", module = "pxr.UsdGeom")]
pub struct PyBoundable(pub Boundable);

usd_geom_schema_with_xform!(PyBoundable, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Boundable::new(extract_prim(prim)?)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr")]
    pub fn create_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_extent_attr())
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Boundable('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Boundable(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Boundable"
    }

    /// Matches C++ `UsdGeomBoundable::ComputeExtentFromPlugins(boundable, time)`.
    #[staticmethod]
    #[pyo3(name = "ComputeExtentFromPlugins")]
    pub fn compute_extent_from_plugins(
        schema: &Bound<'_, pyo3::PyAny>,
        time: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let prim = extract_prim(schema)?;
        let b = Boundable::new(prim);
        let tc = crate::usd::tc_from_py_sdf(time)?;
        Ok(Boundable::compute_extent_from_plugins(&b, tc)
            .map(|v| v.into_iter().map(crate::gf::vec::PyVec3f).collect()))
    }
});

// ============================================================================
// Scope
// ============================================================================

#[pyclass(name = "Scope", module = "pxr.UsdGeom")]
pub struct PyScope(pub Scope);

usd_geom_schema_imageable_scope!(PyScope, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Scope::new(extract_prim(prim)?)))
    }
    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Scope::get(&stage.inner, &p)))
    }
    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Scope::define(&stage.inner, &p)))
    }
    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Scope('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Scope(<invalid>)".to_owned()
        }
    }
    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames")]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Scope::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }
});

// ============================================================================
// Gprim
// ============================================================================

#[pyclass(name = "Gprim", module = "pxr.UsdGeom")]
pub struct PyGprim(pub Gprim);

usd_geom_schema_with_xform!(PyGprim, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Gprim::new(extract_prim(prim)?)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetDisplayColorAttr")]
    pub fn get_display_color_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_display_color_attr())
    }
    #[pyo3(name = "CreateDisplayColorAttr")]
    pub fn create_display_color_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_display_color_attr())
    }
    #[pyo3(name = "GetDisplayOpacityAttr")]
    pub fn get_display_opacity_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_display_opacity_attr())
    }
    #[pyo3(name = "CreateDisplayOpacityAttr")]
    pub fn create_display_opacity_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_display_opacity_attr())
    }
    #[pyo3(name = "GetDoubleSidedAttr")]
    pub fn get_double_sided_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_double_sided_attr())
    }
    #[pyo3(name = "CreateDoubleSidedAttr")]
    pub fn create_double_sided_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_double_sided_attr())
    }
    #[pyo3(name = "GetOrientationAttr")]
    pub fn get_orientation_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_orientation_attr())
    }
    #[pyo3(name = "CreateOrientationAttr")]
    pub fn create_orientation_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_orientation_attr())
    }
    #[pyo3(name = "GetDisplayColorPrimvar")]
    pub fn get_display_color_primvar(&self) -> PyPrimvar {
        PyPrimvar(self.0.get_display_color_primvar())
    }
    #[pyo3(name = "GetDisplayOpacityPrimvar")]
    pub fn get_display_opacity_primvar(&self) -> PyPrimvar {
        PyPrimvar(self.0.get_display_opacity_primvar())
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Gprim('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Gprim(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Gprim"
    }
});

// ============================================================================
// Mesh
// ============================================================================

#[pyclass(name = "Mesh", module = "pxr.UsdGeom")]
pub struct PyMesh(pub Mesh);

usd_geom_schema_with_xform!(PyMesh, no_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Mesh::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Mesh::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Mesh::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }
    /// Matches C++ `GetFaceCount(timeCode=UsdTimeCode::Default())` (`wrapMesh.cpp`).
    #[pyo3(name = "GetFaceCount", signature = (time=None))]
    pub fn get_face_count(&self, time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.get_face_count(t))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Mesh('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Mesh(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Mesh::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "ValidateTopology")]
    #[allow(non_snake_case)]
    pub fn validate_topology_static(
        faceVertexIndices: &crate::vt::PyIntArray,
        faceVertexCounts: &crate::vt::PyIntArray,
        numPoints: usize,
    ) -> (bool, String) {
        let mut reason = String::new();
        let ok = Mesh::validate_topology(
            faceVertexIndices.inner.as_slice(),
            faceVertexCounts.inner.as_slice(),
            numPoints,
            Some(&mut reason),
        );
        (ok, reason)
    }

    // topology
    #[pyo3(name = "GetFaceVertexIndicesAttr")]
    pub fn get_face_vertex_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_face_vertex_indices_attr())
    }
    #[pyo3(name = "CreateFaceVertexIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_face_vertex_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_face_vertex_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFaceVertexCountsAttr")]
    pub fn get_face_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_face_vertex_counts_attr())
    }
    #[pyo3(name = "CreateFaceVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_face_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_face_vertex_counts_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetSubdivisionSchemeAttr")]
    pub fn get_subdivision_scheme_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_subdivision_scheme_attr())
    }
    #[pyo3(name = "CreateSubdivisionSchemeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_subdivision_scheme_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_subdivision_scheme_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetInterpolateBoundaryAttr")]
    pub fn get_interpolate_boundary_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_interpolate_boundary_attr())
    }
    #[pyo3(name = "CreateInterpolateBoundaryAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_interpolate_boundary_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_interpolate_boundary_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFaceVaryingLinearInterpolationAttr")]
    pub fn get_face_varying_linear_interpolation_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_face_varying_linear_interpolation_attr())
    }
    #[pyo3(name = "CreateFaceVaryingLinearInterpolationAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_face_varying_linear_interpolation_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_face_varying_linear_interpolation_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTriangleSubdivisionRuleAttr")]
    pub fn get_triangle_subdivision_rule_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_triangle_subdivision_rule_attr())
    }
    #[pyo3(name = "CreateTriangleSubdivisionRuleAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_triangle_subdivision_rule_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_triangle_subdivision_rule_attr(v, write_sparsely),
        ))
    }

    // points (via point_based)
    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocitiesAttr")]
    pub fn get_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_velocities_attr())
    }
    #[pyo3(name = "CreateVelocitiesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_velocities_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_velocities_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAccelerationsAttr")]
    pub fn get_accelerations_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_accelerations_attr())
    }
    #[pyo3(name = "CreateAccelerationsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_accelerations_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_accelerations_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsAttr")]
    pub fn get_normals_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_normals_attr())
    }
    #[pyo3(name = "CreateNormalsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_normals_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_normals_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetDoubleSidedAttr")]
    pub fn get_double_sided_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().get_double_sided_attr())
    }
    #[pyo3(name = "CreateDoubleSidedAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_double_sided_attr(
        &self,
        default_value: Option<bool>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let attr = self.0.point_based().gprim().create_double_sided_attr();
        if let Some(v) = default_value {
            // Match pxr: sparse default `false` skips authoring only on **defined** prims.
            let skip_author = write_sparsely && !v && self.0.prim().is_defined();
            if !skip_author {
                let _ = attr.set(usd_vt::Value::from(v), usd_sdf::TimeCode::DEFAULT);
            }
        }
        Ok(PyAttribute::from_attr(attr))
    }
    #[pyo3(name = "GetOrientationAttr")]
    pub fn get_orientation_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().get_orientation_attr())
    }
    #[pyo3(name = "CreateOrientationAttr")]
    pub fn create_orientation_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().create_orientation_attr())
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().boundable().get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr")]
    pub fn create_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .point_based()
                .gprim()
                .boundable()
                .create_extent_attr(),
        )
    }
    #[pyo3(name = "GetDisplayColorAttr")]
    pub fn get_display_color_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().get_display_color_attr())
    }
    #[pyo3(name = "CreateDisplayColorAttr")]
    pub fn create_display_color_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().create_display_color_attr())
    }
    #[pyo3(name = "GetDisplayOpacityAttr")]
    pub fn get_display_opacity_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().get_display_opacity_attr())
    }
    #[pyo3(name = "CreateDisplayOpacityAttr")]
    pub fn create_display_opacity_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().gprim().create_display_opacity_attr())
    }

    #[pyo3(name = "ComputePointsAtTime", signature = (time, base_time))]
    pub fn compute_points_at_time(
        &self,
        time: Bound<'_, pyo3::PyAny>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let t = crate::usd::tc_from_py_sdf(&time)?;
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut points: Vec<usd_gf::Vec3f> = Vec::new();
        if !self
            .0
            .point_based()
            .compute_points_at_time(&mut points, t, bt)
        {
            return Ok(Vec::new());
        }
        Ok(points.into_iter().map(crate::gf::vec::PyVec3f).collect())
    }

    #[pyo3(name = "ComputePointsAtTimes", signature = (times, base_time))]
    pub fn compute_points_at_times(
        &self,
        times: &Bound<'_, PyList>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<Vec<crate::gf::vec::PyVec3f>>> {
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut tcs: Vec<TimeCode> = Vec::with_capacity(times.len());
        for item in times.iter() {
            tcs.push(crate::usd::tc_from_py_sdf(&item)?);
        }
        let mut points_array: Vec<Vec<usd_gf::Vec3f>> = Vec::new();
        if !self
            .0
            .point_based()
            .compute_points_at_times(&mut points_array, &tcs, bt)
        {
            return Ok(Vec::new());
        }
        Ok(points_array
            .into_iter()
            .map(|frame| frame.into_iter().map(crate::gf::vec::PyVec3f).collect())
            .collect())
    }

    // crease / corner / hole
    #[pyo3(name = "GetCreaseIndicesAttr")]
    pub fn get_crease_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_crease_indices_attr())
    }
    #[pyo3(name = "CreateCreaseIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_crease_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_crease_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetCreaseLengthsAttr")]
    pub fn get_crease_lengths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_crease_lengths_attr())
    }
    #[pyo3(name = "CreateCreaseLengthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_crease_lengths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_crease_lengths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetCreaseSharpnessesAttr")]
    pub fn get_crease_sharpnesses_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_crease_sharpnesses_attr())
    }
    #[pyo3(name = "CreateCreaseSharpnessesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_crease_sharpnesses_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_crease_sharpnesses_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetCornerIndicesAttr")]
    pub fn get_corner_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_corner_indices_attr())
    }
    #[pyo3(name = "CreateCornerIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_corner_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_corner_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetCornerSharpnessesAttr")]
    pub fn get_corner_sharpnesses_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_corner_sharpnesses_attr())
    }
    #[pyo3(name = "CreateCornerSharpnessesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_corner_sharpnesses_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_corner_sharpnesses_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHoleIndicesAttr")]
    pub fn get_hole_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_hole_indices_attr())
    }
    #[pyo3(name = "CreateHoleIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_hole_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_hole_indices_attr(v, write_sparsely),
        ))
    }

    /// Validate this mesh's topology at time. Returns (is_valid, reason_string).
    #[pyo3(name = "ValidateTopologyAtTime", signature = (time=None))]
    pub fn validate_topology_at_time(&self, time: Option<f64>) -> (bool, String) {
        let t = tc(time);
        let counts = self.0.get_face_vertex_counts(t);
        let indices = self.0.get_face_vertex_indices(t);
        match (counts, indices) {
            (Some(c), Some(i)) => {
                let counts_vec: Vec<i32> = c.iter().copied().collect();
                let indices_vec: Vec<i32> = i.iter().copied().collect();
                let mut reason = String::new();
                let ok = Mesh::validate_topology(
                    &indices_vec,
                    &counts_vec,
                    usize::MAX,
                    Some(&mut reason),
                );
                (ok, reason)
            }
            _ => (false, "Could not read topology attributes".to_owned()),
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_mesh(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomMesh"),
        }
    }

    /// C++ sets `_class.attr("SHARPNESS_INFINITE") = UsdGeomMesh::SHARPNESS_INFINITE` (`wrapMesh.cpp`).
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn SHARPNESS_INFINITE() -> f32 {
        SHARPNESS_INFINITE
    }
});

// ============================================================================
// Sphere
// ============================================================================

#[pyclass(name = "Sphere", module = "pxr.UsdGeom")]
pub struct PySphere(pub Sphere);

usd_geom_schema_with_xform!(PySphere, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Sphere::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Sphere::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Sphere::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetRadiusAttr")]
    pub fn get_radius_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_radius_attr())
    }
    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_radius_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Sphere('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Sphere(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_sphere(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomSphere"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Sphere::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Sphere"
    }
});

// ============================================================================
// Cube
// ============================================================================

#[pyclass(name = "Cube", module = "pxr.UsdGeom")]
pub struct PyCube(pub Cube);

usd_geom_schema_with_xform!(PyCube, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Cube::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cube::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cube::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetSizeAttr")]
    pub fn get_size_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_size_attr())
    }
    #[pyo3(name = "CreateSizeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_size_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_size_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Cube('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Cube(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_cube(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCube"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Cube::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Cube"
    }
});

// ============================================================================
// Cone
// ============================================================================

#[pyclass(name = "Cone", module = "pxr.UsdGeom")]
pub struct PyCone(pub Cone);

usd_geom_schema_with_xform!(PyCone, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Cone::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cone::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cone::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetRadiusAttr")]
    pub fn get_radius_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_radius_attr())
    }
    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_radius_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHeightAttr")]
    pub fn get_height_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_height_attr())
    }
    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_height_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_height_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Cone('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Cone(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_cone(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCone"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Cone::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Cone"
    }
});

// ============================================================================
// Cylinder
// ============================================================================

#[pyclass(name = "Cylinder", module = "pxr.UsdGeom")]
pub struct PyCylinder(pub Cylinder);

usd_geom_schema_with_xform!(PyCylinder, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Cylinder::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cylinder::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cylinder::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetRadiusAttr")]
    pub fn get_radius_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_radius_attr())
    }
    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_radius_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHeightAttr")]
    pub fn get_height_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_height_attr())
    }
    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_height_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_height_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Cylinder('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Cylinder(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_cylinder(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCylinder"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Cylinder::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Cylinder"
    }
});

// ============================================================================
// Cylinder_1
// ============================================================================

#[pyclass(name = "Cylinder_1", module = "pxr.UsdGeom")]
pub struct PyCylinder1(pub Cylinder1);

usd_geom_schema_with_xform!(PyCylinder1, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Cylinder1::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cylinder1::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Cylinder1::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    // Cylinder1 delegates attribute access to the inner Cylinder schema.
    #[pyo3(name = "GetRadiusBottomAttr")]
    pub fn get_radius_bottom_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_cylinder().get_radius_bottom_attr())
    }
    #[pyo3(name = "CreateRadiusBottomAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_bottom_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .as_cylinder()
                .create_radius_bottom_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetRadiusTopAttr")]
    pub fn get_radius_top_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_cylinder().get_radius_top_attr())
    }
    #[pyo3(name = "CreateRadiusTopAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_top_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .as_cylinder()
                .create_radius_top_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHeightAttr")]
    pub fn get_height_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_cylinder().get_height_attr())
    }
    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_height_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_cylinder().create_height_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_cylinder().get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_cylinder().create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_cylinder().get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_cylinder().create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Cylinder_1('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Cylinder_1(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_cylinder1(
        _cls: &Bound<'_, pyo3::types::PyType>,
    ) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCylinder_1"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Cylinder::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Cylinder_1"
    }
});

// ============================================================================
// Capsule
// ============================================================================

#[pyclass(name = "Capsule", module = "pxr.UsdGeom")]
pub struct PyCapsule(pub Capsule);

usd_geom_schema_with_xform!(PyCapsule, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Capsule::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Capsule::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Capsule::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetRadiusAttr")]
    pub fn get_radius_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_radius_attr())
    }
    #[pyo3(name = "CreateRadiusAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_radius_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHeightAttr")]
    pub fn get_height_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_height_attr())
    }
    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_height_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_height_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Capsule('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Capsule(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_capsule(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCapsule"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Capsule::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Capsule"
    }
});

// ============================================================================
// Capsule_1
// ============================================================================

#[pyclass(name = "Capsule_1", module = "pxr.UsdGeom")]
pub struct PyCapsule1(pub Capsule1);

usd_geom_schema_with_xform!(PyCapsule1, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Capsule1::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Capsule1::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Capsule1::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    // Capsule1 delegates attribute access to the inner Capsule schema.
    #[pyo3(name = "GetRadiusTopAttr")]
    pub fn get_radius_top_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_capsule().get_radius_top_attr())
    }
    #[pyo3(name = "CreateRadiusTopAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_top_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .as_capsule()
                .create_radius_top_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetRadiusBottomAttr")]
    pub fn get_radius_bottom_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_capsule().get_radius_bottom_attr())
    }
    #[pyo3(name = "CreateRadiusBottomAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_radius_bottom_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .as_capsule()
                .create_radius_bottom_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHeightAttr")]
    pub fn get_height_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_capsule().get_height_attr())
    }
    #[pyo3(name = "CreateHeightAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_height_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_capsule().create_height_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_capsule().get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_capsule().create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.as_capsule().get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.as_capsule().create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Capsule_1('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Capsule_1(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_capsule1(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCapsule_1"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Capsule1::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Capsule_1"
    }
});

// ============================================================================
// Plane
// ============================================================================

#[pyclass(name = "Plane", module = "pxr.UsdGeom")]
pub struct PyPlane(pub Plane);

usd_geom_schema_with_xform!(PyPlane, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Plane::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Plane::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Plane::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetDoubleSidedAttr")]
    pub fn get_double_sided_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_double_sided_attr())
    }
    #[pyo3(name = "CreateDoubleSidedAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_double_sided_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_double_sided_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthAttr")]
    pub fn get_width_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_width_attr())
    }
    #[pyo3(name = "CreateWidthAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_width_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_width_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetLengthAttr")]
    pub fn get_length_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_length_attr())
    }
    #[pyo3(name = "CreateLengthAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_length_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_length_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAxisAttr")]
    pub fn get_axis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_axis_attr())
    }
    #[pyo3(name = "CreateAxisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_axis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_axis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExtentAttr")]
    pub fn get_extent_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_extent_attr())
    }
    #[pyo3(name = "CreateExtentAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_extent_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_extent_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Plane('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Plane(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_plane(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomPlane"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Plane::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Plane"
    }
});

// ============================================================================
// PointBased
// ============================================================================

#[pyclass(name = "PointBased", module = "pxr.UsdGeom")]
pub struct PyPointBased(pub PointBased);

usd_geom_schema_with_xform!(PyPointBased, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(PointBased::new(extract_prim(prim)?)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocitiesAttr")]
    pub fn get_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_velocities_attr())
    }
    #[pyo3(name = "CreateVelocitiesAttr")]
    pub fn create_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_velocities_attr(None, false))
    }
    #[pyo3(name = "GetNormalsAttr")]
    pub fn get_normals_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_normals_attr())
    }
    #[pyo3(name = "CreateNormalsAttr")]
    pub fn create_normals_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_normals_attr(None, false))
    }

    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0.get_normals_interpolation().as_str().to_owned()
    }

    #[pyo3(name = "ComputePointsAtTime", signature = (time, base_time))]
    pub fn compute_points_at_time(
        &self,
        time: Bound<'_, pyo3::PyAny>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let t = crate::usd::tc_from_py_sdf(&time)?;
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut points: Vec<usd_gf::Vec3f> = Vec::new();
        if !self.0.compute_points_at_time(&mut points, t, bt) {
            return Ok(Vec::new());
        }
        Ok(points.into_iter().map(crate::gf::vec::PyVec3f).collect())
    }

    #[pyo3(name = "ComputePointsAtTimes", signature = (times, base_time))]
    pub fn compute_points_at_times(
        &self,
        times: &Bound<'_, PyList>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<Vec<crate::gf::vec::PyVec3f>>> {
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut tcs: Vec<TimeCode> = Vec::with_capacity(times.len());
        for item in times.iter() {
            tcs.push(crate::usd::tc_from_py_sdf(&item)?);
        }
        let mut points_array: Vec<Vec<usd_gf::Vec3f>> = Vec::new();
        if !self.0.compute_points_at_times(&mut points_array, &tcs, bt) {
            return Ok(Vec::new());
        }
        Ok(points_array
            .into_iter()
            .map(|frame| frame.into_iter().map(crate::gf::vec::PyVec3f).collect())
            .collect())
    }

    /// Axis-aligned bounds of `points` as `[min, max]` (`Gf.Vec3f` each).
    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_static(
        points: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if pts.is_empty() {
            // Match pxr: empty input yields an "empty" `Gf.Range3f` when wrapped as min/max.
            return Ok(vec![
                crate::gf::vec::PyVec3f(usd_gf::Vec3f::new(
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::INFINITY,
                )),
                crate::gf::vec::PyVec3f(usd_gf::Vec3f::new(
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                )),
            ]);
        }
        if !PointBased::compute_extent(&pts, &mut extent) {
            return Ok(Vec::new());
        }
        Ok(extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect())
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.PointBased('{}')", self.0.prim().path())
        } else {
            "UsdGeom.PointBased(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "PointBased"
    }
});

// ============================================================================
// Points
// ============================================================================

#[pyclass(name = "Points", module = "pxr.UsdGeom")]
pub struct PyPoints(pub Points);

usd_geom_schema_with_xform!(PyPoints, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Points::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Points::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Points::define(&stage.inner, &p)))
    }

    /// Bounds of `points` expanded by `widths` (see `UsdGeomPoints::ComputeExtent` in C++).
    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_points(
        points: &Bound<'_, pyo3::PyAny>,
        widths: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let w = f32_vec_from_py(widths)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if !Points::compute_extent(&pts, &w, &mut extent) {
            return Ok(None);
        }
        Ok(Some(
            extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect(),
        ))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsAttr")]
    pub fn get_widths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_widths_attr())
    }
    #[pyo3(name = "CreateWidthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_widths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_widths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetIdsAttr")]
    pub fn get_ids_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_ids_attr())
    }
    #[pyo3(name = "CreateIdsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_ids_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_ids_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Points('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Points(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Points"
    }
});

// ============================================================================
// Curves
// ============================================================================

#[pyclass(name = "Curves", module = "pxr.UsdGeom")]
pub struct PyCurves(pub Curves);

usd_geom_schema_with_xform!(PyCurves, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Curves::new(extract_prim(prim)?)))
    }

    /// Bounds of `points` expanded by curve `widths` (see `UsdGeomCurves::ComputeExtent` in C++).
    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_curves(
        points: &Bound<'_, pyo3::PyAny>,
        widths: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let w = f32_vec_from_py(widths)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if !Curves::compute_extent(&pts, &w, &mut extent) {
            return Ok(None);
        }
        Ok(Some(
            extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect(),
        ))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetCurveVertexCountsAttr")]
    pub fn get_curve_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_curve_vertex_counts_attr())
    }
    #[pyo3(name = "CreateCurveVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_curve_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_curve_vertex_counts_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsAttr")]
    pub fn get_widths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_widths_attr())
    }
    #[pyo3(name = "CreateWidthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_widths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_widths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsInterpolation")]
    pub fn get_widths_interpolation(&self) -> String {
        self.0.get_widths_interpolation().as_str().to_owned()
    }
    #[pyo3(name = "SetWidthsInterpolation")]
    pub fn set_widths_interpolation(&self, interpolation: &str) -> bool {
        self.0.set_widths_interpolation(&Token::new(interpolation))
    }
    /// Matches C++ `UsdGeomCurves::GetCurveCount(UsdTimeCode)`.
    #[pyo3(name = "GetCurveCount", signature = (time=None))]
    pub fn get_curve_count(&self, time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.get_curve_count(t))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Curves('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Curves(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Curves"
    }
});

// ============================================================================
// BasisCurves
// ============================================================================

#[pyclass(name = "BasisCurves", module = "pxr.UsdGeom")]
pub struct PyBasisCurves(pub BasisCurves);

usd_geom_schema_with_xform!(PyBasisCurves, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(BasisCurves::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(BasisCurves::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(BasisCurves::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    /// `UsdGeomCurves::ComputeExtent` — same static as `UsdGeom.Curves` (C++ multiple inheritance).
    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_basis_curves(
        points: &Bound<'_, pyo3::PyAny>,
        widths: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let w = f32_vec_from_py(widths)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if !Curves::compute_extent(&pts, &w, &mut extent) {
            return Ok(None);
        }
        Ok(Some(
            extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect(),
        ))
    }

    #[pyo3(name = "GetCurveVertexCountsAttr")]
    pub fn get_curve_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_curve_vertex_counts_attr())
    }
    #[pyo3(name = "CreateCurveVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_curve_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .create_curve_vertex_counts_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetBasisAttr")]
    pub fn get_basis_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_basis_attr())
    }
    #[pyo3(name = "CreateBasisAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_basis_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_basis_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTypeAttr")]
    pub fn get_type_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_type_attr())
    }
    #[pyo3(name = "CreateTypeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_type_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_type_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWrapAttr")]
    pub fn get_wrap_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_wrap_attr())
    }
    #[pyo3(name = "CreateWrapAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_wrap_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_wrap_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .point_based()
                .create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsAttr")]
    pub fn get_widths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_widths_attr())
    }
    #[pyo3(name = "CreateWidthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_widths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.curves().create_widths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0
            .curves()
            .point_based()
            .get_normals_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "GetWidthsInterpolation")]
    pub fn get_widths_interpolation(&self) -> String {
        self.0
            .curves()
            .get_widths_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "SetWidthsInterpolation")]
    pub fn set_widths_interpolation(&self, interpolation: &str) -> bool {
        self.0
            .curves()
            .set_widths_interpolation(&Token::new(interpolation))
    }
    #[pyo3(name = "GetCurveCount", signature = (time=None))]
    pub fn get_curve_count(&self, time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.curves().get_curve_count(t))
    }

    /// See `UsdGeomBasisCurves::ComputeInterpolationForSize` in C++ (`wrapBasisCurves.cpp`).
    #[pyo3(name = "ComputeInterpolationForSize", signature = (n, time=None))]
    pub fn compute_interpolation_for_size(
        &self,
        n: usize,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<String> {
        let t = tc_from_py_opt(time)?;
        let tok = self.0.compute_interpolation_for_size(n, t, None);
        Ok(tok.as_str().to_owned())
    }
    #[pyo3(name = "ComputeUniformDataSize", signature = (time=None))]
    pub fn compute_uniform_data_size(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.compute_uniform_data_size(t))
    }
    #[pyo3(name = "ComputeVaryingDataSize", signature = (time=None))]
    pub fn compute_varying_data_size(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.compute_varying_data_size(t))
    }
    #[pyo3(name = "ComputeVertexDataSize", signature = (time=None))]
    pub fn compute_vertex_data_size(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.compute_vertex_data_size(t))
    }
    #[pyo3(name = "ComputeSegmentCounts", signature = (time=None))]
    pub fn compute_segment_counts(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Vec<i32>> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.compute_segment_counts(t))
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.BasisCurves('{}')", self.0.prim().path())
        } else {
            "UsdGeom.BasisCurves(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        BasisCurves::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "BasisCurves"
    }
});

// ============================================================================
// NurbsCurves
// ============================================================================

#[pyclass(name = "NurbsCurves", module = "pxr.UsdGeom")]
pub struct PyNurbsCurves(pub NurbsCurves);

usd_geom_schema_with_xform!(PyNurbsCurves, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(NurbsCurves::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(NurbsCurves::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(NurbsCurves::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_nurbs_curves(
        points: &Bound<'_, pyo3::PyAny>,
        widths: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let w = f32_vec_from_py(widths)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if !Curves::compute_extent(&pts, &w, &mut extent) {
            return Ok(None);
        }
        Ok(Some(
            extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect(),
        ))
    }

    #[pyo3(name = "GetCurveVertexCountsAttr")]
    pub fn get_curve_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_curve_vertex_counts_attr())
    }
    #[pyo3(name = "CreateCurveVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_curve_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .create_curve_vertex_counts_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .point_based()
                .create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsAttr")]
    pub fn get_widths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_widths_attr())
    }
    #[pyo3(name = "CreateWidthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_widths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.curves().create_widths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0
            .curves()
            .point_based()
            .get_normals_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "GetWidthsInterpolation")]
    pub fn get_widths_interpolation(&self) -> String {
        self.0
            .curves()
            .get_widths_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "SetWidthsInterpolation")]
    pub fn set_widths_interpolation(&self, interpolation: &str) -> bool {
        self.0
            .curves()
            .set_widths_interpolation(&Token::new(interpolation))
    }
    #[pyo3(name = "GetCurveCount", signature = (time=None))]
    pub fn get_curve_count(&self, time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.curves().get_curve_count(t))
    }

    #[pyo3(name = "GetOrderAttr")]
    pub fn get_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_order_attr())
    }
    #[pyo3(name = "CreateOrderAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_order_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_order_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetKnotsAttr")]
    pub fn get_knots_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_knots_attr())
    }
    #[pyo3(name = "CreateKnotsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_knots_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_knots_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetRangesAttr")]
    pub fn get_ranges_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_ranges_attr())
    }
    #[pyo3(name = "CreateRangesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_ranges_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_ranges_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetPointWeightsAttr")]
    pub fn get_point_weights_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_point_weights_attr())
    }
    #[pyo3(name = "CreatePointWeightsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_point_weights_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_point_weights_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.NurbsCurves('{}')", self.0.prim().path())
        } else {
            "UsdGeom.NurbsCurves(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        NurbsCurves::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "NurbsCurves"
    }
});

// ============================================================================
// HermiteCurves
// ============================================================================

#[pyclass(name = "HermiteCurves", module = "pxr.UsdGeom")]
pub struct PyHermiteCurves(pub HermiteCurves);

usd_geom_schema_with_xform!(PyHermiteCurves, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(HermiteCurves::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(HermiteCurves::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(HermiteCurves::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_hermite_curves(
        points: &Bound<'_, pyo3::PyAny>,
        widths: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Option<Vec<crate::gf::vec::PyVec3f>>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let w = f32_vec_from_py(widths)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if !Curves::compute_extent(&pts, &w, &mut extent) {
            return Ok(None);
        }
        Ok(Some(
            extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect(),
        ))
    }

    #[pyo3(name = "GetCurveVertexCountsAttr")]
    pub fn get_curve_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_curve_vertex_counts_attr())
    }
    #[pyo3(name = "CreateCurveVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_curve_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .create_curve_vertex_counts_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .curves()
                .point_based()
                .create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetWidthsAttr")]
    pub fn get_widths_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.curves().get_widths_attr())
    }
    #[pyo3(name = "CreateWidthsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_widths_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.curves().create_widths_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0
            .curves()
            .point_based()
            .get_normals_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "GetWidthsInterpolation")]
    pub fn get_widths_interpolation(&self) -> String {
        self.0
            .curves()
            .get_widths_interpolation()
            .as_str()
            .to_owned()
    }
    #[pyo3(name = "SetWidthsInterpolation")]
    pub fn set_widths_interpolation(&self, interpolation: &str) -> bool {
        self.0
            .curves()
            .set_widths_interpolation(&Token::new(interpolation))
    }
    #[pyo3(name = "GetCurveCount", signature = (time=None))]
    pub fn get_curve_count(&self, time: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<usize> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.curves().get_curve_count(t))
    }

    #[pyo3(name = "GetTangentsAttr")]
    pub fn get_tangents_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_tangents_attr())
    }
    #[pyo3(name = "CreateTangentsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_tangents_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_tangents_attr(v, write_sparsely),
        ))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.HermiteCurves('{}')", self.0.prim().path())
        } else {
            "UsdGeom.HermiteCurves(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        HermiteCurves::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "HermiteCurves"
    }
});

// ============================================================================
// HermiteCurves::PointAndTangentArrays (wrapHermiteCurves.cpp custom block)
// ============================================================================

#[pyclass(name = "PointAndTangentArrays", module = "pxr.UsdGeom")]
pub struct PyPointAndTangentArrays(pub PointAndTangentArrays);

#[pymethods]
impl PyPointAndTangentArrays {
    #[new]
    #[pyo3(signature = (points=None, tangents=None))]
    pub fn new(
        points: Option<&Bound<'_, pyo3::PyAny>>,
        tangents: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Self> {
        match (points, tangents) {
            (None, None) => Ok(Self(PointAndTangentArrays::new())),
            (Some(p), Some(t)) => {
                let pts = vec3f_vec_from_points_arg(p)?;
                let tans = vec3f_vec_from_points_arg(t)?;
                if pts.len() != tans.len() {
                    return Err(raise_tf_runtime_error_py(
                        "Points and tangents must be the same size.",
                    ));
                }
                Ok(Self(PointAndTangentArrays::from_points_and_tangents(
                    pts, tans,
                )))
            }
            _ => Err(PyValueError::new_err(
                "PointAndTangentArrays expects no arguments or (points, tangents)",
            )),
        }
    }

    #[pyo3(name = "GetPoints")]
    pub fn get_points(&self, py: Python<'_>) -> PyResult<Py<crate::vt::PyVec3fArray>> {
        Py::new(
            py,
            crate::vt::PyVec3fArray {
                inner: VtArray::from(self.0.get_points().to_vec()),
            },
        )
    }

    #[pyo3(name = "GetTangents")]
    pub fn get_tangents(&self, py: Python<'_>) -> PyResult<Py<crate::vt::PyVec3fArray>> {
        Py::new(
            py,
            crate::vt::PyVec3fArray {
                inner: VtArray::from(self.0.get_tangents().to_vec()),
            },
        )
    }

    #[pyo3(name = "IsEmpty")]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Matches C++ `explicit operator bool()` — true when non-empty.
    pub fn __bool__(&self) -> bool {
        !self.0.is_empty()
    }

    #[pyo3(name = "Interleave")]
    pub fn interleave(&self, py: Python<'_>) -> PyResult<Py<crate::vt::PyVec3fArray>> {
        let data = self.0.interleave();
        Py::new(
            py,
            crate::vt::PyVec3fArray {
                inner: VtArray::from(data),
            },
        )
    }

    #[staticmethod]
    #[pyo3(name = "Separate")]
    pub fn separate(interleaved: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let v = vec3f_vec_from_points_arg(interleaved)?;
        if !v.is_empty() && v.len() % 2 != 0 {
            return Err(raise_tf_coding_error_py(
                "Cannot separate odd-shaped interleaved points and tangents data.",
            ));
        }
        Ok(Self(PointAndTangentArrays::separate(&v)))
    }

    pub fn __eq__(&self, other: &Bound<'_, pyo3::PyAny>) -> PyResult<bool> {
        if let Ok(o) = other.extract::<PyRef<'_, Self>>() {
            return Ok(self.0 == o.0);
        }
        Ok(false)
    }

    pub fn __ne__(&self, other: &Bound<'_, pyo3::PyAny>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }

    pub fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let p = Py::new(
            py,
            crate::vt::PyVec3fArray {
                inner: VtArray::from(self.0.get_points().to_vec()),
            },
        )?;
        let t = Py::new(
            py,
            crate::vt::PyVec3fArray {
                inner: VtArray::from(self.0.get_tangents().to_vec()),
            },
        )?;
        let p_repr = p.bind(py).repr()?.extract::<String>()?;
        let t_repr = t.bind(py).repr()?.extract::<String>()?;
        Ok(format!("UsdGeom.HermiteCurves({p_repr}, {t_repr})"))
    }
}

// ============================================================================
// NurbsPatch
// ============================================================================

#[pyclass(name = "NurbsPatch", module = "pxr.UsdGeom")]
pub struct PyNurbsPatch(pub NurbsPatch);

usd_geom_schema_with_xform!(PyNurbsPatch, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(NurbsPatch::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(NurbsPatch::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(NurbsPatch::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    // `bases<UsdGeomPointBased>` — points, normals, velocities, accelerations (`wrapPointBased` surface).
    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocitiesAttr")]
    pub fn get_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_velocities_attr())
    }
    #[pyo3(name = "CreateVelocitiesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_velocities_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_velocities_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAccelerationsAttr")]
    pub fn get_accelerations_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_accelerations_attr())
    }
    #[pyo3(name = "CreateAccelerationsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_accelerations_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_accelerations_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsAttr")]
    pub fn get_normals_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_normals_attr())
    }
    #[pyo3(name = "CreateNormalsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_normals_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_normals_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0
            .point_based()
            .get_normals_interpolation()
            .as_str()
            .to_owned()
    }

    #[pyo3(name = "ComputePointsAtTime", signature = (time, base_time))]
    pub fn compute_points_at_time(
        &self,
        time: Bound<'_, pyo3::PyAny>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let t = crate::usd::tc_from_py_sdf(&time)?;
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut points: Vec<usd_gf::Vec3f> = Vec::new();
        if !self
            .0
            .point_based()
            .compute_points_at_time(&mut points, t, bt)
        {
            return Ok(Vec::new());
        }
        Ok(points.into_iter().map(crate::gf::vec::PyVec3f).collect())
    }

    #[pyo3(name = "ComputePointsAtTimes", signature = (times, base_time))]
    pub fn compute_points_at_times(
        &self,
        times: &Bound<'_, PyList>,
        base_time: Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<Vec<crate::gf::vec::PyVec3f>>> {
        let bt = crate::usd::tc_from_py_sdf(&base_time)?;
        let mut tcs: Vec<TimeCode> = Vec::with_capacity(times.len());
        for item in times.iter() {
            tcs.push(crate::usd::tc_from_py_sdf(&item)?);
        }
        let mut points_array: Vec<Vec<usd_gf::Vec3f>> = Vec::new();
        if !self
            .0
            .point_based()
            .compute_points_at_times(&mut points_array, &tcs, bt)
        {
            return Ok(Vec::new());
        }
        Ok(points_array
            .into_iter()
            .map(|frame| frame.into_iter().map(crate::gf::vec::PyVec3f).collect())
            .collect())
    }

    /// Inherited from `UsdGeomPointBased::ComputeExtent` (`wrapPointBased.cpp`).
    #[staticmethod]
    #[pyo3(name = "ComputeExtent")]
    pub fn compute_extent_point_based(
        points: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let pts = vec3f_vec_from_points_arg(points)?;
        let mut extent = [usd_gf::Vec3f::new(0.0, 0.0, 0.0); 2];
        if pts.is_empty() {
            return Ok(vec![
                crate::gf::vec::PyVec3f(usd_gf::Vec3f::new(
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::INFINITY,
                )),
                crate::gf::vec::PyVec3f(usd_gf::Vec3f::new(
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                )),
            ]);
        }
        if !PointBased::compute_extent(&pts, &mut extent) {
            return Ok(Vec::new());
        }
        Ok(extent.iter().map(|e| crate::gf::vec::PyVec3f(*e)).collect())
    }

    #[pyo3(name = "GetUVertexCountAttr")]
    pub fn get_u_vertex_count_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_u_vertex_count_attr())
    }
    #[pyo3(name = "CreateUVertexCountAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_u_vertex_count_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_u_vertex_count_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVVertexCountAttr")]
    pub fn get_v_vertex_count_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_v_vertex_count_attr())
    }
    #[pyo3(name = "CreateVVertexCountAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_v_vertex_count_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_v_vertex_count_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetUOrderAttr")]
    pub fn get_u_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_u_order_attr())
    }
    #[pyo3(name = "CreateUOrderAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_u_order_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_u_order_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVOrderAttr")]
    pub fn get_v_order_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_v_order_attr())
    }
    #[pyo3(name = "CreateVOrderAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_v_order_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_v_order_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetUKnotsAttr")]
    pub fn get_u_knots_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_u_knots_attr())
    }
    #[pyo3(name = "CreateUKnotsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_u_knots_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_u_knots_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVKnotsAttr")]
    pub fn get_v_knots_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_v_knots_attr())
    }
    #[pyo3(name = "CreateVKnotsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_v_knots_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_v_knots_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetURangeAttr")]
    pub fn get_u_range_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_u_range_attr())
    }
    #[pyo3(name = "CreateURangeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_u_range_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_u_range_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVRangeAttr")]
    pub fn get_v_range_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_v_range_attr())
    }
    #[pyo3(name = "CreateVRangeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_v_range_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_v_range_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetUFormAttr")]
    pub fn get_u_form_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_u_form_attr())
    }
    #[pyo3(name = "CreateUFormAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_u_form_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_u_form_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVFormAttr")]
    pub fn get_v_form_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_v_form_attr())
    }
    #[pyo3(name = "CreateVFormAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_v_form_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_v_form_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetPointWeightsAttr")]
    pub fn get_point_weights_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_point_weights_attr())
    }
    #[pyo3(name = "CreatePointWeightsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_point_weights_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_point_weights_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurveCountsAttr")]
    pub fn get_trim_curve_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_counts_attr())
    }
    #[pyo3(name = "CreateTrimCurveCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_trim_curve_counts_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurveOrdersAttr")]
    pub fn get_trim_curve_orders_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_orders_attr())
    }
    #[pyo3(name = "CreateTrimCurveOrdersAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_orders_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_trim_curve_orders_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurveVertexCountsAttr")]
    pub fn get_trim_curve_vertex_counts_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_vertex_counts_attr())
    }
    #[pyo3(name = "CreateTrimCurveVertexCountsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_vertex_counts_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_trim_curve_vertex_counts_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurveKnotsAttr")]
    pub fn get_trim_curve_knots_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_knots_attr())
    }
    #[pyo3(name = "CreateTrimCurveKnotsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_knots_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_trim_curve_knots_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurveRangesAttr")]
    pub fn get_trim_curve_ranges_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_ranges_attr())
    }
    #[pyo3(name = "CreateTrimCurveRangesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_ranges_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_trim_curve_ranges_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetTrimCurvePointsAttr")]
    pub fn get_trim_curve_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_trim_curve_points_attr())
    }
    #[pyo3(name = "CreateTrimCurvePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_trim_curve_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_trim_curve_points_attr(v, write_sparsely),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        NurbsPatch::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.NurbsPatch('{}')", self.0.prim().path())
        } else {
            "UsdGeom.NurbsPatch(<invalid>)".to_owned()
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "NurbsPatch"
    }
});

// ============================================================================
// TetMesh
// ============================================================================

#[pyclass(name = "TetMesh", module = "pxr.UsdGeom")]
pub struct PyTetMesh(pub TetMesh);

usd_geom_schema_with_xform!(PyTetMesh, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(TetMesh::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(TetMesh::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(TetMesh::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    #[pyo3(name = "GetPointsAttr")]
    pub fn get_points_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_points_attr())
    }
    #[pyo3(name = "CreatePointsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_points_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_points_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocitiesAttr")]
    pub fn get_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_velocities_attr())
    }
    #[pyo3(name = "CreateVelocitiesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_velocities_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_velocities_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAccelerationsAttr")]
    pub fn get_accelerations_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_accelerations_attr())
    }
    #[pyo3(name = "CreateAccelerationsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_accelerations_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .point_based()
                .create_accelerations_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsAttr")]
    pub fn get_normals_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.point_based().get_normals_attr())
    }
    #[pyo3(name = "CreateNormalsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_normals_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.point_based().create_normals_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNormalsInterpolation")]
    pub fn get_normals_interpolation(&self) -> String {
        self.0
            .point_based()
            .get_normals_interpolation()
            .as_str()
            .to_owned()
    }

    #[pyo3(name = "GetTetVertexIndicesAttr")]
    pub fn get_tet_vertex_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_tet_vertex_indices_attr())
    }
    #[pyo3(name = "CreateTetVertexIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_tet_vertex_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_tet_vertex_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetSurfaceFaceVertexIndicesAttr")]
    pub fn get_surface_face_vertex_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_surface_face_vertex_indices_attr())
    }
    #[pyo3(
        name = "CreateSurfaceFaceVertexIndicesAttr",
        signature = (default_value=None, write_sparsely=false)
    )]
    pub fn create_surface_face_vertex_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_surface_face_vertex_indices_attr(v, write_sparsely),
        ))
    }

    /// `wrapTetMesh.cpp` WRAP_CUSTOM — static helpers taking a `UsdGeom.TetMesh` schema.
    #[staticmethod]
    #[pyo3(name = "ComputeSurfaceFaces", signature = (tet_mesh, time_code=None))]
    pub fn compute_surface_faces(
        py: Python<'_>,
        tet_mesh: &Bound<'_, pyo3::PyAny>,
        time_code: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Option<Py<crate::vt::PyVec3iArray>>> {
        let tm: PyRef<'_, PyTetMesh> = tet_mesh.extract()?;
        let t = tc_from_py_opt(time_code)?;
        let Some(faces) = tm.0.compute_surface_faces(t) else {
            return Ok(None);
        };
        Ok(Some(Py::new(
            py,
            crate::vt::PyVec3iArray {
                inner: usd_vt::Array::from(faces),
            },
        )?))
    }

    #[staticmethod]
    #[pyo3(name = "FindInvertedElements", signature = (tet_mesh, time_code=None))]
    pub fn find_inverted_elements(
        py: Python<'_>,
        tet_mesh: &Bound<'_, pyo3::PyAny>,
        time_code: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Option<Py<crate::vt::PyIntArray>>> {
        let tm: PyRef<'_, PyTetMesh> = tet_mesh.extract()?;
        let t = tc_from_py_opt(time_code)?;
        let Some(elems) = tm.0.find_inverted_elements(t) else {
            return Ok(None);
        };
        Ok(Some(Py::new(
            py,
            crate::vt::PyIntArray {
                inner: usd_vt::Array::from(elems),
            },
        )?))
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.TetMesh('{}')", self.0.prim().path())
        } else {
            "UsdGeom.TetMesh(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_tet_mesh(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomTetMesh"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        TetMesh::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "TetMesh"
    }
});

// ============================================================================
// PointInstancer
// ============================================================================

#[pyclass(name = "PointInstancer", module = "pxr.UsdGeom")]
pub struct PyPointInstancer(pub PointInstancer);

usd_geom_schema_with_xform!(PyPointInstancer, no_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(PointInstancer::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(PointInstancer::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(PointInstancer::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetPath")]
    pub fn get_path(&self) -> crate::sdf::PyPath {
        crate::sdf::PyPath::from_path(self.0.prim().path().clone())
    }

    #[pyo3(name = "GetProtoIndicesAttr")]
    pub fn get_proto_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_proto_indices_attr())
    }
    #[pyo3(name = "CreateProtoIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_proto_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_proto_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetPositionsAttr")]
    pub fn get_positions_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_positions_attr())
    }
    #[pyo3(name = "CreatePositionsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_positions_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_positions_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetOrientationsAttr")]
    pub fn get_orientations_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_orientations_attr())
    }
    #[pyo3(name = "CreateOrientationsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_orientations_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_orientations_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetScalesAttr")]
    pub fn get_scales_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_scales_attr())
    }
    #[pyo3(name = "CreateScalesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_scales_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_scales_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocitiesAttr")]
    pub fn get_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_velocities_attr())
    }
    #[pyo3(name = "CreateVelocitiesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_velocities_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_velocities_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetAngularVelocitiesAttr")]
    pub fn get_angular_velocities_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_angular_velocities_attr())
    }
    #[pyo3(name = "CreateAngularVelocitiesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_angular_velocities_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_angular_velocities_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetIdsAttr")]
    pub fn get_ids_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_ids_attr())
    }
    #[pyo3(name = "CreateIdsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_ids_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_ids_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetInvisibleIdsAttr")]
    pub fn get_invisible_ids_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_invisible_ids_attr())
    }
    #[pyo3(name = "CreateInvisibleIdsAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_invisible_ids_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_invisible_ids_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetPrototypesRel")]
    pub fn get_prototypes_rel(&self) -> Vec<String> {
        let rel = self.0.get_prototypes_rel();
        rel.get_targets()
            .into_iter()
            .map(|p| p.to_string())
            .collect()
    }

    #[pyo3(name = "ActivateId")]
    pub fn activate_id(&self, id: i64) -> bool {
        self.0.activate_id(id)
    }
    #[pyo3(name = "ActivateIds")]
    pub fn activate_ids(&self, ids: Vec<i64>) -> bool {
        self.0.activate_ids(&ids)
    }
    #[pyo3(name = "DeactivateId")]
    pub fn deactivate_id(&self, id: i64) -> bool {
        self.0.deactivate_id(id)
    }
    #[pyo3(name = "DeactivateIds")]
    pub fn deactivate_ids(&self, ids: Vec<i64>) -> bool {
        self.0.deactivate_ids(&ids)
    }
    #[pyo3(name = "VisId")]
    pub fn vis_id(&self, id: i64, time: Option<f64>) -> bool {
        self.0.vis_id(id, tc(time))
    }
    #[pyo3(name = "VisIds")]
    pub fn vis_ids(&self, ids: Vec<i64>, time: Option<f64>) -> bool {
        self.0.vis_ids(&ids, tc(time))
    }
    #[pyo3(name = "InvisId")]
    pub fn invis_id(&self, id: i64, time: Option<f64>) -> bool {
        self.0.invis_id(id, tc(time))
    }
    #[pyo3(name = "InvisIds")]
    pub fn invis_ids(&self, ids: Vec<i64>, time: Option<f64>) -> bool {
        self.0.invis_ids(&ids, tc(time))
    }
    #[pyo3(name = "ActivateAllIds")]
    pub fn activate_all_ids(&self) -> bool {
        self.0.activate_all_ids()
    }
    #[pyo3(name = "VisAllIds")]
    pub fn vis_all_ids(&self, time: Option<f64>) -> bool {
        self.0.vis_all_ids(tc(time))
    }

    /// Compute per-instance transforms. Returns flat 16-element list per instance.
    #[pyo3(name = "ComputeInstanceTransformsAtTime", signature = (time, base_time, doProtos=0, applyMask=0))]
    #[allow(non_snake_case)]
    #[allow(unused_variables)]
    pub fn compute_instance_transforms_at_time(
        &self,
        time: f64,
        base_time: f64,
        doProtos: u8,
        applyMask: u8,
    ) -> Vec<Vec<f64>> {
        use usd_geom::point_instancer::{MaskApplication, ProtoXformInclusion};
        let mut xforms = Vec::new();
        self.0.compute_instance_transforms_at_time(
            &mut xforms,
            TimeCode::new(time),
            TimeCode::new(base_time),
            ProtoXformInclusion::IncludeProtoXform,
            MaskApplication::ApplyMask,
        );
        xforms.iter().map(|m| mat4_to_flat(m)).collect()
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.PointInstancer('{}')", self.0.prim().path())
        } else {
            "UsdGeom.PointInstancer(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_point_instancer(
        _cls: &Bound<'_, pyo3::types::PyType>,
    ) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomPointInstancer"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        PointInstancer::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "PointInstancer"
    }

    /// C++ enum UsdGeomPointInstancer::ProtoXformInclusion
    #[classattr]
    #[allow(non_upper_case_globals)]
    pub fn IncludeProtoXform() -> u8 {
        0
    }

    #[classattr]
    #[allow(non_upper_case_globals)]
    pub fn ExcludeProtoXform() -> u8 {
        1
    }
});

// ============================================================================
// Camera
// ============================================================================

#[pyclass(name = "Camera", module = "pxr.UsdGeom")]
pub struct PyCamera(pub Camera);

usd_geom_schema_with_xform!(PyCamera, yes_get_path, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Camera::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Camera::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Camera::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetProjectionAttr")]
    pub fn get_projection_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_projection_attr())
    }
    #[pyo3(name = "CreateProjectionAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_projection_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_projection_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHorizontalApertureAttr")]
    pub fn get_horizontal_aperture_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_horizontal_aperture_attr())
    }
    #[pyo3(name = "CreateHorizontalApertureAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_horizontal_aperture_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_horizontal_aperture_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVerticalApertureAttr")]
    pub fn get_vertical_aperture_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_vertical_aperture_attr())
    }
    #[pyo3(name = "CreateVerticalApertureAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_vertical_aperture_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_vertical_aperture_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetHorizontalApertureOffsetAttr")]
    pub fn get_horizontal_aperture_offset_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_horizontal_aperture_offset_attr())
    }
    #[pyo3(name = "CreateHorizontalApertureOffsetAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_horizontal_aperture_offset_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_horizontal_aperture_offset_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVerticalApertureOffsetAttr")]
    pub fn get_vertical_aperture_offset_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_vertical_aperture_offset_attr())
    }
    #[pyo3(name = "CreateVerticalApertureOffsetAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_vertical_aperture_offset_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_vertical_aperture_offset_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFocalLengthAttr")]
    pub fn get_focal_length_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_focal_length_attr())
    }
    #[pyo3(name = "CreateFocalLengthAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_focal_length_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_focal_length_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetClippingRangeAttr")]
    pub fn get_clipping_range_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_clipping_range_attr())
    }
    #[pyo3(name = "CreateClippingRangeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_clipping_range_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_clipping_range_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetClippingPlanesAttr")]
    pub fn get_clipping_planes_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_clipping_planes_attr())
    }
    #[pyo3(name = "CreateClippingPlanesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_clipping_planes_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_clipping_planes_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFStopAttr")]
    pub fn get_f_stop_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_f_stop_attr())
    }
    #[pyo3(name = "CreateFStopAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_f_stop_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_f_stop_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFocusDistanceAttr")]
    pub fn get_focus_distance_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_focus_distance_attr())
    }
    #[pyo3(name = "CreateFocusDistanceAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_focus_distance_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_focus_distance_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetShutterOpenAttr")]
    pub fn get_shutter_open_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_shutter_open_attr())
    }
    #[pyo3(name = "CreateShutterOpenAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_shutter_open_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_shutter_open_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetShutterCloseAttr")]
    pub fn get_shutter_close_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_shutter_close_attr())
    }
    #[pyo3(name = "CreateShutterCloseAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_shutter_close_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_shutter_close_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetStereoRoleAttr")]
    pub fn get_stereo_role_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_stereo_role_attr())
    }
    #[pyo3(name = "CreateStereoRoleAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_stereo_role_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_stereo_role_attr(v, write_sparsely),
        ))
    }

    #[pyo3(name = "GetExposureAttr")]
    pub fn get_exposure_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_exposure_attr())
    }
    #[pyo3(name = "CreateExposureAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_exposure_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_exposure_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExposureIsoAttr")]
    pub fn get_exposure_iso_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_exposure_iso_attr())
    }
    #[pyo3(name = "CreateExposureIsoAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_exposure_iso_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_exposure_iso_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExposureTimeAttr")]
    pub fn get_exposure_time_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_exposure_time_attr())
    }
    #[pyo3(name = "CreateExposureTimeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_exposure_time_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_exposure_time_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExposureFStopAttr")]
    pub fn get_exposure_f_stop_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_exposure_f_stop_attr())
    }
    #[pyo3(name = "CreateExposureFStopAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_exposure_f_stop_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_exposure_f_stop_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetExposureResponsivityAttr")]
    pub fn get_exposure_responsivity_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_exposure_responsivity_attr())
    }
    #[pyo3(name = "CreateExposureResponsivityAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_exposure_responsivity_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_exposure_responsivity_attr(v, write_sparsely),
        ))
    }

    /// Matches C++ `UsdGeomCamera::SetFromCamera(GfCamera const&, UsdTimeCode)`.
    #[pyo3(name = "SetFromCamera", signature = (camera, time))]
    pub fn set_from_camera(
        &self,
        camera: pyo3::PyRef<'_, crate::gf::geo::PyCamera>,
        time: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<bool> {
        let t = crate::usd::tc_from_py_sdf(time)?;
        Ok(self.0.set_from_camera(&camera.0, t))
    }

    /// Matches C++ `UsdGeomCamera::ComputeLinearExposureScale(UsdTimeCode)`.
    #[pyo3(name = "ComputeLinearExposureScale", signature = (time=None))]
    pub fn compute_linear_exposure_scale(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<f32> {
        let t = tc_from_py_opt(time)?;
        Ok(self.0.compute_linear_exposure_scale(t))
    }

    #[pyo3(name = "GetCamera", signature = (time=None))]
    pub fn get_camera(&self, time: Option<f64>) -> crate::gf::geo::PyCamera {
        crate::gf::geo::PyCamera(self.0.get_camera(tc(time)))
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Camera('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Camera(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_camera(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomCamera"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Camera::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Camera"
    }
});

// ============================================================================
// PrimvarsAPI
// ============================================================================

#[pyclass(name = "PrimvarsAPI", module = "pxr.UsdGeom")]
pub struct PyPrimvarsAPI(pub PrimvarsAPI);

#[pymethods]
impl PyPrimvarsAPI {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(PrimvarsAPI::new(extract_prim(prim)?)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    #[pyo3(name = "CreatePrimvar", signature = (name, type_name_str, interpolation=None, element_size=-1))]
    pub fn create_primvar(
        &self,
        name: &str,
        type_name_str: &str,
        interpolation: Option<&str>,
        element_size: i32,
    ) -> PyPrimvar {
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let tok = Token::new(type_name_str);
        let type_name = registry.find_type_by_token(&tok);
        let interp_tok = interpolation.map(Token::new);
        PyPrimvar(self.0.create_primvar(
            &Token::new(name),
            &type_name,
            interp_tok.as_ref(),
            element_size,
        ))
    }

    #[pyo3(name = "GetPrimvar")]
    pub fn get_primvar(&self, name: &str) -> PyPrimvar {
        PyPrimvar(self.0.get_primvar(&Token::new(name)))
    }
    #[pyo3(name = "GetPrimvars")]
    pub fn get_primvars(&self) -> Vec<PyPrimvar> {
        self.0.get_primvars().into_iter().map(PyPrimvar).collect()
    }
    #[pyo3(name = "GetAuthoredPrimvars")]
    pub fn get_authored_primvars(&self) -> Vec<PyPrimvar> {
        self.0
            .get_authored_primvars()
            .into_iter()
            .map(PyPrimvar)
            .collect()
    }
    #[pyo3(name = "HasPrimvar")]
    pub fn has_primvar(&self, name: &str) -> bool {
        self.0.has_primvar(&Token::new(name))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.PrimvarsAPI('{}')", self.0.prim().path())
        } else {
            "UsdGeom.PrimvarsAPI(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// VisibilityAPI
// ============================================================================

#[pyclass(name = "VisibilityAPI", module = "pxr.UsdGeom")]
pub struct PyVisibilityAPI(pub VisibilityAPI);

#[pymethods]
impl PyVisibilityAPI {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(VisibilityAPI::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Apply")]
    pub fn apply(prim: &PyPrim) -> Self {
        Self(VisibilityAPI::apply(&prim.inner))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetGuideVisibilityAttr")]
    pub fn get_guide_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_guide_visibility_attr())
    }
    #[pyo3(name = "CreateGuideVisibilityAttr")]
    pub fn create_guide_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_guide_visibility_attr())
    }
    #[pyo3(name = "GetProxyVisibilityAttr")]
    pub fn get_proxy_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_proxy_visibility_attr())
    }
    #[pyo3(name = "CreateProxyVisibilityAttr")]
    pub fn create_proxy_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_proxy_visibility_attr())
    }
    #[pyo3(name = "GetRenderVisibilityAttr")]
    pub fn get_render_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_render_visibility_attr())
    }
    #[pyo3(name = "CreateRenderVisibilityAttr")]
    pub fn create_render_visibility_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.create_render_visibility_attr())
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.VisibilityAPI('{}')", self.0.prim().path())
        } else {
            "UsdGeom.VisibilityAPI(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// ModelAPI
// ============================================================================

#[pyclass(name = "ModelAPI", module = "pxr.UsdGeom")]
pub struct PyModelAPI(pub ModelAPI);

#[pymethods]
impl PyModelAPI {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(ModelAPI::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Apply")]
    pub fn apply(prim: &PyPrim) -> PyResult<Self> {
        if !prim.inner.is_valid() {
            return Err(PyException::new_err(
                "Cannot apply UsdGeom.ModelAPI to an invalid prim",
            ));
        }
        Ok(Self(
            ModelAPI::apply(&prim.inner).unwrap_or_else(|| ModelAPI::new(prim.inner.clone())),
        ))
    }

    #[pyo3(name = "SetExtentsHint")]
    pub fn set_extents_hint(&self, hint: &Bound<'_, pyo3::PyAny>) -> PyResult<bool> {
        let vecs = vec3f_vec_from_points_arg(hint)?;
        Ok(self.0.set_extents_hint(&vecs, TimeCode::default()))
    }

    #[pyo3(name = "GetExtentsHint", signature = (time=None))]
    pub fn get_extents_hint(
        &self,
        time: Option<&Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Vec<crate::gf::vec::PyVec3f>> {
        let t = match time {
            None => TimeCode::default(),
            Some(o) => crate::usd::tc_from_py_sdf(o)?,
        };
        let v = self.0.get_extents_hint(t).unwrap_or_default();
        Ok(v.into_iter().map(crate::gf::vec::PyVec3f).collect())
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.get_prim().clone())
    }
    #[pyo3(name = "GetModelDrawModeAttr")]
    pub fn get_model_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .get_model_draw_mode_attr()
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    #[pyo3(name = "CreateModelDrawModeAttr")]
    pub fn create_model_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .create_model_draw_mode_attr(None)
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    #[pyo3(name = "GetModelApplyDrawModeAttr")]
    pub fn get_model_apply_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .get_model_apply_draw_mode_attr()
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    #[pyo3(name = "CreateModelApplyDrawModeAttr")]
    pub fn create_model_apply_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .create_model_apply_draw_mode_attr(None)
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    #[pyo3(name = "GetModelCardGeometryAttr")]
    pub fn get_model_card_geometry_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .get_model_card_geometry_attr()
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    #[pyo3(name = "GetExtentsHintAttr")]
    pub fn get_extents_hint_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(
            self.0
                .get_extents_hint_attr()
                .unwrap_or_else(usd_core::Attribute::invalid),
        )
    }
    pub fn is_valid(&self) -> bool {
        self.0.get_prim().is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.get_prim().is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.get_prim().is_valid() {
            format!("UsdGeom.ModelAPI('{}')", self.0.get_prim().path())
        } else {
            "UsdGeom.ModelAPI(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// MotionAPI
// ============================================================================

#[pyclass(name = "MotionAPI", module = "pxr.UsdGeom")]
pub struct PyMotionAPI(pub MotionAPI);

#[pymethods]
impl PyMotionAPI {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(MotionAPI::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Apply")]
    pub fn apply(prim: &PyPrim) -> PyResult<Self> {
        if !prim.inner.is_valid() {
            return Err(PyException::new_err(
                "Cannot apply UsdGeom.MotionAPI to an invalid prim",
            ));
        }
        Ok(Self(MotionAPI::apply(&prim.inner)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetMotionBlurScaleAttr")]
    pub fn get_motion_blur_scale_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_motion_blur_scale_attr())
    }
    #[pyo3(name = "CreateMotionBlurScaleAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_motion_blur_scale_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_motion_blur_scale_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetVelocityScaleAttr")]
    pub fn get_velocity_scale_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_motion_velocity_scale_attr())
    }
    #[pyo3(name = "CreateVelocityScaleAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_velocity_scale_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_motion_velocity_scale_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetNonlinearSampleCountAttr")]
    pub fn get_nonlinear_sample_count_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_motion_nonlinear_sample_count_attr())
    }
    #[pyo3(name = "CreateNonlinearSampleCountAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_nonlinear_sample_count_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0
                .create_motion_nonlinear_sample_count_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "ComputeVelocityScale")]
    pub fn compute_velocity_scale(&self, time: Option<f64>) -> f64 {
        f64::from(self.0.compute_velocity_scale(tc(time)))
    }
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.MotionAPI('{}')", self.0.prim().path())
        } else {
            "UsdGeom.MotionAPI(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// XformCommonAPI
// ============================================================================

#[pyclass(name = "XformCommonAPI", module = "pxr.UsdGeom")]
pub struct PyXformCommonAPI(pub XformCommonAPI);

#[pymethods]
impl PyXformCommonAPI {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(XformCommonAPI::new(extract_prim(prim)?)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }

    #[pyo3(name = "SetXformVectors")]
    pub fn set_xform_vectors(
        &self,
        translation: (f64, f64, f64),
        rotation: (f32, f32, f32),
        scale: (f32, f32, f32),
        pivot: (f32, f32, f32),
        rotation_order: &str,
        time: Option<f64>,
    ) -> PyResult<bool> {
        let rot_order = parse_rotation_order(rotation_order)?;
        let tr = usd_gf::Vec3d::new(translation.0, translation.1, translation.2);
        let rot = usd_gf::Vec3f::new(rotation.0, rotation.1, rotation.2);
        let sc = usd_gf::Vec3f::new(scale.0, scale.1, scale.2);
        let pv = usd_gf::Vec3f::new(pivot.0, pivot.1, pivot.2);
        Ok(self
            .0
            .set_xform_vectors(tr, rot, sc, pv, rot_order, tc(time)))
    }

    #[pyo3(name = "GetXformVectors")]
    pub fn get_xform_vectors(
        &self,
        time: Option<f64>,
    ) -> (
        (f64, f64, f64),
        (f32, f32, f32),
        (f32, f32, f32),
        (f32, f32, f32),
        String,
    ) {
        let mut tr = usd_gf::Vec3d::new(0.0, 0.0, 0.0);
        let mut rot = usd_gf::Vec3f::new(0.0, 0.0, 0.0);
        let mut sc = usd_gf::Vec3f::new(1.0, 1.0, 1.0);
        let mut pv = usd_gf::Vec3f::new(0.0, 0.0, 0.0);
        let mut rot_order = RotationOrder::XYZ;
        self.0.get_xform_vectors(
            &mut tr,
            &mut rot,
            &mut sc,
            &mut pv,
            &mut rot_order,
            tc(time),
        );
        (
            (tr.x, tr.y, tr.z),
            (rot.x, rot.y, rot.z),
            (sc.x, sc.y, sc.z),
            (pv.x, pv.y, pv.z),
            format!("{rot_order:?}"),
        )
    }

    // Rotation order constants (C++ enum XformCommonAPI::RotationOrder)
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderXYZ() -> &'static str {
        "XYZ"
    }
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderXZY() -> &'static str {
        "XZY"
    }
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderYXZ() -> &'static str {
        "YXZ"
    }
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderYZX() -> &'static str {
        "YZX"
    }
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderZXY() -> &'static str {
        "ZXY"
    }
    #[classattr]
    #[allow(non_upper_case_globals)]
    fn RotationOrderZYX() -> &'static str {
        "ZYX"
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.XformCommonAPI('{}')", self.0.prim().path())
        } else {
            "UsdGeom.XformCommonAPI(<invalid>)".to_owned()
        }
    }
}

// ============================================================================
// Subset
// ============================================================================

#[pyclass(name = "Subset", module = "pxr.UsdGeom")]
pub struct PySubset(pub Subset);

usd_geom_schema_imageable_subset!(PySubset, {
    #[new]
    pub fn new(prim: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        Ok(Self(Subset::new(extract_prim(prim)?)))
    }

    #[staticmethod]
    #[pyo3(name = "Get")]
    pub fn get(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Subset::get(&stage.inner, &p)))
    }

    #[staticmethod]
    #[pyo3(name = "Define")]
    pub fn define(stage: &PyStage, path: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let p = parse_path_py(path)?;
        Ok(Self(Subset::define(&stage.inner, &p)))
    }

    #[pyo3(name = "GetPrim")]
    pub fn get_prim(&self) -> PyPrim {
        PyPrim::from_prim_auto(self.0.prim().clone())
    }
    #[pyo3(name = "GetElementTypeAttr")]
    pub fn get_element_type_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_element_type_attr())
    }
    #[pyo3(name = "CreateElementTypeAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_element_type_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_element_type_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetIndicesAttr")]
    pub fn get_indices_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_indices_attr())
    }
    #[pyo3(name = "CreateIndicesAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_indices_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_indices_attr(v, write_sparsely),
        ))
    }
    #[pyo3(name = "GetFamilyNameAttr")]
    pub fn get_family_name_attr(&self) -> PyAttribute {
        PyAttribute::from_attr(self.0.get_family_name_attr())
    }
    #[pyo3(name = "CreateFamilyNameAttr", signature = (default_value=None, write_sparsely=false))]
    pub fn create_family_name_attr(
        &self,
        default_value: Option<&Bound<'_, pyo3::PyAny>>,
        write_sparsely: bool,
    ) -> PyResult<PyAttribute> {
        let v = match default_value {
            None => None,
            Some(o) => Some(crate::vt::py_to_value(o)?),
        };
        Ok(PyAttribute::from_attr(
            self.0.create_family_name_attr(v, write_sparsely),
        ))
    }

    /// Create a geometry subset child prim under a mesh prim.
    #[staticmethod]
    #[pyo3(name = "CreateGeomSubset")]
    pub fn create_geom_subset(
        geom: &PyImageable,
        subset_name: &str,
        element_type: &str,
        indices: Vec<i32>,
        family_name: &str,
        family_type: &str,
    ) -> Self {
        let subset = Subset::create_geom_subset(
            &geom.0,
            &Token::new(subset_name),
            &Token::new(element_type),
            &indices,
            &Token::new(family_name),
            &Token::new(family_type),
        );
        Self(subset)
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
    pub fn __bool__(&self) -> bool {
        self.0.is_valid()
    }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() {
            format!("UsdGeom.Subset('{}')", self.0.prim().path())
        } else {
            "UsdGeom.Subset(<invalid>)".to_owned()
        }
    }

    #[classmethod]
    #[pyo3(name = "_GetStaticTfType")]
    pub fn get_static_tf_type_subset(_cls: &Bound<'_, pyo3::types::PyType>) -> crate::tf::PyType {
        crate::tf::PyType {
            inner: TfType::find_by_name("UsdGeomSubset"),
        }
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = None))]
    pub fn get_schema_attribute_names(include_inherited: Option<bool>) -> Vec<String> {
        let inc = include_inherited.unwrap_or(true);
        Subset::get_schema_attribute_names(inc)
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    #[staticmethod]
    #[pyo3(name = "GetSchemaTypeName")]
    pub fn get_schema_type_name() -> &'static str {
        "Subset"
    }
});

// ============================================================================
// Metrics — module-level functions
// ============================================================================

/// Get stage up axis token ("Y" or "Z").
#[pyfunction]
#[pyo3(name = "GetStageUpAxis")]
pub fn get_stage_up_axis(stage: &PyStage) -> String {
    metrics::get_stage_up_axis(&stage.inner).as_str().to_owned()
}

/// Set stage up axis. Returns true on success.
#[pyfunction]
#[pyo3(name = "SetStageUpAxis")]
pub fn set_stage_up_axis(stage: &PyStage, axis: &str) -> bool {
    metrics::set_stage_up_axis(&stage.inner, &Token::new(axis))
}

/// Get stage meters-per-unit (e.g. 0.01 = centimeters).
#[pyfunction]
#[pyo3(name = "GetStageMetersPerUnit")]
pub fn get_stage_meters_per_unit(stage: &PyStage) -> f64 {
    metrics::get_stage_meters_per_unit(&stage.inner)
}

/// Set stage meters-per-unit. Returns true on success.
#[pyfunction]
#[pyo3(name = "SetStageMetersPerUnit")]
pub fn set_stage_meters_per_unit(stage: &PyStage, mpu: f64) -> bool {
    metrics::set_stage_meters_per_unit(&stage.inner, mpu)
}

// ============================================================================
// Register
// ============================================================================

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Tokens
    m.add_class::<PyTokens>()?;
    m.add_class::<PyXformOpTypes>()?;

    // Cache types
    m.add_class::<PyBBoxCache>()?;
    m.add_class::<PyXformCache>()?;

    // Low-level attribute/op types
    m.add_class::<PyAttribute>()?;
    m.add_class::<PyXformOp>()?;
    m.add_class::<PyPrimvar>()?;

    // Schema hierarchy
    m.add_class::<PyImageable>()?;
    m.add_class::<PyXformable>()?;
    m.add_class::<PyXform>()?;
    m.add_class::<PyBoundable>()?;
    m.add_class::<PyScope>()?;
    m.add_class::<PyGprim>()?;

    // Concrete geometry schemas
    m.add_class::<PyMesh>()?;
    m.add_class::<PySphere>()?;
    m.add_class::<PyCube>()?;
    m.add_class::<PyCone>()?;
    m.add_class::<PyCylinder>()?;
    m.add_class::<PyCylinder1>()?;
    m.add_class::<PyCapsule>()?;
    m.add_class::<PyCapsule1>()?;
    m.add_class::<PyPlane>()?;

    // Curve + patch schemas
    m.add_class::<PyPointBased>()?;
    m.add_class::<PyCurves>()?;
    m.add_class::<PyBasisCurves>()?;
    m.add_class::<PyNurbsCurves>()?;
    m.add_class::<PyHermiteCurves>()?;
    m.add_class::<PyPointAndTangentArrays>()?;
    m.getattr("HermiteCurves")?.setattr(
        "PointAndTangentArrays",
        _py.get_type::<PyPointAndTangentArrays>(),
    )?;
    m.delattr("PointAndTangentArrays")?;
    m.add_class::<PyNurbsPatch>()?;
    m.add_class::<PyTetMesh>()?;

    // Points + instancer
    m.add_class::<PyPoints>()?;
    m.add_class::<PyPointInstancer>()?;

    // Camera
    m.add_class::<PyCamera>()?;

    // API schemas
    m.add_class::<PyPrimvarsAPI>()?;
    m.add_class::<PyVisibilityAPI>()?;
    m.add_class::<PyModelAPI>()?;
    m.add_class::<PyMotionAPI>()?;
    m.add_class::<PyXformCommonAPI>()?;
    m.add_class::<PySubset>()?;

    // Metrics free functions
    m.add_function(wrap_pyfunction!(get_stage_up_axis, m)?)?;
    m.add_function(wrap_pyfunction!(set_stage_up_axis, m)?)?;
    m.add_function(wrap_pyfunction!(get_stage_meters_per_unit, m)?)?;
    m.add_function(wrap_pyfunction!(set_stage_meters_per_unit, m)?)?;

    // LinearUnits constants as a plain dict
    let lu = pyo3::types::PyDict::new(_py);
    lu.set_item("centimeters", metrics::LinearUnits::CENTIMETERS)?;
    lu.set_item("feet", metrics::LinearUnits::FEET)?;
    lu.set_item("inches", metrics::LinearUnits::INCHES)?;
    lu.set_item("kilometers", metrics::LinearUnits::KILOMETERS)?;
    lu.set_item("meters", metrics::LinearUnits::METERS)?;
    lu.set_item("miles", metrics::LinearUnits::MILES)?;
    lu.set_item("millimeters", metrics::LinearUnits::MILLIMETERS)?;
    lu.set_item("yards", metrics::LinearUnits::YARDS)?;
    m.add("LinearUnits", lu)?;

    Ok(())
}
