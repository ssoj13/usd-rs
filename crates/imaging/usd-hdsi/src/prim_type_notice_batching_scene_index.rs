//! Prim type notice batching scene index.
//!
//! Port of pxr/imaging/hdsi/primTypeNoticeBatchingSceneIndex.
//!
//! Batches prim notices by type using a priority functor. Notices are held
//! until Flush(). The scene index is empty until the first Flush.

use parking_lot::RwLock;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocatorSet};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Token for input args: `primTypePriorityFunctor`.
pub const PRIM_TYPE_PRIORITY_FUNCTOR_TOKEN: &str = "primTypePriorityFunctor";

/// Data source that holds a [`PrimTypePriorityFunctor`] for passing via input args.
///
/// When building input_args for `HdsiPrimTypeNoticeBatchingSceneIndex::new()`, use
/// this as the value for key "primTypePriorityFunctor" to pass a custom functor.
#[derive(Clone)]
pub struct HdPrimTypePriorityFunctorDataSource {
    functor: Arc<dyn PrimTypePriorityFunctor>,
}

impl HdPrimTypePriorityFunctorDataSource {
    /// Creates a data source holding the given functor.
    pub fn new(functor: Arc<dyn PrimTypePriorityFunctor>) -> Arc<Self> {
        Arc::new(Self { functor })
    }

    /// Returns the held functor.
    pub fn get_functor(&self) -> Arc<dyn PrimTypePriorityFunctor> {
        Arc::clone(&self.functor)
    }
}

impl std::fmt::Debug for HdPrimTypePriorityFunctorDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdPrimTypePriorityFunctorDataSource")
            .finish()
    }
}

impl HdDataSourceBase for HdPrimTypePriorityFunctorDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        None
    }
}

fn get_prim_type_priority_functor_from_args(
    args: Option<&HdContainerDataSourceHandle>,
) -> Option<Arc<dyn PrimTypePriorityFunctor>> {
    let container = args?;
    let ds = container.get(&TfToken::new(PRIM_TYPE_PRIORITY_FUNCTOR_TOKEN))?;
    let base = ds.as_ref() as &dyn HdDataSourceBase;
    base.as_any()
        .downcast_ref::<HdPrimTypePriorityFunctorDataSource>()
        .map(|d| d.get_functor())
}

/// Functor mapping prim types to priorities for notice batching.
/// Prims with lower priority number are flushed before higher priority.
pub trait PrimTypePriorityFunctor: Send + Sync {
    /// Priority for given prim type. Result must be < get_num_priorities().
    fn get_priority_for_prim_type(&self, prim_type: &TfToken) -> usize;

    /// Number of priorities (1 + max value from get_priority_for_prim_type).
    fn get_num_priorities(&self) -> usize;
}

/// Entry for a prim that was added.
#[derive(Clone, Debug)]
struct PrimAddedEntry {
    prim_type: TfToken,
}

/// Entry for a prim that was dirtied.
#[derive(Clone, Debug)]
struct PrimDirtiedEntry {
    dirty_locators: HdDataSourceLocatorSet,
}

/// Either an added or dirtied entry for a path.
#[derive(Clone, Debug)]
enum PrimAddedOrDirtiedEntry {
    Added(PrimAddedEntry),
    Dirtied(PrimDirtiedEntry),
}

impl Default for PrimAddedOrDirtiedEntry {
    fn default() -> Self {
        Self::Dirtied(PrimDirtiedEntry {
            dirty_locators: HdDataSourceLocatorSet::new(),
        })
    }
}

/// Scene index that batches prim change notices by type until Flush().
///
/// Consolidates notices: multiple dirtied entries for the same path become
/// one entry with merged locators. Added overrides dirtied for a path.
/// The scene index forwards nothing until the first Flush.
pub struct HdsiPrimTypeNoticeBatchingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Optional priority functor. When None, single priority (0).
    prim_type_priority_functor: Option<Arc<dyn PrimTypePriorityFunctor>>,
    num_priorities: usize,
    state: Mutex<PrimTypeNoticeBatchingState>,
}

#[derive(Default)]
struct PrimTypeNoticeBatchingState {
    /// True after first Flush.
    populated: bool,
    /// Queued add/dirty entries per path.
    added_or_dirtied_prims: BTreeMap<SdfPath, PrimAddedOrDirtiedEntry>,
    /// Normalized: no element is a prefix of another.
    removed_prims: BTreeSet<SdfPath>,
}

impl HdsiPrimTypeNoticeBatchingSceneIndex {
    /// Creates a new notice batching scene index.
    ///
    /// # Arguments
    /// * `input_scene` - The scene index to wrap
    /// * `input_args` - Optional container. Use key "primTypePriorityFunctor"
    ///   with [`HdPrimTypePriorityFunctorDataSource`] value to pass a custom functor.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let functor = get_prim_type_priority_functor_from_args(input_args.as_ref());
        Self::new_with_functor(input_scene, functor)
    }

    /// Create with explicit priority functor.
    pub fn new_with_functor(
        input_scene: HdSceneIndexHandle,
        functor: Option<Arc<dyn PrimTypePriorityFunctor>>,
    ) -> Arc<RwLock<Self>> {
        let num_priorities = functor
            .as_ref()
            .map(|f| {
                let n = f.get_num_priorities();
                if n == 0 { 1 } else { n }
            })
            .unwrap_or(1);
        let mut added_or_dirtied = BTreeMap::new();
        for prim_path in super::utils::collect_prim_paths(&input_scene, &SdfPath::absolute_root()) {
            {
                let prim = si_ref(&input_scene).get_prim(&prim_path);
                added_or_dirtied.insert(
                    prim_path,
                    PrimAddedOrDirtiedEntry::Added(PrimAddedEntry {
                        prim_type: prim.prim_type,
                    }),
                );
            }
        }
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            prim_type_priority_functor: functor,
            num_priorities,
            state: Mutex::new(PrimTypeNoticeBatchingState {
                populated: false,
                added_or_dirtied_prims: added_or_dirtied,
                removed_prims: BTreeSet::new(),
            }),
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

    fn get_priority(&self, prim_type: &TfToken) -> usize {
        if let Some(ref functor) = self.prim_type_priority_functor {
            let p = functor.get_priority_for_prim_type(prim_type);
            if p >= self.num_priorities {
                self.num_priorities.saturating_sub(1)
            } else {
                p
            }
        } else {
            0
        }
    }

    fn get_priority_for_path(&self, prim_path: &SdfPath) -> usize {
        if let Some(ref _functor) = self.prim_type_priority_functor {
            if let Some(input) = self.base.get_input_scene() {
                let prim = si_ref(&input).get_prim(prim_path);
                return self.get_priority(&prim.prim_type);
            }
        }
        0
    }

    /// Remove entries from added_or_dirtied_prims that have path as prefix.
    fn remove_path_from_added_or_dirtied_prims(&self, path: &SdfPath) {
        let mut state = self.state.lock().expect("Lock poisoned");
        let mut keys_to_remove = Vec::new();
        for (k, _) in &state.added_or_dirtied_prims {
            if k.has_prefix(path) {
                keys_to_remove.push(k.clone());
            }
        }
        for k in keys_to_remove {
            state.added_or_dirtied_prims.remove(&k);
        }
    }

    /// Add path to removed_prims, normalized (no descendants, no if ancestor exists).
    fn add_path_to_removed_prims(&self, path: &SdfPath) {
        use std::ops::Bound;
        let mut state = self.state.lock().expect("Lock poisoned");
        // Check if an ancestor of path is already in the set (prev = largest < path).
        let range_before = state
            .removed_prims
            .range((Bound::Unbounded, Bound::Excluded(path)));
        if let Some(prev) = range_before.last() {
            if path.has_prefix(prev) {
                return;
            }
        }
        if state.removed_prims.contains(path) {
            return;
        }
        let to_remove: Vec<SdfPath> = state
            .removed_prims
            .range((Bound::Included(path), Bound::Unbounded))
            .take_while(|p| p.has_prefix(path))
            .cloned()
            .collect();
        for p in &to_remove {
            state.removed_prims.remove(p);
        }
        state.removed_prims.insert(path.clone());
    }

    /// Sends out all queued notices. First call also sends initial scene state.
    pub fn flush(this: &Arc<RwLock<Self>>) {
        let (observed, removed_entries, added_by_priority, dirtied_by_priority, num_priorities) = {
            let guard = this.read();
            let mut state = guard.state.lock().expect("Lock poisoned");
            state.populated = true;
            let observed = guard.base.base().is_observed();
            if !observed {
                state.removed_prims.clear();
                state.added_or_dirtied_prims.clear();
                return;
            }

            let removed_entries: Vec<RemovedPrimEntry> = state
                .removed_prims
                .iter()
                .map(|p| RemovedPrimEntry::new(p.clone()))
                .collect();

            let mut added_by_priority: Vec<Vec<AddedPrimEntry>> =
                (0..guard.num_priorities).map(|_| Vec::new()).collect();
            let mut dirtied_by_priority: Vec<Vec<DirtiedPrimEntry>> =
                (0..guard.num_priorities).map(|_| Vec::new()).collect();

            for (path, entry) in &state.added_or_dirtied_prims {
                match entry {
                    PrimAddedOrDirtiedEntry::Added(added) => {
                        let priority = guard.get_priority(&added.prim_type);
                        added_by_priority[priority].push(AddedPrimEntry {
                            prim_path: path.clone(),
                            prim_type: added.prim_type.clone(),
                            data_source: None,
                        });
                    }
                    PrimAddedOrDirtiedEntry::Dirtied(dirtied) => {
                        let priority = guard.get_priority_for_path(path);
                        dirtied_by_priority[priority].push(DirtiedPrimEntry::new(
                            path.clone(),
                            dirtied.dirty_locators.clone(),
                        ));
                    }
                }
            }

            state.removed_prims.clear();
            state.added_or_dirtied_prims.clear();

            (
                observed,
                removed_entries,
                added_by_priority,
                dirtied_by_priority,
                guard.num_priorities,
            )
        };

        if !observed {
            return;
        }

        let guard = this.read();
        let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
        let sender = &delegate as &dyn HdSceneIndexBase;
        if !removed_entries.is_empty() {
            guard
                .base
                .base()
                .send_prims_removed(sender, &removed_entries);
        }
        for priority in 0..num_priorities {
            if !added_by_priority[priority].is_empty() {
                guard
                    .base
                    .base()
                    .send_prims_added(sender, &added_by_priority[priority]);
            }
            if !dirtied_by_priority[priority].is_empty() {
                guard
                    .base
                    .base()
                    .send_prims_dirtied(sender, &dirtied_by_priority[priority]);
            }
        }
    }
}

impl HdSceneIndexBase for HdsiPrimTypeNoticeBatchingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if !self.state.lock().expect("Lock poisoned").populated {
            return HdSceneIndexPrim::default();
        }
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_prim(prim_path);
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if !self.state.lock().expect("Lock poisoned").populated {
            return Vec::new();
        }
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
        "HdsiPrimTypeNoticeBatchingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiPrimTypeNoticeBatchingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        for entry in entries {
            state.added_or_dirtied_prims.insert(
                entry.prim_path.clone(),
                PrimAddedOrDirtiedEntry::Added(PrimAddedEntry {
                    prim_type: entry.prim_type.clone(),
                }),
            );
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        for entry in entries {
            self.remove_path_from_added_or_dirtied_prims(&entry.prim_path);
            self.add_path_to_removed_prims(&entry.prim_path);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        for entry in entries {
            let e = state
                .added_or_dirtied_prims
                .entry(entry.prim_path.clone())
                .or_default();
            match e {
                PrimAddedOrDirtiedEntry::Added(_) => {}
                PrimAddedOrDirtiedEntry::Dirtied(dirtied) => {
                    dirtied.dirty_locators.insert_set(&entry.dirty_locators);
                }
            }
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
