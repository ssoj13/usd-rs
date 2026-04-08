//! HdTimeSampleArray - Time-sampled value arrays.
//!
//! Port of pxr/imaging/hd/timeSampleArray.h
//!
//! Struct-of-arrays layout for time-sampled attributes. Used by HdSceneDelegate
//! sampling convenience overloads.

use usd_gf::matrix3::{Matrix3d, Matrix3f};
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_gf::{Quatf, Vec2d, Vec2f, Vec3d, Vec3f, Vec3i, Vec4d, Vec4f};
use usd_vt::Value;

/// Maximum number of topology representations in HdReprSelector.
pub const HD_MAX_TOPOLOGY_REPRS: usize = 3;

/// Resample two neighboring samples with linear interpolation.
///
/// For most types: `lerp(alpha, v0, v1)`.
/// Specializations: Quatf uses slerp, VtArray does component-wise.
#[inline]
pub fn hd_resample_neighbors<T: Lerp>(alpha: f32, v0: &T, v1: &T) -> T {
    T::lerp(alpha, v0, v1)
}

/// Trait for types that support linear interpolation.
pub trait Lerp: Clone {
    /// Interpolate between v0 and v1 by alpha (0..1).
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self;
}

impl Lerp for f32 {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        v0 + alpha * (v1 - v0)
    }
}

impl Lerp for f64 {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        v0 + a * (v1 - v0)
    }
}

impl Lerp for Vec2f {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        Vec2f::new(v0.x + alpha * (v1.x - v0.x), v0.y + alpha * (v1.y - v0.y))
    }
}

impl Lerp for Vec3f {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        Vec3f::new(
            v0.x + alpha * (v1.x - v0.x),
            v0.y + alpha * (v1.y - v0.y),
            v0.z + alpha * (v1.z - v0.z),
        )
    }
}

impl Lerp for Vec4f {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        Vec4f::new(
            v0.x + alpha * (v1.x - v0.x),
            v0.y + alpha * (v1.y - v0.y),
            v0.z + alpha * (v1.z - v0.z),
            v0.w + alpha * (v1.w - v0.w),
        )
    }
}

impl Lerp for Vec2d {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        Vec2d::new(v0.x + a * (v1.x - v0.x), v0.y + a * (v1.y - v0.y))
    }
}

impl Lerp for Vec3d {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        Vec3d::new(
            v0.x + a * (v1.x - v0.x),
            v0.y + a * (v1.y - v0.y),
            v0.z + a * (v1.z - v0.z),
        )
    }
}

impl Lerp for Vec4d {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        Vec4d::new(
            v0.x + a * (v1.x - v0.x),
            v0.y + a * (v1.y - v0.y),
            v0.z + a * (v1.z - v0.z),
            v0.w + a * (v1.w - v0.w),
        )
    }
}

impl Lerp for Matrix4d {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        let d0 = v0.to_array();
        let d1 = v1.to_array();
        let mut data = [[0.0_f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                data[i][j] = d0[i][j] + a * (d1[i][j] - d0[i][j]);
            }
        }
        Matrix4d::from_array(data)
    }
}

impl Lerp for Quatf {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        v0.slerp(v1, alpha)
    }
}

impl Lerp for Vec3i {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        // Integer lerp: round to nearest
        Vec3i::new(
            (v0.x as f32 + alpha * (v1.x - v0.x) as f32).round() as i32,
            (v0.y as f32 + alpha * (v1.y - v0.y) as f32).round() as i32,
            (v0.z as f32 + alpha * (v1.z - v0.z) as f32).round() as i32,
        )
    }
}

impl Lerp for Matrix3d {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let a = alpha as f64;
        let d0 = v0.to_array();
        let d1 = v1.to_array();
        let mut data = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                data[i][j] = d0[i][j] + a * (d1[i][j] - d0[i][j]);
            }
        }
        Matrix3d::from_array(data)
    }
}

impl Lerp for Matrix3f {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let d0 = v0.to_array();
        let d1 = v1.to_array();
        let mut data = [[0.0_f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                data[i][j] = d0[i][j] + alpha * (d1[i][j] - d0[i][j]);
            }
        }
        Matrix3f::from_array(data)
    }
}

impl Lerp for Matrix4f {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let d0 = v0.to_array();
        let d1 = v1.to_array();
        let mut data = [[0.0_f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                data[i][j] = d0[i][j] + alpha * (d1[i][j] - d0[i][j]);
            }
        }
        Matrix4f::from_array(data)
    }
}

impl Lerp for Vec<f32> {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let len = v0.len().min(v1.len());
        (0..len).map(|i| v0[i] + alpha * (v1[i] - v0[i])).collect()
    }
}

impl Lerp for Vec<Vec3f> {
    fn lerp(alpha: f32, v0: &Self, v1: &Self) -> Self {
        let len = v0.len().min(v1.len());
        (0..len)
            .map(|i| Vec3f::lerp(alpha, &v0[i], &v1[i]))
            .collect()
    }
}

/// Resample raw time samples at parametric position u.
/// Linear reconstruction. Constant outside sample range.
pub fn hd_resample_raw_time_samples<T: Lerp + Default>(
    u: f32,
    num_samples: usize,
    times: &[f32],
    values: &[T],
) -> T {
    if num_samples == 0 {
        return T::default();
    }
    let mut i = 0;
    for (idx, &t) in times.iter().take(num_samples).enumerate() {
        if t == u {
            // Fast path for exact parameter match (C++ uses == not epsilon).
            return values[idx].clone();
        }
        if t > u {
            i = idx;
            break;
        }
        i = idx + 1;
    }
    if i == 0 {
        values[0].clone()
    } else if i >= num_samples {
        values[num_samples - 1].clone()
    } else if times[i] == times[i - 1] {
        values[i - 1].clone()
    } else {
        let alpha = (u - times[i - 1]) / (times[i] - times[i - 1]);
        hd_resample_neighbors(alpha, &values[i - 1], &values[i])
    }
}

/// Resample indexed raw time samples.
pub fn hd_resample_raw_time_samples_indexed<T: Lerp + Default>(
    u: f32,
    num_samples: usize,
    times: &[f32],
    values: &[T],
    indices: &[Vec<i32>],
) -> (T, Vec<i32>) {
    if num_samples == 0 {
        return (T::default(), Vec::new());
    }
    let mut i = 0;
    for (idx, &t) in times.iter().take(num_samples).enumerate() {
        if t == u {
            // Fast path for exact parameter match (C++ uses == not epsilon).
            return (
                values[idx].clone(),
                indices.get(idx).cloned().unwrap_or_default(),
            );
        }
        if t > u {
            i = idx;
            break;
        }
        i = idx + 1;
    }
    if i == 0 {
        (
            values[0].clone(),
            indices.first().cloned().unwrap_or_default(),
        )
    } else if i >= num_samples {
        (
            values[num_samples - 1].clone(),
            indices.get(num_samples - 1).cloned().unwrap_or_default(),
        )
    } else if times[i] == times[i - 1] {
        (
            values[i - 1].clone(),
            indices.get(i - 1).cloned().unwrap_or_default(),
        )
    } else {
        // C++ uses reversed alpha: (us[i]-u)/(us[i]-us[i-1]) for "hold earlier indices" semantic
        let alpha = (times[i] - u) / (times[i] - times[i - 1]);
        let val = hd_resample_neighbors(alpha, &values[i - 1], &values[i]);
        // Hold earlier value for indices per C++
        let ind = indices.get(i - 1).cloned().unwrap_or_default();
        (val, ind)
    }
}

/// Get contributing sample times for interval [start_time, end_time].
/// Returns (sample_times, value_changing).
/// If value_changing, we have at least 2 distinct times.
pub fn hd_get_contributing_sample_times_for_interval(
    count: usize,
    sample_times: &[f32],
    start_time: f32,
    end_time: f32,
    out_sample_times: &mut Vec<f32>,
) -> bool {
    let mut num_out = 0usize;
    out_sample_times.clear();

    for i in 0..count {
        let t = sample_times[i];
        if num_out == 0 {
            if t > start_time && i > 0 {
                num_out += 1;
                out_sample_times.push(sample_times[i - 1]);
            }
            if t >= start_time {
                num_out += 1;
                out_sample_times.push(t);
            }
        } else {
            num_out += 1;
            out_sample_times.push(t);
        }
        if t >= end_time {
            break;
        }
    }

    if num_out == 0 && count > 0 {
        out_sample_times.push(sample_times[0]);
        return false;
    }
    num_out > 1
}

/// Default capacity for time sample arrays.
pub const HD_TIME_SAMPLE_ARRAY_DEFAULT_CAPACITY: usize = 16;

/// Time sample array - struct of arrays for (time, value) pairs.
#[derive(Debug, Clone)]
pub struct HdTimeSampleArray<T> {
    /// Sample times.
    pub times: Vec<f32>,
    /// Sample values corresponding to each time.
    pub values: Vec<T>,
    /// Number of valid samples.
    pub count: usize,
}

impl<T> Default for HdTimeSampleArray<T> {
    fn default() -> Self {
        Self {
            times: Vec::new(),
            values: Vec::new(),
            count: 0,
        }
    }
}

impl<T: Lerp + Clone + Default> HdTimeSampleArray<T> {
    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self
    where
        T: Default,
    {
        Self {
            times: vec![0.0; capacity],
            values: (0..capacity).map(|_| Default::default()).collect(),
            count: 0,
        }
    }

    /// Resize to new_size, filling new slots with defaults.
    pub fn resize(&mut self, new_size: usize)
    where
        T: Default,
    {
        self.times.resize(new_size, 0.0);
        self.values.resize(new_size, Default::default());
        self.count = new_size;
    }

    /// Resample at parametric time u via linear interpolation.
    pub fn resample(&self, u: f32) -> T {
        hd_resample_raw_time_samples(u, self.count, &self.times, &self.values)
    }

    /// Get contributing sample times for the given interval.
    pub fn get_contributing_sample_times_for_interval(
        &self,
        start_time: f32,
        end_time: f32,
        out: &mut Vec<f32>,
    ) -> bool {
        hd_get_contributing_sample_times_for_interval(
            self.count,
            &self.times,
            start_time,
            end_time,
            out,
        )
    }
}

impl<T: Lerp + Clone + Default + 'static> HdTimeSampleArray<T> {
    /// Unbox from a HdTimeSampleArray<Value> into a typed array.
    ///
    /// Values that don't match type T are replaced with Default::default().
    /// Returns true if all values had the correct type.
    ///
    /// Corresponds to C++ `HdTimeSampleArray::UnboxFrom`.
    pub fn unbox_from(box_: &HdTimeSampleArray<Value>) -> (Self, bool) {
        let mut result = Self::default();
        let mut all_ok = true;
        result.resize(box_.count);
        result.times = box_.times.clone();
        for i in 0..box_.count {
            if let Some(v) = box_.values.get(i).and_then(|val| val.get::<T>()) {
                result.values[i] = v.clone();
            } else {
                result.values[i] = T::default();
                all_ok = false;
            }
        }
        (result, all_ok)
    }
}

/// Indexed time sample array - values + indices per sample.
#[derive(Debug, Clone)]
pub struct HdIndexedTimeSampleArray<T> {
    /// Sample times.
    pub times: Vec<f32>,
    /// Sample values corresponding to each time.
    pub values: Vec<T>,
    /// Index arrays per sample for indexed primvars.
    pub indices: Vec<Vec<i32>>,
    /// Number of valid samples.
    pub count: usize,
}

impl<T> Default for HdIndexedTimeSampleArray<T> {
    fn default() -> Self {
        Self {
            times: Vec::new(),
            values: Vec::new(),
            indices: Vec::new(),
            count: 0,
        }
    }
}

impl<T: Lerp + Clone + Default> HdIndexedTimeSampleArray<T> {
    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self
    where
        T: Default,
    {
        Self {
            times: vec![0.0; capacity],
            values: (0..capacity).map(|_| Default::default()).collect(),
            indices: vec![Vec::new(); capacity],
            count: 0,
        }
    }

    /// Resize to new_size, filling new slots with defaults.
    pub fn resize(&mut self, new_size: usize)
    where
        T: Default,
    {
        self.times.resize(new_size, 0.0);
        self.values.resize(new_size, Default::default());
        self.indices.resize(new_size, Vec::new());
        self.count = new_size;
    }

    /// Resample at parametric time u, returning (value, indices).
    pub fn resample_indexed(&self, u: f32) -> (T, Vec<i32>) {
        hd_resample_raw_time_samples_indexed(
            u,
            self.count,
            &self.times,
            &self.values,
            &self.indices,
        )
    }
}

impl<T: Lerp + Clone + Default + 'static> HdIndexedTimeSampleArray<T> {
    /// Unbox from a HdIndexedTimeSampleArray<Value> into a typed array.
    ///
    /// Values that don't match type T are replaced with Default::default().
    /// Returns true if all values had the correct type.
    ///
    /// Corresponds to C++ `HdIndexedTimeSampleArray::UnboxFrom`.
    pub fn unbox_from(box_: &HdIndexedTimeSampleArray<Value>) -> (Self, bool) {
        let mut result = Self::default();
        let mut all_ok = true;
        result.resize(box_.count);
        result.times = box_.times.clone();
        result.indices = box_.indices.clone();
        for i in 0..box_.count {
            if let Some(v) = box_.values.get(i).and_then(|val| val.get::<T>()) {
                result.values[i] = v.clone();
            } else {
                result.values[i] = T::default();
                all_ok = false;
            }
        }
        (result, all_ok)
    }
}

/// Resample VtValue between two neighbors.
/// Uses type dispatch for supported types; otherwise returns v0 or v1.
pub fn hd_resample_neighbors_value(alpha: f32, v0: &Value, v1: &Value) -> Value {
    if alpha >= 1.0 {
        return v1.clone();
    }
    if alpha <= 0.0 {
        return v0.clone();
    }

    if let (Some(a), Some(b)) = (v0.get::<f32>(), v1.get::<f32>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }
    if let (Some(a), Some(b)) = (v0.get::<f64>(), v1.get::<f64>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }
    if let (Some(a), Some(b)) = (v0.get::<Vec3f>(), v1.get::<Vec3f>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }
    if let (Some(a), Some(b)) = (v0.get::<Vec4f>(), v1.get::<Vec4f>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }
    if let (Some(a), Some(b)) = (v0.get::<Matrix4d>(), v1.get::<Matrix4d>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }
    if let (Some(a), Some(b)) = (v0.get::<Quatf>(), v1.get::<Quatf>()) {
        return Value::from(hd_resample_neighbors(alpha, a, b));
    }

    if alpha < 1.0 { v0.clone() } else { v1.clone() }
}
