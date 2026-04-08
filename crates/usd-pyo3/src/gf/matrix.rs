//! Matrix Python bindings (Matrix2d/f, Matrix3d/f, Matrix4d/f).
//!
//! All matrices are row-major, matching C++ GfMatrix layout.
//! Python indexing returns rows as lists, matching pxr.Gf behaviour.
//!
//! Note: `inverse()` returns `Option<Self>` — we fall back to identity on singular matrices,
//! matching the C++ behaviour of `GetInverse(double* det = NULL)`.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError, PyValueError, PyZeroDivisionError};
use usd_gf::{Matrix2d, Matrix2f, Matrix3d, Matrix3f, Matrix4d, Matrix4f};

// ---------------------------------------------------------------------------
// Row index helpers
// ---------------------------------------------------------------------------

fn idx2(i: isize) -> PyResult<usize> {
    let j = if i < 0 { 2isize + i } else { i };
    if j < 0 || j >= 2 { Err(PyIndexError::new_err("matrix row index out of range")) }
    else { Ok(j as usize) }
}
fn idx3(i: isize) -> PyResult<usize> {
    let j = if i < 0 { 3isize + i } else { i };
    if j < 0 || j >= 3 { Err(PyIndexError::new_err("matrix row index out of range")) }
    else { Ok(j as usize) }
}
fn idx4(i: isize) -> PyResult<usize> {
    let j = if i < 0 { 4isize + i } else { i };
    if j < 0 || j >= 4 { Err(PyIndexError::new_err("matrix row index out of range")) }
    else { Ok(j as usize) }
}

fn hash_f64_slice(data: &[f64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in data { v.to_bits().hash(&mut h); }
    h.finish()
}
fn hash_f32_slice(data: &[f32]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in data { v.to_bits().hash(&mut h); }
    h.finish()
}

// ---------------------------------------------------------------------------
// Matrix2d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix2d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix2d(pub Matrix2d);

#[pymethods]
impl PyMatrix2d {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (2, 2);

    /// Matrix2d(), Matrix2d(scalar), Matrix2d(Vec2d), Matrix2d(Matrix2d),
    /// Matrix2d(a00,a01,a10,a11)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix2d::from_diagonal(1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f64>() { return Ok(Self(Matrix2d::from_diagonal(v, v))); }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix2d>>() { return Ok(Self(m.0)); }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec2d>>() {
                return Ok(Self(Matrix2d::from_diagonal(v.0.x, v.0.y)));
            }
        }
        if n == 4 {
            let mut m = Matrix2d::identity();
            m[0][0] = args.get_item(0)?.extract()?;
            m[0][1] = args.get_item(1)?.extract()?;
            m[1][0] = args.get_item(2)?.extract()?;
            m[1][1] = args.get_item(3)?.extract()?;
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix2d: expected (), (scalar), (Vec2d), (Matrix2d), or (4 floats)"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix2d(({}, {}), ({}, {}))",
            self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2d>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2f>>() {
            for i in 0..2 { for j in 0..2 { if self.0[i][j] != m.0[i][j] as f64 { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx2(tup.0)?; let c = idx2(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx2(key.extract()?)?;
        Ok(super::vec::PyVec2d(usd_gf::Vec2d::new(self.0[r][0], self.0[r][1])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx2(tup.0)?; let c = idx2(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx2(key.extract()?)?;
        let row: Vec<f64> = val.extract()?;
        if row.len() != 2 { return Err(PyIndexError::new_err("expected 2 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    /// Matrix * Matrix or Matrix * scalar
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2d>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2d>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f64>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix2d::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[staticmethod]
    #[pyo3(name = "GetIdentity")]
    fn get_identity() -> Self { Self(Matrix2d::identity()) }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 4 { return Err(PyValueError::new_err("Set: expected 4 floats")); }
        self.0[0][0]=args.get_item(0)?.extract()?; self.0[0][1]=args.get_item(1)?.extract()?;
        self.0[1][0]=args.get_item(2)?.extract()?; self.0[1][1]=args.get_item(3)?.extract()?;
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2d::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f64>() {
            self.0.set_diagonal(v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec2d>>() {
            self.0.set_diagonal(v.0.x, v.0.y); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec2d"))
    }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix2d::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<super::vec::PyVec2d> {
        if i >= 2 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(super::vec::PyVec2d(usd_gf::Vec2d::new(self.0[i][0], self.0[i][1])))
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<super::vec::PyVec2d> {
        if j >= 2 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(super::vec::PyVec2d(usd_gf::Vec2d::new(self.0[0][j], self.0[1][j])))
    }
    #[pyo3(name = "SetRow")] fn set_row(&mut self, i: usize, v: &super::vec::PyVec2d) -> PyResult<()> {
        if i >= 2 { return Err(PyIndexError::new_err("row index out of range")); }
        self.0[i][0]=v.0.x; self.0[i][1]=v.0.y; Ok(())
    }
    #[pyo3(name = "SetColumn")] fn set_column(&mut self, j: usize, v: &super::vec::PyVec2d) -> PyResult<()> {
        if j >= 2 { return Err(PyIndexError::new_err("col index out of range")); }
        self.0[0][j]=v.0.x; self.0[1][j]=v.0.y; Ok(())
    }
}

// ---------------------------------------------------------------------------
// Matrix2f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix2f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix2f(pub Matrix2f);

#[pymethods]
impl PyMatrix2f {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (2, 2);

    /// Matrix2f(), Matrix2f(scalar), Matrix2f(Vec2f), Matrix2f(Matrix2f),
    /// Matrix2f(a00,a01,a10,a11)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix2f::from_diagonal(1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f32>() { return Ok(Self(Matrix2f::from_diagonal(v, v))); }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix2f>>() { return Ok(Self(m.0)); }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec2f>>() {
                return Ok(Self(Matrix2f::from_diagonal(v.0.x, v.0.y)));
            }
        }
        if n == 4 {
            let mut m = Matrix2f::identity();
            m[0][0] = args.get_item(0)?.extract()?;
            m[0][1] = args.get_item(1)?.extract()?;
            m[1][0] = args.get_item(2)?.extract()?;
            m[1][1] = args.get_item(3)?.extract()?;
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix2f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix2f(({}, {}), ({}, {}))",
            self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2f>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2d>>() {
            for i in 0..2 { for j in 0..2 { if (self.0[i][j] as f64) != m.0[i][j] { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx2(tup.0)?; let c = idx2(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx2(key.extract()?)?;
        Ok(super::vec::PyVec2f(usd_gf::Vec2f::new(self.0[r][0], self.0[r][1])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx2(tup.0)?; let c = idx2(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx2(key.extract()?)?;
        let row: Vec<f32> = val.extract()?;
        if row.len() != 2 { return Err(PyIndexError::new_err("expected 2 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2f>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix2f>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f32>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix2f::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 4 { return Err(PyValueError::new_err("Set: expected 4 floats")); }
        self.0[0][0]=args.get_item(0)?.extract()?; self.0[0][1]=args.get_item(1)?.extract()?;
        self.0[1][0]=args.get_item(2)?.extract()?; self.0[1][1]=args.get_item(3)?.extract()?;
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2f::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f32>() {
            self.0.set_diagonal(v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec2f>>() {
            self.0.set_diagonal(v.0.x, v.0.y); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec2f"))
    }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix2f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<super::vec::PyVec2f> {
        if i >= 2 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(super::vec::PyVec2f(usd_gf::Vec2f::new(self.0[i][0], self.0[i][1])))
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<super::vec::PyVec2f> {
        if j >= 2 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(super::vec::PyVec2f(usd_gf::Vec2f::new(self.0[0][j], self.0[1][j])))
    }
    #[pyo3(name = "SetRow")] fn set_row(&mut self, i: usize, v: &super::vec::PyVec2f) -> PyResult<()> {
        if i >= 2 { return Err(PyIndexError::new_err("row index out of range")); }
        self.0[i][0]=v.0.x; self.0[i][1]=v.0.y; Ok(())
    }
    #[pyo3(name = "SetColumn")] fn set_column(&mut self, j: usize, v: &super::vec::PyVec2f) -> PyResult<()> {
        if j >= 2 { return Err(PyIndexError::new_err("col index out of range")); }
        self.0[0][j]=v.0.x; self.0[1][j]=v.0.y; Ok(())
    }
}

// ---------------------------------------------------------------------------
// Matrix3d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix3d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix3d(pub Matrix3d);

#[pymethods]
impl PyMatrix3d {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (3, 3);

    /// Matrix3d(), Matrix3d(scalar), Matrix3d(Vec3d), Matrix3d(Matrix3d),
    /// Matrix3d(Rotation), Matrix3d(Quatd), Matrix3d(9 floats)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix3d::from_diagonal_values(1.0, 1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f64>() {
                return Ok(Self(Matrix3d::from_diagonal_values(v, v, v)));
            }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix3d>>() { return Ok(Self(m.0)); }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                return Ok(Self(Matrix3d::from_diagonal_values(v.0.x, v.0.y, v.0.z)));
            }
            if let Ok(r) = obj.extract::<PyRef<'_, super::geo::PyRotation>>() {
                return Ok(Self(r.0.get_matrix3()));
            }
            if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuatd>>() {
                let r = usd_gf::Rotation::from_quat(&q.0);
                return Ok(Self(r.get_matrix3()));
            }
        }
        if n == 9 {
            let mut m = Matrix3d::identity();
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = args.get_item(i * 3 + j)?.extract()?;
                }
            }
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix3d: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix3d(({},{},{}),({},{},{}),({},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3d>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3f>>() {
            for i in 0..3 { for j in 0..3 { if self.0[i][j] != m.0[i][j] as f64 { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx3(tup.0)?; let c = idx3(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx3(key.extract()?)?;
        Ok(super::vec::PyVec3d(usd_gf::Vec3d::new(self.0[r][0], self.0[r][1], self.0[r][2])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx3(tup.0)?; let c = idx3(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx3(key.extract()?)?;
        let row: Vec<f64> = val.extract()?;
        if row.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1]; self.0[r][2] = row[2];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    /// Matrix3d * Matrix3d or Matrix3d * scalar
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3d>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3d>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f64>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix3d::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 9 { return Err(PyValueError::new_err("Set: expected 9 floats")); }
        for i in 0..3 { for j in 0..3 { self.0[i][j] = args.get_item(i*3+j)?.extract()?; } }
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3d::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f64>() {
            self.0.set_diagonal(v, v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            self.0.set_diagonal(v.0.x, v.0.y, v.0.z); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec3d"))
    }
    #[pyo3(name = "Orthonormalize")] fn orthonormalize(&mut self) -> bool { self.0.orthonormalize() }
    #[pyo3(name = "GetOrthonormalized")] fn get_orthonormalized(&self) -> Self { Self(self.0.orthonormalized()) }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix3d::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }

    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<Vec<f64>> {
        if i >= 3 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(vec![self.0[i][0], self.0[i][1], self.0[i][2]])
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<Vec<f64>> {
        if j >= 3 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(vec![self.0[0][j], self.0[1][j], self.0[2][j]])
    }
    #[pyo3(name = "SetRow")] fn set_row(&mut self, i: usize, row: Vec<f64>) -> PyResult<()> {
        if i >= 3 { return Err(PyIndexError::new_err("row index out of range")); }
        if row.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[i][0]=row[0]; self.0[i][1]=row[1]; self.0[i][2]=row[2];
        Ok(())
    }
    #[pyo3(name = "SetColumn")] fn set_column(&mut self, j: usize, col: Vec<f64>) -> PyResult<()> {
        if j >= 3 { return Err(PyIndexError::new_err("col index out of range")); }
        if col.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[0][j]=col[0]; self.0[1][j]=col[1]; self.0[2][j]=col[2];
        Ok(())
    }

    #[pyo3(name = "ExtractRotation")]
    fn extract_rotation(&self) -> super::geo::PyRotation {
        super::geo::PyRotation(self.0.extract_rotation())
    }

    #[pyo3(name = "GetHandedness")]
    fn get_handedness(&self) -> f64 { self.0.handedness() }

    #[pyo3(name = "IsLeftHanded")]
    fn is_left_handed(&self) -> bool { self.0.is_left_handed() }

    #[pyo3(name = "IsRightHanded")]
    fn is_right_handed(&self) -> bool { !self.0.is_left_handed() }

    /// SetScale — accepts a scalar or Vec3d
    #[pyo3(name = "SetScale")]
    fn set_scale(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
            self.0.set_scale_nonuniform(&v.0);
            return Ok(self.clone());
        }
        if let Ok(f) = s.extract::<f64>() {
            self.0.set_scale_uniform(f);
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetScale: expected scalar or Vec3d"))
    }

    /// SetRotate — accepts Rotation, Quatd, or Matrix3d
    #[pyo3(name = "SetRotate")]
    fn set_rotate(&mut self, py: Python<'_>, rot: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(r) = rot.extract::<pyo3::PyRef<'_, super::geo::PyRotation>>() {
            let m = r.0.get_matrix3();
            self.0 = m;
            return Ok(self.clone());
        }
        if let Ok(q) = rot.extract::<pyo3::PyRef<'_, super::quat::PyQuatd>>() {
            self.0.set_rotate_quat(&q.0);
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetRotate: expected Rotation or Quatd"))
    }

    #[staticmethod]
    #[pyo3(name = "RotationMatrix")]
    fn rotation_matrix(axis: &super::vec::PyVec3d, angle_deg: f64) -> Self {
        let r = usd_gf::Rotation::from_axis_angle(axis.0, angle_deg);
        Self(r.get_matrix3())
    }
    #[staticmethod]
    #[pyo3(name = "ScaleMatrix")]
    fn scale_matrix(scale: f64) -> Self {
        Self(Matrix3d::from_diagonal_values(scale, scale, scale))
    }
}

// ---------------------------------------------------------------------------
// Matrix3f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix3f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix3f(pub Matrix3f);

#[pymethods]
impl PyMatrix3f {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (3, 3);

    /// Matrix3f(), Matrix3f(scalar), Matrix3f(Matrix3f), Matrix3f(Rotation), Matrix3f(Quatf), Matrix3f(9 floats)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix3f::from_diagonal_values(1.0, 1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f32>() { return Ok(Self(Matrix3f::from_diagonal_values(v, v, v))); }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix3f>>() { return Ok(Self(m.0)); }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec3f>>() {
                return Ok(Self(Matrix3f::from_diagonal_values(v.0.x, v.0.y, v.0.z)));
            }
            if let Ok(r) = obj.extract::<PyRef<'_, super::geo::PyRotation>>() {
                let m3d = r.0.get_matrix3();
                let mut m3f = Matrix3f::identity();
                for i in 0..3 { for j in 0..3 { m3f[i][j] = m3d[i][j] as f32; } }
                return Ok(Self(m3f));
            }
            if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuatf>>() {
                let qd = usd_gf::Quatd::new(q.0.real() as f64, usd_gf::Vec3d::new(q.0.imaginary().x as f64, q.0.imaginary().y as f64, q.0.imaginary().z as f64));
                let r = usd_gf::Rotation::from_quat(&qd);
                let m3d = r.get_matrix3();
                let mut m3f = Matrix3f::identity();
                for i in 0..3 { for j in 0..3 { m3f[i][j] = m3d[i][j] as f32; } }
                return Ok(Self(m3f));
            }
        }
        if n == 9 {
            let mut m = Matrix3f::identity();
            for i in 0..3 { for j in 0..3 { m[i][j] = args.get_item(i * 3 + j)?.extract()?; } }
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix3f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix3f(({},{},{}),({},{},{}),({},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3f>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3d>>() {
            for i in 0..3 { for j in 0..3 { if (self.0[i][j] as f64) != m.0[i][j] { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx3(tup.0)?; let c = idx3(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx3(key.extract()?)?;
        Ok(super::vec::PyVec3f(usd_gf::Vec3f::new(self.0[r][0], self.0[r][1], self.0[r][2])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx3(tup.0)?; let c = idx3(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx3(key.extract()?)?;
        let row: Vec<f32> = val.extract()?;
        if row.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3f>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix3f>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f32>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix3f::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 9 { return Err(PyValueError::new_err("Set: expected 9 floats")); }
        for i in 0..3 { for j in 0..3 { self.0[i][j] = args.get_item(i*3+j)?.extract()?; } }
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3f::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f32>() {
            self.0.set_diagonal(v, v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec3f>>() {
            self.0.set_diagonal(v.0.x, v.0.y, v.0.z); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec3f"))
    }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix3f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<super::vec::PyVec3f> {
        if i >= 3 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(super::vec::PyVec3f(usd_gf::Vec3f::new(self.0[i][0], self.0[i][1], self.0[i][2])))
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<super::vec::PyVec3f> {
        if j >= 3 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(super::vec::PyVec3f(usd_gf::Vec3f::new(self.0[0][j], self.0[1][j], self.0[2][j])))
    }
    #[pyo3(name = "SetRow")] fn set_row(&mut self, i: usize, v: &super::vec::PyVec3f) -> PyResult<()> {
        if i >= 3 { return Err(PyIndexError::new_err("row index out of range")); }
        self.0[i][0]=v.0.x; self.0[i][1]=v.0.y; self.0[i][2]=v.0.z; Ok(())
    }
    #[pyo3(name = "SetColumn")] fn set_column(&mut self, j: usize, v: &super::vec::PyVec3f) -> PyResult<()> {
        if j >= 3 { return Err(PyIndexError::new_err("col index out of range")); }
        self.0[0][j]=v.0.x; self.0[1][j]=v.0.y; self.0[2][j]=v.0.z; Ok(())
    }
    #[pyo3(name = "Orthonormalize")] fn orthonormalize(&mut self) -> bool { self.0.orthonormalize() }
    #[pyo3(name = "GetOrthonormalized")] fn get_orthonormalized(&self) -> Self { Self(self.0.orthonormalized()) }
    #[pyo3(name = "GetHandedness")] fn get_handedness(&self) -> f32 { self.0.handedness() }
    #[pyo3(name = "IsLeftHanded")] fn is_left_handed(&self) -> bool { self.0.is_left_handed() }
    #[pyo3(name = "IsRightHanded")] fn is_right_handed(&self) -> bool { !self.0.is_left_handed() }
    #[pyo3(name = "ExtractRotation")]
    fn extract_rotation(&self) -> super::geo::PyRotation {
        // Convert f32->f64 matrix for rotation extraction
        let mut m3d = Matrix3d::identity();
        for i in 0..3 { for j in 0..3 { m3d[i][j] = self.0[i][j] as f64; } }
        super::geo::PyRotation(m3d.extract_rotation())
    }
    /// SetScale — accepts scalar or Vec3f
    #[pyo3(name = "SetScale")]
    fn set_scale(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
            self.0 = Matrix3f::from_diagonal_values(v.0.x, v.0.y, v.0.z);
            return Ok(self.clone());
        }
        if let Ok(f) = s.extract::<f32>() {
            self.0 = Matrix3f::from_diagonal_values(f, f, f);
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetScale: expected scalar or Vec3f"))
    }
    /// SetRotate — accepts Rotation, Quatf
    #[pyo3(name = "SetRotate")]
    fn set_rotate(&mut self, py: Python<'_>, rot: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(r) = rot.extract::<pyo3::PyRef<'_, super::geo::PyRotation>>() {
            let m3d = r.0.get_matrix3();
            for i in 0..3 { for j in 0..3 { self.0[i][j] = m3d[i][j] as f32; } }
            return Ok(self.clone());
        }
        if let Ok(q) = rot.extract::<pyo3::PyRef<'_, super::quat::PyQuatf>>() {
            let qd = usd_gf::Quatd::new(q.0.real() as f64, usd_gf::Vec3d::new(q.0.imaginary().x as f64, q.0.imaginary().y as f64, q.0.imaginary().z as f64));
            let r = usd_gf::Rotation::from_quat(&qd);
            let m3d = r.get_matrix3();
            for i in 0..3 { for j in 0..3 { self.0[i][j] = m3d[i][j] as f32; } }
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetRotate: expected Rotation or Quatf"))
    }
}

// ---------------------------------------------------------------------------
// Matrix4d
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix4d", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix4d(pub Matrix4d);

#[pymethods]
impl PyMatrix4d {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (4, 4);

    /// Matrix4d(), Matrix4d(scalar), Matrix4d(Rotation), Matrix4d(Matrix3d, Vec3d),
    /// Matrix4d(16 floats)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix4d::from_diagonal_values(1.0, 1.0, 1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f64>() {
                return Ok(Self(Matrix4d::from_diagonal_values(v, v, v, v)));
            }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix4d>>() { return Ok(Self(m.0)); }
            if let Ok(r) = obj.extract::<PyRef<'_, super::geo::PyRotation>>() {
                return Ok(Self(r.0.get_matrix4()));
            }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec4d>>() {
                return Ok(Self(Matrix4d::from_diagonal_values(v.0.x, v.0.y, v.0.z, v.0.w)));
            }
        }
        if n == 2 {
            let a0 = args.get_item(0)?;
            let a1 = args.get_item(1)?;
            // Matrix4d(Rotation, Vec3d) — rotation + translation
            if let Ok(rot) = a0.extract::<PyRef<'_, super::geo::PyRotation>>() {
                let mut m4 = rot.0.get_matrix4();
                if let Ok(t) = a1.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                    m4[3][0] = t.0.x; m4[3][1] = t.0.y; m4[3][2] = t.0.z;
                }
                return Ok(Self(m4));
            }
            // Matrix4d(Matrix3d, Vec3d) — rotation + translation
            if let Ok(m3) = a0.extract::<PyRef<'_, PyMatrix3d>>() {
                let mut m4 = Matrix4d::identity();
                for i in 0..3 { for j in 0..3 { m4[i][j] = m3.0[i][j]; } }
                if let Ok(t) = a1.extract::<PyRef<'_, super::vec::PyVec3d>>() {
                    m4[3][0] = t.0.x; m4[3][1] = t.0.y; m4[3][2] = t.0.z;
                }
                return Ok(Self(m4));
            }
        }
        // Matrix4d(16 floats)
        if n == 16 {
            let mut m = Matrix4d::identity();
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] = args.get_item(i * 4 + j)?.extract()?;
                }
            }
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix4d: expected (), (scalar), (Rotation), (Matrix3d, Vec3d), or (16 floats)"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix4d(({},{},{},{}),({},{},{},{}),({},{},{},{}),({},{},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4d>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4f>>() {
            for i in 0..4 { for j in 0..4 { if self.0[i][j] != m.0[i][j] as f64 { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx4(tup.0)?; let c = idx4(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx4(key.extract()?)?;
        Ok(super::vec::PyVec4d(usd_gf::Vec4d::new(self.0[r][0], self.0[r][1], self.0[r][2], self.0[r][3])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx4(tup.0)?; let c = idx4(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx4(key.extract()?)?;
        // Accept Vec4d or list/tuple
        if let Ok(v) = val.extract::<PyRef<'_, super::vec::PyVec4d>>() {
            self.0[r][0]=v.0.x; self.0[r][1]=v.0.y; self.0[r][2]=v.0.z; self.0[r][3]=v.0.w;
            return Ok(());
        }
        let row: Vec<f64> = val.extract()?;
        if row.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2]; self.0[r][3]=row[3];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    /// Matrix4d * Matrix4d or Matrix4d * scalar
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4d>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        // Matrix4d * Vec4d -> Vec4d
        if let Ok(v) = o.extract::<PyRef<'_, super::vec::PyVec4d>>() {
            return Ok(super::vec::PyVec4d(self.0 * v.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4d>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f64>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix4d::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 16 { return Err(PyValueError::new_err("Set: expected 16 floats")); }
        for i in 0..4 { for j in 0..4 { self.0[i][j] = args.get_item(i*4+j)?.extract()?; } }
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4d::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f64>() {
            self.0.set_diagonal(v, v, v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec4d>>() {
            self.0.set_diagonal(v.0.x, v.0.y, v.0.z, v.0.w); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec4d"))
    }
    #[pyo3(name = "Orthonormalize")] fn orthonormalize(&mut self) -> bool { self.0.orthonormalize() }
    #[pyo3(name = "GetOrthonormalized")] fn get_orthonormalized(&self) -> Self { Self(self.0.orthonormalized()) }
    /// Factor(eps?) -> (success, scaleOrientation, scale, rotation, translation, projection)
    #[pyo3(name = "Factor")]
    #[pyo3(signature = (eps=None))]
    fn factor(&self, eps: Option<f64>) -> (bool, Self, super::vec::PyVec3d, Self, super::vec::PyVec3d, Self) {
        let _ = eps; // tolerance not used in current impl
        match self.0.factor() {
            Some((r, s, u, t, p)) => (true, Self(r), super::vec::PyVec3d(s), Self(u), super::vec::PyVec3d(t), Self(p)),
            None => (false, Self(Matrix4d::identity()), super::vec::PyVec3d(usd_gf::Vec3d::default()),
                     Self(Matrix4d::identity()), super::vec::PyVec3d(usd_gf::Vec3d::default()), Self(Matrix4d::identity())),
        }
    }
    /// SetLookAt(eye, center, up) -> Matrix4d
    #[pyo3(name = "SetLookAt")]
    fn set_look_at(&mut self, eye: &super::vec::PyVec3d, center: &super::vec::PyVec3d, up: &super::vec::PyVec3d) -> Self {
        self.0 = Matrix4d::look_at(&eye.0, &center.0, &up.0);
        self.clone()
    }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix4d::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "Invert")] fn invert(&mut self) {
        if let Some(inv) = self.0.inverse() { self.0 = inv; }
    }

    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<Vec<f64>> {
        if i >= 4 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(vec![self.0[i][0], self.0[i][1], self.0[i][2], self.0[i][3]])
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<Vec<f64>> {
        if j >= 4 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(vec![self.0[0][j], self.0[1][j], self.0[2][j], self.0[3][j]])
    }
    #[pyo3(name = "SetRow")] fn set_row(&mut self, i: usize, row: Vec<f64>) -> PyResult<()> {
        if i >= 4 { return Err(PyIndexError::new_err("row index out of range")); }
        if row.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[i][0]=row[0]; self.0[i][1]=row[1]; self.0[i][2]=row[2]; self.0[i][3]=row[3];
        Ok(())
    }
    #[pyo3(name = "SetColumn")] fn set_column(&mut self, j: usize, col: Vec<f64>) -> PyResult<()> {
        if j >= 4 { return Err(PyIndexError::new_err("col index out of range")); }
        if col.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[0][j]=col[0]; self.0[1][j]=col[1]; self.0[2][j]=col[2]; self.0[3][j]=col[3];
        Ok(())
    }

    #[pyo3(name = "ExtractTranslation")]
    fn extract_translation(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.extract_translation())
    }

    #[pyo3(name = "ExtractRotation")]
    fn extract_rotation(&self) -> super::geo::PyRotation {
        super::geo::PyRotation(self.0.extract_rotation())
    }

    #[pyo3(name = "RemoveScaleShear")]
    fn remove_scale_shear(&self) -> Self {
        Self(self.0.remove_scale_shear())
    }

    #[pyo3(name = "GetHandedness")]
    fn get_handedness(&self) -> f64 { self.0.handedness() }

    #[pyo3(name = "IsLeftHanded")]
    fn is_left_handed(&self) -> bool { self.0.is_left_handed() }

    #[pyo3(name = "IsRightHanded")]
    fn is_right_handed(&self) -> bool { !self.0.is_left_handed() }

    /// SetScale — accepts a scalar, Vec3d, or tuple
    #[pyo3(name = "SetScale")]
    fn set_scale(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
            self.0.set_scale_vec(&v.0);
            return Ok(self.clone());
        }
        if let Ok(f) = s.extract::<f64>() {
            self.0.set_scale(f);
            return Ok(self.clone());
        }
        // Accept tuple/list of 3 floats
        if let Ok(tup) = s.extract::<(f64, f64, f64)>() {
            self.0.set_scale_vec(&usd_gf::Vec3d::new(tup.0, tup.1, tup.2));
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetScale: expected scalar or Vec3d"))
    }

    /// SetRotate — accepts Rotation, Quatd, or Matrix3d
    #[pyo3(name = "SetRotate")]
    fn set_rotate(&mut self, py: Python<'_>, rot: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(r) = rot.extract::<pyo3::PyRef<'_, super::geo::PyRotation>>() {
            self.0.set_rotate_rotation(&r.0);
            return Ok(self.clone());
        }
        if let Ok(q) = rot.extract::<pyo3::PyRef<'_, super::quat::PyQuatd>>() {
            self.0.set_rotate(&q.0);
            return Ok(self.clone());
        }
        if let Ok(m) = rot.extract::<pyo3::PyRef<'_, PyMatrix3d>>() {
            self.0.set_rotate_matrix3(&m.0);
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetRotate: expected Rotation, Quatd, or Matrix3d"))
    }

    /// SetRotateOnly — set rotation, preserve translation
    #[pyo3(name = "SetRotateOnly")]
    fn set_rotate_only(&mut self, py: Python<'_>, rot: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        let t = self.0.extract_translation();
        if let Ok(r) = rot.extract::<pyo3::PyRef<'_, super::geo::PyRotation>>() {
            self.0.set_rotate_rotation(&r.0);
        } else if let Ok(q) = rot.extract::<pyo3::PyRef<'_, super::quat::PyQuatd>>() {
            self.0.set_rotate(&q.0);
        } else if let Ok(m) = rot.extract::<pyo3::PyRef<'_, PyMatrix3d>>() {
            self.0.set_rotate_matrix3(&m.0);
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err("SetRotateOnly: expected Rotation, Quatd, or Matrix3d"));
        }
        self.0[3][0] = t.x; self.0[3][1] = t.y; self.0[3][2] = t.z;
        let _ = py;
        Ok(self.clone())
    }

    /// SetTranslate(Vec3d | tuple) -> Matrix4d
    #[pyo3(name = "SetTranslate")]
    fn set_translate(&mut self, t: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = t.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            self.0.set_translate(&v.0);
        } else if let Ok((x, y, z)) = t.extract::<(f64, f64, f64)>() {
            self.0
                .set_translate(&usd_gf::Vec3d::new(x, y, z));
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "SetTranslate: expected Vec3d or 3-tuple of float",
            ));
        }
        Ok(self.clone())
    }

    /// SetTranslateOnly(Vec3d) -> Matrix4d
    #[pyo3(name = "SetTranslateOnly")]
    fn set_translate_only(&mut self, py: Python<'_>, t: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = t.extract::<PyRef<'_, super::vec::PyVec3d>>() {
            self.0.set_translate_only(&v.0);
        } else if let Ok(tup) = t.extract::<(f64, f64, f64)>() {
            self.0.set_translate_only(&usd_gf::Vec3d::new(tup.0, tup.1, tup.2));
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err("SetTranslateOnly: expected Vec3d or tuple"));
        }
        let _ = py;
        Ok(self.clone())
    }

    /// TransformDir(Vec3d) -> Vec3d: transform a direction (no translation)
    #[pyo3(name = "TransformDir")]
    fn transform_dir(&self, py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(vd) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
            let d = vd.0;
            // Transform direction: multiply by upper 3x3 only
            let r = usd_gf::Vec3d::new(
                d.x * self.0[0][0] + d.y * self.0[1][0] + d.z * self.0[2][0],
                d.x * self.0[0][1] + d.y * self.0[1][1] + d.z * self.0[2][1],
                d.x * self.0[0][2] + d.y * self.0[1][2] + d.z * self.0[2][2],
            );
            return Ok(super::vec::PyVec3d(r).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(vf) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
            let d = usd_gf::Vec3d::new(vf.0.x as f64, vf.0.y as f64, vf.0.z as f64);
            let r = usd_gf::Vec3d::new(
                d.x * self.0[0][0] + d.y * self.0[1][0] + d.z * self.0[2][0],
                d.x * self.0[0][1] + d.y * self.0[1][1] + d.z * self.0[2][1],
                d.x * self.0[0][2] + d.y * self.0[1][2] + d.z * self.0[2][2],
            );
            // Matrix4d always returns Vec3d (not Vec3f)
            return Ok(super::vec::PyVec3d(r).into_pyobject(py)?.into_any().unbind());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("TransformDir: expected Vec3d or Vec3f"))
    }

    /// TransformAffine(Vec3d) -> Vec3d: transform a point (with translation)
    #[pyo3(name = "TransformAffine")]
    fn transform_affine(&self, py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        let (dx, dy, dz) = if let Ok(vd) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
            (vd.0.x, vd.0.y, vd.0.z)
        } else if let Ok(vf) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
            (vf.0.x as f64, vf.0.y as f64, vf.0.z as f64)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err("TransformAffine: expected Vec3d or Vec3f"));
        };
        let m = &self.0;
        let r = usd_gf::Vec3d::new(
            dx*m[0][0] + dy*m[1][0] + dz*m[2][0] + m[3][0],
            dx*m[0][1] + dy*m[1][1] + dz*m[2][1] + m[3][1],
            dx*m[0][2] + dy*m[1][2] + dz*m[2][2] + m[3][2],
        );
        Ok(super::vec::PyVec3d(r).into_pyobject(py)?.into_any().unbind())
    }

    #[staticmethod]
    #[pyo3(name = "TranslationMatrix")]
    fn translation_matrix(t: &super::vec::PyVec3d) -> Self {
        Self(Matrix4d::from_translation(t.0))
    }
    #[staticmethod]
    #[pyo3(name = "ScaleMatrix")]
    fn scale_matrix(s: f64) -> Self {
        Self(Matrix4d::from_scale(s))
    }
    #[staticmethod]
    #[pyo3(name = "RotationMatrix")]
    fn rotation_matrix(axis: &super::vec::PyVec3d, angle_deg: f64) -> Self {
        let rot = usd_gf::Rotation::from_axis_angle(axis.0, angle_deg);
        Self(rot.get_matrix4())
    }

    /// Transform(point) -> Vec3d (full transform with perspective divide, always returns Vec3d)
    #[pyo3(name = "Transform")]
    fn transform(&self, py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        let (dx, dy, dz) = if let Ok(vd) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
            (vd.0.x, vd.0.y, vd.0.z)
        } else if let Ok(vf) = v.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
            (vf.0.x as f64, vf.0.y as f64, vf.0.z as f64)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err("Transform: expected Vec3d or Vec3f"));
        };
        let m = &self.0;
        let x = dx*m[0][0] + dy*m[1][0] + dz*m[2][0] + m[3][0];
        let y = dx*m[0][1] + dy*m[1][1] + dz*m[2][1] + m[3][1];
        let z = dx*m[0][2] + dy*m[1][2] + dz*m[2][2] + m[3][2];
        let w = dx*m[0][3] + dy*m[1][3] + dz*m[2][3] + m[3][3];
        let r = if w != 0.0 { usd_gf::Vec3d::new(x/w,y/w,z/w) } else { usd_gf::Vec3d::new(x,y,z) };
        Ok(super::vec::PyVec3d(r).into_pyobject(py)?.into_any().unbind())
    }
}

// ---------------------------------------------------------------------------
// Matrix4f
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Matrix4f", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyMatrix4f(pub Matrix4f);

#[pymethods]
impl PyMatrix4f {
    #[classattr] #[pyo3(name = "dimension")] const DIMENSION: (usize, usize) = (4, 4);

    /// Matrix4f(), Matrix4f(scalar), Matrix4f(Rotation), Matrix4f(Quatf), Matrix4f(16 floats)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Matrix4f::from_diagonal_values(1.0, 1.0, 1.0, 1.0))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(v) = obj.extract::<f32>() {
                return Ok(Self(Matrix4f::from_diagonal_values(v, v, v, v)));
            }
            if let Ok(m) = obj.extract::<PyRef<'_, PyMatrix4f>>() { return Ok(Self(m.0)); }
            if let Ok(v) = obj.extract::<PyRef<'_, super::vec::PyVec4f>>() {
                return Ok(Self(Matrix4f::from_diagonal_values(v.0.x, v.0.y, v.0.z, v.0.w)));
            }
            if let Ok(r) = obj.extract::<PyRef<'_, super::geo::PyRotation>>() {
                let m4d = r.0.get_matrix4();
                let mut m4f = Matrix4f::identity();
                for i in 0..4 { for j in 0..4 { m4f[i][j] = m4d[i][j] as f32; } }
                return Ok(Self(m4f));
            }
            if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuatf>>() {
                let qd = usd_gf::Quatd::new(q.0.real() as f64, usd_gf::Vec3d::new(q.0.imaginary().x as f64, q.0.imaginary().y as f64, q.0.imaginary().z as f64));
                let r = usd_gf::Rotation::from_quat(&qd);
                let m4d = r.get_matrix4();
                let mut m4f = Matrix4f::identity();
                for i in 0..4 { for j in 0..4 { m4f[i][j] = m4d[i][j] as f32; } }
                return Ok(Self(m4f));
            }
        }
        if n == 16 {
            let mut m = Matrix4f::identity();
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] = args.get_item(i * 4 + j)?.extract()?;
                }
            }
            return Ok(Self(m));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix4f: unsupported constructor"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix4f(({},{},{},{}),({},{},{},{}),({},{},{},{}),({},{},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4f>>() { return self.0 == m.0; }
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4d>>() {
            for i in 0..4 { for j in 0..4 { if (self.0[i][j] as f64) != m.0[i][j] { return false; } } }
            return true;
        }
        let _ = py; false
    }
    fn __ne__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> bool { !self.__eq__(py, o) }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }
    fn __int__(&self) -> PyResult<i64> { Err(PyValueError::new_err("cannot convert Matrix to int")) }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx4(tup.0)?; let c = idx4(tup.1)?;
            return Ok(self.0[r][c].into_pyobject(py)?.into_any().unbind());
        }
        let r = idx4(key.extract()?)?;
        Ok(super::vec::PyVec4f(usd_gf::Vec4f::new(self.0[r][0], self.0[r][1], self.0[r][2], self.0[r][3])).into_pyobject(py)?.into_any().unbind())
    }
    fn __setitem__(&mut self, key: &Bound<'_, pyo3::PyAny>, val: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(tup) = key.extract::<(isize, isize)>() {
            let r = idx4(tup.0)?; let c = idx4(tup.1)?;
            self.0[r][c] = val.extract()?;
            return Ok(());
        }
        let r = idx4(key.extract()?)?;
        let row: Vec<f32> = val.extract()?;
        if row.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2]; self.0[r][3]=row[3];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(&mut self, o: &Self) { self.0 = self.0 + o.0; }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(&mut self, o: &Self) { self.0 = self.0 - o.0; }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4f>>() {
            return Ok(Self(self.0 * m.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(m) = o.extract::<PyRef<'_, PyMatrix4f>>() { self.0 = self.0 * m.0; return Ok(()); }
        if let Ok(s) = o.extract::<f32>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __truediv__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(m) = o.extract::<PyRef<'_, Self>>() {
            let inv = m.0.inverse().unwrap_or_else(Matrix4f::identity);
            return Ok(Self(self.0 * inv).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() {
            if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
            return Ok(Self(self.0 * (1.0 / s)).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for /"))
    }

    #[pyo3(name = "Set")]
    #[pyo3(signature = (*args))]
    fn set(&mut self, args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        if args.len() != 16 { return Err(PyValueError::new_err("Set: expected 16 floats")); }
        for i in 0..4 { for j in 0..4 { self.0[i][j] = args.get_item(i*4+j)?.extract()?; } }
        Ok(self.clone())
    }
    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4f::identity(); }
    #[pyo3(name = "SetDiagonal")]
    fn set_diagonal(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<f32>() {
            self.0.set_diagonal(v, v, v, v); return Ok(self.clone());
        }
        if let Ok(v) = s.extract::<PyRef<'_, super::vec::PyVec4f>>() {
            self.0.set_diagonal(v.0.x, v.0.y, v.0.z, v.0.w); return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetDiagonal: expected scalar or Vec4f"))
    }
    /// Factor(eps?) -> (success, scaleOrientation, scale, rotation, translation, projection)
    #[pyo3(name = "Factor")]
    #[pyo3(signature = (eps=None))]
    fn factor(&self, eps: Option<f32>) -> (bool, Self, super::vec::PyVec3f, Self, super::vec::PyVec3f, Self) {
        let _ = eps;
        // Convert to f64 for computation, then back
        let m4d = Matrix4d::from_array([
            [self.0[0][0] as f64,self.0[0][1] as f64,self.0[0][2] as f64,self.0[0][3] as f64],
            [self.0[1][0] as f64,self.0[1][1] as f64,self.0[1][2] as f64,self.0[1][3] as f64],
            [self.0[2][0] as f64,self.0[2][1] as f64,self.0[2][2] as f64,self.0[2][3] as f64],
            [self.0[3][0] as f64,self.0[3][1] as f64,self.0[3][2] as f64,self.0[3][3] as f64],
        ]);
        match m4d.factor() {
            Some((r, s, u, t, p)) => {
                let to4f = |m: Matrix4d| -> Matrix4f {
                    let mut mf = Matrix4f::identity();
                    for i in 0..4 { for j in 0..4 { mf[i][j] = m[i][j] as f32; } }
                    mf
                };
                (true, Self(to4f(r)), super::vec::PyVec3f(usd_gf::Vec3f::new(s.x as f32, s.y as f32, s.z as f32)),
                 Self(to4f(u)), super::vec::PyVec3f(usd_gf::Vec3f::new(t.x as f32, t.y as f32, t.z as f32)), Self(to4f(p)))
            }
            None => (false, Self(Matrix4f::identity()), super::vec::PyVec3f(usd_gf::Vec3f::default()),
                     Self(Matrix4f::identity()), super::vec::PyVec3f(usd_gf::Vec3f::default()), Self(Matrix4f::identity())),
        }
    }
    /// SetScale — accepts a scalar, Vec3f, or tuple
    #[pyo3(name = "SetScale")]
    fn set_scale(&mut self, py: Python<'_>, s: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(v) = s.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
            self.0 = Matrix4f::identity();
            self.0[0][0] = v.0.x; self.0[1][1] = v.0.y; self.0[2][2] = v.0.z;
            return Ok(self.clone());
        }
        if let Ok(f) = s.extract::<f32>() {
            self.0 = Matrix4f::identity();
            self.0[0][0] = f; self.0[1][1] = f; self.0[2][2] = f;
            return Ok(self.clone());
        }
        if let Ok(tup) = s.extract::<(f32, f32, f32)>() {
            self.0 = Matrix4f::identity();
            self.0[0][0] = tup.0; self.0[1][1] = tup.1; self.0[2][2] = tup.2;
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetScale: expected scalar or Vec3f"))
    }
    /// SetTranslate(Vec3f) -> Matrix4f
    #[pyo3(name = "SetTranslate")]
    fn set_translate(&mut self, t: &super::vec::PyVec3f) -> Self {
        self.0 = Matrix4f::identity();
        self.0[3][0] = t.0.x; self.0[3][1] = t.0.y; self.0[3][2] = t.0.z;
        self.clone()
    }
    /// SetRotate — accepts Rotation, Quatf
    #[pyo3(name = "SetRotate")]
    fn set_rotate(&mut self, py: Python<'_>, rot: &Bound<'_, pyo3::PyAny>) -> PyResult<Self> {
        if let Ok(r) = rot.extract::<pyo3::PyRef<'_, super::geo::PyRotation>>() {
            let m4d = r.0.get_matrix4();
            for i in 0..4 { for j in 0..4 { self.0[i][j] = m4d[i][j] as f32; } }
            return Ok(self.clone());
        }
        if let Ok(q) = rot.extract::<pyo3::PyRef<'_, super::quat::PyQuatf>>() {
            let qd = usd_gf::Quatd::new(q.0.real() as f64, usd_gf::Vec3d::new(q.0.imaginary().x as f64, q.0.imaginary().y as f64, q.0.imaginary().z as f64));
            let r = usd_gf::Rotation::from_quat(&qd);
            let m4d = r.get_matrix4();
            for i in 0..4 { for j in 0..4 { self.0[i][j] = m4d[i][j] as f32; } }
            return Ok(self.clone());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("SetRotate: expected Rotation or Quatf"))
    }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix4f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "Invert")] fn invert(&mut self) {
        if let Some(inv) = self.0.inverse() { self.0 = inv; }
    }
    /// Transform(point) -> Vec3f (full transform with perspective divide)
    #[pyo3(name = "Transform")]
    fn transform(&self, v: &super::vec::PyVec3f) -> super::vec::PyVec3f {
        let d = v.0;
        let m = &self.0;
        let x = d.x*m[0][0] + d.y*m[1][0] + d.z*m[2][0] + m[3][0];
        let y = d.x*m[0][1] + d.y*m[1][1] + d.z*m[2][1] + m[3][1];
        let z = d.x*m[0][2] + d.y*m[1][2] + d.z*m[2][2] + m[3][2];
        let w = d.x*m[0][3] + d.y*m[1][3] + d.z*m[2][3] + m[3][3];
        if w != 0.0 { super::vec::PyVec3f(usd_gf::Vec3f::new(x/w,y/w,z/w)) }
        else { super::vec::PyVec3f(usd_gf::Vec3f::new(x,y,z)) }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMatrix2d>()?;
    m.add_class::<PyMatrix2f>()?;
    m.add_class::<PyMatrix3d>()?;
    m.add_class::<PyMatrix3f>()?;
    m.add_class::<PyMatrix4d>()?;
    m.add_class::<PyMatrix4f>()?;
    Ok(())
}
