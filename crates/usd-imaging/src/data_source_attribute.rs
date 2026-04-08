//! Typed USD attribute data source.
//!
//! # C++ Reference
//!
//! Port of `pxr/usdImaging/usdImaging/dataSourceAttribute.h`
//!
//! In C++, `UsdImagingDataSourceAttribute<T>` is a class template inheriting from
//! `HdTypedSampledDataSource<T>`. Each instantiation (e.g. `<VtIntArray>`,
//! `<TfToken>`, `<bool>`) knows its value type at compile time and implements
//! `GetTypedValue()` by calling `UsdAttribute::Get<T>()`.
//!
//! # Rust Design
//!
//! `DataSourceAttribute<T>` is generic over `T: HdValueExtract`. It wraps a USD
//! attribute and implements both:
//! - [`HdSampledDataSource`] — untyped `get_value()` returning `Value`
//! - [`HdTypedSampledDataSource<T>`] — typed `get_typed_value()` returning `T`
//!
//! Since our USD attribute API returns untyped `Value` (not templated like C++),
//! `get_typed_value()` calls `get_value()` and extracts `T` via [`HdValueExtract`].
//!
//! # Factory
//!
//! [`data_source_attribute_new`] maps SDF type names to concrete instantiations,
//! mirroring C++ `UsdImagingDataSourceAttributeNew()`.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use std::marker::PhantomData;
use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_gf::{Vec2f, Vec2i, Vec3f, Vec3i, Vec4f, Vec4i};
use usd_hd::{
    HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator, HdSampledDataSource,
    HdSampledDataSourceTime, HdTypedSampledDataSource, HdValueExtract,
};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::{Array, Value};

fn attr_type_token(attribute: &Attribute) -> Token {
    let type_name = attribute.get_type_name();
    if type_name.is_valid() {
        return type_name.scalar_type().as_token();
    }
    attribute.type_name()
}

fn looks_like_nested_tuple_array(value: &Value) -> bool {
    value
        .get::<Vec<Value>>()
        .and_then(|values| values.first())
        .is_some_and(|first| first.get::<Vec<Value>>().is_some())
}

fn normalize_untyped_value(attribute: &Attribute, value: &Value) -> Option<Value> {
    let type_name = attr_type_token(attribute);
    match type_name.as_str() {
        "int" | "int[]" => {
            if value.get::<Vec<Value>>().is_some() {
                Array::<i32>::extract(value).map(Value::from)
            } else {
                Some(value.clone())
            }
        }
        "float" | "float[]" => {
            if value.get::<Vec<Value>>().is_some() {
                Array::<f32>::extract(value).map(|values| Value::from(values.to_vec()))
            } else {
                Some(value.clone())
            }
        }
        "double" | "double[]" => {
            if value.get::<Vec<Value>>().is_some() {
                Array::<f64>::extract(value).map(|values| Value::from_no_hash(values.to_vec()))
            } else {
                Some(value.clone())
            }
        }
        "token" | "token[]" => {
            if value.get::<Vec<Value>>().is_some() {
                Array::<Token>::extract(value).map(Value::from)
            } else {
                Some(value.clone())
            }
        }
        "float2" | "float2[]" | "texCoord2f" | "texCoord2f[]" => {
            if looks_like_nested_tuple_array(value)
                || value.get::<Vec<Vec2f>>().is_some()
                || value.get::<Array<Vec2f>>().is_some()
            {
                Vec::<Vec2f>::extract(value).map(Value::from)
            } else {
                Vec2f::extract(value).map(Value::from)
            }
        }
        "point3f" | "point3f[]" | "normal3f" | "normal3f[]" | "vector3f" | "vector3f[]"
        | "color3f" | "color3f[]" => {
            if looks_like_nested_tuple_array(value)
                || value.get::<Vec<Vec3f>>().is_some()
                || value.get::<Array<Vec3f>>().is_some()
            {
                Vec::<Vec3f>::extract(value).map(Value::from)
            } else {
                Vec3f::extract(value).map(Value::from)
            }
        }
        _ => Some(value.clone()),
    }
}

fn record_object_in_stage_globals(
    stage_globals: &DataSourceStageGlobalsHandle,
    attribute: &Attribute,
) {
    let type_name = attr_type_token(attribute);
    if matches!(type_name.as_str(), "asset" | "asset[]") {
        stage_globals.flag_as_asset_path_dependent(attribute.path());
    }
}

pub trait HdValueNormalize: HdValueExtract {
    fn normalize_value(attribute: &Attribute, value: &Value) -> Option<Value>;
}

impl HdValueNormalize for Value {
    fn normalize_value(attribute: &Attribute, value: &Value) -> Option<Value> {
        normalize_untyped_value(attribute, value)
    }
}

impl HdValueNormalize for bool {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        bool::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Token {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Token::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Array<i32> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Array::<i32>::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Array<f32> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Array::<f32>::extract(value).map(|values| Value::from(values.to_vec()))
    }
}

impl HdValueNormalize for Array<f64> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Array::<f64>::extract(value).map(|values| Value::from_no_hash(values.to_vec()))
    }
}

impl HdValueNormalize for Array<Token> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Array::<Token>::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Vec<Vec2f> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec::<Vec2f>::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Vec<Vec3f> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec::<Vec3f>::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for Vec2i {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec2i::extract(value).map(Value::from_no_hash)
    }
}

impl HdValueNormalize for Vec4f {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec4f::extract(value).map(Value::from)
    }
}

impl HdValueNormalize for std::vec::Vec<Vec3i> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec::<Vec3i>::extract(value).map(Value::from_no_hash)
    }
}

impl HdValueNormalize for std::vec::Vec<Vec4i> {
    fn normalize_value(_attribute: &Attribute, value: &Value) -> Option<Value> {
        Vec::<Vec4i>::extract(value).map(Value::from_no_hash)
    }
}

/// Typed data source wrapping a USD attribute.
///
/// Generic over `T` — the value type this data source provides. Matches C++
/// `UsdImagingDataSourceAttribute<T>` which inherits from
/// `HdTypedSampledDataSource<T>`.
///
/// # Type Parameter
///
/// * `T` — concrete value type (e.g. `Array<i32>`, `Token`, `bool`).
///   Must implement [`HdValueExtract`] for extraction from untyped `Value`.
///
/// # Examples
///
/// ```ignore
/// // Create typed data source for int array attribute (like faceVertexCounts)
/// let ds = DataSourceAttribute::<Array<i32>>::new(attr, globals, path);
/// let counts: Array<i32> = ds.get_typed_value(0.0);
/// ```
pub struct DataSourceAttribute<T: HdValueNormalize> {
    attribute: Attribute,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
    _phantom: PhantomData<T>,
}

impl<T: HdValueNormalize> Clone for DataSourceAttribute<T> {
    fn clone(&self) -> Self {
        Self {
            attribute: self.attribute.clone(),
            stage_globals: self.stage_globals.clone(),
            scene_index_path: self.scene_index_path.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T: HdValueNormalize> std::fmt::Debug for DataSourceAttribute<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceAttribute")
            .field("type", &std::any::type_name::<T>())
            .field("attribute", &self.attribute.name())
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl<T: HdValueNormalize> DataSourceAttribute<T> {
    /// Create a new typed attribute data source.
    ///
    /// Mirrors C++ `UsdImagingDataSourceAttribute<T>::New(usdAttr, stageGlobals, ...)`.
    ///
    /// # Arguments
    ///
    /// * `attribute` — USD attribute to wrap
    /// * `stage_globals` — stage globals for time context
    /// * `scene_index_path` — scene index path (for locator tracking / diagnostics)
    pub fn new(
        attribute: Attribute,
        stage_globals: DataSourceStageGlobalsHandle,
        scene_index_path: Path,
    ) -> Arc<Self> {
        Self::new_with_locator(
            attribute,
            stage_globals,
            scene_index_path,
            HdDataSourceLocator::empty(),
        )
    }

    /// Create a new typed attribute data source with an explicit Hydra locator.
    ///
    /// Matches the C++ `UsdImagingDataSourceAttributeNew(..., timeVaryingFlagLocator)`
    /// path so callers can register time-varying mapped attributes in stage globals.
    pub fn new_with_locator(
        attribute: Attribute,
        stage_globals: DataSourceStageGlobalsHandle,
        scene_index_path: Path,
        time_varying_flag_locator: HdDataSourceLocator,
    ) -> Arc<Self> {
        if !time_varying_flag_locator.is_empty() && attribute.value_might_be_time_varying() {
            stage_globals.flag_as_time_varying(&scene_index_path, &time_varying_flag_locator);
        }
        record_object_in_stage_globals(&stage_globals, &attribute);
        Arc::new(Self {
            attribute,
            stage_globals,
            scene_index_path,
            _phantom: PhantomData,
        })
    }

    /// Get the wrapped USD attribute.
    pub fn get_attribute(&self) -> &Attribute {
        &self.attribute
    }

    /// Get the attribute name.
    pub fn get_name(&self) -> Token {
        self.attribute.name()
    }

    /// Compute the time code for a given shutter offset.
    fn time_code(&self, shutter_offset: HdSampledDataSourceTime) -> TimeCode {
        let base_time = self.stage_globals.get_time();
        let time_value = base_time.value() + shutter_offset as f64;
        TimeCode::new(time_value)
    }
}

impl<T: HdValueNormalize + std::fmt::Debug> HdDataSourceBase for DataSourceAttribute<T> {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(self.get_value(0.0))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl<T: HdValueNormalize + std::fmt::Debug> HdSampledDataSource for DataSourceAttribute<T> {
    /// Returns the untyped value at the given shutter offset.
    ///
    /// C++ equivalent: `VtValue GetValue(Time shutterOffset) override`
    /// which returns `VtValue(GetTypedValue(shutterOffset))`.
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let time_code = self.time_code(shutter_offset);
        match self.attribute.get(time_code) {
            Some(value) => T::normalize_value(&self.attribute, &value).unwrap_or(value),
            None => Value::empty(),
        }
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let samples = self.attribute.get_time_samples();

        if samples.is_empty() {
            return false;
        }

        let base_time = self.stage_globals.get_time().value();
        let window_start = base_time + start_time as f64;
        let window_end = base_time + end_time as f64;

        let mut has_samples = false;
        for sample_time in &samples {
            if *sample_time >= window_start && *sample_time <= window_end {
                let relative_time = (*sample_time - base_time) as f32;
                out_sample_times.push(relative_time);
                has_samples = true;
            }
        }

        // Boundary samples for interpolation
        let mut prev_sample: Option<f64> = None;
        let mut next_sample: Option<f64> = None;

        for sample_time in &samples {
            if *sample_time < window_start {
                prev_sample = Some(*sample_time);
            }
            if *sample_time > window_end && next_sample.is_none() {
                next_sample = Some(*sample_time);
                break;
            }
        }

        if let Some(prev) = prev_sample {
            let relative_time = (prev - base_time) as f32;
            if !out_sample_times.contains(&relative_time) {
                out_sample_times.insert(0, relative_time);
            }
        }

        if let Some(next) = next_sample {
            let relative_time = (next - base_time) as f32;
            if !out_sample_times.contains(&relative_time) {
                out_sample_times.push(relative_time);
            }
        }

        out_sample_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        has_samples || !out_sample_times.is_empty()
    }
}

impl<T: HdValueNormalize + std::fmt::Debug> HdTypedSampledDataSource<T> for DataSourceAttribute<T> {
    /// Returns the typed value at the given shutter offset.
    ///
    /// C++ equivalent: `T GetTypedValue(Time shutterOffset) override` which
    /// calls `_usdAttrQuery.Get<T>(&result, time)`. In Rust, we go through
    /// `Value` since our attribute API is not generic, then extract `T` via
    /// [`HdValueExtract`].
    ///
    /// Returns `T::default()` if attribute is invalid or value can't be
    /// extracted — matching C++ zero-initialization semantics (`T result{}`).
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> T {
        let time_code = self.time_code(shutter_offset);
        match self.attribute.get(time_code) {
            Some(value) => {
                let value = T::normalize_value(&self.attribute, &value).unwrap_or(value);
                T::extract(&value).unwrap_or_default()
            }
            None => T::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Untyped alias for backward compatibility
// ---------------------------------------------------------------------------

/// Untyped attribute data source — alias for `DataSourceAttribute<Value>`.
///
/// Used when the value type is not known at construction time, or when only
/// the untyped `HdSampledDataSource` interface is needed.
pub type UntypedDataSourceAttribute = DataSourceAttribute<Value>;

/// Handle to an untyped attribute data source.
pub type DataSourceAttributeHandle = Arc<UntypedDataSourceAttribute>;

#[cfg(test)]
mod tests {
    use super::super::data_source_stage_globals::NoOpStageGlobals;
    use super::*;
    use crate::data_source_stage_globals::DataSourceStageGlobals;
    use std::sync::{Arc, Mutex};
    use usd_core::{Stage, common::InitialLoadSet};
    use usd_sdf::Layer;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[derive(Default)]
    struct RecordingStageGlobals {
        flagged: Mutex<Vec<(Path, HdDataSourceLocator)>>,
        assets: Mutex<Vec<Path>>,
    }

    impl DataSourceStageGlobals for RecordingStageGlobals {
        fn get_time(&self) -> usd_core::TimeCode {
            usd_core::TimeCode::default_time()
        }

        fn flag_as_time_varying(&self, hydra_path: &Path, locator: &HdDataSourceLocator) {
            self.flagged
                .lock()
                .expect("recording lock")
                .push((hydra_path.clone(), locator.clone()));
        }

        fn flag_as_asset_path_dependent(&self, usd_path: &Path) {
            self.assets
                .lock()
                .expect("recording lock")
                .push(usd_path.clone());
        }
    }

    #[test]
    fn test_typed_data_source_attribute_debug() {
        let attr = Attribute::invalid();
        let ds =
            DataSourceAttribute::<Value>::new(attr, create_test_globals(), Path::absolute_root());
        let debug_str = format!("{:?}", ds);
        assert!(debug_str.contains("DataSourceAttribute"));
    }

    #[test]
    fn test_typed_data_source_attribute_get_value() {
        let attr = Attribute::invalid();
        let ds =
            DataSourceAttribute::<Value>::new(attr, create_test_globals(), Path::absolute_root());

        // Invalid attribute returns empty value
        let value = ds.get_value(0.0);
        assert!(value.is_empty());
    }

    #[test]
    fn test_typed_data_source_attribute_sample_times() {
        let attr = Attribute::invalid();
        let ds =
            DataSourceAttribute::<Value>::new(attr, create_test_globals(), Path::absolute_root());

        let mut sample_times = Vec::new();
        let has_samples = ds.get_contributing_sample_times(-0.25, 0.25, &mut sample_times);
        assert!(!has_samples);
        assert!(sample_times.is_empty());
    }

    #[test]
    fn test_as_sampled_returns_self() {
        let attr = Attribute::invalid();
        let ds =
            DataSourceAttribute::<Value>::new(attr, create_test_globals(), Path::absolute_root());

        // as_sampled() must return Some — this is required for the adapter fallback
        assert!(ds.as_sampled().is_some());
    }

    #[test]
    fn test_untyped_point_array_value_is_normalized_from_usda_nested_values() {
        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    point3f[] points = [(0,0,0), (1,0,0), (0,1,0)]
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("data_source_attribute_points"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let mesh_path = Path::from_string("/Mesh").expect("mesh path");
        let attr = stage
            .get_prim_at_path(&mesh_path)
            .and_then(|prim| prim.get_attribute("points"))
            .expect("points attr");
        let ds = DataSourceAttribute::<Value>::new(attr, create_test_globals(), mesh_path);

        let value = ds.get_value(0.0);
        let inner_type = value
            .get::<Vec<Value>>()
            .and_then(|values| values.first())
            .and_then(|first| first.type_name())
            .unwrap_or("<no-inner-type>");
        let points = value.as_vec_clone::<Vec3f>().unwrap_or_else(|| {
            panic!(
                "normalized point array payload root_type={:?} inner_type={}",
                value.type_name(),
                inner_type,
            )
        });
        assert_eq!(points[2], Vec3f::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_new_with_locator_flags_time_varying_attribute() {
        let usda = r#"#usda 1.0
def Scope "Scope" {
    float anim.timeSamples = {
        0: 1,
        1: 2,
    }
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("data_source_attribute_time_varying"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let prim_path = Path::from_string("/Scope").expect("scope path");
        let attr = stage
            .get_prim_at_path(&prim_path)
            .and_then(|prim| prim.get_attribute("anim"))
            .expect("anim attr");
        let globals_impl = Arc::new(RecordingStageGlobals::default());
        let globals: DataSourceStageGlobalsHandle = globals_impl.clone();
        let locator = HdDataSourceLocator::from_token(Token::new("anim"));

        let _ = DataSourceAttribute::<Value>::new_with_locator(
            attr,
            globals,
            prim_path.clone(),
            locator.clone(),
        );

        let flagged = globals_impl.flagged.lock().expect("recording lock");
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0], (prim_path, locator));
    }
}
