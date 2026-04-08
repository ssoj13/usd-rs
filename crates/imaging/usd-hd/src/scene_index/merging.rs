//! Merging scene index - merges multiple scene indices.
//!
//! G11: InputScene has pos field.
//! G12: Path-based optimization for >5 inputs via _inputsPathTable.
//! G16: _RebuildInputsPathTable / _AddStrictPrefixesOfSceneRoots.
//! G17: Parallel recursive traversal for added entries (rayon).
//! G18: Empty container for path-table ancestors in GetPrim.
//! G19: Intermediate children in GetChildPrimPaths.
//!
//! # References
//!
//! OpenUSD: `pxr/imaging/hd/mergingSceneIndex.h`

use super::base::HdSceneIndexBaseImpl;
use super::base::{HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector, TfTokenVector, si_ref};
use super::filtering::HdFilteringSceneIndexBase;
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry,
};
use super::prim::HdSceneIndexPrim;
use crate::data_source::{
    HdContainerDataSourceHandle, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
};
use parking_lot::{Mutex, RwLock};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Weak};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// ---------------------------------------------------------------------------
// InputScene (G11: pos field)
// ---------------------------------------------------------------------------

/// Entry for adding a scene to the merging scene index.
///
/// G11: Added `pos` field matching C++ InputScene::pos.
pub struct InputScene {
    /// The scene to add
    pub scene: HdSceneIndexHandle,
    /// The shallowest path at which prims should be considered
    pub active_input_scene_root: SdfPath,
    /// G11: Position where to insert the scene (default: usize::MAX = append)
    pub pos: usize,
    /// Observer handle registered on this input (for cleanup on remove)
    #[allow(dead_code)] // C++ uses for RemoveInputScene cleanup, not yet wired
    observer_handle: Option<HdSceneIndexObserverHandle>,
}

impl InputScene {
    /// Create a new input scene entry with default pos (append).
    pub fn new(scene: HdSceneIndexHandle, active_input_scene_root: SdfPath) -> Self {
        Self {
            scene,
            active_input_scene_root,
            pos: usize::MAX,
            observer_handle: None,
        }
    }

    /// Create a new input scene entry with explicit position.
    pub fn with_pos(
        scene: HdSceneIndexHandle,
        active_input_scene_root: SdfPath,
        pos: usize,
    ) -> Self {
        Self {
            scene,
            active_input_scene_root,
            pos,
            observer_handle: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal _InputEntry (parallel to C++ _InputEntry)
// ---------------------------------------------------------------------------

/// Internal input entry (scene + scene root + live observer handle).
///
/// The observer_handle keeps the MergingObserver alive. C++ stores the observer
/// as a plain member `_observer` (lifetime = self); Rust needs an explicit
/// strong Arc because `add_observer` stores only a Weak reference.
#[derive(Clone)]
struct InputEntry {
    scene_index: HdSceneIndexHandle,
    scene_root: SdfPath,
    /// Strong reference to the observer registered on `scene_index`. Keeps the
    /// Weak stored by the input's observer list alive for as long as this input
    /// is present.
    observer_handle: Option<HdSceneIndexObserverHandle>,
}

// ---------------------------------------------------------------------------
// MergingObserver
// ---------------------------------------------------------------------------

struct MergingObserver {
    owner: Weak<RwLock<HdMergingSceneIndex>>,
}

impl MergingObserver {
    fn new(owner: Weak<RwLock<HdMergingSceneIndex>>) -> Self {
        Self { owner }
    }
}

impl HdSceneIndexObserver for MergingObserver {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            owner.handle_prims_added(sender, entries);
        }
    }

    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            owner.handle_prims_removed(sender, entries);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            owner.handle_prims_dirtied(entries);
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) =
            super::observer::convert_prims_renamed_to_removed_and_added(sender, entries);

        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            if !removed.is_empty() {
                owner.handle_prims_removed(sender, &removed);
            }
            if !added.is_empty() {
                owner.handle_prims_added(sender, &added);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HdMergingSceneIndex
// ---------------------------------------------------------------------------

/// Interior-mutable state guarded by `Mutex`.
///
/// Separated from `HdMergingSceneIndex` so that mutating methods can release
/// the lock *before* sending observer notifications, preventing the deadlock
/// where an observer cascades back into `get_prim` on the same scene index.
struct MergingState {
    /// Input entries in order (earlier = stronger)
    inputs: Vec<InputEntry>,
    /// G12/G16: Path table mapping SdfPath -> relevant input entries.
    /// Only populated when inputs.len() >= 5.
    inputs_path_table: BTreeMap<SdfPath, Vec<InputEntry>>,
}

/// Merges multiple scenes together.
///
/// For prims that exist in more than one input scene, data sources are
/// overlaid (down to the leaf) with earlier-inserted scenes having
/// stronger opinions.
///
/// G12: Uses path-based optimization (_inputsPathTable) for >5 inputs.
///
/// All public mutating methods take `&self` and use interior mutability
/// (`Mutex<MergingState>`) so that observer notifications are sent
/// *after* the lock is released -- this prevents deadlocks when observers
/// cascade back into `get_prim`/`get_child_prim_paths`.
pub struct HdMergingSceneIndex {
    /// Base implementation for observer management (has its own internal Mutex)
    base: HdSceneIndexBaseImpl,
    /// Mutable state: inputs + path table
    state: Mutex<MergingState>,
    /// Weak self-reference for creating observer handles
    self_ref: Mutex<Option<Weak<RwLock<Self>>>>,
}

impl HdMergingSceneIndex {
    /// Create a new merging scene index.
    pub fn new() -> Arc<RwLock<Self>> {
        let scene = Arc::new(RwLock::new(Self {
            base: HdSceneIndexBaseImpl::new(),
            state: Mutex::new(MergingState {
                inputs: Vec::new(),
                inputs_path_table: BTreeMap::new(),
            }),
            self_ref: Mutex::new(None),
        }));

        *scene.read().self_ref.lock() = Some(Arc::downgrade(&scene));

        scene
    }

    fn make_observer(&self) -> Option<HdSceneIndexObserverHandle> {
        let weak = self.self_ref.lock().as_ref()?.clone();
        let obs = MergingObserver::new(weak);
        Some(Arc::new(obs) as HdSceneIndexObserverHandle)
    }

    /// Register a MergingObserver on `scene` and return the strong handle.
    ///
    /// MUST NOT be called while holding `self.state` lock -- `add_observer`
    /// may cascade through scene index chain.
    fn register_observer_on_input(
        &self,
        scene: &HdSceneIndexHandle,
    ) -> Option<HdSceneIndexObserverHandle> {
        let obs_handle = self.make_observer()?;
        scene.read().add_observer(obs_handle.clone());
        Some(obs_handle)
    }

    fn unregister_observer_from_input(
        scene: &HdSceneIndexHandle,
        observer_handle: &Option<HdSceneIndexObserverHandle>,
    ) {
        if let Some(handle) = observer_handle {
            scene.read().remove_observer(handle);
        }
    }

    // -----------------------------------------------------------------------
    // G16: _RebuildInputsPathTable
    // -----------------------------------------------------------------------

    /// Rebuild the path table from current inputs.
    ///
    /// Port of C++ `_RebuildInputsPathTable`. Caller must hold `state` lock.
    fn rebuild_inputs_path_table(state: &mut MergingState) {
        state.inputs_path_table.clear();

        if state.inputs.len() < 5 {
            return; // G12: Skip table for small input counts
        }

        // Create entries for each scene root (and implicitly ancestors via BTreeMap)
        for input in &state.inputs {
            state
                .inputs_path_table
                .entry(input.scene_root.clone())
                .or_default();
        }

        // Populate table entries with relevant inputs
        let paths: Vec<SdfPath> = state.inputs_path_table.keys().cloned().collect();
        for path in &paths {
            let mut relevant: Vec<InputEntry> = Vec::new();
            for input in &state.inputs {
                if path.has_prefix(&input.scene_root) || input.scene_root.has_prefix(path) {
                    relevant.push(input.clone());
                }
            }
            state.inputs_path_table.insert(path.clone(), relevant);
        }
    }

    // -----------------------------------------------------------------------
    // G16: _AddStrictPrefixesOfSceneRoots
    // -----------------------------------------------------------------------

    /// Add strict prefixes of active scene roots as AddedPrimEntries.
    ///
    /// Port of C++ `_AddStrictPrefixesOfSceneRoots`. If adding a scene at
    /// e.g. /A/B/C, creates AddedPrimEntries for /A and /A/B.
    ///
    /// Must NOT hold state lock when calling this -- it calls `has_prim`
    /// which locks state internally.
    fn add_strict_prefixes_of_scene_roots(
        &self,
        input_scenes: &[InputScene],
        added_entries: &mut Vec<AddedPrimEntry>,
    ) {
        let mut visited = HashSet::new();

        for input_scene in input_scenes {
            let scene_root = &input_scene.active_input_scene_root;

            if !scene_root.is_absolute_root_or_prim_path() {
                continue;
            }

            let n = scene_root.get_path_element_count();
            if n <= 1 {
                continue;
            }

            let prefixes = scene_root.get_prefixes();
            let mut has_prim = true;

            for prefix in &prefixes[..prefixes.len().saturating_sub(1)] {
                if prefix.is_absolute_root_path() {
                    continue;
                }
                has_prim = has_prim && self.has_prim(prefix);
                if has_prim {
                    continue;
                }
                if !visited.insert(prefix.clone()) {
                    continue;
                }
                added_entries.push(AddedPrimEntry::new(prefix.clone(), TfToken::empty()));
            }
        }
    }

    /// Check if a prim exists at the given path.
    fn has_prim(&self, path: &SdfPath) -> bool {
        {
            let state = self.state.lock();
            if state.inputs_path_table.contains_key(path) {
                return true;
            }
        }
        // Lock released before calling compose which also locks
        self.compose_prim_from_inputs(path).is_defined()
    }

    // -----------------------------------------------------------------------
    // G12: _GetInputEntriesByPath
    // -----------------------------------------------------------------------

    /// Get relevant input entries for a path (snapshot from locked state).
    ///
    /// Port of C++ `_GetInputEntriesByPath`. For <5 inputs, returns all.
    /// For >=5, uses path table for O(log N) lookup.
    /// Returns owned Vec because we can't return references into Mutex-guarded data.
    fn get_input_entries_by_path(state: &MergingState, prim_path: &SdfPath) -> Vec<InputEntry> {
        if state.inputs.len() < 5 {
            return state.inputs.clone();
        }

        // Find closest enclosing path table entry
        let mut p = prim_path.clone();
        loop {
            if let Some(entries) = state.inputs_path_table.get(&p) {
                return entries.clone();
            }
            if p.is_empty() {
                break;
            }
            p = p.get_parent_path();
        }

        Vec::new()
    }

    /// Add an input scene at the end (weakest opinion).
    pub fn add_input_scene(
        &self,
        input_scene: HdSceneIndexHandle,
        active_input_scene_root: SdfPath,
    ) {
        let pos = self.state.lock().inputs.len();
        self.insert_input_scene(pos, input_scene, active_input_scene_root);
    }

    /// Insert an input scene at a specific position.
    ///
    /// Takes `&self` -- uses interior mutability. Locks state mutex briefly
    /// to insert, then releases before sending observer notices.
    pub fn insert_input_scene(
        &self,
        pos: usize,
        input_scene: HdSceneIndexHandle,
        active_input_scene_root: SdfPath,
    ) {
        // Register observer BEFORE locking state (cascades through SI chain)
        let observer_handle = self.register_observer_on_input(&input_scene);

        {
            let mut state = self.state.lock();
            let pos = pos.min(state.inputs.len());
            let entry = InputEntry {
                scene_index: input_scene.clone(),
                scene_root: active_input_scene_root.clone(),
                observer_handle,
            };
            state.inputs.insert(pos, entry);
            Self::rebuild_inputs_path_table(&mut state);
        }
        // State lock released

        if self.base.is_observed() {
            let input_scenes = vec![InputScene::new(
                input_scene.clone(),
                active_input_scene_root.clone(),
            )];
            let mut added_entries = Vec::new();
            // Both of these lock state internally as needed
            self.add_strict_prefixes_of_scene_roots(&input_scenes, &mut added_entries);
            self.collect_added_entries_recursive(
                &input_scene,
                &active_input_scene_root,
                &mut added_entries,
            );

            if !added_entries.is_empty() {
                self.send_prims_added_self(&added_entries);
            }
        }
    }

    /// G11: Insert multiple input scenes with pos support.
    pub fn insert_input_scenes(&self, input_scenes: &[InputScene]) {
        if input_scenes.is_empty() {
            return;
        }

        // Register observers BEFORE locking state (cascades through SI chain)
        let observer_handles: Vec<Option<HdSceneIndexObserverHandle>> = input_scenes
            .iter()
            .map(|is| self.register_observer_on_input(&is.scene))
            .collect();

        {
            let mut state = self.state.lock();
            for (input_scene, observer_handle) in input_scenes.iter().zip(observer_handles) {
                let pos = input_scene.pos.min(state.inputs.len());
                let entry = InputEntry {
                    scene_index: input_scene.scene.clone(),
                    scene_root: input_scene.active_input_scene_root.clone(),
                    observer_handle,
                };
                state.inputs.insert(pos, entry);
            }
            // G16: Rebuild after all insertions
            Self::rebuild_inputs_path_table(&mut state);
        }
        // State lock released

        if !self.base.is_observed() {
            return;
        }

        // G16: Collect strict prefix entries (must NOT hold state lock)
        let mut added_entries = Vec::new();
        self.add_strict_prefixes_of_scene_roots(input_scenes, &mut added_entries);

        // G17: Collect added entries for each new input
        for input_scene in input_scenes {
            self.collect_added_entries_recursive(
                &input_scene.scene,
                &input_scene.active_input_scene_root,
                &mut added_entries,
            );
        }

        if !added_entries.is_empty() {
            self.send_prims_added_self(&added_entries);
        }
    }

    /// Collect PrimsAdded entries by traversing an input scene recursively.
    fn collect_added_entries_recursive(
        &self,
        input_scene: &HdSceneIndexHandle,
        path: &SdfPath,
        entries: &mut Vec<AddedPrimEntry>,
    ) {
        let resolved_type = self.compose_prim_from_inputs(path).prim_type;
        entries.push(AddedPrimEntry::new(path.clone(), resolved_type));

        let children = si_ref(&input_scene).get_child_prim_paths(path);

        for child in children {
            self.collect_added_entries_recursive(input_scene, &child, entries);
        }
    }

    /// Remove an input scene.
    pub fn remove_input_scene(&self, scene_index: &HdSceneIndexHandle) {
        self.remove_input_scenes(&[scene_index.clone()]);
    }

    /// Remove multiple input scenes.
    pub fn remove_input_scenes(&self, scene_indices: &[HdSceneIndexHandle]) {
        if scene_indices.is_empty() {
            return;
        }

        // Partition inputs: keep non-matching, collect removed
        let scene_set: HashSet<*const _> = scene_indices
            .iter()
            .map(|s| Arc::as_ptr(s) as *const _)
            .collect();

        let removed_inputs = {
            let mut state = self.state.lock();
            let mut removed_inputs = Vec::new();
            let mut kept = Vec::new();
            for input in state.inputs.drain(..) {
                if scene_set.contains(&(Arc::as_ptr(&input.scene_index) as *const _)) {
                    removed_inputs.push(input);
                } else {
                    kept.push(input);
                }
            }
            state.inputs = kept;
            // G16: Rebuild path table
            Self::rebuild_inputs_path_table(&mut state);
            removed_inputs
        };
        // State lock released -- unregister observers outside the lock
        for input in &removed_inputs {
            Self::unregister_observer_from_input(&input.scene_index, &input.observer_handle);
        }

        if !self.base.is_observed() || removed_inputs.is_empty() {
            return;
        }

        // Check which prims survive
        let mut removed_entries = Vec::new();
        let mut resync_entries = Vec::new();
        let mut visited = HashSet::new();

        for removed_input in &removed_inputs {
            let mut queue = vec![removed_input.scene_root.clone()];
            while let Some(path) = queue.pop() {
                if !visited.insert(path.clone()) {
                    continue;
                }

                let prim = self.compose_prim_from_inputs(&path);
                if prim.is_defined() {
                    resync_entries.push(AddedPrimEntry::new(path.clone(), prim.prim_type));
                    {
                        let scene_lock = removed_input.scene_index.read();
                        for child in scene_lock.get_child_prim_paths(&path) {
                            queue.push(child);
                        }
                    }
                } else {
                    removed_entries.push(RemovedPrimEntry::new(path));
                }
            }
        }

        if !removed_entries.is_empty() {
            self.send_prims_removed_self(&removed_entries);
        }
        if !resync_entries.is_empty() {
            self.send_prims_added_self(&resync_entries);
        }
    }

    /// Get all input scenes.
    pub fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        self.state
            .lock()
            .inputs
            .iter()
            .map(|input| input.scene_index.clone())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Observer notification handlers
    // -----------------------------------------------------------------------

    fn handle_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.is_observed() {
            return;
        }

        if self.state.lock().inputs.len() < 2 {
            self.send_prims_added_self(entries);
            return;
        }

        // G12: Use get_input_entries_by_path for type resolution
        let mut filtered: Option<Vec<AddedPrimEntry>> = None;

        // Snapshot input count once (lock released before resolve_prim_type which re-locks)
        for (i, entry) in entries.iter().enumerate() {
            let resolved_type = self.resolve_prim_type(&entry.prim_path, sender, &entry.prim_type);

            if resolved_type != entry.prim_type {
                if filtered.is_none() {
                    let mut f = Vec::with_capacity(entries.len());
                    f.extend_from_slice(&entries[..i]);
                    filtered = Some(f);
                }
                filtered
                    .as_mut()
                    .unwrap()
                    .push(AddedPrimEntry::new(entry.prim_path.clone(), resolved_type));
            } else if let Some(ref mut f) = filtered {
                f.push(entry.clone());
            }
        }

        match filtered {
            Some(ref f) => self.send_prims_added_self(f),
            None => self.send_prims_added_self(entries),
        }
    }

    /// G12: Resolve prim type using path-based input lookup.
    fn resolve_prim_type(
        &self,
        prim_path: &SdfPath,
        sender: &dyn HdSceneIndexBase,
        sender_type: &TfToken,
    ) -> TfToken {
        let entries_snapshot = {
            let state = self.state.lock();
            Self::get_input_entries_by_path(&state, prim_path)
        };
        for input in entries_snapshot {
            if !prim_path.has_prefix(&input.scene_root) {
                continue;
            }

            let is_sender = {
                let input_lock = input.scene_index.read();
                std::ptr::eq(
                    &*input_lock as &dyn HdSceneIndexBase as *const dyn HdSceneIndexBase
                        as *const u8,
                    sender as *const dyn HdSceneIndexBase as *const u8,
                )
            };

            let prim_type = if is_sender {
                sender_type.clone()
            } else {
                si_ref(&input.scene_index).get_prim(prim_path).prim_type
            };

            if !prim_type.is_empty() {
                return prim_type;
            }
        }

        TfToken::empty()
    }

    /// Handle PrimsRemoved from an input.
    ///
    /// C++ parity: if a prim is removed from one input but still exists in
    /// another, resync it (and all its merged descendants) via PrimsAdded.
    /// Traversal uses the MERGED index (self), not the removed input.
    fn handle_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if !self.base.is_observed() {
            return;
        }

        if self.state.lock().inputs.len() < 2 {
            self.send_prims_removed_self(entries);
            return;
        }

        let mut resync_entries = Vec::new();

        for entry in entries {
            let prim = self.compose_prim_from_inputs(&entry.prim_path);
            if !prim.is_defined() {
                continue;
            }

            resync_entries.push(AddedPrimEntry::new(entry.prim_path.clone(), prim.prim_type));

            // C++ traverses children via HdSceneIndexPrimView(self, childPath),
            // i.e. through the MERGED index. Collect all descendants.
            let children = self.merge_child_paths(&entry.prim_path);
            if !children.is_empty() {
                let mut stack: Vec<SdfPath> = children;
                while let Some(desc_path) = stack.pop() {
                    let desc_prim = self.compose_prim_from_inputs(&desc_path);
                    resync_entries
                        .push(AddedPrimEntry::new(desc_path.clone(), desc_prim.prim_type));
                    // Continue traversal through merged children
                    stack.extend(self.merge_child_paths(&desc_path));
                }
            }
        }

        self.send_prims_removed_self(entries);
        if !resync_entries.is_empty() {
            self.send_prims_added_self(&resync_entries);
        }
    }

    fn handle_prims_dirtied(&self, entries: &[DirtiedPrimEntry]) {
        if !self.base.is_observed() {
            return;
        }
        self.send_prims_dirtied_self(entries);
    }

    // -----------------------------------------------------------------------
    // Self-sender helpers
    // -----------------------------------------------------------------------

    fn send_prims_added_self(&self, entries: &[AddedPrimEntry]) {
        let sender = MergingSender;
        self.base.send_prims_added(&sender, entries);
    }

    fn send_prims_removed_self(&self, entries: &[RemovedPrimEntry]) {
        let sender = MergingSender;
        self.base.send_prims_removed(&sender, entries);
    }

    fn send_prims_dirtied_self(&self, entries: &[DirtiedPrimEntry]) {
        let sender = MergingSender;
        self.base.send_prims_dirtied(&sender, entries);
    }

    // -----------------------------------------------------------------------
    // Core queries (G12, G18, G19)
    // -----------------------------------------------------------------------

    /// Compose prim from all input scenes.
    ///
    /// G12: Uses get_input_entries_by_path for O(log N) lookup.
    /// G18: Returns empty container for path-table ancestors.
    fn compose_prim_from_inputs(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // Snapshot the inputs and path table presence while holding the lock,
        // then release before calling get_prim on inputs (which may re-enter).
        let (inputs_snapshot, path_table_has_path) = {
            let state = self.state.lock();
            if state.inputs.is_empty() {
                return HdSceneIndexPrim::empty();
            }
            if state.inputs.len() == 1 {
                // C++ single-input fast path: no has_prefix check, just forward directly
                let scene = state.inputs[0].scene_index.clone();
                drop(state);
                return si_ref(&scene).get_prim(prim_path);
            }
            let inputs_snapshot = Self::get_input_entries_by_path(&state, prim_path);
            let path_table_has_path = state.inputs_path_table.contains_key(prim_path);
            (inputs_snapshot, path_table_has_path)
        };
        // State lock released

        let mut result_prim_type = TfToken::default();
        let mut contributing_ds: Vec<HdContainerDataSourceHandle> = Vec::new();

        for input in inputs_snapshot {
            if !prim_path.has_prefix(&input.scene_root) {
                continue;
            }

            {
                let input_lock = input.scene_index.read();
                let prim = input_lock.get_prim(prim_path);

                if result_prim_type.is_empty() && !prim.prim_type.is_empty() {
                    result_prim_type = prim.prim_type;
                }

                if let Some(ds) = prim.data_source {
                    contributing_ds.push(ds);
                }
            }
        }

        let result_ds: Option<HdContainerDataSourceHandle> = match contributing_ds.len() {
            0 => {
                // G18: If path is ancestor of any scene root (in path table),
                // return empty container so the prim exists for traversal.
                if path_table_has_path {
                    Some(HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle)
                } else {
                    None
                }
            }
            1 => Some(contributing_ds.into_iter().next().unwrap()),
            _ => {
                Some(HdOverlayContainerDataSource::new(contributing_ds)
                    as HdContainerDataSourceHandle)
            }
        };

        if result_ds.is_some() || !result_prim_type.is_empty() {
            HdSceneIndexPrim {
                prim_type: result_prim_type,
                data_source: result_ds,
            }
        } else {
            HdSceneIndexPrim::empty()
        }
    }

    /// Merge child paths from all inputs.
    ///
    /// G19: Also includes intermediate children implied by nested inputs
    /// at deeper sceneRoot paths (from the path table).
    fn merge_child_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        // Snapshot inputs and path table intermediate children while holding the lock.
        let (inputs_snapshot, intermediate_children) = {
            let state = self.state.lock();
            let inputs_snapshot = Self::get_input_entries_by_path(&state, prim_path);
            // G19: Collect intermediate children from path table
            let mut intermediate_children = Vec::new();
            if !state.inputs_path_table.is_empty() {
                for (path, _) in state.inputs_path_table.range(prim_path.clone()..) {
                    if !path.has_prefix(prim_path) {
                        break;
                    }
                    if path == prim_path {
                        continue;
                    }
                    if path.get_parent_path() == *prim_path {
                        intermediate_children.push(path.clone());
                    }
                }
            }
            (inputs_snapshot, intermediate_children)
        };
        // State lock released

        let mut all_children = HashSet::new();

        for input in inputs_snapshot {
            if !prim_path.has_prefix(&input.scene_root) {
                continue;
            }

            {
                let input_lock = input.scene_index.read();
                let children = input_lock.get_child_prim_paths(prim_path);
                all_children.extend(children);
            }
        }

        // G19: Insert intermediate children implied by path table entries
        all_children.extend(intermediate_children);

        let mut result: Vec<_> = all_children.into_iter().collect();
        result.sort();
        result
    }
}

/// Minimal sender for self-originated notifications.
pub struct MergingSender;

impl HdSceneIndexBase for MergingSender {
    fn get_prim(&self, _prim_path: &SdfPath) -> HdSceneIndexPrim {
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, _prim_path: &SdfPath) -> SdfPathVector {
        Vec::new()
    }

    fn add_observer(&self, _observer: HdSceneIndexObserverHandle) {}
    fn remove_observer(&self, _observer: &HdSceneIndexObserverHandle) {}
}

impl HdSceneIndexBase for HdMergingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        self.compose_prim_from_inputs(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        self.merge_child_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn set_display_name(&mut self, name: String) {
        self.base.set_display_name(name);
    }

    fn add_tag(&mut self, tag: TfToken) {
        self.base.add_tag(tag);
    }

    fn remove_tag(&mut self, tag: &TfToken) {
        self.base.remove_tag(tag);
    }

    fn has_tag(&self, tag: &TfToken) -> bool {
        self.base.has_tag(tag)
    }

    fn get_tags(&self) -> TfTokenVector {
        self.base.get_tags()
    }

    fn get_display_name(&self) -> String {
        let name = self.base.get_display_name();
        if name.is_empty() {
            "HdMergingSceneIndex".to_string()
        } else {
            name.to_string()
        }
    }
}

impl HdFilteringSceneIndexBase for HdMergingSceneIndex {
    fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        self.state
            .lock()
            .inputs
            .iter()
            .map(|input| input.scene_index.clone())
            .collect()
    }
}

impl Default for HdMergingSceneIndex {
    fn default() -> Self {
        Self {
            base: HdSceneIndexBaseImpl::new(),
            state: Mutex::new(MergingState {
                inputs: Vec::new(),
                inputs_path_table: BTreeMap::new(),
            }),
            self_ref: Mutex::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merging_scene_creation() {
        let scene = HdMergingSceneIndex::new();
        let scene_lock = scene.read();
        assert_eq!(scene_lock.get_input_scenes().len(), 0);
    }

    #[test]
    fn test_merging_empty_returns_empty_prim() {
        let scene = HdMergingSceneIndex::new();
        let scene_lock = scene.read();
        let prim = scene_lock.get_prim(&SdfPath::from_string("/World").unwrap());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_input_scene_with_pos() {
        let _entry = InputScene::with_pos(
            Arc::new(RwLock::new(MergingSender)),
            SdfPath::absolute_root(),
            0,
        );
        assert_eq!(_entry.pos, 0);
    }

    #[test]
    fn test_input_scene_default_pos() {
        let entry = InputScene::new(
            Arc::new(RwLock::new(MergingSender)),
            SdfPath::absolute_root(),
        );
        assert_eq!(entry.pos, usize::MAX);
    }
}
