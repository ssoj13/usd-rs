//! Prim type and path pruning scene index.
//!
//! Port of pxr/imaging/hdsi/primTypeAndPathPruningSceneIndex.
//!
//! Prunes prims when their type is in a given list AND their path matches
//! a path predicate. Pruned prims keep hierarchy (empty primType, null dataSource).
//! By default the path predicate is empty and no prims are pruned.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Path predicate: returns true if the prim at the path should be pruned.
pub type PathPredicate = Box<dyn Fn(&SdfPath) -> bool + Send + Sync>;

/// Scene index that prunes prims if its type is in a given list and its path
/// matches a given predicate.
///
/// Pruned prims are not removed from the scene index; instead they are given
/// an empty primType and null dataSource to preserve hierarchy.
///
/// By default the predicate is empty and no prims will be pruned.
pub struct HdsiPrimTypeAndPathPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Prim types to prune (from input args, const).
    prim_types: Vec<TfToken>,
    /// Path predicate: when Some, prunes if type matches AND predicate returns true.
    path_predicate: Option<PathPredicate>,
}

fn get_prim_types(args: Option<&HdContainerDataSourceHandle>) -> Vec<TfToken> {
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

impl HdsiPrimTypeAndPathPruningSceneIndex {
    /// Creates a new prim type and path pruning scene index.
    ///
    /// # Arguments
    /// * `input_scene` - The scene index to filter
    /// * `input_args` - Optional container with "primTypes" (Vec<TfToken>)
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let prim_types = get_prim_types(input_args.as_ref());
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            prim_types,
            path_predicate: None,
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

    /// Checks if a prim type should be pruned (is in the prim types list).
    fn should_prune_type(&self, prim_type: &TfToken) -> bool {
        self.prim_types.contains(prim_type)
    }

    /// Sets the path predicate. When the predicate returns true for a path
    /// and the prim type is in the prune list, that prim is pruned.
    /// Setting to None means no prims will be pruned.
    pub fn set_path_predicate(this: &Arc<RwLock<Self>>, path_predicate: Option<PathPredicate>) {
        let (old_predicate, observed, input_handle, prim_types) = {
            let mut guard = this.write();
            let old = std::mem::replace(&mut guard.path_predicate, path_predicate);
            let observed = guard.base.base().is_observed();
            let input = guard.base.get_input_scene().cloned();
            let prim_types = guard.prim_types.clone();
            (old, observed, input, prim_types)
        };

        if !observed {
            return;
        }

        let input_si = match input_handle {
            Some(h) => h,
            None => return,
        };

        let old_fn = old_predicate.as_ref();
        let mut added_entries = Vec::new();
        {
            let guard = this.read();
            let new_fn = guard.path_predicate.as_ref();
            for prim_path in super::utils::collect_prim_paths(&input_si, &SdfPath::absolute_root())
            {
                let old_value = old_fn.map(|f| f(&prim_path)).unwrap_or(false);
                let new_value = new_fn.map(|f| f(&prim_path)).unwrap_or(false);
                if old_value == new_value {
                    continue;
                }

                let prim = si_ref(&input_si).get_prim(&prim_path);
                if !prim_types.contains(&prim.prim_type) {
                    continue;
                }

                let prim_type = if new_value {
                    TfToken::default()
                } else {
                    prim.prim_type
                };
                added_entries.push(AddedPrimEntry {
                    prim_path,
                    prim_type,
                    data_source: prim.data_source,
                });
            }
        }

        if !added_entries.is_empty() {
            let mut guard = this.write();
            let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;
            guard
                .base
                .base_mut()
                .send_prims_added(sender, &added_entries);
        }
    }
}

impl HdSceneIndexBase for HdsiPrimTypeAndPathPruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let prim = si_ref(&input).get_prim(prim_path);
            if let Some(ref pred) = self.path_predicate {
                if self.should_prune_type(&prim.prim_type) && pred(prim_path) {
                    return HdSceneIndexPrim::default();
                }
            }
            return prim;
        }
        HdSceneIndexPrim::default()
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
        "HdsiPrimTypeAndPathPruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiPrimTypeAndPathPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if self.path_predicate.is_none() {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let pred = self.path_predicate.as_ref().unwrap();
        let mut anything_to_filter = false;
        for entry in entries {
            if self.should_prune_type(&entry.prim_type) && pred(&entry.prim_path) {
                anything_to_filter = true;
                break;
            }
        }
        if !anything_to_filter {
            self.base.forward_prims_added(self, entries);
            return;
        }

        let filtered: Vec<AddedPrimEntry> = entries
            .iter()
            .map(|e| {
                let prim_type = if self.should_prune_type(&e.prim_type) && pred(&e.prim_path) {
                    TfToken::default()
                } else {
                    e.prim_type.clone()
                };
                AddedPrimEntry {
                    prim_path: e.prim_path.clone(),
                    prim_type,
                    data_source: e.data_source.clone(),
                }
            })
            .collect();
        self.base.forward_prims_added(self, &filtered);
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
