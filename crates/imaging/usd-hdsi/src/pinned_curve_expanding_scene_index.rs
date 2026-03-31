
//! Pinned curve expanding scene index.
//!
//! Expands pinned endpoint curves (bspline, catmullRom, centripetalCatmullRom)
//! into nonperiodic curves by replicating first/last vertex values.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
    HdSampledDataSource, HdSampledDataSourceTime, HdTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    BASIS_CURVES_TOPOLOGY, CURVE_INDICES, CURVE_VERTEX_COUNTS, GEOM_SUBSET_INDICES,
    HdBasisCurvesSchema, HdBasisCurvesTopologySchema, HdGeomSubsetSchema, HdPrimvarSchema,
    HdPrimvarsSchema, PRIMVAR_VALUE, TYPE_POINT_SET, WRAP,
};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::visit_value;
use usd_vt::visit_value::ValueVisitor;
use usd_vt::{Array, Value};

fn safe_get_typed_value<T: Clone + Default + Send + Sync + 'static>(
    ds: &Option<Arc<dyn HdTypedSampledDataSource<T> + Send + Sync>>,
) -> T {
    ds.as_ref()
        .map(|d| d.get_typed_value(0.0))
        .unwrap_or_default()
}

/// Compute expanded array by replicating first/last values per curve.
fn compute_expanded_value<T: Clone>(
    input: &[T],
    per_curve_counts: &[i32],
    num_repeat: usize,
) -> Vec<T> {
    let num_curves = per_curve_counts.len();
    let mut authored_start: Vec<usize> = Vec::with_capacity(num_curves);
    let mut authored_sum = 0usize;
    for &vc in per_curve_counts {
        authored_start.push(authored_sum);
        authored_sum += vc as usize;
    }

    if input.len() != authored_sum {
        return input.to_vec();
    }

    let output_size = input.len() + 2 * num_repeat * num_curves;
    let mut output: Vec<T> = Vec::with_capacity(output_size);

    for curve_idx in 0..num_curves {
        if per_curve_counts[curve_idx] == 0 {
            continue;
        }
        let input_start = authored_start[curve_idx];
        let input_end = input_start + per_curve_counts[curve_idx] as usize;
        let out_start = input_start + 2 * num_repeat * curve_idx;

        while output.len() < out_start {
            output.push(input[input_start].clone());
        }

        for _ in 0..num_repeat {
            output.push(input[input_start].clone());
        }
        for i in input_start..input_end {
            output.push(input[i].clone());
        }
        for _ in 0..num_repeat {
            output.push(input[input_end - 1].clone());
        }
    }

    output
}

/// Visitor to create retained expanded data source from Value holding an array.
struct ExpandVisitor {
    per_curve_counts: Vec<i32>,
    num_extra_ends: usize,
}

impl ValueVisitor for ExpandVisitor {
    type Output = Option<HdDataSourceBaseHandle>;

    fn visit_array<T: Clone + Send + Sync + 'static>(&mut self, arr: &Array<T>) -> Self::Output {
        use std::any::TypeId;
        if TypeId::of::<T>() != TypeId::of::<i32>() {
            return None;
        }
        // SAFETY: TypeId check above confirms T == i32, so the cast is valid
        #[allow(unsafe_code)]
        let arr_i32 = unsafe { &*(arr as *const Array<T> as *const Array<i32>) };
        if arr_i32.is_empty() {
            return Some(
                HdRetainedTypedSampledDataSource::new(Array::<i32>::default())
                    as HdDataSourceBaseHandle,
            );
        }
        let vec: Vec<i32> = arr_i32.to_vec();
        let expanded = compute_expanded_value(&vec, &self.per_curve_counts, self.num_extra_ends);
        Some(HdRetainedTypedSampledDataSource::new(Array::from(expanded)) as HdDataSourceBaseHandle)
    }

    fn visit_unknown(&mut self, _v: &Value) -> Self::Output {
        None
    }
}

/// Try to expand primvar value/indices. Returns expanded DS if possible.
fn try_expand_primvar_value(
    result: &HdDataSourceBaseHandle,
    curve_vertex_counts: &[i32],
    expansion_size: usize,
) -> Option<HdDataSourceBaseHandle> {
    let val = try_get_sampled_value(result)?;
    let mut visitor = ExpandVisitor {
        per_curve_counts: curve_vertex_counts.to_vec(),
        num_extra_ends: expansion_size,
    };
    visit_value(&val, &mut visitor)
}

/// Try to get Value from a data source (for known retained typed types).
fn try_get_sampled_value(handle: &HdDataSourceBaseHandle) -> Option<Value> {
    let any = handle.as_any();
    if let Some(ds) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Array<i32>>>() {
        return Some(Value::from(ds.get_typed_value(0.0)));
    }
    None
}

/// Primvar container that expands vertex/varying values when needed.
#[derive(Debug, Clone)]
struct PrimvarDataSource {
    input: HdContainerDataSourceHandle,
    primvar_name: TfToken,
    curve_vertex_counts: Vec<i32>,
    num_extra_ends: usize,
    has_curve_indices: bool,
}

impl PrimvarDataSource {
    fn new(
        input: HdContainerDataSourceHandle,
        primvar_name: TfToken,
        curve_vertex_counts: Vec<i32>,
        num_extra_ends: usize,
        has_curve_indices: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            primvar_name,
            curve_vertex_counts,
            num_extra_ends,
            has_curve_indices,
        })
    }
}

impl HdDataSourceBase for PrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: Arc::clone(&self.input),
            primvar_name: self.primvar_name.clone(),
            curve_vertex_counts: self.curve_vertex_counts.clone(),
            num_extra_ends: self.num_extra_ends,
            has_curve_indices: self.has_curve_indices,
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimvarDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input.get(name)?;

        if name.as_str() == PRIMVAR_VALUE.as_str()
            || name.as_str() == usd_hd::schema::PRIMVAR_INDICES.as_str()
        {
            let pvs = HdPrimvarSchema::new(Arc::clone(&self.input));
            let interp: TfToken = safe_get_typed_value(&pvs.get_interpolation());

            let mut expansion_size = 0usize;
            if interp == *usd_hd::schema::PRIMVAR_VERTEX {
                expansion_size = if self.has_curve_indices {
                    0
                } else {
                    self.num_extra_ends
                };
            } else if interp == *usd_hd::schema::PRIMVAR_VARYING {
                expansion_size = self.num_extra_ends.saturating_sub(1);
            }

            if expansion_size > 0 {
                if let Some(expanded) =
                    try_expand_primvar_value(&result, &self.curve_vertex_counts, expansion_size)
                {
                    return Some(expanded);
                }
            }
        }

        Some(result)
    }
}

/// Primvars container that wraps each primvar in PrimvarDataSource.
#[derive(Debug, Clone)]
struct PrimvarsDataSource {
    input: HdContainerDataSourceHandle,
    curve_vertex_counts: Vec<i32>,
    num_extra_ends: usize,
    has_curve_indices: bool,
}

impl PrimvarsDataSource {
    fn new(
        input: HdContainerDataSourceHandle,
        curve_vertex_counts: Vec<i32>,
        num_extra_ends: usize,
        has_curve_indices: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            curve_vertex_counts,
            num_extra_ends,
            has_curve_indices,
        })
    }
}

impl HdDataSourceBase for PrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: Arc::clone(&self.input),
            curve_vertex_counts: self.curve_vertex_counts.clone(),
            num_extra_ends: self.num_extra_ends,
            has_curve_indices: self.has_curve_indices,
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimvarsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input.get(name)?;
        if let Some(pc) = cast_to_container(&result) {
            Some(PrimvarDataSource::new(
                pc,
                name.clone(),
                self.curve_vertex_counts.clone(),
                self.num_extra_ends,
                self.has_curve_indices,
            ) as HdDataSourceBaseHandle)
        } else {
            Some(result)
        }
    }
}

/// Topology container: curveVertexCounts, curveIndices, wrap overrides.
#[derive(Debug, Clone)]
struct TopologyDataSource {
    input: HdContainerDataSourceHandle,
    curve_vertex_counts: Vec<i32>,
    num_extra_ends: usize,
}

impl TopologyDataSource {
    fn new(
        input: HdContainerDataSourceHandle,
        curve_vertex_counts: Vec<i32>,
        num_extra_ends: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            curve_vertex_counts,
            num_extra_ends,
        })
    }
}

impl HdDataSourceBase for TopologyDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: Arc::clone(&self.input),
            curve_vertex_counts: self.curve_vertex_counts.clone(),
            num_extra_ends: self.num_extra_ends,
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for TopologyDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let ts = HdBasisCurvesTopologySchema::new(Arc::clone(&self.input));
        let result = self.input.get(name)?;

        if name.as_str() == CURVE_VERTEX_COUNTS.as_str() {
            let mut counts = self.curve_vertex_counts.clone();
            for c in &mut counts {
                *c += 2 * self.num_extra_ends as i32;
            }
            return Some(HdRetainedTypedSampledDataSource::new(Array::from(counts))
                as HdDataSourceBaseHandle);
        }

        if name.as_str() == CURVE_INDICES.as_str() {
            if let Some(ci) = ts.get_curve_indices() {
                let curve_indices = ci.get_typed_value(0.0);
                if !curve_indices.is_empty() {
                    let vec: Vec<i32> = curve_indices.to_vec();
                    let expanded = compute_expanded_value(
                        &vec,
                        &self.curve_vertex_counts,
                        self.num_extra_ends,
                    );
                    return Some(HdRetainedTypedSampledDataSource::new(Array::from(expanded))
                        as HdDataSourceBaseHandle);
                }
            }
        }

        if name.as_str() == WRAP.as_str() {
            return Some(
                HdRetainedTypedSampledDataSource::new(tokens::NONPERIODIC.clone())
                    as HdDataSourceBaseHandle,
            );
        }

        Some(result)
    }
}

/// Basis curves container that wraps topology.
#[derive(Debug, Clone)]
struct BasisCurvesDataSource {
    input: HdContainerDataSourceHandle,
    curve_vertex_counts: Vec<i32>,
    num_extra_ends: usize,
}

impl BasisCurvesDataSource {
    fn new(
        input: HdContainerDataSourceHandle,
        curve_vertex_counts: Vec<i32>,
        num_extra_ends: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            curve_vertex_counts,
            num_extra_ends,
        })
    }
}

impl HdDataSourceBase for BasisCurvesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: Arc::clone(&self.input),
            curve_vertex_counts: self.curve_vertex_counts.clone(),
            num_extra_ends: self.num_extra_ends,
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for BasisCurvesDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input.get(name)?;
        if name.as_str() == BASIS_CURVES_TOPOLOGY.as_str() {
            if let Some(tc) = cast_to_container(&result) {
                return Some(TopologyDataSource::new(
                    tc,
                    self.curve_vertex_counts.clone(),
                    self.num_extra_ends,
                ) as HdDataSourceBaseHandle);
            }
        }
        Some(result)
    }
}

/// Prim-level overlay: wraps basisCurves and primvars when pinned.
#[derive(Debug, Clone)]
struct PrimDataSource {
    input: HdContainerDataSourceHandle,
}

impl PrimDataSource {
    fn new(input: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { input })
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: Arc::clone(&self.input),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input.get(name)?;

        let bcs = HdBasisCurvesSchema::get_from_parent(&self.input);
        if !bcs.is_defined() {
            return Some(result);
        }
        let ts = match bcs.get_topology() {
            Some(t) => t,
            None => return Some(result),
        };

        let wrap: TfToken = safe_get_typed_value(&ts.get_wrap());
        let basis: TfToken = safe_get_typed_value(&ts.get_basis());

        if wrap != *tokens::PINNED {
            return Some(result);
        }
        if basis != *tokens::BSPLINE
            && basis != *tokens::CATMULL_ROM
            && basis != *tokens::CENTRIPETAL_CATMULL_ROM
        {
            return Some(result);
        }

        let num_extra_ends = if basis == *tokens::BSPLINE { 2 } else { 1 };

        let curve_vertex_counts: Array<i32> = safe_get_typed_value(&ts.get_curve_vertex_counts());
        let curve_vertex_counts_vec: Vec<i32> = curve_vertex_counts.to_vec();

        if name.as_str() == HdBasisCurvesSchema::get_schema_token().as_str() {
            if let Some(bcc) = cast_to_container(&result) {
                return Some(
                    BasisCurvesDataSource::new(bcc, curve_vertex_counts_vec, num_extra_ends)
                        as HdDataSourceBaseHandle,
                );
            }
        }

        if name.as_str() == HdPrimvarsSchema::get_schema_token().as_str() {
            let curve_indices: Array<i32> = safe_get_typed_value(&ts.get_curve_indices());
            let has_curve_indices = !curve_indices.is_empty();
            if let Some(pc) = cast_to_container(&result) {
                return Some(PrimvarsDataSource::new(
                    pc,
                    curve_vertex_counts_vec,
                    num_extra_ends,
                    has_curve_indices,
                ) as HdDataSourceBaseHandle);
            }
        }

        Some(result)
    }
}

/// Compute expanded point indices for a point-set geom subset.
/// Original indices refer to pre-expansion vertices. After pinned curve expansion,
/// we replicate first/last of each curve - this maps each original index to the
/// corresponding expanded indices.
fn remap_point_set_indices_to_expanded(
    original_indices: &[i32],
    curve_vertex_counts: &[i32],
    num_extra_ends: usize,
) -> Vec<i32> {
    let num_curves = curve_vertex_counts.len();
    let mut authored_start: Vec<usize> = Vec::with_capacity(num_curves);
    let mut authored_sum = 0usize;
    for &vc in curve_vertex_counts {
        authored_start.push(authored_sum);
        authored_sum += vc as usize;
    }

    if authored_sum == 0 {
        return original_indices.to_vec();
    }

    let mut result = Vec::with_capacity(original_indices.len() * (1 + 2 * num_extra_ends));

    for &orig_idx in original_indices {
        let v = orig_idx as usize;
        if v >= authored_sum {
            result.push(orig_idx);
            continue;
        }
        // Find curve containing this vertex
        let mut curve_idx = 0;
        while curve_idx + 1 < num_curves && authored_start[curve_idx + 1] <= v {
            curve_idx += 1;
        }
        let count = curve_vertex_counts[curve_idx] as usize;
        if count == 0 {
            result.push(orig_idx);
            continue;
        }
        let local_k = v - authored_start[curve_idx];
        let curve_start: usize = (0..curve_idx)
            .map(|i| 2 * num_extra_ends + curve_vertex_counts[i] as usize)
            .sum();

        if local_k == 0 {
            for i in 0..=num_extra_ends {
                result.push((curve_start + i) as i32);
            }
        } else if local_k == count - 1 {
            let base = curve_start + num_extra_ends + count - 1;
            for i in 0..=num_extra_ends {
                result.push((base + i) as i32);
            }
        } else {
            result.push((curve_start + num_extra_ends + local_k) as i32);
        }
    }
    result
}

/// Geom subset indices override with point-set remapping for expanded curve points.
#[derive(Debug)]
struct SubsetIndicesDataSource {
    data_source: Arc<dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync>,
    type_source: Arc<dyn HdTypedSampledDataSource<TfToken> + Send + Sync>,
    parent_source: HdContainerDataSourceHandle,
}

impl HdDataSourceBase for SubsetIndicesDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            data_source: Arc::clone(&self.data_source),
            type_source: Arc::clone(&self.type_source),
            parent_source: self.parent_source.clone(),
        }) as HdDataSourceBaseHandle
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

impl HdSampledDataSource for SubsetIndicesDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start: HdSampledDataSourceTime,
        end: HdSampledDataSourceTime,
        out: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let mut from_ds = false;
        from_ds |= self
            .data_source
            .get_contributing_sample_times(start, end, out);
        from_ds |= self
            .type_source
            .get_contributing_sample_times(start, end, out);
        let bcs = HdBasisCurvesSchema::get_from_parent(&self.parent_source);
        if let Some(topo) = bcs.get_topology() {
            if let Some(w) = topo.get_wrap() {
                from_ds |= w.get_contributing_sample_times(start, end, out);
            }
            if let Some(b) = topo.get_basis() {
                from_ds |= b.get_contributing_sample_times(start, end, out);
            }
            if let Some(cvc) = topo.get_curve_vertex_counts() {
                from_ds |= cvc.get_contributing_sample_times(start, end, out);
            }
        }
        from_ds
    }
}

impl HdTypedSampledDataSource<Array<i32>> for SubsetIndicesDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Array<i32> {
        if self.type_source.get_typed_value(shutter_offset) == *TYPE_POINT_SET {
            let bcs = HdBasisCurvesSchema::get_from_parent(&self.parent_source);
            if !bcs.is_defined() {
                return self.data_source.get_typed_value(shutter_offset);
            }
            let ts = match bcs.get_topology() {
                Some(t) => t,
                None => return self.data_source.get_typed_value(shutter_offset),
            };
            let wrap: TfToken = safe_get_typed_value(&ts.get_wrap());
            let basis: TfToken = safe_get_typed_value(&ts.get_basis());
            if wrap != *tokens::PINNED {
                return self.data_source.get_typed_value(shutter_offset);
            }
            if basis != *tokens::BSPLINE
                && basis != *tokens::CATMULL_ROM
                && basis != *tokens::CENTRIPETAL_CATMULL_ROM
            {
                return self.data_source.get_typed_value(shutter_offset);
            }
            let num_extra_ends = if basis == *tokens::BSPLINE { 2 } else { 1 };
            let curve_vertex_counts: Array<i32> =
                safe_get_typed_value(&ts.get_curve_vertex_counts());
            let cvc_vec: Vec<i32> = curve_vertex_counts.to_vec();
            let original: Array<i32> = self.data_source.get_typed_value(shutter_offset);
            let orig_vec: Vec<i32> = original.to_vec();
            let remapped = remap_point_set_indices_to_expanded(&orig_vec, &cvc_vec, num_extra_ends);
            return Array::from(remapped);
        }
        self.data_source.get_typed_value(shutter_offset)
    }
}

/// Pinned curve expanding scene index.
pub struct HdsiPinnedCurveExpandingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiPinnedCurveExpandingSceneIndex {
    /// Creates a new pinned curve expanding scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiPinnedCurveExpandingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if prim.data_source.is_none() {
            return prim;
        }

        let data_source = prim.data_source.as_ref().unwrap().clone();

        if prim.prim_type == *tokens::RPRIM_BASIS_CURVES {
            prim.data_source = Some(PrimDataSource::new(data_source));
        } else if prim.prim_type == *tokens::RPRIM_GEOM_SUBSET {
            let parent_path = prim_path.get_parent_path();
            let parent_prim = if let Some(input) = self.base.get_input_scene() {
                si_ref(&input).get_prim(&parent_path)
            } else {
                HdSceneIndexPrim::default()
            };

            if parent_prim.prim_type == *tokens::RPRIM_BASIS_CURVES
                && parent_prim.data_source.is_some()
            {
                let parent_ds = parent_prim.data_source.unwrap();
                let indices_ds = data_source.get(&(*GEOM_SUBSET_INDICES).clone());
                let type_ds = data_source.get(&(*HdGeomSubsetSchema::get_type_token()).clone());

                if let (Some(idx), Some(ty)) = (indices_ds, type_ds) {
                    if let (Some(idxs), Some(tys)) = (
                        idx.as_any().downcast_ref::<Arc<dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync>>(),
                        ty.as_any().downcast_ref::<Arc<dyn HdTypedSampledDataSource<TfToken> + Send + Sync>>(),
                    ) {
                        let subset_indices = SubsetIndicesDataSource {
                            data_source: Arc::clone(idxs),
                            type_source: Arc::clone(tys),
                            parent_source: parent_ds.clone(),
                        };
                        let overlay = HdRetainedContainerDataSource::from_arrays(
                            &[(*GEOM_SUBSET_INDICES).clone()],
                            &[Arc::new(subset_indices) as HdDataSourceBaseHandle],
                        );
                        prim.data_source = Some(
                            HdOverlayContainerDataSource::new_2(overlay, data_source),
                        );
                    }
                }
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiPinnedCurveExpandingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiPinnedCurveExpandingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
