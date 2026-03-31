//! Data source utilities for usdSkelImaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceUtils.h
//!
//! Provides SharedPtrThunk (lazy cache) and GetTypedValue helper.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::HdTypedSampledDataSource;

/// Lazy cache for shared pointers.
///
/// Computes the result only once; subsequent calls return cached value.
/// Thread-safe via RwLock.
pub struct SharedPtrThunk<T: Send + Sync> {
    cache: RwLock<Option<Arc<T>>>,
}

impl<T: Send + Sync> std::fmt::Debug for SharedPtrThunk<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPtrThunk").finish()
    }
}

impl<T: Send + Sync> SharedPtrThunk<T> {
    /// Create new thunk (uninitialized).
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }

    /// Get cached value, computing if necessary.
    pub fn get<F>(&self, compute: F) -> Arc<T>
    where
        F: FnOnce() -> Arc<T>,
    {
        {
            let guard = self.cache.read();
            if let Some(cached) = guard.as_ref() {
                return Arc::clone(cached);
            }
        }
        let result = compute();
        {
            let mut guard = self.cache.write();
            if guard.is_none() {
                *guard = Some(Arc::clone(&result));
            }
        }
        result
    }

    /// Invalidate cache.
    pub fn invalidate(&self) {
        let mut guard = self.cache.write();
        *guard = None;
    }
}

impl<T: Send + Sync> Default for SharedPtrThunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Get typed value from sampled data source at shutter offset.
pub fn get_typed_value<T>(
    ds: Option<&Arc<dyn HdTypedSampledDataSource<T> + Send + Sync>>,
    shutter_offset: f32,
) -> Option<T>
where
    T: Clone + Send + Sync + 'static,
{
    ds.map(|arc| arc.get_typed_value(shutter_offset))
}

use std::sync::LazyLock;
use usd_hd::data_source::HdValueExtract;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Array, Value};

fn get_typed_value_from_container<T>(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<T>
where
    T: usd_hd::data_source::HdValueExtract + std::fmt::Debug,
{
    let child = container.get(name)?;
    let sampled = child.as_sampled()?;
    let value = sampled.get_value(0.0);
    T::extract(&value)
}

/// Get Path from a named child of a container.
pub fn get_typed_value_from_container_path(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Path> {
    get_typed_value_from_container(container, name)
}

/// Get bool from a named child of a container.
pub fn get_typed_value_from_container_bool(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<bool> {
    get_typed_value_from_container(container, name)
}

/// Get Vec<Path> from a named child of a container.
pub fn get_typed_value_from_container_vec_path(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<Path>> {
    let child = container.get(name)?;
    let sampled = child.as_sampled()?;
    let value = sampled.get_value(0.0);

    if let Some(paths) = value.get::<Vec<Path>>() {
        return Some(paths.clone());
    }
    if let Some(paths) = value.get::<Array<Path>>() {
        return Some(paths.to_vec());
    }
    if let Some(values) = value.get::<Vec<Value>>() {
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            result.push(Path::extract(value)?);
        }
        return Some(result);
    }

    None
}

/// Get Token from a named child of a container.
pub fn get_typed_value_from_container_token(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Token> {
    get_typed_value_from_container(container, name)
}

/// Get Vec<Token> from a named child of a container.
pub fn get_typed_value_from_container_vec_token(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<Token>> {
    get_typed_value_from_container(container, name)
}

/// Get typed value from a data source at shutter offset 0.
/// Returns default/empty if data source is None.
pub fn get_typed_value_from_sampled<T: Default + Clone + Send + Sync + 'static>(
    ds: Option<&Arc<dyn HdTypedSampledDataSource<T> + Send + Sync>>,
) -> T {
    ds.map(|arc| arc.get_typed_value(0.0)).unwrap_or_default()
}

/// Get Vec<i32> from a named child of a container.
pub fn get_typed_value_from_container_vec_i32(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<i32>> {
    let child = container.get(name)?;
    let sampled = child.as_sampled()?;
    let value = sampled.get_value(0.0);
    Array::<i32>::extract(&value).map(|array| array.to_vec())
}

/// Get Vec<f32> from a named child of a container.
pub fn get_typed_value_from_container_vec_f32(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<f32>> {
    get_typed_value_from_container(container, name)
}

/// Get i32 from a named child of a container.
pub fn get_typed_value_from_container_i32(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<i32> {
    get_typed_value_from_container(container, name)
}

/// Get Vec<Matrix4d> from a named child of a container.
pub fn get_typed_value_from_container_vec_mat4d(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<usd_gf::matrix4::Matrix4d>> {
    get_typed_value_from_container(container, name)
}

/// Get Vec<Vec3f> from a named child of a container.
pub fn get_typed_value_from_container_vec_vec3f(
    container: &HdContainerDataSourceHandle,
    name: &Token,
) -> Option<Vec<usd_gf::vec3::Vec3f>> {
    get_typed_value_from_container(container, name)
}

/// Token for primvarValue child of primvar container.
pub static PRIMVAR_VALUE: LazyLock<Token> = LazyLock::new(|| Token::new("primvarValue"));
/// Token for interpolation child of primvar container.
pub static INTERPOLATION: LazyLock<Token> = LazyLock::new(|| Token::new("interpolation"));
/// Token for elementSize child of primvar container.
pub static ELEMENT_SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("elementSize"));
/// Constant interpolation token.
pub static CONSTANT: LazyLock<Token> = LazyLock::new(|| Token::new("constant"));
