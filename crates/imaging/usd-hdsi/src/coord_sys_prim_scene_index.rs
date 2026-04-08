//! Coordinate system prim scene index.
//!
//! If prim P has a coord sys binding FOO to another prim Q, the scene
//! index adds a coord sys prim Q/__coordSys_FOO under Q.
//! Rewrites coord sys binding on P to point to Q/__coordSys_FOO.
//! Port of pxr/imaging/hdsi/coordSysPrimSceneIndex.

use crate::utils::{collect_prim_paths, make_coord_sys_prim_path};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_hd::schema::{
    HdCoordSysBindingSchema, HdCoordSysSchema, HdCoordSysSchemaBuilder, HdDependenciesSchema,
    HdDependencySchemaBuilder, HdPathDataSourceHandle, HdXformSchema,
};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

const COORD_SYS_PRIM_PREFIX: &str = "__coordSys_";
const XFORM_DEPENDENCY: &str = "xformDependency";

fn ignore_binding(targeted_prim_path: &SdfPath) -> bool {
    targeted_prim_path.is_empty() || !targeted_prim_path.is_prim_path()
}

#[derive(Debug, Clone)]
struct Binding {
    name: TfToken,
    path: SdfPath,
}

/// Coord sys prim data source: coordSys (name), xform (from target), __dependencies.
#[derive(Clone)]
struct CoordSysPrimDataSource {
    input: HdSceneIndexHandle,
    prim_path: SdfPath,
    name: TfToken,
}

impl fmt::Debug for CoordSysPrimDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoordSysPrimDataSource")
            .field("prim_path", &self.prim_path)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl HdDataSourceBase for CoordSysPrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(CoordSysPrimDataSource {
            input: self.input.clone(),
            prim_path: self.prim_path.clone(),
            name: self.name.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for CoordSysPrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            (*HdCoordSysSchema::get_schema_token()).clone(),
            (*HdXformSchema::get_schema_token()).clone(),
            (*HdDependenciesSchema::get_schema_token()).clone(),
        ]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if *name == **HdCoordSysSchema::get_schema_token() {
            let coord_sys = HdCoordSysSchemaBuilder::default()
                .set_name(HdRetainedTypedSampledDataSource::new(self.name.clone()))
                .build();
            return Some(coord_sys.as_ref().clone_box());
        }
        if *name == **HdXformSchema::get_schema_token() {
            let guard = self.input.read();
            let prim = guard.get_prim(&self.prim_path);
            let prim_ds = prim.data_source.as_ref()?;
            return prim_ds.get(HdXformSchema::get_schema_token());
        }
        if *name == **HdDependenciesSchema::get_schema_token() {
            let xform_locator = HdXformSchema::get_default_locator();
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                    self.prim_path.clone(),
                ) as HdPathDataSourceHandle)
                .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    xform_locator.clone(),
                )
                    as usd_hd::schema::HdLocatorDataSourceHandle)
                .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    xform_locator,
                )
                    as usd_hd::schema::HdLocatorDataSourceHandle)
                .build();
            return Some(
                HdRetainedContainerDataSource::from_entries(&[(
                    TfToken::new(XFORM_DEPENDENCY),
                    dep as HdDataSourceBaseHandle,
                )])
                .as_ref()
                .clone_box(),
            );
        }
        None
    }
}

/// Rewrites paths in coordSysBinding to point to coord sys prims we add.
#[derive(Clone)]
struct CoordSysBindingDataSource {
    input: HdContainerDataSourceHandle,
}

impl fmt::Debug for CoordSysBindingDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoordSysBindingDataSource")
            .finish_non_exhaustive()
    }
}

impl HdDataSourceBase for CoordSysBindingDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(CoordSysBindingDataSource {
            input: self.input.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for CoordSysBindingDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let schema = HdCoordSysBindingSchema::new(self.input.clone());
        let path_ds = schema.get_coord_sys_binding(name)?;
        let targeted_prim_path = path_ds.get_typed_value(0.0f32);
        if ignore_binding(&targeted_prim_path) {
            return self.input.get(name);
        }
        Some(
            HdRetainedTypedSampledDataSource::<SdfPath>::new(make_coord_sys_prim_path(
                &targeted_prim_path,
                name,
            ))
            .as_ref()
            .clone_box(),
        )
    }
}

/// Prim data source rewriting coordSysBinding to point to our coord sys prims.
#[derive(Clone)]
struct PrimDataSource {
    input: HdContainerDataSourceHandle,
}

impl fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrimDataSource").finish_non_exhaustive()
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrimDataSource {
            input: self.input.clone(),
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
        let ds = self.input.get(name)?;
        if *name == **HdCoordSysBindingSchema::get_schema_token() {
            if let Some(container) = cast_to_container(&ds) {
                return Some(Arc::new(CoordSysBindingDataSource { input: container })
                    as HdContainerDataSourceHandle);
            }
            return None;
        }
        Some(ds)
    }
}

fn to_added_entries(paths: &HashSet<SdfPath>) -> Vec<AddedPrimEntry> {
    paths
        .iter()
        .map(|p| AddedPrimEntry {
            prim_path: p.clone(),
            prim_type: tokens::SPRIM_COORD_SYS.clone(),
            data_source: None,
        })
        .collect()
}

fn to_removed_entries(paths: &HashSet<SdfPath>) -> Vec<RemovedPrimEntry> {
    paths
        .iter()
        .map(|p| RemovedPrimEntry::new(p.clone()))
        .collect()
}

/// Coordinate system prim scene index.
pub struct HdsiCoordSysPrimSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<CoordSysPrimSceneIndexState>,
}

#[derive(Default)]
struct CoordSysPrimSceneIndexState {
    targeted_prim_to_name_to_ref_count: HashMap<SdfPath, HashMap<TfToken, usize>>,
    prim_to_bindings: BTreeMap<SdfPath, Vec<Binding>>,
}

impl HdsiCoordSysPrimSceneIndex {
    /// Creates a new coord sys prim scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let slf = Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(CoordSysPrimSceneIndexState::default()),
        };

        let root = SdfPath::absolute_root();
        for prim_path in collect_prim_paths(&input_scene, &root) {
            slf.add_bindings_for_prim(&input_scene, &prim_path, &mut HashSet::new());
        }

        let observer = Arc::new(RwLock::new(slf));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene
                .read()
                .add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    fn get_coord_sys_prim_source(
        &self,
        prim_path: &SdfPath,
    ) -> Option<HdContainerDataSourceHandle> {
        if prim_path.is_absolute_root_path() {
            return None;
        }

        let prim_name = prim_path.get_name();
        if !prim_name.starts_with(COORD_SYS_PRIM_PREFIX) {
            return None;
        }

        let parent_prim_path = prim_path.get_parent_path();
        let state = self.state.lock().expect("Lock poisoned");
        let name_to_ref = state
            .targeted_prim_to_name_to_ref_count
            .get(&parent_prim_path)?;
        let coord_sys_name = TfToken::new(&prim_name[COORD_SYS_PRIM_PREFIX.len()..]);
        if !name_to_ref.contains_key(&coord_sys_name) {
            return None;
        }

        let input = self.base.get_input_scene()?;
        Some(Arc::new(CoordSysPrimDataSource {
            input: input.clone(),
            prim_path: parent_prim_path,
            name: coord_sys_name,
        }))
    }

    fn add_bindings_for_prim(
        &self,
        input: &HdSceneIndexHandle,
        prim_path: &SdfPath,
        added_coord_sys_prims: &mut HashSet<SdfPath>,
    ) {
        let guard = input.read();
        let prim = guard.get_prim(prim_path);
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        let prim_container = prim.data_source.as_ref().unwrap_or(&empty);
        let schema = HdCoordSysBindingSchema::get_from_parent(prim_container);
        drop(guard);

        let mut bindings = Vec::new();
        let mut state = self.state.lock().expect("Lock poisoned");
        for name in schema.get_coord_sys_binding_names() {
            let path_ds = schema.get_coord_sys_binding(&name);
            let path_ds = match path_ds {
                Some(p) => p,
                None => continue,
            };

            let targeted_prim_path = path_ds.get_typed_value(0.0f32);
            if ignore_binding(&targeted_prim_path) {
                continue;
            }

            let name_to_ref = state
                .targeted_prim_to_name_to_ref_count
                .entry(targeted_prim_path.clone())
                .or_insert_with(HashMap::new);

            let count = name_to_ref.entry(name.clone()).or_insert(0);
            if *count == 0 {
                added_coord_sys_prims.insert(make_coord_sys_prim_path(&targeted_prim_path, &name));
            }
            *count += 1;

            bindings.push(Binding {
                name,
                path: targeted_prim_path,
            });
        }

        if !bindings.is_empty() {
            state.prim_to_bindings.insert(prim_path.clone(), bindings);
        }
    }

    fn remove_bindings(
        &self,
        bindings: &[Binding],
        removed_coord_sys_prims: &mut HashSet<SdfPath>,
    ) {
        let mut state = self.state.lock().expect("Lock poisoned");
        for binding in bindings {
            let Some(name_to_ref) = state
                .targeted_prim_to_name_to_ref_count
                .get_mut(&binding.path)
            else {
                continue;
            };

            let Some(count) = name_to_ref.get_mut(&binding.name) else {
                continue;
            };

            if *count == 0 {
                continue;
            }
            *count -= 1;
            if *count > 0 {
                continue;
            }

            removed_coord_sys_prims.insert(make_coord_sys_prim_path(&binding.path, &binding.name));
            name_to_ref.remove(&binding.name);
            if name_to_ref.is_empty() {
                state
                    .targeted_prim_to_name_to_ref_count
                    .remove(&binding.path);
            }
        }
    }

    fn remove_bindings_for_prim(
        &self,
        prim_path: &SdfPath,
        removed_coord_sys_prims: &mut HashSet<SdfPath>,
    ) {
        let bindings = {
            let mut state = self.state.lock().expect("Lock poisoned");
            state.prim_to_bindings.remove(prim_path)
        };
        let Some(bindings) = bindings else {
            return;
        };
        self.remove_bindings(&bindings, removed_coord_sys_prims);
    }

    fn remove_bindings_for_subtree(
        &self,
        prim_path: &SdfPath,
        removed_coord_sys_prims: &mut HashSet<SdfPath>,
    ) {
        let keys: Vec<SdfPath> = {
            let state = self.state.lock().expect("Lock poisoned");
            state
                .prim_to_bindings
                .keys()
                .filter(|k| k.has_prefix(prim_path))
                .cloned()
                .collect()
        };
        for k in keys {
            let bindings = {
                let mut state = self.state.lock().expect("Lock poisoned");
                state.prim_to_bindings.remove(&k)
            };
            if let Some(bindings) = bindings {
                self.remove_bindings(&bindings, removed_coord_sys_prims);
            }
        }
    }
}

impl HdSceneIndexBase for HdsiCoordSysPrimSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(coord_sys_ds) = self.get_coord_sys_prim_source(prim_path) {
            return HdSceneIndexPrim {
                prim_type: tokens::SPRIM_COORD_SYS.clone(),
                data_source: Some(coord_sys_ds),
            };
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };

        let prim = si_ref(&input).get_prim(prim_path);

        if let Some(ds) = prim.data_source {
            HdSceneIndexPrim {
                prim_type: prim.prim_type,
                data_source: Some(Arc::new(PrimDataSource { input: ds })),
            }
        } else {
            prim
        }
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return Vec::new(),
        };

        let mut result = si_ref(&input).get_child_prim_paths(prim_path);

        let state = self.state.lock().expect("Lock poisoned");
        if let Some(name_to_ref) = state.targeted_prim_to_name_to_ref_count.get(prim_path) {
            for name in name_to_ref.keys() {
                result.push(make_coord_sys_prim_path(prim_path, name));
            }
        }

        result
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiCoordSysPrimSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiCoordSysPrimSceneIndex {
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let is_observed = self.base.base().is_observed();
        let mut added_coord_sys_prims = HashSet::new();
        let mut removed_coord_sys_prims = HashSet::new();
        let mut dummy_added = HashSet::new();
        let mut dummy_removed = HashSet::new();

        let input = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => return,
        };

        for entry in entries {
            self.remove_bindings_for_prim(
                &entry.prim_path,
                if is_observed {
                    &mut removed_coord_sys_prims
                } else {
                    &mut dummy_removed
                },
            );
            self.add_bindings_for_prim(
                &input,
                &entry.prim_path,
                if is_observed {
                    &mut added_coord_sys_prims
                } else {
                    &mut dummy_added
                },
            );
        }

        if !is_observed {
            return;
        }

        self.base.forward_prims_added(self, entries);

        if !added_coord_sys_prims.is_empty() {
            self.base
                .base()
                .send_prims_added(sender, &to_added_entries(&added_coord_sys_prims));
        }
        if !removed_coord_sys_prims.is_empty() {
            self.base
                .base()
                .send_prims_removed(sender, &to_removed_entries(&removed_coord_sys_prims));
        }
    }

    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let is_observed = self.base.base().is_observed();
        let mut removed_coord_sys_prims = HashSet::new();
        let mut dummy_removed = HashSet::new();

        let has_bindings = !self
            .state
            .lock()
            .expect("Lock poisoned")
            .prim_to_bindings
            .is_empty();
        if has_bindings {
            for entry in entries {
                self.remove_bindings_for_subtree(
                    &entry.prim_path,
                    if is_observed {
                        &mut removed_coord_sys_prims
                    } else {
                        &mut dummy_removed
                    },
                );
            }
        }

        if !is_observed {
            return;
        }

        self.base.forward_prims_removed(self, entries);

        if !removed_coord_sys_prims.is_empty() {
            self.base
                .base()
                .send_prims_removed(sender, &to_removed_entries(&removed_coord_sys_prims));
        }
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let is_observed = self.base.base().is_observed();
        let mut added_coord_sys_prims = HashSet::new();
        let mut removed_coord_sys_prims = HashSet::new();
        let mut dummy_added = HashSet::new();
        let mut dummy_removed = HashSet::new();

        let input = match self.base.get_input_scene() {
            Some(i) => i.clone(),
            None => return,
        };

        let binding_locator = HdCoordSysBindingSchema::get_default_locator();

        for entry in entries {
            let intersects = entry.dirty_locators.intersects_locator(&binding_locator);

            if intersects {
                self.remove_bindings_for_prim(
                    &entry.prim_path,
                    if is_observed {
                        &mut removed_coord_sys_prims
                    } else {
                        &mut dummy_removed
                    },
                );
                self.add_bindings_for_prim(
                    &input,
                    &entry.prim_path,
                    if is_observed {
                        &mut added_coord_sys_prims
                    } else {
                        &mut dummy_added
                    },
                );
            }
        }

        if !is_observed {
            return;
        }

        self.base.forward_prims_dirtied(self, entries);

        if !added_coord_sys_prims.is_empty() {
            self.base
                .base()
                .send_prims_added(sender, &to_added_entries(&added_coord_sys_prims));
        }
        if !removed_coord_sys_prims.is_empty() {
            self.base
                .base()
                .send_prims_removed(sender, &to_removed_entries(&removed_coord_sys_prims));
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
