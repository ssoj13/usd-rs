//! Geometric Python types: Rotation, BBox3d, Plane, Line, LineSeg, Ray,
//! Interval, MultiInterval, Rect2i, Size2, Size3, Transform, Camera, Frustum.

use pyo3::prelude::*;
use pyo3::exceptions::PyIndexError;
use usd_gf::{
    BBox3d, Rotation, Interval, MultiInterval, Rect2i, Size2, Size3,
    Range1d, Range1f, Range2d, Range2f, Range3d, Range3f,
};
use usd_gf::vec3::Vec3d;
use usd_gf::matrix4::Matrix4d;

// ---------------------------------------------------------------------------
// Rotation
// ---------------------------------------------------------------------------

#[pyclass(name = "Rotation", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyRotation(pub Rotation);

#[pymethods]
impl PyRotation {
    #[new]
    #[pyo3(signature = (axis=None, angle=0.0))]
    fn new(axis: Option<&super::vec::PyVec3d>, angle: f64) -> Self {
        if let Some(ax) = axis {
            Self(Rotation::from_axis_angle(ax.0, angle))
        } else {
            Self(Rotation::new())
        }
    }

    fn __repr__(&self) -> String {
        let ax = self.0.axis();
        format!("Gf.Rotation(({},{},{}), {})", ax.x, ax.y, ax.z, self.0.angle())
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }

    #[pyo3(name = "SetAxisAngle")] fn set_axis_angle(&mut self, axis: &super::vec::PyVec3d, angle: f64) {
        self.0.set_axis_angle(axis.0, angle);
    }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0.set_identity(); }
    #[pyo3(name = "SetQuat")] fn set_quat(&mut self, q: &super::quat::PyQuatd) { self.0.set_quat(&q.0); }

    #[pyo3(name = "GetAxis")]  fn get_axis(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.axis()) }
    #[pyo3(name = "GetAngle")] fn get_angle(&self) -> f64 { self.0.angle() }
    #[pyo3(name = "GetQuat")]  fn get_quat(&self) -> super::quat::PyQuatd {
        super::quat::PyQuatd(self.0.get_quat())
    }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }

    #[getter] fn axis(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.axis()) }
    #[getter] fn angle(&self) -> f64 { self.0.angle() }
    #[setter] fn set_axis_prop(&mut self, v: &super::vec::PyVec3d) {
        self.0.set_axis_angle(v.0, self.0.angle());
    }
    #[setter] fn set_angle_prop(&mut self, a: f64) {
        self.0.set_axis_angle(self.0.axis(), a);
    }
}

// ---------------------------------------------------------------------------
// Range1d / Range1f
// ---------------------------------------------------------------------------

#[pyclass(name = "Range1d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyRange1d(pub Range1d);

#[pymethods]
impl PyRange1d {
    #[new]
    #[pyo3(signature = (min=0.0, max=0.0))]
    fn new(min: f64, max: f64) -> Self { Self(Range1d::new(min, max)) }

    fn __repr__(&self) -> String { format!("Gf.Range1d({}, {})", self.0.min(), self.0.max()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

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
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }  // pxr uses camelCase
    #[setter] fn set_min_prop(&mut self, v: f64) { self.0.set_min(v); }
    #[setter] fn set_max_prop(&mut self, v: f64) { self.0.set_max(v); }
}

#[pyclass(name = "Range1f", module = "pxr.Gf")]
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
// Range2d / Range2f
// ---------------------------------------------------------------------------

#[pyclass(name = "Range2d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyRange2d(pub Range2d);

#[pymethods]
impl PyRange2d {
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&super::vec::PyVec2d>, max: Option<&super::vec::PyVec2d>) -> Self {
        match (min, max) {
            (Some(mn), Some(mx)) => Self(Range2d::new(mn.0, mx.0)),
            _ => Self(Range2d::empty()),
        }
    }

    fn __repr__(&self) -> String {
        format!("Gf.Range2d(({},{}), ({},{}))",
            self.0.min().x, self.0.min().y, self.0.max().x, self.0.max().y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec2d) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.max()) }
    #[getter] fn size(&self) -> super::vec::PyVec2d { super::vec::PyVec2d(self.0.size()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

#[pyclass(name = "Range2f", module = "pxr.Gf")]
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
        format!("Gf.Range2f(({},{}), ({},{}))",
            self.0.min().x, self.0.min().y, self.0.max().x, self.0.max().y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec2f) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "IsEmpty")] fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]  fn get_min(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.min()) }
    #[pyo3(name = "GetMax")]  fn get_max(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.max()) }
    #[pyo3(name = "GetSize")] fn get_size(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec2f { super::vec::PyVec2f(self.0.max()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

// ---------------------------------------------------------------------------
// Range3d / Range3f
// ---------------------------------------------------------------------------

#[pyclass(name = "Range3d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyRange3d(pub Range3d);

#[pymethods]
impl PyRange3d {
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<&super::vec::PyVec3d>, max: Option<&super::vec::PyVec3d>) -> Self {
        match (min, max) {
            (Some(mn), Some(mx)) => Self(Range3d::new(mn.0, mx.0)),
            _ => Self(Range3d::empty()),
        }
    }

    fn __repr__(&self) -> String {
        let mn = self.0.min(); let mx = self.0.max();
        format!("Gf.Range3d(({},{},{}), ({},{},{}))", mn.x,mn.y,mn.z, mx.x,mx.y,mx.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }

    #[pyo3(name = "Contains")] fn contains(&self, p: &super::vec::PyVec3d) -> bool {
        self.0.contains_point(&p.0)
    }
    #[pyo3(name = "Intersects")] fn intersects(&self, o: &Self) -> bool { self.0.intersects(&o.0) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.size()) }
    #[pyo3(name = "GetMidpoint")] fn get_midpoint(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.midpoint())
    }

    #[getter] fn min(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.max()) }
    #[getter] fn size(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.size()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
    #[setter] fn set_min_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_min(v.0); }
    #[setter] fn set_max_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_max(v.0); }
}

#[pyclass(name = "Range3f", module = "pxr.Gf")]
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
        let mn = self.0.min(); let mx = self.0.max();
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
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.min()) }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.max()) }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.size()) }

    #[getter] fn min(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.min()) }
    #[getter] fn max(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(self.0.max()) }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
}

// ---------------------------------------------------------------------------
// BBox3d
// ---------------------------------------------------------------------------

#[pyclass(name = "BBox3d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyBBox3d(pub BBox3d);

#[pymethods]
impl PyBBox3d {
    #[new]
    #[pyo3(signature = (range=None, matrix=None))]
    fn new(range: Option<&PyRange3d>, matrix: Option<&super::matrix::PyMatrix4d>) -> Self {
        match (range, matrix) {
            (Some(r), Some(m)) => Self(BBox3d::from_range_and_matrix(r.0, m.0)),
            (Some(r), None) => Self(BBox3d::from_range(r.0)),
            _ => Self(BBox3d::default()),
        }
    }

    fn __repr__(&self) -> String { format!("Gf.BBox3d(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "GetRange")]    fn get_range(&self) -> PyRange3d { PyRange3d(self.0.range()) }
    #[pyo3(name = "GetBox")]      fn get_box(&self) -> PyRange3d { PyRange3d(self.0.range()) }
    #[pyo3(name = "GetMatrix")]   fn get_matrix(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(self.0.matrix()) }
    #[pyo3(name = "GetInverseMatrix")] fn get_inverse_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(self.0.inverse_matrix())
    }
    #[pyo3(name = "SetRange")] fn set_range(&mut self, r: &PyRange3d) { self.0.set_range(r.0); }
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
        let mut b = self.0;
        b.transform(&m.0);
        Self(b)
    }
    #[staticmethod]
    #[pyo3(name = "Combine")]
    fn combine(a: &Self, b: &Self) -> Self { Self(BBox3d::combine(&a.0, &b.0)) }

    #[getter] fn box_(&self) -> PyRange3d { PyRange3d(self.0.range()) }
    #[getter] fn matrix(&self) -> super::matrix::PyMatrix4d { super::matrix::PyMatrix4d(self.0.matrix()) }
    #[getter] fn hasZeroAreaPrimitives(&self) -> bool { self.0.has_zero_area_primitives() }
    #[setter] fn set_box(&mut self, r: &PyRange3d) { self.0.set_range(r.0); }
    #[setter] fn set_matrix_prop(&mut self, m: &super::matrix::PyMatrix4d) { self.0.set_matrix(m.0); }
    #[setter] fn set_has_zero_area_primitives_prop(&mut self, v: bool) { self.0.set_has_zero_area_primitives(v); }
}

// ---------------------------------------------------------------------------
// Interval
// ---------------------------------------------------------------------------

#[pyclass(name = "Interval", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyInterval(pub Interval);

#[pymethods]
impl PyInterval {
    #[new]
    #[pyo3(signature = (min=0.0, max=0.0, min_closed=true, max_closed=true))]
    fn new(min: f64, max: f64, min_closed: bool, max_closed: bool) -> Self {
        Self(Interval::new(min, max, min_closed, max_closed))
    }

    fn __repr__(&self) -> String { format!("{}", self.0) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }
    fn __and__(&self, o: &Self) -> Self { Self(self.0.clone() & o.0.clone()) }

    #[pyo3(name = "Contains")] fn contains(&self, v: f64) -> bool { self.0.contains(v) }
    #[pyo3(name = "IsEmpty")]  fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "IsFinite")] fn is_finite(&self) -> bool { self.0.is_finite() }
    #[pyo3(name = "GetMin")]   fn get_min(&self) -> f64 { self.0.min() }
    #[pyo3(name = "GetMax")]   fn get_max(&self) -> f64 { self.0.max() }
    #[pyo3(name = "GetSize")]  fn get_size(&self) -> f64 { self.0.size() }
    #[pyo3(name = "Intersects")] fn intersects(&self, o: &Self) -> bool { self.0.intersects(&o.0) }

    #[staticmethod]
    #[pyo3(name = "GetFullInterval")]
    fn get_full_interval() -> Self { Self(Interval::full()) }

    #[getter] fn min(&self) -> f64 { self.0.min() }
    #[getter] fn max(&self) -> f64 { self.0.max() }
    #[getter] fn minClosed(&self) -> bool { self.0.min_closed() }
    #[getter] fn maxClosed(&self) -> bool { self.0.max_closed() }
    #[getter] fn minOpen(&self) -> bool { !self.0.min_closed() }
    #[getter] fn maxOpen(&self) -> bool { !self.0.max_closed() }
    #[getter] fn isEmpty(&self) -> bool { self.0.is_empty() }
    #[getter] fn size(&self) -> f64 { self.0.size() }
    #[getter] fn finite(&self) -> bool { self.0.is_finite() }
}

// ---------------------------------------------------------------------------
// MultiInterval
// ---------------------------------------------------------------------------

#[pyclass(name = "MultiInterval", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMultiInterval(pub MultiInterval);

#[pymethods]
impl PyMultiInterval {
    #[new]
    fn new() -> Self { Self(MultiInterval::new()) }

    fn __repr__(&self) -> String { format!("Gf.MultiInterval(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __bool__(&self) -> bool { !self.0.is_empty() }
    fn __and__(&self, o: &Self) -> Self { Self(self.0.clone() & o.0.clone()) }
    fn __or__(&self, o: &Self) -> Self { Self(self.0.clone() | o.0.clone()) }

    #[pyo3(name = "IsEmpty")]     fn is_empty(&self) -> bool { self.0.is_empty() }
    #[pyo3(name = "IsFinite")]    fn is_finite(&self) -> bool { self.0.is_finite() }
    #[pyo3(name = "Contains")]    fn contains(&self, v: f64) -> bool { self.0.contains(v) }
    #[pyo3(name = "GetSize")]     fn get_size(&self) -> f64 { self.0.size() }
    #[pyo3(name = "Union")]       fn union_with(&self, o: &Self) -> Self { Self(self.0.clone() | o.0.clone()) }
    #[pyo3(name = "Intersection")] fn intersection(&self, o: &Self) -> Self { Self(self.0.clone() & o.0.clone()) }

    #[staticmethod]
    #[pyo3(name = "GetFullInterval")]
    fn get_full_interval() -> PyInterval { PyInterval(Interval::full()) }
}

// ---------------------------------------------------------------------------
// Rect2i
// ---------------------------------------------------------------------------

#[pyclass(name = "Rect2i", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyRect2i(pub Rect2i);

#[pymethods]
impl PyRect2i {
    #[new]
    #[pyo3(signature = (min=None, max=None))]
    fn new(min: Option<(i32, i32)>, max: Option<(i32, i32)>) -> Self {
        match (min, max) {
            (Some((x0,y0)), Some((x1,y1))) => Self(Rect2i::new(
                usd_gf::Vec2i::new(x0,y0), usd_gf::Vec2i::new(x1,y1))),
            _ => Self(Rect2i::default()),
        }
    }

    fn __repr__(&self) -> String {
        format!("Gf.Rect2i(({},{}), ({},{}))",
            self.0.min().x, self.0.min().y, self.0.max().x, self.0.max().y)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "GetMin")]    fn get_min(&self) -> (i32,i32) { (self.0.min().x, self.0.min().y) }
    #[pyo3(name = "GetMax")]    fn get_max(&self) -> (i32,i32) { (self.0.max().x, self.0.max().y) }
    #[pyo3(name = "GetWidth")]  fn get_width(&self) -> i32 { self.0.width() }
    #[pyo3(name = "GetHeight")] fn get_height(&self) -> i32 { self.0.height() }
    #[pyo3(name = "GetSize")]   fn get_size(&self) -> (i32,i32) { (self.0.width(), self.0.height()) }
    #[pyo3(name = "GetArea")]   fn get_area(&self) -> i64 { self.0.area() }
    #[pyo3(name = "Contains")]  fn contains(&self, p: (i32,i32)) -> bool {
        self.0.contains(&usd_gf::Vec2i::new(p.0, p.1))
    }
    #[pyo3(name = "SetMin")]    fn set_min(&mut self, p: (i32,i32)) {
        self.0.set_min(usd_gf::Vec2i::new(p.0, p.1));
    }
    #[pyo3(name = "SetMax")]    fn set_max(&mut self, p: (i32,i32)) {
        self.0.set_max(usd_gf::Vec2i::new(p.0, p.1));
    }

    #[getter] fn min(&self) -> (i32,i32) { (self.0.min().x, self.0.min().y) }
    #[getter] fn max(&self) -> (i32,i32) { (self.0.max().x, self.0.max().y) }
    #[getter] fn width(&self) -> i32 { self.0.width() }
    #[getter] fn height(&self) -> i32 { self.0.height() }
    #[getter] fn area(&self) -> i64 { self.0.area() }
    #[setter] fn set_min_prop(&mut self, p: (i32,i32)) { self.0.set_min(usd_gf::Vec2i::new(p.0,p.1)); }
    #[setter] fn set_max_prop(&mut self, p: (i32,i32)) { self.0.set_max(usd_gf::Vec2i::new(p.0,p.1)); }
}

// ---------------------------------------------------------------------------
// Size2 / Size3
// ---------------------------------------------------------------------------

#[pyclass(name = "Size2", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PySize2(pub Size2);

#[pymethods]
impl PySize2 {
    #[new]
    #[pyo3(signature = (x=0, y=0))]
    fn new(x: usize, y: usize) -> Self { Self(Size2::new(x, y)) }

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
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, s: usize) -> Self { Self(self.0 * s) }
    fn __neg__(&self) -> Self {
        // Size2 uses usize — wrap via isize cast for API compat (edge case)
        Self(Size2::new(0usize.wrapping_sub(self.0[0]), 0usize.wrapping_sub(self.0[1])))
    }
    #[pyo3(name = "Get")] fn get(&self) -> (usize, usize) { (self.0[0], self.0[1]) }
    #[pyo3(name = "Set")] fn set(&mut self, x: usize, y: usize) {
        self.0[0] = x; self.0[1] = y;
    }
}

#[pyclass(name = "Size3", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PySize3(pub Size3);

#[pymethods]
impl PySize3 {
    #[new]
    #[pyo3(signature = (x=0, y=0, z=0))]
    fn new(x: usize, y: usize, z: usize) -> Self { Self(Size3::new(x, y, z)) }

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
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, s: usize) -> Self { Self(self.0 * s) }
    #[pyo3(name = "Get")] fn get(&self) -> (usize, usize, usize) { (self.0[0], self.0[1], self.0[2]) }
    #[pyo3(name = "Set")] fn set(&mut self, x: usize, y: usize, z: usize) {
        self.0[0] = x; self.0[1] = y; self.0[2] = z;
    }
}

// ---------------------------------------------------------------------------
// Transform (thin wrapper — delegates to GfTransform)
// ---------------------------------------------------------------------------

#[pyclass(name = "Transform", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyTransform(pub usd_gf::Transform);

#[pymethods]
impl PyTransform {
    #[new]
    fn new() -> Self { Self(usd_gf::Transform::default()) }

    fn __repr__(&self) -> String { format!("Gf.Transform(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }

    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0.set_identity(); }
    #[pyo3(name = "GetMatrix")]   fn get_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(self.0.matrix())
    }
    #[pyo3(name = "SetMatrix")]   fn set_matrix(&mut self, m: &super::matrix::PyMatrix4d) {
        self.0.set_matrix(&m.0);
    }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> super::matrix::PyMatrix4d {
        // GfTransform::GetInverse returns a matrix, not a Transform
        super::matrix::PyMatrix4d(self.0.matrix().inverse())
    }
    #[pyo3(name = "GetInverseMatrix")] fn get_inverse_matrix(&self) -> super::matrix::PyMatrix4d {
        super::matrix::PyMatrix4d(self.0.matrix().inverse())
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
}

// ---------------------------------------------------------------------------
// Camera (thin wrapper — exposes common properties)
// ---------------------------------------------------------------------------

#[pyclass(name = "Camera", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyCamera(pub usd_gf::Camera);

#[pymethods]
impl PyCamera {
    #[new]
    fn new() -> Self { Self(usd_gf::Camera::default()) }

    fn __repr__(&self) -> String { format!("Gf.Camera(...)") }
    fn __str__(&self) -> String { self.__repr__() }

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
    #[setter] fn set_transform_prop(&mut self, m: &super::matrix::PyMatrix4d) {
        self.0.set_transform(m.0);
    }
    #[getter] fn horizontalAperture(&self) -> f64 { self.0.horizontal_aperture() as f64 }
    #[setter] fn set_horizontal_aperture(&mut self, v: f64) { self.0.set_horizontal_aperture(v as f32); }
    #[getter] fn verticalAperture(&self) -> f64 { self.0.vertical_aperture() as f64 }
    #[setter] fn set_vertical_aperture(&mut self, v: f64) { self.0.set_vertical_aperture(v as f32); }
    #[getter] fn horizontalApertureOffset(&self) -> f64 { self.0.horizontal_aperture_offset() as f64 }
    #[setter] fn set_horizontal_aperture_offset(&mut self, v: f64) {
        self.0.set_horizontal_aperture_offset(v as f32);
    }
    #[getter] fn verticalApertureOffset(&self) -> f64 { self.0.vertical_aperture_offset() as f64 }
    #[setter] fn set_vertical_aperture_offset(&mut self, v: f64) {
        self.0.set_vertical_aperture_offset(v as f32);
    }
    #[getter] fn focalLength(&self) -> f64 { self.0.focal_length() as f64 }
    #[setter] fn set_focal_length(&mut self, v: f64) { self.0.set_focal_length(v as f32); }
    #[getter] fn fStop(&self) -> f64 { self.0.f_stop() as f64 }
    #[setter] fn set_f_stop(&mut self, v: f64) { self.0.set_f_stop(v as f32); }
    #[getter] fn focusDistance(&self) -> f64 { self.0.focus_distance() as f64 }
    #[setter] fn set_focus_distance(&mut self, v: f64) { self.0.set_focus_distance(v as f32); }
    #[getter] fn aspectRatio(&self) -> f64 { self.0.aspect_ratio() as f64 }
    #[getter] fn clippingRange(&self) -> (f64, f64) {
        let r = self.0.clipping_range();
        (r.min() as f64, r.max() as f64)
    }
    #[setter] fn set_clipping_range(&mut self, v: (f64, f64)) {
        self.0.set_clipping_range(usd_gf::Range1f::new(v.0 as f32, v.1 as f32));
    }
    #[getter] fn horizontalFieldOfView(&self) -> f64 {
        self.0.field_of_view(usd_gf::camera::FOVDirection::Horizontal) as f64
    }
    #[getter] fn verticalFieldOfView(&self) -> f64 {
        self.0.field_of_view(usd_gf::camera::FOVDirection::Vertical) as f64
    }
}

// ---------------------------------------------------------------------------
// Plane
// ---------------------------------------------------------------------------

#[pyclass(name = "Plane", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyPlane(pub usd_gf::Plane);

#[pymethods]
impl PyPlane {
    #[new]
    #[pyo3(signature = (normal=None, distance=0.0))]
    fn new(normal: Option<&super::vec::PyVec3d>, distance: f64) -> Self {
        if let Some(n) = normal {
            Self(usd_gf::Plane::from_normal_distance(n.0, distance))
        } else {
            Self(usd_gf::Plane::default())
        }
    }

    fn __repr__(&self) -> String {
        let n = self.0.normal();
        format!("Gf.Plane(({},{},{}), {})", n.x, n.y, n.z, self.0.distance_from_origin())
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "GetNormal")]   fn get_normal(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.normal()) }
    #[pyo3(name = "GetDistanceFromOrigin")] fn get_distance_from_origin(&self) -> f64 { self.0.distance_from_origin() }
    #[pyo3(name = "GetDistance")] fn get_distance(&self, p: &super::vec::PyVec3d) -> f64 {
        self.0.distance(&p.0)
    }
    #[pyo3(name = "SetNormal")]   fn set_normal(&mut self, n: &super::vec::PyVec3d) { self.0.set_normal(n.0); }
    #[pyo3(name = "SetDistance")] fn set_distance(&mut self, d: f64) { self.0.set_distance_from_origin(d); }
}

// ---------------------------------------------------------------------------
// Line
// ---------------------------------------------------------------------------

#[pyclass(name = "Line", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyLine(pub usd_gf::Line);

#[pymethods]
impl PyLine {
    #[new]
    fn new() -> Self { Self(usd_gf::Line::default()) }

    fn __repr__(&self) -> String { format!("Gf.Line(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "Set")] fn set(&mut self, p0: &super::vec::PyVec3d, p1: &super::vec::PyVec3d) -> bool {
        self.0.set(&p0.0, &p1.0)
    }
    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "GetDirection")] fn get_direction(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.direction())
    }
}

// ---------------------------------------------------------------------------
// LineSeg
// ---------------------------------------------------------------------------

#[pyclass(name = "LineSeg", module = "pxr.Gf")]
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

    fn __repr__(&self) -> String { format!("Gf.LineSeg(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "GetDirection")] fn get_direction(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.direction())
    }
    #[pyo3(name = "GetLength")] fn get_length(&self) -> f64 { self.0.length() }

    #[getter] fn direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.direction()) }
    #[getter] fn length(&self) -> f64 { self.0.length() }
}

// ---------------------------------------------------------------------------
// Ray
// ---------------------------------------------------------------------------

#[pyclass(name = "Ray", module = "pxr.Gf")]
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

    fn __repr__(&self) -> String { format!("Gf.Ray(...)") }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[pyo3(name = "SetPointAndDirection")] fn set_point_and_dir(&mut self, p: &super::vec::PyVec3d, d: &super::vec::PyVec3d) {
        self.0.set_point_and_direction(p.0, d.0);
    }
    #[pyo3(name = "GetPoint")] fn get_point(&self, t: f64) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.point(t))
    }
    #[pyo3(name = "FindClosestPoint")] fn find_closest_point(&self, p: &super::vec::PyVec3d) -> (super::vec::PyVec3d, f64) {
        let (pt, t) = self.0.find_closest_point(&p.0);
        (super::vec::PyVec3d(pt), t)
    }
    #[pyo3(name = "Transform")] fn transform(&self, m: &super::matrix::PyMatrix4d) -> Self {
        Self(self.0.transform(&m.0))
    }

    #[getter] fn startPoint(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.start_point()) }
    #[getter] fn direction(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(self.0.direction()) }
    #[setter] fn set_start_point(&mut self, p: &super::vec::PyVec3d) { self.0.set_start_point(p.0); }
    #[setter] fn set_direction(&mut self, d: &super::vec::PyVec3d) { self.0.set_direction(d.0); }
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
    Ok(())
}
