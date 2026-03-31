
//! Ext computation primvar pruning scene index.
//!
//! Port of pxr/imaging/hdsi/extComputationPrimvarPruningSceneIndex.
//!
//! Prunes computed primvars from extComputationPrimvars and presents them as
//! authored primvars in primvars. The computation is executed when pulling on
//! the primvar's value, allowing downstream scene indices to transform the
//! data like any authored primvar.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdSampledDataSource, HdSampledDataSourceTime, cast_to_container,
    hd_merge_contributing_sample_times,
};
use usd_hd::ext_computation_context_internal::HdExtComputationContextInternal;
use usd_hd::ext_computation_cpu_callback::HdExtComputationCpuCallbackValue;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    HdExtComputationPrimvarSchema, HdExtComputationPrimvarsSchema, HdPrimvarsSchema,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Value;

/// Empty container data source used as fallback when invalid input is provided.
#[derive(Debug, Clone)]
struct EmptyContainerDataSource;

impl usd_hd::data_source::HdDataSourceBase for EmptyContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(EmptyContainerDataSource)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for EmptyContainerDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        Vec::new()
    }
    fn get(&self, _name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        None
    }
}

/// Shared computation context that executes the computation network.
struct ExtComputationContext {
    si: HdSceneIndexHandle,
}

impl ExtComputationContext {
    fn new(si: HdSceneIndexHandle) -> Arc<Self> {
        Arc::new(Self { si })
    }

    fn get_computed_value(
        &self,
        _primvar_name: &TfToken,
        source_comp_id: &SdfPath,
        comp_output_name: &TfToken,
        shutter_offset: HdSampledDataSourceTime,
    ) -> Value {
        let result = self.execute_computation_network(source_comp_id, shutter_offset);
        for (name, value) in &result {
            if name == comp_output_name {
                return value.clone();
            }
        }
        Value::default()
    }

    fn get_contributing_sample_times_for_interval(
        &self,
        source_comp_id: &SdfPath,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let (comp_ds_map, _comp_dep_map) = self.gather_computation_sources(source_comp_id);
        let mut sources: Vec<usd_hd::data_source::HdSampledDataSourceHandle> = Vec::new();
        for (_comp_id, comp_ds) in &comp_ds_map {
            let ext_comp = usd_hd::schema::HdExtComputationSchema::new(comp_ds.clone());
            if let Some(input_values) = ext_comp.get_input_values() {
                for name in input_values.get_names() {
                    if let Some(ds) = input_values.get(&name) {
                        if ds.as_ref().as_sampled().is_some() {
                            sources.push(Arc::new(SampledDataSourceHolder(ds.clone()))
                                as usd_hd::data_source::HdSampledDataSourceHandle);
                        }
                    }
                }
            }
        }
        hd_merge_contributing_sample_times(&sources, start_time, end_time, out_sample_times)
    }

    fn gather_computation_sources(
        &self,
        source_comp_id: &SdfPath,
    ) -> (
        HashMap<SdfPath, HdContainerDataSourceHandle>,
        HashMap<SdfPath, Vec<SdfPath>>,
    ) {
        let mut comp_ds_map = HashMap::new();
        let mut comp_dep_map = HashMap::new();
        let mut comps_queue = VecDeque::new();
        comps_queue.push_back(source_comp_id.clone());

        while let Some(cur_comp_id) = comps_queue.pop_front() {
            if comp_dep_map.contains_key(&cur_comp_id) {
                continue;
            }

            let prim = si_ref(&self.si).get_prim(&cur_comp_id);

            let prim_ds = prim.data_source.as_ref().cloned().unwrap_or_else(|| {
                Arc::new(EmptyContainerDataSource) as HdContainerDataSourceHandle
            });
            let ext_comp = usd_hd::schema::HdExtComputationSchema::get_from_parent(&prim_ds);

            if !ext_comp.is_defined() {
                continue;
            }

            if let Some(container) = ext_comp.get_container() {
                comp_ds_map.insert(cur_comp_id.clone(), container.clone());
            }
            let input_comps_container = ext_comp.get_input_computations();

            let mut entry = Vec::new();
            if let Some(input_comps) = input_comps_container {
                for name in input_comps.get_names() {
                    let input_comp = HdExtComputationInputComputationSchema::get_from_parent(
                        &input_comps,
                        &name,
                    );
                    if let Some(path_ds) = input_comp.get_source_computation() {
                        let path: SdfPath = path_ds.get_typed_value(0.0);
                        entry.push(path.clone());
                        comps_queue.push_back(path);
                    }
                }
            }
            comp_dep_map.insert(cur_comp_id, entry);
        }

        (comp_ds_map, comp_dep_map)
    }

    fn execute_computation_network(
        &self,
        source_comp_id: &SdfPath,
        shutter_offset: HdSampledDataSourceTime,
    ) -> Vec<(TfToken, Value)> {
        let (comp_ds_map, mut comp_dep_map) = self.gather_computation_sources(source_comp_id);

        // Topological ordering (Kahn's algorithm)
        let mut ordered_comps = Vec::new();
        let mut comps_queue = VecDeque::new();

        for (comp, deps) in comp_dep_map.iter() {
            if deps.is_empty() {
                comps_queue.push_back(comp.clone());
            }
        }
        for comp in &comps_queue {
            comp_dep_map.remove(comp);
        }

        while let Some(ind_comp) = comps_queue.pop_front() {
            ordered_comps.push(ind_comp.clone());

            comp_dep_map.retain(|comp, deps| {
                deps.retain(|d| *d != ind_comp);
                if deps.is_empty() {
                    comps_queue.push_back(comp.clone());
                    false
                } else {
                    true
                }
            });
        }

        let mut result = Vec::new();
        let mut value_store: HashMap<TfToken, Value> = HashMap::new();

        for comp_id in &ordered_comps {
            let comp_ds = match comp_ds_map.get(comp_id) {
                Some(d) => d,
                None => continue,
            };

            let ext_comp = usd_hd::schema::HdExtComputationSchema::new(comp_ds.clone());

            if let Some(input_values) = ext_comp.get_input_values() {
                for name in input_values.get_names() {
                    if let Some(ds) = input_values.get(&name) {
                        if let Some(sampled) = ds.as_ref().as_sampled() {
                            value_store.insert(name.clone(), sampled.get_value(shutter_offset));
                        }
                    }
                }
            }

            let outputs = ext_comp.get_outputs();
            let output_names: Vec<TfToken> =
                outputs.as_ref().map(|o| o.get_names()).unwrap_or_default();
            if output_names.is_empty() {
                continue;
            }

            let cpu_callback_ds = match ext_comp.get_cpu_callback() {
                Some(d) => d,
                None => continue,
            };
            let callback_value = if let Some(sampled) = cpu_callback_ds.as_ref().as_sampled() {
                let val = sampled.get_value(0.0);
                val.get::<HdExtComputationCpuCallbackValue>().cloned()
            } else {
                None
            };
            let callback = match callback_value {
                Some(v) => v.get().clone(),
                None => continue,
            };

            let mut execution_ctx = HdExtComputationContextInternal::new();
            if let Some(input_values) = ext_comp.get_input_values() {
                for name in input_values.get_names() {
                    if let Some(v) = value_store.get(&name) {
                        execution_ctx.set_input_value(name.clone(), v.clone());
                    }
                }
            }
            if let Some(input_comps) = ext_comp.get_input_computations() {
                for name in input_comps.get_names() {
                    let input_comp = HdExtComputationInputComputationSchema::get_from_parent(
                        &input_comps,
                        &name,
                    );
                    if let (Some(_), Some(output_ds)) = (
                        input_comp.get_source_computation(),
                        input_comp.get_source_computation_output_name(),
                    ) {
                        let output_name: TfToken = output_ds.get_typed_value(0.0);
                        if let Some(v) = value_store.get(&output_name) {
                            execution_ctx.set_input_value(name.clone(), v.clone());
                        }
                    }
                }
            }

            callback.compute(&mut execution_ctx);

            if execution_ctx.has_computation_error() {
                continue;
            }

            let update_result = *comp_id == *source_comp_id;
            for name in &output_names {
                if let Some(value) = execution_ctx.get_output_value(name) {
                    if update_result {
                        result.push((name.clone(), value));
                    } else {
                        value_store.insert(name.clone(), value);
                    }
                }
            }
        }

        result
    }
}

use usd_hd::schema::HdExtComputationInputComputationSchema;

/// Wrapper that delegates to the sampled interface of a data source.
struct SampledDataSourceHolder(HdDataSourceBaseHandle);
impl std::fmt::Debug for SampledDataSourceHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SampledDataSourceHolder")
    }
}
impl HdDataSourceBase for SampledDataSourceHolder {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(SampledDataSourceHolder(self.0.clone()))
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
impl HdSampledDataSource for SampledDataSourceHolder {
    fn get_value(&self, t: HdSampledDataSourceTime) -> Value {
        self.0.as_ref().as_sampled().unwrap().get_value(t)
    }
    fn get_contributing_sample_times(
        &self,
        start: HdSampledDataSourceTime,
        end: HdSampledDataSourceTime,
        out: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.0
            .as_ref()
            .as_sampled()
            .unwrap()
            .get_contributing_sample_times(start, end, out)
    }
}

/// Sampled data source that returns computed primvar value from context.
struct SampledExtCompPrimvarDataSource {
    input: HdContainerDataSourceHandle,
    primvar_name: TfToken,
    ctx: Arc<ExtComputationContext>,
}
impl std::fmt::Debug for SampledExtCompPrimvarDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampledExtCompPrimvarDataSource")
            .field("primvar_name", &self.primvar_name)
            .finish()
    }
}
impl HdDataSourceBase for SampledExtCompPrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: self.input.clone(),
            primvar_name: self.primvar_name.clone(),
            ctx: self.ctx.clone(),
        })
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
impl HdSampledDataSource for SampledExtCompPrimvarDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let s = HdExtComputationPrimvarSchema::new(self.input.clone());
        if let (Some(h1), Some(h2)) = (
            s.get_source_computation(),
            s.get_source_computation_output_name(),
        ) {
            let source_comp: SdfPath = h1.get_typed_value(0.0);
            let output_name: TfToken = h2.get_typed_value(0.0);
            return self.ctx.get_computed_value(
                &self.primvar_name,
                &source_comp,
                &output_name,
                shutter_offset,
            );
        }
        Value::default()
    }
    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let s = HdExtComputationPrimvarSchema::new(self.input.clone());
        if let Some(h1) = s.get_source_computation() {
            let source_comp: SdfPath = h1.get_typed_value(0.0);
            return self.ctx.get_contributing_sample_times_for_interval(
                &source_comp,
                start_time,
                end_time,
                out_sample_times,
            );
        }
        out_sample_times.push(0.0);
        false
    }
}

/// Container data source for a computed primvar (primvarValue, interpolation, role).
#[derive(Clone)]
struct ExtCompPrimvarDataSource {
    input: HdContainerDataSourceHandle,
    primvar_name: TfToken,
    ctx: Arc<ExtComputationContext>,
}
impl std::fmt::Debug for ExtCompPrimvarDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtCompPrimvarDataSource")
            .field("primvar_name", &self.primvar_name)
            .finish()
    }
}
impl HdDataSourceBase for ExtCompPrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: self.input.clone(),
            primvar_name: self.primvar_name.clone(),
            ctx: self.ctx.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}
impl HdContainerDataSource for ExtCompPrimvarDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new("primvarValue"),
            TfToken::new("interpolation"),
            TfToken::new("role"),
        ]
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == "interpolation" || name == "role" {
            return self.input.get(name);
        }
        if name == "primvarValue" {
            return Some(Arc::new(SampledExtCompPrimvarDataSource {
                input: self.input.clone(),
                primvar_name: self.primvar_name.clone(),
                ctx: self.ctx.clone(),
            }) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Primvars data source that merges authored and computed primvars.
#[derive(Clone)]
struct PrimvarsDataSource {
    primvars_ds: HdContainerDataSourceHandle,
    ext_comp_primvars_ds: HdContainerDataSourceHandle,
    ctx: Arc<ExtComputationContext>,
}
impl std::fmt::Debug for PrimvarsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarsDataSource").finish()
    }
}
impl HdDataSourceBase for PrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            primvars_ds: self.primvars_ds.clone(),
            ext_comp_primvars_ds: self.ext_comp_primvars_ds.clone(),
            ctx: self.ctx.clone(),
        })
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
        let mut res: Vec<TfToken> = self.primvars_ds.get_names();
        res.extend(self.ext_comp_primvars_ds.get_names());
        res
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if let Some(authored) = self.primvars_ds.get(name) {
            return Some(authored);
        }
        if let Some(child) = self.ext_comp_primvars_ds.get(name) {
            if let Some(c) = cast_to_container(&child) {
                return Some(Arc::new(ExtCompPrimvarDataSource {
                    input: c,
                    primvar_name: name.clone(),
                    ctx: self.ctx.clone(),
                }) as HdDataSourceBaseHandle);
            }
        }
        None
    }
}

/// Prim-level data source override.
#[derive(Clone)]
struct PrimDataSource {
    input: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
}
impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource").finish()
    }
}
impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: self.input.clone(),
            si: self.si.clone(),
        })
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
        let mut names = self.input.get_names();
        let has_ext_comp = names.iter().any(|n| n == "extComputationPrimvars");
        let has_primvars = names.iter().any(|n| n == "primvars");
        if has_ext_comp && !has_primvars {
            names.push(TfToken::new("primvars"));
        }
        names
    }
    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input.get(name);

        if name == "primvars" {
            if let Some(ext_comp_child) = self.input.get(&TfToken::new("extComputationPrimvars")) {
                if let Some(c) = cast_to_container(&ext_comp_child) {
                    let es = HdExtComputationPrimvarsSchema::new(c.clone());
                    if !es.get_ext_computation_primvar_names().is_empty() {
                        let primvars_ds = result
                            .as_ref()
                            .and_then(|r| cast_to_container(r))
                            .unwrap_or_else(|| {
                                Arc::new(EmptyContainerDataSource) as HdContainerDataSourceHandle
                            });
                        let ctx = ExtComputationContext::new(self.si.clone());
                        return Some(Arc::new(PrimvarsDataSource {
                            primvars_ds,
                            ext_comp_primvars_ds: c,
                            ctx,
                        }) as HdDataSourceBaseHandle);
                    }
                }
            }
        }

        if name == "extComputationPrimvars" {
            return Some(Arc::new(EmptyContainerDataSource) as HdDataSourceBaseHandle);
        }

        result
    }
}

/// ExtComputation primvar pruning scene index.
pub struct HdsiExtComputationPrimvarPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiExtComputationPrimvarPruningSceneIndex {
    /// Creates a new ext computation primvar pruning scene index.
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

impl HdSceneIndexBase for HdsiExtComputationPrimvarPruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let mut prim = si_ref(&input).get_prim(prim_path);

        let is_rprim = prim.prim_type == "mesh"
            || prim.prim_type == "basisCurves"
            || prim.prim_type == "points";

        if is_rprim {
            if let Some(ref ds) = prim.data_source {
                prim.data_source = Some(Arc::new(PrimDataSource {
                    input: ds.clone(),
                    si: input.clone(),
                }) as HdContainerDataSourceHandle);
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
        "HdsiExtComputationPrimvarPruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiExtComputationPrimvarPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let ext_comp_locator = HdExtComputationPrimvarsSchema::get_default_locator();
        let primvars_locator = HdPrimvarsSchema::get_default_locator();
        let edited: Vec<DirtiedPrimEntry> = entries
            .iter()
            .map(|e| DirtiedPrimEntry {
                prim_path: e.prim_path.clone(),
                dirty_locators: e
                    .dirty_locators
                    .replace_prefix(&ext_comp_locator, &primvars_locator),
            })
            .collect();
        self.base.forward_prims_dirtied(self, &edited);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
