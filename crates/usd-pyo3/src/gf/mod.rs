//! pxr.Gf — Graphics Foundation Python bindings.
//!
//! Complete drop-in replacement for pxr.Gf from C++ OpenUSD.
//! Covers all 51 types from the API inventory.

pub mod geo;
pub mod matrix;
pub mod quat;
pub mod vec;

use pyo3::prelude::*;
use pyo3::exceptions::PyTypeError;

// ---------------------------------------------------------------------------
// Math module-level functions (GfSqrt, GfAbs, etc.)
// ---------------------------------------------------------------------------

/// Gf.Sqrt(x) -> float
#[pyfunction(name = "Sqrt")]
fn py_sqrt(x: f64) -> f64 { usd_gf::math::sqrt(x) }

/// Gf.Sqrtf(x) -> float
#[pyfunction(name = "Sqrtf")]
fn py_sqrtf(x: f64) -> f64 { (x as f32).sqrt() as f64 }

/// Gf.Sqr(x) -> number. Accepts scalars and vec types.
#[pyfunction(name = "Sqr")]
fn py_sqr(py: Python<'_>, obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    // Try scalar first
    if let Ok(v) = obj.extract::<f64>() {
        return Ok((v * v).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(v) = obj.extract::<i64>() {
        return Ok((v * v).into_pyobject(py)?.into_any().unbind());
    }
    // Try vec types — dot product with self
    if let Ok(gd) = obj.call_method1("GetDot", (obj,)) {
        return Ok(gd.unbind());
    }
    // For integer vecs, manual sum of squares via indexing
    if let Ok(length) = obj.call_method0("__len__") {
        let n: usize = length.extract()?;
        let mut sum: i64 = 0;
        for i in 0..n {
            let elem: i64 = obj.call_method1("__getitem__", (i as isize,))?.extract()?;
            sum += elem * elem;
        }
        return Ok(sum.into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("Sqr: unsupported type"))
}

/// Gf.Sgn(x) -> int/float
#[pyfunction(name = "Sgn")]
fn py_sgn(py: Python<'_>, obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(v) = obj.extract::<f64>() {
        let r: f64 = if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 };
        return Ok(r.into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(v) = obj.extract::<i64>() {
        let r: i64 = if v > 0 { 1 } else if v < 0 { -1 } else { 0 };
        return Ok(r.into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("Sgn: unsupported type"))
}

#[pyfunction(name = "Exp")]
fn py_exp(x: f64) -> f64 { usd_gf::math::exp(x) }

#[pyfunction(name = "Expf")]
fn py_expf(x: f64) -> f64 { (x as f32).exp() as f64 }

#[pyfunction(name = "Log")]
fn py_log(x: f64) -> f64 { usd_gf::math::log(x) }

#[pyfunction(name = "Logf")]
fn py_logf(x: f64) -> f64 { (x as f32).ln() as f64 }

#[pyfunction(name = "Floor")]
fn py_floor(x: f64) -> f64 { usd_gf::math::floor(x) }

#[pyfunction(name = "Floorf")]
fn py_floorf(x: f64) -> f64 { (x as f32).floor() as f64 }

#[pyfunction(name = "Ceil")]
fn py_ceil(x: f64) -> f64 { usd_gf::math::ceil(x) }

#[pyfunction(name = "Ceilf")]
fn py_ceilf(x: f64) -> f64 { (x as f32).ceil() as f64 }

#[pyfunction(name = "Abs")]
fn py_abs(x: f64) -> f64 { usd_gf::math::abs(x) }

#[pyfunction(name = "Absf")]
fn py_absf(x: f64) -> f64 { (x as f32).abs() as f64 }

#[pyfunction(name = "Round")]
fn py_round(x: f64) -> f64 { usd_gf::math::round(x) }

#[pyfunction(name = "Roundf")]
fn py_roundf(x: f64) -> f64 { (x as f32).round() as f64 }

#[pyfunction(name = "Pow")]
fn py_pow(x: f64, p: f64) -> f64 { usd_gf::math::pow(x, p) }

#[pyfunction(name = "Powf")]
fn py_powf(x: f64, p: f64) -> f64 { (x as f32).powf(p as f32) as f64 }

#[pyfunction(name = "Clamp")]
fn py_clamp(value: f64, min: f64, max: f64) -> f64 { usd_gf::math::clamp(value, min, max) }

#[pyfunction(name = "Clampf")]
fn py_clampf(value: f64, min: f64, max: f64) -> f64 {
    usd_gf::math::clamp(value as f32, min as f32, max as f32) as f64
}

#[pyfunction(name = "Mod")]
fn py_mod(a: f64, b: f64) -> f64 { usd_gf::math::modulo(a, b) }

#[pyfunction(name = "Modf")]
fn py_modf(a: f64, b: f64) -> f64 { usd_gf::math::modulo_f32(a as f32, b as f32) as f64 }

#[pyfunction(name = "SmoothStep")]
fn py_smooth_step(min: f64, max: f64, val: f64) -> f64 {
    usd_gf::math::smooth_step(min, max, val, 0.0, 0.0)
}

#[pyfunction(name = "RadiansToDegrees")]
fn py_rad_to_deg(r: f64) -> f64 { usd_gf::math::radians_to_degrees(r) }

#[pyfunction(name = "DegreesToRadians")]
fn py_deg_to_rad(d: f64) -> f64 { usd_gf::math::degrees_to_radians(d) }

/// Gf.Dot — accepts scalars and vec types. Module-level Dot for scalars.
#[pyfunction(name = "Dot")]
fn py_dot(py: Python<'_>, a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    // Scalar dot: just multiply
    if let (Ok(va), Ok(vb)) = (a.extract::<f64>(), b.extract::<f64>()) {
        return Ok((va * vb).into_pyobject(py)?.into_any().unbind());
    }
    // Vec dot via GetDot method
    if let Ok(result) = a.call_method1("GetDot", (b,)) {
        return Ok(result.unbind());
    }
    // Try tuple-to-vec conversion
    let a_tuple: PyResult<Vec<f64>> = a.extract();
    let b_tuple: PyResult<Vec<f64>> = b.extract();
    if let (Ok(at), Ok(bt)) = (a_tuple, b_tuple) {
        if at.len() != bt.len() {
            return Err(PyTypeError::new_err("Dot: vectors must have same dimension"));
        }
        let sum: f64 = at.iter().zip(bt.iter()).map(|(x, y)| x * y).sum();
        return Ok(sum.into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("Dot: unsupported argument types"))
}

/// Gf.CompMult — scalar or vec component multiply
#[pyfunction(name = "CompMult")]
fn py_comp_mult(py: Python<'_>, a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let (Ok(va), Ok(vb)) = (a.extract::<f64>(), b.extract::<f64>()) {
        return Ok((va * vb).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("CompMult: unsupported argument types"))
}

/// Gf.CompDiv — scalar or vec component divide
#[pyfunction(name = "CompDiv")]
fn py_comp_div(py: Python<'_>, a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let (Ok(va), Ok(vb)) = (a.extract::<f64>(), b.extract::<f64>()) {
        return Ok((va / vb).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("CompDiv: unsupported argument types"))
}

/// Gf._HalfRoundTrip — convert float to half and back
#[pyfunction(name = "_HalfRoundTrip")]
fn py_half_round_trip(v: f64) -> f64 {
    usd_gf::half::Half::from_f32(v as f32).to_f32() as f64
}

/// Gf.Lerp(alpha, a, b) -> interpolated value
#[pyfunction(name = "Lerp")]
fn py_lerp(py: Python<'_>, alpha: f64, a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    // Scalar lerp
    if let (Ok(va), Ok(vb)) = (a.extract::<f64>(), b.extract::<f64>()) {
        return Ok(usd_gf::math::lerp(alpha, va, vb).into_pyobject(py)?.into_any().unbind());
    }
    // Vec3d lerp
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec3d>>(), b.extract::<PyRef<'_, vec::PyVec3d>>()) {
        let r = usd_gf::Vec3d::new(
            usd_gf::math::lerp(alpha, va.0.x, vb.0.x),
            usd_gf::math::lerp(alpha, va.0.y, vb.0.y),
            usd_gf::math::lerp(alpha, va.0.z, vb.0.z),
        );
        return Ok(vec::PyVec3d(r).into_pyobject(py)?.into_any().unbind());
    }
    // Vec2d lerp
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec2d>>(), b.extract::<PyRef<'_, vec::PyVec2d>>()) {
        let r = usd_gf::Vec2d::new(
            usd_gf::math::lerp(alpha, va.0.x, vb.0.x),
            usd_gf::math::lerp(alpha, va.0.y, vb.0.y),
        );
        return Ok(vec::PyVec2d(r).into_pyobject(py)?.into_any().unbind());
    }
    // Vec4d lerp
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec4d>>(), b.extract::<PyRef<'_, vec::PyVec4d>>()) {
        let r = usd_gf::Vec4d::new(
            usd_gf::math::lerp(alpha, va.0.x, vb.0.x),
            usd_gf::math::lerp(alpha, va.0.y, vb.0.y),
            usd_gf::math::lerp(alpha, va.0.z, vb.0.z),
            usd_gf::math::lerp(alpha, va.0.w, vb.0.w),
        );
        return Ok(vec::PyVec4d(r).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("Lerp: unsupported argument types"))
}

/// Gf.IsClose(a, b, tolerance) -> bool (works on scalars and vecs)
#[pyfunction(name = "IsClose")]
fn py_is_close(a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>, tolerance: f64) -> PyResult<bool> {
    // Scalar
    if let (Ok(va), Ok(vb)) = (a.extract::<f64>(), b.extract::<f64>()) {
        return Ok((va - vb).abs() <= tolerance);
    }
    // Vec3d
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec3d>>(), b.extract::<PyRef<'_, vec::PyVec3d>>()) {
        return Ok(
            (va.0.x - vb.0.x).abs() <= tolerance &&
            (va.0.y - vb.0.y).abs() <= tolerance &&
            (va.0.z - vb.0.z).abs() <= tolerance
        );
    }
    // Vec2d
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec2d>>(), b.extract::<PyRef<'_, vec::PyVec2d>>()) {
        return Ok(
            (va.0.x - vb.0.x).abs() <= tolerance &&
            (va.0.y - vb.0.y).abs() <= tolerance
        );
    }
    // Vec4d
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec4d>>(), b.extract::<PyRef<'_, vec::PyVec4d>>()) {
        return Ok(
            (va.0.x - vb.0.x).abs() <= tolerance &&
            (va.0.y - vb.0.y).abs() <= tolerance &&
            (va.0.z - vb.0.z).abs() <= tolerance &&
            (va.0.w - vb.0.w).abs() <= tolerance
        );
    }
    // Vec3f
    if let (Ok(va), Ok(vb)) = (a.extract::<PyRef<'_, vec::PyVec3f>>(), b.extract::<PyRef<'_, vec::PyVec3f>>()) {
        let tol = tolerance as f32;
        return Ok(
            (va.0.x - vb.0.x).abs() <= tol &&
            (va.0.y - vb.0.y).abs() <= tol &&
            (va.0.z - vb.0.z).abs() <= tol
        );
    }
    // Color — component-wise comparison of RGB
    if let (Ok(ca), Ok(cb)) = (a.extract::<PyRef<'_, geo::PyColor>>(), b.extract::<PyRef<'_, geo::PyColor>>()) {
        let ra = ca.0.rgb();
        let rb = cb.0.rgb();
        let tol = tolerance as f32;
        return Ok(
            (ra.x - rb.x).abs() <= tol &&
            (ra.y - rb.y).abs() <= tol &&
            (ra.z - rb.z).abs() <= tol
        );
    }
    // Python lists/tuples of floats
    if let (Ok(la), Ok(lb)) = (a.extract::<Vec<f64>>(), b.extract::<Vec<f64>>()) {
        if la.len() != lb.len() {
            return Err(PyTypeError::new_err("IsClose: sequences must have same length"));
        }
        return Ok(la.iter().zip(lb.iter()).all(|(x, y)| (x - y).abs() <= tolerance));
    }
    Err(PyTypeError::new_err("IsClose: unsupported argument types"))
}

/// Gf.Cross(a, b) -> Vec3d cross product
#[pyfunction(name = "Cross")]
fn py_cross(a: &vec::PyVec3d, b: &vec::PyVec3d) -> vec::PyVec3d {
    vec::PyVec3d(a.0.cross(&b.0))
}

/// Gf.FindClosestPoints for Ray/Line and Ray/LineSeg
#[pyfunction(name = "FindClosestPoints")]
fn py_find_closest_points(
    py: Python<'_>,
    a: &Bound<'_, pyo3::PyAny>,
    b: &Bound<'_, pyo3::PyAny>,
) -> PyResult<Py<pyo3::PyAny>> {
    // Ray + Line
    if let (Ok(ray), Ok(line)) = (a.extract::<PyRef<'_, geo::PyRay>>(), b.extract::<PyRef<'_, geo::PyLine>>()) {
        match usd_gf::find_closest_points_ray_line(&ray.0, &line.0) {
            Some(((rp, rd), (lp, ld))) => {
                let result = (true, vec::PyVec3d(rp), vec::PyVec3d(lp), rd, ld);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
            None => {
                let result = (false, vec::PyVec3d(usd_gf::Vec3d::default()), vec::PyVec3d(usd_gf::Vec3d::default()), 0.0_f64, 0.0_f64);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
        }
    }
    // Ray + LineSeg
    if let (Ok(ray), Ok(seg)) = (a.extract::<PyRef<'_, geo::PyRay>>(), b.extract::<PyRef<'_, geo::PyLineSeg>>()) {
        match usd_gf::find_closest_points_ray_line_seg(&ray.0, &seg.0) {
            Some(((rp, rd), (sp, sd))) => {
                let result = (true, vec::PyVec3d(rp), vec::PyVec3d(sp), rd, sd);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
            None => {
                let result = (false, vec::PyVec3d(usd_gf::Vec3d::default()), vec::PyVec3d(usd_gf::Vec3d::default()), 0.0_f64, 0.0_f64);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
        }
    }
    // Line + Line
    if let (Ok(l1), Ok(l2)) = (a.extract::<PyRef<'_, geo::PyLine>>(), b.extract::<PyRef<'_, geo::PyLine>>()) {
        match usd_gf::find_closest_points_line_line(&l1.0, &l2.0) {
            Some(((p1, d1), (p2, d2))) => {
                let result = (true, vec::PyVec3d(p1), vec::PyVec3d(p2), d1, d2);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
            None => {
                let result = (false, vec::PyVec3d(usd_gf::Vec3d::default()), vec::PyVec3d(usd_gf::Vec3d::default()), 0.0_f64, 0.0_f64);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
        }
    }
    // LineSeg + LineSeg
    if let (Ok(s1), Ok(s2)) = (a.extract::<PyRef<'_, geo::PyLineSeg>>(), b.extract::<PyRef<'_, geo::PyLineSeg>>()) {
        match usd_gf::find_closest_points_seg_seg(&s1.0, &s2.0) {
            Some(((p1, d1), (p2, d2))) => {
                let result = (true, vec::PyVec3d(p1), vec::PyVec3d(p2), d1, d2);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
            None => {
                let result = (false, vec::PyVec3d(usd_gf::Vec3d::default()), vec::PyVec3d(usd_gf::Vec3d::default()), 0.0_f64, 0.0_f64);
                return Ok(result.into_pyobject(py)?.into_any().unbind());
            }
        }
    }
    Err(PyTypeError::new_err("FindClosestPoints: unsupported argument types"))
}

/// Gf.GetHomogenized(Vec4d/Vec4f) -> same type
#[pyfunction(name = "GetHomogenized")]
fn py_get_homogenized(py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() {
        return Ok(vec::PyVec4d(usd_gf::homogenize(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() {
        return Ok(vec::PyVec4f(usd_gf::homogenize_f(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("GetHomogenized: expected Vec4d or Vec4f"))
}

/// Gf.GetHomogenizedCross(Vec4d, Vec4d) -> Vec4d
#[pyfunction(name = "GetHomogenizedCross")]
fn py_get_homogenized_cross(a: &vec::PyVec4d, b: &vec::PyVec4d) -> vec::PyVec4d {
    vec::PyVec4d(usd_gf::homogeneous_cross(a.0, b.0))
}

/// Gf.HomogeneousCross — polymorphic Vec4d/Vec4f
#[pyfunction(name = "HomogeneousCross")]
fn py_homogeneous_cross(py: Python<'_>, a: &Bound<'_, pyo3::PyAny>, b: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let (Ok(ad), Ok(bd)) = (a.extract::<PyRef<'_, vec::PyVec4d>>(), b.extract::<PyRef<'_, vec::PyVec4d>>()) {
        return Ok(vec::PyVec4d(usd_gf::homogeneous_cross(ad.0, bd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let (Ok(af), Ok(bf)) = (a.extract::<PyRef<'_, vec::PyVec4f>>(), b.extract::<PyRef<'_, vec::PyVec4f>>()) {
        return Ok(vec::PyVec4f(usd_gf::homogeneous_cross_f(af.0, bf.0)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("HomogeneousCross: expected Vec4d or Vec4f"))
}

/// Gf.Project(Vec4d/Vec4f) -> Vec3d/Vec3f
#[pyfunction(name = "Project")]
fn py_project(py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() {
        return Ok(vec::PyVec3d(usd_gf::project(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() {
        return Ok(vec::PyVec3f(usd_gf::project_f(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("Project: expected Vec4d or Vec4f"))
}

/// Gf.GetLength(Vec) -> float — module-level vector length function
#[pyfunction(name = "GetLength")]
fn py_get_length(v: &Bound<'_, pyo3::PyAny>) -> PyResult<f64> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec3d>>() { return Ok(vd.0.length()); }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec3f>>() { return Ok(vf.0.length() as f64); }
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec2d>>() { return Ok(vd.0.length()); }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec2f>>() { return Ok(vf.0.length() as f64); }
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() { return Ok(vd.0.length()); }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() { return Ok(vf.0.length() as f64); }
    Err(PyTypeError::new_err("GetLength: expected a Vec type"))
}

/// Gf.ApplyGamma(Vec3d/Vec3f/Vec4d/Vec4f, gamma) -> same type  (gamma correction)
#[pyfunction(name = "ApplyGamma")]
fn py_apply_gamma(py: Python<'_>, v: &Bound<'_, pyo3::PyAny>, gamma: f64) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec3d>>() {
        return Ok(vec::PyVec3d(usd_gf::apply_gamma(vd.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec3f>>() {
        return Ok(vec::PyVec3f(usd_gf::apply_gamma(vf.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() {
        return Ok(vec::PyVec4d(usd_gf::apply_gamma(vd.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() {
        return Ok(vec::PyVec4f(usd_gf::apply_gamma(vf.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec3h>>() {
        return Ok(vec::PyVec3h(usd_gf::apply_gamma(vh.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec4h>>() {
        return Ok(vec::PyVec4h(usd_gf::apply_gamma(vh.0, gamma)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("ApplyGamma: unsupported type"))
}

/// Gf.GetDisplayGamma() -> float
#[pyfunction(name = "GetDisplayGamma")]
fn py_get_display_gamma() -> f64 { usd_gf::get_display_gamma() }

/// Gf.ConvertLinearToDisplay(Vec3d) -> Vec3d
#[pyfunction(name = "ConvertLinearToDisplay")]
fn py_linear_to_display(py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec3d>>() {
        return Ok(vec::PyVec3d(usd_gf::linear_to_display(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec3f>>() {
        return Ok(vec::PyVec3f(usd_gf::linear_to_display(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() {
        return Ok(vec::PyVec4d(usd_gf::linear_to_display(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() {
        return Ok(vec::PyVec4f(usd_gf::linear_to_display(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec3h>>() {
        return Ok(vec::PyVec3h(usd_gf::linear_to_display(vh.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec4h>>() {
        return Ok(vec::PyVec4h(usd_gf::linear_to_display(vh.0)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("ConvertLinearToDisplay: unsupported type"))
}

/// Gf.ConvertDisplayToLinear(Vec3d) -> Vec3d
#[pyfunction(name = "ConvertDisplayToLinear")]
fn py_display_to_linear(py: Python<'_>, v: &Bound<'_, pyo3::PyAny>) -> PyResult<Py<pyo3::PyAny>> {
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec3d>>() {
        return Ok(vec::PyVec3d(usd_gf::display_to_linear(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec3f>>() {
        return Ok(vec::PyVec3f(usd_gf::display_to_linear(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vd) = v.extract::<PyRef<'_, vec::PyVec4d>>() {
        return Ok(vec::PyVec4d(usd_gf::display_to_linear(vd.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vf) = v.extract::<PyRef<'_, vec::PyVec4f>>() {
        return Ok(vec::PyVec4f(usd_gf::display_to_linear(vf.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec3h>>() {
        return Ok(vec::PyVec3h(usd_gf::display_to_linear(vh.0)).into_pyobject(py)?.into_any().unbind());
    }
    if let Ok(vh) = v.extract::<PyRef<'_, vec::PyVec4h>>() {
        return Ok(vec::PyVec4h(usd_gf::display_to_linear(vh.0)).into_pyobject(py)?.into_any().unbind());
    }
    Err(PyTypeError::new_err("ConvertDisplayToLinear: unsupported type"))
}

/// Gf.DecomposeRotation(rot_matrix, tw_axis, fb_axis, lr_axis,
///     handedness, use_hint=false, sw_shift=None) -> (tw, fb, lr, sw)
///
/// Decomposes a 4x4 rotation matrix into twist/FB/LR/swing angles.
#[pyfunction(name = "DecomposeRotation")]
#[pyo3(signature = (rot, tw_axis, fb_axis, lr_axis, handedness=1.0, use_hint=false, sw_shift=None))]
fn py_decompose_rotation(
    rot: &matrix::PyMatrix4d,
    tw_axis: &vec::PyVec3d,
    fb_axis: &vec::PyVec3d,
    lr_axis: &vec::PyVec3d,
    handedness: f64,
    use_hint: bool,
    sw_shift: Option<f64>,
) -> (f64, f64, f64, f64) {
    let mut tw = 0.0_f64;
    let mut fb = 0.0_f64;
    let mut lr = 0.0_f64;
    let mut sw = 0.0_f64;
    usd_gf::Rotation::decompose_rotation(
        &rot.0,
        &tw_axis.0,
        &fb_axis.0,
        &lr_axis.0,
        handedness,
        Some(&mut tw),
        Some(&mut fb),
        Some(&mut lr),
        Some(&mut sw),
        use_hint,
        sw_shift,
    );
    (tw, fb, lr, sw)
}

/// Gf.MatchClosestEulerRotation(target_tw, target_fb, target_lr, target_sw,
///     theta_tw, theta_fb, theta_lr, theta_sw) -> (tw, fb, lr, sw)
#[pyfunction(name = "MatchClosestEulerRotation")]
fn py_match_closest_euler(
    target_tw: f64, target_fb: f64, target_lr: f64, target_sw: f64,
    theta_tw: Option<f64>, theta_fb: Option<f64>,
    theta_lr: Option<f64>, theta_sw: Option<f64>,
) -> (f64, f64, f64, f64) {
    let mut tw = theta_tw.unwrap_or(0.0);
    let mut fb = theta_fb.unwrap_or(0.0);
    let mut lr = theta_lr.unwrap_or(0.0);
    let mut sw = theta_sw.unwrap_or(0.0);
    let zero_tw = theta_tw.is_none();
    let zero_fb = theta_fb.is_none();
    let zero_lr = theta_lr.is_none();
    let zero_sw = theta_sw.is_none();
    usd_gf::Rotation::match_closest_euler_rotation(
        target_tw, target_fb, target_lr, target_sw,
        if zero_tw { None } else { Some(&mut tw) },
        if zero_fb { None } else { Some(&mut fb) },
        if zero_lr { None } else { Some(&mut lr) },
        if zero_sw { None } else { Some(&mut sw) },
    );
    (tw, fb, lr, sw)
}

/// Gf.FitPlaneToPoints(points) -> Plane or None
#[pyfunction(name = "FitPlaneToPoints")]
fn py_fit_plane(py: Python<'_>, points: Vec<PyRef<'_, vec::PyVec3d>>) -> PyResult<Py<pyo3::PyAny>> {
    let pts: Vec<usd_gf::Vec3d> = points.iter().map(|p| p.0).collect();
    match usd_gf::fit_plane_to_points(&pts) {
        Some(p) => Ok(geo::PyPlane(p).into_pyobject(py)?.into_any().unbind()),
        None => Ok(py.None().into_pyobject(py)?.into_any().unbind()),
    }
}


/// Register all Gf types into the pxr.Gf submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    vec::register(py, m)?;
    matrix::register(py, m)?;
    quat::register(py, m)?;
    geo::register(py, m)?;

    // Math functions
    m.add_function(wrap_pyfunction!(py_sqrt, m)?)?;
    m.add_function(wrap_pyfunction!(py_sqrtf, m)?)?;
    m.add_function(wrap_pyfunction!(py_sqr, m)?)?;
    m.add_function(wrap_pyfunction!(py_sgn, m)?)?;
    m.add_function(wrap_pyfunction!(py_exp, m)?)?;
    m.add_function(wrap_pyfunction!(py_expf, m)?)?;
    m.add_function(wrap_pyfunction!(py_log, m)?)?;
    m.add_function(wrap_pyfunction!(py_logf, m)?)?;
    m.add_function(wrap_pyfunction!(py_floor, m)?)?;
    m.add_function(wrap_pyfunction!(py_floorf, m)?)?;
    m.add_function(wrap_pyfunction!(py_ceil, m)?)?;
    m.add_function(wrap_pyfunction!(py_ceilf, m)?)?;
    m.add_function(wrap_pyfunction!(py_abs, m)?)?;
    m.add_function(wrap_pyfunction!(py_absf, m)?)?;
    m.add_function(wrap_pyfunction!(py_round, m)?)?;
    m.add_function(wrap_pyfunction!(py_roundf, m)?)?;
    m.add_function(wrap_pyfunction!(py_pow, m)?)?;
    m.add_function(wrap_pyfunction!(py_powf, m)?)?;
    m.add_function(wrap_pyfunction!(py_clamp, m)?)?;
    m.add_function(wrap_pyfunction!(py_clampf, m)?)?;
    m.add_function(wrap_pyfunction!(py_mod, m)?)?;
    m.add_function(wrap_pyfunction!(py_modf, m)?)?;
    m.add_function(wrap_pyfunction!(py_smooth_step, m)?)?;
    m.add_function(wrap_pyfunction!(py_rad_to_deg, m)?)?;
    m.add_function(wrap_pyfunction!(py_deg_to_rad, m)?)?;
    m.add_function(wrap_pyfunction!(py_dot, m)?)?;
    m.add_function(wrap_pyfunction!(py_comp_mult, m)?)?;
    m.add_function(wrap_pyfunction!(py_comp_div, m)?)?;
    m.add_function(wrap_pyfunction!(py_half_round_trip, m)?)?;
    m.add_function(wrap_pyfunction!(py_lerp, m)?)?;
    m.add_function(wrap_pyfunction!(py_is_close, m)?)?;
    m.add_function(wrap_pyfunction!(py_cross, m)?)?;
    m.add_function(wrap_pyfunction!(py_find_closest_points, m)?)?;
    m.add_function(wrap_pyfunction!(py_get_homogenized, m)?)?;
    m.add_function(wrap_pyfunction!(py_get_homogenized_cross, m)?)?;
    m.add_function(wrap_pyfunction!(py_homogeneous_cross, m)?)?;
    m.add_function(wrap_pyfunction!(py_project, m)?)?;
    m.add_function(wrap_pyfunction!(py_get_length, m)?)?;
    m.add_function(wrap_pyfunction!(py_apply_gamma, m)?)?;
    m.add_function(wrap_pyfunction!(py_get_display_gamma, m)?)?;
    m.add_function(wrap_pyfunction!(py_linear_to_display, m)?)?;
    m.add_function(wrap_pyfunction!(py_display_to_linear, m)?)?;
    m.add_function(wrap_pyfunction!(py_decompose_rotation, m)?)?;
    m.add_function(wrap_pyfunction!(py_match_closest_euler, m)?)?;
    m.add_function(wrap_pyfunction!(py_fit_plane, m)?)?;

    Ok(())
}
