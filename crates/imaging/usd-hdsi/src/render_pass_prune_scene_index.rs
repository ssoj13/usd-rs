
//! Render pass prune scene index.
//!
//! Port of pxr/imaging/hdsi/renderPassPruneSceneIndex.
//!
//! Applies prune rules of the active render pass specified in HdSceneGlobalsSchema.

use crate::HdCollectionExpressionEvaluator;
use crate::utils::{compile_collection, remove_pruned_children};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{HdCollectionsSchema, HdSceneGlobalsSchema};
use usd_sdf::Path as SdfPath;
use usd_sdf::PathExpression;
use usd_tf::Token as TfToken;

static PRUNE_TOKEN: Lazy<TfToken> = Lazy::new(|| TfToken::new("prune"));

/// Prune state for the active render pass.
struct RenderPassPruneState {
    render_pass_path: SdfPath,
    prune_expr: PathExpression,
    prune_eval: Option<HdCollectionExpressionEvaluator>,
}

impl Default for RenderPassPruneState {
    fn default() -> Self {
        Self {
            render_pass_path: SdfPath::default(),
            prune_expr: PathExpression::new(),
            prune_eval: None,
        }
    }
}

impl RenderPassPruneState {
    fn does_prune(&self, prim_path: &SdfPath) -> bool {
        if let Some(ref eval) = self.prune_eval {
            is_pruned(prim_path, eval)
        } else {
            false
        }
    }
}

use crate::utils::is_pruned;

/// Scene index that applies prune rules of the active render pass.
pub struct HdsiRenderPassPruneSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<RenderPassPruneSceneIndexState>,
}

#[derive(Default)]
struct RenderPassPruneSceneIndexState {
    active_render_pass: RenderPassPruneState,
    has_populated: bool,
}

impl HdsiRenderPassPruneSceneIndex {
    /// Creates a new render pass prune scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(RenderPassPruneSceneIndexState::default()),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    fn update_active_render_pass_state(
        &self,
        added_entries: &mut Vec<AddedPrimEntry>,
        removed_entries: &mut Vec<RemovedPrimEntry>,
    ) {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return,
        };

        let (prior_state, has_populated) = {
            let mut state = self.state.lock().expect("Lock poisoned");
            (
                std::mem::take(&mut state.active_render_pass),
                state.has_populated,
            )
        };
        let mut new_state = RenderPassPruneState::default();

        let input_guard = input.read();
        let globals =
            HdSceneGlobalsSchema::get_from_scene_index(input_guard.deref());
        if let Some(path_ds) = globals.get_active_render_pass_prim() {
            new_state.render_pass_path = path_ds.get_typed_value(0.0);
        }

        if new_state.render_pass_path.is_empty() && prior_state.render_pass_path.is_empty() {
            self.state.lock().expect("Lock poisoned").active_render_pass = new_state;
            return;
        }

        if !new_state.render_pass_path.is_empty() {
            let guard = input.read();
            let pass_prim = guard.get_prim(&new_state.render_pass_path);
            drop(guard);

            if let Some(ref ds) = pass_prim.data_source {
                let collections = HdCollectionsSchema::get_from_parent(ds);
                if collections.is_defined() {
                    compile_collection(
                        &collections,
                        &*PRUNE_TOKEN,
                        &input,
                        &mut new_state.prune_expr,
                        &mut new_state.prune_eval,
                    );
                }
            }
        }

        if new_state.prune_expr == prior_state.prune_expr {
            self.state.lock().expect("Lock poisoned").active_render_pass = new_state;
            return;
        }

        if !has_populated {
            self.state.lock().expect("Lock poisoned").active_render_pass = new_state;
            return;
        }

        let paths = crate::utils::collect_prim_paths(&input, &SdfPath::absolute_root());
        for path in paths {
            if prior_state.does_prune(&path) {
                if !new_state.does_prune(&path) {
                    let prim = si_ref(&input).get_prim(&path);
                    added_entries.push(AddedPrimEntry {
                        prim_path: path,
                        prim_type: prim.prim_type.clone(),
                        data_source: prim.data_source.clone(),
                    });
                }
            } else if new_state.does_prune(&path) {
                removed_entries.push(RemovedPrimEntry::new(path));
            }
        }

        self.state.lock().expect("Lock poisoned").active_render_pass = new_state;
    }
}

use std::ops::Deref;

fn entry_could_affect_pass(entries: &[AddedPrimEntry], active_render_pass_path: &SdfPath) -> bool {
    entries
        .iter()
        .any(|e| e.prim_path.is_absolute_root_path() || e.prim_path == *active_render_pass_path)
}

fn entry_could_affect_pass_removed(
    entries: &[RemovedPrimEntry],
    active_render_pass_path: &SdfPath,
) -> bool {
    entries
        .iter()
        .any(|e| e.prim_path.is_absolute_root_path() || e.prim_path == *active_render_pass_path)
}

fn entry_could_affect_pass_dirtied(
    entries: &[DirtiedPrimEntry],
    active_render_pass_path: &SdfPath,
) -> bool {
    entries
        .iter()
        .any(|e| e.prim_path.is_absolute_root_path() || e.prim_path == *active_render_pass_path)
}

fn prune_added_entries(
    prune_eval: &Option<HdCollectionExpressionEvaluator>,
    entries: &[AddedPrimEntry],
    post_prune: &mut Vec<AddedPrimEntry>,
) -> bool {
    let eval = match prune_eval {
        Some(e) => e,
        None => return false,
    };
    if entries.iter().any(|e| is_pruned(&e.prim_path, eval)) {
        post_prune.extend(
            entries
                .iter()
                .filter(|e| !is_pruned(&e.prim_path, eval))
                .cloned(),
        );
        true
    } else {
        false
    }
}

fn prune_removed_entries(
    prune_eval: &Option<HdCollectionExpressionEvaluator>,
    entries: &[RemovedPrimEntry],
    post_prune: &mut Vec<RemovedPrimEntry>,
) -> bool {
    let eval = match prune_eval {
        Some(e) => e,
        None => return false,
    };
    if entries.iter().any(|e| is_pruned(&e.prim_path, eval)) {
        post_prune.extend(
            entries
                .iter()
                .filter(|e| !is_pruned(&e.prim_path, eval))
                .cloned(),
        );
        true
    } else {
        false
    }
}

fn prune_dirtied_entries(
    prune_eval: &Option<HdCollectionExpressionEvaluator>,
    entries: &[DirtiedPrimEntry],
    post_prune: &mut Vec<DirtiedPrimEntry>,
) -> bool {
    let eval = match prune_eval {
        Some(e) => e,
        None => return false,
    };
    if entries.iter().any(|e| is_pruned(&e.prim_path, eval)) {
        post_prune.extend(
            entries
                .iter()
                .filter(|e| !is_pruned(&e.prim_path, eval))
                .cloned(),
        );
        true
    } else {
        false
    }
}

impl HdSceneIndexBase for HdsiRenderPassPruneSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if prim_path.is_empty() {
            return HdSceneIndexPrim::default();
        }
        if self
            .state
            .lock()
            .expect("Lock poisoned")
            .active_render_pass
            .does_prune(prim_path)
        {
            return HdSceneIndexPrim::default();
        }
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_prim(prim_path);
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return Vec::new(),
        };
        let mut child_paths = si_ref(&input).get_child_prim_paths(prim_path);
        let state = self.state.lock().expect("Lock poisoned");
        if let Some(ref eval) = state.active_render_pass.prune_eval {
            remove_pruned_children(prim_path, eval, &mut child_paths);
        }
        child_paths
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiRenderPassPruneSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiRenderPassPruneSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut extra_added = Vec::new();
        let mut extra_removed = Vec::new();

        let active_render_pass_path = self
            .state
            .lock()
            .expect("Lock poisoned")
            .active_render_pass
            .render_pass_path
            .clone();
        if entry_could_affect_pass(entries, &active_render_pass_path) {
            self.update_active_render_pass_state(&mut extra_added, &mut extra_removed);
        }

        self.state.lock().expect("Lock poisoned").has_populated = true;

        let should_forward_original = {
            let state = self.state.lock().expect("Lock poisoned");
            !prune_added_entries(&state.active_render_pass.prune_eval, entries, &mut extra_added)
        };
        if should_forward_original {
            self.base.forward_prims_added(self, entries);
        }
        self.base.forward_prims_added(self, &extra_added);
        self.base.forward_prims_removed(self, &extra_removed);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut extra_added = Vec::new();
        let mut extra_removed = Vec::new();

        let active_render_pass_path = self
            .state
            .lock()
            .expect("Lock poisoned")
            .active_render_pass
            .render_pass_path
            .clone();
        if entry_could_affect_pass_removed(entries, &active_render_pass_path) {
            self.update_active_render_pass_state(&mut extra_added, &mut extra_removed);
        }

        let should_forward_original = {
            let state = self.state.lock().expect("Lock poisoned");
            !prune_removed_entries(&state.active_render_pass.prune_eval, entries, &mut extra_removed)
        };
        if should_forward_original {
            self.base.forward_prims_removed(self, entries);
        }
        self.base.forward_prims_added(self, &extra_added);
        self.base.forward_prims_removed(self, &extra_removed);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut extra_added = Vec::new();
        let mut extra_removed = Vec::new();
        let mut extra_dirty = Vec::new();

        let active_render_pass_path = self
            .state
            .lock()
            .expect("Lock poisoned")
            .active_render_pass
            .render_pass_path
            .clone();
        let affects_pass = entry_could_affect_pass_dirtied(entries, &active_render_pass_path);
        if affects_pass {
            self.update_active_render_pass_state(&mut extra_added, &mut extra_removed);
        }

        let should_forward_original = {
            let state = self.state.lock().expect("Lock poisoned");
            !prune_dirtied_entries(&state.active_render_pass.prune_eval, entries, &mut extra_dirty)
        };
        if entries.len() >= 1000 || !extra_added.is_empty() || !extra_removed.is_empty() || !extra_dirty.is_empty() {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[render_pass_prune] on_prims_dirtied in={} affects_pass={} forward_original={} extra_added={} extra_removed={} extra_dirty={} sender={} first={}",
                entries.len(),
                affects_pass,
                should_forward_original,
                extra_added.len(),
                extra_removed.len(),
                extra_dirty.len(),
                sender.get_display_name(),
                first,
            );
        }
        if should_forward_original {
            self.base.forward_prims_dirtied(self, entries);
        }
        self.base.forward_prims_added(self, &extra_added);
        self.base.forward_prims_removed(self, &extra_removed);
        self.base.forward_prims_dirtied(self, &extra_dirty);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
