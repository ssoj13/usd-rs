// Native instancing propagation scene index. Some nested instancing paths
// (per-prototype scene index chains) are not yet fully exercised. The allow
// below suppresses dead_code warnings for paths not yet used in the render loop.
#![allow(dead_code)]

//! Native instance prototype propagating scene index.
//!
//! Port of pxr/usdImaging/usdImaging/niPrototypePropagatingSceneIndex.h/cpp
//!
//! Implements USD native instancing using HdMergingSceneIndex with:
//! - prototype scene index chain (pruning/rerooting, NiPrototype, Flattening, callback)
//! - instance aggregation scene index (produces instancers)
//! - dynamically added NiPrototypePropagatingSceneIndex per instancer (nested instancing)

use crate::flattened_data_source_providers::usd_imaging_flattened_data_source_providers;
use crate::ni_instance_aggregation_scene_index::UsdImagingNiInstanceAggregationSceneIndex;
use crate::ni_prototype_pruning_scene_index::UsdImagingNiPrototypePruningSceneIndex;
use crate::ni_prototype_scene_index::UsdImagingNiPrototypeSceneIndex;
use crate::rerooting_scene_index::HdRerootingSceneIndex;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use usd_hd::HdDataSourceBaseHandle;
use usd_hd::scene_index::flattening::HdFlatteningSceneIndex;
use usd_hd::scene_index::merging::HdMergingSceneIndex;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, SdfPathVector, scene_index_to_handle, wire_filter_to_input,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

struct MergingSceneIndexObserver {
    owner: Weak<RwLock<UsdImagingNiPrototypePropagatingSceneIndex>>,
}

impl MergingSceneIndexObserver {
    fn new(owner: Weak<RwLock<UsdImagingNiPrototypePropagatingSceneIndex>>) -> Self {
        Self { owner }
    }
}

impl HdSceneIndexObserver for MergingSceneIndexObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner = unsafe { &*owner.data_ptr() };
            // Match OpenUSD `_owner->_SendPrimsAdded(entries)`: downstream
            // observers are allowed to query `sender.get_prim(...)` during the
            // callback, so the sender must be the propagating scene index view.
            owner.base_impl.send_prims_added(owner, entries);
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner = unsafe { &*owner.data_ptr() };
            owner.base_impl.send_prims_removed(owner, entries);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner = unsafe { &*owner.data_ptr() };
            owner.base_impl.send_prims_dirtied(owner, entries);
        }
    }

    fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner = unsafe { &*owner.data_ptr() };
            owner.base_impl.send_prims_renamed(owner, entries);
        }
    }
}

/// Callback to wrap scene index with additional scene indices (e.g. DrawModeSceneIndex).
///
/// Port of SceneIndexAppendCallback. Applied at each prototype recursion level.
pub type SceneIndexAppendCallback =
    Option<Box<dyn Fn(HdSceneIndexHandle) -> HdSceneIndexHandle + Send + Sync>>;

/// Caches scene indices for each USD prototype.
///
/// Port of _SceneIndexCache from niPrototypePropagatingSceneIndex.cpp.
struct SceneIndexCache {
    input_scene_index: HdSceneIndexHandle,
    instance_data_source_names: Vec<TfToken>,
    scene_index_append_callback: SceneIndexAppendCallback,
    /// prototype_name -> (isolating_si, hash -> (prototype_si, instance_agg_si))
    prototype_to_indices: std::collections::HashMap<
        String,
        (
            Option<HdSceneIndexHandle>,
            std::collections::HashMap<u64, (HdSceneIndexHandle, HdSceneIndexHandle)>,
        ),
    >,
}

impl SceneIndexCache {
    fn new(
        input_scene_index: HdSceneIndexHandle,
        instance_data_source_names: Vec<TfToken>,
        scene_index_append_callback: SceneIndexAppendCallback,
    ) -> Self {
        Self {
            input_scene_index,
            instance_data_source_names,
            scene_index_append_callback,
            prototype_to_indices: std::collections::HashMap::new(),
        }
    }

    fn get_input_scene_index(&self) -> &HdSceneIndexHandle {
        &self.input_scene_index
    }

    fn get_scene_indices_for_prototype(
        &mut self,
        prototype_name: &str,
        overlay_hash: u64,
        overlay_ds: Option<HdContainerDataSourceHandle>,
    ) -> (HdSceneIndexHandle, HdSceneIndexHandle) {
        let prototype_key = prototype_name.to_string();
        self.prototype_to_indices
            .entry(prototype_key.clone())
            .or_insert_with(|| (None, std::collections::HashMap::new()));

        let isolating = if let Some(isolating) = self
            .prototype_to_indices
            .get(&prototype_key)
            .and_then(|entry| entry.0.clone())
        {
            isolating
        } else {
            let isolating = self.compute_isolating_scene_index(prototype_name);
            self.prototype_to_indices
                .get_mut(&prototype_key)
                .expect("prototype entry must exist")
                .0 = Some(isolating.clone());
            isolating
        };

        if let Some((prototype_si, instance_agg_si)) = self
            .prototype_to_indices
            .get(&prototype_key)
            .and_then(|entry| entry.1.get(&overlay_hash).cloned())
        {
            return (prototype_si, instance_agg_si);
        }

        let for_prototype = !prototype_name.is_empty();

        let prototype_si =
            self.compute_prototype_scene_index(&isolating, for_prototype, overlay_ds);
        let instance_agg_input =
            UsdImagingNiInstanceAggregationSceneIndex::prepare_aggregation_input(
                prototype_si.clone(),
                for_prototype,
            );
        let instance_agg_si =
            scene_index_to_handle(UsdImagingNiInstanceAggregationSceneIndex::new_with_params(
                instance_agg_input,
                for_prototype,
                self.instance_data_source_names.clone(),
            ));

        self.prototype_to_indices
            .get_mut(&prototype_key)
            .expect("prototype entry must exist")
            .1
            .insert(
                overlay_hash,
                (prototype_si.clone(), instance_agg_si.clone()),
            );

        (prototype_si, instance_agg_si)
    }

    fn compute_isolating_scene_index(&self, prototype_name: &str) -> HdSceneIndexHandle {
        if prototype_name.is_empty() {
            let pruning_input = self.input_scene_index.clone();
            let pruning = UsdImagingNiPrototypePruningSceneIndex::new(pruning_input.clone(), None);
            wire_filter_to_input(&pruning, &pruning_input);
            scene_index_to_handle(pruning)
        } else {
            let prototype_path = SdfPath::absolute_root()
                .append_child(prototype_name)
                .unwrap_or_else(|| SdfPath::absolute_root());
            let prototype_dst = UsdImagingNiPrototypeSceneIndex::get_prototype_path();
            scene_index_to_handle(HdRerootingSceneIndex::new_with_prefixes(
                Some(self.input_scene_index.clone()),
                prototype_path,
                prototype_dst,
            ))
        }
    }

    fn compute_prototype_scene_index(
        &self,
        isolating: &HdSceneIndexHandle,
        for_prototype: bool,
        overlay_ds: Option<HdContainerDataSourceHandle>,
    ) -> HdSceneIndexHandle {
        let prototype_input = isolating.clone();
        let prototype_scene_index = UsdImagingNiPrototypeSceneIndex::new(
            prototype_input.clone(),
            for_prototype,
            overlay_ds,
        );
        wire_filter_to_input(&prototype_scene_index, &prototype_input);
        let mut chain = scene_index_to_handle(prototype_scene_index);

        let flatten_input = chain.clone();
        let flattening = HdFlatteningSceneIndex::new(
            Some(chain),
            Some(usd_imaging_flattened_data_source_providers()),
        );
        wire_filter_to_input(&flattening, &flatten_input);
        chain = scene_index_to_handle(flattening);
        if let Some(ref cb) = self.scene_index_append_callback {
            chain = cb(chain);
        }
        chain
    }

    fn garbage_collect(&mut self, _prototype_name: &str, _overlay_hash: u64) {
        // Placeholder - full impl would remove unused cached entries
    }
}

/// RAII helper that batches merge operations (remove then insert on drop).
///
/// Port of _MergingSceneIndexOperations.
struct MergingSceneIndexOperations {
    merging: Arc<RwLock<HdMergingSceneIndex>>,
    to_remove: Vec<HdSceneIndexHandle>,
    to_insert: Vec<(HdSceneIndexHandle, SdfPath)>,
}

impl MergingSceneIndexOperations {
    fn new(merging: Arc<RwLock<HdMergingSceneIndex>>) -> Self {
        Self {
            merging,
            to_remove: Vec::new(),
            to_insert: Vec::new(),
        }
    }

    fn add_input_scene(&mut self, scene: HdSceneIndexHandle, active_root: SdfPath) {
        self.to_insert.push((scene, active_root));
    }

    fn remove_input_scene(&mut self, scene: HdSceneIndexHandle) {
        self.to_remove.push(scene);
    }
}

impl Drop for MergingSceneIndexOperations {
    fn drop(&mut self) {
        let merging = self.merging.write();
        for scene in &self.to_remove {
            merging.remove_input_scene(scene);
        }
        for (scene, root) in &self.to_insert {
            merging.add_input_scene(scene.clone(), root.clone());
        }
    }
}

/// Native instance prototype propagating scene index.
///
/// Uses HdMergingSceneIndex internally. Adds prototype scene index and instance
/// aggregation as inputs. Dynamically adds rerooted NiPrototypePropagatingSceneIndex
/// per instancer when instance aggregation produces instancer prims.
pub struct UsdImagingNiPrototypePropagatingSceneIndex {
    prototype_name: String,
    _overlay_hash: u64,
    cache: Arc<RwLock<SceneIndexCache>>,
    merging_scene_index: Arc<RwLock<HdMergingSceneIndex>>,
    instance_aggregation_scene_index: HdSceneIndexHandle,
    instancers_to_propagated: Mutex<BTreeMap<SdfPath, HdSceneIndexHandle>>,
    base_impl: usd_hd::scene_index::base::HdSceneIndexBaseImpl,
}

impl UsdImagingNiPrototypePropagatingSceneIndex {
    /// Creates the root-level prototype propagating scene index.
    ///
    /// Port of New(inputSceneIndex, instanceDataSourceNames, sceneIndexAppendCallback).
    pub fn new_with_instance_names(
        input_scene: HdSceneIndexHandle,
        instance_data_source_names: Vec<TfToken>,
        scene_index_append_callback: SceneIndexAppendCallback,
    ) -> Arc<RwLock<Self>> {
        Self::_new(
            "",
            None,
            Arc::new(RwLock::new(SceneIndexCache::new(
                input_scene,
                instance_data_source_names,
                scene_index_append_callback,
            ))),
        )
    }

    fn _new(
        prototype_name: &str,
        overlay_ds: Option<HdContainerDataSourceHandle>,
        cache: Arc<RwLock<SceneIndexCache>>,
    ) -> Arc<RwLock<Self>> {
        let overlay_hash = overlay_ds.as_ref().map_or(0, |ds| {
            usd_hd::data_source::hd_data_source_hash(
                &(ds.clone() as usd_hd::data_source::HdDataSourceBaseHandle),
                0.0,
                0.0,
            )
        });

        let (prototype_si, instance_agg_si) = {
            let mut c = cache.write();
            c.get_scene_indices_for_prototype(prototype_name, overlay_hash, overlay_ds.clone())
        };

        let merging = HdMergingSceneIndex::new();
        {
            let m = merging.write();
            m.add_input_scene(prototype_si.clone(), SdfPath::absolute_root());
            m.add_input_scene(instance_agg_si.clone(), SdfPath::absolute_root());
        }

        let self_arc = Arc::new(RwLock::new(Self {
            prototype_name: prototype_name.to_string(),
            _overlay_hash: overlay_hash,
            cache: cache.clone(),
            merging_scene_index: merging.clone(),
            instance_aggregation_scene_index: instance_agg_si.clone(),
            instancers_to_propagated: Mutex::new(BTreeMap::new()),
            base_impl: usd_hd::scene_index::base::HdSceneIndexBaseImpl::new(),
        }));

        let observer = usd_hd::scene_index::filtering::FilteringSceneIndexObserver::new(
            Arc::downgrade(&self_arc) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>,
        );
        let observer_handle = Arc::new(observer);
        instance_agg_si.read().add_observer(observer_handle);
        let merging_observer = Arc::new(MergingSceneIndexObserver::new(Arc::downgrade(&self_arc)));
        merging.read().add_observer(merging_observer);

        let mut ops = MergingSceneIndexOperations::new(merging);
        let self_lock = self_arc.write();
        self_lock.populate(&instance_agg_si, &mut ops);
        drop(self_lock);
        drop(ops);

        self_arc
    }

    fn get_binding_scope_data_source(
        instance_agg: &HdSceneIndexHandle,
        prim_path: &SdfPath,
    ) -> Option<HdContainerDataSourceHandle> {
        let binding_scope =
            UsdImagingNiInstanceAggregationSceneIndex::get_binding_scope_from_instancer_path(
                prim_path,
            );
        let agg = instance_agg.read();
        let prim = agg.get_prim(&binding_scope);
        prim.data_source
    }

    fn populate(&self, instance_agg: &HdSceneIndexHandle, ops: &mut MergingSceneIndexOperations) {
        let root = SdfPath::absolute_root();
        let prim_paths = Self::collect_prim_paths(instance_agg, &root);
        for path in prim_paths {
            self.add_prim(&path, ops);
        }
    }

    fn collect_prim_paths(scene: &HdSceneIndexHandle, root: &SdfPath) -> Vec<SdfPath> {
        let mut result = Vec::new();
        let s = scene.read();
        let mut stack = vec![root.clone()];
        while let Some(path) = stack.pop() {
            result.push(path.clone());
            let children = s.get_child_prim_paths(&path);
            for c in children.into_iter().rev() {
                stack.push(c);
            }
        }
        result
    }

    fn add_prim(&self, prim_path: &SdfPath, ops: &mut MergingSceneIndexOperations) {
        let prototype_name =
            UsdImagingNiInstanceAggregationSceneIndex::get_prototype_name_from_instancer_path(
                prim_path,
            );
        if prototype_name.as_str().is_empty() {
            return;
        }

        if let Some(prev) = self
            .instancers_to_propagated
            .lock()
            .expect("Lock poisoned")
            .remove(prim_path)
        {
            ops.remove_input_scene(prev);
        }

        let overlay_ds =
            Self::get_binding_scope_data_source(&self.instance_aggregation_scene_index, prim_path);
        let propagated = Self::_new(prototype_name.as_str(), overlay_ds, Arc::clone(&self.cache));

        let instancer_path = UsdImagingNiPrototypeSceneIndex::get_instancer_path();
        let rerooted = HdRerootingSceneIndex::new_with_prefixes(
            Some(scene_index_to_handle(propagated)),
            instancer_path,
            prim_path.clone(),
        );

        let rerooted_handle = scene_index_to_handle(rerooted);
        self.instancers_to_propagated
            .lock()
            .expect("Lock poisoned")
            .insert(prim_path.clone(), rerooted_handle.clone());
        ops.add_input_scene(rerooted_handle, prim_path.clone());
    }

    fn remove_prim(&self, prim_path: &SdfPath, ops: &mut MergingSceneIndexOperations) {
        let to_remove: Vec<_> = self
            .instancers_to_propagated
            .lock()
            .expect("Lock poisoned")
            .range(prim_path.clone()..)
            .take_while(|(k, _)| k.has_prefix(prim_path))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (_, scene) in to_remove {
            ops.remove_input_scene(scene);
        }
        self.instancers_to_propagated
            .lock()
            .expect("Lock poisoned")
            .retain(|k, _| !k.has_prefix(prim_path));
    }
}

impl HdSceneIndexBase for UsdImagingNiPrototypePropagatingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let inner = unsafe { &*self.merging_scene_index.data_ptr() };
        inner.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let inner = unsafe { &*self.merging_scene_index.data_ptr() };
        inner.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base_impl.add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base_impl.remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        if self.prototype_name.is_empty() {
            "UsdImagingNiPrototypePropagatingSceneIndex".to_string()
        } else {
            format!("Propagating native prototype {}", self.prototype_name)
        }
    }
}

impl FilteringObserverTarget for UsdImagingNiPrototypePropagatingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut ops = MergingSceneIndexOperations::new(self.merging_scene_index.clone());
        for e in entries {
            self.add_prim(&e.prim_path, &mut ops);
        }
        drop(ops);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut ops = MergingSceneIndexOperations::new(self.merging_scene_index.clone());
        for e in entries {
            self.remove_prim(&e.prim_path, &mut ops);
        }
        drop(ops);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base_impl.send_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base_impl.send_prims_renamed(self, entries);
    }
}
