//! Prim type pruning scene index.
//!
//! Port of pxr/imaging/hdsi/primTypePruningSceneIndex.
//!
//! Prunes prims of given type (e.g., material) and optionally bindings to that type.
//! Pruned prims keep hierarchy but get empty primType and null dataSource.
//!
//! # Deprecation
//!
//! Use `HdsiSceneMaterialPruningSceneIndex` or `HdsiPrimTypeAndPathPruningSceneIndex` instead.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Container that filters out a binding token from its input.
/// Port of C++ _PrimDataSource in primTypePruningSceneIndex.cpp.
#[derive(Clone)]
struct BindingFilterContainerDataSource {
    input: HdContainerDataSourceHandle,
    exclude_token: TfToken,
}

impl BindingFilterContainerDataSource {
    fn new(input: HdContainerDataSourceHandle, exclude_token: TfToken) -> Arc<Self> {
        Arc::new(Self {
            input,
            exclude_token,
        })
    }
}

impl std::fmt::Debug for BindingFilterContainerDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindingFilterContainerDataSource")
            .field("exclude_token", &self.exclude_token)
            .finish()
    }
}

impl HdDataSourceBase for BindingFilterContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input: self.input.clone(),
            exclude_token: self.exclude_token.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for BindingFilterContainerDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input
            .get_names()
            .into_iter()
            .filter(|n| n != &self.exclude_token)
            .collect()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name == &self.exclude_token {
            return None;
        }
        self.input.get(name)
    }
}

/// Prim type pruning scene index.
///
/// Pruned prims keep hierarchy but get empty primType and null dataSource.
/// When binding_token is set, prims that bind pruned types get their binding filtered.
pub struct HdsiPrimTypePruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    prim_types: Vec<TfToken>,
    binding_token: TfToken,
    do_not_prune_non_prim_paths: bool,
    state: Mutex<PrimTypePruningState>,
}

#[derive(Default)]
struct PrimTypePruningState {
    prune_map: HashMap<SdfPath, bool>,
    enabled: bool,
}

/// Extract Vec<TfToken> from input args.
fn get_prim_types(args: Option<&HdContainerDataSourceHandle>) -> Vec<TfToken> {
    use usd_hd::data_source::{HdRetainedTypedSampledDataSource, HdTypedSampledDataSource};

    let container = match args {
        Some(c) => c,
        None => return Vec::new(),
    };
    let ds = match container.get(&TfToken::new("primTypes")) {
        Some(d) => d,
        None => return Vec::new(),
    };
    (ds.as_ref() as &dyn HdDataSourceBase)
        .as_any()
        .downcast_ref::<HdRetainedTypedSampledDataSource<Vec<TfToken>>>()
        .map(|r| r.get_typed_value(0.0))
        .unwrap_or_default()
}

/// Extract TfToken from input args.
fn get_binding_token(args: Option<&HdContainerDataSourceHandle>) -> TfToken {
    use usd_hd::data_source::{HdRetainedTypedSampledDataSource, HdTypedSampledDataSource};

    let container = match args {
        Some(c) => c,
        None => return TfToken::default(),
    };
    let ds = match container.get(&TfToken::new("bindingToken")) {
        Some(d) => d,
        None => return TfToken::default(),
    };
    (ds.as_ref() as &dyn HdDataSourceBase)
        .as_any()
        .downcast_ref::<HdRetainedTypedSampledDataSource<TfToken>>()
        .map(|r| r.get_typed_value(0.0))
        .unwrap_or_default()
}

/// Extract bool from input args.
fn get_do_not_prune_non_prim_paths(args: Option<&HdContainerDataSourceHandle>) -> bool {
    use usd_hd::data_source::{HdRetainedTypedSampledDataSource, HdTypedSampledDataSource};

    let container = match args {
        Some(c) => c,
        None => return false,
    };
    let ds = match container.get(&TfToken::new("doNotPruneNonPrimPaths")) {
        Some(d) => d,
        None => return false,
    };
    (ds.as_ref() as &dyn HdDataSourceBase)
        .as_any()
        .downcast_ref::<HdRetainedTypedSampledDataSource<bool>>()
        .map(|r| r.get_typed_value(0.0))
        .unwrap_or(false)
}

impl HdsiPrimTypePruningSceneIndex {
    /// Creates a new prim type pruning scene index.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene index to filter
    /// * `input_args` - Optional container with:
    ///   - `primTypes`: Vec<TfToken> of types to prune
    ///   - `bindingToken`: TfToken for binding to also prune
    ///   - `doNotPruneNonPrimPaths`: bool, if true only prune prim paths
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let prim_types = get_prim_types(input_args.as_ref());
        let binding_token = get_binding_token(input_args.as_ref());
        let do_not_prune_non_prim_paths = get_do_not_prune_non_prim_paths(input_args.as_ref());

        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            prim_types,
            binding_token,
            do_not_prune_non_prim_paths,
            state: Mutex::new(PrimTypePruningState::default()),
        }));
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

    /// Creates a new prim type pruning scene index with explicit configuration.
    ///
    /// This is a convenience method that avoids the need to construct a data source.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene index to filter
    /// * `prim_types` - List of prim type tokens to prune
    /// * `binding_token` - Optional binding token for pruning bindings
    /// * `do_not_prune_non_prim_paths` - If true, only prune actual prim paths
    pub fn new_with_config(
        input_scene: HdSceneIndexHandle,
        prim_types: Vec<TfToken>,
        binding_token: Option<TfToken>,
        do_not_prune_non_prim_paths: bool,
    ) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            prim_types,
            binding_token: binding_token.unwrap_or_default(),
            do_not_prune_non_prim_paths,
            state: Mutex::new(PrimTypePruningState::default()),
        }));
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

    /// Returns whether pruning is currently enabled.
    ///
    /// # Returns
    ///
    /// `true` if prims of specified types will be pruned, `false` otherwise
    pub fn get_enabled(&self) -> bool {
        self.state.lock().expect("Lock poisoned").enabled
    }

    /// Enables or disables prim type pruning.
    ///
    /// When the state changes, sends PrimsAdded for affected prims.
    /// Call via `HdsiPrimTypePruningSceneIndex::set_enabled(&scene_index, enabled)`.
    pub fn set_enabled(this: &Arc<RwLock<Self>>, enabled: bool) {
        let (input_handle, prim_types, do_not_prune) = {
            let guard = this.read();
            let mut state = guard.state.lock().expect("Lock poisoned");
            if state.enabled == enabled {
                return;
            }
            state.enabled = enabled;
            (
                guard.base.get_input_scene().cloned(),
                guard.prim_types.clone(),
                guard.do_not_prune_non_prim_paths,
            )
        };

        let input_si = match input_handle {
            Some(h) => h,
            None => return,
        };

        let prune_path = |path: &SdfPath| {
            if do_not_prune {
                path.is_prim_path()
            } else {
                true
            }
        };

        let mut added_entries = Vec::new();

        if enabled {
            for prim_path in super::utils::collect_prim_paths(&input_si, &SdfPath::absolute_root())
            {
                if !prune_path(&prim_path) {
                    continue;
                }
                let prim = si_ref(&input_si).get_prim(&prim_path);
                if prim_types.contains(&prim.prim_type) {
                    this.read()
                        .state
                        .lock()
                        .expect("Lock poisoned")
                        .prune_map
                        .insert(prim_path.clone(), true);
                    added_entries.push(AddedPrimEntry {
                        prim_path,
                        prim_type: TfToken::empty(),
                        data_source: None,
                    });
                }
            }
        } else {
            let previously_pruned: Vec<SdfPath> = this
                .read()
                .state
                .lock()
                .expect("Lock poisoned")
                .prune_map
                .keys()
                .cloned()
                .collect();
            for prim_path in previously_pruned {
                let prim = si_ref(&input_si).get_prim(&prim_path);
                if !prim.prim_type.is_empty() {
                    added_entries.push(AddedPrimEntry {
                        prim_path,
                        prim_type: prim.prim_type,
                        data_source: prim.data_source,
                    });
                }
            }
            this.read()
                .state
                .lock()
                .expect("Lock poisoned")
                .prune_map
                .clear();
        }

        if !added_entries.is_empty() {
            let guard = this.read();
            let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;
            guard.base.base().send_prims_added(sender, &added_entries);
        }
    }

    /// Checks if a prim type should be pruned.
    ///
    /// # Arguments
    ///
    /// * `prim_type` - The type token to check
    ///
    /// # Returns
    ///
    /// `true` if pruning is enabled and the type is in the pruning list
    fn prune_type(&self, prim_type: &TfToken) -> bool {
        self.state.lock().expect("Lock poisoned").enabled && self.prim_types.contains(prim_type)
    }

    /// Checks if a path should be pruned.
    ///
    /// When `do_not_prune_non_prim_paths` is true, only actual prim paths
    /// should be pruned (property paths should be preserved).
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check
    ///
    /// # Returns
    ///
    /// `true` if the path should be pruned
    fn prune_path(&self, path: &SdfPath) -> bool {
        if self.do_not_prune_non_prim_paths {
            // Only prune actual prim paths, not property paths
            path.is_prim_path()
        } else {
            // Prune all paths
            true
        }
    }
}

impl HdSceneIndexBase for HdsiPrimTypePruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let input_locked = input.read();
        let mut prim = input_locked.get_prim(prim_path);

        if !self.state.lock().expect("Lock poisoned").enabled {
            return prim;
        }
        if !self.prune_path(prim_path) {
            return prim;
        }
        if self.prune_type(&prim.prim_type) {
            return HdSceneIndexPrim::default();
        }

        if !self.binding_token.is_empty() {
            if let Some(ref ds) = prim.data_source {
                if ds.get(&self.binding_token).is_some() {
                    let filtered = BindingFilterContainerDataSource::new(
                        ds.clone(),
                        self.binding_token.clone(),
                    );
                    prim.data_source = Some(filtered);
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

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiPrimTypePruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiPrimTypePruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.state.lock().expect("Lock poisoned").enabled {
            self.base.forward_prims_added(self, entries);
            return;
        }
        let mut state = self.state.lock().expect("Lock poisoned");
        let filtered: Vec<AddedPrimEntry> = entries
            .iter()
            .map(|e| {
                let mut entry = e.clone();
                if self.prune_path(&entry.prim_path) && self.prim_types.contains(&entry.prim_type) {
                    state.prune_map.insert(entry.prim_path.clone(), true);
                    entry.prim_type = TfToken::empty();
                    entry.data_source = None;
                }
                entry
            })
            .collect();
        drop(state);
        self.base.forward_prims_added(self, &filtered);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        for entry in entries {
            state
                .prune_map
                .retain(|path, _| !path.has_prefix(&entry.prim_path));
        }
        drop(state);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
