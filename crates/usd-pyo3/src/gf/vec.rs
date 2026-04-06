//! Vec2/3/4 Python bindings (all scalar variants: d/f/h/i).
//!
//! Macro-driven to reduce repetition. Each vec type gets the same interface
//! with scalar-appropriate implementations. Half-precision uses f32 at the Python
//! boundary since Python has no native f16.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIndexError, PyZeroDivisionError, PyValueError};
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

// ---------------------------------------------------------------------------
// Vec2d
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec2d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec2d(pub usd_gf::Vec2d);

#[pymethods]
impl PyVec2d {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0))]
    fn new(x: f64, y: f64) -> Self { Self(usd_gf::Vec2d::new(x, y)) }

    fn __repr__(&self) -> String { format!("Gf.Vec2d({}, {})", self.0.x, self.0.y) }
    fn __str__(&self) -> String  { format!("({}, {})", self.0.x, self.0.y) }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, other: &Self) -> bool { self.0 == other.0 }
    fn __ne__(&self, other: &Self) -> bool { self.0 != other.0 }
    fn __hash__(&self) -> u64 { hash2(self.0.x.to_bits(), self.0.y.to_bits()) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec2d::new(-self.0.x, -self.0.y)) }

    fn __getitem__(&self, i: isize) -> PyResult<f64> {
        match norm_idx(i, 2)? { 0 => Ok(self.0.x), 1 => Ok(self.0.y), _ => unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f64) -> PyResult<()> {
        match norm_idx(i, 2)? { 0 => self.0.x = v, 1 => self.0.y = v, _ => unreachable!() }
        Ok(())
    }
    fn __contains__(&self, v: f64) -> bool { self.0.x == v || self.0.y == v }

    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec2d::new(self.0.x+o.0.x, self.0.y+o.0.y)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec2d::new(self.0.x-o.0.x, self.0.y-o.0.y)) }
    fn __mul__(&self, s: f64) -> Self { Self(usd_gf::Vec2d::new(self.0.x*s, self.0.y*s)) }
    fn __rmul__(&self, s: f64) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec2d::new(self.0.x/s, self.0.y/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec2d::new(1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec2d::new(0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0 => Ok(Self::x_axis()), 1 => Ok(Self::y_axis()),
            _ => Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")] fn get_length(&self) -> f64 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 { Self(usd_gf::Vec2d::new(self.0.x/l, self.0.y/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f64 {
        let l = self.0.length(); if l > 0.0 { self.0.x /= l; self.0.y /= l; } l
    }
    #[pyo3(name = "GetDot")] fn get_dot(&self, o: &Self) -> f64 { self.0.dot(&o.0) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 { return Self(usd_gf::Vec2d::new(0.0,0.0)); }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec2d::new(onto.0.x*s, onto.0.y*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec2d::new(self.0.x-p.0.x, self.0.y-p.0.y))
    }

    #[getter] fn dimension(&self) -> usize { 2 }
}

// ---------------------------------------------------------------------------
// Vec2f
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec2f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec2f(pub usd_gf::Vec2f);

#[pymethods]
impl PyVec2f {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0))]
    fn new(x: f32, y: f32) -> Self { Self(usd_gf::Vec2f::new(x, y)) }

    fn __repr__(&self) -> String { format!("Gf.Vec2f({}, {})", self.0.x, self.0.y) }
    fn __str__(&self) -> String  { format!("({}, {})", self.0.x, self.0.y) }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash2(self.0.x.to_bits() as u64, self.0.y.to_bits() as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec2f::new(-self.0.x, -self.0.y)) }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,2)? { 0=>Ok(self.0.x), 1=>Ok(self.0.y), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,2)? { 0=>self.0.x=v, 1=>self.0.y=v, _=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: f32) -> bool { self.0.x==v || self.0.y==v }

    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec2f::new(self.0.x+o.0.x, self.0.y+o.0.y)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec2f::new(self.0.x-o.0.x, self.0.y-o.0.y)) }
    fn __mul__(&self, s: f32) -> Self { Self(usd_gf::Vec2f::new(self.0.x*s, self.0.y*s)) }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec2f::new(self.0.x/s, self.0.y/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec2f::new(1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec2f::new(0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0=>Ok(Self::x_axis()), 1=>Ok(Self::y_axis()),
            _=>Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")] fn get_length(&self) -> f32 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 { Self(usd_gf::Vec2f::new(self.0.x/l, self.0.y/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f32 {
        let l = self.0.length(); if l > 0.0 { self.0.x/=l; self.0.y/=l; } l
    }
    #[pyo3(name = "GetDot")] fn get_dot(&self, o: &Self) -> f32 { self.0.dot(&o.0) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d == 0.0 { return Self(usd_gf::Vec2f::new(0.0,0.0)); }
        let s = self.0.dot(&onto.0) / d;
        Self(usd_gf::Vec2f::new(onto.0.x*s, onto.0.y*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec2f::new(self.0.x-p.0.x, self.0.y-p.0.y))
    }

    #[getter] fn dimension(&self) -> usize { 2 }
}

// ---------------------------------------------------------------------------
// Vec2h (half-precision — Python boundary uses f32)
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec2h", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec2h(pub usd_gf::Vec2h);

#[pymethods]
impl PyVec2h {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0))]
    fn new(x: f32, y: f32) -> Self {
        Self(usd_gf::Vec2h::new(Half::from_f32(x), Half::from_f32(y)))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec2h({}, {})", self.0.x.to_f32(), self.0.y.to_f32())
    }
    fn __str__(&self) -> String { format!("({}, {})", self.0.x.to_f32(), self.0.y.to_f32()) }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 {
        // Half uses .bits() not .to_bits()
        hash2(self.0.x.bits() as u64, self.0.y.bits() as u64)
    }
    fn __neg__(&self) -> Self {
        Self(usd_gf::Vec2h::new(-self.0.x, -self.0.y))
    }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,2)? { 0=>Ok(self.0.x.to_f32()), 1=>Ok(self.0.y.to_f32()), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,2)? {
            0=>self.0.x=Half::from_f32(v),
            1=>self.0.y=Half::from_f32(v),
            _=>unreachable!()
        }
        Ok(())
    }

    fn __add__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2h::new(self.0.x+o.0.x, self.0.y+o.0.y))
    }
    fn __sub__(&self, o: &Self) -> Self {
        Self(usd_gf::Vec2h::new(self.0.x-o.0.x, self.0.y-o.0.y))
    }
    fn __mul__(&self, s: f32) -> Self {
        let hs = Half::from_f32(s);
        Self(usd_gf::Vec2h::new(self.0.x*hs, self.0.y*hs))
    }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s == 0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        let hs = Half::from_f32(s);
        Ok(Self(usd_gf::Vec2h::new(self.0.x/hs, self.0.y/hs)))
    }

    #[getter] fn dimension(&self) -> usize { 2 }
}

// ---------------------------------------------------------------------------
// Vec2i
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec2i", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec2i(pub usd_gf::Vec2i);

#[pymethods]
impl PyVec2i {
    #[new]
    #[pyo3(signature = (x=0, y=0))]
    fn new(x: i32, y: i32) -> Self { Self(usd_gf::Vec2i::new(x, y)) }

    fn __repr__(&self) -> String { format!("Gf.Vec2i({}, {})", self.0.x, self.0.y) }
    fn __str__(&self) -> String  { format!("({}, {})", self.0.x, self.0.y) }
    fn __len__(&self) -> usize { 2 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash2(self.0.x as u64, self.0.y as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec2i::new(-self.0.x, -self.0.y)) }

    fn __getitem__(&self, i: isize) -> PyResult<i32> {
        match norm_idx(i,2)? { 0=>Ok(self.0.x), 1=>Ok(self.0.y), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: i32) -> PyResult<()> {
        match norm_idx(i,2)? { 0=>self.0.x=v, 1=>self.0.y=v, _=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: i32) -> bool { self.0.x==v || self.0.y==v }

    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec2i::new(self.0.x+o.0.x, self.0.y+o.0.y)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec2i::new(self.0.x-o.0.x, self.0.y-o.0.y)) }
    fn __mul__(&self, s: i32) -> Self { Self(usd_gf::Vec2i::new(self.0.x*s, self.0.y*s)) }
    fn __rmul__(&self, s: i32) -> Self { self.__mul__(s) }

    #[getter] fn dimension(&self) -> usize { 2 }
}

// ---------------------------------------------------------------------------
// Vec3d
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec3d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec3d(pub usd_gf::Vec3d);

#[pymethods]
impl PyVec3d {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0))]
    fn new(x: f64, y: f64, z: f64) -> Self { Self(usd_gf::Vec3d::new(x, y, z)) }

    fn __repr__(&self) -> String { format!("Gf.Vec3d({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __str__(&self) -> String  { format!("({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash3(self.0.x.to_bits(), self.0.y.to_bits(), self.0.z.to_bits()) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec3d::new(-self.0.x, -self.0.y, -self.0.z)) }

    fn __getitem__(&self, i: isize) -> PyResult<f64> {
        match norm_idx(i,3)? { 0=>Ok(self.0.x), 1=>Ok(self.0.y), 2=>Ok(self.0.z), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f64) -> PyResult<()> {
        match norm_idx(i,3)? { 0=>self.0.x=v, 1=>self.0.y=v, 2=>self.0.z=v, _=>unreachable!() }
        Ok(())
    }
    fn __contains__(&self, v: f64) -> bool { self.0.x==v || self.0.y==v || self.0.z==v }
    fn __xor__(&self, o: &Self) -> Self { Self(self.0.cross(&o.0)) }

    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec3d::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec3d::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z)) }
    fn __mul__(&self, s: f64) -> Self { Self(usd_gf::Vec3d::new(self.0.x*s,self.0.y*s,self.0.z*s)) }
    fn __rmul__(&self, s: f64) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec3d::new(self.0.x/s,self.0.y/s,self.0.z/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec3d::new(1.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec3d::new(0.0,1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "ZAxis")] fn z_axis() -> Self { Self(usd_gf::Vec3d::new(0.0,0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0=>Ok(Self::x_axis()), 1=>Ok(Self::y_axis()), 2=>Ok(Self::z_axis()),
            _=>Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f64 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l = self.0.length();
        if l > 0.0 { Self(usd_gf::Vec3d::new(self.0.x/l,self.0.y/l,self.0.z/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f64 {
        let l = self.0.length(); if l>0.0 { self.0.x/=l; self.0.y/=l; self.0.z/=l; } l
    }
    #[pyo3(name = "GetDot")]        fn get_dot(&self, o: &Self) -> f64 { self.0.dot(&o.0) }
    #[pyo3(name = "GetCross")]      fn get_cross(&self, o: &Self) -> Self { Self(self.0.cross(&o.0)) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d = onto.0.dot(&onto.0);
        if d==0.0 { return Self(usd_gf::Vec3d::new(0.0,0.0,0.0)); }
        let s = self.0.dot(&onto.0)/d;
        Self(usd_gf::Vec3d::new(onto.0.x*s,onto.0.y*s,onto.0.z*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p = self.get_projection(onto);
        Self(usd_gf::Vec3d::new(self.0.x-p.0.x,self.0.y-p.0.y,self.0.z-p.0.z))
    }

    #[getter] fn dimension(&self) -> usize { 3 }
}

// ---------------------------------------------------------------------------
// Vec3f
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec3f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec3f(pub usd_gf::Vec3f);

#[pymethods]
impl PyVec3f {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0))]
    fn new(x: f32, y: f32, z: f32) -> Self { Self(usd_gf::Vec3f::new(x, y, z)) }

    fn __repr__(&self) -> String { format!("Gf.Vec3f({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __str__(&self) -> String  { format!("({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash3(self.0.x.to_bits() as u64, self.0.y.to_bits() as u64, self.0.z.to_bits() as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec3f::new(-self.0.x,-self.0.y,-self.0.z)) }
    fn __xor__(&self, o: &Self) -> Self { Self(self.0.cross(&o.0)) }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,3)? { 0=>Ok(self.0.x), 1=>Ok(self.0.y), 2=>Ok(self.0.z), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,3)? { 0=>self.0.x=v, 1=>self.0.y=v, 2=>self.0.z=v, _=>unreachable!() }
        Ok(())
    }
    fn __contains__(&self, v: f32) -> bool { self.0.x==v || self.0.y==v || self.0.z==v }

    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec3f::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec3f::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z)) }
    fn __mul__(&self, s: f32) -> Self { Self(usd_gf::Vec3f::new(self.0.x*s,self.0.y*s,self.0.z*s)) }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec3f::new(self.0.x/s,self.0.y/s,self.0.z/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec3f::new(1.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec3f::new(0.0,1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "ZAxis")] fn z_axis() -> Self { Self(usd_gf::Vec3f::new(0.0,0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0=>Ok(Self::x_axis()), 1=>Ok(Self::y_axis()), 2=>Ok(Self::z_axis()),
            _=>Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f32 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l=self.0.length();
        if l>0.0 { Self(usd_gf::Vec3f::new(self.0.x/l,self.0.y/l,self.0.z/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f32 {
        let l=self.0.length(); if l>0.0 { self.0.x/=l; self.0.y/=l; self.0.z/=l; } l
    }
    #[pyo3(name = "GetDot")]    fn get_dot(&self, o: &Self) -> f32 { self.0.dot(&o.0) }
    #[pyo3(name = "GetCross")]  fn get_cross(&self, o: &Self) -> Self { Self(self.0.cross(&o.0)) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d=onto.0.dot(&onto.0);
        if d==0.0 { return Self(usd_gf::Vec3f::new(0.0,0.0,0.0)); }
        let s=self.0.dot(&onto.0)/d;
        Self(usd_gf::Vec3f::new(onto.0.x*s,onto.0.y*s,onto.0.z*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p=self.get_projection(onto);
        Self(usd_gf::Vec3f::new(self.0.x-p.0.x,self.0.y-p.0.y,self.0.z-p.0.z))
    }

    #[getter] fn dimension(&self) -> usize { 3 }
}

// ---------------------------------------------------------------------------
// Vec3h
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec3h", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec3h(pub usd_gf::Vec3h);

#[pymethods]
impl PyVec3h {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0))]
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self(usd_gf::Vec3h::new(Half::from_f32(x), Half::from_f32(y), Half::from_f32(z)))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec3h({}, {}, {})", self.0.x.to_f32(), self.0.y.to_f32(), self.0.z.to_f32())
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {})", self.0.x.to_f32(), self.0.y.to_f32(), self.0.z.to_f32())
    }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash3(self.0.x.bits() as u64, self.0.y.bits() as u64, self.0.z.bits() as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec3h::new(-self.0.x,-self.0.y,-self.0.z)) }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,3)? {
            0=>Ok(self.0.x.to_f32()), 1=>Ok(self.0.y.to_f32()), 2=>Ok(self.0.z.to_f32()), _=>unreachable!()
        }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,3)? {
            0=>self.0.x=Half::from_f32(v), 1=>self.0.y=Half::from_f32(v), 2=>self.0.z=Half::from_f32(v), _=>unreachable!()
        }
        Ok(())
    }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec3h::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec3h::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z)) }
    fn __mul__(&self, s: f32) -> Self {
        let hs=Half::from_f32(s);
        Self(usd_gf::Vec3h::new(self.0.x*hs,self.0.y*hs,self.0.z*hs))
    }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        let hs=Half::from_f32(s);
        Ok(Self(usd_gf::Vec3h::new(self.0.x/hs,self.0.y/hs,self.0.z/hs)))
    }

    #[getter] fn dimension(&self) -> usize { 3 }
}

// ---------------------------------------------------------------------------
// Vec3i
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec3i", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec3i(pub usd_gf::Vec3i);

#[pymethods]
impl PyVec3i {
    #[new]
    #[pyo3(signature = (x=0, y=0, z=0))]
    fn new(x: i32, y: i32, z: i32) -> Self { Self(usd_gf::Vec3i::new(x, y, z)) }

    fn __repr__(&self) -> String { format!("Gf.Vec3i({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __str__(&self) -> String  { format!("({}, {}, {})", self.0.x, self.0.y, self.0.z) }
    fn __len__(&self) -> usize { 3 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash3(self.0.x as u64, self.0.y as u64, self.0.z as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec3i::new(-self.0.x,-self.0.y,-self.0.z)) }

    fn __getitem__(&self, i: isize) -> PyResult<i32> {
        match norm_idx(i,3)? { 0=>Ok(self.0.x), 1=>Ok(self.0.y), 2=>Ok(self.0.z), _=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: i32) -> PyResult<()> {
        match norm_idx(i,3)? { 0=>self.0.x=v, 1=>self.0.y=v, 2=>self.0.z=v, _=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: i32) -> bool { self.0.x==v || self.0.y==v || self.0.z==v }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec3i::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec3i::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z)) }
    fn __mul__(&self, s: i32) -> Self { Self(usd_gf::Vec3i::new(self.0.x*s,self.0.y*s,self.0.z*s)) }
    fn __rmul__(&self, s: i32) -> Self { self.__mul__(s) }

    #[getter] fn dimension(&self) -> usize { 3 }
}

// ---------------------------------------------------------------------------
// Vec4d
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec4d", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec4d(pub usd_gf::Vec4d);

#[pymethods]
impl PyVec4d {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0, w=0.0))]
    fn new(x: f64, y: f64, z: f64, w: f64) -> Self { Self(usd_gf::Vec4d::new(x,y,z,w)) }

    fn __repr__(&self) -> String { format!("Gf.Vec4d({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __str__(&self) -> String  { format!("({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash4(self.0.x.to_bits(),self.0.y.to_bits(),self.0.z.to_bits(),self.0.w.to_bits()) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec4d::new(-self.0.x,-self.0.y,-self.0.z,-self.0.w)) }

    fn __getitem__(&self, i: isize) -> PyResult<f64> {
        match norm_idx(i,4)? { 0=>Ok(self.0.x),1=>Ok(self.0.y),2=>Ok(self.0.z),3=>Ok(self.0.w),_=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f64) -> PyResult<()> {
        match norm_idx(i,4)? { 0=>self.0.x=v,1=>self.0.y=v,2=>self.0.z=v,3=>self.0.w=v,_=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: f64) -> bool { self.0.x==v||self.0.y==v||self.0.z==v||self.0.w==v }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec4d::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z,self.0.w+o.0.w)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec4d::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z,self.0.w-o.0.w)) }
    fn __mul__(&self, s: f64) -> Self { Self(usd_gf::Vec4d::new(self.0.x*s,self.0.y*s,self.0.z*s,self.0.w*s)) }
    fn __rmul__(&self, s: f64) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f64) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec4d::new(self.0.x/s,self.0.y/s,self.0.z/s,self.0.w/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec4d::new(1.0,0.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec4d::new(0.0,1.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "ZAxis")] fn z_axis() -> Self { Self(usd_gf::Vec4d::new(0.0,0.0,1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "WAxis")] fn w_axis() -> Self { Self(usd_gf::Vec4d::new(0.0,0.0,0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0=>Ok(Self::x_axis()),1=>Ok(Self::y_axis()),2=>Ok(Self::z_axis()),3=>Ok(Self::w_axis()),
            _=>Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f64 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l=self.0.length();
        if l>0.0 { Self(usd_gf::Vec4d::new(self.0.x/l,self.0.y/l,self.0.z/l,self.0.w/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f64 {
        let l=self.0.length(); if l>0.0 { self.0.x/=l;self.0.y/=l;self.0.z/=l;self.0.w/=l; } l
    }
    #[pyo3(name = "GetDot")] fn get_dot(&self, o: &Self) -> f64 { self.0.dot(&o.0) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d=onto.0.dot(&onto.0);
        if d==0.0 { return Self(usd_gf::Vec4d::new(0.0,0.0,0.0,0.0)); }
        let s=self.0.dot(&onto.0)/d;
        Self(usd_gf::Vec4d::new(onto.0.x*s,onto.0.y*s,onto.0.z*s,onto.0.w*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p=self.get_projection(onto);
        Self(usd_gf::Vec4d::new(self.0.x-p.0.x,self.0.y-p.0.y,self.0.z-p.0.z,self.0.w-p.0.w))
    }

    #[getter] fn dimension(&self) -> usize { 4 }
}

// ---------------------------------------------------------------------------
// Vec4f
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec4f", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec4f(pub usd_gf::Vec4f);

#[pymethods]
impl PyVec4f {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0, w=0.0))]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self { Self(usd_gf::Vec4f::new(x,y,z,w)) }

    fn __repr__(&self) -> String { format!("Gf.Vec4f({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __str__(&self) -> String  { format!("({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash4(self.0.x.to_bits() as u64,self.0.y.to_bits() as u64,self.0.z.to_bits() as u64,self.0.w.to_bits() as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec4f::new(-self.0.x,-self.0.y,-self.0.z,-self.0.w)) }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,4)? { 0=>Ok(self.0.x),1=>Ok(self.0.y),2=>Ok(self.0.z),3=>Ok(self.0.w),_=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,4)? { 0=>self.0.x=v,1=>self.0.y=v,2=>self.0.z=v,3=>self.0.w=v,_=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: f32) -> bool { self.0.x==v||self.0.y==v||self.0.z==v||self.0.w==v }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec4f::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z,self.0.w+o.0.w)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec4f::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z,self.0.w-o.0.w)) }
    fn __mul__(&self, s: f32) -> Self { Self(usd_gf::Vec4f::new(self.0.x*s,self.0.y*s,self.0.z*s,self.0.w*s)) }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        Ok(Self(usd_gf::Vec4f::new(self.0.x/s,self.0.y/s,self.0.z/s,self.0.w/s)))
    }

    #[staticmethod] #[pyo3(name = "XAxis")] fn x_axis() -> Self { Self(usd_gf::Vec4f::new(1.0,0.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "YAxis")] fn y_axis() -> Self { Self(usd_gf::Vec4f::new(0.0,1.0,0.0,0.0)) }
    #[staticmethod] #[pyo3(name = "ZAxis")] fn z_axis() -> Self { Self(usd_gf::Vec4f::new(0.0,0.0,1.0,0.0)) }
    #[staticmethod] #[pyo3(name = "WAxis")] fn w_axis() -> Self { Self(usd_gf::Vec4f::new(0.0,0.0,0.0,1.0)) }
    #[staticmethod] #[pyo3(name = "Axis")]
    fn axis(i: usize) -> PyResult<Self> {
        match i { 0=>Ok(Self::x_axis()),1=>Ok(Self::y_axis()),2=>Ok(Self::z_axis()),3=>Ok(Self::w_axis()),
            _=>Err(PyValueError::new_err("axis index out of range")) }
    }

    #[pyo3(name = "GetLength")]    fn get_length(&self) -> f32 { self.0.length() }
    #[pyo3(name = "GetNormalized")] fn get_normalized(&self) -> Self {
        let l=self.0.length();
        if l>0.0 { Self(usd_gf::Vec4f::new(self.0.x/l,self.0.y/l,self.0.z/l,self.0.w/l)) } else { self.clone() }
    }
    #[pyo3(name = "Normalize")] fn normalize(&mut self) -> f32 {
        let l=self.0.length(); if l>0.0 { self.0.x/=l;self.0.y/=l;self.0.z/=l;self.0.w/=l; } l
    }
    #[pyo3(name = "GetDot")] fn get_dot(&self, o: &Self) -> f32 { self.0.dot(&o.0) }
    #[pyo3(name = "GetProjection")] fn get_projection(&self, onto: &Self) -> Self {
        let d=onto.0.dot(&onto.0);
        if d==0.0 { return Self(usd_gf::Vec4f::new(0.0,0.0,0.0,0.0)); }
        let s=self.0.dot(&onto.0)/d;
        Self(usd_gf::Vec4f::new(onto.0.x*s,onto.0.y*s,onto.0.z*s,onto.0.w*s))
    }
    #[pyo3(name = "GetComplement")] fn get_complement(&self, onto: &Self) -> Self {
        let p=self.get_projection(onto);
        Self(usd_gf::Vec4f::new(self.0.x-p.0.x,self.0.y-p.0.y,self.0.z-p.0.z,self.0.w-p.0.w))
    }

    #[getter] fn dimension(&self) -> usize { 4 }
}

// ---------------------------------------------------------------------------
// Vec4h
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec4h", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec4h(pub usd_gf::Vec4h);

#[pymethods]
impl PyVec4h {
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0, z=0.0, w=0.0))]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(usd_gf::Vec4h::new(Half::from_f32(x),Half::from_f32(y),Half::from_f32(z),Half::from_f32(w)))
    }

    fn __repr__(&self) -> String {
        format!("Gf.Vec4h({}, {}, {}, {})", self.0.x.to_f32(),self.0.y.to_f32(),self.0.z.to_f32(),self.0.w.to_f32())
    }
    fn __str__(&self) -> String {
        format!("({}, {}, {}, {})", self.0.x.to_f32(),self.0.y.to_f32(),self.0.z.to_f32(),self.0.w.to_f32())
    }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash4(self.0.x.bits() as u64,self.0.y.bits() as u64,self.0.z.bits() as u64,self.0.w.bits() as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec4h::new(-self.0.x,-self.0.y,-self.0.z,-self.0.w)) }

    fn __getitem__(&self, i: isize) -> PyResult<f32> {
        match norm_idx(i,4)? { 0=>Ok(self.0.x.to_f32()),1=>Ok(self.0.y.to_f32()),2=>Ok(self.0.z.to_f32()),3=>Ok(self.0.w.to_f32()),_=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: f32) -> PyResult<()> {
        match norm_idx(i,4)? {
            0=>self.0.x=Half::from_f32(v), 1=>self.0.y=Half::from_f32(v),
            2=>self.0.z=Half::from_f32(v), 3=>self.0.w=Half::from_f32(v), _=>unreachable!()
        }
        Ok(())
    }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec4h::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z,self.0.w+o.0.w)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec4h::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z,self.0.w-o.0.w)) }
    fn __mul__(&self, s: f32) -> Self {
        let hs=Half::from_f32(s);
        Self(usd_gf::Vec4h::new(self.0.x*hs,self.0.y*hs,self.0.z*hs,self.0.w*hs))
    }
    fn __rmul__(&self, s: f32) -> Self { self.__mul__(s) }
    fn __truediv__(&self, s: f32) -> PyResult<Self> {
        if s==0.0 { return Err(PyZeroDivisionError::new_err("division by zero")); }
        let hs=Half::from_f32(s);
        Ok(Self(usd_gf::Vec4h::new(self.0.x/hs,self.0.y/hs,self.0.z/hs,self.0.w/hs)))
    }

    #[getter] fn dimension(&self) -> usize { 4 }
}

// ---------------------------------------------------------------------------
// Vec4i
// ---------------------------------------------------------------------------

#[pyclass(name = "Vec4i", module = "pxr.Gf")]
#[derive(Clone)]
pub struct PyVec4i(pub usd_gf::Vec4i);

#[pymethods]
impl PyVec4i {
    #[new]
    #[pyo3(signature = (x=0, y=0, z=0, w=0))]
    fn new(x: i32, y: i32, z: i32, w: i32) -> Self { Self(usd_gf::Vec4i::new(x,y,z,w)) }

    fn __repr__(&self) -> String { format!("Gf.Vec4i({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __str__(&self) -> String  { format!("({}, {}, {}, {})", self.0.x,self.0.y,self.0.z,self.0.w) }
    fn __len__(&self) -> usize { 4 }
    fn __eq__(&self, o: &Self) -> bool { self.0 == o.0 }
    fn __ne__(&self, o: &Self) -> bool { self.0 != o.0 }
    fn __hash__(&self) -> u64 { hash4(self.0.x as u64,self.0.y as u64,self.0.z as u64,self.0.w as u64) }
    fn __neg__(&self) -> Self { Self(usd_gf::Vec4i::new(-self.0.x,-self.0.y,-self.0.z,-self.0.w)) }

    fn __getitem__(&self, i: isize) -> PyResult<i32> {
        match norm_idx(i,4)? { 0=>Ok(self.0.x),1=>Ok(self.0.y),2=>Ok(self.0.z),3=>Ok(self.0.w),_=>unreachable!() }
    }
    fn __setitem__(&mut self, i: isize, v: i32) -> PyResult<()> {
        match norm_idx(i,4)? { 0=>self.0.x=v,1=>self.0.y=v,2=>self.0.z=v,3=>self.0.w=v,_=>unreachable!() } Ok(())
    }
    fn __contains__(&self, v: i32) -> bool { self.0.x==v||self.0.y==v||self.0.z==v||self.0.w==v }
    fn __add__(&self, o: &Self) -> Self { Self(usd_gf::Vec4i::new(self.0.x+o.0.x,self.0.y+o.0.y,self.0.z+o.0.z,self.0.w+o.0.w)) }
    fn __sub__(&self, o: &Self) -> Self { Self(usd_gf::Vec4i::new(self.0.x-o.0.x,self.0.y-o.0.y,self.0.z-o.0.z,self.0.w-o.0.w)) }
    fn __mul__(&self, s: i32) -> Self { Self(usd_gf::Vec4i::new(self.0.x*s,self.0.y*s,self.0.z*s,self.0.w*s)) }
    fn __rmul__(&self, s: i32) -> Self { self.__mul__(s) }

    #[getter] fn dimension(&self) -> usize { 4 }
}

// ---------------------------------------------------------------------------
// Internal hash helpers
// ---------------------------------------------------------------------------

fn hash2(a: u64, b: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h); b.hash(&mut h); h.finish()
}

fn hash3(a: u64, b: u64, c: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h); b.hash(&mut h); c.hash(&mut h); h.finish()
}

fn hash4(a: u64, b: u64, c: u64, d: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h); b.hash(&mut h); c.hash(&mut h); d.hash(&mut h); h.finish()
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
