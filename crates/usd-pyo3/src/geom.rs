//! pxr.UsdGeom — Python bindings for the USD Geometry module.
//!
//! Drop-in replacement for `pxr.UsdGeom` from C++ OpenUSD.
//! All 38 schema classes, plus BBoxCache, XformCache, Primvar, XformOp, Tokens, Metrics.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::module_name_repetitions)]

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

// Qualified imports to avoid name collisions with #[pyfunction] names.
use usd_geom::metrics;
use usd_geom::{
    BBoxCache, XformCache, Primvar, XformOp, XformOpType, XformOpPrecision,
    Imageable, Xformable, Xform, Boundable, Scope, Gprim,
    Mesh, SHARPNESS_INFINITE,
    Sphere, Cube, Cone, Cylinder, Cylinder1, Capsule, Capsule1, Plane,
    PointBased, Points, Curves, BasisCurves, NurbsCurves, HermiteCurves,
    NurbsPatch, TetMesh, PointInstancer, Camera,
    PrimvarsAPI, VisibilityAPI, ModelAPI, MotionAPI, XformCommonAPI,
    RotationOrder, Subset,
};

// ============================================================================
// Helpers
// ============================================================================

fn tc(t: Option<f64>) -> TimeCode {
    match t {
        Some(v) => TimeCode::new(v),
        None => TimeCode::default(),
    }
}

fn parse_path(s: &str) -> PyResult<Path> {
    Path::from_string(s)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: '{s}'")))
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

fn bbox_to_flat(bbox: &usd_gf::BBox3d) -> Vec<f64> {
    let r = bbox.range();
    vec![r.min().x, r.min().y, r.min().z, r.max().x, r.max().y, r.max().z]
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
        _ => return Err(PyValueError::new_err(format!("Unknown XformOpPrecision: '{s}'"))),
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
        _ => return Err(PyValueError::new_err(format!("Unknown RotationOrder: '{s}'"))),
    })
}

// ============================================================================
// Core opaque wrappers (Stage / Prim / Attribute)
// ============================================================================

/// Opaque wrapper for Arc<Stage> — passed in from pxr.Usd.Stage.
#[pyclass(name = "Stage", module = "pxr_rs.Usd")]
pub struct PyStage(pub std::sync::Arc<usd_core::Stage>);

/// Opaque wrapper for Prim.
#[pyclass(name = "Prim", module = "pxr_rs.Usd")]
pub struct PyPrim(pub usd_core::Prim);

/// Opaque wrapper for Attribute.
#[pyclass(name = "Attribute", module = "pxr_rs.Usd")]
pub struct PyAttribute(pub usd_core::Attribute);

#[pymethods]
impl PyAttribute {
    pub fn get_name(&self) -> String { self.0.name().as_str().to_owned() }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }
    pub fn __repr__(&self) -> String { format!("Usd.Attribute('{}')", self.0.name()) }
}

// ============================================================================
// Tokens
// ============================================================================

#[pyclass(name = "Tokens", module = "pxr_rs.UsdGeom")]
pub struct PyTokens;

#[pymethods]
impl PyTokens {
    #[classattr] fn visibility() -> &'static str { "visibility" }
    #[classattr] fn purpose() -> &'static str { "purpose" }
    #[classattr] fn proxy_prim() -> &'static str { "proxyPrim" }
    #[classattr] fn inherited() -> &'static str { "inherited" }
    #[classattr] fn invisible() -> &'static str { "invisible" }
    #[classattr] fn default_() -> &'static str { "default" }
    #[classattr] fn render() -> &'static str { "render" }
    #[classattr] fn proxy() -> &'static str { "proxy" }
    #[classattr] fn guide() -> &'static str { "guide" }
    #[classattr] fn extent() -> &'static str { "extent" }
    #[classattr] fn double_sided() -> &'static str { "doubleSided" }
    #[classattr] fn orientation() -> &'static str { "orientation" }
    #[classattr] fn right_handed() -> &'static str { "rightHanded" }
    #[classattr] fn left_handed() -> &'static str { "leftHanded" }
    #[classattr] fn points() -> &'static str { "points" }
    #[classattr] fn velocities() -> &'static str { "velocities" }
    #[classattr] fn normals() -> &'static str { "normals" }
    #[classattr] fn face_vertex_indices() -> &'static str { "faceVertexIndices" }
    #[classattr] fn face_vertex_counts() -> &'static str { "faceVertexCounts" }
    #[classattr] fn subdivision_scheme() -> &'static str { "subdivisionScheme" }
    #[classattr] fn interpolate_boundary() -> &'static str { "interpolateBoundary" }
    #[classattr] fn face_varying_linear_interpolation() -> &'static str { "faceVaryingLinearInterpolation" }
    #[classattr] fn crease_indices() -> &'static str { "creaseIndices" }
    #[classattr] fn crease_lengths() -> &'static str { "creaseLengths" }
    #[classattr] fn crease_sharpnesses() -> &'static str { "creaseSharpnesses" }
    #[classattr] fn corner_indices() -> &'static str { "cornerIndices" }
    #[classattr] fn corner_sharpnesses() -> &'static str { "cornerSharpnesses" }
    #[classattr] fn hole_indices() -> &'static str { "holeIndices" }
    #[classattr] fn xform_op_order() -> &'static str { "xformOpOrder" }
    #[classattr] fn interpolation() -> &'static str { "interpolation" }
    #[classattr] fn constant() -> &'static str { "constant" }
    #[classattr] fn uniform() -> &'static str { "uniform" }
    #[classattr] fn vertex() -> &'static str { "vertex" }
    #[classattr] fn varying() -> &'static str { "varying" }
    #[classattr] fn face_varying() -> &'static str { "faceVarying" }
    #[classattr] fn catmull_clark() -> &'static str { "catmullClark" }
    #[classattr] fn loop_() -> &'static str { "loop" }
    #[classattr] fn bilinear() -> &'static str { "bilinear" }
    #[classattr] fn none() -> &'static str { "none" }
    #[classattr] fn up_axis() -> &'static str { "upAxis" }
    #[classattr] fn meters_per_unit() -> &'static str { "metersPerUnit" }
    #[classattr] fn radius() -> &'static str { "radius" }
    #[classattr] fn height() -> &'static str { "height" }
    #[classattr] fn size() -> &'static str { "size" }
    #[classattr] fn axis() -> &'static str { "axis" }
    #[classattr] fn x() -> &'static str { "X" }
    #[classattr] fn y() -> &'static str { "Y" }
    #[classattr] fn z() -> &'static str { "Z" }
    #[classattr] fn display_color() -> &'static str { "primvars:displayColor" }
    #[classattr] fn display_opacity() -> &'static str { "primvars:displayOpacity" }
    #[classattr] fn draw_mode() -> &'static str { "model:drawMode" }
    #[classattr] fn cards() -> &'static str { "cards" }
    #[classattr] fn bounds() -> &'static str { "bounds" }
    #[classattr] fn origin() -> &'static str { "origin" }
}

// ============================================================================
// XformOp
// ============================================================================

#[pyclass(name = "XformOp", module = "pxr_rs.UsdGeom")]
pub struct PyXformOp(pub XformOp);

#[pymethods]
impl PyXformOp {
    pub fn get_op_type(&self) -> String { format!("{}", self.0.op_type()) }
    pub fn get_attr(&self) -> PyAttribute { PyAttribute(self.0.attr().clone()) }
    pub fn get_name(&self) -> String { self.0.op_name().as_str().to_owned() }
    pub fn is_inverse_op(&self) -> bool { self.0.is_inverse_op() }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }
    pub fn __repr__(&self) -> String { format!("UsdGeom.XformOp('{}')", self.0.op_name()) }
}

// ============================================================================
// Primvar
// ============================================================================

#[pyclass(name = "Primvar", module = "pxr_rs.UsdGeom")]
pub struct PyPrimvar(pub Primvar);

#[pymethods]
impl PyPrimvar {
    pub fn get_interpolation(&self) -> String {
        self.0.get_interpolation().as_str().to_owned()
    }
    pub fn set_interpolation(&self, interp: &str) -> bool {
        self.0.set_interpolation(&Token::new(interp))
    }
    pub fn has_value(&self) -> bool { self.0.has_value() }
    pub fn is_indexed(&self) -> bool { self.0.is_indexed() }
    pub fn get_attr(&self) -> PyAttribute { PyAttribute(self.0.get_attr().clone()) }
    pub fn get_name(&self) -> String { self.0.get_attr().name().as_str().to_owned() }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }
    pub fn __repr__(&self) -> String { format!("UsdGeom.Primvar('{}')", self.0.get_attr().name()) }
}

// ============================================================================
// BBoxCache
// ============================================================================

#[pyclass(name = "BBoxCache", module = "pxr_rs.UsdGeom")]
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
        Self(BBoxCache::new(TimeCode::new(time), purposes, use_extents_hint, ignore_visibility))
    }

    pub fn compute_world_bound(&mut self, prim: &PyPrim) -> Vec<f64> {
        bbox_to_flat(&self.0.compute_world_bound(&prim.0))
    }

    pub fn compute_local_bound(&mut self, prim: &PyPrim) -> Vec<f64> {
        bbox_to_flat(&self.0.compute_local_bound(&prim.0))
    }

    pub fn set_time(&mut self, time: f64) {
        self.0.set_time(TimeCode::new(time));
    }

    pub fn clear(&mut self) { self.0.clear(); }
    pub fn __repr__(&self) -> &'static str { "UsdGeom.BBoxCache" }
}

// ============================================================================
// XformCache
// ============================================================================

#[pyclass(name = "XformCache", module = "pxr_rs.UsdGeom")]
pub struct PyXformCache(pub XformCache);

#[pymethods]
impl PyXformCache {
    #[new]
    #[pyo3(signature = (time = None))]
    pub fn new(time: Option<f64>) -> Self { Self(XformCache::new(tc(time))) }

    pub fn get_local_to_world_transform(&mut self, prim: &PyPrim) -> Vec<f64> {
        mat4_to_flat(&self.0.get_local_to_world_transform(&prim.0))
    }

    pub fn get_parent_to_world_transform(&mut self, prim: &PyPrim) -> Vec<f64> {
        mat4_to_flat(&self.0.get_parent_to_world_transform(&prim.0))
    }

    pub fn set_time(&mut self, time: f64) {
        self.0.set_time(TimeCode::new(time));
    }

    pub fn clear(&mut self) { self.0.clear(); }
    pub fn __repr__(&self) -> &'static str { "UsdGeom.XformCache" }
}

// ============================================================================
// Imageable
// ============================================================================

#[pyclass(name = "Imageable", module = "pxr_rs.UsdGeom")]
pub struct PyImageable(pub Imageable);

#[pymethods]
impl PyImageable {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Imageable::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        let prim = stage.0.get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at '{path}'")))?;
        Ok(Self(Imageable::new(prim)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.get_visibility_attr()) }
    pub fn create_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.create_visibility_attr()) }
    pub fn get_purpose_attr(&self) -> PyAttribute { PyAttribute(self.0.get_purpose_attr()) }
    pub fn create_purpose_attr(&self) -> PyAttribute { PyAttribute(self.0.create_purpose_attr()) }

    pub fn compute_visibility(&self, time: Option<f64>) -> String {
        self.0.compute_visibility(tc(time)).as_str().to_owned()
    }

    pub fn make_visible(&self, time: Option<f64>) { self.0.make_visible(tc(time)); }
    pub fn make_invisible(&self, time: Option<f64>) { self.0.make_invisible(tc(time)); }

    pub fn compute_world_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
        let mut cache = BBoxCache::new(
            TimeCode::new(time), vec![Token::new(purpose)], false, false,
        );
        bbox_to_flat(&cache.compute_world_bound(self.0.prim()))
    }

    pub fn compute_local_to_world_transform(&self, time: Option<f64>) -> Vec<f64> {
        let mut cache = XformCache::new(tc(time));
        mat4_to_flat(&cache.get_local_to_world_transform(self.0.prim()))
    }

    #[staticmethod]
    pub fn get_ordered_purpose_tokens() -> Vec<String> {
        Imageable::get_ordered_purpose_tokens().iter().map(|t| t.as_str().to_owned()).collect()
    }

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Imageable('{}')", self.0.prim().path()) }
        else { "UsdGeom.Imageable(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Imageable" }
}

// ============================================================================
// Xformable
// ============================================================================

#[pyclass(name = "Xformable", module = "pxr_rs.UsdGeom")]
pub struct PyXformable(pub Xformable);

#[pymethods]
impl PyXformable {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Xformable::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        let prim = stage.0.get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at '{path}'")))?;
        Ok(Self(Xformable::new(prim)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_xform_op_order_attr(&self) -> PyAttribute { PyAttribute(self.0.get_xform_op_order_attr()) }
    pub fn create_xform_op_order_attr(&self) -> PyAttribute { PyAttribute(self.0.create_xform_op_order_attr()) }

    pub fn add_translate_op(&self, suffix: Option<&str>, is_inverse: bool) -> PyXformOp {
        let tok = suffix.map(Token::new);
        PyXformOp(self.0.add_xform_op(XformOpType::Translate, XformOpPrecision::Double, tok.as_ref(), is_inverse))
    }

    pub fn add_rotate_xyz_op(&self, suffix: Option<&str>, is_inverse: bool) -> PyXformOp {
        let tok = suffix.map(Token::new);
        PyXformOp(self.0.add_xform_op(XformOpType::RotateXYZ, XformOpPrecision::Float, tok.as_ref(), is_inverse))
    }

    pub fn add_scale_op(&self, suffix: Option<&str>, is_inverse: bool) -> PyXformOp {
        let tok = suffix.map(Token::new);
        PyXformOp(self.0.add_xform_op(XformOpType::Scale, XformOpPrecision::Float, tok.as_ref(), is_inverse))
    }

    pub fn add_transform_op(&self, suffix: Option<&str>, is_inverse: bool) -> PyXformOp {
        let tok = suffix.map(Token::new);
        PyXformOp(self.0.add_xform_op(XformOpType::Transform, XformOpPrecision::Double, tok.as_ref(), is_inverse))
    }

    pub fn add_xform_op(
        &self,
        op_type_str: &str,
        precision_str: &str,
        suffix: Option<&str>,
        is_inverse: bool,
    ) -> PyResult<PyXformOp> {
        let op_type = parse_xform_op_type(op_type_str)?;
        let precision = parse_xform_precision(precision_str)?;
        let tok = suffix.map(Token::new);
        Ok(PyXformOp(self.0.add_xform_op(op_type, precision, tok.as_ref(), is_inverse)))
    }

    pub fn get_ordered_xform_ops(&self) -> Vec<PyXformOp> {
        self.0.get_ordered_xform_ops().into_iter().map(PyXformOp).collect()
    }

    /// Returns (matrix_flat_16, resets_xform_stack).
    pub fn get_local_transformation(&self, time: Option<f64>) -> (Vec<f64>, bool) {
        let (mat, resets) = self.0.get_local_transformation_with_reset(tc(time));
        (mat4_to_flat(&mat), resets)
    }

    pub fn make_matrix_xform(&self) -> PyXformOp { PyXformOp(self.0.make_matrix_xform()) }
    pub fn clear_xform_op_order(&self) -> bool { self.0.clear_xform_op_order() }
    pub fn get_reset_xform_stack(&self) -> bool { self.0.get_reset_xform_stack() }
    pub fn set_reset_xform_stack(&self, reset: bool) -> bool { self.0.set_reset_xform_stack(reset) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Xformable('{}')", self.0.prim().path()) }
        else { "UsdGeom.Xformable(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Xformable" }
}

// ============================================================================
// Xform
// ============================================================================

#[pyclass(name = "Xform", module = "pxr_rs.UsdGeom")]
pub struct PyXform(pub Xform);

#[pymethods]
impl PyXform {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Xform::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Xform::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Xform::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Xform('{}')", self.0.prim().path()) }
        else { "UsdGeom.Xform(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Xform" }
}

// ============================================================================
// Boundable
// ============================================================================

#[pyclass(name = "Boundable", module = "pxr_rs.UsdGeom")]
pub struct PyBoundable(pub Boundable);

#[pymethods]
impl PyBoundable {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Boundable::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_extent_attr(&self) -> PyAttribute { PyAttribute(self.0.get_extent_attr()) }
    pub fn create_extent_attr(&self) -> PyAttribute { PyAttribute(self.0.create_extent_attr()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Boundable('{}')", self.0.prim().path()) }
        else { "UsdGeom.Boundable(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Boundable" }
}

// ============================================================================
// Scope
// ============================================================================

#[pyclass(name = "Scope", module = "pxr_rs.UsdGeom")]
pub struct PyScope(pub Scope);

#[pymethods]
impl PyScope {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Scope::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Scope::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Scope::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Scope('{}')", self.0.prim().path()) }
        else { "UsdGeom.Scope(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Scope" }
}

// ============================================================================
// Gprim
// ============================================================================

#[pyclass(name = "Gprim", module = "pxr_rs.UsdGeom")]
pub struct PyGprim(pub Gprim);

#[pymethods]
impl PyGprim {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Gprim::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_display_color_attr(&self) -> PyAttribute { PyAttribute(self.0.get_display_color_attr()) }
    pub fn create_display_color_attr(&self) -> PyAttribute { PyAttribute(self.0.create_display_color_attr()) }
    pub fn get_display_opacity_attr(&self) -> PyAttribute { PyAttribute(self.0.get_display_opacity_attr()) }
    pub fn create_display_opacity_attr(&self) -> PyAttribute { PyAttribute(self.0.create_display_opacity_attr()) }
    pub fn get_double_sided_attr(&self) -> PyAttribute { PyAttribute(self.0.get_double_sided_attr()) }
    pub fn create_double_sided_attr(&self) -> PyAttribute { PyAttribute(self.0.create_double_sided_attr()) }
    pub fn get_orientation_attr(&self) -> PyAttribute { PyAttribute(self.0.get_orientation_attr()) }
    pub fn create_orientation_attr(&self) -> PyAttribute { PyAttribute(self.0.create_orientation_attr()) }
    pub fn get_display_color_primvar(&self) -> PyPrimvar { PyPrimvar(self.0.get_display_color_primvar()) }
    pub fn get_display_opacity_primvar(&self) -> PyPrimvar { PyPrimvar(self.0.get_display_opacity_primvar()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Gprim('{}')", self.0.prim().path()) }
        else { "UsdGeom.Gprim(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Gprim" }
}

// ============================================================================
// Mesh
// ============================================================================

#[pyclass(name = "Mesh", module = "pxr_rs.UsdGeom")]
pub struct PyMesh(pub Mesh);

#[pymethods]
impl PyMesh {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Mesh::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Mesh::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Mesh::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Mesh('{}')", self.0.prim().path()) }
        else { "UsdGeom.Mesh(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Mesh" }

    // topology
    pub fn get_face_vertex_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_face_vertex_indices_attr()) }
    pub fn create_face_vertex_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_face_vertex_indices_attr(None, false)) }
    pub fn get_face_vertex_counts_attr(&self) -> PyAttribute { PyAttribute(self.0.get_face_vertex_counts_attr()) }
    pub fn create_face_vertex_counts_attr(&self) -> PyAttribute { PyAttribute(self.0.create_face_vertex_counts_attr(None, false)) }
    pub fn get_subdivision_scheme_attr(&self) -> PyAttribute { PyAttribute(self.0.get_subdivision_scheme_attr()) }
    pub fn create_subdivision_scheme_attr(&self) -> PyAttribute { PyAttribute(self.0.create_subdivision_scheme_attr(None, false)) }
    pub fn get_interpolate_boundary_attr(&self) -> PyAttribute { PyAttribute(self.0.get_interpolate_boundary_attr()) }
    pub fn create_interpolate_boundary_attr(&self) -> PyAttribute { PyAttribute(self.0.create_interpolate_boundary_attr(None, false)) }
    pub fn get_face_varying_linear_interpolation_attr(&self) -> PyAttribute { PyAttribute(self.0.get_face_varying_linear_interpolation_attr()) }
    pub fn create_face_varying_linear_interpolation_attr(&self) -> PyAttribute { PyAttribute(self.0.create_face_varying_linear_interpolation_attr(None, false)) }

    // points (via point_based)
    pub fn get_points_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().get_points_attr()) }
    pub fn create_points_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().create_points_attr(None, false)) }
    pub fn get_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().get_velocities_attr()) }
    pub fn get_normals_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().get_normals_attr()) }
    pub fn create_normals_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().create_normals_attr(None, false)) }

    // crease / corner / hole
    pub fn get_crease_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_crease_indices_attr()) }
    pub fn create_crease_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_crease_indices_attr(None, false)) }
    pub fn get_crease_lengths_attr(&self) -> PyAttribute { PyAttribute(self.0.get_crease_lengths_attr()) }
    pub fn create_crease_lengths_attr(&self) -> PyAttribute { PyAttribute(self.0.create_crease_lengths_attr(None, false)) }
    pub fn get_crease_sharpnesses_attr(&self) -> PyAttribute { PyAttribute(self.0.get_crease_sharpnesses_attr()) }
    pub fn create_crease_sharpnesses_attr(&self) -> PyAttribute { PyAttribute(self.0.create_crease_sharpnesses_attr(None, false)) }
    pub fn get_corner_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_corner_indices_attr()) }
    pub fn create_corner_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_corner_indices_attr(None, false)) }
    pub fn get_corner_sharpnesses_attr(&self) -> PyAttribute { PyAttribute(self.0.get_corner_sharpnesses_attr()) }
    pub fn create_corner_sharpnesses_attr(&self) -> PyAttribute { PyAttribute(self.0.create_corner_sharpnesses_attr(None, false)) }
    pub fn get_hole_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_hole_indices_attr()) }
    pub fn create_hole_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_hole_indices_attr(None, false)) }

    /// Validate mesh topology at time. Returns (is_valid, reason_string).
    pub fn validate_topology(&self, time: Option<f64>) -> (bool, String) {
        let t = tc(time);
        let counts = self.0.get_face_vertex_counts(t);
        let indices = self.0.get_face_vertex_indices(t);
        match (counts, indices) {
            (Some(c), Some(i)) => {
                let counts_vec: Vec<i32> = c.iter().copied().collect();
                let indices_vec: Vec<i32> = i.iter().copied().collect();
                let mut reason = String::new();
                let ok = Mesh::validate_topology(&indices_vec, &counts_vec, usize::MAX, Some(&mut reason));
                (ok, reason)
            }
            _ => (false, "Could not read topology attributes".to_owned()),
        }
    }

    #[staticmethod]
    pub fn sharpness_infinite() -> f32 { SHARPNESS_INFINITE }
}

// ============================================================================
// Sphere
// ============================================================================

#[pyclass(name = "Sphere", module = "pxr_rs.UsdGeom")]
pub struct PySphere(pub Sphere);

#[pymethods]
impl PySphere {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Sphere::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Sphere::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Sphere::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.get_radius_attr()) }
    pub fn create_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.create_radius_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Sphere('{}')", self.0.prim().path()) }
        else { "UsdGeom.Sphere(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Sphere" }
}

// ============================================================================
// Cube
// ============================================================================

#[pyclass(name = "Cube", module = "pxr_rs.UsdGeom")]
pub struct PyCube(pub Cube);

#[pymethods]
impl PyCube {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Cube::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cube::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cube::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_size_attr(&self) -> PyAttribute { PyAttribute(self.0.get_size_attr()) }
    pub fn create_size_attr(&self) -> PyAttribute { PyAttribute(self.0.create_size_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Cube('{}')", self.0.prim().path()) }
        else { "UsdGeom.Cube(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Cube" }
}

// ============================================================================
// Cone
// ============================================================================

#[pyclass(name = "Cone", module = "pxr_rs.UsdGeom")]
pub struct PyCone(pub Cone);

#[pymethods]
impl PyCone {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Cone::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cone::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cone::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.get_radius_attr()) }
    pub fn create_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.create_radius_attr(None, false)) }
    pub fn get_height_attr(&self) -> PyAttribute { PyAttribute(self.0.get_height_attr()) }
    pub fn create_height_attr(&self) -> PyAttribute { PyAttribute(self.0.create_height_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.create_axis_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Cone('{}')", self.0.prim().path()) }
        else { "UsdGeom.Cone(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Cone" }
}

// ============================================================================
// Cylinder
// ============================================================================

#[pyclass(name = "Cylinder", module = "pxr_rs.UsdGeom")]
pub struct PyCylinder(pub Cylinder);

#[pymethods]
impl PyCylinder {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Cylinder::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cylinder::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cylinder::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.get_radius_attr()) }
    pub fn create_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.create_radius_attr(None, false)) }
    pub fn get_height_attr(&self) -> PyAttribute { PyAttribute(self.0.get_height_attr()) }
    pub fn create_height_attr(&self) -> PyAttribute { PyAttribute(self.0.create_height_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.create_axis_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Cylinder('{}')", self.0.prim().path()) }
        else { "UsdGeom.Cylinder(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Cylinder" }
}

// ============================================================================
// Cylinder_1
// ============================================================================

#[pyclass(name = "Cylinder_1", module = "pxr_rs.UsdGeom")]
pub struct PyCylinder1(pub Cylinder1);

#[pymethods]
impl PyCylinder1 {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Cylinder1::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cylinder1::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Cylinder1::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    // Cylinder1 delegates attribute access to the inner Cylinder schema.
    pub fn get_radius_bottom_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().get_radius_bottom_attr()) }
    pub fn create_radius_bottom_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().create_radius_bottom_attr(None, false)) }
    pub fn get_radius_top_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().get_radius_top_attr()) }
    pub fn create_radius_top_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().create_radius_top_attr(None, false)) }
    pub fn get_height_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().get_height_attr()) }
    pub fn create_height_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().create_height_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.as_cylinder().create_axis_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Cylinder_1('{}')", self.0.prim().path()) }
        else { "UsdGeom.Cylinder_1(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Cylinder_1" }
}

// ============================================================================
// Capsule
// ============================================================================

#[pyclass(name = "Capsule", module = "pxr_rs.UsdGeom")]
pub struct PyCapsule(pub Capsule);

#[pymethods]
impl PyCapsule {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Capsule::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Capsule::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Capsule::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.get_radius_attr()) }
    pub fn create_radius_attr(&self) -> PyAttribute { PyAttribute(self.0.create_radius_attr(None, false)) }
    pub fn get_height_attr(&self) -> PyAttribute { PyAttribute(self.0.get_height_attr()) }
    pub fn create_height_attr(&self) -> PyAttribute { PyAttribute(self.0.create_height_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.create_axis_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Capsule('{}')", self.0.prim().path()) }
        else { "UsdGeom.Capsule(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Capsule" }
}

// ============================================================================
// Capsule_1
// ============================================================================

#[pyclass(name = "Capsule_1", module = "pxr_rs.UsdGeom")]
pub struct PyCapsule1(pub Capsule1);

#[pymethods]
impl PyCapsule1 {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Capsule1::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Capsule1::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Capsule1::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    // Capsule1 delegates attribute access to the inner Capsule schema.
    pub fn get_radius_top_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().get_radius_top_attr()) }
    pub fn create_radius_top_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().create_radius_top_attr(None, false)) }
    pub fn get_radius_bottom_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().get_radius_bottom_attr()) }
    pub fn create_radius_bottom_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().create_radius_bottom_attr(None, false)) }
    pub fn get_height_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().get_height_attr()) }
    pub fn create_height_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().create_height_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.as_capsule().create_axis_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Capsule_1('{}')", self.0.prim().path()) }
        else { "UsdGeom.Capsule_1(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Capsule_1" }
}

// ============================================================================
// Plane
// ============================================================================

#[pyclass(name = "Plane", module = "pxr_rs.UsdGeom")]
pub struct PyPlane(pub Plane);

#[pymethods]
impl PyPlane {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Plane::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Plane::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Plane::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_width_attr(&self) -> PyAttribute { PyAttribute(self.0.get_width_attr()) }
    pub fn create_width_attr(&self) -> PyAttribute { PyAttribute(self.0.create_width_attr(None, false)) }
    pub fn get_length_attr(&self) -> PyAttribute { PyAttribute(self.0.get_length_attr()) }
    pub fn create_length_attr(&self) -> PyAttribute { PyAttribute(self.0.create_length_attr(None, false)) }
    pub fn get_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.get_axis_attr()) }
    pub fn create_axis_attr(&self) -> PyAttribute { PyAttribute(self.0.create_axis_attr(None, false)) }
    pub fn get_double_sided_attr(&self) -> PyAttribute { PyAttribute(self.0.gprim().get_double_sided_attr()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Plane('{}')", self.0.prim().path()) }
        else { "UsdGeom.Plane(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Plane" }
}

// ============================================================================
// PointBased
// ============================================================================

#[pyclass(name = "PointBased", module = "pxr_rs.UsdGeom")]
pub struct PyPointBased(pub PointBased);

#[pymethods]
impl PyPointBased {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(PointBased::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_points_attr(&self) -> PyAttribute { PyAttribute(self.0.get_points_attr()) }
    pub fn create_points_attr(&self) -> PyAttribute { PyAttribute(self.0.create_points_attr(None, false)) }
    pub fn get_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.get_velocities_attr()) }
    pub fn create_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.create_velocities_attr(None, false)) }
    pub fn get_normals_attr(&self) -> PyAttribute { PyAttribute(self.0.get_normals_attr()) }
    pub fn create_normals_attr(&self) -> PyAttribute { PyAttribute(self.0.create_normals_attr(None, false)) }

    pub fn get_normals_interpolation(&self) -> String {
        self.0.get_normals_interpolation().as_str().to_owned()
    }

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.PointBased('{}')", self.0.prim().path()) }
        else { "UsdGeom.PointBased(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "PointBased" }
}

// ============================================================================
// Points
// ============================================================================

#[pyclass(name = "Points", module = "pxr_rs.UsdGeom")]
pub struct PyPoints(pub Points);

#[pymethods]
impl PyPoints {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Points::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Points::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Points::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_points_attr(&self) -> PyAttribute { PyAttribute(self.0.point_based().get_points_attr()) }
    pub fn get_widths_attr(&self) -> PyAttribute { PyAttribute(self.0.get_widths_attr()) }
    pub fn create_widths_attr(&self) -> PyAttribute { PyAttribute(self.0.create_widths_attr(None, false)) }
    pub fn get_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.get_ids_attr()) }
    pub fn create_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.create_ids_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Points('{}')", self.0.prim().path()) }
        else { "UsdGeom.Points(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Points" }
}

// ============================================================================
// Curves
// ============================================================================

#[pyclass(name = "Curves", module = "pxr_rs.UsdGeom")]
pub struct PyCurves(pub Curves);

#[pymethods]
impl PyCurves {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Curves::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_curve_vertex_counts_attr(&self) -> PyAttribute { PyAttribute(self.0.get_curve_vertex_counts_attr()) }
    pub fn create_curve_vertex_counts_attr(&self) -> PyAttribute { PyAttribute(self.0.create_curve_vertex_counts_attr(None, false)) }
    pub fn get_widths_attr(&self) -> PyAttribute { PyAttribute(self.0.get_widths_attr()) }
    pub fn create_widths_attr(&self) -> PyAttribute { PyAttribute(self.0.create_widths_attr(None, false)) }
    pub fn get_widths_interpolation(&self) -> String { self.0.get_widths_interpolation().as_str().to_owned() }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Curves('{}')", self.0.prim().path()) }
        else { "UsdGeom.Curves(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Curves" }
}

// ============================================================================
// BasisCurves
// ============================================================================

#[pyclass(name = "BasisCurves", module = "pxr_rs.UsdGeom")]
pub struct PyBasisCurves(pub BasisCurves);

#[pymethods]
impl PyBasisCurves {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(BasisCurves::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(BasisCurves::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(BasisCurves::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_basis_attr(&self) -> PyAttribute { PyAttribute(self.0.get_basis_attr()) }
    pub fn create_basis_attr(&self) -> PyAttribute { PyAttribute(self.0.create_basis_attr(None, false)) }
    pub fn get_type_attr(&self) -> PyAttribute { PyAttribute(self.0.get_type_attr()) }
    pub fn create_type_attr(&self) -> PyAttribute { PyAttribute(self.0.create_type_attr(None, false)) }
    pub fn get_wrap_attr(&self) -> PyAttribute { PyAttribute(self.0.get_wrap_attr()) }
    pub fn create_wrap_attr(&self) -> PyAttribute { PyAttribute(self.0.create_wrap_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.BasisCurves('{}')", self.0.prim().path()) }
        else { "UsdGeom.BasisCurves(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "BasisCurves" }
}

// ============================================================================
// NurbsCurves
// ============================================================================

#[pyclass(name = "NurbsCurves", module = "pxr_rs.UsdGeom")]
pub struct PyNurbsCurves(pub NurbsCurves);

#[pymethods]
impl PyNurbsCurves {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(NurbsCurves::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(NurbsCurves::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(NurbsCurves::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_order_attr(&self) -> PyAttribute { PyAttribute(self.0.get_order_attr()) }
    pub fn create_order_attr(&self) -> PyAttribute { PyAttribute(self.0.create_order_attr(None, false)) }
    pub fn get_knots_attr(&self) -> PyAttribute { PyAttribute(self.0.get_knots_attr()) }
    pub fn create_knots_attr(&self) -> PyAttribute { PyAttribute(self.0.create_knots_attr(None, false)) }
    pub fn get_ranges_attr(&self) -> PyAttribute { PyAttribute(self.0.get_ranges_attr()) }
    pub fn create_ranges_attr(&self) -> PyAttribute { PyAttribute(self.0.create_ranges_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.NurbsCurves('{}')", self.0.prim().path()) }
        else { "UsdGeom.NurbsCurves(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "NurbsCurves" }
}

// ============================================================================
// HermiteCurves
// ============================================================================

#[pyclass(name = "HermiteCurves", module = "pxr_rs.UsdGeom")]
pub struct PyHermiteCurves(pub HermiteCurves);

#[pymethods]
impl PyHermiteCurves {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(HermiteCurves::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(HermiteCurves::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(HermiteCurves::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_tangents_attr(&self) -> PyAttribute { PyAttribute(self.0.get_tangents_attr()) }
    pub fn create_tangents_attr(&self) -> PyAttribute { PyAttribute(self.0.create_tangents_attr(None, false)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.HermiteCurves('{}')", self.0.prim().path()) }
        else { "UsdGeom.HermiteCurves(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "HermiteCurves" }
}

// ============================================================================
// NurbsPatch
// ============================================================================

#[pyclass(name = "NurbsPatch", module = "pxr_rs.UsdGeom")]
pub struct PyNurbsPatch(pub NurbsPatch);

#[pymethods]
impl PyNurbsPatch {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(NurbsPatch::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(NurbsPatch::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(NurbsPatch::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_u_vertex_count_attr(&self) -> PyAttribute { PyAttribute(self.0.get_u_vertex_count_attr()) }
    pub fn get_v_vertex_count_attr(&self) -> PyAttribute { PyAttribute(self.0.get_v_vertex_count_attr()) }
    pub fn get_u_order_attr(&self) -> PyAttribute { PyAttribute(self.0.get_u_order_attr()) }
    pub fn get_v_order_attr(&self) -> PyAttribute { PyAttribute(self.0.get_v_order_attr()) }
    pub fn get_u_knots_attr(&self) -> PyAttribute { PyAttribute(self.0.get_u_knots_attr()) }
    pub fn get_v_knots_attr(&self) -> PyAttribute { PyAttribute(self.0.get_v_knots_attr()) }
    pub fn get_u_range_attr(&self) -> PyAttribute { PyAttribute(self.0.get_u_range_attr()) }
    pub fn get_v_range_attr(&self) -> PyAttribute { PyAttribute(self.0.get_v_range_attr()) }
    pub fn get_u_form_attr(&self) -> PyAttribute { PyAttribute(self.0.get_u_form_attr()) }
    pub fn get_v_form_attr(&self) -> PyAttribute { PyAttribute(self.0.get_v_form_attr()) }
    pub fn get_point_weights_attr(&self) -> PyAttribute { PyAttribute(self.0.get_point_weights_attr()) }
    pub fn get_trim_curve_counts_attr(&self) -> PyAttribute { PyAttribute(self.0.get_trim_curve_counts_attr()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.NurbsPatch('{}')", self.0.prim().path()) }
        else { "UsdGeom.NurbsPatch(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "NurbsPatch" }
}

// ============================================================================
// TetMesh
// ============================================================================

#[pyclass(name = "TetMesh", module = "pxr_rs.UsdGeom")]
pub struct PyTetMesh(pub TetMesh);

#[pymethods]
impl PyTetMesh {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(TetMesh::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(TetMesh::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(TetMesh::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_tet_vertex_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_tet_vertex_indices_attr()) }
    pub fn create_tet_vertex_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_tet_vertex_indices_attr(None, false)) }
    pub fn get_surface_face_vertex_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_surface_face_vertex_indices_attr()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.TetMesh('{}')", self.0.prim().path()) }
        else { "UsdGeom.TetMesh(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "TetMesh" }
}

// ============================================================================
// PointInstancer
// ============================================================================

#[pyclass(name = "PointInstancer", module = "pxr_rs.UsdGeom")]
pub struct PyPointInstancer(pub PointInstancer);

#[pymethods]
impl PyPointInstancer {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(PointInstancer::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(PointInstancer::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(PointInstancer::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }

    pub fn get_proto_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_proto_indices_attr()) }
    pub fn create_proto_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_proto_indices_attr(None, false)) }
    pub fn get_positions_attr(&self) -> PyAttribute { PyAttribute(self.0.get_positions_attr()) }
    pub fn create_positions_attr(&self) -> PyAttribute { PyAttribute(self.0.create_positions_attr(None, false)) }
    pub fn get_orientations_attr(&self) -> PyAttribute { PyAttribute(self.0.get_orientations_attr()) }
    pub fn create_orientations_attr(&self) -> PyAttribute { PyAttribute(self.0.create_orientations_attr(None, false)) }
    pub fn get_scales_attr(&self) -> PyAttribute { PyAttribute(self.0.get_scales_attr()) }
    pub fn create_scales_attr(&self) -> PyAttribute { PyAttribute(self.0.create_scales_attr(None, false)) }
    pub fn get_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.get_velocities_attr()) }
    pub fn create_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.create_velocities_attr(None, false)) }
    pub fn get_angular_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.get_angular_velocities_attr()) }
    pub fn create_angular_velocities_attr(&self) -> PyAttribute { PyAttribute(self.0.create_angular_velocities_attr(None, false)) }
    pub fn get_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.get_ids_attr()) }
    pub fn create_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.create_ids_attr(None, false)) }
    pub fn get_invisible_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.get_invisible_ids_attr()) }
    pub fn create_invisible_ids_attr(&self) -> PyAttribute { PyAttribute(self.0.create_invisible_ids_attr(None, false)) }

    pub fn get_prototypes_rel(&self) -> Vec<String> {
        let rel = self.0.get_prototypes_rel();
        rel.get_targets().into_iter().map(|p| p.to_string()).collect()
    }

    pub fn activate_id(&self, id: i64) -> bool { self.0.activate_id(id) }
    pub fn activate_ids(&self, ids: Vec<i64>) -> bool { self.0.activate_ids(&ids) }
    pub fn deactivate_id(&self, id: i64) -> bool { self.0.deactivate_id(id) }
    pub fn deactivate_ids(&self, ids: Vec<i64>) -> bool { self.0.deactivate_ids(&ids) }
    pub fn vis_id(&self, id: i64, time: Option<f64>) -> bool { self.0.vis_id(id, tc(time)) }
    pub fn vis_ids(&self, ids: Vec<i64>, time: Option<f64>) -> bool { self.0.vis_ids(&ids, tc(time)) }
    pub fn invis_id(&self, id: i64, time: Option<f64>) -> bool { self.0.invis_id(id, tc(time)) }
    pub fn invis_ids(&self, ids: Vec<i64>, time: Option<f64>) -> bool { self.0.invis_ids(&ids, tc(time)) }
    pub fn activate_all_ids(&self) -> bool { self.0.activate_all_ids() }
    pub fn vis_all_ids(&self, time: Option<f64>) -> bool { self.0.vis_all_ids(tc(time)) }

    /// Compute per-instance transforms. Returns flat 16-element list per instance.
    pub fn compute_instance_transforms_at_time(&self, time: f64, base_time: f64) -> Vec<Vec<f64>> {
        use usd_geom::point_instancer::{ProtoXformInclusion, MaskApplication};
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

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.PointInstancer('{}')", self.0.prim().path()) }
        else { "UsdGeom.PointInstancer(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "PointInstancer" }
}

// ============================================================================
// Camera
// ============================================================================

#[pyclass(name = "Camera", module = "pxr_rs.UsdGeom")]
pub struct PyCamera(pub Camera);

#[pymethods]
impl PyCamera {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Camera::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Camera::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Camera::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_projection_attr(&self) -> PyAttribute { PyAttribute(self.0.get_projection_attr()) }
    pub fn create_projection_attr(&self) -> PyAttribute { PyAttribute(self.0.create_projection_attr(None, false)) }
    pub fn get_horizontal_aperture_attr(&self) -> PyAttribute { PyAttribute(self.0.get_horizontal_aperture_attr()) }
    pub fn create_horizontal_aperture_attr(&self) -> PyAttribute { PyAttribute(self.0.create_horizontal_aperture_attr(None, false)) }
    pub fn get_vertical_aperture_attr(&self) -> PyAttribute { PyAttribute(self.0.get_vertical_aperture_attr()) }
    pub fn create_vertical_aperture_attr(&self) -> PyAttribute { PyAttribute(self.0.create_vertical_aperture_attr(None, false)) }
    pub fn get_horizontal_aperture_offset_attr(&self) -> PyAttribute { PyAttribute(self.0.get_horizontal_aperture_offset_attr()) }
    pub fn create_horizontal_aperture_offset_attr(&self) -> PyAttribute { PyAttribute(self.0.create_horizontal_aperture_offset_attr(None, false)) }
    pub fn get_vertical_aperture_offset_attr(&self) -> PyAttribute { PyAttribute(self.0.get_vertical_aperture_offset_attr()) }
    pub fn create_vertical_aperture_offset_attr(&self) -> PyAttribute { PyAttribute(self.0.create_vertical_aperture_offset_attr(None, false)) }
    pub fn get_focal_length_attr(&self) -> PyAttribute { PyAttribute(self.0.get_focal_length_attr()) }
    pub fn create_focal_length_attr(&self) -> PyAttribute { PyAttribute(self.0.create_focal_length_attr(None, false)) }
    pub fn get_clipping_range_attr(&self) -> PyAttribute { PyAttribute(self.0.get_clipping_range_attr()) }
    pub fn create_clipping_range_attr(&self) -> PyAttribute { PyAttribute(self.0.create_clipping_range_attr(None, false)) }
    pub fn get_clipping_planes_attr(&self) -> PyAttribute { PyAttribute(self.0.get_clipping_planes_attr()) }
    pub fn create_clipping_planes_attr(&self) -> PyAttribute { PyAttribute(self.0.create_clipping_planes_attr(None, false)) }
    pub fn get_f_stop_attr(&self) -> PyAttribute { PyAttribute(self.0.get_f_stop_attr()) }
    pub fn create_f_stop_attr(&self) -> PyAttribute { PyAttribute(self.0.create_f_stop_attr(None, false)) }
    pub fn get_focus_distance_attr(&self) -> PyAttribute { PyAttribute(self.0.get_focus_distance_attr()) }
    pub fn create_focus_distance_attr(&self) -> PyAttribute { PyAttribute(self.0.create_focus_distance_attr(None, false)) }
    pub fn get_shutter_open_attr(&self) -> PyAttribute { PyAttribute(self.0.get_shutter_open_attr()) }
    pub fn create_shutter_open_attr(&self) -> PyAttribute { PyAttribute(self.0.create_shutter_open_attr(None, false)) }
    pub fn get_shutter_close_attr(&self) -> PyAttribute { PyAttribute(self.0.get_shutter_close_attr()) }
    pub fn create_shutter_close_attr(&self) -> PyAttribute { PyAttribute(self.0.create_shutter_close_attr(None, false)) }
    pub fn get_stereo_role_attr(&self) -> PyAttribute { PyAttribute(self.0.get_stereo_role_attr()) }
    pub fn create_stereo_role_attr(&self) -> PyAttribute { PyAttribute(self.0.create_stereo_role_attr(None, false)) }

    /// Returns key camera parameters as a Python dict.
    pub fn get_camera(&self, py: Python<'_>, time: Option<f64>) -> PyResult<Py<PyAny>> {
        let gf_cam = self.0.get_camera(tc(time));
        let d = pyo3::types::PyDict::new(py);
        d.set_item("focalLength", gf_cam.focal_length())?;
        d.set_item("horizontalAperture", gf_cam.horizontal_aperture())?;
        d.set_item("verticalAperture", gf_cam.vertical_aperture())?;
        Ok(d.into_any().unbind())
    }

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Camera('{}')", self.0.prim().path()) }
        else { "UsdGeom.Camera(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Camera" }
}

// ============================================================================
// PrimvarsAPI
// ============================================================================

#[pyclass(name = "PrimvarsAPI", module = "pxr_rs.UsdGeom")]
pub struct PyPrimvarsAPI(pub PrimvarsAPI);

#[pymethods]
impl PyPrimvarsAPI {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(PrimvarsAPI::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }

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
        PyPrimvar(self.0.create_primvar(&Token::new(name), &type_name, interp_tok.as_ref(), element_size))
    }

    pub fn get_primvar(&self, name: &str) -> PyPrimvar { PyPrimvar(self.0.get_primvar(&Token::new(name))) }
    pub fn get_primvars(&self) -> Vec<PyPrimvar> { self.0.get_primvars().into_iter().map(PyPrimvar).collect() }
    pub fn get_authored_primvars(&self) -> Vec<PyPrimvar> { self.0.get_authored_primvars().into_iter().map(PyPrimvar).collect() }
    pub fn has_primvar(&self, name: &str) -> bool { self.0.has_primvar(&Token::new(name)) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.PrimvarsAPI('{}')", self.0.prim().path()) }
        else { "UsdGeom.PrimvarsAPI(<invalid>)".to_owned() }
    }
}

// ============================================================================
// VisibilityAPI
// ============================================================================

#[pyclass(name = "VisibilityAPI", module = "pxr_rs.UsdGeom")]
pub struct PyVisibilityAPI(pub VisibilityAPI);

#[pymethods]
impl PyVisibilityAPI {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(VisibilityAPI::new(prim.0.clone())) }

    #[staticmethod]
    pub fn apply(prim: &PyPrim) -> Self { Self(VisibilityAPI::apply(&prim.0)) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_guide_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.get_guide_visibility_attr()) }
    pub fn create_guide_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.create_guide_visibility_attr()) }
    pub fn get_proxy_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.get_proxy_visibility_attr()) }
    pub fn create_proxy_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.create_proxy_visibility_attr()) }
    pub fn get_render_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.get_render_visibility_attr()) }
    pub fn create_render_visibility_attr(&self) -> PyAttribute { PyAttribute(self.0.create_render_visibility_attr()) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.VisibilityAPI('{}')", self.0.prim().path()) }
        else { "UsdGeom.VisibilityAPI(<invalid>)".to_owned() }
    }
}

// ============================================================================
// ModelAPI
// ============================================================================

#[pyclass(name = "ModelAPI", module = "pxr_rs.UsdGeom")]
pub struct PyModelAPI(pub ModelAPI);

#[pymethods]
impl PyModelAPI {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(ModelAPI::new(prim.0.clone())) }

    #[staticmethod]
    pub fn apply(prim: &PyPrim) -> Self {
        ModelAPI::apply(&prim.0)
            .map_or_else(|| Self(ModelAPI::new(prim.0.clone())), |api| Self(api))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.get_prim().clone()) }
    pub fn get_model_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute(self.0.get_model_draw_mode_attr().unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn create_model_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute(self.0.create_model_draw_mode_attr(None).unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn get_model_apply_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute(self.0.get_model_apply_draw_mode_attr().unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn create_model_apply_draw_mode_attr(&self) -> PyAttribute {
        PyAttribute(self.0.create_model_apply_draw_mode_attr(None).unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn get_model_card_geometry_attr(&self) -> PyAttribute {
        PyAttribute(self.0.get_model_card_geometry_attr().unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn get_extents_hint_attr(&self) -> PyAttribute {
        PyAttribute(self.0.get_extents_hint_attr().unwrap_or_else(usd_core::Attribute::invalid))
    }
    pub fn is_valid(&self) -> bool { self.0.get_prim().is_valid() }
    pub fn __bool__(&self) -> bool { self.0.get_prim().is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.get_prim().is_valid() { format!("UsdGeom.ModelAPI('{}')", self.0.get_prim().path()) }
        else { "UsdGeom.ModelAPI(<invalid>)".to_owned() }
    }
}

// ============================================================================
// MotionAPI
// ============================================================================

#[pyclass(name = "MotionAPI", module = "pxr_rs.UsdGeom")]
pub struct PyMotionAPI(pub MotionAPI);

#[pymethods]
impl PyMotionAPI {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(MotionAPI::new(prim.0.clone())) }

    #[staticmethod]
    pub fn apply(prim: &PyPrim) -> Self { Self(MotionAPI::apply(&prim.0)) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_motion_blur_scale_attr(&self) -> PyAttribute { PyAttribute(self.0.get_motion_blur_scale_attr()) }
    pub fn create_motion_blur_scale_attr(&self) -> PyAttribute { PyAttribute(self.0.create_motion_blur_scale_attr(None, false)) }
    pub fn get_velocity_scale_attr(&self) -> PyAttribute { PyAttribute(self.0.get_motion_velocity_scale_attr()) }
    pub fn create_velocity_scale_attr(&self) -> PyAttribute { PyAttribute(self.0.create_motion_velocity_scale_attr(None, false)) }
    pub fn get_nonlinear_sample_count_attr(&self) -> PyAttribute { PyAttribute(self.0.get_motion_nonlinear_sample_count_attr()) }
    pub fn create_nonlinear_sample_count_attr(&self) -> PyAttribute { PyAttribute(self.0.create_motion_nonlinear_sample_count_attr(None, false)) }
    pub fn compute_velocity_scale(&self, time: Option<f64>) -> f64 { f64::from(self.0.compute_velocity_scale(tc(time))) }
    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.MotionAPI('{}')", self.0.prim().path()) }
        else { "UsdGeom.MotionAPI(<invalid>)".to_owned() }
    }
}

// ============================================================================
// XformCommonAPI
// ============================================================================

#[pyclass(name = "XformCommonAPI", module = "pxr_rs.UsdGeom")]
pub struct PyXformCommonAPI(pub XformCommonAPI);

#[pymethods]
impl PyXformCommonAPI {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(XformCommonAPI::new(prim.0.clone())) }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }

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
        Ok(self.0.set_xform_vectors(tr, rot, sc, pv, rot_order, tc(time)))
    }

    pub fn get_xform_vectors(
        &self,
        time: Option<f64>,
    ) -> ((f64, f64, f64), (f32, f32, f32), (f32, f32, f32), (f32, f32, f32), String) {
        let mut tr = usd_gf::Vec3d::new(0.0, 0.0, 0.0);
        let mut rot = usd_gf::Vec3f::new(0.0, 0.0, 0.0);
        let mut sc = usd_gf::Vec3f::new(1.0, 1.0, 1.0);
        let mut pv = usd_gf::Vec3f::new(0.0, 0.0, 0.0);
        let mut rot_order = RotationOrder::XYZ;
        self.0.get_xform_vectors(&mut tr, &mut rot, &mut sc, &mut pv, &mut rot_order, tc(time));
        (
            (tr.x, tr.y, tr.z),
            (rot.x, rot.y, rot.z),
            (sc.x, sc.y, sc.z),
            (pv.x, pv.y, pv.z),
            format!("{rot_order:?}"),
        )
    }

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.XformCommonAPI('{}')", self.0.prim().path()) }
        else { "UsdGeom.XformCommonAPI(<invalid>)".to_owned() }
    }
}

// ============================================================================
// Subset
// ============================================================================

#[pyclass(name = "Subset", module = "pxr_rs.UsdGeom")]
pub struct PySubset(pub Subset);

#[pymethods]
impl PySubset {
    #[new]
    pub fn new(prim: &PyPrim) -> Self { Self(Subset::new(prim.0.clone())) }

    #[staticmethod]
    pub fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Subset::get(&stage.0, &p)))
    }

    #[staticmethod]
    pub fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = parse_path(path)?;
        Ok(Self(Subset::define(&stage.0, &p)))
    }

    pub fn get_prim(&self) -> PyPrim { PyPrim(self.0.prim().clone()) }
    pub fn get_element_type_attr(&self) -> PyAttribute { PyAttribute(self.0.get_element_type_attr()) }
    pub fn create_element_type_attr(&self) -> PyAttribute { PyAttribute(self.0.create_element_type_attr(None, false)) }
    pub fn get_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.get_indices_attr()) }
    pub fn create_indices_attr(&self) -> PyAttribute { PyAttribute(self.0.create_indices_attr(None, false)) }
    pub fn get_family_name_attr(&self) -> PyAttribute { PyAttribute(self.0.get_family_name_attr()) }
    pub fn create_family_name_attr(&self) -> PyAttribute { PyAttribute(self.0.create_family_name_attr(None, false)) }

    /// Create a geometry subset child prim under a mesh prim.
    #[staticmethod]
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

    pub fn is_valid(&self) -> bool { self.0.is_valid() }
    pub fn __bool__(&self) -> bool { self.0.is_valid() }

    pub fn __repr__(&self) -> String {
        if self.0.is_valid() { format!("UsdGeom.Subset('{}')", self.0.prim().path()) }
        else { "UsdGeom.Subset(<invalid>)".to_owned() }
    }

    #[staticmethod]
    pub fn get_schema_type_name() -> &'static str { "Subset" }
}

// ============================================================================
// Metrics — module-level functions
// ============================================================================

/// Get stage up axis token ("Y" or "Z").
#[pyfunction]
pub fn get_stage_up_axis(stage: &PyStage) -> String {
    metrics::get_stage_up_axis(&stage.0).as_str().to_owned()
}

/// Set stage up axis. Returns true on success.
#[pyfunction]
pub fn set_stage_up_axis(stage: &PyStage, axis: &str) -> bool {
    metrics::set_stage_up_axis(&stage.0, &Token::new(axis))
}

/// Get stage meters-per-unit (e.g. 0.01 = centimeters).
#[pyfunction]
pub fn get_stage_meters_per_unit(stage: &PyStage) -> f64 {
    metrics::get_stage_meters_per_unit(&stage.0)
}

/// Set stage meters-per-unit. Returns true on success.
#[pyfunction]
pub fn set_stage_meters_per_unit(stage: &PyStage, mpu: f64) -> bool {
    metrics::set_stage_meters_per_unit(&stage.0, mpu)
}

// ============================================================================
// Register
// ============================================================================

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Tokens
    m.add_class::<PyTokens>()?;

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
