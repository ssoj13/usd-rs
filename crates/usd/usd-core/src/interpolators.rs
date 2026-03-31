//! Interpolators for USD attribute value resolution.
//!
//! Port of pxr/usd/usd/interpolators.h
//!
//! Provides interpolation strategies for resolving attribute values
//! at times that do not have authored time samples.

use std::marker::PhantomData;
use usd_gf::{
    Half, Matrix2d, Matrix2f, Matrix3d, Matrix3f, Matrix4d, Matrix4f, Quatd, Quatf, Quath, Vec2d,
    Vec2f, Vec3d, Vec3f, Vec4d, Vec4f, lerp, quat_slerp,
};
use usd_sdf::Path;
use usd_vt::{Array, Value};

// ============================================================================
// Interpolator Trait
// ============================================================================

/// Base trait for objects implementing interpolation for attribute values.
///
/// This is invoked during value resolution for times that do not have
/// authored time samples.
pub trait Interpolator<T> {
    /// Interpolates a value between two time samples.
    ///
    /// # Arguments
    /// * `path` - The path to the attribute being interpolated
    /// * `time` - The requested time
    /// * `lower` - The time of the lower sample
    /// * `upper` - The time of the upper sample
    /// * `lower_value` - The value at the lower time sample
    /// * `upper_value` - The value at the upper time sample
    ///
    /// # Returns
    /// The interpolated value, or None if interpolation failed.
    fn interpolate(
        &self,
        path: &Path,
        time: f64,
        lower: f64,
        upper: f64,
        lower_value: &T,
        upper_value: &T,
    ) -> Option<T>;
}

// ============================================================================
// Lerp trait for interpolatable types
// ============================================================================

/// Trait for types that support linear interpolation.
pub trait Lerpable: Clone + Send + Sync + 'static {
    /// Linearly interpolate between two values.
    ///
    /// # Arguments
    /// * `alpha` - Interpolation factor (0.0 = self, 1.0 = other)
    /// * `other` - The other value to interpolate towards
    fn lerp(&self, alpha: f64, other: &Self) -> Self;
}

// Implement Lerpable for numeric types
macro_rules! impl_lerpable_numeric {
    ($($ty:ty),*) => {
        $(
            impl Lerpable for $ty {
                fn lerp(&self, alpha: f64, other: &Self) -> Self {
                    lerp(alpha, *self as f64, *other as f64) as $ty
                }
            }
        )*
    };
}

impl_lerpable_numeric!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

// Implement Lerpable for quaternion types using slerp
impl Lerpable for Quath {
    fn lerp(&self, alpha: f64, other: &Self) -> Self {
        quat_slerp(Half::from(alpha as f32), self, other)
    }
}

impl Lerpable for Quatf {
    fn lerp(&self, alpha: f64, other: &Self) -> Self {
        quat_slerp(alpha as f32, self, other)
    }
}

impl Lerpable for Quatd {
    fn lerp(&self, alpha: f64, other: &Self) -> Self {
        quat_slerp(alpha, self, other)
    }
}

// ============================================================================
// NullInterpolator
// ============================================================================

/// Null interpolator for use in cases where interpolation is not expected.
///
/// Always returns None, indicating interpolation is not supported.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullInterpolator<T>(PhantomData<T>);

impl<T> NullInterpolator<T> {
    /// Creates a new null interpolator.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Clone> Interpolator<T> for NullInterpolator<T> {
    fn interpolate(
        &self,
        _path: &Path,
        _time: f64,
        _lower: f64,
        _upper: f64,
        _lower_value: &T,
        _upper_value: &T,
    ) -> Option<T> {
        None
    }
}

// ============================================================================
// HeldInterpolator
// ============================================================================

/// Object implementing "held" interpolation for attribute values.
///
/// With "held" interpolation, authored time sample values are held constant
/// across time until the next authored time sample. In other words, the
/// attribute value for a time with no samples authored is the nearest
/// preceding value.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeldInterpolator<T>(PhantomData<T>);

impl<T> HeldInterpolator<T> {
    /// Creates a new held interpolator.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Clone> Interpolator<T> for HeldInterpolator<T> {
    fn interpolate(
        &self,
        _path: &Path,
        _time: f64,
        _lower: f64,
        _upper: f64,
        lower_value: &T,
        _upper_value: &T,
    ) -> Option<T> {
        // In case of held interpolation, lower sample's value is held
        Some(lower_value.clone())
    }
}

// ============================================================================
// LinearInterpolator
// ============================================================================

/// Object implementing linear interpolation for attribute values.
///
/// With linear interpolation, the attribute value for a time with no samples
/// will be linearly interpolated from the previous and next time samples.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinearInterpolator<T>(PhantomData<T>);

impl<T> LinearInterpolator<T> {
    /// Creates a new linear interpolator.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Lerpable> Interpolator<T> for LinearInterpolator<T> {
    fn interpolate(
        &self,
        _path: &Path,
        time: f64,
        lower: f64,
        upper: f64,
        lower_value: &T,
        upper_value: &T,
    ) -> Option<T> {
        // Calculate parametric time
        let parametric_time = if (upper - lower).abs() < 1e-10 {
            0.0
        } else {
            (time - lower) / (upper - lower)
        };

        Some(lower_value.lerp(parametric_time, upper_value))
    }
}

// ============================================================================
// ArrayLinearInterpolator
// ============================================================================

/// Linear interpolator specialized for array types.
///
/// Linearly interpolates each element of the array. Falls back to held
/// interpolation if array sizes don't match (e.g., meshes with varying topology).
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayLinearInterpolator<T>(PhantomData<T>);

impl<T> ArrayLinearInterpolator<T> {
    /// Creates a new array linear interpolator.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Lerpable> Interpolator<Array<T>> for ArrayLinearInterpolator<T> {
    fn interpolate(
        &self,
        _path: &Path,
        time: f64,
        lower: f64,
        upper: f64,
        lower_value: &Array<T>,
        upper_value: &Array<T>,
    ) -> Option<Array<T>> {
        // Fall back to held interpolation if sizes don't match
        if lower_value.len() != upper_value.len() {
            return Some(lower_value.clone());
        }

        // Calculate parametric time
        let parametric_time = if (upper - lower).abs() < 1e-10 {
            0.0
        } else {
            (time - lower) / (upper - lower)
        };

        // Optimize for exact boundaries
        if parametric_time == 0.0 {
            return Some(lower_value.clone());
        }
        if parametric_time == 1.0 {
            return Some(upper_value.clone());
        }

        // Interpolate each element
        let mut result = Vec::with_capacity(lower_value.len());
        for (l, u) in lower_value.iter().zip(upper_value.iter()) {
            result.push(l.lerp(parametric_time, u));
        }

        Some(Array::from(result))
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Gets or interpolates a value at the given time.
///
/// If `lower == upper`, returns the value at that time.
/// Otherwise, interpolates between lower and upper using the given interpolator.
///
/// # Arguments
/// * `time` - The requested time
/// * `lower` - The time of the lower bracket
/// * `upper` - The time of the upper bracket
/// * `lower_value` - The value at the lower time
/// * `upper_value` - The value at the upper time
/// * `interpolator` - The interpolator to use
pub fn get_or_interpolate<T: Clone, I>(
    path: &Path,
    time: f64,
    lower: f64,
    upper: f64,
    lower_value: &T,
    upper_value: &T,
    interpolator: &I,
) -> Option<T>
where
    I: Interpolator<T>,
{
    // If lower and upper are close enough, just return lower value
    if is_close(lower, upper, 1e-6) {
        return Some(lower_value.clone());
    }

    interpolator.interpolate(path, time, lower, upper, lower_value, upper_value)
}

/// Checks if two floating point values are close within epsilon.
#[inline]
fn is_close(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() <= epsilon
}

/// Interpolate a type-erased Value.
///
/// Attempts to determine the value type and apply appropriate interpolation.
pub fn interpolate_value(
    _path: &Path,
    time: f64,
    lower: f64,
    upper: f64,
    lower_value: &Value,
    upper_value: &Value,
    use_linear: bool,
) -> Option<Value> {
    // If close times, return lower
    if is_close(lower, upper, 1e-6) {
        return Some(lower_value.clone());
    }

    // If not linear interpolation, use held
    if !use_linear {
        return Some(lower_value.clone());
    }

    // Calculate parametric time
    let parametric_time = (time - lower) / (upper - lower);

    // Try to interpolate based on type
    // This is a simplified implementation - full version would handle all VtValue types
    // Scalar types
    if let (Some(&l), Some(&u)) = (lower_value.get::<f64>(), upper_value.get::<f64>()) {
        return Some(Value::from(lerp(parametric_time, l, u)));
    }
    if let (Some(&l), Some(&u)) = (lower_value.get::<f32>(), upper_value.get::<f32>()) {
        return Some(Value::from(lerp(parametric_time, l as f64, u as f64) as f32));
    }
    // Integral scalar types are held-only in USD, even when the stage
    // interpolation mode is Linear.
    if lower_value.get::<i32>().is_some() && upper_value.get::<i32>().is_some() {
        return Some(lower_value.clone());
    }
    if lower_value.get::<i64>().is_some() && upper_value.get::<i64>().is_some() {
        return Some(lower_value.clone());
    }
    if lower_value.get::<u32>().is_some() && upper_value.get::<u32>().is_some() {
        return Some(lower_value.clone());
    }
    if lower_value.get::<u64>().is_some() && upper_value.get::<u64>().is_some() {
        return Some(lower_value.clone());
    }

    // Vec2 types — component-wise lerp
    if let (Some(l), Some(u)) = (lower_value.get::<Vec2d>(), upper_value.get::<Vec2d>()) {
        return Some(Value::from(Vec2d::new(
            lerp(parametric_time, l.x, u.x),
            lerp(parametric_time, l.y, u.y),
        )));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Vec2f>(), upper_value.get::<Vec2f>()) {
        return Some(Value::from(Vec2f::new(
            lerp(parametric_time, l.x as f64, u.x as f64) as f32,
            lerp(parametric_time, l.y as f64, u.y as f64) as f32,
        )));
    }

    // Vec3 types — component-wise lerp
    if let (Some(l), Some(u)) = (lower_value.get::<Vec3d>(), upper_value.get::<Vec3d>()) {
        return Some(Value::from(Vec3d::new(
            lerp(parametric_time, l.x, u.x),
            lerp(parametric_time, l.y, u.y),
            lerp(parametric_time, l.z, u.z),
        )));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Vec3f>(), upper_value.get::<Vec3f>()) {
        return Some(Value::from(Vec3f::new(
            lerp(parametric_time, l.x as f64, u.x as f64) as f32,
            lerp(parametric_time, l.y as f64, u.y as f64) as f32,
            lerp(parametric_time, l.z as f64, u.z as f64) as f32,
        )));
    }

    // Vec4 types — component-wise lerp
    if let (Some(l), Some(u)) = (lower_value.get::<Vec4d>(), upper_value.get::<Vec4d>()) {
        return Some(Value::from(Vec4d::new(
            lerp(parametric_time, l.x, u.x),
            lerp(parametric_time, l.y, u.y),
            lerp(parametric_time, l.z, u.z),
            lerp(parametric_time, l.w, u.w),
        )));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Vec4f>(), upper_value.get::<Vec4f>()) {
        return Some(Value::from(Vec4f::new(
            lerp(parametric_time, l.x as f64, u.x as f64) as f32,
            lerp(parametric_time, l.y as f64, u.y as f64) as f32,
            lerp(parametric_time, l.z as f64, u.z as f64) as f32,
            lerp(parametric_time, l.w as f64, u.w as f64) as f32,
        )));
    }

    // Quaternion types — slerp
    // quat_slerp(alpha, q0, q1) where alpha is T, q0/q1 are &Quat<T>
    if let (Some(l), Some(u)) = (lower_value.get::<Quatd>(), upper_value.get::<Quatd>()) {
        return Some(Value::from(quat_slerp(parametric_time, l, u)));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Quatf>(), upper_value.get::<Quatf>()) {
        // Promote to f64 for slerp precision, then downcast
        let ld = Quatd::from_components(
            l.real() as f64,
            l.imaginary().x as f64,
            l.imaginary().y as f64,
            l.imaginary().z as f64,
        );
        let ud = Quatd::from_components(
            u.real() as f64,
            u.imaginary().x as f64,
            u.imaginary().y as f64,
            u.imaginary().z as f64,
        );
        let rd = quat_slerp(parametric_time, &ld, &ud);
        let ri = rd.imaginary();
        return Some(Value::from(Quatf::from_components(
            rd.real() as f32,
            ri.x as f32,
            ri.y as f32,
            ri.z as f32,
        )));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Quath>(), upper_value.get::<Quath>()) {
        let lf = |h: &Quath| -> Quatd {
            Quatd::from_components(
                f32::from(h.real()) as f64,
                f32::from(h.imaginary().x) as f64,
                f32::from(h.imaginary().y) as f64,
                f32::from(h.imaginary().z) as f64,
            )
        };
        let rd = quat_slerp(parametric_time, &lf(l), &lf(u));
        let ri = rd.imaginary();
        return Some(Value::from(Quath::from_components(
            Half::from(rd.real() as f32),
            Half::from(ri.x as f32),
            Half::from(ri.y as f32),
            Half::from(ri.z as f32),
        )));
    }

    // Matrix types — element-wise lerp using row/column indexing
    macro_rules! lerp_matrix {
        ($lower:expr, $upper:expr, $Ty:ty, $N:expr, $alpha:expr) => {{
            let mut arr = [[0.0f64; $N]; $N];
            for r in 0..$N {
                for c in 0..$N {
                    arr[r][c] = lerp($alpha, $lower[r][c] as f64, $upper[r][c] as f64);
                }
            }
            arr
        }};
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix2d>(), upper_value.get::<Matrix2d>()) {
        let arr = lerp_matrix!(l, u, Matrix2d, 2, parametric_time);
        return Some(Value::from(Matrix2d::from_array(arr)));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix2f>(), upper_value.get::<Matrix2f>()) {
        let arr = lerp_matrix!(l, u, Matrix2f, 2, parametric_time);
        return Some(Value::from(Matrix2f::from_array([
            [arr[0][0] as f32, arr[0][1] as f32],
            [arr[1][0] as f32, arr[1][1] as f32],
        ])));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix3d>(), upper_value.get::<Matrix3d>()) {
        let arr = lerp_matrix!(l, u, Matrix3d, 3, parametric_time);
        return Some(Value::from(Matrix3d::from_array(arr)));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix3f>(), upper_value.get::<Matrix3f>()) {
        let arr = lerp_matrix!(l, u, Matrix3f, 3, parametric_time);
        return Some(Value::from(Matrix3f::from_array([
            [arr[0][0] as f32, arr[0][1] as f32, arr[0][2] as f32],
            [arr[1][0] as f32, arr[1][1] as f32, arr[1][2] as f32],
            [arr[2][0] as f32, arr[2][1] as f32, arr[2][2] as f32],
        ])));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix4d>(), upper_value.get::<Matrix4d>()) {
        let arr = lerp_matrix!(l, u, Matrix4d, 4, parametric_time);
        return Some(Value::from(Matrix4d::from_array(arr)));
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Matrix4f>(), upper_value.get::<Matrix4f>()) {
        let arr = lerp_matrix!(l, u, Matrix4f, 4, parametric_time);
        return Some(Value::from(Matrix4f::from_array([
            [
                arr[0][0] as f32,
                arr[0][1] as f32,
                arr[0][2] as f32,
                arr[0][3] as f32,
            ],
            [
                arr[1][0] as f32,
                arr[1][1] as f32,
                arr[1][2] as f32,
                arr[1][3] as f32,
            ],
            [
                arr[2][0] as f32,
                arr[2][1] as f32,
                arr[2][2] as f32,
                arr[2][3] as f32,
            ],
            [
                arr[3][0] as f32,
                arr[3][1] as f32,
                arr[3][2] as f32,
                arr[3][3] as f32,
            ],
        ])));
    }

    // Integer array types — element-wise lerp
    if let (Some(l), Some(u)) = (
        lower_value.get::<Array<i32>>(),
        upper_value.get::<Array<i32>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<i32> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv as f64, uv as f64) as i32)
                .collect();
            return Some(Value::from(Array::from(result)));
        }
    }
    if let (Some(l), Some(u)) = (
        lower_value.get::<Array<i64>>(),
        upper_value.get::<Array<i64>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<i64> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv as f64, uv as f64) as i64)
                .collect();
            return Some(Value::from(Array::from(result)));
        }
    }

    // Float array types — element-wise lerp
    // Note: f32/f64 don't implement Hash; use Value::from_no_hash to wrap Array directly
    if let (Some(l), Some(u)) = (
        lower_value.get::<Array<f32>>(),
        upper_value.get::<Array<f32>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<f32> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv as f64, uv as f64) as f32)
                .collect();
            return Some(Value::from_no_hash(Array::from(result)));
        }
    }
    if let (Some(l), Some(u)) = (
        lower_value.get::<Array<f64>>(),
        upper_value.get::<Array<f64>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<f64> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv, uv))
                .collect();
            return Some(Value::from_no_hash(Array::from(result)));
        }
    }

    // Vec<Vec3f> arrays (USDA parser typed arrays)
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Vec3f>>(),
        upper_value.get::<Vec<Vec3f>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<Vec3f> = l
                .iter()
                .zip(u.iter())
                .map(|(lv, uv)| {
                    Vec3f::new(
                        lerp(parametric_time, lv.x as f64, uv.x as f64) as f32,
                        lerp(parametric_time, lv.y as f64, uv.y as f64) as f32,
                        lerp(parametric_time, lv.z as f64, uv.z as f64) as f32,
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Vec3d>>(),
        upper_value.get::<Vec<Vec3d>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<Vec3d> = l
                .iter()
                .zip(u.iter())
                .map(|(lv, uv)| {
                    Vec3d::new(
                        lerp(parametric_time, lv.x, uv.x),
                        lerp(parametric_time, lv.y, uv.y),
                        lerp(parametric_time, lv.z, uv.z),
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Vec2f>>(),
        upper_value.get::<Vec<Vec2f>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<Vec2f> = l
                .iter()
                .zip(u.iter())
                .map(|(lv, uv)| {
                    Vec2f::new(
                        lerp(parametric_time, lv.x as f64, uv.x as f64) as f32,
                        lerp(parametric_time, lv.y as f64, uv.y as f64) as f32,
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }

    // Vec<Quath> arrays — element-wise slerp
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Quath>>(),
        upper_value.get::<Vec<Quath>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<Quath> = l
                .iter()
                .zip(u.iter())
                .map(|(lv, uv)| {
                    let ld = Quatd::from_components(
                        f32::from(lv.real()) as f64,
                        f32::from(lv.imaginary().x) as f64,
                        f32::from(lv.imaginary().y) as f64,
                        f32::from(lv.imaginary().z) as f64,
                    );
                    let ud = Quatd::from_components(
                        f32::from(uv.real()) as f64,
                        f32::from(uv.imaginary().x) as f64,
                        f32::from(uv.imaginary().y) as f64,
                        f32::from(uv.imaginary().z) as f64,
                    );
                    let rd = quat_slerp(parametric_time, &ld, &ud);
                    let ri = rd.imaginary();
                    Quath::from_components(
                        Half::from(rd.real() as f32),
                        Half::from(ri.x as f32),
                        Half::from(ri.y as f32),
                        Half::from(ri.z as f32),
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Quatf>>(),
        upper_value.get::<Vec<Quatf>>(),
    ) {
        if l.len() == u.len() {
            let result: Vec<Quatf> = l
                .iter()
                .zip(u.iter())
                .map(|(lv, uv)| {
                    let ld = Quatd::from_components(
                        lv.real() as f64,
                        lv.imaginary().x as f64,
                        lv.imaginary().y as f64,
                        lv.imaginary().z as f64,
                    );
                    let ud = Quatd::from_components(
                        uv.real() as f64,
                        uv.imaginary().x as f64,
                        uv.imaginary().y as f64,
                        uv.imaginary().z as f64,
                    );
                    let rd = quat_slerp(parametric_time, &ld, &ud);
                    let ri = rd.imaginary();
                    Quatf::from_components(rd.real() as f32, ri.x as f32, ri.y as f32, ri.z as f32)
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }

    // Vec<i32> arrays
    if let (Some(l), Some(u)) = (lower_value.get::<Vec<i32>>(), upper_value.get::<Vec<i32>>()) {
        if l.len() == u.len() {
            let result: Vec<i32> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv as f64, uv as f64) as i32)
                .collect();
            return Some(Value::new(result));
        }
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Vec<f32>>(), upper_value.get::<Vec<f32>>()) {
        if l.len() == u.len() {
            let result: Vec<f32> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv as f64, uv as f64) as f32)
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }
    if let (Some(l), Some(u)) = (lower_value.get::<Vec<f64>>(), upper_value.get::<Vec<f64>>()) {
        if l.len() == u.len() {
            let result: Vec<f64> = l
                .iter()
                .zip(u.iter())
                .map(|(&lv, &uv)| lerp(parametric_time, lv, uv))
                .collect();
            return Some(Value::from_no_hash(result));
        }
    }

    // Vec<Value> arrays (USDA parser produces these for typed array time samples)
    if let (Some(l), Some(u)) = (
        lower_value.get::<Vec<Value>>(),
        upper_value.get::<Vec<Value>>(),
    ) {
        if l.len() == u.len() {
            // Try f64 element-wise lerp
            let mut result = Vec::with_capacity(l.len());
            let mut all_f64 = true;
            for (lv, uv) in l.iter().zip(u.iter()) {
                let lf = lv
                    .get::<f64>()
                    .copied()
                    .or_else(|| lv.get::<i64>().map(|&i| i as f64));
                let uf = uv
                    .get::<f64>()
                    .copied()
                    .or_else(|| uv.get::<i64>().map(|&i| i as f64));
                if let (Some(lf), Some(uf)) = (lf, uf) {
                    result.push(Value::from(lerp(parametric_time, lf, uf)));
                } else {
                    all_f64 = false;
                    break;
                }
            }
            if all_f64 {
                return Some(Value::from_no_hash(result));
            }
        }
    }

    // Fall back to held interpolation for unsupported types
    Some(lower_value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_held_interpolator() {
        let interp = HeldInterpolator::<f64>::new();
        let path = Path::empty();

        let result = interp.interpolate(&path, 0.5, 0.0, 1.0, &10.0, &20.0);
        assert_eq!(result, Some(10.0));
    }

    #[test]
    fn test_linear_interpolator() {
        let interp = LinearInterpolator::<f64>::new();
        let path = Path::empty();

        let result = interp.interpolate(&path, 0.5, 0.0, 1.0, &10.0, &20.0);
        assert_eq!(result, Some(15.0));

        let result = interp.interpolate(&path, 0.0, 0.0, 1.0, &10.0, &20.0);
        assert_eq!(result, Some(10.0));

        let result = interp.interpolate(&path, 1.0, 0.0, 1.0, &10.0, &20.0);
        assert_eq!(result, Some(20.0));
    }

    #[test]
    fn test_array_linear_interpolator() {
        let interp = ArrayLinearInterpolator::<f64>::new();
        let path = Path::empty();

        let lower = Array::from(vec![0.0f64, 10.0, 20.0]);
        let upper = Array::from(vec![10.0f64, 20.0, 30.0]);

        let result = interp
            .interpolate(&path, 0.5, 0.0, 1.0, &lower, &upper)
            .unwrap();
        assert_eq!(result[0], 5.0);
        assert_eq!(result[1], 15.0);
        assert_eq!(result[2], 25.0);
    }

    #[test]
    fn test_array_size_mismatch_fallback() {
        let interp = ArrayLinearInterpolator::<f64>::new();
        let path = Path::empty();

        let lower = Array::from(vec![0.0f64, 10.0]);
        let upper = Array::from(vec![10.0f64, 20.0, 30.0]);

        // Should fall back to held (lower value) when sizes don't match
        let result = interp
            .interpolate(&path, 0.5, 0.0, 1.0, &lower, &upper)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 0.0);
        assert_eq!(result[1], 10.0);
    }

    #[test]
    fn test_null_interpolator() {
        let interp = NullInterpolator::<f64>::new();
        let path = Path::empty();

        let result = interp.interpolate(&path, 0.5, 0.0, 1.0, &10.0, &20.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_close() {
        assert!(is_close(1.0, 1.0, 1e-6));
        assert!(is_close(1.0, 1.0000001, 1e-6));
        assert!(!is_close(1.0, 1.001, 1e-6));
    }
    // ---- interpolate_value tests covering the fixed lerp_matrix macro ----

    #[test]
    fn test_interpolate_value_f64_midpoint() {
        let path = Path::empty();
        let lo = Value::from(0.0_f64);
        let hi = Value::from(10.0_f64);
        let v = interpolate_value(&path, 0.5, 0.0, 1.0, &lo, &hi, true).unwrap();
        assert_eq!(*v.get::<f64>().unwrap(), 5.0);
    }

    #[test]
    fn test_interpolate_value_f32() {
        let path = Path::empty();
        let lo = Value::from(0.0_f32);
        let hi = Value::from(4.0_f32);
        let v = interpolate_value(&path, 0.25, 0.0, 1.0, &lo, &hi, true).unwrap();
        assert!((v.get::<f32>().unwrap() - 1.0_f32).abs() < 1e-5);
    }

    #[test]
    fn test_interpolate_value_held_mode_returns_lower() {
        let path = Path::empty();
        let lo = Value::from(0.0_f64);
        let hi = Value::from(10.0_f64);
        let v = interpolate_value(&path, 0.5, 0.0, 1.0, &lo, &hi, false).unwrap();
        assert_eq!(*v.get::<f64>().unwrap(), 0.0);
    }

    #[test]
    fn test_interpolate_value_vec3f_midpoint() {
        let path = Path::empty();
        let lo = Value::from(Vec3f::new(0.0, 0.0, 0.0));
        let hi = Value::from(Vec3f::new(2.0, 4.0, 6.0));
        let v = interpolate_value(&path, 0.5, 0.0, 1.0, &lo, &hi, true).unwrap();
        let r = v.get::<Vec3f>().unwrap();
        assert!((r.x - 1.0).abs() < 1e-5, "x={}", r.x);
        assert!((r.y - 2.0).abs() < 1e-5, "y={}", r.y);
        assert!((r.z - 3.0).abs() < 1e-5, "z={}", r.z);
    }

    #[test]
    fn test_interpolate_value_vec4d_midpoint() {
        let path = Path::empty();
        let lo = Value::from(Vec4d::new(0.0, 0.0, 0.0, 0.0));
        let hi = Value::from(Vec4d::new(2.0, 4.0, 6.0, 8.0));
        let v = interpolate_value(&path, 0.5, 0.0, 1.0, &lo, &hi, true).unwrap();
        let r = v.get::<Vec4d>().unwrap();
        assert!((r.x - 1.0).abs() < 1e-9);
        assert!((r.y - 2.0).abs() < 1e-9);
        assert!((r.z - 3.0).abs() < 1e-9);
        assert!((r.w - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_interpolate_value_matrix4d_lerp_matrix_fix() {
        // This test verifies the lerp_matrix! macro fix: $lower[$r][c] -> $lower[r][c]
        // Before fix this failed to compile; after fix each element is correctly lerped.
        let path = Path::empty();
        let lo = Matrix4d::from_array([
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ]);
        let hi = Matrix4d::from_array([
            [2.0, 4.0, 6.0, 8.0],
            [10.0, 12.0, 14.0, 16.0],
            [18.0, 20.0, 22.0, 24.0],
            [26.0, 28.0, 30.0, 32.0],
        ]);
        let v = interpolate_value(
            &path,
            0.5,
            0.0,
            1.0,
            &Value::from(lo),
            &Value::from(hi),
            true,
        )
        .unwrap();
        let r = v.get::<Matrix4d>().unwrap();
        assert!((r[0][0] - 1.0).abs() < 1e-9, "[0][0]={}", r[0][0]);
        assert!((r[0][1] - 2.0).abs() < 1e-9, "[0][1]={}", r[0][1]);
        assert!((r[1][0] - 5.0).abs() < 1e-9, "[1][0]={}", r[1][0]);
        assert!((r[3][3] - 16.0).abs() < 1e-9, "[3][3]={}", r[3][3]);
    }

    #[test]
    fn test_interpolate_value_matrix3d() {
        let path = Path::empty();
        let lo = Matrix3d::from_array([[0.0; 3]; 3]);
        let hi = Matrix3d::from_array([[2.0, 4.0, 6.0], [8.0, 10.0, 12.0], [14.0, 16.0, 18.0]]);
        let v = interpolate_value(
            &path,
            0.5,
            0.0,
            1.0,
            &Value::from(lo),
            &Value::from(hi),
            true,
        )
        .unwrap();
        let r = v.get::<Matrix3d>().unwrap();
        assert!((r[0][0] - 1.0).abs() < 1e-9);
        assert!((r[1][1] - 5.0).abs() < 1e-9);
        assert!((r[2][2] - 9.0).abs() < 1e-9);
    }

    #[test]
    fn test_interpolate_value_quatd_identity_at_t0() {
        let path = Path::empty();
        // identity quat slerped with anything at t=0 should give identity
        let identity = Quatd::from_components(1.0, 0.0, 0.0, 0.0);
        let rot180z = Quatd::from_components(0.0, 0.0, 0.0, 1.0);
        let v = interpolate_value(
            &path,
            0.0,
            0.0,
            1.0,
            &Value::from(identity),
            &Value::from(rot180z),
            true,
        )
        .unwrap();
        let r = v.get::<Quatd>().unwrap();
        assert!((r.real() - 1.0).abs() < 1e-6, "real={}", r.real());
    }

    #[test]
    fn test_interpolate_value_same_times_returns_lower() {
        let path = Path::empty();
        let lo = Value::from(42.0_f64);
        let hi = Value::from(99.0_f64);
        // lower and upper differ by less than epsilon -> return lower
        let v = interpolate_value(&path, 1.0, 1.0, 1.0 + 1e-9, &lo, &hi, true).unwrap();
        assert_eq!(*v.get::<f64>().unwrap(), 42.0);
    }
}
