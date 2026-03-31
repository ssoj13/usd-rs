//! Retained data sources - in-memory implementations.

use super::base::{HdBlockDataSource, HdDataSourceBase, HdDataSourceBaseHandle};
use super::container::{HdContainerDataSource, HdContainerDataSourceHandle, cast_to_container};
use super::sampled::{HdSampledDataSource, HdSampledDataSourceTime};
use super::typed::HdTypedSampledDataSource;
use super::vector::HdVectorDataSource;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// A retained container data source with in-memory storage.
///
/// Stores child data sources in a HashMap for efficient lookup. Data is
/// fully stored locally, disconnected from any backing scene.
///
/// # Thread Safety
///
/// Uses `RwLock` internally for safe concurrent access.
///
/// # Examples
///
/// ```ignore
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
/// use usd_vt::Value;
/// use std::collections::HashMap;
///
/// // Create empty container
/// let container = HdRetainedContainerDataSource::new_empty();
///
/// // Create container with children
/// let mut children = HashMap::new();
/// children.insert(
///     Token::new("myValue"),
///     HdRetainedSampledDataSource::new(Value::from(42i32)) as HdDataSourceBaseHandle
/// );
/// let container2 = HdRetainedContainerDataSource::new(children);
/// ```
#[derive(Clone)]
pub struct HdRetainedContainerDataSource {
    /// Child data sources by name
    children: Arc<RwLock<HashMap<Token, HdDataSourceBaseHandle>>>,
}

impl HdRetainedContainerDataSource {
    /// Creates an empty container.
    pub fn new_empty() -> Arc<Self> {
        Arc::new(Self {
            children: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Creates a container from a HashMap of children.
    pub fn new(children: HashMap<Token, HdDataSourceBaseHandle>) -> Arc<Self> {
        Arc::new(Self {
            children: Arc::new(RwLock::new(children)),
        })
    }

    /// Creates a container with a single child.
    pub fn new_1(name: Token, value: HdDataSourceBaseHandle) -> Arc<Self> {
        let mut children = HashMap::new();
        children.insert(name, value);
        Self::new(children)
    }

    /// Creates a container with two children.
    pub fn new_2(
        name1: Token,
        value1: HdDataSourceBaseHandle,
        name2: Token,
        value2: HdDataSourceBaseHandle,
    ) -> Arc<Self> {
        let mut children = HashMap::new();
        children.insert(name1, value1);
        children.insert(name2, value2);
        Self::new(children)
    }

    /// Creates a container with three children.
    pub fn new_3(
        name1: Token,
        value1: HdDataSourceBaseHandle,
        name2: Token,
        value2: HdDataSourceBaseHandle,
        name3: Token,
        value3: HdDataSourceBaseHandle,
    ) -> Arc<Self> {
        let mut children = HashMap::new();
        children.insert(name1, value1);
        children.insert(name2, value2);
        children.insert(name3, value3);
        Self::new(children)
    }

    /// Creates a container from an array of names and values.
    pub fn from_arrays(names: &[Token], values: &[HdDataSourceBaseHandle]) -> Arc<Self> {
        let mut children = HashMap::new();
        for (name, value) in names.iter().zip(values.iter()) {
            children.insert(name.clone(), value.clone());
        }
        Self::new(children)
    }

    /// Creates a container from a slice of (name, value) tuples.
    pub fn from_entries(entries: &[(Token, HdDataSourceBaseHandle)]) -> Arc<Self> {
        let children: HashMap<Token, HdDataSourceBaseHandle> = entries
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self::new(children)
    }
}

impl fmt::Debug for HdRetainedContainerDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let children = self.children.read();
        f.debug_struct("HdRetainedContainerDataSource")
            .field("num_children", &children.len())
            .finish()
    }
}

impl HdDataSourceBase for HdRetainedContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for HdRetainedContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        let children = self.children.read();
        children.keys().cloned().collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let children = self.children.read();
        children.get(name).cloned()
    }
}

/// Lazily composes two or more container source hierarchies.
///
/// Earlier entries in the containers array have stronger opinion strength
/// for overlapping child names. Overlapping children which are all containers
/// themselves are returned as another instance of this class.
///
/// Port of HdOverlayContainerDataSource from pxr/imaging/hd/overlayContainerDataSource.h
///
/// # Examples
///
/// ```ignore
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
///
/// let container1 = HdRetainedContainerDataSource::new_1(
///     Token::new("a"),
///     HdRetainedSampledDataSource::new(Value::from(1i32))
/// );
/// let container2 = HdRetainedContainerDataSource::new_1(
///     Token::new("b"),
///     HdRetainedSampledDataSource::new(Value::from(2i32))
/// );
///
/// // Overlay container1 over container2
/// let overlay = HdOverlayContainerDataSource::new_2(container1, container2);
/// ```
#[derive(Clone)]
pub struct HdOverlayContainerDataSource {
    /// Container sources in priority order (first has highest priority)
    containers: Vec<HdContainerDataSourceHandle>,
}

impl HdOverlayContainerDataSource {
    /// Creates an overlay from a vector of containers.
    pub fn new(containers: Vec<HdContainerDataSourceHandle>) -> Arc<Self> {
        Arc::new(Self { containers })
    }

    /// Creates an overlay from two containers.
    pub fn new_2(
        src1: HdContainerDataSourceHandle,
        src2: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            containers: vec![src1, src2],
        })
    }

    /// Creates an overlay from three containers.
    pub fn new_3(
        src1: HdContainerDataSourceHandle,
        src2: HdContainerDataSourceHandle,
        src3: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            containers: vec![src1, src2, src3],
        })
    }

    /// Creates an overlay from four containers.
    pub fn new_4(
        src1: HdContainerDataSourceHandle,
        src2: HdContainerDataSourceHandle,
        src3: HdContainerDataSourceHandle,
        src4: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            containers: vec![src1, src2, src3, src4],
        })
    }

    /// Creates HdOverlayContainerDataSource from sources, but only if needed.
    ///
    /// If one of the given handles is None, the other handle is returned instead.
    pub fn overlayed(
        src1: Option<HdContainerDataSourceHandle>,
        src2: Option<HdContainerDataSourceHandle>,
    ) -> Option<HdContainerDataSourceHandle> {
        match (src1, src2) {
            (None, None) => None,
            (Some(s1), None) => Some(s1),
            (None, Some(s2)) => Some(s2),
            (Some(s1), Some(s2)) => Some(Self::new_2(s1, s2)),
        }
    }
}

impl fmt::Debug for HdOverlayContainerDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdOverlayContainerDataSource")
            .field("num_containers", &self.containers.len())
            .finish()
    }
}

impl HdDataSourceBase for HdOverlayContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for HdOverlayContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut used_names = HashSet::new();
        for c in &self.containers {
            for name in c.get_names() {
                used_names.insert(name);
            }
        }
        used_names.into_iter().collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let mut child_containers: Vec<HdContainerDataSourceHandle> = Vec::new();

        for c in &self.containers {
            if let Some(child) = c.get(name) {
                // Try to cast to container
                if let Some(child_container) = cast_to_container(&child) {
                    child_containers.push(child_container);
                } else {
                    // If there are already containers to our left, we should
                    // return those rather than replace it with a non-container value
                    if !child_containers.is_empty() {
                        break;
                    }

                    // HdBlockDataSource's role is to mask values
                    if child.as_any().downcast_ref::<HdBlockDataSource>().is_some() {
                        return None;
                    }

                    return Some(child);
                }
            }
        }

        match child_containers.len() {
            0 => None,
            1 => Some(child_containers.into_iter().next().unwrap()),
            _ => Some(HdOverlayContainerDataSource::new(child_containers)),
        }
    }
}

/// A retained sampled data source with a single uniform value.
///
/// Stores a single value that is constant across all time samples.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_vt::Value;
///
/// let ds = HdRetainedSampledDataSource::new(Value::from(3.14f64));
/// let value = ds.get_value(0.0);
/// assert_eq!(value.get::<f64>(), Some(&3.14));
/// ```
#[derive(Clone)]
pub struct HdRetainedSampledDataSource {
    /// The stored value
    value: Value,
}

impl HdRetainedSampledDataSource {
    /// Creates a new retained sampled data source.
    pub fn new(value: Value) -> Arc<Self> {
        Arc::new(Self { value })
    }
}

impl fmt::Debug for HdRetainedSampledDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdRetainedSampledDataSource")
            .field("value", &self.value)
            .finish()
    }
}

impl HdDataSourceBase for HdRetainedSampledDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.value.clone())
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for HdRetainedSampledDataSource {
    fn get_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Value {
        self.value.clone()
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: HdSampledDataSourceTime,
        _end_time: HdSampledDataSourceTime,
        _out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        // Uniform value - no varying samples
        false
    }
}

/// A retained typed sampled data source.
///
/// Like `HdRetainedSampledDataSource` but with strongly-typed access.
///
/// # Type Parameter
///
/// * `T` - The concrete type stored
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// let ds = HdRetainedTypedSampledDataSource::new(42i32);
/// let value = ds.get_typed_value(0.0);
/// assert_eq!(value, 42);
/// ```
#[derive(Clone)]
pub struct HdRetainedTypedSampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    /// The stored value
    value: T,
}

impl<T> HdRetainedTypedSampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    /// Creates a new typed retained sampled data source.
    pub fn new(value: T) -> Arc<Self> {
        Arc::new(Self { value })
    }
}

impl<T> fmt::Debug for HdRetainedTypedSampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdRetainedTypedSampledDataSource")
            .field("value", &self.value)
            .finish()
    }
}

impl<T> HdDataSourceBase for HdRetainedTypedSampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
    HdRetainedTypedSampledDataSource<T>: HdSampledDataSource,
{
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        None
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

// No generic impl of HdSampledDataSource - only for specific types via macro
// This avoids conflicts with the macro implementations below

impl<T> HdTypedSampledDataSource<T> for HdRetainedTypedSampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
    HdRetainedTypedSampledDataSource<T>: HdSampledDataSource,
{
    fn get_typed_value(&self, _shutter_offset: HdSampledDataSourceTime) -> T {
        self.value.clone()
    }
}

// Concrete implementations for common types that can convert to Value.
// Split into Hash / non-Hash variants to satisfy Value::new() vs from_no_hash().
macro_rules! impl_typed_to_value {
    ($($t:ty),*) => {
        $(
            impl HdSampledDataSource for HdRetainedTypedSampledDataSource<$t> {
                fn get_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Value {
                    Value::new(self.value.clone())
                }

                fn get_contributing_sample_times(
                    &self,
                    _start_time: HdSampledDataSourceTime,
                    _end_time: HdSampledDataSourceTime,
                    _out_sample_times: &mut Vec<HdSampledDataSourceTime>,
                ) -> bool {
                    false
                }
            }
        )*
    };
}

// Variant for types that don't implement Hash (floats, PathExpression, etc.)
macro_rules! impl_typed_to_value_no_hash {
    ($($t:ty),*) => {
        $(
            impl HdSampledDataSource for HdRetainedTypedSampledDataSource<$t> {
                fn get_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Value {
                    Value::from_no_hash(self.value.clone())
                }

                fn get_contributing_sample_times(
                    &self,
                    _start_time: HdSampledDataSourceTime,
                    _end_time: HdSampledDataSourceTime,
                    _out_sample_times: &mut Vec<HdSampledDataSourceTime>,
                ) -> bool {
                    false
                }
            }
        )*
    };
}

// Types that implement Hash
impl_typed_to_value!(bool, i8, i16, i32, i64, u8, u16, u32, u64, usize, String);

// Token (implements Hash)
impl_typed_to_value!(usd_tf::Token);

// Path (implements Hash)
impl_typed_to_value!(usd_sdf::Path);

// HdTupleType (ext computation primvar valueType)
impl_typed_to_value!(crate::types::HdTupleType);

// ArResolverContext (used in hdar asset resolution data sources)
impl_typed_to_value!(usd_ar::ResolverContext);

// HdDataSourceLocator - used by HdDependencySchema
impl_typed_to_value!(crate::data_source::HdDataSourceLocator);

// Float types (no Hash)
impl_typed_to_value_no_hash!(f32, f64);

// Gf matrix/vector/quat types (contain floats, no Hash)
impl_typed_to_value_no_hash!(
    usd_gf::Matrix4d,
    usd_gf::Matrix4f,
    usd_gf::Vec2d,
    usd_gf::Vec2f,
    usd_gf::Vec3d,
    usd_gf::Vec3f,
    usd_gf::Vec4d,
    usd_gf::Vec4f,
    usd_gf::Quatd,
    usd_gf::Quatf
);

// HdExtComputationCpuCallbackValue (no Hash)
impl_typed_to_value_no_hash!(crate::ext_computation_cpu_callback::HdExtComputationCpuCallbackValue);

// Array<i32> for mesh topology (faceVertexCounts, faceVertexIndices)
impl_typed_to_value!(usd_vt::Array<i32>);

// PathExpression for collection membership (no Hash)
impl_typed_to_value_no_hash!(usd_sdf::PathExpression);

// Array types that implement Hash
impl_typed_to_value! {
    Vec<i32>,
    Vec<bool>,
    Vec<usd_tf::Token>,
    Vec<usd_sdf::Path>,
    Vec<usd_gf::Vec2i>
}

// Array types containing floats (no Hash)
impl_typed_to_value_no_hash! {
    Vec<f32>,
    Vec<usd_gf::Matrix4d>,
    Vec<usd_gf::Matrix4f>,
    Vec<usd_gf::Vec2f>,
    Vec<usd_gf::Vec3f>,
    Vec<usd_gf::Vec4f>,
    Vec<usd_gf::Matrix3f>,
    Vec<usd_gf::Quatf>,
    Vec<usd_gf::Quath>
}

/// A retained multi-sampled data source with multiple time samples.
///
/// Stores multiple (time, value) pairs for motion blur. Values are interpolated
/// using nearest-neighbor selection.
///
/// # Type Parameter
///
/// * `T` - The concrete type stored
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// let times = vec![-0.25, 0.0, 0.25];
/// let values = vec![1.0f64, 2.0, 3.0];
/// let ds = HdRetainedTypedMultisampledDataSource::new(&times, &values);
///
/// // Query at different times
/// let v0 = ds.get_typed_value(0.0);
/// let v1 = ds.get_typed_value(0.1);
/// ```
#[derive(Clone)]
pub struct HdRetainedTypedMultisampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    /// Sorted list of (time, value) pairs
    samples: Vec<(HdSampledDataSourceTime, T)>,
}

impl<T> HdRetainedTypedMultisampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    /// Creates a new multi-sampled data source.
    ///
    /// The `times` array should be sorted. If not, behavior is undefined.
    pub fn new(times: &[HdSampledDataSourceTime], values: &[T]) -> Arc<Self> {
        let samples: Vec<_> = times.iter().copied().zip(values.iter().cloned()).collect();
        Arc::new(Self { samples })
    }
}

impl<T> fmt::Debug for HdRetainedTypedMultisampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdRetainedTypedMultisampledDataSource")
            .field("num_samples", &self.samples.len())
            .finish()
    }
}

impl<T> HdDataSourceBase for HdRetainedTypedMultisampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + 'static,
    HdRetainedTypedMultisampledDataSource<T>: HdSampledDataSource,
{
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

// No generic impl - only concrete types via macro

impl<T> HdTypedSampledDataSource<T> for HdRetainedTypedMultisampledDataSource<T>
where
    T: Clone + Send + Sync + fmt::Debug + Default + 'static,
    HdRetainedTypedMultisampledDataSource<T>: HdSampledDataSource,
{
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> T {
        // Return default for empty samples (matches C++ behavior)
        if self.samples.is_empty() {
            return T::default();
        }

        // Find closest sample
        let mut best_idx = 0;
        let mut best_dist = (self.samples[0].0 - shutter_offset).abs();

        for (i, (time, _)) in self.samples.iter().enumerate().skip(1) {
            let dist = (*time - shutter_offset).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        self.samples[best_idx].1.clone()
    }
}

// Concrete implementations for multisampled
macro_rules! impl_multisampled_to_value {
    ($($t:ty),*) => {
        $(
            impl HdSampledDataSource for HdRetainedTypedMultisampledDataSource<$t> {
                fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
                    Value::from(self.get_typed_value(shutter_offset))
                }

                fn get_contributing_sample_times(
                    &self,
                    _start_time: HdSampledDataSourceTime,
                    _end_time: HdSampledDataSourceTime,
                    out_sample_times: &mut Vec<HdSampledDataSourceTime>,
                ) -> bool {
                    if self.samples.len() < 2 {
                        return false;
                    }

                    out_sample_times.clear();
                    out_sample_times.extend(self.samples.iter().map(|(t, _)| *t));
                    true
                }
            }
        )*
    };
}

impl_multisampled_to_value!(
    bool,
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    u64,
    f32,
    f64,
    String,
    usd_tf::Token,
    usd_gf::Matrix4d,
    usd_gf::Vec3d,
    usd_gf::Vec3f,
    usd_sdf::Path
);

/// A retained vector data source with in-memory storage.
///
/// Stores an array of data sources. Uses a `Vec` for storage.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_vt::Value;
///
/// let elements = vec![
///     HdRetainedSampledDataSource::new(Value::from(1i32)) as HdDataSourceBaseHandle,
///     HdRetainedSampledDataSource::new(Value::from(2i32)) as HdDataSourceBaseHandle,
/// ];
/// let vec_ds = HdRetainedSmallVectorDataSource::new(&elements);
///
/// assert_eq!(vec_ds.get_num_elements(), 2);
/// ```
#[derive(Clone)]
pub struct HdRetainedSmallVectorDataSource {
    /// The stored elements
    elements: Vec<HdDataSourceBaseHandle>,
}

impl HdRetainedSmallVectorDataSource {
    /// Creates a new vector data source.
    pub fn new(elements: &[HdDataSourceBaseHandle]) -> Arc<Self> {
        Arc::new(Self {
            elements: elements.to_vec(),
        })
    }

    /// Creates an empty vector data source.
    pub fn new_empty() -> Arc<Self> {
        Arc::new(Self {
            elements: Vec::new(),
        })
    }
}

impl fmt::Debug for HdRetainedSmallVectorDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdRetainedSmallVectorDataSource")
            .field("num_elements", &self.elements.len())
            .finish()
    }
}

impl HdDataSourceBase for HdRetainedSmallVectorDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_vector(&self) -> Option<crate::data_source::vector::HdVectorDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdVectorDataSource for HdRetainedSmallVectorDataSource {
    fn get_num_elements(&self) -> usize {
        self.elements.len()
    }

    fn get_element(&self, element: usize) -> Option<HdDataSourceBaseHandle> {
        self.elements.get(element).cloned()
    }
}

/// Create a typed retained data source from a Value.
///
/// Port of C++ `HdCreateTypedRetainedDataSource`. Dispatches on Value's held
/// type to create the appropriate `HdRetainedTypedSampledDataSource<T>`.
///
/// Returns None for empty values. For unsupported types, falls back to
/// wrapping the Value in a generic HdRetainedSampledDataSource.
pub fn hd_create_typed_retained_data_source(value: &Value) -> Option<Arc<dyn HdSampledDataSource>> {
    use usd_gf::*;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    if value.is_empty() {
        return None;
    }

    // Macro to try each concrete type
    macro_rules! try_type {
        ($t:ty) => {
            if let Some(v) = value.get::<$t>() {
                return Some(HdRetainedTypedSampledDataSource::new(v.clone()));
            }
        };
    }

    // Primitives
    try_type!(bool);
    try_type!(i32);
    try_type!(i64);
    try_type!(u32);
    try_type!(u64);
    try_type!(f32);
    try_type!(f64);
    try_type!(String);
    try_type!(Token);
    try_type!(SdfPath);

    // Gf types
    try_type!(Matrix4d);
    try_type!(Matrix4f);
    try_type!(Vec2f);
    try_type!(Vec2d);
    try_type!(Vec3f);
    try_type!(Vec3d);
    try_type!(Vec4f);
    try_type!(Vec4d);
    try_type!(Quatd);
    try_type!(Quatf);

    // Fallback: wrap as generic sampled DS
    Some(HdRetainedSampledDataSource::new(value.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retained_container_empty() {
        let container = HdRetainedContainerDataSource::new_empty();
        assert_eq!(container.get_names().len(), 0);
        assert!(container.get(&Token::new("test")).is_none());
    }

    #[test]
    fn test_retained_container_one() {
        let value = HdRetainedSampledDataSource::new(Value::from(42i32));
        let container = HdRetainedContainerDataSource::new_1(
            Token::new("test"),
            value as HdDataSourceBaseHandle,
        );

        let names = container.get_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&Token::new("test")));
    }

    #[test]
    fn test_retained_sampled() {
        let ds = HdRetainedSampledDataSource::new(Value::from(3.14f64));
        let value = ds.get_value(0.0);
        assert!(value.is::<f64>());
    }

    #[test]
    fn test_retained_typed() {
        let ds = HdRetainedTypedSampledDataSource::new(true);
        let value = ds.get_typed_value(0.0);
        assert_eq!(value, true);
    }

    #[test]
    fn test_retained_multisampled() {
        let times = vec![-0.5, 0.0, 0.5];
        let values = vec![1.0f64, 2.0, 3.0];
        let ds = HdRetainedTypedMultisampledDataSource::new(&times, &values);

        let v = ds.get_typed_value(0.0);
        assert!((v - 2.0).abs() < 0.001);

        let mut sample_times = Vec::new();
        let has_samples = ds.get_contributing_sample_times(-1.0, 1.0, &mut sample_times);
        assert!(has_samples);
        assert_eq!(sample_times.len(), 3);
    }

    #[test]
    fn test_retained_vector() {
        let elements = vec![
            HdRetainedSampledDataSource::new(Value::from(1i32)) as HdDataSourceBaseHandle,
            HdRetainedSampledDataSource::new(Value::from(2i32)) as HdDataSourceBaseHandle,
        ];
        let vec_ds = HdRetainedSmallVectorDataSource::new(&elements);

        assert_eq!(vec_ds.get_num_elements(), 2);
        assert!(vec_ds.get_element(0).is_some());
        assert!(vec_ds.get_element(1).is_some());
        assert!(vec_ds.get_element(2).is_none());
    }
}
