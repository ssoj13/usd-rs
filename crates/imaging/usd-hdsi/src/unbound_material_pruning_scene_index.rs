
//! Unbound material pruning scene index.
//!
//! Port of pxr/imaging/hdsi/unboundMaterialPruningSceneIndex.
//!
//! Clears prim type and data source for material prims that are not bound
//! to any geometry. Does not remove prims from topology.

use crate::tokens::UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS;
use crate::utils;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::HdTypedSampledDataSource;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocatorSet,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{HdMaterialBindingsSchema, MATERIAL_BINDING_ALL_PURPOSE};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

fn get_material_binding_purposes(input_args: Option<&HdContainerDataSourceHandle>) -> Vec<TfToken> {
    let container = match input_args {
        Some(c) => c,
        None => return vec![MATERIAL_BINDING_ALL_PURPOSE.clone().clone()],
    };
    let ds = match container
        .get(&UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS.material_binding_purposes)
    {
        Some(d) => d,
        None => return vec![MATERIAL_BINDING_ALL_PURPOSE.clone().clone()],
    };
    if let Some(typed) = (ds.as_ref() as &dyn HdDataSourceBase)
        .as_any()
        .downcast_ref::<HdRetainedTypedSampledDataSource<Vec<TfToken>>>()
    {
        let v = typed.get_typed_value(0.0);
        if !v.is_empty() {
            return v;
        }
    }
    vec![MATERIAL_BINDING_ALL_PURPOSE.clone().clone()]
}

fn compute_binding_locators(binding_purposes: &[TfToken]) -> HdDataSourceLocatorSet {
    let mut locators = HdDataSourceLocatorSet::new();
    for purpose in binding_purposes {
        locators.insert(HdMaterialBindingsSchema::get_default_locator().append(purpose));
    }
    locators
}

fn get_bound_material_paths(
    prim_container: Option<&HdContainerDataSourceHandle>,
    binding_purposes: &[TfToken],
) -> Vec<SdfPath> {
    let container = match prim_container {
        Some(c) => c,
        None => return Vec::new(),
    };
    let bindings = HdMaterialBindingsSchema::get_from_parent(container);
    if !bindings.is_defined() {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for purpose in binding_purposes {
        let binding = bindings.get_material_binding(purpose);
        if let Some(path) = binding.get_path() {
            paths.push(path);
        }
    }
    paths
}

/// Unbound material pruning scene index.
///
/// Clears prim type and data source for material prims that are not bound
/// to any geometry. Does not remove prims from the scene topology.
pub struct HdsiUnboundMaterialPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    binding_purposes: Vec<TfToken>,
    binding_locators: HdDataSourceLocatorSet,
    state: Mutex<UnboundMaterialPruningState>,
}

#[derive(Default)]
struct UnboundMaterialPruningState {
    bound_material_paths: HashSet<SdfPath>,
    added_material_paths: HashSet<SdfPath>,
}

impl HdsiUnboundMaterialPruningSceneIndex {
    /// Creates a new unbound material pruning scene index.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let binding_purposes = get_material_binding_purposes(input_args.as_ref());
        let binding_locators = compute_binding_locators(&binding_purposes);

        let slf = Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            binding_purposes: binding_purposes.clone(),
            binding_locators: binding_locators.clone(),
            state: Mutex::new(UnboundMaterialPruningState::default()),
        };
        slf.populate_from_input_scene(&input_scene);

        let observer = Arc::new(RwLock::new(slf));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    fn is_bound_material(&self, prim_path: &SdfPath) -> bool {
        self.state
            .lock()
            .expect("Lock poisoned")
            .bound_material_paths
            .contains(prim_path)
    }

    fn was_added(&self, prim_path: &SdfPath) -> bool {
        self.state
            .lock()
            .expect("Lock poisoned")
            .added_material_paths
            .contains(prim_path)
    }

    fn populate_from_input_scene(&self, input: &HdSceneIndexHandle) {
        let root = SdfPath::absolute_root();
        let prim_paths = utils::collect_prim_paths(input, &root);
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        let mut state = self.state.lock().expect("Lock poisoned");

        let guard = input.read();

        for prim_path in prim_paths {
            let prim = guard.get_prim(&prim_path);

            if prim.prim_type.is_empty() {
                continue;
            }

            if prim.prim_type == *tokens::SPRIM_MATERIAL {
                state.added_material_paths.insert(prim_path);
                continue;
            }

            let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);
            let bound = get_bound_material_paths(Some(prim_ds), &self.binding_purposes);
            if bound.is_empty() {
                continue;
            }
            for p in bound {
                state.bound_material_paths.insert(p);
            }
        }

        drop(guard);
        // Note: We don't send PrimsAdded for unbound materials here because
        // we haven't registered our observer yet. The C++ does send, but at
        // ctor time there are typically no observers. Observers added later
        // will get correct state on first query.
    }

    /// Controls whether unbound materials should be pruned.
    pub fn set_prune_unbound_materials(&mut self, _prune: bool) {
        // Stored for API but we always prune when enabled - the logic is in get_prim
    }
}

impl HdSceneIndexBase for HdsiUnboundMaterialPruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            return HdSceneIndexPrim::default();
        };

        if prim.prim_type == *tokens::SPRIM_MATERIAL && !self.is_bound_material(prim_path) {
            prim.prim_type = TfToken::default();
            prim.data_source = None;
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
        "HdsiUnboundMaterialPruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiUnboundMaterialPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => {
                self.base.forward_prims_added(self, entries);
                return;
            }
        };
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();

        let mut added_material_indices = Vec::new();
        let mut bound_material_paths = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            if entry.prim_type.is_empty() {
                continue;
            }
            if entry.prim_type == *tokens::SPRIM_MATERIAL {
                added_material_indices.push(i);
                continue;
            }
            let prim = si_ref(&input).get_prim(&entry.prim_path);
            let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);
            bound_material_paths.extend(get_bound_material_paths(
                Some(prim_ds),
                &self.binding_purposes,
            ));
        }

        let mut newly_bound = Vec::new();
        for mat_path in &bound_material_paths {
            if self.was_added(mat_path) && !self.is_bound_material(mat_path) {
                newly_bound.push(mat_path.clone());
            }
        }
        {
            let mut state = self.state.lock().expect("Lock poisoned");
            state.bound_material_paths.extend(bound_material_paths);
        }

        let mut added_unbound_indices = Vec::new();
        for &i in &added_material_indices {
            let mat_path = &entries[i].prim_path;
            if !self.is_bound_material(mat_path) {
                added_unbound_indices.push(i);
            }
            self.state
                .lock()
                .expect("Lock poisoned")
                .added_material_paths
                .insert(mat_path.clone());
        }

        if newly_bound.is_empty() && added_unbound_indices.is_empty() {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let mut edited: Vec<AddedPrimEntry> = entries.to_vec();
        for i in added_unbound_indices {
            edited[i].prim_type = TfToken::default();
        }
        for mat_path in newly_bound {
            edited.push(AddedPrimEntry {
                prim_path: mat_path,
                prim_type: tokens::SPRIM_MATERIAL.clone(),
                data_source: None,
            });
        }
        self.base.forward_prims_added(self, &edited);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        for entry in entries {
            state
                .added_material_paths
                .retain(|path| !path.has_prefix(&entry.prim_path));
            state
                .bound_material_paths
                .retain(|path| !path.has_prefix(&entry.prim_path));
        }
        drop(state);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => {
                self.base.forward_prims_dirtied(self, entries);
                return;
            }
        };
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();

        let mut newly_bound = Vec::new();
        let mut intersecting_entries = 0usize;

        for entry in entries {
            if !entry.dirty_locators.intersects(&self.binding_locators) {
                continue;
            }
            intersecting_entries += 1;
            let prim = si_ref(&input).get_prim(&entry.prim_path);
            if prim.prim_type.is_empty() {
                continue;
            }
            let prim_ds = prim.data_source.as_ref().unwrap_or(&empty);
            for mat_path in get_bound_material_paths(Some(prim_ds), &self.binding_purposes) {
                if !self.is_bound_material(&mat_path) {
                    if self.was_added(&mat_path) {
                        newly_bound.push(mat_path.clone());
                    }
                    self.state
                        .lock()
                        .expect("Lock poisoned")
                        .bound_material_paths
                        .insert(mat_path);
                }
            }
        }

        if entries.len() >= 500 || intersecting_entries >= 100 || !newly_bound.is_empty() {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_else(|| "<none>".to_string());
            eprintln!(
                "[unbound_material_pruning] on_prims_dirtied in={} intersects={} newly_bound={} sender={} first={}",
                entries.len(),
                intersecting_entries,
                newly_bound.len(),
                sender.get_display_name(),
                first_path
            );
        }

        if !newly_bound.is_empty() {
            let added: Vec<AddedPrimEntry> = newly_bound
                .into_iter()
                .map(|p| AddedPrimEntry {
                    prim_path: p,
                    prim_type: tokens::SPRIM_MATERIAL.clone(),
                    data_source: None,
                })
                .collect();
            // These material reintroductions are synthesized by this pruning
            // scene index. Downstream observers must see this filtering view as
            // the sender, matching OpenUSD's `_SendPrimsAdded(this, ...)`
            // contract for derived notices.
            self.base.base().send_prims_added(self, &added);
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
