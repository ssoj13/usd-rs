//! Matrix Python bindings (Matrix2d/f, Matrix3d/f, Matrix4d/f).
//!
//! All matrices are row-major, matching C++ GfMatrix layout.
//! Python indexing returns rows as lists, matching pxr.Gf behaviour.
//!
//! Note: `inverse()` returns `Option<Self>` — we fall back to identity on singular matrices,
//! matching the C++ behaviour of `GetInverse(double* det = NULL)`.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError, PyZeroDivisionError};
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

    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f64) -> Self { Self(Matrix2d::from_diagonal(s, s)) }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix2d(({}, {}), ({}, {}))",
            self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f64>> {
        let r = idx2(i)?;
        Ok(vec![self.0[r][0], self.0[r][1]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f64>) -> PyResult<()> {
        let r = idx2(i)?;
        if row.len() != 2 { return Err(PyIndexError::new_err("expected 2 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        // Matrix2 has no Div<T>; scale by 1/s
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[staticmethod]
    #[pyo3(name = "GetIdentity")]
    fn get_identity() -> Self { Self(Matrix2d::identity()) }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2d::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix2d::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "GetRow")] fn get_row(&self, i: usize) -> PyResult<Vec<f64>> {
        if i >= 2 { return Err(PyIndexError::new_err("row index out of range")); }
        Ok(vec![self.0[i][0], self.0[i][1]])
    }
    #[pyo3(name = "GetColumn")] fn get_column(&self, j: usize) -> PyResult<Vec<f64>> {
        if j >= 2 { return Err(PyIndexError::new_err("col index out of range")); }
        Ok(vec![self.0[0][j], self.0[1][j]])
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

    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f32) -> Self { Self(Matrix2f::from_diagonal(s, s)) }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix2f(({}, {}), ({}, {}))",
            self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[self.0[0][0], self.0[0][1], self.0[1][0], self.0[1][1]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f32>> {
        let r = idx2(i)?;
        Ok(vec![self.0[r][0], self.0[r][1]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f32>) -> PyResult<()> {
        let r = idx2(i)?;
        if row.len() != 2 { return Err(PyIndexError::new_err("expected 2 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix2f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
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

    /// Matrix3d(), Matrix3d(scalar), Matrix3d(Rotation), Matrix3d(Quatd)
    #[new]
    #[pyo3(signature = (s=None))]
    fn new(s: Option<&Bound<'_, pyo3::PyAny>>) -> PyResult<Self> {
        let Some(obj) = s else { return Ok(Self(Matrix3d::from_diagonal_values(1.0, 1.0, 1.0))); };
        if let Ok(v) = obj.extract::<f64>() {
            return Ok(Self(Matrix3d::from_diagonal_values(v, v, v)));
        }
        if let Ok(r) = obj.extract::<PyRef<'_, super::geo::PyRotation>>() {
            return Ok(Self(r.0.get_matrix3()));
        }
        if let Ok(q) = obj.extract::<PyRef<'_, super::quat::PyQuatd>>() {
            let r = usd_gf::Rotation::from_quat(&q.0);
            return Ok(Self(r.get_matrix3()));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Matrix3d: expected scalar, Rotation, or Quatd"))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix3d(({},{},{}),({},{},{}),({},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f64>> {
        let r = idx3(i)?;
        Ok(vec![self.0[r][0], self.0[r][1], self.0[r][2]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f64>) -> PyResult<()> {
        let r = idx3(i)?;
        if row.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[r][0] = row[0]; self.0[r][1] = row[1]; self.0[r][2] = row[2];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3d::identity(); }
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

    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f32) -> Self { Self(Matrix3f::from_diagonal_values(s, s, s)) }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix3f(({},{},{}),({},{},{}),({},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],
            self.0[1][0],self.0[1][1],self.0[1][2],
            self.0[2][0],self.0[2][1],self.0[2][2]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f32>> {
        let r = idx3(i)?;
        Ok(vec![self.0[r][0], self.0[r][1], self.0[r][2]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f32>) -> PyResult<()> {
        let r = idx3(i)?;
        if row.len() != 3 { return Err(PyIndexError::new_err("expected 3 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix3f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
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
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f64_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f64>> {
        let r = idx4(i)?;
        Ok(vec![self.0[r][0], self.0[r][1], self.0[r][2], self.0[r][3]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f64>) -> PyResult<()> {
        let r = idx4(i)?;
        if row.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2]; self.0[r][3]=row[3];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4d::identity(); }
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

    /// SetTranslate(Vec3d) -> Matrix4d
    #[pyo3(name = "SetTranslate")]
    fn set_translate(&mut self, t: &super::vec::PyVec3d) -> Self {
        self.0.set_translate(&t.0);
        self.clone()
    }

    /// SetTranslateOnly(Vec3d) -> Matrix4d
    #[pyo3(name = "SetTranslateOnly")]
    fn set_translate_only(&mut self, t: &super::vec::PyVec3d) -> Self {
        self.0.set_translate_only(&t.0);
        self.clone()
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
            return Ok(super::vec::PyVec3f(usd_gf::Vec3f::new(r.x as f32, r.y as f32, r.z as f32)).into_pyobject(py)?.into_any().unbind());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("TransformDir: expected Vec3d or Vec3f"))
    }

    /// TransformAffine(Vec3d) -> Vec3d: transform a point (with translation)
    #[pyo3(name = "TransformAffine")]
    fn transform_affine(&self, v: &super::vec::PyVec3d) -> super::vec::PyVec3d {
        let d = v.0;
        super::vec::PyVec3d(usd_gf::Vec3d::new(
            d.x * self.0[0][0] + d.y * self.0[1][0] + d.z * self.0[2][0] + self.0[3][0],
            d.x * self.0[0][1] + d.y * self.0[1][1] + d.z * self.0[2][1] + self.0[3][1],
            d.x * self.0[0][2] + d.y * self.0[1][2] + d.z * self.0[2][2] + self.0[3][2],
        ))
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

    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f32) -> Self { Self(Matrix4f::from_diagonal_values(s, s, s, s)) }

    fn __repr__(&self) -> String {
        format!("Gf.Matrix4f(({},{},{},{}),({},{},{},{}),({},{},{},{}),({},{},{},{}))",
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3])
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        hash_f32_slice(&[
            self.0[0][0],self.0[0][1],self.0[0][2],self.0[0][3],
            self.0[1][0],self.0[1][1],self.0[1][2],self.0[1][3],
            self.0[2][0],self.0[2][1],self.0[2][2],self.0[2][3],
            self.0[3][0],self.0[3][1],self.0[3][2],self.0[3][3]])
    }
    fn __neg__(&self) -> Self { Self(-self.0) }

    fn __getitem__(&self, i: isize) -> PyResult<Vec<f32>> {
        let r = idx4(i)?;
        Ok(vec![self.0[r][0], self.0[r][1], self.0[r][2], self.0[r][3]])
    }
    fn __setitem__(&mut self, i: isize, row: Vec<f32>) -> PyResult<()> {
        let r = idx4(i)?;
        if row.len() != 4 { return Err(PyIndexError::new_err("expected 4 elements")); }
        self.0[r][0]=row[0]; self.0[r][1]=row[1]; self.0[r][2]=row[2]; self.0[r][3]=row[3];
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix4f::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose_mut(&mut self) { self.0 = self.0.transpose(); }
    #[pyo3(name = "Invert")] fn invert(&mut self) {
        if let Some(inv) = self.0.inverse() { self.0 = inv; }
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
