//! Quaternion Python bindings: Quatd/f/h, Quaternion, DualQuatd/f/h.

use pyo3::prelude::*;
use usd_gf::{Quatd, Quatf, Quath, Quaternion, DualQuatd, DualQuatf, DualQuath};
use usd_gf::half::Half;

fn hash_f64_2(a: f64, b: f64, c: f64, d: f64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.to_bits().hash(&mut h); b.to_bits().hash(&mut h);
    c.to_bits().hash(&mut h); d.to_bits().hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// Quatd
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quatd", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuatd(pub Quatd);

#[pymethods]
impl PyQuatd {
    #[new]
    #[pyo3(signature = (real=1.0, i=0.0, j=0.0, k=0.0))]
    fn new(real: f64, i: f64, j: f64, k: f64) -> Self {
        Self(Quatd::from_components(real, i, j, k))
    }

    fn __repr__(&self) -> String {
        let im = self.0.imaginary();
        format!("Gf.Quatd({}, ({}, {}, {}))", self.0.real(), im.x, im.y, im.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let im = self.0.imaginary();
        hash_f64_2(self.0.real(), im.x, im.y, im.z)
    }
    fn __neg__(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quatd::from_components(-self.0.real(), -im.x, -im.y, -im.z))
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 / s))
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(Quatd::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(Quatd::identity()) }

    #[pyo3(name = "GetReal")]      fn get_real(&self) -> f64 { self.0.real() }
    #[pyo3(name = "SetReal")]      fn set_real(&mut self, v: f64) { self.0.set_real(v); }
    #[pyo3(name = "GetImaginary")] fn get_imaginary(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(*self.0.imaginary())
    }
    #[pyo3(name = "SetImaginary")] fn set_imaginary(&mut self, v: &super::vec::PyVec3d) {
        self.0.set_imaginary(v.0);
    }
    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f64 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")]    fn normalize(&mut self) -> f64 { self.0.normalize() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }

    #[getter] fn real(&self) -> f64 { self.0.real() }
    #[setter] fn set_real_prop(&mut self, v: f64) { self.0.set_real(v); }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.imaginary()) }
    #[setter] fn set_imaginary_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_imaginary(v.0); }
}

// ---------------------------------------------------------------------------
// Quatf
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quatf", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuatf(pub Quatf);

#[pymethods]
impl PyQuatf {
    #[new]
    #[pyo3(signature = (real=1.0, i=0.0, j=0.0, k=0.0))]
    fn new(real: f32, i: f32, j: f32, k: f32) -> Self {
        Self(Quatf::from_components(real, i, j, k))
    }

    fn __repr__(&self) -> String {
        let im = self.0.imaginary();
        format!("Gf.Quatf({}, ({}, {}, {}))", self.0.real(), im.x, im.y, im.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let im = self.0.imaginary();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.real().to_bits().hash(&mut h); im.x.to_bits().hash(&mut h);
        im.y.to_bits().hash(&mut h); im.z.to_bits().hash(&mut h);
        h.finish()
    }
    fn __neg__(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quatf::from_components(-self.0.real(), -im.x, -im.y, -im.z))
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 / s))
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(Quatf::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(Quatf::identity()) }

    #[pyo3(name = "GetReal")]      fn get_real(&self) -> f32 { self.0.real() }
    #[pyo3(name = "SetReal")]      fn set_real(&mut self, v: f32) { self.0.set_real(v); }
    #[pyo3(name = "GetImaginary")] fn get_imaginary(&self) -> super::vec::PyVec3f {
        super::vec::PyVec3f(*self.0.imaginary())
    }
    #[pyo3(name = "SetImaginary")] fn set_imaginary(&mut self, v: &super::vec::PyVec3f) {
        self.0.set_imaginary(v.0);
    }
    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f32 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")]    fn normalize(&mut self) -> f32 { self.0.normalize() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }

    #[getter] fn real(&self) -> f32 { self.0.real() }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.imaginary()) }
}

// ---------------------------------------------------------------------------
// Quath (half-precision — Python boundary uses f32)
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quath", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuath(pub Quath);

#[pymethods]
impl PyQuath {
    #[new]
    #[pyo3(signature = (real=1.0, i=0.0, j=0.0, k=0.0))]
    fn new(real: f32, i: f32, j: f32, k: f32) -> Self {
        Self(Quath::from_components(
            Half::from_f32(real),
            Half::from_f32(i),
            Half::from_f32(j),
            Half::from_f32(k),
        ))
    }

    fn __repr__(&self) -> String {
        let im = self.0.imaginary();
        format!("Gf.Quath({}, ({}, {}, {}))",
            self.0.real().to_f32(), im.x.to_f32(), im.y.to_f32(), im.z.to_f32())
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let im = self.0.imaginary();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // Half uses .bits() not .to_bits()
        self.0.real().bits().hash(&mut h);
        im.x.bits().hash(&mut h); im.y.bits().hash(&mut h); im.z.bits().hash(&mut h);
        h.finish()
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(Quath::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(Quath::identity()) }

    #[pyo3(name = "GetReal")]      fn get_real(&self) -> f32 { self.0.real().to_f32() }
    #[pyo3(name = "SetReal")]      fn set_real(&mut self, v: f32) { self.0.set_real(Half::from_f32(v)); }
    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f32 { self.0.length().to_f32() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")]    fn normalize(&mut self) -> f32 { self.0.normalize().to_f32() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }

    #[getter] fn real(&self) -> f32 { self.0.real().to_f32() }
}

// ---------------------------------------------------------------------------
// Quaternion (legacy GfQuaternion, double real + double imaginary)
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quaternion", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuaternion(pub Quaternion);

#[pymethods]
impl PyQuaternion {
    #[new]
    #[pyo3(signature = (real=1.0, i=0.0, j=0.0, k=0.0))]
    fn new(real: f64, i: f64, j: f64, k: f64) -> Self {
        Self(Quaternion::new(real, usd_gf::vec3::Vec3d::new(i, j, k)))
    }

    fn __repr__(&self) -> String {
        let im = self.0.imaginary();
        format!("Gf.Quaternion({}, ({}, {}, {}))", self.0.real(), im.x, im.y, im.z)
    }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let im = self.0.imaginary();
        hash_f64_2(self.0.real(), im.x, im.y, im.z)
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(Quaternion::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(Quaternion::identity()) }

    #[pyo3(name = "GetReal")]      fn get_real(&self) -> f64 { self.0.real() }
    #[pyo3(name = "SetReal")]      fn set_real(&mut self, v: f64) { self.0.set_real(v); }
    #[pyo3(name = "GetImaginary")] fn get_imaginary(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(*self.0.imaginary())
    }
    #[pyo3(name = "SetImaginary")] fn set_imaginary(&mut self, v: &super::vec::PyVec3d) {
        self.0.set_imaginary(v.0);
    }
    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f64 { self.0.length() }
    // Quaternion::normalized/normalize take an eps parameter
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.get_normalized()) }
    #[pyo3(name = "Normalize")]    fn normalize(&mut self) -> f64 { self.0.normalize(1e-10) }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.get_inverse()) }
    // Quaternion (legacy) has no conjugate method; implement manually
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quaternion::new(self.0.real(), usd_gf::vec3::Vec3d::new(-im.x, -im.y, -im.z)))
    }

    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __neg__(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quaternion::new(-self.0.real(), usd_gf::vec3::Vec3d::new(-im.x, -im.y, -im.z)))
    }

    #[getter] fn real(&self) -> f64 { self.0.real() }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.imaginary()) }
}

// ---------------------------------------------------------------------------
// DualQuatd
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "DualQuatd", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyDualQuatd(pub DualQuatd);

#[pymethods]
impl PyDualQuatd {
    #[new]
    fn new() -> Self { Self(DualQuatd::identity()) }

    fn __repr__(&self) -> String { format!("Gf.DualQuatd(real={}, dual={})",
        self.0.real().real(), self.0.dual().real()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuatd::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuatd::identity()) }

    #[pyo3(name = "GetReal")]    fn get_real(&self) -> PyQuatd { PyQuatd(*self.0.real()) }
    #[pyo3(name = "SetReal")]    fn set_real(&mut self, q: &PyQuatd) { self.0.set_real(q.0); }
    #[pyo3(name = "GetDual")]    fn get_dual(&self) -> PyQuatd { PyQuatd(*self.0.dual()) }
    #[pyo3(name = "SetDual")]    fn set_dual(&mut self, q: &PyQuatd) { self.0.set_dual(q.0); }
    // DualQuat::length() returns (T, T) — real and dual parts of the length
    #[pyo3(name = "GetLength")]  fn get_length(&self) -> (f64, f64) { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")]  fn normalize(&mut self) { self.0.normalize(); }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "SetTranslation")] fn set_translation(&mut self, t: &super::vec::PyVec3d) {
        self.0.set_translation(&t.0);
    }
    #[pyo3(name = "GetTranslation")] fn get_translation(&self) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.translation())
    }
    #[pyo3(name = "Transform")] fn transform(&self, p: &super::vec::PyVec3d) -> super::vec::PyVec3d {
        super::vec::PyVec3d(self.0.transform(&p.0))
    }

    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }

    #[getter] fn real(&self) -> PyQuatd { PyQuatd(*self.0.real()) }
    #[setter] fn set_real_prop(&mut self, q: &PyQuatd) { self.0.set_real(q.0); }
    #[getter] fn dual(&self) -> PyQuatd { PyQuatd(*self.0.dual()) }
    #[setter] fn set_dual_prop(&mut self, q: &PyQuatd) { self.0.set_dual(q.0); }
}

// ---------------------------------------------------------------------------
// DualQuatf
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "DualQuatf", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyDualQuatf(pub DualQuatf);

#[pymethods]
impl PyDualQuatf {
    #[new]
    fn new() -> Self { Self(DualQuatf::identity()) }

    fn __repr__(&self) -> String { format!("Gf.DualQuatf(real={}, dual={})",
        self.0.real().real(), self.0.dual().real()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuatf::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuatf::identity()) }

    #[pyo3(name = "GetReal")]    fn get_real(&self) -> PyQuatf { PyQuatf(*self.0.real()) }
    #[pyo3(name = "SetReal")]    fn set_real(&mut self, q: &PyQuatf) { self.0.set_real(q.0); }
    #[pyo3(name = "GetDual")]    fn get_dual(&self) -> PyQuatf { PyQuatf(*self.0.dual()) }
    #[pyo3(name = "SetDual")]    fn set_dual(&mut self, q: &PyQuatf) { self.0.set_dual(q.0); }
    #[pyo3(name = "GetLength")]  fn get_length(&self) -> (f32, f32) { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")]  fn normalize(&mut self) { self.0.normalize(); }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "SetTranslation")] fn set_translation(&mut self, t: &super::vec::PyVec3f) {
        self.0.set_translation(&t.0);
    }
    #[pyo3(name = "GetTranslation")] fn get_translation(&self) -> super::vec::PyVec3f {
        super::vec::PyVec3f(self.0.translation())
    }
    #[pyo3(name = "Transform")] fn transform(&self, p: &super::vec::PyVec3f) -> super::vec::PyVec3f {
        super::vec::PyVec3f(self.0.transform(&p.0))
    }

    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
}

// ---------------------------------------------------------------------------
// DualQuath
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "DualQuath", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyDualQuath(pub DualQuath);

#[pymethods]
impl PyDualQuath {
    #[new]
    fn new() -> Self { Self(DualQuath::identity()) }

    fn __repr__(&self) -> String { format!("Gf.DualQuath(real={}, dual={})",
        self.0.real().real().to_f32(), self.0.dual().real().to_f32()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuath::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuath::identity()) }

    #[pyo3(name = "GetReal")]  fn get_real(&self) -> PyQuath { PyQuath(*self.0.real()) }
    #[pyo3(name = "GetDual")]  fn get_dual(&self) -> PyQuath { PyQuath(*self.0.dual()) }
    // DualQuat::length() returns (Half, Half); expose as (f32, f32)
    #[pyo3(name = "GetLength")] fn get_length(&self) -> (f32, f32) {
        let (a, b) = self.0.length();
        (a.to_f32(), b.to_f32())
    }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) { self.0.normalize(); }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

#[pyfunction(name = "Slerp")]
pub fn slerp_quatd(alpha: f64, q0: &PyQuatd, q1: &PyQuatd) -> PyQuatd {
    PyQuatd(usd_gf::quat_slerp(alpha, &q0.0, &q1.0))
}

#[pyfunction(name = "Dot")]
pub fn dot_quatd(a: &PyQuatd, b: &PyQuatd) -> f64 {
    usd_gf::quat_dot(&a.0, &b.0)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyQuatd>()?;
    m.add_class::<PyQuatf>()?;
    m.add_class::<PyQuath>()?;
    m.add_class::<PyQuaternion>()?;
    m.add_class::<PyDualQuatd>()?;
    m.add_class::<PyDualQuatf>()?;
    m.add_class::<PyDualQuath>()?;
    m.add_function(wrap_pyfunction!(slerp_quatd, m)?)?;
    m.add_function(wrap_pyfunction!(dot_quatd, m)?)?;
    Ok(())
}
