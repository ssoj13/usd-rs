//! Prefixing scene index - adds a path prefix to all scene data.
//!
//! Port of pxr/imaging/hd/prefixingSceneIndex.{h,cpp}

use super::base::{HdSceneIndexHandle, si_ref};
use super::filtering::{FilteringObserverTarget, HdSingleInputFilteringSceneIndexBase};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry,
};
use crate::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdSampledDataSource,
    HdVectorDataSource, HdVectorDataSourceHandle, cast_to_container, cast_to_vector,
};
use crate::schema::{HdSystemSchema, SYSTEM};
use crate::{HdSceneIndexBase, HdSceneIndexPrim};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

fn do_add_prefix(prefix: &SdfPath, path: &SdfPath) -> SdfPath {
    path.replace_prefix(&SdfPath::absolute_root(), prefix)
        .unwrap_or_else(|| path.clone())
}

// ---------------------------------------------------------------------------
// PrefixingObserver — forwards input-scene notifications to the owning index
// ---------------------------------------------------------------------------

/// Observer registered on the input scene. Holds a weak back-reference to the
/// owning `HdPrefixingSceneIndex` so that it does not prevent the index from
/// being dropped. Mirrors the `MergingObserver` pattern in merging.rs.
struct PrefixingObserver {
    owner: Weak<RwLock<HdPrefixingSceneIndex>>,
}

impl HdSceneIndexObserver for PrefixingObserver {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            {
                let owner = super::base::rwlock_data_ref(arc.as_ref());
                owner.on_prims_added(sender, entries);
            }
        }
    }

    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            {
                let owner = super::base::rwlock_data_ref(arc.as_ref());
                owner.on_prims_removed(sender, entries);
            }
        }
    }

    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            {
                let owner = super::base::rwlock_data_ref(arc.as_ref());
                owner.on_prims_dirtied(sender, entries);
            }
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            {
                let owner = super::base::rwlock_data_ref(arc.as_ref());
                owner.on_prims_renamed(sender, entries);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HdPrefixingSceneIndex
// ---------------------------------------------------------------------------

/// Path prefixing scene index.
///
/// The input scene contains data sources whose paths are all prefixed with
/// a given prefix. This scene index strips the prefix for internal queries
/// and adds it back when returning results.
pub struct HdPrefixingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    prefix: SdfPath,
    add_prefix_map: RwLock<HashMap<SdfPath, SdfPath>>,
    remove_prefix_map: RwLock<HashMap<SdfPath, SdfPath>>,
    /// Weak self-reference used to create the `PrefixingObserver`.
    self_ref: Option<Weak<RwLock<Self>>>,
    /// Strong handle to the observer registered on the input scene.
    /// Keeps the Weak stored by the input's observer list alive for as long
    /// as this index lives — mirroring `InputEntry::observer_handle` in
    /// merging.rs and C++'s `AddObserver(TfCreateRefPtr(this))`.
    input_observer: Option<HdSceneIndexObserverHandle>,
}

impl HdPrefixingSceneIndex {
    /// Creates a new prefixing scene index.
    ///
    /// Mirrors C++ `HdPrefixingSceneIndex` constructor: after construction the
    /// index registers a `PrefixingObserver` on its input scene so that
    /// mutations to the retained input are forwarded through this index
    /// (with the prefix applied).
    pub fn new(input_scene: Option<HdSceneIndexHandle>, prefix: SdfPath) -> Arc<RwLock<Self>> {
        let arc = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            prefix,
            add_prefix_map: RwLock::new(HashMap::new()),
            remove_prefix_map: RwLock::new(HashMap::new()),
            self_ref: None,
            input_observer: None,
        }));

        // Store weak self-reference and register as observer on the input,
        // matching C++: _GetInputSceneIndex()->AddObserver(TfCreateRefPtr(this)).
        {
            let mut s = arc.write();
            s.self_ref = Some(Arc::downgrade(&arc));
            s.register_on_input();
        }

        arc
    }

    /// Register a `PrefixingObserver` on the current input scene and store
    /// the strong observer handle so the weak held by the input stays alive.
    fn register_on_input(&mut self) {
        let weak = match &self.self_ref {
            Some(w) => w.clone(),
            None => return,
        };
        let observer: HdSceneIndexObserverHandle = Arc::new(PrefixingObserver { owner: weak });
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.write();
                input_lock.add_observer(observer.clone());
            }
        }
        self.input_observer = Some(observer);
    }

    /// Add prefix to the given path.
    pub fn add_prefix(&self, prim_path: &SdfPath) -> SdfPath {
        {
            let map = self.add_prefix_map.read();
            if let Some(result) = map.get(prim_path) {
                return result.clone();
            }
        }
        do_add_prefix(&self.prefix, prim_path)
    }

    /// Remove prefix from the given path.
    pub fn remove_prefix(&self, prim_path: &SdfPath) -> SdfPath {
        {
            let map = self.remove_prefix_map.read();
            if let Some(result) = map.get(prim_path) {
                return result.clone();
            }
        }
        prim_path
            .replace_prefix(&self.prefix, &SdfPath::absolute_root())
            .unwrap_or_else(|| prim_path.clone())
    }

    fn populate_prefix_maps(&self, path: &SdfPath) {
        let prefixed = do_add_prefix(&self.prefix, path);
        {
            let mut add_map = self.add_prefix_map.write();
            add_map.insert(path.clone(), prefixed.clone());
        }
        {
            let mut remove_map = self.remove_prefix_map.write();
            remove_map.insert(prefixed, path.clone());
        }
    }

    fn forward_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        for entry in entries {
            self.populate_prefix_maps(&entry.prim_path);
        }
        let prefixed: Vec<AddedPrimEntry> = entries
            .iter()
            .map(|e| AddedPrimEntry {
                prim_path: self.add_prefix(&e.prim_path),
                prim_type: e.prim_type.clone(),
                data_source: e.data_source.clone(),
            })
            .collect();
        self.base.forward_prims_added(self, &prefixed);
    }

    fn forward_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let prefixed: Vec<RemovedPrimEntry> = entries
            .iter()
            .map(|e| RemovedPrimEntry {
                prim_path: self.add_prefix(&e.prim_path),
            })
            .collect();
        self.base.forward_prims_removed(self, &prefixed);
    }

    fn forward_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let prefixed: Vec<DirtiedPrimEntry> = entries
            .iter()
            .map(|e| DirtiedPrimEntry {
                prim_path: self.add_prefix(&e.prim_path),
                dirty_locators: e.dirty_locators.clone(),
            })
            .collect();
        self.base.forward_prims_dirtied(self, &prefixed);
    }

    fn forward_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let prefixed: Vec<RenamedPrimEntry> = entries
            .iter()
            .map(|e| RenamedPrimEntry {
                old_prim_path: self.add_prefix(&e.old_prim_path),
                new_prim_path: self.add_prefix(&e.new_prim_path),
            })
            .collect();
        self.base.forward_prims_renamed(self, &prefixed);
    }
}

fn create_prefixing_datasource(
    prefix: &SdfPath,
    input: HdDataSourceBaseHandle,
) -> HdDataSourceBaseHandle {
    if let Some(container) = cast_to_container(&input) {
        let wrapped = PrefixingContainerDataSource {
            prefix: prefix.clone(),
            input: Some(container),
        };
        return Arc::new(wrapped) as HdDataSourceBaseHandle;
    }
    if let Some(vector) = cast_to_vector(&input) {
        let wrapped = PrefixingVectorDataSource {
            prefix: prefix.clone(),
            input: vector,
        };
        return Arc::new(wrapped) as HdDataSourceBaseHandle;
    }
    if let Some(sampled) = input.as_sampled() {
        let val = sampled.get_value(0.0);
        if val.get::<SdfPath>().is_some() {
            // G20: Wrap with sample-time-preserving data source
            return Arc::new(PrefixingPathDataSource {
                prefix: prefix.clone(),
                input: input.clone(),
            }) as HdDataSourceBaseHandle;
        }
        if val.get::<Vec<SdfPath>>().is_some() {
            // G20: Wrap with sample-time-preserving data source
            return Arc::new(PrefixingPathArrayDataSource {
                prefix: prefix.clone(),
                input: input.clone(),
            }) as HdDataSourceBaseHandle;
        }
    }
    input
}

// ---------------------------------------------------------------------------
// G20: Sample-time-preserving path data sources
// ---------------------------------------------------------------------------

/// Path data source that adds prefix while preserving sample times.
///
/// Port of C++ `Hd_PrefixingSceneIndexPathDataSource`.
#[derive(Debug)]
struct PrefixingPathDataSource {
    prefix: SdfPath,
    input: HdDataSourceBaseHandle,
}

impl HdDataSourceBase for PrefixingPathDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrefixingPathDataSource {
            prefix: self.prefix.clone(),
            input: self.input.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PrefixingPathDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        if let Some(sampled) = self.input.as_sampled() {
            let val = sampled.get_value(shutter_offset);
            if let Some(path) = val.get::<SdfPath>() {
                return usd_vt::Value::from(do_add_prefix(&self.prefix, path));
            }
        }
        usd_vt::Value::empty()
    }

    /// G20: Forward sample times from the wrapped input data source.
    fn get_contributing_sample_times(
        &self,
        start_time: f32,
        end_time: f32,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        if let Some(sampled) = self.input.as_sampled() {
            return sampled.get_contributing_sample_times(start_time, end_time, out_sample_times);
        }
        false
    }
}

/// Path array data source that adds prefix while preserving sample times.
///
/// Port of C++ `Hd_PrefixingSceneIndexPathArrayDataSource`.
#[derive(Debug)]
struct PrefixingPathArrayDataSource {
    prefix: SdfPath,
    input: HdDataSourceBaseHandle,
}

impl HdDataSourceBase for PrefixingPathArrayDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrefixingPathArrayDataSource {
            prefix: self.prefix.clone(),
            input: self.input.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PrefixingPathArrayDataSource {
    fn get_value(&self, shutter_offset: f32) -> usd_vt::Value {
        if let Some(sampled) = self.input.as_sampled() {
            let val = sampled.get_value(shutter_offset);
            if let Some(paths) = val.get::<Vec<SdfPath>>() {
                let prefixed: Vec<SdfPath> = paths
                    .iter()
                    .map(|p| do_add_prefix(&self.prefix, p))
                    .collect();
                return usd_vt::Value::new(prefixed);
            }
        }
        usd_vt::Value::empty()
    }

    /// G20: Forward sample times from the wrapped input data source.
    fn get_contributing_sample_times(
        &self,
        start_time: f32,
        end_time: f32,
        out_sample_times: &mut Vec<f32>,
    ) -> bool {
        if let Some(sampled) = self.input.as_sampled() {
            return sampled.get_contributing_sample_times(start_time, end_time, out_sample_times);
        }
        false
    }
}

/// Container data source that recursively prefixes path values in children.
#[derive(Debug)]
struct PrefixingContainerDataSource {
    prefix: SdfPath,
    input: Option<HdContainerDataSourceHandle>,
}

impl HdContainerDataSource for PrefixingContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.input
            .as_ref()
            .map(|c| c.get_names())
            .unwrap_or_default()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let input = self.input.as_ref()?;
        let result = input.get(name)?;
        Some(create_prefixing_datasource(&self.prefix, result))
    }
}

impl HdDataSourceBase for PrefixingContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrefixingContainerDataSource {
            prefix: self.prefix.clone(),
            input: self.input.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(PrefixingContainerDataSource {
            prefix: self.prefix.clone(),
            input: self.input.clone(),
        }))
    }
}

/// Absolute root prim container - excludes "system" since that's underlayed on children.
#[derive(Debug)]
struct PrefixingAbsoluteRootContainerDataSource {
    inner: PrefixingContainerDataSource,
}

impl HdContainerDataSource for PrefixingAbsoluteRootContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.inner.get_names();
        names.retain(|n| n != &*SYSTEM);
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*SYSTEM {
            return None;
        }
        self.inner.get(name)
    }
}

impl HdDataSourceBase for PrefixingAbsoluteRootContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrefixingAbsoluteRootContainerDataSource {
            inner: PrefixingContainerDataSource {
                prefix: self.inner.prefix.clone(),
                input: self.inner.input.clone(),
            },
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(PrefixingAbsoluteRootContainerDataSource {
            inner: PrefixingContainerDataSource {
                prefix: self.inner.prefix.clone(),
                input: self.inner.input.clone(),
            },
        }))
    }
}

/// Vector data source that recursively wraps elements.
#[derive(Debug)]
struct PrefixingVectorDataSource {
    prefix: SdfPath,
    input: HdVectorDataSourceHandle,
}

impl HdVectorDataSource for PrefixingVectorDataSource {
    fn get_num_elements(&self) -> usize {
        self.input.get_num_elements()
    }

    fn get_element(&self, element: usize) -> Option<HdDataSourceBaseHandle> {
        let child = self.input.get_element(element)?;
        Some(create_prefixing_datasource(&self.prefix, child))
    }
}

impl HdDataSourceBase for PrefixingVectorDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrefixingVectorDataSource {
            prefix: self.prefix.clone(),
            input: Arc::clone(&self.input),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_vector(&self) -> Option<HdVectorDataSourceHandle> {
        Some(Arc::new(PrefixingVectorDataSource {
            prefix: self.prefix.clone(),
            input: Arc::clone(&self.input),
        }) as HdVectorDataSourceHandle)
    }
}

impl HdSceneIndexBase for HdPrefixingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if !prim_path.has_prefix(&self.prefix) {
            if self.prefix.has_prefix(prim_path) {
                return HdSceneIndexPrim::new(
                    Token::default(),
                    Some(HdRetainedContainerDataSource::new_empty()),
                );
            }
            return HdSceneIndexPrim::default();
        }

        let input_path = self.remove_prefix(prim_path);
        let input = self.base.get_input_scene();
        let mut prim = if let Some(ref input_handle) = input {
            si_ref(&input_handle).get_prim(&input_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if let Some(ref prim_container) = prim.data_source {
            let prim_container = prim_container.clone();
            let wrapped = if input_path.is_absolute_root_path() {
                Arc::new(PrefixingAbsoluteRootContainerDataSource {
                    inner: PrefixingContainerDataSource {
                        prefix: self.prefix.clone(),
                        input: Some(prim_container),
                    },
                }) as HdContainerDataSourceHandle
            } else {
                Arc::new(PrefixingContainerDataSource {
                    prefix: self.prefix.clone(),
                    input: Some(prim_container),
                }) as HdContainerDataSourceHandle
            };

            if input_path.is_root_prim_path() {
                if let Some(input_handle) = input {
                    let (system_ds, _) =
                        HdSystemSchema::compose_as_prim_ds(input_handle, &input_path);
                    if let Some(system_container) = system_ds {
                        prim.data_source = Some(HdOverlayContainerDataSource::new_2(
                            system_container,
                            wrapped,
                        )
                            as HdContainerDataSourceHandle);
                    } else {
                        prim.data_source = Some(wrapped);
                    }
                } else {
                    prim.data_source = Some(wrapped);
                }
            } else {
                prim.data_source = Some(wrapped);
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> Vec<SdfPath> {
        if prim_path.has_prefix(&self.prefix) {
            let input_path = self.remove_prefix(prim_path);
            let input = self.base.get_input_scene();
            let mut result = if let Some(ref input_handle) = input {
                si_ref(&input_handle).get_child_prim_paths(&input_path)
            } else {
                Vec::new()
            };
            for p in &mut result {
                *p = self.add_prefix(p);
            }
            return result;
        }
        if self.prefix.has_prefix(prim_path) {
            let prefixes = self.prefix.get_prefixes();
            let idx = prim_path.get_path_element_count() + 1;
            if idx < prefixes.len() {
                return vec![prefixes[idx].clone()];
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdPrefixingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdPrefixingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.forward_prims_renamed(self, entries);
    }
}
