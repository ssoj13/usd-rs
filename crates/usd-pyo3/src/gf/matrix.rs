//! Matrix Python bindings (Matrix2d/f, Matrix3d/f, Matrix4d/f).
//!
//! All matrices are row-major, matching C++ GfMatrix layout.
//! Python indexing returns rows as lists, matching pxr.Gf behaviour.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError, PyZeroDivisionError};
use usd_gf::{Matrix2d, Matrix2f, Matrix3d, Matrix3f, Matrix4d, Matrix4f};
use usd_gf::vec3::{Vec3d, Vec3f};

// ---------------------------------------------------------------------------
// Index helpers
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

#[pyclass(name = "Matrix2d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix2d(pub Matrix2d);

#[pymethods]
impl PyMatrix2d {
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
        Ok(Self(self.0 / s))
    }

    #[staticmethod]
    #[pyo3(name = "SetIdentity")]
    fn identity() -> Self { Self(Matrix2d::identity()) }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2d::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self {
        Self(self.0.inverse().unwrap_or_else(Matrix2d::identity))
    }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transpose()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transpose(); }
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

#[pyclass(name = "Matrix2f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix2f(pub Matrix2f);

#[pymethods]
impl PyMatrix2f {
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
        Ok(Self(self.0 / s))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix2f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix2f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transposed()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transposed(); }
}

// ---------------------------------------------------------------------------
// Matrix3d
// ---------------------------------------------------------------------------

#[pyclass(name = "Matrix3d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix3d(pub Matrix3d);

#[pymethods]
impl PyMatrix3d {
    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f64) -> Self { Self(Matrix3d::from_diagonal(s, s, s)) }

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
        Ok(Self(self.0 / s))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3d::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transposed()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transposed(); }

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
        super::geo::PyRotation(usd_gf::Rotation::from_matrix3(&self.0))
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
        Self(Matrix3d::from_diagonal(scale, scale, scale))
    }
}

// ---------------------------------------------------------------------------
// Matrix3f
// ---------------------------------------------------------------------------

#[pyclass(name = "Matrix3f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix3f(pub Matrix3f);

#[pymethods]
impl PyMatrix3f {
    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f32) -> Self { Self(Matrix3f::from_diagonal(s, s, s)) }

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
        Ok(Self(self.0 / s))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix3f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix3f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transposed()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transposed(); }
}

// ---------------------------------------------------------------------------
// Matrix4d
// ---------------------------------------------------------------------------

#[pyclass(name = "Matrix4d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix4d(pub Matrix4d);

#[pymethods]
impl PyMatrix4d {
    #[new]
    #[pyo3(signature = (s=1.0))]
    fn new(s: f64) -> Self { Self(Matrix4d::from_diagonal_values(s, s, s, s)) }

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
        Ok(Self(self.0 / s))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4d::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4d::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f64 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transposed()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transposed(); }
    #[pyo3(name = "Invert")] fn invert(&mut self) { self.0 = self.0.inverse(); }

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

#[pyclass(name = "Matrix4f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyMatrix4f(pub Matrix4f);

#[pymethods]
impl PyMatrix4f {
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
        Ok(Self(self.0 / s))
    }

    #[pyo3(name = "SetZero")] fn set_zero(&mut self) { self.0 = Matrix4f::zero(); }
    #[pyo3(name = "SetIdentity")] fn set_identity(&mut self) { self.0 = Matrix4f::identity(); }
    #[pyo3(name = "GetDeterminant")] fn get_determinant(&self) -> f32 { self.0.determinant() }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetTranspose")] fn get_transpose(&self) -> Self { Self(self.0.transposed()) }
    #[pyo3(name = "Transpose")] fn transpose(&mut self) { self.0 = self.0.transposed(); }
    #[pyo3(name = "Invert")] fn invert(&mut self) { self.0 = self.0.inverse(); }
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
