//! Vec2/3/4 Python bindings (all scalar variants: d/f/h/i).
//!
//! Macro-driven to reduce repetition. Each vec type gets the same interface
//! with scalar-appropriate implementations. Half-precision uses f32 at the Python
//! boundary since Python has no native f16.

use pyo3::exceptions::{PyIndexError, PyValueError, PyZeroDivisionError};
use pyo3::prelude::*;
use usd_gf::half::Half;

// ---------------------------------------------------------------------------
// Shared index normalization
// ---------------------------------------------------------------------------

pub(super) fn norm_idx(i: isize, len: usize) -> PyResult<usize> {
    let idx = if i < 0 { len as isize + i } else { i };
    if idx < 0 || idx >= len as isize {
        Err(PyIndexError::new_err("index out of range"))
    } else {
        Ok(idx as usize)
    }
}

/// Validate that `vals` length matches `indices` length for slice assignment.
pub(super) fn check_slice_len<T>(vals: &[T], indices: &[usize]) -> PyResult<()> {
    if vals.len() != indices.len() {
        return Err(PyValueError::new_err(format!(
            "attempt to assign sequence of size {} to extended slice of size {}",
            vals.len(),
            indices.len()
        )));
    }
    Ok(())
}

/// Resolve a Python slice(start, stop, step) for a vector of `len` elements.
/// Returns the indices to include.
pub(super) fn resolve_slice(
    py: Python<'_>,
    key: &Bound<'_, pyo3::PyAny>,
    len: usize,
) -> PyResult<Option<Vec<usize>>> {
    let slice_type = py.get_type::<pyo3::types::PySlice>();
    if !key.is_instance(&slice_type)? {
        return Ok(None);
    }
    let slice: &Bound<'_, pyo3::types::PySlice> = key.cast()?;
    let indices = slice.indices(len as isize)?;
    let mut result = Vec::new();
    let mut i = indices.start;
    while (indices.step > 0 && i < indices.stop) || (indices.step < 0 && i > indices.stop) {
        result.push(i as usize);
        i += indices.step;
    }
    Ok(Some(result))
}

// ---------------------------------------------------------------------------
// Vec2d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec2d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec2d(pub usd_gf::Vec2d);

#[pymethods]
impl PyVec2d {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec2d::new(0.0, 0.0)));
        }
        if n == 2 {
            let x: f64 = args.get_item(0)?.extract()?;
            let y: f64 = args.get_item(1)?.extract()?;
            return Ok(Self(usd_gf::Vec2d::new(x, y)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2d>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2f>>() {
                return Ok(Self(usd_gf::Vec2d::new(v.0.x as f64, v.0.y as f64)));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2h>>() {
                return Ok(Self(usd_gf::Vec2d::new(
                    v.0.x.to_f32() as f64,
                    v.0.y.to_f32() as f64,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2i>>() {
                return Ok(Self(usd_gf::Vec2d::new(v.0.x as f64, v.0.y as f64)));
            }
            if let Ok(seq) = obj.extract::<Vec<f64>>() {
                if seq.len() == 2 {
                    return Ok(Self(usd_gf::Vec2d::new(seq[0], seq[1])));
                }
            }
            if let Ok(s) = obj.extract::<f64>() {
                return Ok(Self(usd_gf::Vec2d::new(s, s)));
            }
        }
        Err(PyValueError::new_err("Vec2d: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec2d({}, {})", self.0.x, self.0.y)
    }
    fn __str__(&self) -> String {
        format!("({}, {})", self.0.x, self.0.y)
    }
    fn __len__(&self) -> usize {
        2
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2d>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2f>>() {
            return self.0.x == v.0.x as f64 && self.0.y == v.0.y as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2h>>() {
            return self.0.x == v.0.x.to_f32() as f64 && self.0.y == v.0.y.to_f32() as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2i>>() {
            return self.0.x == v.0.x as f64 && self.0.y == v.0.y as f64;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash2(self.0.x.to_bits(), self.0.y.to_bits())
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec2d::new(-self.0.x, -self.0.y))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f64> = vec![slf.0.x, slf.0.y];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec2d to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f64> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 2)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f64> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f64 = val.extract()?;
        *elems[norm_idx(i, 2)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f64) -> bool {
        self.0.x == v || self.0.y == v
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2d::new(self.0.x + o.0.x, self.0.y + o.0.y))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2d::new(self.0.x - o.0.x, self.0.y - o.0.y))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2d>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(usd_gf::Vec2d::new(self.0.x * s, self.0.y * s))
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f64) -> Self {
        Self(usd_gf::Vec2d::new(self.0.x * s, self.0.y * s))
    }
    fn __imul__(&mut self, s: f64) {
        self.0.x *= s;
        self.0.y *= s;
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec2d::new(self.0.x / s, self.0.y / s)))
    }
    fn __itruediv__(&mut self, s: f64) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec2d::new(1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec2d::new(0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f64 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec2d::new(self.0.x / l, self.0.y / l))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f64 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f64 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec2d::new(0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec2d::new(onto.0.x * s, onto.0.y * s))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec2d::new(self.0.x - p.0.x, self.0.y - p.0.y))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 2;
    #[getter]
    fn dimension(&self) -> usize {
        2
    }
}

// ---------------------------------------------------------------------------
// Vec2f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec2f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec2f(pub usd_gf::Vec2f);

#[pymethods]
impl PyVec2f {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec2f::new(0.0, 0.0)));
        }
        if n == 2 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            return Ok(Self(usd_gf::Vec2f::new(x, y)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2f>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2d>>() {
                return Ok(Self(usd_gf::Vec2f::new(v.0.x as f32, v.0.y as f32)));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2h>>() {
                return Ok(Self(usd_gf::Vec2f::new(v.0.x.to_f32(), v.0.y.to_f32())));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2i>>() {
                return Ok(Self(usd_gf::Vec2f::new(v.0.x as f32, v.0.y as f32)));
            }
            if let Ok(seq) = obj.extract::<Vec<f32>>() {
                if seq.len() == 2 {
                    return Ok(Self(usd_gf::Vec2f::new(seq[0], seq[1])));
                }
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec2f::new(s, s)));
            }
        }
        Err(PyValueError::new_err("Vec2f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec2f({}, {})", self.0.x, self.0.y)
    }
    fn __str__(&self) -> String {
        format!("({}, {})", self.0.x, self.0.y)
    }
    fn __len__(&self) -> usize {
        2
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2f>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2h>>() {
            return self.0.x == v.0.x.to_f32() && self.0.y == v.0.y.to_f32();
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2i>>() {
            return self.0.x == v.0.x as f32 && self.0.y == v.0.y as f32;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash2(self.0.x.to_bits() as u64, self.0.y.to_bits() as u64)
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec2f::new(-self.0.x, -self.0.y))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f32> = vec![slf.0.x, slf.0.y];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec2f to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 2)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        *elems[norm_idx(i, 2)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f32) -> bool {
        self.0.x == v || self.0.y == v
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2f::new(self.0.x + o.0.x, self.0.y + o.0.y))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2f::new(self.0.x - o.0.x, self.0.y - o.0.y))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2f>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(Self(usd_gf::Vec2f::new(self.0.x * s, self.0.y * s))
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f32) -> Self {
        Self(usd_gf::Vec2f::new(self.0.x * s, self.0.y * s))
    }
    fn __imul__(&mut self, s: f32) {
        self.0.x *= s;
        self.0.y *= s;
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec2f::new(self.0.x / s, self.0.y / s)))
    }
    fn __itruediv__(&mut self, s: f32) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec2f::new(1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec2f::new(0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f32 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec2f::new(self.0.x / l, self.0.y / l))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f32 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f32 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec2f::new(0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec2f::new(onto.0.x * s, onto.0.y * s))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec2f::new(self.0.x - p.0.x, self.0.y - p.0.y))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 2;
    #[getter]
    fn dimension(&self) -> usize {
        2
    }
}

// ---------------------------------------------------------------------------
// Vec2h (half-precision — Python boundary uses f32)
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec2h", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec2h(pub usd_gf::Vec2h);

#[pymethods]
impl PyVec2h {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec2h::new(
                Half::from_f32(0.0),
                Half::from_f32(0.0),
            )));
        }
        if n == 2 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            return Ok(Self(usd_gf::Vec2h::new(
                Half::from_f32(x),
                Half::from_f32(y),
            )));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2h>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2d>>() {
                return Ok(Self(usd_gf::Vec2h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2f>>() {
                return Ok(Self(usd_gf::Vec2h::new(
                    Half::from_f32(v.0.x),
                    Half::from_f32(v.0.y),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2i>>() {
                return Ok(Self(usd_gf::Vec2h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                )));
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec2h::new(
                    Half::from_f32(s),
                    Half::from_f32(s),
                )));
            }
        }
        Err(PyValueError::new_err("Vec2h: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec2h({}, {})", self.0.x.to_f32(), self.0.y.to_f32())
    }
    fn __str__(&self) -> String {
        format!("({}, {})", self.0.x.to_f32(), self.0.y.to_f32())
    }
    fn __len__(&self) -> usize {
        2
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        // Half uses .bits() not .to_bits()
        hash2(self.0.x.bits() as u64, self.0.y.bits() as u64)
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec2h::new(-self.0.x, -self.0.y))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f32> = vec![slf.0.x.to_f32(), slf.0.y.to_f32()];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec2h to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x.to_f32(), self.0.y.to_f32()];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 2)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            let elems = [&mut self.0.x, &mut self.0.y];
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = Half::from_f32(vals[idx]);
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        match norm_idx(i, 2)? {
            0 => self.0.x = Half::from_f32(v),
            1 => self.0.y = Half::from_f32(v),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2h::new(self.0.x + o.0.x, self.0.y + o.0.y))
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2h::new(self.0.x - o.0.x, self.0.y - o.0.y))
    }
    fn __mul__(&self, s: f32) -> Self {
        let hs = Half::from_f32(s);
        Self(usd_gf::Vec2h::new(self.0.x * hs, self.0.y * hs))
    }
    fn __rmul__(&self, s: f32) -> Self {
        self.__mul__(s)
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        let hs = Half::from_f32(s);
        Ok(Self(usd_gf::Vec2h::new(self.0.x / hs, self.0.y / hs)))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 2;
    #[getter]
    fn dimension(&self) -> usize {
        2
    }
}

// ---------------------------------------------------------------------------
// Vec2i
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec2i", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec2i(pub usd_gf::Vec2i);

#[pymethods]
impl PyVec2i {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec2i::new(0, 0)));
        }
        if n == 2 {
            let x: i32 = args.get_item(0)?.extract()?;
            let y: i32 = args.get_item(1)?.extract()?;
            return Ok(Self(usd_gf::Vec2i::new(x, y)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec2i>>() {
                return Ok(Self(v.0));
            }
            if let Ok(seq) = obj.extract::<Vec<i32>>() {
                if seq.len() == 2 {
                    return Ok(Self(usd_gf::Vec2i::new(seq[0], seq[1])));
                }
            }
            if let Ok(s) = obj.extract::<i32>() {
                return Ok(Self(usd_gf::Vec2i::new(s, s)));
            }
        }
        Err(PyValueError::new_err("Vec2i: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec2i({}, {})", self.0.x, self.0.y)
    }
    fn __str__(&self) -> String {
        format!("({}, {})", self.0.x, self.0.y)
    }
    fn __len__(&self) -> usize {
        2
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        hash2(self.0.x as u64, self.0.y as u64)
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec2i::new(-self.0.x, -self.0.y))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<i32> = vec![slf.0.x, slf.0.y];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec2i to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<i32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 2)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y];
        if let Some(indices) = resolve_slice(py, key, 2)? {
            let vals: Vec<i32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: i32 = val.extract()?;
        *elems[norm_idx(i, 2)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: i32) -> bool {
        self.0.x == v || self.0.y == v
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2i::new(self.0.x + o.0.x, self.0.y + o.0.y))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2i::new(self.0.x - o.0.x, self.0.y - o.0.y))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec2i>>() {
            return Ok((self.0.x * v.0.x + self.0.y * v.0.y)
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        if let Ok(s) = o.extract::<i32>() {
            return Ok(Self(usd_gf::Vec2i::new(self.0.x * s, self.0.y * s))
                .into_pyobject(py)?
                .into_any()
                .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: i32) -> Self {
        Self(usd_gf::Vec2i::new(self.0.x * s, self.0.y * s))
    }
    fn __imul__(&mut self, s: i32) {
        self.0.x *= s;
        self.0.y *= s;
    }
    fn __truediv__(&self, s: i32) -> PyResult<Self> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec2i::new(self.0.x / s, self.0.y / s)))
    }
    fn __itruediv__(&mut self, s: i32) -> PyResult<()> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec2i::new(1, 0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec2i::new(0, 1))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 2;
    #[getter]
    fn dimension(&self) -> usize {
        2
    }
}

// ---------------------------------------------------------------------------
// Vec3d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec3d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec3d(pub usd_gf::Vec3d);

#[pymethods]
impl PyVec3d {
    /// Vec3d(), Vec3d(scalar), Vec3d(x,y,z), Vec3d(Vec3d), Vec3d(Vec3f), Vec3d(Vec3h), Vec3d(Vec3i),
    /// Vec3d((x,y,z)), Vec3d([x,y,z])
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec3d::new(0.0, 0.0, 0.0)));
        }
        if n == 3 {
            let x: f64 = args.get_item(0)?.extract()?;
            let y: f64 = args.get_item(1)?.extract()?;
            let z: f64 = args.get_item(2)?.extract()?;
            return Ok(Self(usd_gf::Vec3d::new(x, y, z)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            // Copy constructor
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3d>>() {
                return Ok(Self(v.0));
            }
            // Cross-type constructors
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3f>>() {
                return Ok(Self(usd_gf::Vec3d::new(
                    v.0.x as f64,
                    v.0.y as f64,
                    v.0.z as f64,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3h>>() {
                return Ok(Self(usd_gf::Vec3d::new(
                    v.0.x.to_f32() as f64,
                    v.0.y.to_f32() as f64,
                    v.0.z.to_f32() as f64,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3i>>() {
                return Ok(Self(usd_gf::Vec3d::new(
                    v.0.x as f64,
                    v.0.y as f64,
                    v.0.z as f64,
                )));
            }
            // Tuple/list
            if let Ok(seq) = obj.extract::<Vec<f64>>() {
                if seq.len() == 3 {
                    return Ok(Self(usd_gf::Vec3d::new(seq[0], seq[1], seq[2])));
                }
            }
            // Single scalar → broadcast
            if let Ok(s) = obj.extract::<f64>() {
                return Ok(Self(usd_gf::Vec3d::new(s, s, s)));
            }
        }
        Err(PyValueError::new_err(
            "Vec3d: expected (), (scalar), (x,y,z), Vec3d, Vec3f, Vec3h, Vec3i, or sequence",
        ))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec3d({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __len__(&self) -> usize {
        3
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3d>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3f>>() {
            return self.0.x == v.0.x as f64
                && self.0.y == v.0.y as f64
                && self.0.z == v.0.z as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3h>>() {
            return self.0.x == v.0.x.to_f32() as f64
                && self.0.y == v.0.y.to_f32() as f64
                && self.0.z == v.0.z.to_f32() as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3i>>() {
            return self.0.x == v.0.x as f64
                && self.0.y == v.0.y as f64
                && self.0.z == v.0.z as f64;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash3(self.0.x.to_bits(), self.0.y.to_bits(), self.0.z.to_bits())
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec3d::new(-self.0.x, -self.0.y, -self.0.z))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let vals: Vec<f64> = vec![slf.0.x, slf.0.y, slf.0.z];
        pyo3::types::PyList::new(slf.py(), vals)
            .map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec3d to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f64> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 3)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f64> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f64 = val.extract()?;
        *elems[norm_idx(i, 3)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f64) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v
    }
    fn __xor__(&self, o: &Self) -> Self {
        Self(self.0.cross(&o.0))
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3d::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3d::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
    }
    /// v * scalar → scale, v * vec → dot product
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3d>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(
                Self(usd_gf::Vec3d::new(self.0.x * s, self.0.y * s, self.0.z * s))
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f64) -> Self {
        Self(usd_gf::Vec3d::new(self.0.x * s, self.0.y * s, self.0.z * s))
    }
    fn __imul__(&mut self, s: f64) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec3d::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
        )))
    }
    fn __itruediv__(&mut self, s: f64) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec3d::new(1.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec3d::new(0.0, 1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec3d::new(0.0, 0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f64 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec3d::new(self.0.x / l, self.0.y / l, self.0.z / l))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f64 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
            self.0.z /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f64 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetCross")]
    fn get_cross(&self, o: &Self) -> Self {
        Self(self.0.cross(&o.0))
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec3d::new(0.0, 0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec3d::new(onto.0.x * s, onto.0.y * s, onto.0.z * s))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec3d::new(
            self.0.x - p.0.x,
            self.0.y - p.0.y,
            self.0.z - p.0.z,
        ))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 3;
    #[getter]
    fn dimension(&self) -> usize {
        3
    }
}

// ---------------------------------------------------------------------------
// Vec3f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec3f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec3f(pub usd_gf::Vec3f);

#[pymethods]
impl PyVec3f {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec3f::new(0.0, 0.0, 0.0)));
        }
        if n == 3 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            let z: f32 = args.get_item(2)?.extract()?;
            return Ok(Self(usd_gf::Vec3f::new(x, y, z)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3f>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3d>>() {
                return Ok(Self(usd_gf::Vec3f::new(
                    v.0.x as f32,
                    v.0.y as f32,
                    v.0.z as f32,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3h>>() {
                return Ok(Self(usd_gf::Vec3f::new(
                    v.0.x.to_f32(),
                    v.0.y.to_f32(),
                    v.0.z.to_f32(),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3i>>() {
                return Ok(Self(usd_gf::Vec3f::new(
                    v.0.x as f32,
                    v.0.y as f32,
                    v.0.z as f32,
                )));
            }
            if let Ok(seq) = obj.extract::<Vec<f32>>() {
                if seq.len() == 3 {
                    return Ok(Self(usd_gf::Vec3f::new(seq[0], seq[1], seq[2])));
                }
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec3f::new(s, s, s)));
            }
        }
        Err(PyValueError::new_err("Vec3f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec3f({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __len__(&self) -> usize {
        3
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3f>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3h>>() {
            return self.0.x == v.0.x.to_f32()
                && self.0.y == v.0.y.to_f32()
                && self.0.z == v.0.z.to_f32();
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3i>>() {
            return self.0.x == v.0.x as f32
                && self.0.y == v.0.y as f32
                && self.0.z == v.0.z as f32;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash3(
            self.0.x.to_bits() as u64,
            self.0.y.to_bits() as u64,
            self.0.z.to_bits() as u64,
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec3f::new(-self.0.x, -self.0.y, -self.0.z))
    }
    fn __xor__(&self, o: &Self) -> Self {
        Self(self.0.cross(&o.0))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let vals: Vec<f32> = vec![slf.0.x, slf.0.y, slf.0.z];
        pyo3::types::PyList::new(slf.py(), vals)
            .map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec3f to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 3)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        *elems[norm_idx(i, 3)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f32) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3f::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3f::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3f>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(
                Self(usd_gf::Vec3f::new(self.0.x * s, self.0.y * s, self.0.z * s))
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f32) -> Self {
        Self(usd_gf::Vec3f::new(self.0.x * s, self.0.y * s, self.0.z * s))
    }
    fn __imul__(&mut self, s: f32) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec3f::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
        )))
    }
    fn __itruediv__(&mut self, s: f32) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec3f::new(1.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec3f::new(0.0, 1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec3f::new(0.0, 0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f32 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec3f::new(self.0.x / l, self.0.y / l, self.0.z / l))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f32 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
            self.0.z /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f32 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetCross")]
    fn get_cross(&self, o: &Self) -> Self {
        Self(self.0.cross(&o.0))
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec3f::new(0.0, 0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec3f::new(onto.0.x * s, onto.0.y * s, onto.0.z * s))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec3f::new(
            self.0.x - p.0.x,
            self.0.y - p.0.y,
            self.0.z - p.0.z,
        ))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 3;
    #[getter]
    fn dimension(&self) -> usize {
        3
    }
}

// ---------------------------------------------------------------------------
// Vec3h
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec3h", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec3h(pub usd_gf::Vec3h);

#[pymethods]
impl PyVec3h {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec3h::new(
                Half::from_f32(0.0),
                Half::from_f32(0.0),
                Half::from_f32(0.0),
            )));
        }
        if n == 3 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            let z: f32 = args.get_item(2)?.extract()?;
            return Ok(Self(usd_gf::Vec3h::new(
                Half::from_f32(x),
                Half::from_f32(y),
                Half::from_f32(z),
            )));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3h>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3d>>() {
                return Ok(Self(usd_gf::Vec3h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                    Half::from_f32(v.0.z as f32),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3f>>() {
                return Ok(Self(usd_gf::Vec3h::new(
                    Half::from_f32(v.0.x),
                    Half::from_f32(v.0.y),
                    Half::from_f32(v.0.z),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3i>>() {
                return Ok(Self(usd_gf::Vec3h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                    Half::from_f32(v.0.z as f32),
                )));
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec3h::new(
                    Half::from_f32(s),
                    Half::from_f32(s),
                    Half::from_f32(s),
                )));
            }
        }
        Err(PyValueError::new_err("Vec3h: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!(
            "Gf.Vec3h({}, {}, {})",
            self.0.x.to_f32(),
            self.0.y.to_f32(),
            self.0.z.to_f32()
        )
    }
    fn __str__(&self) -> String {
        format!(
            "({}, {}, {})",
            self.0.x.to_f32(),
            self.0.y.to_f32(),
            self.0.z.to_f32()
        )
    }
    fn __len__(&self) -> usize {
        3
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        hash3(
            self.0.x.bits() as u64,
            self.0.y.bits() as u64,
            self.0.z.bits() as u64,
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec3h::new(-self.0.x, -self.0.y, -self.0.z))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f32> = vec![slf.0.x.to_f32(), slf.0.y.to_f32(), slf.0.z.to_f32()];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec3h to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x.to_f32(), self.0.y.to_f32(), self.0.z.to_f32()];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 3)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z];
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = Half::from_f32(vals[idx]);
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        match norm_idx(i, 3)? {
            0 => self.0.x = Half::from_f32(v),
            1 => self.0.y = Half::from_f32(v),
            2 => self.0.z = Half::from_f32(v),
            _ => unreachable!(),
        }
        Ok(())
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3h::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
        ))
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3h::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
        ))
    }
    fn __mul__(&self, s: f32) -> Self {
        let hs = Half::from_f32(s);
        Self(usd_gf::Vec3h::new(
            self.0.x * hs,
            self.0.y * hs,
            self.0.z * hs,
        ))
    }
    fn __rmul__(&self, s: f32) -> Self {
        self.__mul__(s)
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        let hs = Half::from_f32(s);
        Ok(Self(usd_gf::Vec3h::new(
            self.0.x / hs,
            self.0.y / hs,
            self.0.z / hs,
        )))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 3;
    #[getter]
    fn dimension(&self) -> usize {
        3
    }
}

// ---------------------------------------------------------------------------
// Vec3i
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec3i", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec3i(pub usd_gf::Vec3i);

#[pymethods]
impl PyVec3i {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec3i::new(0, 0, 0)));
        }
        if n == 3 {
            let x: i32 = args.get_item(0)?.extract()?;
            let y: i32 = args.get_item(1)?.extract()?;
            let z: i32 = args.get_item(2)?.extract()?;
            return Ok(Self(usd_gf::Vec3i::new(x, y, z)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec3i>>() {
                return Ok(Self(v.0));
            }
            if let Ok(seq) = obj.extract::<Vec<i32>>() {
                if seq.len() == 3 {
                    return Ok(Self(usd_gf::Vec3i::new(seq[0], seq[1], seq[2])));
                }
            }
            if let Ok(s) = obj.extract::<i32>() {
                return Ok(Self(usd_gf::Vec3i::new(s, s, s)));
            }
        }
        Err(PyValueError::new_err("Vec3i: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec3i({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
    fn __len__(&self) -> usize {
        3
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        hash3(self.0.x as u64, self.0.y as u64, self.0.z as u64)
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec3i::new(-self.0.x, -self.0.y, -self.0.z))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let vals: Vec<i32> = vec![slf.0.x, slf.0.y, slf.0.z];
        pyo3::types::PyList::new(slf.py(), vals)
            .map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec3i to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z];
        if let Some(indices) = resolve_slice(py, key, 3)? {
            let vals: Vec<i32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 3)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(&mut self, i: isize, v: i32) -> PyResult<()> {
        match norm_idx(i, 3)? {
            0 => self.0.x = v,
            1 => self.0.y = v,
            2 => self.0.z = v,
            _ => unreachable!(),
        }
        Ok(())
    }
    fn __contains__(&self, v: i32) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3i::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec3i::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec3i>>() {
            let d = self.0.x * v.0.x + self.0.y * v.0.y + self.0.z * v.0.z;
            return Ok(d.into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<i32>() {
            return Ok(
                Self(usd_gf::Vec3i::new(self.0.x * s, self.0.y * s, self.0.z * s))
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: i32) -> Self {
        Self(usd_gf::Vec3i::new(self.0.x * s, self.0.y * s, self.0.z * s))
    }
    fn __imul__(&mut self, s: i32) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
    }
    fn __truediv__(&self, s: i32) -> PyResult<Self> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec3i::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
        )))
    }
    fn __itruediv__(&mut self, s: i32) -> PyResult<()> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec3i::new(1, 0, 0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec3i::new(0, 1, 0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec3i::new(0, 0, 1))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 3;
    #[getter]
    fn dimension(&self) -> usize {
        3
    }
}

// ---------------------------------------------------------------------------
// Vec4d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec4d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec4d(pub usd_gf::Vec4d);

#[pymethods]
impl PyVec4d {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec4d::new(0.0, 0.0, 0.0, 0.0)));
        }
        if n == 4 {
            let x: f64 = args.get_item(0)?.extract()?;
            let y: f64 = args.get_item(1)?.extract()?;
            let z: f64 = args.get_item(2)?.extract()?;
            let w: f64 = args.get_item(3)?.extract()?;
            return Ok(Self(usd_gf::Vec4d::new(x, y, z, w)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4d>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4f>>() {
                return Ok(Self(usd_gf::Vec4d::new(
                    v.0.x as f64,
                    v.0.y as f64,
                    v.0.z as f64,
                    v.0.w as f64,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4i>>() {
                return Ok(Self(usd_gf::Vec4d::new(
                    v.0.x as f64,
                    v.0.y as f64,
                    v.0.z as f64,
                    v.0.w as f64,
                )));
            }
            if let Ok(seq) = obj.extract::<Vec<f64>>() {
                if seq.len() == 4 {
                    return Ok(Self(usd_gf::Vec4d::new(seq[0], seq[1], seq[2], seq[3])));
                }
            }
            if let Ok(s) = obj.extract::<f64>() {
                return Ok(Self(usd_gf::Vec4d::new(s, s, s, s)));
            }
        }
        Err(PyValueError::new_err("Vec4d: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!(
            "Gf.Vec4d({}, {}, {}, {})",
            self.0.x, self.0.y, self.0.z, self.0.w
        )
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {}, {})", self.0.x, self.0.y, self.0.z, self.0.w)
    }
    fn __len__(&self) -> usize {
        4
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4d>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4f>>() {
            return self.0.x == v.0.x as f64
                && self.0.y == v.0.y as f64
                && self.0.z == v.0.z as f64
                && self.0.w == v.0.w as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4h>>() {
            return self.0.x == v.0.x.to_f32() as f64
                && self.0.y == v.0.y.to_f32() as f64
                && self.0.z == v.0.z.to_f32() as f64
                && self.0.w == v.0.w.to_f32() as f64;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4i>>() {
            return self.0.x == v.0.x as f64
                && self.0.y == v.0.y as f64
                && self.0.z == v.0.z as f64
                && self.0.w == v.0.w as f64;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash4(
            self.0.x.to_bits(),
            self.0.y.to_bits(),
            self.0.z.to_bits(),
            self.0.w.to_bits(),
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec4d::new(
            -self.0.x, -self.0.y, -self.0.z, -self.0.w,
        ))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f64> = vec![slf.0.x, slf.0.y, slf.0.z, slf.0.w];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec4d to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z, self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f64> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 4)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z, &mut self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f64> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f64 = val.extract()?;
        *elems[norm_idx(i, 4)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f64) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v || self.0.w == v
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4d::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
            self.0.w + o.0.w,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
        self.0.w += o.0.w;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4d::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
            self.0.w - o.0.w,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
        self.0.w -= o.0.w;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4d>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(usd_gf::Vec4d::new(
                self.0.x * s,
                self.0.y * s,
                self.0.z * s,
                self.0.w * s,
            ))
            .into_pyobject(py)?
            .into_any()
            .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f64) -> Self {
        Self(usd_gf::Vec4d::new(
            self.0.x * s,
            self.0.y * s,
            self.0.z * s,
            self.0.w * s,
        ))
    }
    fn __imul__(&mut self, s: f64) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
        self.0.w *= s;
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec4d::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
            self.0.w / s,
        )))
    }
    fn __itruediv__(&mut self, s: f64) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        self.0.w /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec4d::new(1.0, 0.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec4d::new(0.0, 1.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec4d::new(0.0, 0.0, 1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "WAxis")]
    fn w_axis() -> Self {
        Self(usd_gf::Vec4d::new(0.0, 0.0, 0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            3 => Ok(Self::w_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f64 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec4d::new(
                self.0.x / l,
                self.0.y / l,
                self.0.z / l,
                self.0.w / l,
            ))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f64 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
            self.0.z /= l;
            self.0.w /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f64 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec4d::new(0.0, 0.0, 0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec4d::new(
            onto.0.x * s,
            onto.0.y * s,
            onto.0.z * s,
            onto.0.w * s,
        ))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec4d::new(
            self.0.x - p.0.x,
            self.0.y - p.0.y,
            self.0.z - p.0.z,
            self.0.w - p.0.w,
        ))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 4;
    #[getter]
    fn dimension(&self) -> usize {
        4
    }
}

// ---------------------------------------------------------------------------
// Vec4f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec4f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec4f(pub usd_gf::Vec4f);

#[pymethods]
impl PyVec4f {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec4f::new(0.0, 0.0, 0.0, 0.0)));
        }
        if n == 4 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            let z: f32 = args.get_item(2)?.extract()?;
            let w: f32 = args.get_item(3)?.extract()?;
            return Ok(Self(usd_gf::Vec4f::new(x, y, z, w)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4f>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4d>>() {
                return Ok(Self(usd_gf::Vec4f::new(
                    v.0.x as f32,
                    v.0.y as f32,
                    v.0.z as f32,
                    v.0.w as f32,
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4i>>() {
                return Ok(Self(usd_gf::Vec4f::new(
                    v.0.x as f32,
                    v.0.y as f32,
                    v.0.z as f32,
                    v.0.w as f32,
                )));
            }
            if let Ok(seq) = obj.extract::<Vec<f32>>() {
                if seq.len() == 4 {
                    return Ok(Self(usd_gf::Vec4f::new(seq[0], seq[1], seq[2], seq[3])));
                }
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec4f::new(s, s, s, s)));
            }
        }
        Err(PyValueError::new_err("Vec4f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!(
            "Gf.Vec4f({}, {}, {}, {})",
            self.0.x, self.0.y, self.0.z, self.0.w
        )
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {}, {})", self.0.x, self.0.y, self.0.z, self.0.w)
    }
    fn __len__(&self) -> usize {
        4
    }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4f>>() {
            return self.0 == v.0;
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4h>>() {
            return self.0.x == v.0.x.to_f32()
                && self.0.y == v.0.y.to_f32()
                && self.0.z == v.0.z.to_f32()
                && self.0.w == v.0.w.to_f32();
        }
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4i>>() {
            return self.0.x == v.0.x as f32
                && self.0.y == v.0.y as f32
                && self.0.z == v.0.z as f32
                && self.0.w == v.0.w as f32;
        }
        let _ = py;
        false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        !self.__eq__(py, o)
    }
    fn __hash__(&self) -> u64 {
        hash4(
            self.0.x.to_bits() as u64,
            self.0.y.to_bits() as u64,
            self.0.z.to_bits() as u64,
            self.0.w.to_bits() as u64,
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec4f::new(
            -self.0.x, -self.0.y, -self.0.z, -self.0.w,
        ))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f32> = vec![slf.0.x, slf.0.y, slf.0.z, slf.0.w];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec4f to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z, self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 4)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z, &mut self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        *elems[norm_idx(i, 4)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: f32) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v || self.0.w == v
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4f::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
            self.0.w + o.0.w,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
        self.0.w += o.0.w;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4f::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
            self.0.w - o.0.w,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
        self.0.w -= o.0.w;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4f>>() {
            return Ok(self.0.dot(&v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(Self(usd_gf::Vec4f::new(
                self.0.x * s,
                self.0.y * s,
                self.0.z * s,
                self.0.w * s,
            ))
            .into_pyobject(py)?
            .into_any()
            .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: f32) -> Self {
        Self(usd_gf::Vec4f::new(
            self.0.x * s,
            self.0.y * s,
            self.0.z * s,
            self.0.w * s,
        ))
    }
    fn __imul__(&mut self, s: f32) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
        self.0.w *= s;
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec4f::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
            self.0.w / s,
        )))
    }
    fn __itruediv__(&mut self, s: f32) -> PyResult<()> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        self.0.w /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec4f::new(1.0, 0.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec4f::new(0.0, 1.0, 0.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec4f::new(0.0, 0.0, 1.0, 0.0))
    }
    #[staticmethod]
    #[pyo3(name = "WAxis")]
    fn w_axis() -> Self {
        Self(usd_gf::Vec4f::new(0.0, 0.0, 0.0, 1.0))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            3 => Ok(Self::w_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[pyo3(name = "GetLength")]
    fn get_length(&self) -> f32 {
        self.0.length()
    }
    #[pyo3(name = "GetNormalized")]
    fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 {
            Self(usd_gf::Vec4f::new(
                self.0.x / l,
                self.0.y / l,
                self.0.z / l,
                self.0.w / l,
            ))
        } else {
            self.clone()
        }
    }
    #[pyo3(name = "Normalize")]
    fn normalize(&mut self) -> f32 {
        let l = self.0.length();
        if l > 0.0 {
            self.0.x /= l;
            self.0.y /= l;
            self.0.z /= l;
            self.0.w /= l;
        }
        l
    }
    #[pyo3(name = "GetDot")]
    fn get_dot(&self, o: &Self) -> f32 {
        self.0.dot(&o.0)
    }
    #[pyo3(name = "GetProjection")]
    fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 {
            return Self(usd_gf::Vec4f::new(0.0, 0.0, 0.0, 0.0));
        }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec4f::new(
            onto.0.x * s,
            onto.0.y * s,
            onto.0.z * s,
            onto.0.w * s,
        ))
    }
    #[pyo3(name = "GetComplement")]
    fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec4f::new(
            self.0.x - p.0.x,
            self.0.y - p.0.y,
            self.0.z - p.0.z,
            self.0.w - p.0.w,
        ))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 4;
    #[getter]
    fn dimension(&self) -> usize {
        4
    }
}

// ---------------------------------------------------------------------------
// Vec4h
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec4h", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec4h(pub usd_gf::Vec4h);

#[pymethods]
impl PyVec4h {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec4h::new(
                Half::from_f32(0.0),
                Half::from_f32(0.0),
                Half::from_f32(0.0),
                Half::from_f32(0.0),
            )));
        }
        if n == 4 {
            let x: f32 = args.get_item(0)?.extract()?;
            let y: f32 = args.get_item(1)?.extract()?;
            let z: f32 = args.get_item(2)?.extract()?;
            let w: f32 = args.get_item(3)?.extract()?;
            return Ok(Self(usd_gf::Vec4h::new(
                Half::from_f32(x),
                Half::from_f32(y),
                Half::from_f32(z),
                Half::from_f32(w),
            )));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4h>>() {
                return Ok(Self(v.0));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4d>>() {
                return Ok(Self(usd_gf::Vec4h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                    Half::from_f32(v.0.z as f32),
                    Half::from_f32(v.0.w as f32),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4f>>() {
                return Ok(Self(usd_gf::Vec4h::new(
                    Half::from_f32(v.0.x),
                    Half::from_f32(v.0.y),
                    Half::from_f32(v.0.z),
                    Half::from_f32(v.0.w),
                )));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4i>>() {
                return Ok(Self(usd_gf::Vec4h::new(
                    Half::from_f32(v.0.x as f32),
                    Half::from_f32(v.0.y as f32),
                    Half::from_f32(v.0.z as f32),
                    Half::from_f32(v.0.w as f32),
                )));
            }
            if let Ok(s) = obj.extract::<f32>() {
                return Ok(Self(usd_gf::Vec4h::new(
                    Half::from_f32(s),
                    Half::from_f32(s),
                    Half::from_f32(s),
                    Half::from_f32(s),
                )));
            }
        }
        Err(PyValueError::new_err("Vec4h: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!(
            "Gf.Vec4h({}, {}, {}, {})",
            self.0.x.to_f32(),
            self.0.y.to_f32(),
            self.0.z.to_f32(),
            self.0.w.to_f32()
        )
    }
    fn __str__(&self) -> String {
        format!(
            "({}, {}, {}, {})",
            self.0.x.to_f32(),
            self.0.y.to_f32(),
            self.0.z.to_f32(),
            self.0.w.to_f32()
        )
    }
    fn __len__(&self) -> usize {
        4
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        hash4(
            self.0.x.bits() as u64,
            self.0.y.bits() as u64,
            self.0.z.bits() as u64,
            self.0.w.bits() as u64,
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec4h::new(
            -self.0.x, -self.0.y, -self.0.z, -self.0.w,
        ))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<f32> = vec![
            slf.0.x.to_f32(),
            slf.0.y.to_f32(),
            slf.0.z.to_f32(),
            slf.0.w.to_f32(),
        ];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec4h to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [
            self.0.x.to_f32(),
            self.0.y.to_f32(),
            self.0.z.to_f32(),
            self.0.w.to_f32(),
        ];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 4)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<f32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z, &mut self.0.w];
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = Half::from_f32(vals[idx]);
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: f32 = val.extract()?;
        match norm_idx(i, 4)? {
            0 => self.0.x = Half::from_f32(v),
            1 => self.0.y = Half::from_f32(v),
            2 => self.0.z = Half::from_f32(v),
            3 => self.0.w = Half::from_f32(v),
            _ => unreachable!(),
        }
        Ok(())
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4h::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
            self.0.w + o.0.w,
        ))
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4h::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
            self.0.w - o.0.w,
        ))
    }
    fn __mul__(&self, s: f32) -> Self {
        let hs = Half::from_f32(s);
        Self(usd_gf::Vec4h::new(
            self.0.x * hs,
            self.0.y * hs,
            self.0.z * hs,
            self.0.w * hs,
        ))
    }
    fn __rmul__(&self, s: f32) -> Self {
        self.__mul__(s)
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        let hs = Half::from_f32(s);
        Ok(Self(usd_gf::Vec4h::new(
            self.0.x / hs,
            self.0.y / hs,
            self.0.z / hs,
            self.0.w / hs,
        )))
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 4;
    #[getter]
    fn dimension(&self) -> usize {
        4
    }
}

// ---------------------------------------------------------------------------
// Vec4i
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object, name = "Vec4i", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyVec4i(pub usd_gf::Vec4i);

#[pymethods]
impl PyVec4i {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 {
            return Ok(Self(usd_gf::Vec4i::new(0, 0, 0, 0)));
        }
        if n == 4 {
            let x: i32 = args.get_item(0)?.extract()?;
            let y: i32 = args.get_item(1)?.extract()?;
            let z: i32 = args.get_item(2)?.extract()?;
            let w: i32 = args.get_item(3)?.extract()?;
            return Ok(Self(usd_gf::Vec4i::new(x, y, z, w)));
        }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<PyRef<'_, PyVec4i>>() {
                return Ok(Self(v.0));
            }
            if let Ok(seq) = obj.extract::<Vec<i32>>() {
                if seq.len() == 4 {
                    return Ok(Self(usd_gf::Vec4i::new(seq[0], seq[1], seq[2], seq[3])));
                }
            }
            if let Ok(s) = obj.extract::<i32>() {
                return Ok(Self(usd_gf::Vec4i::new(s, s, s, s)));
            }
        }
        Err(PyValueError::new_err("Vec4i: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!(
            "Gf.Vec4i({}, {}, {}, {})",
            self.0.x, self.0.y, self.0.z, self.0.w
        )
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {}, {})", self.0.x, self.0.y, self.0.z, self.0.w)
    }
    fn __len__(&self) -> usize {
        4
    }
    fn __eq__(&self, o: &Self) -> bool {
        self.0 == o.0
    }
    fn __ne__(&self, o: &Self) -> bool {
        self.0 != o.0
    }
    fn __hash__(&self) -> u64 {
        hash4(
            self.0.x as u64,
            self.0.y as u64,
            self.0.z as u64,
            self.0.w as u64,
        )
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec4i::new(
            -self.0.x, -self.0.y, -self.0.z, -self.0.w,
        ))
    }
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<pyo3::PyAny>> {
        let v: Vec<i32> = vec![slf.0.x, slf.0.y, slf.0.z, slf.0.w];
        pyo3::types::PyList::new(slf.py(), v).map(|l| l.call_method0("__iter__").unwrap().unbind())
    }
    fn __int__(&self) -> PyResult<i64> {
        Err(PyValueError::new_err("cannot convert Vec4i to int"))
    }

    fn __getitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<Py<pyo3::PyAny>> {
        let elems = [self.0.x, self.0.y, self.0.z, self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<i32> = indices.iter().map(|&i| elems[i]).collect();
            return Ok(pyo3::types::PyList::new(py, vals)?.into_any().unbind());
        }
        let i: isize = key.extract()?;
        Ok(elems[norm_idx(i, 4)?]
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }
    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, pyo3::PyAny>,
        val: &Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        let elems = [&mut self.0.x, &mut self.0.y, &mut self.0.z, &mut self.0.w];
        if let Some(indices) = resolve_slice(py, key, 4)? {
            let vals: Vec<i32> = val.extract()?;
            check_slice_len(&vals, &indices)?;
            for (idx, &i) in indices.iter().enumerate() {
                *elems[i] = vals[idx];
            }
            return Ok(());
        }
        let i: isize = key.extract()?;
        let v: i32 = val.extract()?;
        *elems[norm_idx(i, 4)?] = v;
        Ok(())
    }
    fn __contains__(&self, v: i32) -> bool {
        self.0.x == v || self.0.y == v || self.0.z == v || self.0.w == v
    }
    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4i::new(
            self.0.x + o.0.x,
            self.0.y + o.0.y,
            self.0.z + o.0.z,
            self.0.w + o.0.w,
        ))
    }
    fn __iadd__(&mut self, o: &Self) {
        self.0.x += o.0.x;
        self.0.y += o.0.y;
        self.0.z += o.0.z;
        self.0.w += o.0.w;
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec4i::new(
            self.0.x - o.0.x,
            self.0.y - o.0.y,
            self.0.z - o.0.z,
            self.0.w - o.0.w,
        ))
    }
    fn __isub__(&mut self, o: &Self) {
        self.0.x -= o.0.x;
        self.0.y -= o.0.y;
        self.0.z -= o.0.z;
        self.0.w -= o.0.w;
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(v) = o.extract::<PyRef<'_, PyVec4i>>() {
            return Ok(
                (self.0.x * v.0.x + self.0.y * v.0.y + self.0.z * v.0.z + self.0.w * v.0.w)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
        }
        if let Ok(s) = o.extract::<i32>() {
            return Ok(Self(usd_gf::Vec4i::new(
                self.0.x * s,
                self.0.y * s,
                self.0.z * s,
                self.0.w * s,
            ))
            .into_pyobject(py)?
            .into_any()
            .unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type for *",
        ))
    }
    fn __rmul__(&self, s: i32) -> Self {
        Self(usd_gf::Vec4i::new(
            self.0.x * s,
            self.0.y * s,
            self.0.z * s,
            self.0.w * s,
        ))
    }
    fn __imul__(&mut self, s: i32) {
        self.0.x *= s;
        self.0.y *= s;
        self.0.z *= s;
        self.0.w *= s;
    }
    fn __truediv__(&self, s: i32) -> PyResult<Self> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        Ok(Self(usd_gf::Vec4i::new(
            self.0.x / s,
            self.0.y / s,
            self.0.z / s,
            self.0.w / s,
        )))
    }
    fn __itruediv__(&mut self, s: i32) -> PyResult<()> {
        if s == 0 {
            return Err(PyZeroDivisionError::new_err("division by zero"));
        }
        self.0.x /= s;
        self.0.y /= s;
        self.0.z /= s;
        self.0.w /= s;
        Ok(())
    }

    #[staticmethod]
    #[pyo3(name = "XAxis")]
    fn x_axis() -> Self {
        Self(usd_gf::Vec4i::new(1, 0, 0, 0))
    }
    #[staticmethod]
    #[pyo3(name = "YAxis")]
    fn y_axis() -> Self {
        Self(usd_gf::Vec4i::new(0, 1, 0, 0))
    }
    #[staticmethod]
    #[pyo3(name = "ZAxis")]
    fn z_axis() -> Self {
        Self(usd_gf::Vec4i::new(0, 0, 1, 0))
    }
    #[staticmethod]
    #[pyo3(name = "WAxis")]
    fn w_axis() -> Self {
        Self(usd_gf::Vec4i::new(0, 0, 0, 1))
    }
    #[staticmethod]
    #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i {
            0 => Ok(Self::x_axis()),
            1 => Ok(Self::y_axis()),
            2 => Ok(Self::z_axis()),
            3 => Ok(Self::w_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")),
        }
    }

    #[classattr]
    #[pyo3(name = "dimension")]
    const DIMENSION: usize = 4;
    #[getter]
    fn dimension(&self) -> usize {
        4
    }
}

// ---------------------------------------------------------------------------
// Internal hash helpers
// ---------------------------------------------------------------------------

fn hash2(a: u64, b: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    h.finish()
}

fn hash3(a: u64, b: u64, c: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    c.hash(&mut h);
    h.finish()
}

fn hash4(a: u64, b: u64, c: u64, d: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    c.hash(&mut h);
    d.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Vec2 family
    m.add_class::<PyVec2d>()?;
    m.add_class::<PyVec2f>()?;
    m.add_class::<PyVec2h>()?;
    m.add_class::<PyVec2i>()?;
    // Vec3 family
    m.add_class::<PyVec3d>()?;
    m.add_class::<PyVec3f>()?;
    m.add_class::<PyVec3h>()?;
    m.add_class::<PyVec3i>()?;
    // Vec4 family
    m.add_class::<PyVec4d>()?;
    m.add_class::<PyVec4f>()?;
    m.add_class::<PyVec4h>()?;
    m.add_class::<PyVec4i>()?;
    Ok(())
}
