//! Geometric Python types: Rotation, BBox3d, Plane, Line, LineSeg, Ray,
//! Interval, MultiInterval, Rect2i, Size2, Size3, Transform, Camera.
//!
//! Many getters intentionally use camelCase names to match pxr.Gf Python API.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError, PyTypeError};
use usd_gf::{
    BBox3d, Rotation, Interval, MultiInterval, Rect2i, Size2, Size3,
    Range1d, Range1f, Range2d, Range2f, Range3d, Range3f,
    Frustum, ProjectionType, Color, ColorSpace, ColorSpaceName,
};

/// Hash helper for f64 slices
fn hash_f64_n(vals: &[f64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in vals { v.to_bits().hash(&mut h); }
    h.finish()
}

/// Extract a Vec3d from PyVec3d or a 3-tuple of floats.
fn extract_vec3d(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<usd_gf::Vec3d> {
    if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec3d>>() {
        return Ok(v.0);
    }
    if let Ok(t) = obj.extract::<(f64, f64, f64)>() {
        return Ok(usd_gf::Vec3d::new(t.0, t.1, t.2));
    }
    Err(PyTypeError::new_err("expected Vec3d or 3-tuple of floats"))
}

/// Hash helper for i32 pairs
fn hash_i32_n(vals: &[i32]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in vals { v.hash(&mut h); }
    h.finish()
}

// ---------------------------------------------------------------------------
// Rotation
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Rotation", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRotation(pub Rotation);

#[pymethods]
impl PyRotation {
    /// Constructor: Rotation(), Rotation(axis, angle), Rotation(from_vec, to_vec),
    /// Rotation(Quaternion), Rotation(Quatd), Rotation(Rotation), Rotation(Matrix3d)
    #[new]
    #[pyo3(signature = (axis_or_quat=None, angle_or_to=None))]
    fn new(axis_or_quat: Option<&Bound<'_, pyo3::PyAny>>, angle_or_to: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let Some(obj) = axis_or_quat else {
            return Ok(Self(Rotation::new()));
        };
        // Rotation(Rotation) — copy constructor
        if let Ok(r) = obj.extract::<PyRef<'_, PyRotation>>() {
            return Ok(Self(r.0.clone()));
        }
        // Rotation(Quaternion)
        if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuaternion>>() {
            return Ok(Self(Rotation::from_quaternion(&q.0)));
        }
        // Rotation(Quatd)
        if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuatd>>() {
            return Ok(Self(Rotation::from_quat(&q.0)));
        }
        // Rotation(Matrix3d) — extract rotation from 3x3 matrix
        if let Ok(m) = obj.extract::<PyRef<'_, super::matrix::PyMatrix3d>>() {
            let mut r = Rotation::new();
            r.set_matrix(&m.0);
            return Ok(Self(r));
        }
        // Two-arg forms: Rotation(Vec3d, angle) or Rotation(Vec3d, Vec3d)
        if let Ok(ax) = obj.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            if let Some(second) = angle_or_to {
                // Rotation(Vec3d, Vec3d) — rotate from→to
                if let Ok(to) = second.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                    return Ok(Self(Rotation::from_rotate_into(&ax.0, &to.0)));
                }
                // Rotation(Vec3d, angle)
                let a: f64 = second.extract()?;
                return Ok(Self(Rotation::from_axis_angle(ax.0, a)));
            }
            return Ok(Self(Rotation::from_axis_angle(ax.0, 0.0)));
        }
        // Rotation((x,y,z), angle) — tuple axis
        if let Ok(tup) = obj.extract::<(f64, f64, f64)>() {
            let a: f64 = if let Some(second) = angle_or_to { second.extract()? } else { 0.0 };
            return Ok(Self(Rotation::from_axis_angle(usd_gf::Vec3d::new(tup.0, tup.1, tup.2), a)));
        }
        Err(PyTypeError::new_err("Rotation: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String {
        let ax = self.0.axis();
        format!("Gf.Rotation(Gf.Vec3d({}, {}, {}), {})", ax.x, ax.y, ax.z, self.0.angle())
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let ax = self.0.axis();
        hash_f64_n(&[ax.x, ax.y, ax.z, self.0.angle()])
    }
    /// Rotation * Rotation -> composed, Rotation * scalar -> scale angle
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(r) = o.extract::<PyRef<'_, PyRotation>>() {
            return Ok(Self(self.0 * r.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(Rotation::from_axis_angle(self.0.axis(), self.0.angle() * s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(PyTypeError::new_err("Rotation.__mul__: expected Rotation or scalar"))
    }
    fn __rmul__(&self, s: f64) -> Self {
        Self(Rotation::from_axis_angle(self.0.axis(), self.0.angle() * s))
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(PyTypeError::new_err("division by zero")); }
        Ok(Self(Rotation::from_axis_angle(self.0.axis(), self.0.angle() / s)))
    }

    #[pyo3(name = "SetAxisAngle")] fn set_axis_angle(&mut self, axis: &super::vec::PyVec3d, angle: f64) -> Self {
        self.0.set_axis_angle(axis.0, angle);
        self.clone()
    }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) -> Self {
        self.0.set_identity();
        self.clone()
    }
    #[pyo3(name = "SetQuat")] fn set_quat(&mut self, q: &super::quat::PyQuatd) -> Self {
        self.0.set_quat(&q.0);
        self.clone()
    }
    #[pyo3(name = "SetQuaternion")] fn set_quaternion(&mut self, q: &super::quat::PyQuaternion) -> Self {
        self.0 = Rotation::from_quaternion(&q.0);
        self.clone()
    }
    #[pyo3(name = "SetRotateInto")] fn set_rotate_into(&mut self, from: &super::vec::PyVec3d, to: &super::vec::PyVec3d) -> Self {
        self.0.set_rotate_into(&from.0, &to.0);
        self.clone()
    }

    #[pyo3(name = "GetAxis")]  fn get_axis(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.axis()) }
    #[pyo3(name = "GetAngle")] fn get_angle(&self) -> f64 { self.0.angle() }
    #[pyo3(name = "GetQuat")]  fn get_quat(&self) -> super::quat::PyQuatd {
        super::quat::PyQuatd(self.0.get_quat())
    }
    /// GetQuaternion() -> Quaternion (legacy GfQuaternion type)
    #[pyo3(name = "GetQuaternion")] fn get_quaternion(&self) -> super::quat::PyQuaternion {
        let q = self.0.get_quat();
        super::quat::PyQuaternion(usd_gf::Quaternion::new(q.real(), *q.imaginary()))
    }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }

    /// TransformDir(Vec3d) -> Vec3d: rotate a direction vector
    #[pyo3(name = "TransformDir")]
    fn transform_dir(&self, py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        // Accept Vec3d
        if let Ok(vd) = v.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            return Ok(super::vec::PyVec3d(self.0.transform_dir(&vd.0)).into_pyobject(py)?.into_any().unbind());
        }
        // Accept Vec3f (convert through Vec3d)
        if let Ok(vf) = v.extract::<PyRef<'_, super::vec::PyVec3f>>() {
            let d = usd_gf::Vec3d::new(vf.0.x as f64, vf.0.y as f64, vf.0.z as f64);
            let rd = self.0.transform_dir(&d);
            return Ok(super::vec::PyVec3f(usd_gf::Vec3f::new(rd.x as f32, rd.y as f32, rd.z as f32)).into_pyobject(py)?.into_any().unbind());
        }
        Err(PyTypeError::new_err("TransformDir: expected Vec3d or Vec3f"))
    }

    /// Decompose(axis0, axis1, axis2) -> Vec3d of angle components
    #[pyo3(name = "Decompose")]
    fn decompose(&self, axis0: &super::vec::PyVec3d, axis1: &super::vec::PyVec3d, axis2: &super::vec::PyVec3d) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.decompose(&axis0.0, &axis1.0, &axis2.0))
    }

    /// RotateOntoProjected(v1, v2, axis) -> Rotation
    #[staticmethod]
    #[pyo3(name = "RotateOntoProjected")]
    fn rotate_onto_projected(v1: &super::vec::PyVec3d, v2: &super::vec::PyVec3d, axis: &super::vec::PyVec3d) -> Self {
        Self(Rotation::rotate_onto_projected(&v1.0, &v2.0, &axis.0))
    }

    /// MultiRotate(start, end, t) -> Rotation (slerp)
    #[staticmethod]
    #[pyo3(name = "MultiRotate")]
    fn multi_rotate(start: &Self, end: &Self, t: f64) -> Self {
        Self(Rotation::multi_rotate(&start.0, &end.0, t))
    }

    /// DecomposeRotation(matrix, twAxis, fbAxis, lrAxis, handedness,
    ///     thetaTwHint, thetaFBHint, thetaLRHint, thetaSwHint, useHint, swShift)
    /// Returns (tw, fb, lr, sw) angles in radians.
    /// DecomposeRotation — pass None for a hint to omit that angle channel
    #[staticmethod]
    #[pyo3(name = "DecomposeRotation")]
    #[pyo3(signature = (rot, twAxis=None, fbAxis=None, lrAxis=None, handedness=1.0,
        thetaTwHint=None, thetaFBHint=None, thetaLRHint=None, thetaSwHint=None,
        useHint=false, swShift=None))]
    fn decompose_rotation_static(
        rot: &super::matrix::PyMatrix4d,
        twAxis: Option<&super::vec::PyVec3d>,
        fbAxis: Option<&super::vec::PyVec3d>,
        lrAxis: Option<&super::vec::PyVec3d>,
        handedness: f64,
        thetaTwHint: Option<f64>,
        thetaFBHint: Option<f64>,
        thetaLRHint: Option<f64>,
        thetaSwHint: Option<f64>,
        useHint: bool,
        swShift: Option<f64>,
    ) -> (f64, f64, f64, f64) {
        let default_tw = usd_gf::Vec3d::new(0.0, 0.0, 1.0);
        let default_fb = usd_gf::Vec3d::new(1.0, 0.0, 0.0);
        let default_lr = usd_gf::Vec3d::new(0.0, 1.0, 0.0);
        let tw_ax = twAxis.map(|v| v.0).unwrap_or(default_tw);
        let fb_ax = fbAxis.map(|v| v.0).unwrap_or(default_fb);
        let lr_ax = lrAxis.map(|v| v.0).unwrap_or(default_lr);
        let mut tw = thetaTwHint.unwrap_or(0.0);
        let mut fb = thetaFBHint.unwrap_or(0.0);
        let mut lr = thetaLRHint.unwrap_or(0.0);
        let mut sw = thetaSwHint.unwrap_or(0.0);
        Rotation::decompose_rotation(
            &rot.0, &tw_ax, &fb_ax, &lr_ax,
            handedness,
            if thetaTwHint.is_some() { Some(&mut tw) } else { None },
            if thetaFBHint.is_some() { Some(&mut fb) } else { None },
            if thetaLRHint.is_some() { Some(&mut lr) } else { None },
            if thetaSwHint.is_some() { Some(&mut sw) } else { None },
            useHint,
            swShift,
        );
        (tw, fb, lr, sw)
    }

    /// DecomposeRotation3 — 3-channel variant (no swing), returns (tw, fb, lr)
    #[staticmethod]
    #[pyo3(name = "DecomposeRotation3")]
    #[pyo3(signature = (rot, twAxis=None, fbAxis=None, lrAxis=None, handedness=1.0,
        thetaTwHint=None, thetaFBHint=None, thetaLRHint=None,
        useHint=false, swShift=None))]
    fn decompose_rotation3_static(
        rot: &super::matrix::PyMatrix4d,
        twAxis: Option<&super::vec::PyVec3d>,
        fbAxis: Option<&super::vec::PyVec3d>,
        lrAxis: Option<&super::vec::PyVec3d>,
        handedness: f64,
        thetaTwHint: Option<f64>,
        thetaFBHint: Option<f64>,
        thetaLRHint: Option<f64>,
        useHint: bool,
        swShift: Option<f64>,
    ) -> (f64, f64, f64) {
        let default_tw = usd_gf::Vec3d::new(0.0, 0.0, 1.0);
        let default_fb = usd_gf::Vec3d::new(1.0, 0.0, 0.0);
        let default_lr = usd_gf::Vec3d::new(0.0, 1.0, 0.0);
        let tw_ax = twAxis.map(|v| v.0).unwrap_or(default_tw);
        let fb_ax = fbAxis.map(|v| v.0).unwrap_or(default_fb);
        let lr_ax = lrAxis.map(|v| v.0).unwrap_or(default_lr);
        let mut tw = thetaTwHint.unwrap_or(0.0);
        let mut fb = thetaFBHint.unwrap_or(0.0);
        let mut lr = thetaLRHint.unwrap_or(0.0);
        Rotation::decompose_rotation(
            &rot.0, &tw_ax, &fb_ax, &lr_ax,
            handedness,
            Some(&mut tw), Some(&mut fb), Some(&mut lr), None,
            useHint, swShift,
        );
        (tw, fb, lr)
    }

    /// MatchClosestEulerRotation(hint_tw, hint_fb, hint_lr, hint_sw, val_tw, val_fb, val_lr, val_sw)
    /// -> (tw, fb, lr, sw). Pass None for any val to leave that channel unchanged.
    #[staticmethod]
    #[pyo3(name = "MatchClosestEulerRotation")]
    fn match_closest_euler_rotation_static(
        target_tw: f64, target_fb: f64, target_lr: f64, target_sw: f64,
        theta_tw: Option<f64>, theta_fb: Option<f64>,
        theta_lr: Option<f64>, theta_sw: Option<f64>,
    ) -> (f64, f64, f64, f64) {
        let mut tw = theta_tw.unwrap_or(0.0);
        let mut fb = theta_fb.unwrap_or(0.0);
        let mut lr = theta_lr.unwrap_or(0.0);
        let mut sw = theta_sw.unwrap_or(0.0);
        Rotation::match_closest_euler_rotation(
            target_tw, target_fb, target_lr, target_sw,
            if theta_tw.is_some() { Some(&mut tw) } else { None },
            if theta_fb.is_some() { Some(&mut fb) } else { None },
            if theta_lr.is_some() { Some(&mut lr) } else { None },
            if theta_sw.is_some() { Some(&mut sw) } else { None },
        );
        (tw, fb, lr, sw)
    }

    #[getter] fn axis(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.axis()) }
    #[getter] fn angle(&self) -> f64 { self.0.angle() }
    #[setter(axis)] fn set_axis(&mut self, v: &super::vec::PyVec3d) {
        self.0.set_axis_angle(v.0, self.0.angle());
    }
    #[setter(angle)] fn set_angle(&mut self, a: f64) {
        self.0.set_axis_angle(self.0.axis(), a);
    }
}

// ---------------------------------------------------------------------------
// Range1d / Range1f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Range1d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange1d(pub Range1d);

#[pymethods]
impl PyRange1d {
    /// Range1d(), Range1d(Range1d), Range1d(min, max)
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&Bound<'_, pyo3::PyAny>>, max: Option<f64>) -> PyResult<Self> {
        let Some(mn_obj) = min else { return Ok(Self(Range1d::new(0.0, 0.0))); };
        // Copy constructor: Range1d(Range1d)
        if let Ok(r) = mn_obj.extract::<PyRef<'_, PyRange1d>>() { return Ok(Self(r.0.clone())); }
        let mn: f64 = mn_obj.extract()?;
        let mx = max.unwrap_or(0.0);
        Ok(Self(Range1d::new(mn, mx)))
    }

    fn __repr__(&self) -> String { format!("Gf.Range1d({}, {})", self.0.min(), self.0.max()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash_f64_n(&[self.0.min(), self.0.max()]) }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    /// scalar * Range1d and Range1d * scalar
    fn __mul__(&self, s: f64) -> Self { Self(Range1d::new(self.0.min() * s, self.0.max() * s)) }
    fn __rmul__(&self, s: f64) -> Self { Self(Range1d::new(self.0.min() * s, self.0.max() * s)) }

    #[pyo3(name = "Contains")] fn contains(&self, v: f64) -> bool { self.0.contains(v) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> f64 { self.0.min() }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> f64 { self.0.max() }
    #[pyo3(name = "SetMin")]   fn set_min(&mut self, v: f64) { self.0.set_min(v); }
    #[pyo3(name = "SetMax")]   fn set_max(&mut self, v: f64) { self.0.set_max(v); }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> f64 { self.0.size() }

    #[staticmethod]
    #[pyo3(name = "GetFullInterval")]
    fn get_full_interval() -> Self {
        Self(Range1d::new(f64::NEG_INFINITY, f64::INFINITY))
    }

    #[getter] fn min(&self) -> f64 { self.0.min() }
    #[getter] fn max(&self) -> f64 { self.0.max() }
    #[getter] fn size(&self) -> f64 { self.0.size() }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }  // pxr camelCase
    #[setter] fn set_min_prop(&mut self, v: f64) { self.0.set_min(v); }
    #[setter] fn set_max_prop(&mut self, v: f64) { self.0.set_max(v); }
}

#[pyclass(skip_from_py_object,name = "Range1f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange1f(pub Range1f);

#[pymethods]
impl PyRange1f {
    #[new]
    #[pyo3(signature = (min=0.0, max=0.0))]
    fn new(min: f32, max: f32) -> Self { Self(Range1f::new(min, max)) }

    fn __repr__(&self) -> String { format!("Gf.Range1f({}, {})", self.0.min(), self.0.max()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, v: f32) -> bool { self.0.contains(v) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> f32 { self.0.min() }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> f32 { self.0.max() }
    #[pyo3(name = "SetMin")]   fn set_min(&mut self, v: f32) { self.0.set_min(v); }
    #[pyo3(name = "SetMax")]   fn set_max(&mut self, v: f32) { self.0.set_max(v); }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> f32 { self.0.size() }

    #[getter] fn min(&self) -> f32 { self.0.min() }
    #[getter] fn max(&self) -> f32 { self.0.max() }
    #[getter] fn size(&self) -> f32 { self.0.size() }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

// ---------------------------------------------------------------------------
// Range2d / Range2f  — min()/max() return &Vec2<T>, copy with *
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Range2d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange2d(pub Range2d);

#[pymethods]
impl PyRange2d {
    /// Range2d(), Range2d(Range2d), Range2d(Vec2d, Vec2d)
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&Bound<'_, pyo3::PyAny>>, max: Option<&super::vec::PyVec2d>) -> PyResult<Self> {
        let Some(mn_obj) = min else { return Ok(Self(Range2d::empty())); };
        if let Ok(r) = mn_obj.extract::<PyRef<'_, PyRange2d>>() { return Ok(Self(r.0.clone())); }
        if let Ok(mn) = mn_obj.extract::<PyRef<'_, super::vec::PyVec2d>>() {
            if let Some(mx) = max { return Ok(Self(Range2d::new(mn.0, mx.0))); }
        }
        Ok(Self(Range2d::empty()))
    }

    fn __repr__(&self) -> String {
        let mn = *self.0.min(); let mx = *self.0.max();
        format!("Gf.Range2d(({},{}), ({},{}))", mn.x, mn.y, mx.x, mx.y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let mn = *self.0.min(); let mx = *self.0.max();
        hash_f64_n(&[mn.x, mn.y, mx.x, mx.y])
    }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec2d) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(*self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(*self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(*self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(*self.0.max()) }
    #[getter] fn size(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.size()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

#[pyclass(skip_from_py_object,name = "Range2f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange2f(pub Range2f);

#[pymethods]
impl PyRange2f {
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&super::vec::PyVec2f>, max: Option<&super::vec::PyVec2f>) -> Self {
        match (min, max) {
            (Some(mn), Some(mx)) => Self(Range2f::new(mn.0, mx.0)),
            _ => Self(Range2f::empty()),
        }
    }

    fn __repr__(&self) -> String {
        let mn = *self.0.min(); let mx = *self.0.max();
        format!("Gf.Range2f(({},{}), ({},{}))", mn.x, mn.y, mx.x, mx.y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec2f) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "IsEmpty")] fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]  fn get_min(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(*self.0.min()) }
    #[pyo3(name = "GetMax")]  fn get_max(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(*self.0.max()) }
    #[pyo3(name = "GetSize")] fn get_size(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(*self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(*self.0.max()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

// ---------------------------------------------------------------------------
// Range3d / Range3f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Range3d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange3d(pub Range3d);

#[pymethods]
impl PyRange3d {
    /// Range3d(), Range3d(Vec3d, Vec3d), Range3d((x,y,z), (x,y,z))
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&Bound<'_, pyo3::PyAny>>, max: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let Some(mn_obj) = min else { return Ok(Self(Range3d::empty())); };
        let Some(mx_obj) = max else { return Ok(Self(Range3d::empty())); };
        let mn = extract_vec3d(mn_obj)?;
        let mx = extract_vec3d(mx_obj)?;
        Ok(Self(Range3d::new(mn, mx)))
    }

    fn __repr__(&self) -> String {
        let mn = *self.0.min(); let mx = *self.0.max();
        format!("Gf.Range3d(({},{},{}), ({},{},{}))", mn.x,mn.y,mn.z, mx.x,mx.y,mx.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let mn = *self.0.min(); let mx = *self.0.max();
        hash_f64_n(&[mn.x, mn.y, mn.z, mx.x, mx.y, mx.z])
    }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec3d) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "Intersects")] fn intersects(&self, o: &Self) -> bool { !self.0.is_outside(&o.0) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.size()) }
    #[pyo3(name = "GetMidpoint")] fn get_midpoint(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.midpoint())
    }

    #[getter] fn min(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.max()) }
    #[getter] fn size(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.size()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
    #[setter] fn set_min_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_min(v.0); }
    #[setter] fn set_max_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_max(v.0); }
}

#[pyclass(skip_from_py_object,name = "Range3f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRange3f(pub Range3f);

#[pymethods]
impl PyRange3f {
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&super::vec::PyVec3f>, max: Option<&super::vec::PyVec3f>) -> Self {
        match (min, max) {
            (Some(mn), Some(mx)) => Self(Range3f::new(mn.0, mx.0)),
            _ => Self(Range3f::empty()),
        }
    }

    fn __repr__(&self) -> String {
        let mn = *self.0.min(); let mx = *self.0.max();
        format!("Gf.Range3f(({},{},{}), ({},{},{}))", mn.x,mn.y,mn.z, mx.x,mx.y,mx.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec3f) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.max()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

// ---------------------------------------------------------------------------
// BBox3d — range()/matrix()/inverse_matrix() return references, dereference
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "BBox3d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyBBox3d(pub BBox3d);

#[pymethods]
impl PyBBox3d {
    /// BBox3d(), BBox3d(BBox3d), BBox3d(Range3d), BBox3d(Range3d, Matrix4d)
    #[new]
    #[pyo3(signature = (range=None, matrix=None))]
    fn new(range: Option<&Bound<'_, pyo3::PyAny>>, matrix: Option<&super::matrix::PyMatrix4d>) -> PyResult<Self> {
        let Some(r_obj) = range else { return Ok(Self(BBox3d::default())); };
        // Copy constructor: BBox3d(BBox3d)
        if let Ok(bb) = r_obj.extract::<PyRef<'_, PyBBox3d>>() {
            return Ok(Self(bb.0.clone()));
        }
        // BBox3d(Range3d, optional Matrix4d)
        if let Ok(r) = r_obj.extract::<PyRef<'_, PyRange3d>>() {
            if let Some(m) = matrix {
                return Ok(Self(BBox3d::from_range_matrix(r.0.clone(), m.0)));
            }
            return Ok(Self(BBox3d::from_range(r.0.clone())));
        }
        Err(PyTypeError::new_err("BBox3d: expected Range3d or BBox3d"))
    }

    fn __repr__(&self) -> String { "Gf.BBox3d(...)".to_string() }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let r = self.0.range();
        let mn = *r.min(); let mx = *r.max();
        hash_f64_n(&[mn.x, mn.y, mn.z, mx.x, mx.y, mx.z])
    }

    /// Set(range, matrix) -> BBox3d (matches pxr.Gf.BBox3d.Set)
    #[pyo3(name = "Set")] fn set(&mut self, r: &PyRange3d, m: &super::matrix::PyMatrix4d) -> Self {
        self.0.set_range(r.0.clone());
        self.0.set_matrix(m.0);
        self.clone()
    }

    #[pyo3(name = "GetRange")]    fn get_range(&self) -> PyRange3d { PyRange3d(self.0.range().clone()) }
    #[pyo3(name = "GetBox")]      fn get_box(&self) -> PyRange3d { PyRange3d(self.0.range().clone()) }
    #[pyo3(name = "GetMatrix")]   fn get_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(*self.0.matrix())
    }
    #[pyo3(name = "GetInverseMatrix")] fn get_inverse_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(*self.0.inverse_matrix())
    }
    #[pyo3(name = "SetRange")] fn set_range(&mut self, r: &PyRange3d) { self.0.set_range(r.0.clone()); }
    #[pyo3(name = "SetMatrix")] fn set_matrix(&mut self, m: &super::matrix::PyMatrix4d) { self.0.set_matrix(m.0); }
    #[pyo3(name = "HasZeroAreaPrimitives")] fn has_zero_area_primitives(&self) -> bool {
        self.0.has_zero_area_primitives()
    }
    #[pyo3(name = "SetHasZeroAreaPrimitives")] fn set_has_zero_area_primitives(&mut self, v: bool) {
        self.0.set_has_zero_area_primitives(v);
    }
    #[pyo3(name = "ComputeAlignedRange")] fn compute_aligned_range(&self) -> PyRange3d {
        PyRange3d(self.0.compute_aligned_range())
    }
    #[pyo3(name = "ComputeAlignedBox")] fn compute_aligned_box(&self) -> PyRange3d {
        PyRange3d(self.0.compute_aligned_range())
    }
    #[pyo3(name = "ComputeCentroid")] fn compute_centroid(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.compute_centroid())
    }
    #[pyo3(name = "GetVolume")] fn get_volume(&self) -> f64 { self.0.volume() }
    #[pyo3(name = "Transform")] fn transform(&self, m: &super::matrix::PyMatrix4d) -> Self {
        let mut b = self.0.clone();
        b.transform(&m.0);
        Self(b)
    }
    #[staticmethod]
    #[pyo3(name = "Combine")]
    fn combine(a: &Self, b: &Self) -> Self { Self(BBox3d::combine(&a.0, &b.0)) }

    #[getter(r#box)]
    fn get_box_prop(&self) -> PyRange3d { PyRange3d(self.0.range().clone()) }
    #[getter] fn matrix(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(*self.0.matrix()) }
    #[getter] fn hasZeroAreaPrimitives(&self) -> bool { self.0.has_zero_area_primitives() }
    #[setter(r#box)] fn set_box(&mut self, r: &PyRange3d) { self.0.set_range(r.0.clone()); }
    #[setter(matrix)] fn set_matrix_prop(&mut self, m: &super::matrix::PyMatrix4d) { self.0.set_matrix(m.0); }
    #[setter(hasZeroAreaPrimitives)] fn set_has_zero_area_primitives_prop(&mut self, v: bool) { self.0.set_has_zero_area_primitives(v); }
}

// ---------------------------------------------------------------------------
// Interval — is_min_closed()/is_max_closed() (not min_closed()/max_closed())
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Interval", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyInterval(pub Interval);

#[pymethods]
impl PyInterval {
    /// Interval(), Interval(Interval), Interval(min, max),
    /// Interval(min, max, min_closed, max_closed)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Interval::new(0.0, 0.0, true, true))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(iv) = obj.extract::<PyRef<'_, PyInterval>>() { return Ok(Self(iv.0.clone())); }
            if let Ok(v) = obj.extract::<f64>() { return Ok(Self(Interval::new(v, v, true, true))); }
        }
        if n == 2 {
            let min: f64 = args.get_item(0)?.extract()?;
            let max: f64 = args.get_item(1)?.extract()?;
            return Ok(Self(Interval::new(min, max, true, true)));
        }
        if n == 4 {
            let min: f64 = args.get_item(0)?.extract()?;
            let max: f64 = args.get_item(1)?.extract()?;
            let min_closed: bool = args.get_item(2)?.extract()?;
            let max_closed: bool = args.get_item(3)?.extract()?;
            return Ok(Self(Interval::new(min, max, min_closed, max_closed)));
        }
        Err(PyTypeError::new_err("Interval: unsupported constructor"))
    }

    fn __repr__(&self) -> String { format!("{}", self.0) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f64_n(&[self.0.get_min(), self.0.get_max()])
    }
    fn __bool__(&self) -> bool { !self.0.is_empty() }
    fn __and__(&self, o: &Self) -> Self { Self(self.0.clone() & o.0.clone()) }

    #[pyo3(name = "Contains")] fn contains(&self, v: f64) -> bool { self.0.contains(v) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "IsFinite")] fn is_finite(&self) -> bool { self.0.is_finite() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> f64 { self.0.get_min() }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> f64 { self.0.get_max() }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> f64 { self.0.size() }
    #[pyo3(name = "Intersects")] fn intersects(&self, o: &Self) -> bool { self.0.intersects(&o.0) }

    #[staticmethod]
    #[pyo3(name = "GetFullInterval")]
    fn get_full_interval() -> Self { Self(Interval::full()) }

    #[getter] fn min(&self) -> f64 { self.0.get_min() }
    #[getter] fn max(&self) -> f64 { self.0.get_max() }
    // pxr uses camelCase property names
    #[getter] fn minClosed(&self) -> bool { self.0.is_min_closed() }
    #[getter] fn maxClosed(&self) -> bool { self.0.is_max_closed() }
    #[getter] fn minOpen(&self) -> bool { self.0.is_min_open() }
    #[getter] fn maxOpen(&self) -> bool { self.0.is_max_open() }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
    #[getter] fn size(&self) -> f64 { self.0.size() }
    #[getter] fn finite(&self) -> bool { self.0.is_finite() }
}

// ---------------------------------------------------------------------------
// MultiInterval — no & or | operators; use add/remove/intersect methods
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "MultiInterval", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMultiInterval(pub MultiInterval);

#[pymethods]
impl PyMultiInterval {
    /// MultiInterval(), MultiInterval(MultiInterval), MultiInterval(Interval)
    #[new]
    #[pyo3(signature = (arg=None))]
    fn new(arg: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let Some(obj) = arg else { return Ok(Self(MultiInterval::new())); };
        if let Ok(mi) = obj.extract::<PyRef<'_, PyMultiInterval>>() { return Ok(Self(mi.0.clone())); }
        if let Ok(iv) = obj.extract::<PyRef<'_, PyInterval>>() {
            let mut mi = MultiInterval::new();
            mi.add(iv.0.clone());
            return Ok(Self(mi));
        }
        Err(PyTypeError::new_err("MultiInterval: expected (), (MultiInterval), or (Interval)"))
    }

    fn __repr__(&self) -> String { "Gf.MultiInterval(...)".to_string() }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    // Union via add_multi
    fn __or__(&self, o: &Self) -> Self {
        let mut result = self.0.clone();
        result.add_multi(&o.0);
        Self(result)
    }
    // Intersection via intersect_multi
    fn __and__(&self, o: &Self) -> Self {
        let mut result = self.0.clone();
        result.intersect_multi(&o.0);
        Self(result)
    }

    #[pyo3(name = "IsEmpty")]     fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "Contains")]    fn contains(&self, v: f64) -> bool { self.0.contains_value(v) }
    #[pyo3(name = "GetSize")]     fn get_size(&self) -> usize { self.0.len() }
    #[pyo3(name = "Add")]         fn add(&mut self, i: &PyInterval) { self.0.add(i.0.clone()); }
    #[pyo3(name = "Remove")]      fn remove(&mut self, i: &PyInterval) { self.0.remove(i.0.clone()); }
    #[pyo3(name = "Intersect")]   fn intersect(&mut self, i: &PyInterval) { self.0.intersect(i.0.clone()); }
    #[pyo3(name = "Clear")]       fn clear(&mut self) { self.0.clear(); }
    #[pyo3(name = "Union")]       fn union_with(&mut self, o: &Self) { self.0.add_multi(&o.0); }
    #[pyo3(name = "Intersection")] fn intersection(&mut self, o: &Self) { self.0.intersect_multi(&o.0); }
    #[pyo3(name = "GetBounds")]   fn get_bounds(&self) -> PyInterval { PyInterval(self.0.bounds()) }
    #[pyo3(name = "Complement")]  fn complement(&self) -> Self { Self(self.0.complement()) }

    #[staticmethod]
    #[pyo3(name = "GetFullInterval")]
    fn get_full_interval() -> PyInterval { PyInterval(Interval::full()) }
}

// ---------------------------------------------------------------------------
// Rect2i — area() returns u64, cast to i64 for Python int compat
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Rect2i", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRect2i(pub Rect2i);

#[pymethods]
impl PyRect2i {
    /// Rect2i(), Rect2i(Rect2i), Rect2i(Vec2i, Vec2i), Rect2i(Vec2i, width, height)
    #[new]
    #[pyo3(signature = (min=None, max_or_width=None, height=None))]
    fn new(min: Option<&Bound<'_, pyo3::PyAny>>, max_or_width: Option<&Bound<'_, pyo3::PyAny>>, height: Option<i32>) -> PyResult<Self> {
        let Some(mn_obj) = min else { return Ok(Self(Rect2i::default())); };
        // Copy constructor: Rect2i(Rect2i)
        if let Ok(r) = mn_obj.extract::<PyRef<'_, PyRect2i>>() {
            return Ok(Self(r.0.clone()));
        }
        // Extract min as Vec2i
        let mn = if let Ok(v) = mn_obj.extract::<PyRef<'_, super::vec::PyVec2i>>() {
            v.0
        } else if let Ok((x, y)) = mn_obj.extract::<(i32, i32)>() {
            usd_gf::Vec2i::new(x, y)
        } else {
            return Err(PyTypeError::new_err("Rect2i: expected Vec2i or (int,int) for min"));
        };
        let Some(mx_obj) = max_or_width else {
            return Ok(Self(Rect2i::new(mn, mn)));
        };
        // Rect2i(Vec2i, width, height) form
        if let Some(h) = height {
            let w: i32 = mx_obj.extract()?;
            let mx = usd_gf::Vec2i::new(mn.x + w - 1, mn.y + h - 1);
            return Ok(Self(Rect2i::new(mn, mx)));
        }
        // Rect2i(Vec2i, Vec2i) form
        let mx = if let Ok(v) = mx_obj.extract::<PyRef<'_, super::vec::PyVec2i>>() {
            v.0
        } else if let Ok((x, y)) = mx_obj.extract::<(i32, i32)>() {
            usd_gf::Vec2i::new(x, y)
        } else {
            return Err(PyTypeError::new_err("Rect2i: expected Vec2i or (int,int) for max"));
        };
        Ok(Self(Rect2i::new(mn, mx)))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Rect2i(Gf.Vec2i({}, {}), Gf.Vec2i({}, {}))",
            self.0.min().x, self.0.min().y, self.0.max().x, self.0.max().y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_i32_n(&[self.0.min().x, self.0.min().y, self.0.max().x, self.0.max().y])
    }

    #[pyo3(name = "IsNull")]   fn is_null(&self) -> bool { self.0.is_null() }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "IsValid")]  fn is_valid(&self) -> bool { self.0.is_valid() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        Self(self.0.normalized())
    }
    #[pyo3(name = "GetCenter")] fn get_center(&self) -> super::vec::PyVec2i {
        super::vec::PyVec2i(self.0.center())
    }
    #[pyo3(name = "GetMin")]    fn get_min(&self) -> super::vec::PyVec2i { super::vec::PyVec2i(*self.0.min()) }
    #[pyo3(name = "GetMax")]    fn get_max(&self) -> super::vec::PyVec2i { super::vec::PyVec2i(*self.0.max()) }
    #[pyo3(name = "GetWidth")]  fn get_width(&self) -> i32 { self.0.width() }
    #[pyo3(name = "GetHeight")] fn get_height(&self) -> i32 { self.0.height() }
    #[pyo3(name = "GetSize")]   fn get_size(&self) -> super::vec::PyVec2i { super::vec::PyVec2i(self.0.size()) }
    #[pyo3(name = "GetArea")]   fn get_area(&self) -> u64 { self.0.area() }
    #[pyo3(name = "Contains")]  fn contains(&self, p: &super::vec::PyVec2i) -> bool {
        self.0.contains(&p.0)
    }
    #[pyo3(name = "SetMin")]    fn set_min(&mut self, p: &super::vec::PyVec2i) {
        self.0.set_min(p.0);
    }
    #[pyo3(name = "SetMax")]    fn set_max(&mut self, p: &super::vec::PyVec2i) {
        self.0.set_max(p.0);
    }
    #[pyo3(name = "Translate")] fn translate(&mut self, displacement: &super::vec::PyVec2i) {
        self.0.translate(&displacement.0);
    }
    #[pyo3(name = "GetIntersection")] fn get_intersection(&self, other: &Self) -> Self {
        Self(self.0.get_intersection(&other.0))
    }
    #[pyo3(name = "GetUnion")] fn get_union(&self, other: &Self) -> Self {
        Self(self.0.get_union(&other.0))
    }
    fn __iadd__(&mut self, other: &Self) {
        self.0 = self.0.get_union(&other.0);
    }

    #[getter] fn min(&self) -> super::vec::PyVec2i { super::vec::PyVec2i(*self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec2i { super::vec::PyVec2i(*self.0.max()) }
    #[getter] fn minX(&self) -> i32 { self.0.min().x }
    #[getter] fn maxX(&self) -> i32 { self.0.max().x }
    #[getter] fn minY(&self) -> i32 { self.0.min().y }
    #[getter] fn maxY(&self) -> i32 { self.0.max().y }
    #[getter] fn width(&self) -> i32 { self.0.width() }
    #[getter] fn height(&self) -> i32 { self.0.height() }
    #[getter] fn area(&self) -> u64 { self.0.area() }
    #[setter(min)] fn set_min_val(&mut self, p: &super::vec::PyVec2i) { self.0.set_min(p.0); }
    #[setter(max)] fn set_max_val(&mut self, p: &super::vec::PyVec2i) { self.0.set_max(p.0); }
    #[setter(minX)] fn set_minX_val(&mut self, v: i32) { self.0.set_min(usd_gf::Vec2i::new(v, self.0.min().y)); }
    #[setter(maxX)] fn set_maxX_val(&mut self, v: i32) { self.0.set_max(usd_gf::Vec2i::new(v, self.0.max().y)); }
    #[setter(minY)] fn set_minY_val(&mut self, v: i32) { self.0.set_min(usd_gf::Vec2i::new(self.0.min().x, v)); }
    #[setter(maxY)] fn set_maxY_val(&mut self, v: i32) { self.0.set_max(usd_gf::Vec2i::new(self.0.max().x, v)); }
}

// ---------------------------------------------------------------------------
// Size2 / Size3
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Size2", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PySize2(pub Size2);

#[pymethods]
impl PySize2 {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: usize = 2;

    /// Size2(), Size2(x, y), Size2(Size2), Size2(Vec2i)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Size2::new(0, 0))); }
        if n == 2 {
            let x: usize = args.get_item(0)?.extract()?;
            let y: usize = args.get_item(1)?.extract()?;
            return Ok(Self(Size2::new(x, y)));
        }
        if n == 1 {
            let a = args.get_item(0)?;
            if let Ok(s) = a.extract::<PyRef<'_, PySize2>>() {
                return Ok(Self(s.0));
            }
            if let Ok(v) = a.extract::<PyRef<'_, super::vec::PyVec2i>>() {
                return Ok(Self(Size2::new(v.0.x as usize, v.0.y as usize)));
            }
        }
        Err(PyTypeError::new_err("Size2: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String { format!("Gf.Size2({}, {})", self.0[0], self.0[1]) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __getitem__(&self, i: isize) -> PyResult<usize> {
        let j = if i < 0 { 2isize + i } else { i };
        if j < 0 || j >= 2 { return Err(PyIndexError::new_err("index out of range")); }
        Ok(self.0[j as usize])
    }
    fn __setitem__(&mut self, i: isize, v: usize) -> PyResult<()> {
        let j = if i < 0 { 2isize + i } else { i };
        if j < 0 || j >= 2 { return Err(PyIndexError::new_err("index out of range")); }
        self.0[j as usize] = v; Ok(())
    }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(s) = o.extract::<usize>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(other) = o.extract::<PyRef<'_, PySize2>>() {
            return Ok(Self(Size2::new(self.0[0] * other.0[0], self.0[1] * other.0[1])).into_pyobject(py)?.into_any().unbind());
        }
        Err(PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: usize) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, s: usize) { self.0 = self.0 * s; }
    fn __truediv__(&self, s: usize) -> PyResult<Self> {
        if s == 0 { return Err(PyTypeError::new_err("division by zero")); }
        Ok(Self(Size2::new(self.0[0] / s, self.0[1] / s)))
    }
    fn __itruediv__(&mut self, s: usize) -> PyResult<()> {
        if s == 0 { return Err(PyTypeError::new_err("division by zero")); }
        self.0[0] /= s; self.0[1] /= s; Ok(())
    }
    fn __contains__(&self, v: usize) -> bool { self.0[0] == v || self.0[1] == v }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let vals = vec![slf.0[0], slf.0[1]];
        pyo3::types::PyList::new(slf.py(), vals).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    #[pyo3(name = "Get")] fn get(&self) -> (usize, usize) { (self.0[0], self.0[1]) }
    #[pyo3(name = "Set")] fn set(&mut self, x: usize, y: usize) -> Self {
        self.0[0] = x; self.0[1] = y;
        self.clone()
    }
}

#[pyclass(skip_from_py_object,name = "Size3", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PySize3(pub Size3);

#[pymethods]
impl PySize3 {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: usize = 3;

    /// Size3(), Size3(x, y, z), Size3(Size3), Size3(Vec3i)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Size3::new(0, 0, 0))); }
        if n == 3 {
            let x: usize = args.get_item(0)?.extract()?;
            let y: usize = args.get_item(1)?.extract()?;
            let z: usize = args.get_item(2)?.extract()?;
            return Ok(Self(Size3::new(x, y, z)));
        }
        if n == 1 {
            let a = args.get_item(0)?;
            if let Ok(s) = a.extract::<PyRef<'_, PySize3>>() {
                return Ok(Self(s.0));
            }
            if let Ok(v) = a.extract::<PyRef<'_, super::vec::PyVec3i>>() {
                return Ok(Self(Size3::new(v.0.x as usize, v.0.y as usize, v.0.z as usize)));
            }
        }
        Err(PyTypeError::new_err("Size3: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String { format!("Gf.Size3({}, {}, {})", self.0[0], self.0[1], self.0[2]) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __getitem__(&self, i: isize) -> PyResult<usize> {
        let j = if i < 0 { 3isize + i } else { i };
        if j < 0 || j >= 3 { return Err(PyIndexError::new_err("index out of range")); }
        Ok(self.0[j as usize])
    }
    fn __setitem__(&mut self, i: isize, v: usize) -> PyResult<()> {
        let j = if i < 0 { 3isize + i } else { i };
        if j < 0 || j >= 3 { return Err(PyIndexError::new_err("index out of range")); }
        self.0[j as usize] = v; Ok(())
    }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(s) = o.extract::<usize>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(other) = o.extract::<PyRef<'_, PySize3>>() {
            return Ok(Self(Size3::new(self.0[0]*other.0[0], self.0[1]*other.0[1], self.0[2]*other.0[2])).into_pyobject(py)?.into_any().unbind());
        }
        Err(PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: usize) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, s: usize) { self.0 = self.0 * s; }
    fn __truediv__(&self, s: usize) -> PyResult<Self> {
        if s == 0 { return Err(PyTypeError::new_err("division by zero")); }
        Ok(Self(Size3::new(self.0[0] / s, self.0[1] / s, self.0[2] / s)))
    }
    fn __itruediv__(&mut self, s: usize) -> PyResult<()> {
        if s == 0 { return Err(PyTypeError::new_err("division by zero")); }
        self.0[0] /= s; self.0[1] /= s; self.0[2] /= s; Ok(())
    }
    fn __contains__(&self, v: usize) -> bool { self.0[0] == v || self.0[1] == v || self.0[2] == v }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let vals = vec![slf.0[0], slf.0[1], slf.0[2]];
        pyo3::types::PyList::new(slf.py(), vals).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    #[pyo3(name = "Get")] fn get(&self) -> (usize, usize, usize) { (self.0[0], self.0[1], self.0[2]) }
    #[pyo3(name = "Set")] fn set(&mut self, x: usize, y: usize, z: usize) -> Self {
        self.0[0] = x; self.0[1] = y; self.0[2] = z;
        self.clone()
    }
}

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Transform", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyTransform(pub usd_gf::Transform);

#[pymethods]
impl PyTransform {
    /// Transform(), Transform(Matrix4d), Transform(Vec3d, Rotation, Vec3d, Vec3d, Rotation)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(usd_gf::Transform::default())); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(m) = obj.extract::<PyRef<'_, super::matrix::PyMatrix4d>>() {
                return Ok(Self(usd_gf::Transform::from_matrix(&m.0)));
            }
        }
        if n == 5 {
            let tr = args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>()?.0;
            let rot = args.get_item(1)?.extract::<PyRef<'_, PyRotation>>()?.0;
            let scale = args.get_item(2)?.extract::<PyRef<'_, super::vec::PyVec3d>>()?.0;
            let pivot_pos = args.get_item(3)?.extract::<PyRef<'_, super::vec::PyVec3d>>()?.0;
            let pivot_orient = args.get_item(4)?.extract::<PyRef<'_, PyRotation>>()?.0;
            return Ok(Self(usd_gf::Transform::from_components(tr, rot, scale, pivot_pos, pivot_orient)));
        }
        Err(PyTypeError::new_err("Transform: expected (), (Matrix4d), or (Vec3d, Rotation, Vec3d, Vec3d, Rotation)"))
    }

    fn __repr__(&self) -> String {
        let t = self.0.translation();
        let r = self.0.rotation();
        let s = self.0.scale();
        let pp = self.0.pivot_position();
        let po = self.0.pivot_orientation();
        let r_ax = r.axis();
        let po_ax = po.axis();
        format!(
            "Gf.Transform(Gf.Vec3d({}, {}, {}), Gf.Rotation(Gf.Vec3d({}, {}, {}), {}), Gf.Vec3d({}, {}, {}), Gf.Vec3d({}, {}, {}), Gf.Rotation(Gf.Vec3d({}, {}, {}), {}))",
            t.x, t.y, t.z,
            r_ax.x, r_ax.y, r_ax.z, r.angle(),
            s.x, s.y, s.z,
            pp.x, pp.y, pp.z,
            po_ax.x, po_ax.y, po_ax.z, po.angle()
        )
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let t = self.0.translation();
        hash_f64_n(&[t.x, t.y, t.z, self.0.scale().x, self.0.scale().y, self.0.scale().z])
    }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __imul__(&mut self, o: &Self) {
        self.0 = self.0 * o.0;
    }

    /// Set(translation, rotation, scale, pivotPosition, pivotOrientation) -> Transform
    #[pyo3(name = "Set")]
    fn set(&mut self, t: &super::vec::PyVec3d, rot: &PyRotation, scale: &super::vec::PyVec3d,
           pivot_pos: &super::vec::PyVec3d, pivot_orient: &PyRotation) -> Self {
        self.0.set(t.0, rot.0, scale.0, pivot_pos.0, pivot_orient.0);
        self.clone()
    }

    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0.set_identity(); }
    #[pyo3(name = "GetMatrix")]   fn get_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(self.0.matrix())
    }
    #[pyo3(name = "SetMatrix")]   fn set_matrix(&mut self, m: &super::matrix::PyMatrix4d) -> Self {
        self.0.set_matrix(&m.0);
        self.clone()
    }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> super::matrix::PyMatrix4d {
        let inv = self.0.matrix().inverse().unwrap_or_else(usd_gf::Matrix4d::identity);
        super::matrix::PyMatrix4d(inv)
    }
    #[pyo3(name = "GetInverseMatrix")] fn get_inverse_matrix(&self) -> super::matrix::PyMatrix4d {
        let inv = self.0.matrix().inverse().unwrap_or_else(usd_gf::Matrix4d::identity);
        super::matrix::PyMatrix4d(inv)
    }
    #[pyo3(name = "GetTranslation")] fn get_translation(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.translation())
    }
    #[pyo3(name = "SetTranslation")] fn set_translation(&mut self, t: &super::vec::PyVec3d) {
        self.0.set_translation(t.0);
    }
    #[pyo3(name = "GetRotation")] fn get_rotation(&self) -> PyRotation {
        PyRotation(*self.0.rotation())
    }
    #[pyo3(name = "SetRotation")] fn set_rotation(&mut self, r: &PyRotation) {
        self.0.set_rotation(r.0);
    }
    #[pyo3(name = "GetScale")] fn get_scale(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.scale())
    }
    #[pyo3(name = "SetScale")] fn set_scale(&mut self, s: &super::vec::PyVec3d) {
        self.0.set_scale(s.0);
    }
    #[pyo3(name = "GetPivotPosition")] fn get_pivot_position(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.pivot_position())
    }
    #[pyo3(name = "SetPivotPosition")] fn set_pivot_position(&mut self, p: &super::vec::PyVec3d) {
        self.0.set_pivot_position(p.0);
    }
    #[pyo3(name = "GetPivotOrientation")] fn get_pivot_orientation(&self) -> PyRotation {
        PyRotation(*self.0.pivot_orientation())
    }
    #[pyo3(name = "SetPivotOrientation")] fn set_pivot_orientation(&mut self, r: &PyRotation) {
        self.0.set_pivot_orientation(r.0);
    }

    #[getter] fn translation(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.translation()) }
    #[setter(translation)] fn set_translation_val(&mut self, t: &super::vec::PyVec3d) { self.0.set_translation(t.0); }
    #[getter] fn rotation(&self) -> PyRotation { PyRotation(*self.0.rotation()) }
    #[setter(rotation)] fn set_rotation_val(&mut self, r: &PyRotation) { self.0.set_rotation(r.0); }
    #[getter] fn scale(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.scale()) }
    #[setter(scale)] fn set_scale_val(&mut self, s: &super::vec::PyVec3d) { self.0.set_scale(s.0); }
    #[getter] fn pivotPosition(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.pivot_position()) }
    #[setter] fn set_pivotPosition(&mut self, p: &super::vec::PyVec3d) { self.0.set_pivot_position(p.0); }
    #[getter] fn pivotOrientation(&self) -> PyRotation { PyRotation(*self.0.pivot_orientation()) }
    #[setter] fn set_pivotOrientation(&mut self, r: &PyRotation) { self.0.set_pivot_orientation(r.0); }
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Camera", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyCamera(pub usd_gf::Camera);

#[pymethods]
impl PyCamera {
    /// Camera constants (class attributes) — use f64 literals for Python precision
    #[classattr] const DEFAULT_HORIZONTAL_APERTURE: f64 = 20.955;
    #[classattr] const DEFAULT_VERTICAL_APERTURE: f64 = 15.2908;
    #[classattr] const APERTURE_UNIT: f64 = usd_gf::APERTURE_UNIT;
    #[classattr] const FOCAL_LENGTH_UNIT: f64 = usd_gf::FOCAL_LENGTH_UNIT;
    /// Projection type enum: 0 = Perspective, 1 = Orthographic
    #[allow(non_upper_case_globals)]
    #[classattr] const Perspective: i32 = 0;
    #[allow(non_upper_case_globals)]
    #[classattr] const Orthographic: i32 = 1;
    /// FOV direction constants
    #[allow(non_upper_case_globals)]
    #[classattr] const FOVHorizontal: i32 = 0;
    #[allow(non_upper_case_globals)]
    #[classattr] const FOVVertical: i32 = 1;

    /// Camera(), Camera(Camera) — copy constructor
    #[new]
    #[pyo3(signature = (other=None))]
    fn new(other: Option<&PyCamera>) -> Self {
        match other {
            Some(c) => Self(c.0.clone()),
            None => Self(usd_gf::Camera::default()),
        }
    }

    fn __repr__(&self) -> String {
        format!("Gf.Camera(Gf.Matrix4d{:?}, {}, {}, {}, {}, {}, {}, {}, Gf.Range1f({}, {}), {}, Gf.Camera.{})",
            "(... elided ...)",
            self.0.projection() as i32,
            self.0.horizontal_aperture(),
            self.0.vertical_aperture(),
            self.0.horizontal_aperture_offset(),
            self.0.vertical_aperture_offset(),
            self.0.focal_length(),
            self.0.f_stop(),
            self.0.clipping_range().min(),
            self.0.clipping_range().max(),
            self.0.focus_distance(),
            if self.0.projection() == usd_gf::CameraProjection::Perspective { "Perspective" } else { "Orthographic" },
        )
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool {
        self.0.horizontal_aperture() == o.0.horizontal_aperture()
            && self.0.vertical_aperture() == o.0.vertical_aperture()
            && self.0.horizontal_aperture_offset() == o.0.horizontal_aperture_offset()
            && self.0.vertical_aperture_offset() == o.0.vertical_aperture_offset()
            && self.0.focal_length() == o.0.focal_length()
            && self.0.f_stop() == o.0.f_stop()
            && self.0.focus_distance() == o.0.focus_distance()
            && self.0.projection() == o.0.projection()
            && *self.0.transform() == *o.0.transform()
            && self.0.clipping_range() == o.0.clipping_range()
    }
    fn __ne__(&self, o: &Self) -> bool { !self.__eq__(o) }
    fn __hash__(&self) -> u64 {
        hash_f64_n(&[self.0.horizontal_aperture() as f64, self.0.vertical_aperture() as f64, self.0.focal_length() as f64])
    }

    #[pyo3(name = "GetFieldOfView")] fn get_fov(&self, direction: i32) -> f64 {
        let dir = if direction == 0 {
            usd_gf::camera::FOVDirection::Horizontal
        } else {
            usd_gf::camera::FOVDirection::Vertical
        };
        self.0.field_of_view(dir) as f64
    }

    #[pyo3(name = "SetPerspectiveFromAspectRatioAndFieldOfView")]
    fn set_perspective_from_ar_and_fov(&mut self, aspect: f32, fov: f32, direction: i32) {
        let dir = if direction == 0 {
            usd_gf::camera::FOVDirection::Horizontal
        } else {
            usd_gf::camera::FOVDirection::Vertical
        };
        self.0.set_perspective_from_aspect_ratio_and_fov(aspect, fov, dir);
    }

    #[getter] fn transform(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(*self.0.transform())
    }
    #[setter(transform)] fn set_transform_prop(&mut self, m: &super::matrix::PyMatrix4d) {
        self.0.set_transform(m.0);
    }
    #[getter] fn horizontalAperture(&self) -> f64 { self.0.horizontal_aperture() as f64 }
    #[setter(horizontalAperture)] fn set_horizontal_aperture(&mut self, v: f64) { self.0.set_horizontal_aperture(v as f32); }
    #[getter] fn verticalAperture(&self) -> f64 { self.0.vertical_aperture() as f64 }
    #[setter(verticalAperture)] fn set_vertical_aperture(&mut self, v: f64) { self.0.set_vertical_aperture(v as f32); }
    #[getter] fn horizontalApertureOffset(&self) -> f64 { self.0.horizontal_aperture_offset() as f64 }
    #[setter(horizontalApertureOffset)] fn set_horizontal_aperture_offset(&mut self, v: f64) {
        self.0.set_horizontal_aperture_offset(v as f32);
    }
    #[getter] fn verticalApertureOffset(&self) -> f64 { self.0.vertical_aperture_offset() as f64 }
    #[setter(verticalApertureOffset)] fn set_vertical_aperture_offset(&mut self, v: f64) {
        self.0.set_vertical_aperture_offset(v as f32);
    }
    #[getter] fn focalLength(&self) -> f64 { self.0.focal_length() as f64 }
    #[setter(focalLength)] fn set_focal_length(&mut self, v: f64) { self.0.set_focal_length(v as f32); }
    #[getter] fn fStop(&self) -> f64 { self.0.f_stop() as f64 }
    #[setter(fStop)] fn set_f_stop(&mut self, v: f64) { self.0.set_f_stop(v as f32); }
    #[getter] fn focusDistance(&self) -> f64 { self.0.focus_distance() as f64 }
    #[setter(focusDistance)] fn set_focus_distance(&mut self, v: f64) { self.0.set_focus_distance(v as f32); }
    #[getter] fn aspectRatio(&self) -> f64 { self.0.aspect_ratio() as f64 }
    #[getter] fn clippingRange(&self) -> (f64, f64) {
        let r = self.0.clipping_range();
        (r.min() as f64, r.max() as f64)
    }
    #[setter(clippingRange)] fn set_clipping_range(&mut self, v: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(r) = v.extract::<PyRef<'_, PyRange1f>>() {
            self.0.set_clipping_range(r.0.clone());
            return Ok(());
        }
        if let Ok(t) = v.extract::<(f64, f64)>() {
            self.0.set_clipping_range(usd_gf::Range1f::new(t.0 as f32, t.1 as f32));
            return Ok(());
        }
        Err(PyTypeError::new_err("clippingRange: expected Range1f or (min, max)"))
    }
    #[getter] fn horizontalFieldOfView(&self) -> f64 {
        self.0.field_of_view(usd_gf::camera::FOVDirection::Horizontal) as f64
    }
    #[getter] fn verticalFieldOfView(&self) -> f64 {
        self.0.field_of_view(usd_gf::camera::FOVDirection::Vertical) as f64
    }

    #[pyo3(name = "SetFromViewAndProjectionMatrix")]
    #[pyo3(signature = (view, proj, focal_length = 50.0))]
    fn set_from_view_and_proj(&mut self, view: &super::matrix::PyMatrix4d, proj: &super::matrix::PyMatrix4d, focal_length: f32) {
        self.0.set_from_view_and_projection_matrix(&view.0, &proj.0, focal_length);
    }

    /// projection getter: 0 = Perspective, 1 = Orthographic
    #[getter] fn projection(&self) -> i32 {
        match self.0.projection() {
            usd_gf::CameraProjection::Perspective => 0,
            usd_gf::CameraProjection::Orthographic => 1,
        }
    }
    #[setter(projection)] fn set_projection(&mut self, v: i32) {
        let p = if v == 1 { usd_gf::CameraProjection::Orthographic } else { usd_gf::CameraProjection::Perspective };
        self.0.set_projection(p);
    }
}

// ---------------------------------------------------------------------------
// Plane — no set_normal/set_distance; rebuild via from_normal_distance
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Plane", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyPlane(pub usd_gf::Plane);

#[pymethods]
impl PyPlane {
    /// Plane(), Plane(normal, distance), Plane(normal, point), Plane(p0, p1, p2), Plane(Vec4d)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(usd_gf::Plane::default())); }
        if n == 1 {
            // Plane(Vec4d) — from equation
            if let Ok(v) = args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec4d>>() {
                return Ok(Self(usd_gf::Plane::from_equation(v.0)));
            }
        }
        if n == 2 {
            let a0 = args.get_item(0)?;
            let a1 = args.get_item(1)?;
            if let Ok(norm) = a0.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                // Plane(normal, distance: float)
                if let Ok(d) = a1.extract::<f64>() {
                    return Ok(Self(usd_gf::Plane::from_normal_distance(norm.0, d)));
                }
                // Plane(normal, point: Vec3d)
                if let Ok(pt) = a1.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                    return Ok(Self(usd_gf::Plane::from_normal_point(norm.0, pt.0)));
                }
            }
        }
        if n == 3 {
            // Plane(p0, p1, p2) — three points
            if let (Ok(p0), Ok(p1), Ok(p2)) = (
                args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(1)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(2)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
            ) {
                return Ok(Self(usd_gf::Plane::from_three_points(p0.0, p1.0, p2.0)));
            }
        }
        Err(PyTypeError::new_err("Plane: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String {
        let n = self.0.normal();
        format!("Gf.Plane(Gf.Vec3d({}, {}, {}), {})", n.x, n.y, n.z, self.0.distance_from_origin())
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let n = self.0.normal();
        hash_f64_n(&[n.x, n.y, n.z, self.0.distance_from_origin()])
    }

    #[pyo3(name = "GetNormal")]   fn get_normal(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(*self.0.normal())
    }
    #[pyo3(name = "GetDistanceFromOrigin")] fn get_distance_from_origin(&self) -> f64 {
        self.0.distance_from_origin()
    }
    #[pyo3(name = "GetDistance")] fn get_distance(&self, p: &super::vec::PyVec3d) -> f64 {
        self.0.distance(&p.0)
    }
    #[pyo3(name = "GetEquation")] fn get_equation(&self) -> super::vec::PyVec4d {
        super::vec::PyVec4d(self.0.equation())
    }
    #[pyo3(name = "IntersectsPositiveHalfSpace")]
    fn intersects_positive_half_space(&self, p: &Bound<'_, pyo3::PyAny>) -> PyResult<bool> {
        if let Ok(v) = p.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            return Ok(self.0.intersects_positive_half_space_point(&v.0));
        }
        if let Ok(r) = p.extract::<PyRef<'_, PyRange3d>>() {
            return Ok(self.0.intersects_positive_half_space_box(&r.0));
        }
        Err(PyTypeError::new_err("expected Vec3d or Range3d"))
    }
    #[pyo3(name = "SetNormal")]   fn set_normal(&mut self, n: &super::vec::PyVec3d) {
        self.0 = usd_gf::Plane::from_normal_distance(n.0, self.0.distance_from_origin());
    }
    #[pyo3(name = "SetDistance")] fn set_distance(&mut self, d: f64) {
        self.0 = usd_gf::Plane::from_normal_distance(*self.0.normal(), d);
    }
    /// Set(normal, distance) or Set(normal, point) or Set(p0, p1, p2) -> Plane
    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set_plane(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 2 {
            let a0 = args.get_item(0)?;
            let a1 = args.get_item(1)?;
            if let Ok(norm) = a0.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                if let Ok(d) = a1.extract::<f64>() {
                    self.0 = usd_gf::Plane::from_normal_distance(norm.0, d);
                    return Ok(self.clone());
                }
                if let Ok(pt) = a1.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                    self.0 = usd_gf::Plane::from_normal_point(norm.0, pt.0);
                    return Ok(self.clone());
                }
            }
        }
        if n == 3 {
            if let (Ok(p0), Ok(p1), Ok(p2)) = (
                args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(1)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(2)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
            ) {
                self.0 = usd_gf::Plane::from_three_points(p0.0, p1.0, p2.0);
                return Ok(self.clone());
            }
        }
        Err(PyTypeError::new_err("Plane.Set: unsupported arguments"))
    }
    /// Reorient(p) -> Plane (returns self, unlike C++ which is void)
    #[pyo3(name = "Reorient")] fn reorient(&mut self, p: &super::vec::PyVec3d) -> Self {
        self.0.reorient(&p.0);
        self.clone()
    }
    #[pyo3(name = "Project")] fn project(&self, p: &super::vec::PyVec3d) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.project(&p.0))
    }
    #[pyo3(name = "Transform")] fn transform(&mut self, m: &super::matrix::PyMatrix4d) {
        self.0.transform(&m.0);
    }

    #[getter] fn normal(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.normal()) }
    #[getter] fn distanceFromOrigin(&self) -> f64 { self.0.distance_from_origin() }
}

// ---------------------------------------------------------------------------
// Line — set() takes values by value; direction()/origin() return &Vec3d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Line", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyLine(pub usd_gf::Line);

#[pymethods]
impl PyLine {
    /// Line(), Line(origin, direction)
    #[new]
    #[pyo3(signature = (origin=None, direction=None))]
    fn new(origin: Option<&super::vec::PyVec3d>, direction: Option<&super::vec::PyVec3d>) -> Self {
        match (origin, direction) {
            (Some(o), Some(d)) => Self(usd_gf::Line::new(o.0, d.0)),
            _ => Self(usd_gf::Line::default()),
        }
    }

    fn __repr__(&self) -> String {
        let o = self.0.origin();
        let d = self.0.direction();
        format!("Gf.Line(Gf.Vec3d({}, {}, {}), Gf.Vec3d({}, {}, {}))", o.x, o.y, o.z, d.x, d.y, d.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let o = self.0.origin();
        let d = self.0.direction();
        hash_f64_n(&[o.x, o.y, o.z, d.x, d.y, d.z])
    }

    #[pyo3(name = "Set")] fn set(&mut self, p0: &super::vec::PyVec3d, direction: &super::vec::PyVec3d) -> f64 {
        self.0.set(p0.0, direction.0)
    }
    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "GetDirection")] fn get_direction(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(*self.0.direction())
    }
    #[pyo3(name = "FindClosestPoint")] fn find_closest_point(&self, p: &super::vec::PyVec3d) -> (super::vec::PyVec3d, f64) {
        let (pt, t) = self.0.find_closest_point(&p.0);
        (super::vec::PyVec3d(pt), t)
    }

    #[getter] fn direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.direction()) }
    #[setter] fn set_direction(&mut self, v: &super::vec::PyVec3d) { self.0.set(*self.0.origin(), v.0); }
    #[getter] fn origin(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.origin()) }
    #[setter] fn set_origin(&mut self, v: &super::vec::PyVec3d) { self.0.set(v.0, *self.0.direction()); }
}

// ---------------------------------------------------------------------------
// LineSeg
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "LineSeg", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyLineSeg(pub usd_gf::LineSeg);

#[pymethods]
impl PyLineSeg {
    #[new]
    #[pyo3(signature = (p0=None, p1=None))]
    fn new(p0: Option<&super::vec::PyVec3d>, p1: Option<&super::vec::PyVec3d>) -> Self {
        match (p0, p1) {
            (Some(a), Some(b)) => Self(usd_gf::LineSeg::new(a.0, b.0)),
            _ => Self(usd_gf::LineSeg::default()),
        }
    }

    fn __repr__(&self) -> String {
        let s = self.0.start();
        let e = self.0.end();
        format!("Gf.LineSeg(Gf.Vec3d({}, {}, {}), Gf.Vec3d({}, {}, {}))", s.x, s.y, s.z, e.x, e.y, e.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let s = self.0.start();
        let e = self.0.end();
        hash_f64_n(&[s.x, s.y, s.z, e.x, e.y, e.z])
    }

    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "GetDirection")] fn get_direction(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(*self.0.direction())
    }
    #[pyo3(name = "GetLength")] fn get_length(&self) -> f64 { self.0.length() }
    #[pyo3(name = "FindClosestPoint")] fn find_closest_point(&self, p: &super::vec::PyVec3d) -> (super::vec::PyVec3d, f64) {
        let (pt, t) = self.0.find_closest_point(&p.0);
        (super::vec::PyVec3d(pt), t)
    }

    #[getter] fn direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.direction()) }
    #[getter] fn length(&self) -> f64 { self.0.length() }
}

// ---------------------------------------------------------------------------
// Ray — set() takes values, transform() mutates; use transformed() for immut
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Ray", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyRay(pub usd_gf::Ray);

#[pymethods]
impl PyRay {
    #[new]
    #[pyo3(signature = (start=None, direction=None))]
    fn new(start: Option<&super::vec::PyVec3d>, direction: Option<&super::vec::PyVec3d>) -> Self {
        match (start, direction) {
            (Some(s), Some(d)) => Self(usd_gf::Ray::new(s.0, d.0)),
            _ => Self(usd_gf::Ray::default()),
        }
    }

    fn __repr__(&self) -> String {
        let s = self.0.start_point();
        let d = self.0.direction();
        format!("Gf.Ray(Gf.Vec3d({}, {}, {}), Gf.Vec3d({}, {}, {}))", s.x, s.y, s.z, d.x, d.y, d.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let s = self.0.start_point();
        let d = self.0.direction();
        hash_f64_n(&[s.x, s.y, s.z, d.x, d.y, d.z])
    }

    #[pyo3(name = "SetPointAndDirection")] fn set_point_and_dir(&mut self, p: &super::vec::PyVec3d, d: &super::vec::PyVec3d) {
        self.0.set(p.0, d.0);
    }
    #[pyo3(name = "SetEnds")] fn set_ends(&mut self, start: &super::vec::PyVec3d, end: &super::vec::PyVec3d) {
        self.0.set_endpoints(start.0, end.0);
    }
    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "FindClosestPoint")] fn find_closest_point(&self, p: &super::vec::PyVec3d) -> (super::vec::PyVec3d, f64) {
        let (pt, t) = self.0.find_closest_point(&p.0);
        (super::vec::PyVec3d(pt), t)
    }
    /// Transform mutates self (matches pxr API)
    #[pyo3(name = "Transform")] fn transform(&mut self, m: &super::matrix::PyMatrix4d) {
        self.0.transform(&m.0);
    }

    /// Intersect — polymorphic: sphere, plane, triangle, box, cylinder, cone
    #[pyo3(name = "Intersect")]
    #[pyo3(signature = (*args))]
    fn intersect(&self, py: Python<'_>, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Py<PyAny>> {
        let n = args.len();
        // Intersect(center: Vec3d, radius: float) -> (bool, enter, exit) -- sphere
        if n == 2 {
            if let (Ok(c), Ok(r)) = (args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(), args.get_item(1)?.extract::<f64>()) {
                return match self.0.intersect_sphere(&c.0, r) {
                    Some((enter, exit)) => {
                        // hit = true only if sphere has forward intersection
                        let hit = exit >= 0.0;
                        Ok((hit, enter, exit).into_pyobject(py)?.into_any().unbind())
                    }
                    None => Ok((false, 0.0_f64, 0.0_f64).into_pyobject(py)?.into_any().unbind()),
                };
            }
        }
        // Intersect(p0, p1, p2) -> (bool, dist, bary, front) -- triangle
        if n == 3 {
            if let (Ok(p0), Ok(p1), Ok(p2)) = (
                args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(1)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(2)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
            ) {
                return match self.0.intersect_triangle(&p0.0, &p1.0, &p2.0, f64::MAX) {
                    Some((dist, bary, front)) => {
                        let b = super::vec::PyVec3d(bary);
                        Ok((true, dist, b, front).into_pyobject(py)?.into_any().unbind())
                    }
                    None => {
                        let z = super::vec::PyVec3d(usd_gf::Vec3d::default());
                        Ok((false, 0.0_f64, z, false).into_pyobject(py)?.into_any().unbind())
                    }
                };
            }
        }
        // Intersect(plane: Plane) -> (bool, distance, front_facing)
        if n == 1 {
            let obj = args.get_item(0)?;
            // Plane
            if let Ok(p) = obj.extract::<PyRef<'_, PyPlane>>() {
                return match self.0.intersect_plane(&p.0) {
                    Some((dist, front)) => Ok((true, dist, front).into_pyobject(py)?.into_any().unbind()),
                    None => Ok((false, 0.0_f64, false).into_pyobject(py)?.into_any().unbind()),
                };
            }
            // Range3d (axis-aligned box)
            if let Ok(r) = obj.extract::<PyRef<'_, PyRange3d>>() {
                return match self.0.intersect_range(&r.0) {
                    Some((enter, exit)) => {
                        let hit = exit >= 0.0;
                        Ok((hit, enter, exit).into_pyobject(py)?.into_any().unbind())
                    }
                    None => Ok((false, 0.0_f64, 0.0_f64).into_pyobject(py)?.into_any().unbind()),
                };
            }
            // BBox3d (oriented box)
            if let Ok(b) = obj.extract::<PyRef<'_, PyBBox3d>>() {
                return match self.0.intersect_bbox(&b.0) {
                    Some((enter, exit)) => {
                        let hit = exit >= 0.0;
                        Ok((hit, enter, exit).into_pyobject(py)?.into_any().unbind())
                    }
                    None => Ok((false, 0.0_f64, 0.0_f64).into_pyobject(py)?.into_any().unbind()),
                };
            }
        }
        // Cylinder: (origin: Vec3d, axis: Vec3d, radius: float) -> (bool, enter, exit)
        if n == 3 {
            if let (Ok(origin), Ok(axis), Ok(radius)) = (
                args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(1)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(2)?.extract::<f64>(),
            ) {
                return match self.0.intersect_cylinder(&origin.0, &axis.0, radius) {
                    Some((enter, exit)) => {
                        let hit = exit >= 0.0;
                        Ok((hit, enter, exit).into_pyobject(py)?.into_any().unbind())
                    }
                    None => Ok((false, 0.0_f64, 0.0_f64).into_pyobject(py)?.into_any().unbind()),
                };
            }
        }
        // Cone: (origin: Vec3d, axis: Vec3d, radius: float, height: float) -> (bool, enter, exit)
        if n == 4 {
            if let (Ok(origin), Ok(axis), Ok(radius), Ok(height)) = (
                args.get_item(0)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(1)?.extract::<PyRef<'_, super::vec::PyVec3d>>(),
                args.get_item(2)?.extract::<f64>(),
                args.get_item(3)?.extract::<f64>(),
            ) {
                return match self.0.intersect_cone(&origin.0, &axis.0, radius, height) {
                    Some((enter, exit)) => {
                        let hit = exit >= 0.0;
                        Ok((hit, enter, exit).into_pyobject(py)?.into_any().unbind())
                    }
                    None => Ok((false, 0.0_f64, 0.0_f64).into_pyobject(py)?.into_any().unbind()),
                };
            }
        }
        Err(PyTypeError::new_err("Ray.Intersect: unsupported argument combination"))
    }

    #[getter] fn startPoint(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.start_point()) }
    #[setter] fn set_startPoint(&mut self, v: &super::vec::PyVec3d) { self.0.set(v.0, *self.0.direction()); }
    #[getter] fn direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.direction()) }
    #[setter] fn set_direction(&mut self, v: &super::vec::PyVec3d) { self.0.set(*self.0.start_point(), v.0); }
}

// ---------------------------------------------------------------------------
// Frustum
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Frustum", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyFrustum(pub Frustum);

#[pymethods]
impl PyFrustum {
    /// Projection type class attributes (Gf.Frustum.Perspective / .Orthographic)
    #[allow(non_upper_case_globals)]
    #[classattr] const Perspective: i32 = 0;
    #[allow(non_upper_case_globals)]
    #[classattr] const Orthographic: i32 = 1;

    /// Flexible constructor matching C++ GfFrustum Python overloads:
    ///   Frustum()
    ///   Frustum(Frustum)                         — copy
    ///   Frustum(Matrix4d, Range2d, Range1d, projType, viewDistance=5)
    ///   Frustum(Vec3d, Rotation, Range2d, Range1d, projType, viewDistance=5)
    ///   Frustum(position=, rotation=, window=, nearFar=, projectionType=, viewDistance=)
    #[new]
    #[pyo3(signature = (*args, position=None, rotation=None, window=None, nearFar=None, projectionType=None, viewDistance=None))]
    fn new(
        args: &Bound<'_, pyo3::types::PyTuple>,
        position: Option<&super::vec::PyVec3d>,
        rotation: Option<&PyRotation>,
        window: Option<&PyRange2d>,
        nearFar: Option<&PyRange1d>,
        projectionType: Option<i32>,
        viewDistance: Option<f64>,
    ) -> PyResult<Self> {
        let n = args.len();

        // No positional args: default or keyword-only construction
        if n == 0 {
            let has_kw = position.is_some() || rotation.is_some() || window.is_some()
                || nearFar.is_some() || projectionType.is_some() || viewDistance.is_some();
            if !has_kw {
                return Ok(Self(Frustum::new()));
            }
            // Keyword-only construction
            let pos = position.map(|p| p.0).unwrap_or_default();
            let rot = rotation.map(|r| r.0).unwrap_or_else(Rotation::new);
            let win = window.map(|w| w.0.clone()).unwrap_or_else(|| Range2d::new(
                usd_gf::Vec2d::new(-1.0, -1.0), usd_gf::Vec2d::new(1.0, 1.0)));
            let nf = nearFar.map(|r| r.0.clone()).unwrap_or_else(|| Range1d::new(1.0, 10.0));
            let pt = proj_type_from_int(projectionType.unwrap_or(0));
            let vd = viewDistance.unwrap_or(5.0);
            return Ok(Self(Frustum::from_params(pos, rot, win, nf, pt, vd)));
        }

        // 1 arg: copy constructor Frustum(Frustum)
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(f) = obj.extract::<PyRef<'_, PyFrustum>>() {
                return Ok(Self(f.0.clone()));
            }
        }

        // 4-5 args with Matrix4d first: Frustum(Matrix4d, Range2d, Range1d, projType, vd?)
        if n == 4 || n == 5 {
            let obj0 = args.get_item(0)?;
            if let Ok(m) = obj0.extract::<PyRef<'_, super::matrix::PyMatrix4d>>() {
                let win: PyRef<'_, PyRange2d> = args.get_item(1)?.extract()?;
                let nf: PyRef<'_, PyRange1d> = args.get_item(2)?.extract()?;
                let pt_int: i32 = args.get_item(3)?.extract()?;
                let vd = if n == 5 { args.get_item(4)?.extract()? } else { viewDistance.unwrap_or(5.0) };
                return Ok(Self(Frustum::from_matrix(&m.0, win.0.clone(), nf.0.clone(), proj_type_from_int(pt_int), vd)));
            }
        }

        // 5-6 args with Vec3d first: Frustum(Vec3d, Rotation, Range2d, Range1d, projType, vd?)
        if n == 5 || n == 6 {
            let pos = extract_vec3d(&args.get_item(0)?)?;
            let rot: PyRef<'_, PyRotation> = args.get_item(1)?.extract()?;
            let win: PyRef<'_, PyRange2d> = args.get_item(2)?.extract()?;
            let nf: PyRef<'_, PyRange1d> = args.get_item(3)?.extract()?;
            let pt_int: i32 = args.get_item(4)?.extract()?;
            let vd = if n == 6 { args.get_item(5)?.extract()? } else { viewDistance.unwrap_or(5.0) };
            return Ok(Self(Frustum::from_params(pos, rot.0, win.0.clone(), nf.0.clone(), proj_type_from_int(pt_int), vd)));
        }

        Err(PyTypeError::new_err("Frustum: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String {
        let f = &self.0;
        let pos = f.position();
        let rot = f.rotation();
        let ax = rot.axis();
        let win = f.window();
        let nf = f.near_far();
        let proj_name = match f.projection_type() {
            ProjectionType::Perspective => "Gf.Frustum.Perspective",
            ProjectionType::Orthographic => "Gf.Frustum.Orthographic",
        };
        format!(
            "Gf.Frustum(Gf.Vec3d({}, {}, {}), Gf.Rotation(Gf.Vec3d({}, {}, {}), {}), Gf.Range2d(Gf.Vec2d({}, {}), Gf.Vec2d({}, {})), Gf.Range1d({}, {}), {}, viewDistance = {})",
            pos.x, pos.y, pos.z,
            ax.x, ax.y, ax.z, rot.angle(),
            win.min().x, win.min().y, win.max().x, win.max().y,
            nf.min(), nf.max(),
            proj_name,
            f.view_distance(),
        )
    }
    fn __str__(&self) -> String { format!("{}", self.0) }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut h);
        h.finish()
    }

    // --- Properties ---
    #[getter] fn position(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.position()) }
    #[setter(position)] fn set_position_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_position(v.0); }
    #[getter] fn rotation(&self) -> PyRotation { PyRotation(*self.0.rotation()) }
    #[setter(rotation)] fn set_rotation_prop(&mut self, r: &PyRotation) { self.0.set_rotation(r.0); }
    #[getter] fn window(&self) -> PyRange2d { PyRange2d(self.0.window().clone()) }
    #[setter(window)] fn set_window_prop(&mut self, w: &PyRange2d) { self.0.set_window(w.0.clone()); }
    #[getter] fn nearFar(&self) -> PyRange1d { PyRange1d(self.0.near_far().clone()) }
    #[setter(nearFar)] fn set_near_far_prop(&mut self, nf: &PyRange1d) { self.0.set_near_far(nf.0.clone()); }
    #[getter] fn viewDistance(&self) -> f64 { self.0.view_distance() }
    #[setter(viewDistance)] fn set_view_distance_prop(&mut self, v: f64) { self.0.set_view_distance(v); }
    #[getter] fn projectionType(&self) -> i32 { proj_type_to_int(self.0.projection_type()) }
    #[setter(projectionType)] fn set_projection_type_prop(&mut self, v: i32) {
        self.0.set_projection_type(proj_type_from_int(v));
    }

    // --- Methods ---
    #[staticmethod]
    #[pyo3(name = "GetReferencePlaneDepth")]
    fn get_reference_plane_depth() -> f64 { Frustum::get_reference_plane_depth() }

    #[pyo3(name = "SetPerspective")]
    fn set_perspective(&mut self, fov: f64, is_fov_vertical: bool, aspect: f64, near: f64, far: f64) {
        self.0.set_perspective(fov, is_fov_vertical, aspect, near, far);
    }

    /// GetPerspective(isFovVertical) -> (fov, aspect, near, far) or None
    #[pyo3(name = "GetPerspective")]
    fn get_perspective(&self, py: Python<'_>, is_fov_vertical: bool) -> Py<pyo3::PyAny> {
        match self.0.get_perspective(is_fov_vertical) {
            Some((fov, aspect, near, far)) => (fov, aspect, near, far).into_pyobject(py).unwrap().into_any().unbind(),
            None => py.None().into_pyobject(py).unwrap().into_any().unbind(),
        }
    }

    /// GetFOV(isFovVertical?) -> f64. Default is horizontal (false).
    #[pyo3(name = "GetFOV")]
    #[pyo3(signature = (is_fov_vertical=None))]
    fn get_fov(&self, is_fov_vertical: Option<bool>) -> f64 {
        // C++ GetFOV() with no arg computes horizontal when perspective, 0 when ortho
        match is_fov_vertical {
            Some(v) => self.0.get_fov(v),
            None => {
                // Default: horizontal for perspective, 0 for ortho
                if self.0.projection_type() == ProjectionType::Orthographic {
                    0.0
                } else {
                    self.0.get_fov(false)
                }
            }
        }
    }

    #[pyo3(name = "SetOrthographic")]
    fn set_orthographic(&mut self, left: f64, right: f64, bottom: f64, top: f64, near: f64, far: f64) {
        self.0.set_orthographic(left, right, bottom, top, near, far);
    }

    /// GetOrthographic() -> (l,r,b,t,n,f) or empty tuple ()
    #[pyo3(name = "GetOrthographic")]
    fn get_orthographic(&self, py: Python<'_>) -> Py<pyo3::PyAny> {
        match self.0.get_orthographic() {
            Some((l, r, b, t, n, f)) => (l, r, b, t, n, f).into_pyobject(py).unwrap().into_any().unbind(),
            None => pyo3::types::PyTuple::empty(py).into_any().unbind(),
        }
    }

    #[pyo3(name = "ComputeViewDirection")]
    fn compute_view_direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.compute_view_direction()) }

    #[pyo3(name = "ComputeUpVector")]
    fn compute_up_vector(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.compute_up_vector()) }

    #[pyo3(name = "ComputeViewFrame")]
    fn compute_view_frame(&self) -> (super::vec::PyVec3d, super::vec::PyVec3d, super::vec::PyVec3d) {
        let (s, u, v) = self.0.compute_view_frame();
        (super::vec::PyVec3d(s), super::vec::PyVec3d(u), super::vec::PyVec3d(v))
    }

    #[pyo3(name = "ComputeLookAtPoint")]
    fn compute_look_at_point(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.compute_look_at_point()) }

    #[pyo3(name = "ComputeAspectRatio")]
    fn compute_aspect_ratio(&self) -> f64 { self.0.compute_aspect_ratio() }

    #[pyo3(name = "ComputeViewMatrix")]
    fn compute_view_matrix(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(self.0.compute_view_matrix()) }

    #[pyo3(name = "ComputeViewInverse")]
    fn compute_view_inverse(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(self.0.compute_view_inverse()) }

    #[pyo3(name = "ComputeProjectionMatrix")]
    fn compute_projection_matrix(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(self.0.compute_projection_matrix()) }

    /// ComputeCorners() -> list of 8 Vec3d
    #[pyo3(name = "ComputeCorners")]
    fn compute_corners(&self) -> Vec<super::vec::PyVec3d> {
        self.0.compute_corners().iter().map(|c| super::vec::PyVec3d(*c)).collect()
    }

    /// ComputeCornersAtDistance(d) -> list of 4 Vec3d
    #[pyo3(name = "ComputeCornersAtDistance")]
    fn compute_corners_at_distance(&self, distance: f64) -> Vec<super::vec::PyVec3d> {
        self.0.compute_corners_at_distance(distance).iter().map(|c| super::vec::PyVec3d(*c)).collect()
    }

    /// ComputePickRay(Vec2d) or ComputePickRay(Vec3d)
    #[pyo3(name = "ComputePickRay")]
    fn compute_pick_ray(&self, arg: &Bound<'_, pyo3::PyAny>) -> PyResult<PyRay> {
        if let Ok(v2) = arg.extract::<PyRef<'_, super::vec::PyVec2d>>() {
            return Ok(PyRay(self.0.compute_pick_ray_from_window(v2.0)));
        }
        if let Ok(v3) = arg.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            return Ok(PyRay(self.0.compute_pick_ray_from_world(v3.0)));
        }
        Err(PyTypeError::new_err("ComputePickRay: expected Vec2d or Vec3d"))
    }

    /// ComputeRay(Vec2d) or ComputeRay(Vec3d)
    #[pyo3(name = "ComputeRay")]
    fn compute_ray(&self, arg: &Bound<'_, pyo3::PyAny>) -> PyResult<PyRay> {
        if let Ok(v2) = arg.extract::<PyRef<'_, super::vec::PyVec2d>>() {
            return Ok(PyRay(self.0.compute_ray_from_window(v2.0)));
        }
        if let Ok(v3) = arg.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            return Ok(PyRay(self.0.compute_ray_from_world(v3.0)));
        }
        Err(PyTypeError::new_err("ComputeRay: expected Vec2d or Vec3d"))
    }

    /// ComputeNarrowedFrustum(Vec2d|Vec3d, Vec2d) -> Frustum
    #[pyo3(name = "ComputeNarrowedFrustum")]
    fn compute_narrowed_frustum(&self, pos: &Bound<'_, pyo3::PyAny>, size: &super::vec::PyVec2d) -> PyResult<Self> {
        if let Ok(v2) = pos.extract::<PyRef<'_, super::vec::PyVec2d>>() {
            return Ok(Self(self.0.compute_narrowed_frustum(v2.0, size.0)));
        }
        if let Ok(v3) = pos.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            return Ok(Self(self.0.compute_narrowed_frustum_at_world(v3.0, size.0)));
        }
        Err(PyTypeError::new_err("ComputeNarrowedFrustum: expected Vec2d or Vec3d as first arg"))
    }

    /// FitToSphere(center, radius, slack=0)
    #[pyo3(name = "FitToSphere")]
    #[pyo3(signature = (center, radius, slack=0.0))]
    fn fit_to_sphere(&mut self, center: &super::vec::PyVec3d, radius: f64, slack: f64) {
        self.0.fit_to_sphere(center.0, radius, slack);
    }

    /// Transform(matrix) -> self (returns self for chaining, matching C++ pxr)
    #[pyo3(name = "Transform")]
    fn transform(&mut self, m: &super::matrix::PyMatrix4d) -> Self {
        self.0.transform(&m.0);
        self.clone()
    }

    /// Intersects(Vec3d) / Intersects(BBox3d) / Intersects(Vec3d,Vec3d) / Intersects(Vec3d,Vec3d,Vec3d)
    #[pyo3(name = "Intersects")]
    #[pyo3(signature = (*args))]
    fn intersects(&self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<bool> {
        let n = args.len();
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                return Ok(self.0.intersects_point(&v.0));
            }
            if let Ok(b) = obj.extract::<PyRef<'_, PyBBox3d>>() {
                return Ok(self.0.intersects_bbox(&b.0));
            }
        }
        if n == 2 {
            let p0 = extract_vec3d(&args.get_item(0)?)?;
            let p1 = extract_vec3d(&args.get_item(1)?)?;
            return Ok(self.0.intersects_segment(&p0, &p1));
        }
        if n == 3 {
            let p0 = extract_vec3d(&args.get_item(0)?)?;
            let p1 = extract_vec3d(&args.get_item(1)?)?;
            let p2 = extract_vec3d(&args.get_item(2)?)?;
            return Ok(self.0.intersects_triangle(&p0, &p1, &p2));
        }
        Err(PyTypeError::new_err("Intersects: expected Vec3d, BBox3d, (Vec3d,Vec3d), or (Vec3d,Vec3d,Vec3d)"))
    }

    /// IntersectsViewVolume(bbox, viewProjMatrix) -> bool
    #[staticmethod]
    #[pyo3(name = "IntersectsViewVolume")]
    fn intersects_view_volume(bbox: &PyBBox3d, view_proj: &super::matrix::PyMatrix4d) -> bool {
        Frustum::intersects_view_volume(&bbox.0, &view_proj.0)
    }
}

/// Helper: int -> ProjectionType
fn proj_type_from_int(v: i32) -> ProjectionType {
    if v == 1 { ProjectionType::Orthographic } else { ProjectionType::Perspective }
}
/// Helper: ProjectionType -> int
fn proj_type_to_int(pt: ProjectionType) -> i32 {
    match pt { ProjectionType::Perspective => 0, ProjectionType::Orthographic => 1 }
}

// ---------------------------------------------------------------------------
// ColorSpaceNames — static holder for predefined name tokens
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "ColorSpaceNames", module = "pxr_rs.Gf")]
pub struct PyColorSpaceNames;

#[pymethods]
impl PyColorSpaceNames {
    #[classattr] #[pyo3(name = "LinearAP1")] fn linear_ap1() -> String { "lin_ap1_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearAP0")] fn linear_ap0() -> String { "lin_ap0_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearRec709")] fn linear_rec709() -> String { "lin_rec709_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearP3D65")] fn linear_p3d65() -> String { "lin_p3d65_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearRec2020")] fn linear_rec2020() -> String { "lin_rec2020_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearAdobeRGB")] fn linear_adobergb() -> String { "lin_adobergb_scene".to_string() }
    #[classattr] #[pyo3(name = "LinearCIEXYZD65")] fn linear_ciexyzd65() -> String { "lin_ciexyzd65_scene".to_string() }
    #[classattr] #[pyo3(name = "SRGBRec709")] fn srgb_rec709() -> String { "srgb_rec709_scene".to_string() }
    #[classattr] #[pyo3(name = "G22Rec709")] fn g22_rec709() -> String { "g22_rec709_scene".to_string() }
    #[classattr] #[pyo3(name = "G18Rec709")] fn g18_rec709() -> String { "g18_rec709_scene".to_string() }
    #[classattr] #[pyo3(name = "SRGBAP1")] fn srgb_ap1() -> String { "srgb_ap1_scene".to_string() }
    #[classattr] #[pyo3(name = "G22AP1")] fn g22_ap1() -> String { "g22_ap1_scene".to_string() }
    #[classattr] #[pyo3(name = "SRGBP3D65")] fn srgb_p3d65() -> String { "srgb_p3d65_scene".to_string() }
    #[classattr] #[pyo3(name = "G22AdobeRGB")] fn g22_adobergb() -> String { "g22_adobergb_scene".to_string() }
    #[classattr] #[pyo3(name = "Identity")] fn identity() -> String { "identity".to_string() }
    #[classattr] #[pyo3(name = "Data")] fn data() -> String { "data".to_string() }
    #[classattr] #[pyo3(name = "Raw")] fn raw() -> String { "raw".to_string() }
    #[classattr] #[pyo3(name = "Unknown")] fn unknown() -> String { "unknown".to_string() }
}

// ---------------------------------------------------------------------------
// ColorSpace
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "ColorSpace", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyColorSpace(pub ColorSpace);

#[pymethods]
impl PyColorSpace {
    /// ColorSpace(), ColorSpace(str), ColorSpace(ColorSpace)
    #[new]
    #[pyo3(signature = (name=None))]
    fn new(name: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let Some(obj) = name else {
            return Ok(Self(ColorSpace::default()));
        };
        // Copy constructor
        if let Ok(cs) = obj.extract::<PyRef<'_, PyColorSpace>>() {
            return Ok(Self(cs.0.clone()));
        }
        // From string name
        if let Ok(s) = obj.extract::<String>() {
            let token = usd_tf::Token::new(&s);
            return Ok(Self(ColorSpace::from_token(&token)));
        }
        Err(PyTypeError::new_err("ColorSpace: expected str or ColorSpace"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.ColorSpace(\"{}\")", self.0.name().as_str())
    }
    fn __str__(&self) -> String { format!("{}", self.0) }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.name().as_str().hash(&mut h);
        h.finish()
    }

    #[pyo3(name = "GetName")]
    fn get_name(&self) -> String { self.0.name().as_str().to_string() }

    #[pyo3(name = "IsValid")]
    fn is_valid(&self) -> bool {
        ColorSpaceName::parse(self.0.name().as_str()).is_some()
    }
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Color", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyColor(pub Color);

#[pymethods]
impl PyColor {
    /// Flexible constructor:
    ///   Color()                         — black in LinearRec709
    ///   Color(ColorSpace)               — black in given space
    ///   Color(Vec3f, ColorSpace)         — rgb in given space
    ///   Color(Color)                    — copy
    ///   Color(Color, ColorSpace)        — convert to target space
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(Color::black()));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            // Color(Color) — copy
            if let Ok(c) = obj.extract::<PyRef<'_, PyColor>>() {
                return Ok(Self(c.0.clone()));
            }
            // Color(ColorSpace) — black in given space
            if let Ok(cs) = obj.extract::<PyRef<'_, PyColorSpace>>() {
                return Ok(Self(Color::black_in(cs.0.clone())));
            }
        }
        if n == 2 {
            let a0 = args.get_item(0)?;
            let a1 = args.get_item(1)?;
            // Color(Vec3f, ColorSpace) — rgb in given space
            if let Ok(v) = a0.extract::<PyRef<'_, super::vec::PyVec3f>>() {
                let cs: PyRef<'_, PyColorSpace> = a1.extract()?;
                return Ok(Self(Color::new(v.0, cs.0.clone())));
            }
            // Color(Color, ColorSpace) — convert to target space
            if let Ok(src) = a0.extract::<PyRef<'_, PyColor>>() {
                let cs: PyRef<'_, PyColorSpace> = a1.extract()?;
                return Ok(Self(Color::convert(&src.0, cs.0.clone())));
            }
        }
        Err(PyTypeError::new_err("Color: unsupported constructor arguments"))
    }

    fn __repr__(&self) -> String {
        let rgb = self.0.rgb();
        let cs_name = self.0.color_space().name().as_str().to_string();
        format!("Gf.Color(Gf.Vec3f({}, {}, {}), Gf.ColorSpace(\"{}\"))", rgb.x, rgb.y, rgb.z, cs_name)
    }
    fn __str__(&self) -> String { format!("{}", self.0) }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "GetRGB")]
    fn get_rgb(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.rgb()) }

    #[pyo3(name = "GetColorSpace")]
    fn get_color_space(&self) -> PyColorSpace { PyColorSpace(self.0.color_space().clone()) }

    #[pyo3(name = "SetFromPlanckianLocus")]
    fn set_from_planckian_locus(&mut self, kelvin: f32, luminance: f32) {
        self.0.set_from_planckian_locus(kelvin, luminance);
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRotation>()?;
    m.add_class::<PyRange1d>()?;
    m.add_class::<PyRange1f>()?;
    m.add_class::<PyRange2d>()?;
    m.add_class::<PyRange2f>()?;
    m.add_class::<PyRange3d>()?;
    m.add_class::<PyRange3f>()?;
    m.add_class::<PyBBox3d>()?;
    m.add_class::<PyInterval>()?;
    m.add_class::<PyMultiInterval>()?;
    m.add_class::<PyRect2i>()?;
    m.add_class::<PySize2>()?;
    m.add_class::<PySize3>()?;
    m.add_class::<PyTransform>()?;
    m.add_class::<PyCamera>()?;
    m.add_class::<PyPlane>()?;
    m.add_class::<PyLine>()?;
    m.add_class::<PyLineSeg>()?;
    m.add_class::<PyRay>()?;
    m.add_class::<PyFrustum>()?;
    m.add_class::<PyColorSpaceNames>()?;
    m.add_class::<PyColorSpace>()?;
    m.add_class::<PyColor>()?;
    Ok(())
}
