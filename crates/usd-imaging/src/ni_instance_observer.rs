//! Instance observer for native instance aggregation.
//!
//! Port of _InstanceObserver from niInstanceAggregationSceneIndex.cpp.
//! Observes input scene index, aggregates native instances into instancers,
//! and populates a retained scene index as output.

use crate::ni_instance_aggregation_data_sources::InstancerPrimSource;
use crate::ni_instance_aggregation_impl::{self, InstanceInfo};
use crate::ni_prototype_scene_index::UsdImagingNiPrototypeSceneIndex;
use crate::rerooting_container_data_source::UsdImagingRerootingContainerDataSource;
use std::collections::{BTreeMap, HashMap, HashSet};
use parking_lot::RwLock;
use std::sync::{Arc, Mutex, Weak};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdLazyContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
    convert_prims_renamed_to_removed_and_added,
};
use usd_hd::scene_index::retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};
use usd_hd::scene_index::{
    HdContainerDataSourceHandle as PrimDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexObserver, HdSceneIndexPrim,
};
use usd_hd::schema::{
    HdInstanceSchema, HdInstancerTopologySchema, HdPrimvarsSchema, HdVisibilitySchema,
    HdXformSchema,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_trace::trace_function;

/// Level of removal when pruning the info-to-instance map.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum RemovalLevel {
    None = 0,
    Instance = 1,
    Instancer = 2,
    BindingScope = 3,
    EnclosingPrototypeRoot = 4,
}

/// Observer that aggregates native instances into instancers.
///
/// Port of _InstanceObserver.
pub struct InstanceObserver {
    input_scene: HdSceneIndexHandle,
    retained_scene_index: Arc<RwLock<HdRetainedSceneIndex>>,
    for_native_prototype: bool,
    instance_data_source_names: Vec<TfToken>,
    resync_locators: HdDataSourceLocatorSet,
    state: Mutex<InstanceObserverState>,
    /// Weak self-ref for lazy data sources (breaks cycle)
    self_weak: Option<Weak<RwLock<InstanceObserver>>>,
}

struct InstanceObserverState {
    /// enclosing_prototype_root -> binding_hash -> prototype_name -> instances (RwLock for mutation)
    info_to_instance:
        BTreeMap<SdfPath, BTreeMap<TfToken, BTreeMap<TfToken, Arc<RwLock<HashSet<SdfPath>>>>>>,
    instance_to_info: BTreeMap<SdfPath, InstanceInfo>,
    /// instancer_path -> instance_path -> index (lazily computed)
    instancer_to_instance_to_index:
        HashMap<SdfPath, Arc<RwLock<Option<Arc<HashMap<SdfPath, i32>>>>>>,
}

impl InstanceObserverState {
    fn new() -> Self {
        Self {
            info_to_instance: BTreeMap::new(),
            instance_to_info: BTreeMap::new(),
            instancer_to_instance_to_index: HashMap::new(),
        }
    }
}

impl InstanceObserver {
    /// Creates a new instance observer. Call `populate()` after wrapping in Arc.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        for_native_prototype: bool,
        instance_data_source_names: Vec<TfToken>,
    ) -> Self {
        let retained_scene_index = HdRetainedSceneIndex::new();
        let resync_locators =
            ni_instance_aggregation_impl::compute_resync_locators(&instance_data_source_names);

        Self {
            input_scene,
            retained_scene_index,
            for_native_prototype,
            instance_data_source_names: instance_data_source_names.clone(),
            resync_locators,
            state: Mutex::new(InstanceObserverState::new()),
            self_weak: None,
        }
    }

    /// Set weak self-reference for lazy data source callbacks. Call before populate.
    pub fn set_self_weak(&mut self, weak: Weak<RwLock<InstanceObserver>>) {
        self.self_weak = Some(weak);
    }

    /// Get the retained scene index (output).
    pub fn get_retained_scene_index(&self) -> Arc<RwLock<HdRetainedSceneIndex>> {
        Arc::clone(&self.retained_scene_index)
    }

    /// Get the input scene.
    pub fn get_input_scene(&self) -> &HdSceneIndexHandle {
        &self.input_scene
    }

    /// Populate from input scene. Call after set_self_weak.
    pub fn populate(&self) {
        let mut ops = RetainedSceneIndexOperations::new(Arc::clone(&self.retained_scene_index));

        for prim_path in Self::collect_prim_paths(&self.input_scene, &SdfPath::absolute_root()) {
            self.add_prim(&prim_path, &mut ops);
        }
    }

    /// Collect prim paths in depth-first order, including root.
    /// Port of HdSceneIndexPrimView: "all descendants of a given prim (including the prim itself) in depth-first order".
    fn collect_prim_paths(scene: &HdSceneIndexHandle, root: &SdfPath) -> Vec<SdfPath> {
        let mut result = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(path) = stack.pop() {
            result.push(path.clone());
            let guard = scene.read();
            let children = guard.get_child_prim_paths(&path);
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
        result
    }

    fn get_info(&self, prim_path: &SdfPath) -> InstanceInfo {
        let guard = self.input_scene.read();
        let scene = &*guard;
        self.get_info_from_prim(scene.get_prim(prim_path).data_source.as_ref(), prim_path)
    }

    fn get_info_from_prim(
        &self,
        prim_source: Option<&PrimDataSourceHandle>,
        prim_path: &SdfPath,
    ) -> InstanceInfo {
        let prim_source = match prim_source {
            Some(ds) => ds,
            None => {
                return InstanceInfo {
                    enclosing_prototype_root: SdfPath::default(),
                    binding_hash: TfToken::default(),
                    prototype_name: TfToken::default(),
                };
            }
        };

        let prototype_name = ni_instance_aggregation_impl::get_usd_prototype_name(prim_source);
        if prototype_name.is_empty() {
            return InstanceInfo {
                enclosing_prototype_root: SdfPath::default(),
                binding_hash: TfToken::default(),
                prototype_name: TfToken::default(),
            };
        }

        let enclosing_prototype_root =
            ni_instance_aggregation_impl::get_prototype_root(prim_source);
        let enclosing_prototype_root = if enclosing_prototype_root.is_empty() {
            if self.for_native_prototype {
                UsdImagingNiPrototypeSceneIndex::get_prototype_path()
            } else {
                SdfPath::absolute_root()
            }
        } else {
            enclosing_prototype_root
        };

        let prototype_path = SdfPath::absolute_root()
            .append_child(prototype_name.as_str())
            .unwrap_or_else(SdfPath::absolute_root);
        let rerooted = UsdImagingRerootingContainerDataSource::new(
            prim_source.clone(),
            prim_path.clone(),
            prototype_path,
        );
        let rerooted_handle: HdContainerDataSourceHandle = rerooted;
        let binding_hash = ni_instance_aggregation_impl::compute_binding_hash(
            &rerooted_handle,
            &self.instance_data_source_names,
        );

        InstanceInfo {
            enclosing_prototype_root,
            binding_hash,
            prototype_name,
        }
    }

    fn add_prim(&self, prim_path: &SdfPath, ops: &mut RetainedSceneIndexOperations) {
        let info = self.get_info(prim_path);
        if info.is_instance() {
            self.add_instance(prim_path, &info, ops);
        }
    }

    fn add_instance(
        &self,
        prim_path: &SdfPath,
        info: &InstanceInfo,
        ops: &mut RetainedSceneIndexOperations,
    ) {
        let instancer_path = info.get_instancer_path();
        let (needs_binding_copy, instances, ptr) = {
            let mut state = self.state.lock().expect("Lock poisoned");
            state
                .instance_to_info
                .insert(prim_path.clone(), info.clone());
            let (needs_binding_copy, instances) = {
                let binding_scope = state
                    .info_to_instance
                    .entry(info.enclosing_prototype_root.clone())
                    .or_default();
                let by_prototype = binding_scope.entry(info.binding_hash.clone()).or_default();
                let needs_binding_copy = by_prototype.is_empty();
                let instances = by_prototype
                    .entry(info.prototype_name.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(HashSet::new())))
                    .clone();
                (needs_binding_copy, instances)
            };
            let ptr = state
                .instancer_to_instance_to_index
                .entry(instancer_path.clone())
                .or_insert_with(|| Arc::new(RwLock::new(None)))
                .clone();
            (needs_binding_copy, instances, ptr)
        };

        if needs_binding_copy {
            let guard = self.input_scene.read();
            let scene = &*guard;
            let prim_ds = scene.get_prim(prim_path).data_source.clone();
            let prim_ds = prim_ds.unwrap_or_else(|| {
                HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle
            });
            let rerooted = UsdImagingRerootingContainerDataSource::new(
                prim_ds,
                prim_path.clone(),
                info.get_prototype_path(),
            );
            let rerooted_handle: HdContainerDataSourceHandle = rerooted;
            let binding_copy = ni_instance_aggregation_impl::make_binding_copy(
                &rerooted_handle,
                &self.instance_data_source_names,
            );
            ops.add_prim(
                info.get_binding_prim_path(),
                HdSceneIndexPrim::new(TfToken::default(), Some(binding_copy)),
            );
        }

        let locators = HdDataSourceLocatorSet::from_iter([
            HdInstancerTopologySchema::get_default_locator()
                .append(&TfToken::new("instanceIndices")),
            HdPrimvarsSchema::get_default_locator(),
        ]);

        let already_exists = !instances.read().is_empty();
        instances.write().insert(prim_path.clone());

        if already_exists {
            ops.dirty_prim(instancer_path.clone(), locators);
        } else {
            ops.add_prim(
                info.get_propagated_prototype_base(),
                HdSceneIndexPrim::new(
                    TfToken::default(),
                    Some(HdRetainedContainerDataSource::new_empty()),
                ),
            );
            let instancer_ds = InstancerPrimSource::new(
                Arc::clone(&self.input_scene),
                info.enclosing_prototype_root.clone(),
                info.get_prototype_path(),
                Arc::clone(&instances),
                self.for_native_prototype,
            );
            ops.add_prim(
                instancer_path.clone(),
                HdSceneIndexPrim::new(TfToken::new("instancer"), Some(instancer_ds)),
            );
        }

        let instance_ds = self.get_data_source_for_instance(prim_path);
        ops.add_prim(
            prim_path.clone(),
            HdSceneIndexPrim::new(TfToken::default(), Some(instance_ds)),
        );
        Self::dirty_instances_and_reset_pointer_static(&ptr, ops);
    }

    fn get_data_source_for_instance(&self, prim_path: &SdfPath) -> HdContainerDataSourceHandle {
        let self_weak = match &self.self_weak {
            Some(w) => w.clone(),
            None => {
                return HdRetainedContainerDataSource::new_empty();
            }
        };
        let prim_path = prim_path.clone();
        HdRetainedContainerDataSource::from_entries(&[(
            (**HdInstanceSchema::get_schema_token()).clone(),
            HdLazyContainerDataSource::new(move || {
                self_weak.upgrade().and_then(|arc| {
                    {
                        let obs = arc.read();
                        obs.get_instance_schema_data_source(&prim_path)
                    }
                })
            }) as usd_hd::data_source::HdDataSourceBaseHandle,
        )])
    }

    fn get_instance_schema_data_source(
        &self,
        prim_path: &SdfPath,
    ) -> Option<HdContainerDataSourceHandle> {
        let info = self
            .state
            .lock()
            .expect("Lock poisoned")
            .instance_to_info
            .get(prim_path)
            .cloned()?;
        let instancer_path = info.get_instancer_path();
        let instance_index = self.get_instance_index(&info, prim_path);
        Some(HdInstanceSchema::build_retained(
            Some(HdRetainedTypedSampledDataSource::new(instancer_path)),
            Some(HdRetainedTypedSampledDataSource::new(0i32)),
            Some(HdRetainedTypedSampledDataSource::new(instance_index)),
        ))
    }

    fn get_instance_index(&self, info: &InstanceInfo, instance_path: &SdfPath) -> i32 {
        trace_function!();
        let ptr = match self.get_instance_to_index(info) {
            Some(p) => p,
            None => return -1,
        };
        let guard = ptr.read();
        let map = match guard.as_ref() {
            Some(m) => m,
            None => return -1,
        };
        map.get(instance_path).copied().unwrap_or(-1)
    }

    fn get_instance_to_index(
        &self,
        info: &InstanceInfo,
    ) -> Option<Arc<RwLock<Option<Arc<HashMap<SdfPath, i32>>>>>> {
        trace_function!();
        self.state
            .lock()
            .expect("Lock poisoned")
            .instancer_to_instance_to_index
            .get(&info.get_instancer_path())
            .cloned()
    }

    /// Port of _ComputeInstanceToIndex.
    /// C++ uses SdfPathSet (sorted) for deterministic iteration order.
    /// We sort paths to match C++ behavior and ensure stable instance indices.
    #[allow(dead_code)]
    fn compute_instance_to_index(&self, info: &InstanceInfo) -> Arc<HashMap<SdfPath, i32>> {
        trace_function!();
        let mut result = HashMap::new();
        let instances = self
            .state
            .lock()
            .expect("Lock poisoned")
            .info_to_instance
            .get(&info.enclosing_prototype_root)
            .and_then(|m| m.get(&info.binding_hash))
            .and_then(|m| m.get(&info.prototype_name))
            .cloned();
        if let Some(instances) = instances {
            let guard = instances.read();
            let mut paths: Vec<_> = guard.iter().cloned().collect();
            paths.sort();
            for (i, path) in paths.into_iter().enumerate() {
                result.insert(path, i as i32);
            }
        }
        Arc::new(result)
    }

    fn dirty_instances_and_reset_pointer_static(
        ptr: &Arc<RwLock<Option<Arc<HashMap<SdfPath, i32>>>>>,
        ops: &mut RetainedSceneIndexOperations,
    ) {
        let old = ptr.read().clone();
        let old = match old {
            Some(o) => o,
            None => return,
        };
        *ptr.write() = None;
        let instance_locator =
            HdDataSourceLocatorSet::from_locator(HdInstanceSchema::get_default_locator());
        for path in old.keys() {
            ops.dirty_prim(path.clone(), instance_locator.clone());
        }
    }

    fn remove_prim(&self, prim_path: &SdfPath, ops: &mut RetainedSceneIndexOperations) {
        let info = self
            .state
            .lock()
            .expect("Lock poisoned")
            .instance_to_info
            .remove(prim_path);
        if let Some(info) = info {
            self.remove_instance_entry(prim_path, &info, ops);
        }
    }

    fn remove_instance_entry(
        &self,
        instance_path: &SdfPath,
        info: &InstanceInfo,
        ops: &mut RetainedSceneIndexOperations,
    ) {
        let level = self.remove_instance_from_info_to_instance(instance_path, info);

        if level > RemovalLevel::None {
            ops.remove_prim(instance_path.clone());
        }

        if level == RemovalLevel::Instance {
            let locators = HdDataSourceLocatorSet::from_iter([
                HdInstancerTopologySchema::get_default_locator()
                    .append(&TfToken::new("instanceIndices")),
                HdPrimvarsSchema::get_default_locator(),
            ]);
            ops.dirty_prim(info.get_instancer_path(), locators);
            if let Some(ptr) = self
                .state
                .lock()
                .expect("Lock poisoned")
                .instancer_to_instance_to_index
                .get(&info.get_instancer_path())
                .cloned()
            {
                Self::dirty_instances_and_reset_pointer_static(&ptr, ops);
            }
        }

        if level >= RemovalLevel::Instancer {
            ops.remove_prim(info.get_instancer_path());
            ops.remove_prim(info.get_propagated_prototype_base());
            self.state
                .lock()
                .expect("Lock poisoned")
                .instancer_to_instance_to_index
                .remove(&info.get_instancer_path());
        }

        if level >= RemovalLevel::BindingScope {
            ops.remove_prim(info.get_binding_prim_path());
        }
    }

    fn remove_instance_from_info_to_instance(
        &self,
        prim_path: &SdfPath,
        info: &InstanceInfo,
    ) -> RemovalLevel {
        let mut state = self.state.lock().expect("Lock poisoned");
        let it0 = match state
            .info_to_instance
            .get_mut(&info.enclosing_prototype_root)
        {
            Some(x) => x,
            None => return RemovalLevel::None,
        };
        let it1 = match it0.get_mut(&info.binding_hash) {
            Some(x) => x,
            None => return RemovalLevel::None,
        };
        let it2 = match it1.get_mut(&info.prototype_name) {
            Some(x) => x,
            None => return RemovalLevel::None,
        };
        let is_empty_after = {
            let mut guard = it2.write();
            guard.remove(prim_path);
            guard.is_empty()
        };
        if is_empty_after {
            it1.remove(&info.prototype_name);
            if !it1.is_empty() {
                return RemovalLevel::Instancer;
            }
            it0.remove(&info.binding_hash);
            if !it0.is_empty() {
                return RemovalLevel::BindingScope;
            }
            state.info_to_instance.remove(&info.enclosing_prototype_root);
            RemovalLevel::EnclosingPrototypeRoot
        } else {
            RemovalLevel::Instance
        }
    }

    fn resync_prim(&self, prim_path: &SdfPath, ops: &mut RetainedSceneIndexOperations) {
        self.remove_prim(prim_path, ops);
        self.add_prim(prim_path, ops);
    }

    fn dirty_instancer_for_instance(
        &self,
        instance: &SdfPath,
        locators: &HdDataSourceLocatorSet,
        ops: &mut RetainedSceneIndexOperations,
    ) {
        if let Some(info) = self
            .state
            .lock()
            .expect("Lock poisoned")
            .instance_to_info
            .get(instance)
            .cloned()
        {
            ops.dirty_prim(info.get_instancer_path(), locators.clone());
        }
    }
}

fn get_primvar_value_locators_and_needs_resync(
    locators: &HdDataSourceLocatorSet,
) -> (HdDataSourceLocatorSet, bool) {
    let mut primvar_value_locators = HdDataSourceLocatorSet::new();
    let mut needs_resync = false;
    let primvars_loc = HdPrimvarsSchema::get_default_locator();
    for loc in locators.iter() {
        if primvars_loc.has_prefix(loc) || loc.has_prefix(&primvars_loc) {
            if loc.len() >= 3 && loc.get_element(2).map(|e| e.as_str()) == Some("primvarValue") {
                primvar_value_locators.insert(loc.clone());
            } else {
                needs_resync = true;
                return (primvar_value_locators, needs_resync);
            }
        }
    }
    (primvar_value_locators, needs_resync)
}

impl HdSceneIndexObserver for InstanceObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut ops = RetainedSceneIndexOperations::new(Arc::clone(&self.retained_scene_index));
        for entry in entries {
            self.resync_prim(&entry.prim_path, &mut ops);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if self
            .state
            .lock()
            .expect("Lock poisoned")
            .instance_to_info
            .is_empty()
        {
            return;
        }
        let mut ops = RetainedSceneIndexOperations::new(Arc::clone(&self.retained_scene_index));

        let xform_locators =
            HdDataSourceLocatorSet::from_locator(HdXformSchema::get_default_locator());
        let instance_transform_locators = HdDataSourceLocatorSet::from_locator(
            HdPrimvarsSchema::get_default_locator()
                .append(&TfToken::new("hydra:instanceTransforms"))
                .append(&TfToken::new("primvarValue")),
        );
        let mask_locators = HdDataSourceLocatorSet::from_locator(
            HdInstancerTopologySchema::get_default_locator().append(&TfToken::new("mask")),
        );

        for entry in entries {
            let path = &entry.prim_path;
            let locators = &entry.dirty_locators;

            if locators.intersects(&self.resync_locators) {
                self.resync_prim(path, &mut ops);
                continue;
            }

            if locators.intersects(&xform_locators) {
                self.dirty_instancer_for_instance(path, &instance_transform_locators, &mut ops);
            }

            let (primvar_value_locators, needs_resync) =
                get_primvar_value_locators_and_needs_resync(locators);
            if needs_resync {
                self.resync_prim(path, &mut ops);
            } else if !primvar_value_locators.is_empty() {
                self.dirty_instancer_for_instance(path, &primvar_value_locators, &mut ops);
            }

            let vis_locators =
                HdDataSourceLocatorSet::from_locator(HdVisibilitySchema::get_default_locator());
            if locators.intersects(&vis_locators) {
                self.dirty_instancer_for_instance(path, &mask_locators, &mut ops);
            }
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut ops = RetainedSceneIndexOperations::new(Arc::clone(&self.retained_scene_index));
        if self
            .state
            .lock()
            .expect("Lock poisoned")
            .instance_to_info
            .is_empty()
        {
            return;
        }
        for entry in entries {
            let path = &entry.prim_path;
            let to_remove: Vec<_> = self
                .state
                .lock()
                .expect("Lock poisoned")
                .instance_to_info
                .range(path.clone()..)
                .take_while(|(p, _)| p.has_prefix(path))
                .map(|(p, _)| p.clone())
                .collect();
            for p in to_remove {
                self.remove_prim(&p, &mut ops);
            }
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) = convert_prims_renamed_to_removed_and_added(sender, entries);
        if !removed.is_empty() {
            self.prims_removed(sender, &removed);
        }
        if !added.is_empty() {
            let added_entries: Vec<AddedPrimEntry> = added
                .into_iter()
                .map(|e| AddedPrimEntry::new(e.prim_path, e.prim_type))
                .collect();
            self.prims_added(sender, &added_entries);
        }
    }
}

/// RAII batch operations for retained scene index.
struct RetainedSceneIndexOperations {
    retained: Arc<RwLock<HdRetainedSceneIndex>>,
    operations: HashMap<SdfPath, RetainedOperation>,
}

enum RetainedOperation {
    Dirty { locators: HdDataSourceLocatorSet },
    Add { prim: HdSceneIndexPrim },
    Remove,
}

impl RetainedSceneIndexOperations {
    fn new(retained: Arc<RwLock<HdRetainedSceneIndex>>) -> Self {
        Self {
            retained,
            operations: HashMap::new(),
        }
    }

    fn add_prim(&mut self, prim_path: SdfPath, prim: HdSceneIndexPrim) {
        self.operations
            .insert(prim_path, RetainedOperation::Add { prim });
    }

    fn remove_prim(&mut self, prim_path: SdfPath) {
        self.operations.insert(prim_path, RetainedOperation::Remove);
    }

    fn dirty_prim(&mut self, prim_path: SdfPath, new_locators: HdDataSourceLocatorSet) {
        let op = self
            .operations
            .entry(prim_path)
            .or_insert_with(|| RetainedOperation::Dirty {
                locators: HdDataSourceLocatorSet::new(),
            });
        if let RetainedOperation::Dirty { locators } = op {
            locators.insert_set(&new_locators);
        }
    }
}

impl Drop for RetainedSceneIndexOperations {
    fn drop(&mut self) {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut dirtied = Vec::new();

        for (path, op) in std::mem::take(&mut self.operations) {
            match op {
                RetainedOperation::Add { prim } => {
                    added.push(RetainedAddedPrimEntry::new(
                        path,
                        prim.prim_type,
                        prim.data_source,
                    ));
                }
                RetainedOperation::Remove => {
                    removed.push(RemovedPrimEntry::new(path));
                }
                RetainedOperation::Dirty { locators } => {
                    dirtied.push(DirtiedPrimEntry::new(path, locators));
                }
            }
        }

        let mut r = self.retained.write();
        if !added.is_empty() {
            r.add_prims(&added);
        }
        if !removed.is_empty() {
            r.remove_prims(&removed);
        }
        if !dirtied.is_empty() {
            r.dirty_prims(&dirtied);
        }
    }
}
