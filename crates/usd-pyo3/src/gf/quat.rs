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
    /// Quatd(), Quatd(real), Quatd(real, i, j, k), Quatd(real, Vec3d),
    /// Quatd(real, [i,j,k]), Quatd(Quatd)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &pyo3::Bound<'_, pyo3::types::PyTuple>) -> pyo3::PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Quatd::identity())); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatd>>() { return Ok(Self(q.0)); }
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatf>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quatd::from_components(q.0.real() as f64, im.x as f64, im.y as f64, im.z as f64)));
            }
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuath>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quatd::from_components(q.0.real().to_f32() as f64, im.x.to_f32() as f64, im.y.to_f32() as f64, im.z.to_f32() as f64)));
            }
            let r: f64 = obj.extract()?;
            return Ok(Self(Quatd::from_components(r, 0.0, 0.0, 0.0)));
        }
        if n == 2 {
            let r: f64 = args.get_item(0)?.extract()?;
            let im_obj = args.get_item(1)?;
            if let Ok(v) = im_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
                return Ok(Self(Quatd::from_components(r, v.0.x, v.0.y, v.0.z)));
            }
            if let Ok(v) = im_obj.extract::<Vec<f64>>() {
                if v.len() == 3 { return Ok(Self(Quatd::from_components(r, v[0], v[1], v[2]))); }
            }
            return Err(pyo3::exceptions::PyTypeError::new_err("Quatd: second arg must be Vec3d or [i,j,k]"));
        }
        if n == 4 {
            let r: f64 = args.get_item(0)?.extract()?;
            let i: f64 = args.get_item(1)?.extract()?;
            let j: f64 = args.get_item(2)?.extract()?;
            let k: f64 = args.get_item(3)?.extract()?;
            return Ok(Self(Quatd::from_components(r, i, j, k)));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Quatd: expected (), (real), (real, Vec3d), or (real, i, j, k)"))
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
    fn __iadd__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { let b = o.extract::<PyRef<'_, Self>>()?; b.0 };
        let lhs = { slf.borrow().0 };
        slf.borrow_mut().0 = lhs + rhs; Ok(())
    }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { let b = o.extract::<PyRef<'_, Self>>()?; b.0 };
        let lhs = { slf.borrow().0 };
        slf.borrow_mut().0 = lhs - rhs; Ok(())
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(q) = o.extract::<PyRef<'_, Self>>() { return Ok(Self(self.0 * q.0).into_pyobject(py)?.into_any().unbind()); }
        if let Ok(s) = o.extract::<f64>() { return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind()); }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self { Self(self.0 * s) }
    fn __imul__(slf: Bound<'_, Self>, py: Python<'_>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(s) = o.extract::<f64>() {
            let lhs = { slf.borrow().0 };
            slf.borrow_mut().0 = lhs * s; return Ok(());
        }
        let rhs = { let b = o.extract::<PyRef<'_, Self>>()?; b.0 };
        let lhs = { slf.borrow().0 };
        slf.borrow_mut().0 = lhs * rhs;
        let _ = py; Ok(())
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 / s))
    }
    fn __itruediv__(&mut self, s: f64) -> PyResult<()> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        self.0 = self.0 / s; Ok(())
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
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=f64::MIN_POSITIVE))]
    fn get_normalized(&self, eps: f64) -> Self { let mut q = self.0; q.normalize_with_eps(eps); Self(q) }
    #[pyo3(name = "Normalize")]
    #[pyo3(signature = (eps=f64::MIN_POSITIVE))]
    fn normalize(&mut self, eps: f64) -> Self { self.0.normalize_with_eps(eps); self.clone() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }

    #[getter] fn real(&self) -> f64 { self.0.real() }
    #[setter(real)] fn set_real_prop(&mut self, v: f64) { self.0.set_real(v); }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.imaginary()) }
    #[setter(imaginary)] fn set_imaginary_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_imaginary(v.0); }
}

// ---------------------------------------------------------------------------
// Quatf
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quatf", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuatf(pub Quatf);

#[pymethods]
impl PyQuatf {
    /// Quatf(), Quatf(real), Quatf(real, i, j, k), Quatf(real, Vec3f),
    /// Quatf(real, [i,j,k]), Quatf(Quatf)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &pyo3::Bound<'_, pyo3::types::PyTuple>) -> pyo3::PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Quatf::identity())); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatf>>() { return Ok(Self(q.0)); }
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatd>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quatf::from_components(q.0.real() as f32, im.x as f32, im.y as f32, im.z as f32)));
            }
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuath>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quatf::from_components(q.0.real().to_f32(), im.x.to_f32(), im.y.to_f32(), im.z.to_f32())));
            }
            let r: f32 = obj.extract()?;
            return Ok(Self(Quatf::from_components(r, 0.0, 0.0, 0.0)));
        }
        if n == 2 {
            let r: f32 = args.get_item(0)?.extract()?;
            let im_obj = args.get_item(1)?;
            if let Ok(v) = im_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() {
                return Ok(Self(Quatf::from_components(r, v.0.x, v.0.y, v.0.z)));
            }
            if let Ok(v) = im_obj.extract::<Vec<f32>>() {
                if v.len() == 3 { return Ok(Self(Quatf::from_components(r, v[0], v[1], v[2]))); }
            }
            return Err(pyo3::exceptions::PyTypeError::new_err("Quatf: second arg must be Vec3f or [i,j,k]"));
        }
        if n == 4 {
            let r: f32 = args.get_item(0)?.extract()?;
            let i: f32 = args.get_item(1)?.extract()?;
            let j: f32 = args.get_item(2)?.extract()?;
            let k: f32 = args.get_item(3)?.extract()?;
            return Ok(Self(Quatf::from_components(r, i, j, k)));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Quatf: expected (), (real), (real, Vec3f), or (real, i, j, k)"))
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
    fn __iadd__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs + rhs; Ok(())
    }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs - rhs; Ok(())
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(q) = o.extract::<PyRef<'_, Self>>() { return Ok(Self(self.0 * q.0).into_pyobject(py)?.into_any().unbind()); }
        if let Ok(s) = o.extract::<f32>() { return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind()); }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * s) }
    fn __imul__(slf: Bound<'_, Self>, py: Python<'_>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(s) = o.extract::<f32>() { let l = { slf.borrow().0 }; slf.borrow_mut().0 = l * s; return Ok(()); }
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs * rhs;
        let _ = py; Ok(())
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 / s))
    }
    fn __itruediv__(&mut self, s: f32) -> PyResult<()> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        self.0 = self.0 / s; Ok(())
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
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=f32::MIN_POSITIVE))]
    fn get_normalized(&self, eps: f32) -> Self { let mut q = self.0; q.normalize_with_eps(eps); Self(q) }
    #[pyo3(name = "Normalize")]
    #[pyo3(signature = (eps=f32::MIN_POSITIVE))]
    fn normalize(&mut self, eps: f32) -> Self { self.0.normalize_with_eps(eps); self.clone() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }

    #[getter] fn real(&self) -> f32 { self.0.real() }
    #[setter(real)] fn set_real_prop(&mut self, v: f32) { self.0.set_real(v); }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3f { super::vec::PyVec3f(*self.0.imaginary()) }
    #[setter(imaginary)] fn set_imaginary_prop(&mut self, v: &super::vec::PyVec3f) { self.0.set_imaginary(v.0); }
}

// ---------------------------------------------------------------------------
// Quath (half-precision — Python boundary uses f32)
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quath", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuath(pub Quath);

#[pymethods]
impl PyQuath {
    /// Quath(), Quath(real), Quath(real, i, j, k), Quath(real, Vec3h),
    /// Quath(real, [i,j,k]), Quath(Quath)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &pyo3::Bound<'_, pyo3::types::PyTuple>) -> pyo3::PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Quath::identity())); }
        if n == 1 {
            let obj = args.get_item(0)?;
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuath>>() { return Ok(Self(q.0)); }
            // Cross-type: Quath(Quatf) or Quath(Quatd)
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatf>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quath::from_components(Half::from_f32(q.0.real()), Half::from_f32(im.x), Half::from_f32(im.y), Half::from_f32(im.z))));
            }
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuatd>>() {
                let im = q.0.imaginary();
                return Ok(Self(Quath::from_components(Half::from_f32(q.0.real() as f32), Half::from_f32(im.x as f32), Half::from_f32(im.y as f32), Half::from_f32(im.z as f32))));
            }
            let r: f32 = obj.extract()?;
            return Ok(Self(Quath::from_components(Half::from_f32(r), Half::from_f32(0.0), Half::from_f32(0.0), Half::from_f32(0.0))));
        }
        if n == 2 {
            let r: f32 = args.get_item(0)?.extract()?;
            let im_obj = args.get_item(1)?;
            if let Ok(v) = im_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3h>>() {
                return Ok(Self(Quath::from_components(Half::from_f32(r), v.0.x, v.0.y, v.0.z)));
            }
            if let Ok(v) = im_obj.extract::<Vec<f32>>() {
                if v.len() == 3 {
                    return Ok(Self(Quath::from_components(Half::from_f32(r), Half::from_f32(v[0]), Half::from_f32(v[1]), Half::from_f32(v[2]))));
                }
            }
            return Err(pyo3::exceptions::PyTypeError::new_err("Quath: second arg must be Vec3h or [i,j,k]"));
        }
        if n == 4 {
            let r: f32 = args.get_item(0)?.extract()?;
            let i: f32 = args.get_item(1)?.extract()?;
            let j: f32 = args.get_item(2)?.extract()?;
            let k: f32 = args.get_item(3)?.extract()?;
            return Ok(Self(Quath::from_components(Half::from_f32(r), Half::from_f32(i), Half::from_f32(j), Half::from_f32(k))));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Quath: expected (), (real), (real, Vec3h), or (real, i, j, k)"))
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
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=f32::MIN_POSITIVE))]
    fn get_normalized(&self, eps: f32) -> Self { let mut q = self.0; q.normalize_with_eps(Half::from_f32(eps)); Self(q) }
    #[pyo3(name = "Normalize")]
    #[pyo3(signature = (eps=f32::MIN_POSITIVE))]
    fn normalize(&mut self, eps: f32) -> Self { self.0.normalize_with_eps(Half::from_f32(eps)); self.clone() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.inverse()) }

    #[getter] fn real(&self) -> f32 { self.0.real().to_f32() }
    #[setter(real)] fn set_real_prop(&mut self, v: f32) { self.0.set_real(Half::from_f32(v)); }
    #[pyo3(name = "GetImaginary")] fn get_imaginary(&self) -> super::vec::PyVec3h {
        super::vec::PyVec3h(*self.0.imaginary())
    }
    #[pyo3(name = "SetImaginary")] fn set_imaginary_method(&mut self, v: &super::vec::PyVec3h) {
        self.0.set_imaginary(v.0);
    }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3h { super::vec::PyVec3h(*self.0.imaginary()) }
    #[setter(imaginary)] fn set_imaginary_prop(&mut self, v: &super::vec::PyVec3h) {
        self.0.set_imaginary(v.0);
    }

    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs + rhs; Ok(())
    }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs - rhs; Ok(())
    }
    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(q) = o.extract::<PyRef<'_, Self>>() { return Ok(Self(self.0 * q.0).into_pyobject(py)?.into_any().unbind()); }
        if let Ok(s) = o.extract::<f32>() { return Ok(Self(self.0 * Half::from_f32(s)).into_pyobject(py)?.into_any().unbind()); }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * Half::from_f32(s)) }
    fn __imul__(slf: Bound<'_, Self>, py: Python<'_>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(s) = o.extract::<f32>() { let l = { slf.borrow().0 }; slf.borrow_mut().0 = l * Half::from_f32(s); return Ok(()); }
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let lhs = { slf.borrow().0 }; slf.borrow_mut().0 = lhs * rhs;
        let _ = py; Ok(())
    }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 / Half::from_f32(s)))
    }
    fn __itruediv__(&mut self, s: f32) -> PyResult<()> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        self.0 = self.0 / Half::from_f32(s); Ok(())
    }
}

// ---------------------------------------------------------------------------
// Quaternion (legacy GfQuaternion, double real + double imaginary)
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "Quaternion", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyQuaternion(pub Quaternion);

#[pymethods]
impl PyQuaternion {
    /// Quaternion(), Quaternion(real), Quaternion(real, i, j, k),
    /// Quaternion(real, Vec3d), Quaternion(real, [i,j,k]), Quaternion(Quaternion)
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, pyo3::types::PyTuple>) -> pyo3::PyResult<Self> {
        let n = args.len();
        if n == 0 { return Ok(Self(Quaternion::new(1.0, usd_gf::vec3::Vec3d::default()))); }
        if n == 1 {
            let obj = args.get_item(0)?;
            // Copy constructor: Quaternion(Quaternion)
            if let Ok(q) = obj.extract::<pyo3::PyRef<'_, PyQuaternion>>() {
                return Ok(Self(q.0.clone()));
            }
            let real: f64 = obj.extract()?;
            return Ok(Self(Quaternion::new(real, usd_gf::vec3::Vec3d::default())));
        }
        if n == 2 {
            let real: f64 = args.get_item(0)?.extract()?;
            let im_obj = args.get_item(1)?;
            if let Ok(v) = im_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
                return Ok(Self(Quaternion::new(real, v.0)));
            }
            // Accept list/tuple of 3 floats as imaginary
            if let Ok(v) = im_obj.extract::<Vec<f64>>() {
                if v.len() == 3 {
                    return Ok(Self(Quaternion::new(real, usd_gf::vec3::Vec3d::new(v[0], v[1], v[2]))));
                }
            }
        }
        if n == 4 {
            let real: f64 = args.get_item(0)?.extract()?;
            let i: f64 = args.get_item(1)?.extract()?;
            let j: f64 = args.get_item(2)?.extract()?;
            let k: f64 = args.get_item(3)?.extract()?;
            return Ok(Self(Quaternion::new(real, usd_gf::vec3::Vec3d::new(i, j, k))));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("Quaternion: expected (), (real), (real, Vec3d), (real, [i,j,k]), or (real, i, j, k)"))
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
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=1e-10))]
    fn get_normalized(&self, eps: f64) -> Self { Self(self.0.normalized(eps)) }
    #[pyo3(name = "Normalize")]
    #[pyo3(signature = (eps=1e-10))]
    fn normalize(&mut self, eps: f64) -> Self { self.0.normalize(eps); self.clone() }
    #[pyo3(name = "GetInverse")]   fn get_inverse(&self) -> Self { Self(self.0.get_inverse()) }
    // Quaternion (legacy) has no conjugate method; implement manually
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quaternion::new(self.0.real(), usd_gf::vec3::Vec3d::new(-im.x, -im.y, -im.z)))
    }

    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(q) = o.extract::<pyo3::PyRef<'_, PyQuaternion>>() {
            return Ok(Self(self.0 * q.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            let im = self.0.imaginary();
            return Ok(Self(Quaternion::new(self.0.real() * s, usd_gf::vec3::Vec3d::new(im.x * s, im.y * s, im.z * s))).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self {
        let im = self.0.imaginary();
        Self(Quaternion::new(self.0.real() * s, usd_gf::vec3::Vec3d::new(im.x * s, im.y * s, im.z * s)))
    }
    fn __imul__(slf: Bound<'_, Self>, py: Python<'_>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(s) = o.extract::<f64>() {
            let mut m = slf.borrow_mut();
            let im = m.0.imaginary();
            m.0 = Quaternion::new(m.0.real() * s, usd_gf::vec3::Vec3d::new(im.x * s, im.y * s, im.z * s));
            return Ok(());
        }
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let mut m = slf.borrow_mut();
        m.0 = m.0 * rhs;
        let _ = py; Ok(())
    }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __iadd__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let mut m = slf.borrow_mut();
        m.0 = m.0 + rhs; Ok(())
    }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __isub__(slf: Bound<'_, Self>, o: Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        let rhs = { o.extract::<PyRef<'_, Self>>()?.0 };
        let mut m = slf.borrow_mut();
        m.0 = m.0 - rhs; Ok(())
    }
    fn __neg__(&self) -> Self {
        let im = self.0.imaginary();
        Self(Quaternion::new(-self.0.real(), usd_gf::vec3::Vec3d::new(-im.x, -im.y, -im.z)))
    }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        let im = self.0.imaginary();
        Ok(Self(Quaternion::new(self.0.real() / s, usd_gf::vec3::Vec3d::new(im.x / s, im.y / s, im.z / s))))
    }
    fn __itruediv__(&mut self, s: f64) -> PyResult<()> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        let im = self.0.imaginary();
        self.0 = Quaternion::new(self.0.real() / s, usd_gf::vec3::Vec3d::new(im.x / s, im.y / s, im.z / s));
        Ok(())
    }

    #[getter] fn real(&self) -> f64 { self.0.real() }
    #[setter(real)] fn set_real_prop(&mut self, v: f64) { self.0.set_real(v); }
    #[getter] fn imaginary(&self) -> super::vec::PyVec3d { super::vec::PyVec3d(*self.0.imaginary()) }
    #[setter(imaginary)] fn set_imaginary_prop(&mut self, v: &super::vec::PyVec3d) { self.0.set_imaginary(v.0); }
}

// ---------------------------------------------------------------------------
// DualQuatd
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object,name = "DualQuatd", module = "pxr_rs.Gf")]
#[derive(Clone)]
pub struct PyDualQuatd(pub DualQuatd);

#[pymethods]
impl PyDualQuatd {
    /// DualQuatd(), DualQuatd(real_scalar), DualQuatd(Quatd), DualQuatd(Quatd, Quatd),
    /// DualQuatd(Quatd, Vec3d) — translation form, DualQuatd(DualQuatd)
    #[new]
    #[pyo3(signature = (real=None, dual=None))]
    fn new(real: Option<&Bound<'_, pyo3::PyAny>>, dual: Option<&Bound<'_, pyo3::PyAny>>) -> pyo3::PyResult<Self> {
        let Some(r_obj) = real else { return Ok(Self(DualQuatd::identity())); };
        // DualQuatd(DualQuatd) — copy
        if let Ok(dq) = r_obj.extract::<pyo3::PyRef<'_, PyDualQuatd>>() {
            return Ok(Self(dq.0.clone()));
        }
        // DualQuatd(scalar)
        if let Ok(v) = r_obj.extract::<f64>() {
            let q = Quatd::from_components(v, 0.0, 0.0, 0.0);
            let mut dq = DualQuatd::from_real(q);
            if let Some(d_obj) = dual {
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuatd>>() { dq.set_dual(d.0); }
            }
            return Ok(Self(dq));
        }
        // DualQuatd(Quatd, ...) — from real quaternion
        if let Ok(q) = r_obj.extract::<pyo3::PyRef<'_, PyQuatd>>() {
            let mut dq = DualQuatd::from_real(q.0);
            if let Some(d_obj) = dual {
                // DualQuatd(Quatd, Quatd)
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuatd>>() {
                    dq.set_dual(d.0);
                }
                // DualQuatd(Quatd, Vec3d) — set translation
                else if let Ok(v) = d_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3d>>() {
                    dq.set_translation(&v.0);
                }
            }
            return Ok(Self(dq));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("DualQuatd: unsupported constructor"))
    }

    fn __repr__(&self) -> String { format!("Gf.DualQuatd(real={}, dual={})",
        self.0.real().real(), self.0.dual().real()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        let r = self.0.real(); let d = self.0.dual();
        let ri = r.imaginary(); let di = d.imaginary();
        hash_f64_2(r.real(), ri.x, ri.y, ri.z) ^ hash_f64_2(d.real(), di.x, di.y, di.z)
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuatd::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuatd::identity()) }

    #[pyo3(name = "GetReal")]    fn get_real(&self) -> PyQuatd { PyQuatd(*self.0.real()) }
    #[pyo3(name = "SetReal")]    fn set_real(&mut self, q: &PyQuatd) { self.0.set_real(q.0); }
    #[pyo3(name = "GetDual")]    fn get_dual(&self) -> PyQuatd { PyQuatd(*self.0.dual()) }
    #[pyo3(name = "SetDual")]    fn set_dual(&mut self, q: &PyQuatd) { self.0.set_dual(q.0); }
    // DualQuat::length() returns (T, T) — real and dual parts of the length
    #[pyo3(name = "GetLength")]  fn get_length(&self) -> (f64, f64) { self.0.length() }
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=1e-10))]
    fn get_normalized(&self, eps: f64) -> Self { Self(self.0.normalized_with_eps(eps)) }
    #[pyo3(name = "Normalize")]
    #[pyo3(signature = (eps=1e-10))]
    fn normalize(&mut self, eps: f64) { let _ = eps; self.0.normalize(); }
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

    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(dq) = o.extract::<pyo3::PyRef<'_, PyDualQuatd>>() {
            return Ok(Self(self.0 * dq.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f64>() {
            return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind());
        }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f64) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(dq) = o.extract::<pyo3::PyRef<'_, PyDualQuatd>>() {
            self.0 = self.0 * dq.0; return Ok(());
        }
        if let Ok(s) = o.extract::<f64>() {
            self.0 = self.0 * s; return Ok(());
        }
        let _ = py;
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(pyo3::exceptions::PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(self.0 * (1.0 / s)))
    }

    #[getter] fn real(&self) -> PyQuatd { PyQuatd(*self.0.real()) }
    #[setter(real)] fn set_real_val(&mut self, q: &PyQuatd) { self.0.set_real(q.0); }
    #[getter] fn dual(&self) -> PyQuatd { PyQuatd(*self.0.dual()) }
    #[setter(dual)] fn set_dual_val(&mut self, q: &PyQuatd) { self.0.set_dual(q.0); }
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
    #[pyo3(signature = (real=None, dual=None))]
    fn new(real: Option<&Bound<'_, pyo3::PyAny>>, dual: Option<&Bound<'_, pyo3::PyAny>>) -> pyo3::PyResult<Self> {
        let Some(r_obj) = real else { return Ok(Self(DualQuatf::identity())); };
        if let Ok(dq) = r_obj.extract::<pyo3::PyRef<'_, PyDualQuatf>>() { return Ok(Self(dq.0.clone())); }
        if let Ok(v) = r_obj.extract::<f32>() {
            let q = Quatf::from_components(v, 0.0, 0.0, 0.0);
            let mut dq = DualQuatf::from_real(q);
            if let Some(d_obj) = dual {
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuatf>>() { dq.set_dual(d.0); }
            }
            return Ok(Self(dq));
        }
        if let Ok(q) = r_obj.extract::<pyo3::PyRef<'_, PyQuatf>>() {
            let mut dq = DualQuatf::from_real(q.0);
            if let Some(d_obj) = dual {
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuatf>>() { dq.set_dual(d.0); }
                else if let Ok(v) = d_obj.extract::<pyo3::PyRef<'_, super::vec::PyVec3f>>() { dq.set_translation(&v.0); }
            }
            return Ok(Self(dq));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("DualQuatf: unsupported constructor"))
    }

    fn __repr__(&self) -> String { format!("Gf.DualQuatf(real={}, dual={})",
        self.0.real().real(), self.0.dual().real()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let r = self.0.real(); let d = self.0.dual();
        let ri = r.imaginary(); let di = d.imaginary();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        r.real().to_bits().hash(&mut h); ri.x.to_bits().hash(&mut h);
        ri.y.to_bits().hash(&mut h); ri.z.to_bits().hash(&mut h);
        d.real().to_bits().hash(&mut h); di.x.to_bits().hash(&mut h);
        di.y.to_bits().hash(&mut h); di.z.to_bits().hash(&mut h);
        h.finish()
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuatf::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuatf::identity()) }

    #[pyo3(name = "GetReal")]    fn get_real(&self) -> PyQuatf { PyQuatf(*self.0.real()) }
    #[pyo3(name = "SetReal")]    fn set_real(&mut self, q: &PyQuatf) { self.0.set_real(q.0); }
    #[pyo3(name = "GetDual")]    fn get_dual(&self) -> PyQuatf { PyQuatf(*self.0.dual()) }
    #[pyo3(name = "SetDual")]    fn set_dual(&mut self, q: &PyQuatf) { self.0.set_dual(q.0); }
    #[pyo3(name = "GetLength")]  fn get_length(&self) -> (f32, f32) { self.0.length() }
    #[pyo3(name = "GetNormalized")]
    #[pyo3(signature = (eps=1e-6))]
    fn get_normalized(&self, eps: f32) -> Self { Self(self.0.normalized_with_eps(eps)) }
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

    fn __mul__(&self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
        if let Ok(dq) = o.extract::<pyo3::PyRef<'_, PyDualQuatf>>() {
            return Ok(Self(self.0 * dq.0).into_pyobject(py)?.into_any().unbind());
        }
        if let Ok(s) = o.extract::<f32>() { return Ok(Self(self.0 * s).into_pyobject(py)?.into_any().unbind()); }
        Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *"))
    }
    fn __rmul__(&self, s: f32) -> Self { Self(self.0 * s) }
    fn __imul__(&mut self, py: Python<'_>, o: &Bound<'_, pyo3::PyAny>) -> PyResult<()> {
        if let Ok(dq) = o.extract::<pyo3::PyRef<'_, PyDualQuatf>>() { self.0 = self.0 * dq.0; return Ok(()); }
        if let Ok(s) = o.extract::<f32>() { self.0 = self.0 * s; return Ok(()); }
        let _ = py; Err(pyo3::exceptions::PyTypeError::new_err("unsupported operand type for *="))
    }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }

    #[getter] fn real(&self) -> PyQuatf { PyQuatf(*self.0.real()) }
    #[setter(real)] fn set_real_val(&mut self, q: &PyQuatf) { self.0.set_real(q.0); }
    #[getter] fn dual(&self) -> PyQuatf { PyQuatf(*self.0.dual()) }
    #[setter(dual)] fn set_dual_val(&mut self, q: &PyQuatf) { self.0.set_dual(q.0); }
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
    #[pyo3(signature = (real=None, dual=None))]
    fn new(real: Option<&Bound<'_, pyo3::PyAny>>, dual: Option<&Bound<'_, pyo3::PyAny>>) -> pyo3::PyResult<Self> {
        let Some(r_obj) = real else { return Ok(Self(DualQuath::identity())); };
        if let Ok(dq) = r_obj.extract::<pyo3::PyRef<'_, PyDualQuath>>() { return Ok(Self(dq.0.clone())); }
        if let Ok(v) = r_obj.extract::<f32>() {
            let q = Quath::from_components(Half::from_f32(v), Half::from_f32(0.0), Half::from_f32(0.0), Half::from_f32(0.0));
            let mut dq = DualQuath::from_real(q);
            if let Some(d_obj) = dual {
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuath>>() { dq.set_dual(d.0); }
            }
            return Ok(Self(dq));
        }
        if let Ok(q) = r_obj.extract::<pyo3::PyRef<'_, PyQuath>>() {
            let mut dq = DualQuath::from_real(q.0);
            if let Some(d_obj) = dual {
                if let Ok(d) = d_obj.extract::<pyo3::PyRef<'_, PyQuath>>() { dq.set_dual(d.0); }
            }
            return Ok(Self(dq));
        }
        Err(pyo3::exceptions::PyTypeError::new_err("DualQuath: unsupported constructor"))
    }

    fn __repr__(&self) -> String { format!("Gf.DualQuath(real={}, dual={})",
        self.0.real().real().to_f32(), self.0.dual().real().to_f32()) }
    fn __str__(&self) -> String { self.__repr__() }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let r = self.0.real(); let d = self.0.dual();
        let ri = r.imaginary(); let di = d.imaginary();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        r.real().bits().hash(&mut h); ri.x.bits().hash(&mut h);
        ri.y.bits().hash(&mut h); ri.z.bits().hash(&mut h);
        d.real().bits().hash(&mut h); di.x.bits().hash(&mut h);
        di.y.bits().hash(&mut h); di.z.bits().hash(&mut h);
        h.finish()
    }

    #[staticmethod] #[pyo3(name = "GetZero")]     fn get_zero() -> Self { Self(DualQuath::zero()) }
    #[staticmethod] #[pyo3(name = "GetIdentity")] fn get_identity() -> Self { Self(DualQuath::identity()) }

    #[pyo3(name = "GetReal")]  fn get_real(&self) -> PyQuath { PyQuath(*self.0.real()) }
    #[pyo3(name = "SetReal")]  fn set_real(&mut self, q: &PyQuath) { self.0.set_real(q.0); }
    #[pyo3(name = "GetDual")]  fn get_dual(&self) -> PyQuath { PyQuath(*self.0.dual()) }
    #[pyo3(name = "SetDual")]  fn set_dual(&mut self, q: &PyQuath) { self.0.set_dual(q.0); }
    #[pyo3(name = "GetLength")] fn get_length(&self) -> (f32, f32) {
        let (a, b) = self.0.length();
        (a.to_f32(), b.to_f32())
    }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self { Self(self.0.normalized()) }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) { self.0.normalize(); }
    #[pyo3(name = "GetConjugate")] fn get_conjugate(&self) -> Self { Self(self.0.conjugate()) }
    #[pyo3(name = "GetInverse")] fn get_inverse(&self) -> Self { Self(self.0.inverse()) }

    fn __mul__(&self, o: &Self) -> Self { Self(self.0 * o.0) }
    fn __add__(&self, o: &Self) -> Self { Self(self.0 + o.0) }
    fn __sub__(&self, o: &Self) -> Self { Self(self.0 - o.0) }

    #[getter] fn real(&self) -> PyQuath { PyQuath(*self.0.real()) }
    #[setter(real)] fn set_real_val(&mut self, q: &PyQuath) { self.0.set_real(q.0); }
    #[getter] fn dual(&self) -> PyQuath { PyQuath(*self.0.dual()) }
    #[setter(dual)] fn set_dual_val(&mut self, q: &PyQuath) { self.0.set_dual(q.0); }
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
