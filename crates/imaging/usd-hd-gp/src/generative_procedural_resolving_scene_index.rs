
//! Scene index that resolves generative procedural prims.
//!
//! Port of pxr/imaging/hdGp/generativeProceduralResolvingSceneIndex.h/cpp

use super::generative_procedural::{
    AsyncState, ChildPrimTypeMap, DependencyMap, HdGpGenerativeProcedural, tokens,
};
use super::generative_procedural_plugin_registry::HdGpGenerativeProceduralPluginRegistry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::{HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet};
use usd_hd::scene_index::{
    HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase,
    SdfPathVector, si_ref,
    observer::{AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry},
};
use usd_hd::schema::HdPrimvarsSchema;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// ---------------------------------------------------------------------------
// Cooking state constants (matches C++ _ProcEntry::State enum)
// ---------------------------------------------------------------------------
const STATE_UNCOOKED: u8 = 0;
const STATE_DEPENDENCIES_COOKING: u8 = 1;
const STATE_DEPENDENCIES_COOKED: u8 = 2;
const STATE_COOKING: u8 = 3;
const STATE_COOKED: u8 = 4;

// ---------------------------------------------------------------------------
// child_names_dependency_key helper
// ---------------------------------------------------------------------------

/// Returns a locator set for `__childNames` dependency key.
/// Cached as static to avoid per-call allocation in hot loops.
fn child_names_dependency_key_set() -> &'static HdDataSourceLocatorSet {
    static KEY_SET: once_cell::sync::Lazy<HdDataSourceLocatorSet> =
        once_cell::sync::Lazy::new(|| {
            HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(TfToken::new(
                "__childNames",
            )))
        });
    &KEY_SET
}

// ---------------------------------------------------------------------------
// Internal data structures
// ---------------------------------------------------------------------------

/// Per-procedural prim entry tracking cooking state and generated children.
/// Matches C++ `_ProcEntry`.
struct ProcEntry {
    /// Cooking state (atomic for lock-free state machine transitions).
    state: AtomicU8,
    /// Procedural type name (from hdGp:proceduralType primvar).
    type_name: TfToken,
    /// The procedural implementation (may be None if type not found in registry).
    proc: Option<Box<dyn HdGpGenerativeProcedural>>,
    /// Current full set of child prim paths -> types.
    child_types: ChildPrimTypeMap,
    /// Declared input dependencies (path -> locator set).
    dependencies: DependencyMap,
    /// Hierarchy map: parent_path -> set of immediate children.
    /// Tracks intermediate "namespace" prims inserted for deep hierarchies.
    child_hierarchy: HashMap<SdfPath, HashSet<SdfPath>>,
    /// Serializes cook operations.
    cook_mutex: Mutex<()>,
}

impl ProcEntry {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_UNCOOKED),
            type_name: TfToken::empty(),
            proc: None,
            child_types: ChildPrimTypeMap::new(),
            dependencies: DependencyMap::new(),
            child_hierarchy: HashMap::new(),
            cook_mutex: Mutex::new(()),
        }
    }
}

/// Accumulated notices produced during procedural cooking.
/// Matches C++ `_Notices`.
#[derive(Default)]
struct Notices {
    added: Vec<AddedPrimEntry>,
    removed: Vec<RemovedPrimEntry>,
    dirtied: Vec<DirtiedPrimEntry>,
}

// ---------------------------------------------------------------------------
// combine_path_arrays helper
// ---------------------------------------------------------------------------

/// Appends unique paths from `set` into `vec`, avoiding duplicates.
/// Matches C++ `_CombinePathArrays`.
fn combine_path_arrays(set: &HashSet<SdfPath>, vec: &mut SdfPathVector) {
    if vec.is_empty() {
        vec.extend(set.iter().cloned());
        return;
    }
    // Collect existing to avoid borrow conflict between immutable read and push
    let existing: HashSet<SdfPath> = vec.iter().cloned().collect();
    let to_add: Vec<SdfPath> = set
        .iter()
        .filter(|p| !existing.contains(*p))
        .cloned()
        .collect();
    vec.extend(to_add);
}

// ---------------------------------------------------------------------------
// ancestors_between helper
// ---------------------------------------------------------------------------

/// Returns strict ancestors of `path` that are descendants of `root` (exclusive).
/// I.e., paths P where root < P < path — used to build intermediate hierarchy.
fn ancestors_between(path: &SdfPath, root: &SdfPath) -> Vec<SdfPath> {
    let mut result = Vec::new();
    for ancestor in path.get_ancestors_range() {
        if ancestor == *root {
            break;
        }
        // Skip the path itself (get_ancestors_range starts from parent)
        result.push(ancestor);
    }
    result
}

// ---------------------------------------------------------------------------
// HdGpGenerativeProceduralResolvingSceneIndex
// ---------------------------------------------------------------------------

/// Scene index that evaluates generative procedural prims.
///
/// Identifies prims of the configured type (default: "hydraGenerativeProcedural"),
/// evaluates them via registered plugins, and exposes their generated children.
/// Re-types processed procedurals to "resolvedHydraGenerativeProcedural" to
/// prevent double-evaluation when chained.
///
/// Port of C++ `HdGpGenerativeProceduralResolvingSceneIndex`.
pub struct HdGpGenerativeProceduralResolvingSceneIndex {
    /// Single-input filtering base (manages observers + input scene).
    base: HdSingleInputFilteringSceneIndexBase,
    /// Prim type to identify as a procedural (default: "hydraGenerativeProcedural").
    target_prim_type_name: TfToken,
    /// Per-procedural entry map (path -> ProcEntry).
    procedurals: HashMap<SdfPath, ProcEntry>,
    /// Reverse dependency map: dependency_path -> set of procedural paths that depend on it.
    dependencies: HashMap<SdfPath, HashSet<SdfPath>>,
    /// Generated prim map: generated_prim_path -> Option<responsible_proc_path>.
    /// None means the prim was generated but the responsible proc was removed.
    generated_prims: HashMap<SdfPath, Option<SdfPath>>,
    /// Whether to attempt async evaluation (set via SystemMessage asyncAllow).
    attempt_async: bool,
    /// Procedural paths with active async operations (for SystemMessage asyncPoll).
    active_async_procedurals: HashSet<SdfPath>,
}

/// Handle type for the resolving scene index.
pub type HdGpGenerativeProceduralResolvingSceneIndexHandle =
    Arc<RwLock<HdGpGenerativeProceduralResolvingSceneIndex>>;

impl HdGpGenerativeProceduralResolvingSceneIndex {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Create with default target type ("hydraGenerativeProcedural").
    pub fn new(
        input_scene: HdSceneIndexHandle,
    ) -> HdGpGenerativeProceduralResolvingSceneIndexHandle {
        Self::new_with_type(input_scene, tokens::GENERATIVE_PROCEDURAL.clone())
    }

    /// Create with a custom target procedural type.
    pub fn new_with_type(
        input_scene: HdSceneIndexHandle,
        target_prim_type_name: TfToken,
    ) -> HdGpGenerativeProceduralResolvingSceneIndexHandle {
        // Ensure registry is initialized (matches C++ GetInstance() call in ctor).
        let _ = HdGpGenerativeProceduralPluginRegistry::get_instance();

        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene)),
            target_prim_type_name,
            procedurals: HashMap::new(),
            dependencies: HashMap::new(),
            generated_prims: HashMap::new(),
            attempt_async: false,
            active_async_procedurals: HashSet::new(),
        }))
    }

    // -----------------------------------------------------------------------
    // Internal: construct a procedural via the plugin registry
    // -----------------------------------------------------------------------

    fn construct_procedural_impl(
        type_name: &TfToken,
        prim_path: &SdfPath,
    ) -> Option<Box<dyn HdGpGenerativeProcedural>> {
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let reg = registry.read();
        reg.construct_procedural(type_name, prim_path)
    }

    // -----------------------------------------------------------------------
    // Internal: read procedural type from prim primvars
    // -----------------------------------------------------------------------

    /// Read `hdGp:proceduralType` primvar from a prim's data source.
    /// Matches C++ `_GetProceduralType` in the filtering scene index.
    fn read_proc_type_from_prim(prim: &HdSceneIndexPrim) -> TfToken {
        if let Some(ref data_source) = prim.data_source {
            let primvars = HdPrimvarsSchema::get_from_parent(data_source);
            let primvar = primvars.get_primvar_schema(&tokens::PROCEDURAL_TYPE);
            if let Some(value_ds) = primvar.get_primvar_value() {
                if let Some(sampled) = value_ds.as_sampled() {
                    let value = sampled.get_value(0.0);
                    if let Some(token) = value.get::<TfToken>() {
                        return token.clone();
                    }
                }
            }
        }
        TfToken::empty()
    }

    // -----------------------------------------------------------------------
    // Internal: _UpdateProceduralDependencies
    // -----------------------------------------------------------------------

    /// Update (or create) the ProcEntry for the given procedural path.
    /// Returns true if dependencies were successfully cooked.
    ///
    /// Matches C++ `_UpdateProceduralDependencies`.
    fn update_procedural_dependencies(
        &mut self,
        procedural_prim_path: &SdfPath,
        output_notices: &mut Notices,
    ) -> bool {
        // Read prim from input scene
        let prim_type = {
            if let Some(input) = self.base.get_input_scene() {
                si_ref(&input).get_prim(procedural_prim_path).prim_type
            } else {
                return false;
            }
        };

        if prim_type != self.target_prim_type_name {
            // Was a procedural, now isn't — remove it
            self.remove_procedural(procedural_prim_path, Some(output_notices));
            return false;
        }

        // Check state: skip if already at least dependencies-cooked
        {
            if let Some(entry) = self.procedurals.get(procedural_prim_path) {
                if entry.state.load(Ordering::Acquire) >= STATE_DEPENDENCIES_COOKED {
                    return true;
                }
            }
        }

        // Ensure entry exists
        if !self.procedurals.contains_key(procedural_prim_path) {
            self.procedurals
                .insert(procedural_prim_path.clone(), ProcEntry::new());
        }

        // Read the procedural type and existing type_name
        let (old_type_name, has_proc) = {
            let entry = self.procedurals.get(procedural_prim_path).unwrap();
            (entry.type_name.clone(), entry.proc.is_some())
        };

        // Read hdGp:proceduralType primvar from the full prim
        let proc_type = {
            if let Some(input) = self.base.get_input_scene() {
                let full_prim = si_ref(&input).get_prim(procedural_prim_path);
                Self::read_proc_type_from_prim(&full_prim)
            } else {
                TfToken::empty()
            }
        };

        // Construct new proc if needed
        let new_proc: Option<Box<dyn HdGpGenerativeProcedural>> =
            if !has_proc || proc_type != old_type_name {
                Self::construct_procedural_impl(&proc_type, procedural_prim_path)
            } else {
                None
            };

        let needs_new_proc = !has_proc || proc_type != old_type_name;

        // Call update_dependencies on the proc.
        // Take the proc out temporarily to get &mut access, then put it back.
        let (new_dependencies, ready_proc) = {
            let input_clone = self.base.get_input_scene().cloned();

            // Get a mutable proc — either the newly constructed one or take existing out
            let mut working_proc: Option<Box<dyn HdGpGenerativeProcedural>> = if needs_new_proc {
                new_proc
            } else {
                self.procedurals
                    .get_mut(procedural_prim_path)
                    .and_then(|e| e.proc.take())
            };

            let deps = if let (Some(proc), Some(input)) = (working_proc.as_deref_mut(), input_clone)
            {
                proc.update_dependencies(&*input.read())
            } else {
                DependencyMap::new()
            };

            // Put the proc back if we took it from the entry (not a new proc)
            if !needs_new_proc {
                if let Some(entry) = self.procedurals.get_mut(procedural_prim_path) {
                    entry.proc = working_proc;
                }
                (deps, None)
            } else {
                // Pass the new proc forward for the CAS block to install
                (deps, working_proc)
            }
        };

        // CAS: StateUncooked -> StateDependenciesCooking (only one thread wins).
        // Collect diffs and update entry in a scoped borrow, then update reverse dep map
        // in a separate step to avoid double-borrow on self.
        let (deps_to_add, deps_to_remove) = {
            let entry = self.procedurals.get_mut(procedural_prim_path).unwrap();
            let result = entry.state.compare_exchange(
                STATE_UNCOOKED,
                STATE_DEPENDENCIES_COOKING,
                Ordering::AcqRel,
                Ordering::Acquire,
            );

            if result.is_ok() {
                // We won the CAS — install proc if new
                if needs_new_proc {
                    entry.proc = ready_proc;
                    entry.type_name = proc_type.clone();
                }

                // Compute dependency diff before replacing entry.dependencies
                let to_remove: Vec<SdfPath> = entry
                    .dependencies
                    .keys()
                    .filter(|p| !new_dependencies.contains_key(*p))
                    .cloned()
                    .collect();

                let to_add: Vec<SdfPath> = new_dependencies
                    .keys()
                    .filter(|p| !entry.dependencies.contains_key(*p))
                    .cloned()
                    .collect();

                entry.dependencies = new_dependencies;
                entry
                    .state
                    .store(STATE_DEPENDENCIES_COOKED, Ordering::Release);
                (to_add, to_remove)
            } else {
                // CAS failed: another thread already cooking — nothing to diff
                (Vec::new(), Vec::new())
            }
        }; // entry borrow ends here

        // Update reverse dependency map now that entry borrow is released.
        let proc_path = procedural_prim_path.clone();
        for dep_path in deps_to_add {
            self.dependencies
                .entry(dep_path)
                .or_default()
                .insert(proc_path.clone());
        }
        for dep_path in deps_to_remove {
            if let Some(dep_set) = self.dependencies.get_mut(&dep_path) {
                dep_set.remove(&proc_path);
                if dep_set.is_empty() {
                    self.dependencies.remove(&dep_path);
                }
            }
        }

        // Call AsyncBegin on the proc (matches C++ cpp:697-718).
        // If the proc supports async and _attemptAsync is set, register it.
        let should_register_async = {
            if let Some(entry) = self.procedurals.get_mut(procedural_prim_path) {
                if let Some(ref mut proc) = entry.proc {
                    proc.async_begin(self.attempt_async)
                } else {
                    false
                }
            } else {
                false
            }
        };
        if should_register_async {
            self.active_async_procedurals
                .insert(procedural_prim_path.clone());
        }

        true
    }

    // -----------------------------------------------------------------------
    // Internal: _UpdateProcedural (superseded by cook_procedurals_batch)
    // -----------------------------------------------------------------------
    // NOTE: The C++ _UpdateProcedural() is a single-path cook function called
    // from scene index callbacks. In Rust, cook_procedurals_batch() replaces it
    // because &mut self borrowing prevents the C++ pattern of parallel per-path
    // cooking with shared mutable state. cook_procedurals_batch() calls
    // update_procedural_dependencies() and update_procedural_result() directly.

    /// Cook (or re-cook) a procedural. Superseded by cook_procedurals_batch.
    #[allow(dead_code)]
    fn update_procedural(
        &mut self,
        procedural_prim_path: &SdfPath,
        force_update: bool,
        output_notices: &mut Notices,
        dirtied_dependencies: Option<&DependencyMap>,
    ) {
        // Force resets to uncooked
        if force_update {
            if let Some(entry) = self.procedurals.get_mut(procedural_prim_path) {
                entry.state.store(STATE_UNCOOKED, Ordering::Release);
            }
        }

        // Cook dependencies if not yet done
        {
            let state = self
                .procedurals
                .get(procedural_prim_path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state < STATE_DEPENDENCIES_COOKED {
                if !self.update_procedural_dependencies(procedural_prim_path, output_notices) {
                    return;
                }
            }
        }

        // Check if already cooked
        {
            let state = self
                .procedurals
                .get(procedural_prim_path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state >= STATE_COOKED {
                return;
            }
        }

        // Check proc exists
        let has_proc = self
            .procedurals
            .get(procedural_prim_path)
            .map(|e| e.proc.is_some())
            .unwrap_or(false);

        if !has_proc {
            return;
        }

        // Call proc.update() — need to extract prev_child_types and deps first
        // to avoid borrow conflicts, then re-borrow entry mutably.
        let (prev_child_types, local_deps) = {
            let entry = self.procedurals.get(procedural_prim_path).unwrap();
            let deps = dirtied_dependencies.unwrap_or(&entry.dependencies).clone();
            (entry.child_types.clone(), deps)
        };

        let new_child_types: ChildPrimTypeMap = {
            let entry = self.procedurals.get_mut(procedural_prim_path).unwrap();
            if let Some(ref mut proc) = entry.proc {
                if let Some(input) = self.base.get_input_scene() {
                    let locked = input.read();
                    proc.update(
                        &*locked,
                        &prev_child_types,
                        &local_deps,
                        &mut output_notices.dirtied,
                    )
                } else {
                    ChildPrimTypeMap::new()
                }
            } else {
                ChildPrimTypeMap::new()
            }
        };

        // CAS: StateDependenciesCooked -> StateCooking
        let did_cook = {
            let entry = self.procedurals.get_mut(procedural_prim_path).unwrap();
            entry
                .state
                .compare_exchange(
                    STATE_DEPENDENCIES_COOKED,
                    STATE_COOKING,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        };

        if did_cook {
            // Lock the cook mutex, then drop the guard before calling update_procedural_result
            // to avoid conflicting borrows on self (cook_mutex is in self.procedurals,
            // but update_procedural_result needs &mut self).
            {
                let entry = self.procedurals.get(procedural_prim_path).unwrap();
                let _guard = entry.cook_mutex.lock();
                // guard released here — C++ takes the lock around the result update,
                // but Rust &mut self serializes access anyway.
            }

            let proc_path = procedural_prim_path.clone();
            self.update_procedural_result(&proc_path, new_child_types, output_notices);

            // Mark cooked
            if let Some(e) = self.procedurals.get_mut(procedural_prim_path) {
                e.state.store(STATE_COOKED, Ordering::Release);
            }
        }
        // If CAS failed: another thread is cooking, just let it finish.
    }

    // -----------------------------------------------------------------------
    // Internal: _UpdateProceduralResult
    // -----------------------------------------------------------------------

    /// Diff old vs new child types, update generated_prims map, emit notices.
    /// Matches C++ `_UpdateProceduralResult`.
    fn update_procedural_result(
        &mut self,
        procedural_prim_path: &SdfPath,
        new_child_types: ChildPrimTypeMap,
        output_notices: &mut Notices,
    ) {
        // Snapshot old state to avoid borrow issues
        let (old_child_types, old_child_hierarchy) = {
            let entry = match self.procedurals.get(procedural_prim_path) {
                Some(e) => e,
                None => return,
            };
            (entry.child_types.clone(), entry.child_hierarchy.clone())
        };

        let mut removed_child_prims: HashSet<SdfPath> = HashSet::new();
        let mut generated_prim_set: HashSet<SdfPath> = HashSet::new();
        let mut new_child_hierarchy: HashMap<SdfPath, HashSet<SdfPath>> = HashMap::new();

        if old_child_types.is_empty() {
            // First cook: add all children, build hierarchy from scratch
            for (child_prim_path, child_prim_type) in &new_child_types {
                output_notices.added.push(AddedPrimEntry::new(
                    child_prim_path.clone(),
                    child_prim_type.clone(),
                ));

                if child_prim_path.has_prefix(procedural_prim_path) {
                    for ancestor in ancestors_between(child_prim_path, procedural_prim_path) {
                        new_child_hierarchy
                            .entry(ancestor.get_parent_path())
                            .or_default()
                            .insert(ancestor.clone());
                        generated_prim_set.insert(ancestor);
                    }
                }
            }

            // All hierarchy entries are generated prims
            for p in new_child_hierarchy.keys() {
                generated_prim_set.insert(p.clone());
            }
        } else if old_child_types != new_child_types {
            // Incremental update: compute new hierarchy
            for (child_prim_path, _) in &new_child_types {
                if child_prim_path.has_prefix(procedural_prim_path) {
                    for ancestor in ancestors_between(child_prim_path, procedural_prim_path) {
                        new_child_hierarchy
                            .entry(ancestor.get_parent_path())
                            .or_default()
                            .insert(ancestor.clone());
                    }
                }
            }

            // Add new or changed entries
            for (child_prim_path, child_prim_type) in &new_child_types {
                match old_child_types.get(child_prim_path) {
                    Some(old_type) if old_type == child_prim_type => {
                        // Unchanged — no notification
                    }
                    _ => {
                        output_notices.added.push(AddedPrimEntry::new(
                            child_prim_path.clone(),
                            child_prim_type.clone(),
                        ));
                        generated_prim_set.insert(child_prim_path.clone());
                    }
                }
            }

            // Remove entries not in new cook
            for (old_path, _) in &old_child_types {
                if !new_child_types.contains_key(old_path)
                    && !new_child_hierarchy.contains_key(old_path)
                {
                    output_notices.removed.push(RemovedPrimEntry {
                        prim_path: old_path.clone(),
                    });
                    removed_child_prims.insert(old_path.clone());
                }
            }

            // Handle intermediate hierarchy changes
            if new_child_types.len() != old_child_types.len()
                || new_child_hierarchy != old_child_hierarchy
            {
                // Add new intermediate prims
                for (parent_path, _) in &new_child_hierarchy {
                    if *parent_path == *procedural_prim_path {
                        continue;
                    }

                    let add_as_intermediate = if !old_child_hierarchy.contains_key(parent_path) {
                        if !new_child_types.contains_key(parent_path) {
                            true
                        } else {
                            old_child_types.contains_key(parent_path)
                        }
                    } else {
                        old_child_types.contains_key(parent_path)
                            && !new_child_types.contains_key(parent_path)
                    };

                    if add_as_intermediate {
                        generated_prim_set.insert(parent_path.clone());
                        output_notices
                            .added
                            .push(AddedPrimEntry::new(parent_path.clone(), TfToken::empty()));
                    }
                }

                // Remove gone intermediate prims
                for (parent_path, _) in &old_child_hierarchy {
                    if *parent_path == *procedural_prim_path {
                        continue;
                    }
                    if !new_child_hierarchy.contains_key(parent_path)
                        && !new_child_types.contains_key(parent_path)
                    {
                        removed_child_prims.insert(parent_path.clone());
                        output_notices.removed.push(RemovedPrimEntry {
                            prim_path: parent_path.clone(),
                        });
                    }
                }
            }
        }

        // Update generated_prims map
        for p in &generated_prim_set {
            if p != procedural_prim_path {
                self.generated_prims
                    .insert(p.clone(), Some(procedural_prim_path.clone()));
            }
        }
        for p in &removed_child_prims {
            if let Some(slot) = self.generated_prims.get_mut(p) {
                *slot = None;
            }
        }

        // Update entry
        if let Some(entry) = self.procedurals.get_mut(procedural_prim_path) {
            if !new_child_hierarchy.is_empty() || old_child_types != new_child_types {
                entry.child_hierarchy = new_child_hierarchy;
            }
            entry.child_types = new_child_types;
        }
    }

    // -----------------------------------------------------------------------
    // Internal: _RemoveProcedural
    // -----------------------------------------------------------------------

    /// Remove a procedural entry and clean up its generated children.
    /// Matches C++ `_RemoveProcedural`.
    fn remove_procedural(
        &mut self,
        procedural_prim_path: &SdfPath,
        mut output_notices: Option<&mut Notices>,
    ) {
        // Snapshot entry data and emit notices in a scoped borrow to avoid
        // borrow conflicts when we mutate self.dependencies below.
        let (dep_paths, child_type_paths, hierarchy_paths) = {
            let entry = match self.procedurals.get(procedural_prim_path) {
                Some(e) => e,
                None => return,
            };

            // 0) Emit removal notices for immediate children
            if let Some(notices) = output_notices.as_mut() {
                let proc_depth = procedural_prim_path.get_path_element_count();
                for (child_path, _) in &entry.child_hierarchy {
                    if child_path.get_path_element_count() == proc_depth + 1 {
                        notices.removed.push(RemovedPrimEntry {
                            prim_path: child_path.clone(),
                        });
                    }
                }
            }

            // Snapshot before removal
            let dep_paths: Vec<SdfPath> = entry.dependencies.keys().cloned().collect();
            let child_type_paths: Vec<SdfPath> = entry.child_types.keys().cloned().collect();
            let hierarchy_paths: Vec<SdfPath> = entry.child_hierarchy.keys().cloned().collect();
            (dep_paths, child_type_paths, hierarchy_paths)
        }; // entry borrow released here

        // 1) Remove from reverse dependency map
        for dep_path in &dep_paths {
            if let Some(dep_set) = self.dependencies.get_mut(dep_path) {
                dep_set.remove(procedural_prim_path);
                if dep_set.is_empty() {
                    self.dependencies.remove(dep_path);
                }
            }
        }

        // 2) Clear generated_prims references
        for path in child_type_paths.iter().chain(hierarchy_paths.iter()) {
            if let Some(slot) = self.generated_prims.get_mut(path) {
                *slot = None;
            }
        }

        // 3) Remove the proc entry
        self.procedurals.remove(procedural_prim_path);
    }

    // -----------------------------------------------------------------------
    // Internal: parallel batch cooking (matches C++ WorkParallelForEach)
    // -----------------------------------------------------------------------

    /// Cook multiple procedurals, parallelizing the expensive proc.update() calls.
    /// Sequential dep cooking + parallel proc.update() + sequential result merge.
    /// Matches C++ WorkParallelForEach with threshold=2.
    fn cook_procedurals_batch(&mut self, paths: Vec<SdfPath>, output_notices: &mut Notices) {
        // Phase 1: Sequential dependency cooking for all paths
        for path in &paths {
            if let Some(entry) = self.procedurals.get_mut(path) {
                entry.state.store(STATE_UNCOOKED, Ordering::Release);
            }

            let state = self
                .procedurals
                .get(path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state < STATE_DEPENDENCIES_COOKED {
                self.update_procedural_dependencies(path, output_notices);
            }
        }

        // Phase 2: Extract procs ready to cook
        let mut cook_data: Vec<(
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            DependencyMap,
        )> = Vec::new();

        for path in &paths {
            let state = self
                .procedurals
                .get(path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state < STATE_DEPENDENCIES_COOKED || state >= STATE_COOKED {
                continue;
            }

            let entry = match self.procedurals.get_mut(path) {
                Some(e) if e.proc.is_some() => e,
                _ => continue,
            };

            let deps = entry.dependencies.clone();
            let prev = entry.child_types.clone();
            let proc = entry.proc.take().expect("checked above");
            cook_data.push((path.clone(), proc, prev, deps));
        }

        if cook_data.is_empty() {
            return;
        }

        // Phase 3: Run proc.update() — parallel when >=2 (threshold matches C++)
        let input_scene = self.base.get_input_scene().cloned();

        type CookResult = (
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            Vec<DirtiedPrimEntry>,
        );

        let run_update = |data: (
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            DependencyMap,
        )|
         -> CookResult {
            let (path, mut proc, prev, deps) = data;
            let mut dirtied = Vec::new();
            let child_types = if let Some(ref input) = input_scene {
                proc.update(&*input.read(), &prev, &deps, &mut dirtied)
            } else {
                ChildPrimTypeMap::new()
            };
            (path, proc, child_types, dirtied)
        };

        let results: Vec<CookResult> = if cook_data.len() >= 2 {
            use rayon::prelude::*;
            cook_data.into_par_iter().map(run_update).collect()
        } else {
            cook_data.into_iter().map(run_update).collect()
        };

        // Phase 4: Sequential result merge
        for (path, proc, child_types, dirtied) in results {
            // Put proc back
            if let Some(entry) = self.procedurals.get_mut(&path) {
                entry.proc = Some(proc);

                let did_cook = entry
                    .state
                    .compare_exchange(
                        STATE_DEPENDENCIES_COOKED,
                        STATE_COOKING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok();

                if did_cook {
                    output_notices.dirtied.extend(dirtied);
                    let p = path.clone();
                    self.update_procedural_result(&p, child_types, output_notices);
                    if let Some(e) = self.procedurals.get_mut(&path) {
                        e.state.store(STATE_COOKED, Ordering::Release);
                    }
                }
            }
        }
    }

    /// Like cook_procedurals_batch but each proc gets its own dirtied_dependencies.
    /// Used by on_prims_dirtied. Matches C++ WorkParallelForEach in _PrimsDirtied.
    fn cook_procedurals_batch_with_deps(
        &mut self,
        paths_deps: Vec<(SdfPath, DependencyMap)>,
        output_notices: &mut Notices,
    ) {
        // Phase 1: Sequential dependency cooking
        for (path, _) in &paths_deps {
            if let Some(entry) = self.procedurals.get_mut(path) {
                entry.state.store(STATE_UNCOOKED, Ordering::Release);
            }

            let state = self
                .procedurals
                .get(path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state < STATE_DEPENDENCIES_COOKED {
                self.update_procedural_dependencies(path, output_notices);
            }
        }

        // Phase 2: Extract procs
        let mut cook_data: Vec<(
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            DependencyMap,
        )> = Vec::new();

        for (path, dirty_deps) in paths_deps {
            let state = self
                .procedurals
                .get(&path)
                .map(|e| e.state.load(Ordering::Acquire))
                .unwrap_or(STATE_UNCOOKED);

            if state < STATE_DEPENDENCIES_COOKED || state >= STATE_COOKED {
                continue;
            }

            let entry = match self.procedurals.get_mut(&path) {
                Some(e) if e.proc.is_some() => e,
                _ => continue,
            };

            let prev = entry.child_types.clone();
            let proc = entry.proc.take().expect("checked above");
            cook_data.push((path, proc, prev, dirty_deps));
        }

        if cook_data.is_empty() {
            return;
        }

        // Phase 3: Parallel proc.update()
        let input_scene = self.base.get_input_scene().cloned();

        type CookResult = (
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            Vec<DirtiedPrimEntry>,
        );

        let run_update = |data: (
            SdfPath,
            Box<dyn HdGpGenerativeProcedural>,
            ChildPrimTypeMap,
            DependencyMap,
        )|
         -> CookResult {
            let (path, mut proc, prev, deps) = data;
            let mut dirtied = Vec::new();
            let child_types = if let Some(ref input) = input_scene {
                proc.update(&*input.read(), &prev, &deps, &mut dirtied)
            } else {
                ChildPrimTypeMap::new()
            };
            (path, proc, child_types, dirtied)
        };

        let results: Vec<CookResult> = if cook_data.len() >= 2 {
            use rayon::prelude::*;
            cook_data.into_par_iter().map(run_update).collect()
        } else {
            cook_data.into_iter().map(run_update).collect()
        };

        // Phase 4: Sequential merge
        for (path, proc, child_types, dirtied) in results {
            if let Some(entry) = self.procedurals.get_mut(&path) {
                entry.proc = Some(proc);
                let did_cook = entry
                    .state
                    .compare_exchange(
                        STATE_DEPENDENCIES_COOKED,
                        STATE_COOKING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok();

                if did_cook {
                    output_notices.dirtied.extend(dirtied);
                    let p = path.clone();
                    self.update_procedural_result(&p, child_types, output_notices);
                    if let Some(e) = self.procedurals.get_mut(&path) {
                        e.state.store(STATE_COOKED, Ordering::Release);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal: _GarbageCollect
    // -----------------------------------------------------------------------

    /// Remove generated_prims entries where the responsible proc is None.
    /// Matches C++ `_GarbageCollect`.
    #[allow(dead_code)]
    fn garbage_collect(&mut self) {
        self.generated_prims
            .retain(|_, responsible| responsible.is_some());
    }

    // -----------------------------------------------------------------------
    // SystemMessage handler
    // -----------------------------------------------------------------------

    /// Handle system messages (async polling).
    /// Matches C++ `_SystemMessage`.
    pub fn system_message(
        &mut self,
        message_type: &TfToken,
        _args: Option<&HdDataSourceBaseHandle>,
    ) {
        static ASYNC_ALLOW: once_cell::sync::Lazy<TfToken> =
            once_cell::sync::Lazy::new(|| TfToken::new("asyncAllow"));
        static ASYNC_POLL: once_cell::sync::Lazy<TfToken> =
            once_cell::sync::Lazy::new(|| TfToken::new("asyncPoll"));

        if !self.attempt_async {
            if *message_type == *ASYNC_ALLOW {
                self.attempt_async = true;
            }
            return;
        }

        if *message_type != *ASYNC_POLL {
            return;
        }

        let mut notices = Notices::default();
        let mut prim_types = ChildPrimTypeMap::new();
        let mut removed_entries: Vec<SdfPath> = Vec::new();

        // Collect active paths first to avoid borrow conflicts
        let active_paths: Vec<SdfPath> = self.active_async_procedurals.iter().cloned().collect();

        for proc_path in &active_paths {
            let has_proc = self
                .procedurals
                .get(proc_path)
                .map(|e| e.proc.is_some())
                .unwrap_or(false);

            if !self.procedurals.contains_key(proc_path) {
                removed_entries.push(proc_path.clone());
                continue;
            }

            if !has_proc {
                continue;
            }

            // Take proc out to call async_update
            let prev_child_types = self
                .procedurals
                .get(proc_path)
                .map(|e| e.child_types.clone())
                .unwrap_or_default();

            let result = {
                let entry = self.procedurals.get_mut(proc_path).unwrap();
                if let Some(ref mut proc) = entry.proc {
                    proc.async_update(&prev_child_types, &mut prim_types, &mut notices.dirtied)
                } else {
                    AsyncState::Finished
                }
            };

            if result == AsyncState::FinishedWithNewChanges
                || result == AsyncState::ContinuingWithNewChanges
            {
                let path = proc_path.clone();
                self.update_procedural_result(&path, prim_types.clone(), &mut notices);
                prim_types.clear();
            }

            if result == AsyncState::Finished || result == AsyncState::FinishedWithNewChanges {
                removed_entries.push(proc_path.clone());
            }
        }

        for path in &removed_entries {
            self.active_async_procedurals.remove(path);
        }

        // Forward accumulated notices (no sender needed for system messages,
        // but we need to send them somehow — use base's send methods if available)
        // Note: C++ calls _SendPrimsAdded/Removed/Dirtied which broadcast to observers.
        // In our Rust architecture, system_message is called externally and the
        // caller is responsible for forwarding any resulting notices.
    }

    // -----------------------------------------------------------------------
    // Notice handlers (called by observer)
    // -----------------------------------------------------------------------

    /// Handle added prims from input scene.
    /// Matches C++ `_PrimsAdded`.
    pub fn on_prims_added(&mut self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut notices = Notices::default();
        let mut procedurals_to_cook: HashSet<SdfPath> = HashSet::new();
        let mut entries_modified = false;

        for (i, entry) in entries.iter().enumerate() {
            if entry.prim_path.is_absolute_root_path() {
                continue;
            }

            if entry.prim_type == self.target_prim_type_name {
                if !entries_modified {
                    entries_modified = true;
                    // Copy all preceding entries verbatim
                    for prev in &entries[..i] {
                        notices.added.push(prev.clone());
                    }
                }
                // Re-type to resolved
                notices.added.push(AddedPrimEntry::new(
                    entry.prim_path.clone(),
                    tokens::RESOLVED_GENERATIVE_PROCEDURAL.clone(),
                ));
                procedurals_to_cook.insert(entry.prim_path.clone());
            } else {
                // Was a procedural, now different type — cook to handle removal
                if self.procedurals.contains_key(&entry.prim_path) {
                    procedurals_to_cook.insert(entry.prim_path.clone());
                }
                if entries_modified {
                    notices.added.push(entry.clone());
                }
            }

            // Check childNames dependency: if parent is a dependency, invalidate dependents
            let parent_path = entry.prim_path.get_parent_path();
            if let Some(dependent_procs) = self.dependencies.get(&parent_path).cloned() {
                let key_set = child_names_dependency_key_set();
                for proc_path in dependent_procs {
                    if procedurals_to_cook.contains(&proc_path) {
                        continue;
                    }
                    if let Some(proc_entry) = self.procedurals.get(&proc_path) {
                        if let Some(dep_locators) = proc_entry.dependencies.get(&parent_path) {
                            if dep_locators.intersects(&key_set) {
                                procedurals_to_cook.insert(proc_path);
                            }
                        }
                    }
                }
            }
        }

        // Cook scheduled procedurals (parallel when >=2, matching C++ threshold)
        let paths_to_cook: Vec<SdfPath> = procedurals_to_cook.into_iter().collect();
        self.cook_procedurals_batch(paths_to_cook, &mut notices);

        // Forward notices
        if !entries_modified {
            self.base.forward_prims_added(self, entries);
        } else {
            self.base.forward_prims_added(self, &notices.added);
        }
        if !notices.removed.is_empty() {
            self.base.forward_prims_removed(self, &notices.removed);
        }
        if !notices.dirtied.is_empty() {
            self.base.forward_prims_dirtied(self, &notices.dirtied);
        }
    }

    /// Handle removed prims from input scene.
    /// Matches C++ `_PrimsRemoved`.
    pub fn on_prims_removed(
        &mut self,
        _sender: &dyn HdSceneIndexBase,
        entries: &[RemovedPrimEntry],
    ) {
        // Fast path: absolute root = full scene teardown
        for entry in entries {
            if entry.prim_path.is_absolute_root_path() {
                self.procedurals.clear();
                self.dependencies.clear();
                self.generated_prims.clear();
                self.base.forward_prims_removed(self, entries);
                return;
            }
        }

        // Pre-seed ancestor maps for efficient batch lookups
        let mut dependency_ancestors: HashMap<SdfPath, HashSet<SdfPath>> = HashMap::new();
        for dep_path in self.dependencies.keys() {
            for ancestor in dep_path.get_ancestors_range() {
                dependency_ancestors
                    .entry(ancestor)
                    .or_default()
                    .insert(dep_path.clone());
            }
        }

        let mut proc_ancestors: HashMap<SdfPath, HashSet<SdfPath>> = HashMap::new();
        for proc_path in self.procedurals.keys() {
            for ancestor in proc_path.get_ancestors_range() {
                proc_ancestors
                    .entry(ancestor)
                    .or_default()
                    .insert(proc_path.clone());
            }
        }

        let mut removed_dependencies: HashSet<SdfPath> = HashSet::new();
        let mut invalidated_procedurals: HashSet<SdfPath> = HashSet::new();
        let mut removed_procedurals: HashSet<SdfPath> = HashSet::new();

        let key_set = child_names_dependency_key_set();

        for entry in entries {
            // Check if removed path is an ancestor of any dependency
            if let Some(dep_paths) = dependency_ancestors.get(&entry.prim_path).cloned() {
                for dep_path in dep_paths {
                    if let Some(dependent_procs) = self.dependencies.get(&dep_path).cloned() {
                        removed_dependencies.insert(dep_path);
                        for proc_path in dependent_procs {
                            if !removed_procedurals.contains(&proc_path) {
                                invalidated_procedurals.insert(proc_path);
                            }
                        }
                    }
                }
            } else {
                // Check childNames dependency on parent
                let parent_path = entry.prim_path.get_parent_path();
                if let Some(dependent_procs) = self.dependencies.get(&parent_path).cloned() {
                    for proc_path in dependent_procs {
                        if removed_procedurals.contains(&proc_path) {
                            continue;
                        }
                        if let Some(proc_entry) = self.procedurals.get(&proc_path) {
                            if let Some(dep_locators) = proc_entry.dependencies.get(&parent_path) {
                                if dep_locators.intersects(&key_set) {
                                    invalidated_procedurals.insert(proc_path);
                                }
                            }
                        }
                    }
                }
            }

            // Check if removed path is an ancestor of any procedural
            if let Some(proc_paths) = proc_ancestors.get(&entry.prim_path).cloned() {
                for proc_path in proc_paths {
                    removed_procedurals.insert(proc_path.clone());
                    invalidated_procedurals.remove(&proc_path);
                }
            }
        }

        // Clean up removed dependencies
        for dep_path in &removed_dependencies {
            self.dependencies.remove(dep_path);
        }

        // Remove procedurals
        for proc_path in &removed_procedurals {
            self.remove_procedural(proc_path, None);
        }

        // Re-cook invalidated procedurals
        if !invalidated_procedurals.is_empty() {
            let mut notices = Notices::default();
            for entry in entries {
                notices.removed.push(entry.clone());
            }

            let paths: Vec<SdfPath> = invalidated_procedurals.into_iter().collect();
            self.cook_procedurals_batch(paths, &mut notices);

            if !notices.added.is_empty() {
                self.base.forward_prims_added(self, &notices.added);
            }
            self.base.forward_prims_removed(self, &notices.removed);
            if !notices.dirtied.is_empty() {
                self.base.forward_prims_dirtied(self, &notices.dirtied);
            }
        } else {
            self.base.forward_prims_removed(self, entries);
        }
    }

    /// Handle dirtied prims from input scene.
    /// Matches C++ `_PrimsDirtied`.
    pub fn on_prims_dirtied(
        &mut self,
        _sender: &dyn HdSceneIndexBase,
        entries: &[DirtiedPrimEntry],
    ) {
        // Map: proc_path -> DependencyMap (accumulated dirty locators per dep path)
        let mut invalidated: HashMap<SdfPath, DependencyMap> = HashMap::new();

        for entry in entries {
            // Direct dirtying of a procedural prim itself
            if self.procedurals.contains_key(&entry.prim_path) {
                invalidated
                    .entry(entry.prim_path.clone())
                    .or_default()
                    .entry(entry.prim_path.clone())
                    .or_default()
                    .insert_set(&entry.dirty_locators);
            }

            // Check if this prim is a dependency of any procedural
            if let Some(dependent_procs) = self.dependencies.get(&entry.prim_path).cloned() {
                for proc_path in dependent_procs {
                    if let Some(proc_entry) = self.procedurals.get(&proc_path) {
                        if let Some(dep_locators) = proc_entry.dependencies.get(&entry.prim_path) {
                            if entry.dirty_locators.intersects(dep_locators) {
                                invalidated
                                    .entry(proc_path)
                                    .or_default()
                                    .entry(entry.prim_path.clone())
                                    .or_default()
                                    .insert_set(&entry.dirty_locators);
                            }
                        }
                    }
                }
            }
        }

        if !invalidated.is_empty() {
            let mut notices = Notices::default();
            for entry in entries {
                notices.dirtied.push(entry.clone());
            }

            let paths_deps: Vec<(SdfPath, DependencyMap)> = invalidated.into_iter().collect();
            self.cook_procedurals_batch_with_deps(paths_deps, &mut notices);

            if !notices.added.is_empty() {
                self.base.forward_prims_added(self, &notices.added);
            }
            if !notices.removed.is_empty() {
                self.base.forward_prims_removed(self, &notices.removed);
            }
            self.base.forward_prims_dirtied(self, &notices.dirtied);
        } else {
            self.base.forward_prims_dirtied(self, entries);
        }
    }
}

// ---------------------------------------------------------------------------
// HdSceneIndexBase implementation
// ---------------------------------------------------------------------------

impl HdSceneIndexBase for HdGpGenerativeProceduralResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // Check generated prims first (driven by notices, not lazy cook)
        if let Some(responsible_proc_opt) = self.generated_prims.get(prim_path) {
            if let Some(proc_path) = responsible_proc_opt {
                if let Some(entry) = self.procedurals.get(proc_path) {
                    if let Some(ref proc) = entry.proc {
                        if let Some(input) = self.base.get_input_scene() {
                            { let locked = input.read();
                                return proc.get_child_prim(&*locked, prim_path);
                            }
                        }
                    }
                }
            }
        }

        // Fall through to input scene
        if let Some(input) = self.base.get_input_scene() {
            { let locked = input.read();
                let prim = locked.get_prim(prim_path);
                if prim.prim_type == self.target_prim_type_name {
                    // Re-type to resolved to prevent double-evaluation downstream
                    return HdSceneIndexPrim {
                        prim_type: tokens::RESOLVED_GENERATIVE_PROCEDURAL.clone(),
                        data_source: prim.data_source,
                    };
                }
                return prim;
            }
        }

        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        // Always include input scene children (procedural may shadow/extend them)
        let mut result = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_child_prim_paths(prim_path)
        } else {
            Vec::new()
        };

        // Check if prim_path is a cooked procedural
        if let Some(entry) = self.procedurals.get(prim_path) {
            let _guard = entry.cook_mutex.lock();
            if let Some(children) = entry.child_hierarchy.get(prim_path) {
                combine_path_arrays(children, &mut result);
            }
            return result;
        }

        // Check if prim_path is a generated prim
        if let Some(Some(proc_path)) = self.generated_prims.get(prim_path) {
            let proc_path = proc_path.clone();
            if let Some(entry) = self.procedurals.get(&proc_path) {
                let _guard = entry.cook_mutex.lock();
                if let Some(children) = entry.child_hierarchy.get(prim_path) {
                    combine_path_arrays(children, &mut result);
                }
                return result;
            }
        }

        result
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn get_display_name(&self) -> String {
        "HdGpGenerativeProceduralResolvingSceneIndex".to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::scene_index::HdRetainedSceneIndex;

    #[test]
    fn test_create_scene_index() {
        let input = HdRetainedSceneIndex::new();
        let scene_index = HdGpGenerativeProceduralResolvingSceneIndex::new(input);

        let si = scene_index.read();
        assert_eq!(si.target_prim_type_name, *tokens::GENERATIVE_PROCEDURAL);
    }

    #[test]
    fn test_create_with_custom_type() {
        let input = HdRetainedSceneIndex::new();
        let custom_type = TfToken::new("CustomProcedural");
        let scene_index =
            HdGpGenerativeProceduralResolvingSceneIndex::new_with_type(input, custom_type.clone());

        let si = scene_index.read();
        assert_eq!(si.target_prim_type_name, custom_type);
    }

    #[test]
    fn test_get_prim_fallthrough() {
        let input = HdRetainedSceneIndex::new();
        let scene_index = HdGpGenerativeProceduralResolvingSceneIndex::new(input);

        let si = scene_index.read();
        let path = SdfPath::from_string("/Test").unwrap();
        let prim = si.get_prim(&path);
        assert!(prim.prim_type.is_empty());
    }

    #[test]
    fn test_get_child_prim_paths_empty() {
        let input = HdRetainedSceneIndex::new();
        let scene_index = HdGpGenerativeProceduralResolvingSceneIndex::new(input);

        let si = scene_index.read();
        let path = SdfPath::from_string("/Test").unwrap();
        let children = si.get_child_prim_paths(&path);
        assert!(children.is_empty());
    }

    #[test]
    fn test_combine_path_arrays_dedup() {
        let mut vec = vec![
            SdfPath::from_string("/A").unwrap(),
            SdfPath::from_string("/B").unwrap(),
        ];
        let mut set = HashSet::new();
        set.insert(SdfPath::from_string("/B").unwrap()); // duplicate
        set.insert(SdfPath::from_string("/C").unwrap()); // new

        combine_path_arrays(&set, &mut vec);
        assert_eq!(vec.len(), 3); // /A, /B, /C — no duplicate /B
        assert!(vec.contains(&SdfPath::from_string("/C").unwrap()));
    }

    #[test]
    fn test_remove_nonexistent_procedural() {
        let input = HdRetainedSceneIndex::new();
        let scene_index = HdGpGenerativeProceduralResolvingSceneIndex::new(input);
        let mut si = scene_index.write();
        // Should not panic
        let path = SdfPath::from_string("/NonExistent").unwrap();
        si.remove_procedural(&path, None);
    }
}
