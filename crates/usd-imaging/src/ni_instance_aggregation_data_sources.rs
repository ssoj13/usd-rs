//! Instance aggregation data sources for instancer prims.
//!
//! Port of _PrimvarValueDataSource, _PrimvarDataSource, _InstanceTransform*,
//! _PrimvarsDataSource, _InstanceIndicesDataSource, _InstanceLocationsDataSource,
//! _InstancerTopologyDataSource, _InstancerPrimSource from niInstanceAggregationSceneIndex.cpp.

use usd_gf::Matrix4d;
use usd_hd::data_source::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdRetainedSampledDataSource,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, HdSampledDataSourceHandle,
    HdSampledDataSourceTime, HdTypedSampledDataSource, HdVectorDataSource,
};
use usd_hd::schema::{
    HdInstancedBySchema, HdInstancerTopologySchema, HdPrimvarsSchema, HdVisibilitySchema,
    HdXformSchema,
};
use usd_hd::tokens::INSTANCE_TRANSFORMS;
use usd_tf::Token as TfToken;
use usd_vt::{Value, ValueVisitor, visit_value};
// Instancer topology field names (schema uses these; instancer module is private)
const INSTANCE_INDICES: &str = "instanceIndices";
const INSTANCE_LOCATIONS: &str = "instanceLocations";
const MASK: &str = "mask";
const PROTOTYPES: &str = "prototypes";
use crate::ni_instance_aggregation_impl;
use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_sdf::Path as SdfPath;
use usd_trace::trace_function;

/// Shared mutable instance set for observer updates. Data sources lock when reading.
pub type SharedInstanceSet = Arc<RwLock<HashSet<SdfPath>>>;

fn shared_len(s: &SharedInstanceSet) -> usize {
    s.read().len()
}
fn shared_iter(s: &SharedInstanceSet) -> Vec<SdfPath> {
    s.read().iter().cloned().collect()
}
/// Iteration in sorted order for deterministic instance index correspondence.
/// C++ uses SdfPathSet (std::set) with sorted iteration.
fn shared_iter_sorted(s: &SharedInstanceSet) -> Vec<SdfPath> {
    let mut v = shared_iter(s);
    v.sort();
    v
}

// Primvar schema tokens (HdPrimvarSchemaTokens)
const PRIMVAR_VALUE: &str = "primvarValue";
const INTERPOLATION: &str = "interpolation";
const ROLE: &str = "role";
const INTERPOLATION_INSTANCE: &str = "instance";

/// Creates [0, 1, ..., n-1]. Port of _Range.
fn range(n: usize) -> Vec<i32> {
    (0..n as i32).collect()
}

// =============================================================================
// InstanceIndicesDataSource - HdVectorDataSource with 1 element: [0,1,...,n-1]
// =============================================================================

/// Data source for instancerTopology.instanceIndices.
/// Returns a vector with one element: the array [0, 1, ..., n-1].
struct InstanceIndicesDataSource {
    instances: SharedInstanceSet,
}

impl InstanceIndicesDataSource {
    fn new(instances: SharedInstanceSet) -> Arc<Self> {
        Arc::new(Self { instances })
    }
}

impl HdDataSourceBase for InstanceIndicesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_vector(&self) -> Option<usd_hd::HdVectorDataSourceHandle> {
        Some(Arc::new(self.clone()) as usd_hd::HdVectorDataSourceHandle)
    }
}

impl HdVectorDataSource for InstanceIndicesDataSource {
    fn get_num_elements(&self) -> usize {
        1
    }
    fn get_element(&self, _element: usize) -> Option<HdDataSourceBaseHandle> {
        Some(
            HdRetainedTypedSampledDataSource::new(range(shared_len(&self.instances)))
                as HdDataSourceBaseHandle,
        )
    }
}

impl Clone for InstanceIndicesDataSource {
    fn clone(&self) -> Self {
        Self {
            instances: Arc::clone(&self.instances),
        }
    }
}

impl std::fmt::Debug for InstanceIndicesDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceIndicesDataSource")
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// InstanceLocationsDataSource - path array for picking
// =============================================================================

/// Data source for instancerTopology.instanceLocations.
/// Returns instance paths in stable order.
struct InstanceLocationsDataSource {
    instances: SharedInstanceSet,
}

impl InstanceLocationsDataSource {
    fn new(instances: SharedInstanceSet) -> Arc<Self> {
        Arc::new(Self { instances })
    }
}

impl HdDataSourceBase for InstanceLocationsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdTypedSampledDataSource<Vec<SdfPath>> for InstanceLocationsDataSource {
    fn get_typed_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Vec<SdfPath> {
        shared_iter_sorted(&self.instances)
    }
}

impl HdSampledDataSource for InstanceLocationsDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> usd_vt::Value {
        usd_vt::Value::from_no_hash(self.get_typed_value(shutter_offset))
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

impl Clone for InstanceLocationsDataSource {
    fn clone(&self) -> Self {
        Self {
            instances: Arc::clone(&self.instances),
        }
    }
}

impl std::fmt::Debug for InstanceLocationsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceLocationsDataSource")
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// InstanceTransformPrimvarValueDataSource - xform matrices from instances
// =============================================================================

/// Data source for primvars:hydra:instanceTransforms:primvarValue.
/// Extracts transforms from each instance's xform schema.
/// Port of _InstanceTransformPrimvarValueDataSource.
struct InstanceTransformPrimvarValueDataSource {
    xform_sources: Vec<HdSampledDataSourceHandle>,
}

impl InstanceTransformPrimvarValueDataSource {
    fn new(input_scene: &dyn usd_hd::HdSceneIndexBase, instances: &SharedInstanceSet) -> Arc<Self> {
        let inst_vec = shared_iter_sorted(instances);
        let mut xform_sources: Vec<HdSampledDataSourceHandle> = Vec::with_capacity(inst_vec.len());
        for instance_path in &inst_vec {
            if let Some(ref ds) = input_scene.get_prim(instance_path).data_source {
                let xform = HdXformSchema::get_from_parent(ds);
                if let Some(matrix_ds) = xform.get_matrix() {
                    xform_sources.push(matrix_ds as HdSampledDataSourceHandle);
                } else {
                    let identity = HdRetainedTypedSampledDataSource::new(Matrix4d::identity());
                    xform_sources.push(identity as HdSampledDataSourceHandle);
                }
            } else {
                let identity = HdRetainedTypedSampledDataSource::new(Matrix4d::identity());
                xform_sources.push(identity as HdSampledDataSourceHandle);
            }
        }
        Arc::new(Self { xform_sources })
    }
}

impl HdDataSourceBase for InstanceTransformPrimvarValueDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdTypedSampledDataSource<Vec<Matrix4d>> for InstanceTransformPrimvarValueDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Vec<Matrix4d> {
        trace_function!();
        self.xform_sources
            .iter()
            .map(|src| {
                let val = src.get_value(shutter_offset);
                val.get::<Matrix4d>()
                    .cloned()
                    .unwrap_or_else(Matrix4d::identity)
            })
            .collect()
    }
}

impl HdSampledDataSource for InstanceTransformPrimvarValueDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> usd_vt::Value {
        usd_vt::Value::from(self.get_typed_value(shutter_offset))
    }
    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        trace_function!();
        usd_hd::data_source::hd_merge_contributing_sample_times(
            &self.xform_sources,
            start_time,
            end_time,
            out_sample_times,
        )
    }
}

impl Clone for InstanceTransformPrimvarValueDataSource {
    fn clone(&self) -> Self {
        Self {
            xform_sources: self.xform_sources.clone(),
        }
    }
}

impl std::fmt::Debug for InstanceTransformPrimvarValueDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceTransformPrimvarValueDataSource")
            .field("count", &self.xform_sources.len())
            .finish()
    }
}

// =============================================================================
// InstanceTransformPrimvarDataSource - primvars:hydra:instanceTransforms
// =============================================================================

/// Container for primvars:hydra:instanceTransforms (primvarValue, interpolation, role).
struct InstanceTransformPrimvarDataSource {
    input_scene: usd_hd::HdSceneIndexHandle,
    instances: SharedInstanceSet,
}

impl InstanceTransformPrimvarDataSource {
    fn new(input_scene: usd_hd::HdSceneIndexHandle, instances: SharedInstanceSet) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            instances,
        })
    }
}

impl HdContainerDataSource for InstanceTransformPrimvarDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new(PRIMVAR_VALUE),
            TfToken::new(INTERPOLATION),
            TfToken::new(ROLE),
        ]
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == INTERPOLATION {
            return Some(
                HdRetainedTypedSampledDataSource::new(TfToken::new(INTERPOLATION_INSTANCE))
                    as HdDataSourceBaseHandle,
            );
        }
        if name == PRIMVAR_VALUE {
            let guard = self.input_scene.read();
            let scene = &*guard;
            let ds = InstanceTransformPrimvarValueDataSource::new(scene, &self.instances);
            return Some(ds as HdDataSourceBaseHandle);
        }
        None
    }
}

impl HdDataSourceBase for InstanceTransformPrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl Clone for InstanceTransformPrimvarDataSource {
    fn clone(&self) -> Self {
        Self {
            input_scene: Arc::clone(&self.input_scene),
            instances: Arc::clone(&self.instances),
        }
    }
}

impl std::fmt::Debug for InstanceTransformPrimvarDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceTransformPrimvarDataSource")
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// PrimvarDataSource - primvars:NAME for constant primvars
// =============================================================================

// =============================================================================
// PrimvarValueAggregatorVisitor - VtVisitValue-style type dispatch for aggregation
// =============================================================================

/// Visitor that aggregates primvar values from all instances.
/// Port of _PrimvarValueDataSourceFactory + _PrimvarValueDataSource<T>::GetTypedValue.
/// Collects get_typed_primvar_value::<T> for each instance into Vec<T>, returns Value.
struct PrimvarValueAggregatorVisitor<'a> {
    input_scene: &'a dyn usd_hd::scene_index::HdSceneIndexBase,
    instances: &'a [SdfPath],
    primvar_name: &'a TfToken,
}

impl ValueVisitor for PrimvarValueAggregatorVisitor<'_> {
    type Output = Option<Value>;

    fn visit_int(&mut self, _v: i32) -> Self::Output {
        let result: Vec<i32> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_float(&mut self, _v: f32) -> Self::Output {
        let result: Vec<f32> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_double(&mut self, _v: f64) -> Self::Output {
        let result: Vec<f64> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_string(&mut self, _v: &str) -> Self::Output {
        let result: Vec<String> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_token(&mut self, _v: &TfToken) -> Self::Output {
        let result: Vec<TfToken> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_matrix4d(&mut self, _v: &Matrix4d) -> Self::Output {
        let result: Vec<Matrix4d> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_vec2f(&mut self, _v: usd_gf::Vec2f) -> Self::Output {
        let result: Vec<usd_gf::Vec2f> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_vec2d(&mut self, _v: usd_gf::Vec2d) -> Self::Output {
        let result: Vec<usd_gf::Vec2d> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_vec3f(&mut self, _v: usd_gf::Vec3f) -> Self::Output {
        let result: Vec<usd_gf::Vec3f> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_vec3d(&mut self, _v: usd_gf::Vec3d) -> Self::Output {
        let result: Vec<usd_gf::Vec3d> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_vec4f(&mut self, _v: usd_gf::Vec4f) -> Self::Output {
        let result: Vec<usd_gf::Vec4f> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_vec4d(&mut self, _v: usd_gf::Vec4d) -> Self::Output {
        let result: Vec<usd_gf::Vec4d> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from_no_hash(result))
    }

    fn visit_bool(&mut self, _v: bool) -> Self::Output {
        let result: Vec<bool> = self
            .instances
            .iter()
            .map(|p| {
                ni_instance_aggregation_impl::get_typed_primvar_value(
                    self.input_scene,
                    p,
                    self.primvar_name,
                )
            })
            .collect();
        Some(Value::from(result))
    }

    fn visit_unknown(&mut self, v: &Value) -> Self::Output {
        // C++ _PrimvarValueDataSourceFactory: special case for SdfPath (skel:animationSource)
        if v.get::<SdfPath>().is_some() {
            let result: Vec<SdfPath> = self
                .instances
                .iter()
                .map(|p| {
                    ni_instance_aggregation_impl::get_typed_primvar_value(
                        self.input_scene,
                        p,
                        self.primvar_name,
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
        // Special case for AssetPath (not in ValueVisitor trait due to circular dep)
        if v.get::<usd_sdf::AssetPath>().is_some() {
            let result: Vec<usd_sdf::AssetPath> = self
                .instances
                .iter()
                .map(|p| {
                    ni_instance_aggregation_impl::get_typed_primvar_value(
                        self.input_scene,
                        p,
                        self.primvar_name,
                    )
                })
                .collect();
            return Some(Value::from_no_hash(result));
        }
        // Handle array types: C++ _PrimvarValueDataSource for VtArray<T> extracts first element
        // from each instance's array. Explicit dispatch for supported array element types.
        macro_rules! try_array {
            ($t:ty, $from:expr) => {
                if let Some(arr) = v.get::<Vec<$t>>() {
                    if !arr.is_empty() {
                        let result: Vec<$t> = self
                            .instances
                            .iter()
                            .map(|p| {
                                ni_instance_aggregation_impl::get_typed_primvar_value::<$t>(
                                    self.input_scene,
                                    p,
                                    self.primvar_name,
                                )
                            })
                            .collect();
                        return Some($from(result));
                    }
                }
            };
        }
        try_array!(i32, Value::from);
        try_array!(f32, Value::from_no_hash);
        try_array!(f64, Value::from_no_hash);
        try_array!(usd_gf::Vec3f, Value::from);
        try_array!(usd_gf::Vec3d, Value::from);
        try_array!(Matrix4d, Value::from);
        try_array!(TfToken, Value::from);
        try_array!(String, Value::from);
        try_array!(bool, Value::from);
        try_array!(SdfPath, Value::from_no_hash);
        None
    }
}

/// Makes primvar value data source by aggregating from instances.
/// Port of _MakePrimvarValueDataSource: VtVisitValue on first instance's value
/// to dispatch by type, then create aggregated array from all instances.
fn make_primvar_value_data_source(
    input_scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    instances: &SharedInstanceSet,
    primvar_name: &TfToken,
) -> Option<HdDataSourceBaseHandle> {
    let inst_vec = shared_iter_sorted(instances);
    let first = inst_vec.first()?;
    let value = ni_instance_aggregation_impl::get_primvar_value(input_scene, first, primvar_name)?;
    let mut visitor = PrimvarValueAggregatorVisitor {
        input_scene,
        instances: &inst_vec,
        primvar_name,
    };
    let aggregated = visit_value(&value, &mut visitor)?;
    Some(HdRetainedSampledDataSource::new(aggregated) as HdDataSourceBaseHandle)
}

/// Container for primvars:NAME (primvarValue, interpolation=instance, role).
struct PrimvarDataSource {
    input_scene: usd_hd::HdSceneIndexHandle,
    instances: SharedInstanceSet,
    primvar_name: TfToken,
}

impl PrimvarDataSource {
    fn new(
        input_scene: usd_hd::HdSceneIndexHandle,
        instances: SharedInstanceSet,
        primvar_name: TfToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            instances,
            primvar_name,
        })
    }
}

impl HdContainerDataSource for PrimvarDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new(PRIMVAR_VALUE),
            TfToken::new(INTERPOLATION),
            TfToken::new(ROLE),
        ]
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == INTERPOLATION {
            return Some(
                HdRetainedTypedSampledDataSource::new(TfToken::new(INTERPOLATION_INSTANCE))
                    as HdDataSourceBaseHandle,
            );
        }
        if name == PRIMVAR_VALUE {
            let guard = self.input_scene.read();
            let scene = &*guard;
            return make_primvar_value_data_source(scene, &self.instances, &self.primvar_name);
        }
        if name == ROLE {
            let guard = self.input_scene.read();
            let scene = &*guard;
            // C++: *(_instances->begin()) — first in set order
            let first = shared_iter_sorted(&self.instances).into_iter().next()?;
            let primvar = ni_instance_aggregation_impl::get_primvar_schema(
                scene,
                &first,
                &self.primvar_name,
            )?;
            return primvar.get(&TfToken::new(ROLE));
        }
        None
    }
}

impl HdDataSourceBase for PrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl Clone for PrimvarDataSource {
    fn clone(&self) -> Self {
        Self {
            input_scene: Arc::clone(&self.input_scene),
            instances: Arc::clone(&self.instances),
            primvar_name: self.primvar_name.clone(),
        }
    }
}

impl std::fmt::Debug for PrimvarDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarDataSource")
            .field("name", &self.primvar_name.as_str())
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// PrimvarsDataSource - primvars for instancer
// =============================================================================

/// Container for instancer primvars (instanceTransforms + constant primvars).
struct PrimvarsDataSource {
    input_scene: usd_hd::HdSceneIndexHandle,
    instances: SharedInstanceSet,
}

impl PrimvarsDataSource {
    fn new(input_scene: usd_hd::HdSceneIndexHandle, instances: SharedInstanceSet) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            instances,
        })
    }
}

impl HdContainerDataSource for PrimvarsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mut result = if shared_len(&self.instances) == 0 {
            Vec::new()
        } else {
            let guard = self.input_scene.read();
            let scene = &*guard;
            // C++: *(_instances->begin()) — first in set (sorted) order
            let first = shared_iter_sorted(&self.instances)
                .into_iter()
                .next()
                .unwrap();
            ni_instance_aggregation_impl::get_constant_primvar_names(scene, &first)
        };
        result.push(INSTANCE_TRANSFORMS.clone());
        result
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == &*INSTANCE_TRANSFORMS {
            return Some(InstanceTransformPrimvarDataSource::new(
                Arc::clone(&self.input_scene),
                Arc::clone(&self.instances),
            ) as HdDataSourceBaseHandle);
        }
        if shared_len(&self.instances) > 0 {
            let guard = self.input_scene.read();
            let scene = &*guard;
            // C++: *(_instances->begin()) — first in set order
            let first = shared_iter_sorted(&self.instances)
                .into_iter()
                .next()
                .unwrap();
            if ni_instance_aggregation_impl::is_constant_primvar(scene, &first, name) {
                return Some(PrimvarDataSource::new(
                    Arc::clone(&self.input_scene),
                    Arc::clone(&self.instances),
                    name.clone(),
                ) as HdDataSourceBaseHandle);
            }
        }
        None
    }
}

impl HdDataSourceBase for PrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl Clone for PrimvarsDataSource {
    fn clone(&self) -> Self {
        Self {
            input_scene: Arc::clone(&self.input_scene),
            instances: Arc::clone(&self.instances),
        }
    }
}

impl std::fmt::Debug for PrimvarsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarsDataSource")
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// GetVisibility / ComputeMask
// =============================================================================

fn get_visibility(scene: &dyn usd_hd::scene_index::HdSceneIndexBase, prim_path: &SdfPath) -> bool {
    let prim = scene.get_prim(prim_path);
    if let Some(ref ds) = prim.data_source {
        let schema = HdVisibilitySchema::get_from_parent(ds);
        if let Some(vis_ds) = schema.get_visibility() {
            return vis_ds.get_typed_value(0.0);
        }
    }
    true
}

fn compute_mask(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    instances: &SharedInstanceSet,
) -> Vec<bool> {
    shared_iter_sorted(instances)
        .into_iter()
        .map(|p| get_visibility(scene, &p))
        .collect()
}

// =============================================================================
// InstancerTopologyDataSource
// =============================================================================

/// Container for instancerTopology (instanceIndices, prototypes, instanceLocations, mask).
struct InstancerTopologyDataSource {
    input_scene: usd_hd::HdSceneIndexHandle,
    prototype_path: SdfPath,
    instances: SharedInstanceSet,
}

impl InstancerTopologyDataSource {
    fn new(
        input_scene: usd_hd::HdSceneIndexHandle,
        prototype_path: SdfPath,
        instances: SharedInstanceSet,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            prototype_path,
            instances,
        })
    }
}

impl HdContainerDataSource for InstancerTopologyDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new(INSTANCE_INDICES),
            TfToken::new(PROTOTYPES),
            TfToken::new(INSTANCE_LOCATIONS),
            TfToken::new(MASK),
        ]
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == INSTANCE_INDICES {
            return Some(InstanceIndicesDataSource::new(Arc::clone(&self.instances))
                as HdDataSourceBaseHandle);
        }
        if name == PROTOTYPES {
            return Some(
                HdRetainedTypedSampledDataSource::new(vec![self.prototype_path.clone()])
                    as HdDataSourceBaseHandle,
            );
        }
        if name == INSTANCE_LOCATIONS {
            return Some(
                InstanceLocationsDataSource::new(Arc::clone(&self.instances))
                    as HdDataSourceBaseHandle,
            );
        }
        if name == MASK {
            let guard = self.input_scene.read();
            let scene = &*guard;
            let mask = compute_mask(scene, &self.instances);
            return Some(HdRetainedTypedSampledDataSource::new(mask) as HdDataSourceBaseHandle);
        }
        None
    }
}

impl HdDataSourceBase for InstancerTopologyDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl Clone for InstancerTopologyDataSource {
    fn clone(&self) -> Self {
        Self {
            input_scene: Arc::clone(&self.input_scene),
            prototype_path: self.prototype_path.clone(),
            instances: Arc::clone(&self.instances),
        }
    }
}

impl std::fmt::Debug for InstancerTopologyDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstancerTopologyDataSource")
            .field("prototype", &self.prototype_path)
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}

// =============================================================================
// InstancerPrimSource - top-level instancer data source
// =============================================================================

/// Top-level data source for instancer prim (instancedBy, instancerTopology, primvars).
pub struct InstancerPrimSource {
    input_scene: usd_hd::HdSceneIndexHandle,
    enclosing_prototype_root: SdfPath,
    prototype_path: SdfPath,
    instances: SharedInstanceSet,
    for_native_prototype: bool,
}

impl InstancerPrimSource {
    /// Creates a new instancer prim source for the given prototype and instance set.
    pub fn new(
        input_scene: usd_hd::HdSceneIndexHandle,
        enclosing_prototype_root: SdfPath,
        prototype_path: SdfPath,
        instances: SharedInstanceSet,
        for_native_prototype: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_scene,
            enclosing_prototype_root,
            prototype_path,
            instances,
            for_native_prototype,
        })
    }
}

impl HdContainerDataSource for InstancerPrimSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            (**HdInstancedBySchema::get_schema_token()).clone(),
            (**HdInstancerTopologySchema::get_schema_token()).clone(),
            (**HdPrimvarsSchema::get_schema_token()).clone(),
        ]
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == &**HdInstancedBySchema::get_schema_token() {
            let guard = self.input_scene.read();
            let scene = &*guard;
            let prim = scene.get_prim(&self.enclosing_prototype_root);
            if let Some(ref ds) = prim.data_source {
                let schema = HdInstancedBySchema::get_from_parent(ds);
                if schema.is_defined() {
                    if let Some(cont) = schema.get_container() {
                        return Some(cont.clone());
                    }
                }
            }
            if self.for_native_prototype {
                if let Some(ds) =
                    crate::ni_prototype_scene_index::UsdImagingNiPrototypeSceneIndex::get_instanced_by_data_source()
                {
                    return Some(ds as HdDataSourceBaseHandle);
                }
            }
            return None;
        }
        if name == &**HdInstancerTopologySchema::get_schema_token() {
            return Some(InstancerTopologyDataSource::new(
                Arc::clone(&self.input_scene),
                self.prototype_path.clone(),
                Arc::clone(&self.instances),
            ) as HdDataSourceBaseHandle);
        }
        if name == &**HdPrimvarsSchema::get_schema_token() {
            return Some(PrimvarsDataSource::new(
                Arc::clone(&self.input_scene),
                Arc::clone(&self.instances),
            ) as HdDataSourceBaseHandle);
        }
        None
    }
}

impl HdDataSourceBase for InstancerPrimSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl Clone for InstancerPrimSource {
    fn clone(&self) -> Self {
        Self {
            input_scene: Arc::clone(&self.input_scene),
            enclosing_prototype_root: self.enclosing_prototype_root.clone(),
            prototype_path: self.prototype_path.clone(),
            instances: Arc::clone(&self.instances),
            for_native_prototype: self.for_native_prototype,
        }
    }
}

impl std::fmt::Debug for InstancerPrimSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstancerPrimSource")
            .field("enclosing", &self.enclosing_prototype_root)
            .field("prototype", &self.prototype_path)
            .field("count", &shared_len(&self.instances))
            .finish()
    }
}
