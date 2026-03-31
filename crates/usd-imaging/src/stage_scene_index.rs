//! Stage scene index - main USD to Hydra bridge.
//!
//! Port of pxr/usdImaging/usdImaging/stageSceneIndex.{h,cpp}

use super::adapter_manager::{AdapterManager, AdaptersEntry};
use super::adapter_registry::AdapterRegistry;
use super::data_source_stage::DataSourceStage;
use super::data_source_stage_globals::DataSourceStageGlobals;
use super::tokens::UsdImagingTokens;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_core::{Prim, Stage, TimeCode};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
    HdTypedSampledDataSource,
};
use usd_hd::{
    AddedPrimEntry, DirtiedPrimEntry, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdSceneIndexBase, HdSceneIndexObserver, HdSceneIndexPrim, RemovedPrimEntry,
};
use usd_hd::flo_debug::{flo_debug_enabled, summarize_dirtied_entries};
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// Free functions (C++ anonymous namespace equivalents)
// ============================================================================

/// Read includeUnloadedPrims from StageSceneIndex input args.
fn get_include_unloaded_prims(input_args: Option<&HdContainerDataSourceHandle>) -> bool {
    let Some(container) = input_args else {
        return false;
    };
    let Some(ds) = container.get(UsdImagingTokens::stage_scene_index_include_unloaded_prims())
    else {
        return false;
    };
    let any = (&*ds).as_any();
    if let Some(bool_ds) = any.downcast_ref::<HdRetainedTypedSampledDataSource<bool>>() {
        return bool_ds.get_typed_value(0.0);
    }
    false
}

/// Collect imaging subprims from an AdaptersEntry.
/// C++ parity: _GetImagingSubprims (stageSceneIndex.cpp:47-98).
/// Ensures the trivial subprim "" always exists for traversal/inherited attrs.
///
/// In C++, this iterates over all AdapterEntry (prim + API schema adapters).
/// In Rust, the PrimAdapter is the primary source. API schema adapters can
/// contribute additional subprims via the APISchemaAdapter trait when registered.
fn collect_imaging_subprims(prim: &Prim, entry: &AdaptersEntry) -> Vec<Token> {
    if entry.all_adapters.is_empty() {
        return vec![Token::new("")];
    }

    let mut subprims = Vec::new();
    let mut seen = HashSet::new();
    for adapter_entry in &entry.all_adapters {
        for subprim in adapter_entry
            .adapter
            .get_imaging_subprims(prim, &adapter_entry.applied_instance_name)
        {
            if seen.insert(subprim.clone()) {
                subprims.push(subprim);
            }
        }
    }

    if !seen.contains(&Token::new("")) {
        subprims.push(Token::new(""));
    }

    subprims
}

/// Get the imaging subprim type from an AdaptersEntry.
/// C++ parity: _GetImagingSubprimType - strongest non-empty opinion wins.
fn resolve_subprim_type(entry: &AdaptersEntry, prim: &Prim, subprim: &Token) -> Token {
    for adapter_entry in &entry.all_adapters {
        let result = adapter_entry.adapter.get_imaging_subprim_type(
            prim,
            subprim,
            &adapter_entry.applied_instance_name,
        );
        if !result.as_str().is_empty() {
            return result;
        }
    }
    Token::new("")
}

/// Get the imaging subprim data source from an AdaptersEntry.
/// C++ parity: _GetImagingSubprimData - overlay multiple data sources.
///
/// With a single adapter, returns its data source directly.
/// With multiple adapters (prim + API schema), overlays them via
/// HdOverlayContainerDataSource so no contributions are silently dropped.
fn resolve_subprim_data(
    entry: &AdaptersEntry,
    prim: &Prim,
    subprim: &Token,
    stage_globals: &Arc<dyn DataSourceStageGlobals>,
) -> Option<HdContainerDataSourceHandle> {
    let mut containers: Vec<HdContainerDataSourceHandle> = Vec::new();
    for adapter_entry in &entry.all_adapters {
        if let Some(ds) = adapter_entry.adapter.get_imaging_subprim_data(
            prim,
            subprim,
            &adapter_entry.applied_instance_name,
            stage_globals,
        ) {
            containers.push(ds);
        }
    }

    match containers.len() {
        0 => None,
        1 => Some(containers.into_iter().next().unwrap()),
        _ => {
            use usd_hd::data_source::HdOverlayContainerDataSource;
            Some(HdOverlayContainerDataSource::new(containers) as HdContainerDataSourceHandle)
        }
    }
}

/// Invalidate imaging subprim across adapters in an AdaptersEntry.
/// C++ parity: _InvalidateImagingSubprim.
fn resolve_invalidation(
    entry: &AdaptersEntry,
    prim: &Prim,
    subprim: &Token,
    properties: &[Token],
    invalidation_type: PropertyInvalidationType,
) -> HdDataSourceLocatorSet {
    if entry.all_adapters.is_empty() {
        return HdDataSourceLocatorSet::empty();
    }

    let mut result = HdDataSourceLocatorSet::empty();
    for adapter_entry in &entry.all_adapters {
        result.insert_set(&adapter_entry.adapter.invalidate_imaging_subprim(
            prim,
            subprim,
            &adapter_entry.applied_instance_name,
            properties,
            invalidation_type,
        ));
    }
    result
}

/// Delete all entries from a map whose keys have the given prefix.
/// C++ parity: _DeletePrefix.
fn delete_prefix(prefix: &Path, map: &mut HashMap<Path, Vec<Token>>) {
    map.retain(|path, _| !path.has_prefix(prefix));
}

// ============================================================================
// StageSceneIndex
// ============================================================================

/// Stage scene index wrapping a USD stage.
///
/// This is the main entry point for converting USD stage data into
/// Hydra scene indices. It:
///
/// - Wraps a UsdStage and exposes it as an HdSceneIndex
/// - Uses AdapterManager with multiple adapters (prim + API schema) per prim
/// - Tracks time-varying attributes for change notifications
/// - Handles USD stage edits with proper resync/update coalescing
/// - Tracks populated paths for efficient remove+add optimization
///
/// # C++ Parity
///
/// Full port of UsdImagingStageSceneIndex from OpenUSD, including:
/// - AdapterManager with multi-adapter overlay support
/// - Population mode handling (RepresentsSelfAndDescendents, RepresentedByAncestor)
/// - _FindResponsibleAncestor for descendant invalidation
/// - _populatedPaths tracking for resync optimization
/// - Prototype population at absolute root
/// - Property path subprim extraction in GetPrim
/// - Instance proxy filtering
pub struct StageSceneIndex {
    /// Standard observer management (C++ HdSceneIndexBase)
    base: usd_hd::scene_index::base::HdSceneIndexBaseImpl,
    /// Include unloaded prims in traversal
    include_unloaded_prims: bool,
    /// USD stage being wrapped
    stage: Arc<RwLock<Option<Arc<Stage>>>>,
    /// Current time code
    time: Arc<RwLock<TimeCode>>,
    /// Adapter registry (legacy, for backwards compat)
    adapter_registry: Arc<AdapterRegistry>,
    /// Adapter manager for multi-adapter support (C++ _adapterManager)
    adapter_manager: Arc<AdapterManager>,
    /// Stage globals for data sources
    stage_globals: Arc<StageGlobalsImpl>,
    /// Set of populated paths for resync optimization (C++ _populatedPaths)
    populated_paths: Arc<Mutex<BTreeSet<Path>>>,
    /// Prim paths queued for resync (C++ _usdPrimsToResync)
    usd_prims_to_resync: Arc<Mutex<Vec<Path>>>,
    /// Properties queued for resync (C++ _usdPropertiesToResync)
    usd_properties_to_resync: Arc<Mutex<HashMap<Path, Vec<Token>>>>,
    /// Properties queued for update (C++ _usdPropertiesToUpdate)
    usd_properties_to_update: Arc<Mutex<HashMap<Path, Vec<Token>>>>,
    /// Legacy pending resyncs (kept for backwards compat)
    pending_resyncs: Arc<Mutex<HashSet<Path>>>,
    /// Legacy pending changes (kept for backwards compat)
    pending_changes: Arc<Mutex<HashMap<Path, Vec<Token>>>>,

}

impl StageSceneIndex {
    /// Create new stage scene index.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::new_with_flags(false))
    }

    /// Create stage scene index with input args (for create_scene_indices).
    /// C++ parity: UsdImagingStageSceneIndex::New(HdContainerDataSourceHandle).
    ///
    /// Returns `Arc<Self>` (no outer RwLock) because StageSceneIndex uses
    /// interior mutability for all mutable state. An outer RwLock would cause
    /// recursive read-lock deadlocks in parking_lot when notification cascades
    /// (populate -> notify -> observer -> get_prim) re-enter this scene index.
    pub fn new_with_input_args(
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<Self> {
        let include_unloaded_prims = get_include_unloaded_prims(input_args.as_ref());
        Arc::new(Self::new_with_flags(include_unloaded_prims))
    }

    fn new_with_flags(include_unloaded_prims: bool) -> Self {
        let time = Arc::new(RwLock::new(TimeCode::default_time()));
        let stage_globals = Arc::new(StageGlobalsImpl::new(time.clone()));

        // Create registry with all default adapters (Mesh, Camera, Lights, etc.)
        // and wire AdapterManager to it so get_prim() returns correct types.
        let registry = Arc::new(AdapterRegistry::new_with_defaults());
        let adapter_manager = Arc::new(AdapterManager::new_with_registry(registry.clone()));

        Self {
            base: usd_hd::scene_index::base::HdSceneIndexBaseImpl::new(),
            include_unloaded_prims,
            stage: Arc::new(RwLock::new(None)),
            time,
            adapter_registry: registry,
            adapter_manager,

            stage_globals,
            populated_paths: Arc::new(Mutex::new(BTreeSet::new())),
            usd_prims_to_resync: Arc::new(Mutex::new(Vec::new())),
            usd_properties_to_resync: Arc::new(Mutex::new(HashMap::new())),
            usd_properties_to_update: Arc::new(Mutex::new(HashMap::new())),
            pending_resyncs: Arc::new(Mutex::new(HashSet::new())),
            pending_changes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set the USD stage.
    /// C++ parity: SetStage - clears populated paths, sends single PrimsRemoved
    /// for absolute root, resets adapter manager, then repopulates.
    pub fn set_stage(&self, stage: Arc<Stage>) {
        // Remove all existing prims via single root removal (C++ parity)
        let had_stage = {
            let mut current = self.stage.write();
            let had = current.is_some();
            *current = Some(stage.clone());
            had
        };
        // Notify after releasing stage write lock to avoid deadlock
        if had_stage {
            self.populated_paths.lock().expect("Lock poisoned").clear();
            self.notify_prims_removed(&[RemovedPrimEntry {
                prim_path: Path::absolute_root(),
            }]);
            self.stage_globals.clear();
            self.adapter_manager.reset();
        }

        // Repopulate
        self.populate();
    }

    /// Get the current USD stage.
    pub fn get_stage(&self) -> Option<Arc<Stage>> {
        self.stage.read().clone()
    }

    /// Set the current time and dirty time-varying prims.
    ///
    /// Port of `UsdImagingStageSceneIndex::SetTime`.
    ///
    /// Collects all time-varying prim paths from `StageGlobals`, builds
    /// `DirtiedPrimEntry` notices and dispatches them synchronously through
    /// the observer chain via `notify_prims_dirtied`. parking_lot guarantees
    /// reentrant reads, so cascading observer dispatch cannot deadlock.
    pub fn set_time(&self, time: TimeCode, force_dirtying: bool) {
        let old_time = *self.time.read();
        if !force_dirtying && time == old_time {
            return;
        }

        *self.time.write() = time;

        let mut dirtied = Vec::new();
        self.stage_globals.set_time(time, &mut dirtied);

        if !dirtied.is_empty() {
            self.notify_prims_dirtied(&dirtied);
        }
    }

    /// Get the current time.
    pub fn get_time(&self) -> TimeCode {
        *self.time.read()
    }

    /// Get the adapter registry (legacy).
    pub fn adapter_registry(&self) -> &AdapterRegistry {
        &self.adapter_registry
    }

    /// Get the adapter manager.
    pub fn adapter_manager(&self) -> &AdapterManager {
        &self.adapter_manager
    }

    // ========================================================================
    // Apply pending updates - C++ parity: ApplyPendingUpdates
    // ========================================================================

    /// Apply pending updates from USD stage edits.
    /// C++ parity: ApplyPendingUpdates.
    pub fn apply_pending_updates(&self) {
        let Some(_stage) = self.get_stage() else {
            return;
        };

        // Merge legacy pending_resyncs into usd_prims_to_resync
        {
            let mut legacy = self.pending_resyncs.lock().expect("Lock poisoned");
            if !legacy.is_empty() {
                let mut resyncs = self.usd_prims_to_resync.lock().expect("Lock poisoned");
                for path in legacy.drain() {
                    resyncs.push(path);
                }
            }
        }

        // Merge legacy pending_changes into usd_properties_to_update
        {
            let mut legacy = self.pending_changes.lock().expect("Lock poisoned");
            if !legacy.is_empty() {
                let mut updates = self.usd_properties_to_update.lock().expect("Lock poisoned");
                for (path, props) in legacy.drain() {
                    updates.entry(path).or_default().extend(props);
                }
            }
        }

        // Check if there's anything to do
        {
            let resyncs = self.usd_prims_to_resync.lock().expect("Lock poisoned");
            let props_resync = self.usd_properties_to_resync.lock().expect("Lock poisoned");
            let props_update = self.usd_properties_to_update.lock().expect("Lock poisoned");
            if resyncs.is_empty() && props_resync.is_empty() && props_update.is_empty() {
                return;
            }
        }

        // Process resyncs first
        self.apply_pending_resyncs();

        // Process property resyncs -> dirtied entries
        let mut dirtied = Vec::new();
        {
            let props_resync: HashMap<Path, Vec<Token>> = {
                let mut m = self.usd_properties_to_resync.lock().expect("Lock poisoned");
                std::mem::take(&mut *m)
            };
            self.compute_dirtied_entries(
                &props_resync,
                PropertyInvalidationType::Resync,
                &mut dirtied,
            );
        }

        // Process property updates -> dirtied entries
        {
            let props_update: HashMap<Path, Vec<Token>> = {
                let mut m = self.usd_properties_to_update.lock().expect("Lock poisoned");
                std::mem::take(&mut *m)
            };
            self.compute_dirtied_entries(
                &props_update,
                PropertyInvalidationType::PropertyChanged,
                &mut dirtied,
            );
        }

        // Resync any prims whose property invalidation indicated repopulation
        {
            let has_resyncs = !self
                .usd_prims_to_resync
                .lock()
                .expect("Lock poisoned")
                .is_empty();
            if has_resyncs {
                self.apply_pending_resyncs();
            }
        }

        if !dirtied.is_empty() {
            self.notify_prims_dirtied(&dirtied);
        }
    }

    // ========================================================================
    // Population - C++ parity: _Populate / _PopulateSubtree
    // ========================================================================

    /// Populate scene from stage.
    /// C++ parity: _Populate - also iterates stage prototypes.
    fn populate(&self) {
        let Some(stage) = self.get_stage() else {
            return;
        };

        let mut added = Vec::new();

        // Populate main scene hierarchy
        self.populate_subtree(&stage.get_pseudo_root(), &mut added);

        // Populate USD prototypes (C++ parity: item #8)
        for proto_prim in stage.get_prototypes() {
            self.populate_subtree(&proto_prim, &mut added);
        }

        // Track populated paths (C++ parity: item #9)
        {
            let mut populated = self.populated_paths.lock().expect("Lock poisoned");
            for entry in &added {
                populated.insert(entry.prim_path.clone());
            }
        }

        log::debug!("[populate] total added entries: {}", added.len());
        if !added.is_empty() {
            self.notify_prims_added(&added);
        }
    }

    /// Recursively populate prims from a subtree.
    /// C++ parity: _PopulateSubtree - uses AdapterManager, checks PopulationMode,
    /// prunes children for RepresentsSelfAndDescendents.
    fn populate_subtree(&self, prim: &Prim, added: &mut Vec<AddedPrimEntry>) {
        if !prim.is_valid() {
            return;
        }

        // Handle pseudo root: always add it with empty type (C++ parity)
        if prim.is_pseudo_root() {
            added.push(AddedPrimEntry {
                prim_path: Path::absolute_root(),
                prim_type: Token::new(""),
                data_source: None,
            });

            // Recurse to children using _GetPrimPredicate (no IsDefined filter)
            for child in prim.get_all_children() {
                if self.should_traverse(&child) {
                    self.populate_subtree(&child, added);
                }
            }
            return;
        }

        // Look up adapters via AdapterManager (C++ _adapterManager->LookupAdapters)
        let entry = self.adapter_manager.lookup_adapters(prim);

        // Check RepresentsSelfAndDescendents -> prune children
        let prune_children = entry.prim_adapter.as_ref().is_some_and(|a| {
            a.get_population_mode() == PopulationMode::RepresentsSelfAndDescendents
        });

        // Enumerate imaging subprims
        let prim_path = prim.get_path().clone();
        let subprims = collect_imaging_subprims(prim, &entry);
        for subprim in &subprims {
            let subpath = if subprim.as_str().is_empty() {
                prim_path.clone()
            } else {
                prim_path
                    .append_property(subprim.as_str())
                    .unwrap_or_else(|| prim_path.clone())
            };

            let prim_type = resolve_subprim_type(&entry, prim, subprim);

            added.push(AddedPrimEntry {
                prim_path: subpath,
                prim_type,
                data_source: None, // Data source is lazy (fetched via GetPrim)
            });
        }

        // Recurse to children (unless pruned by RepresentsSelfAndDescendents)
        // C++ parity: uses _GetPrimPredicate (Active && !Abstract [&& Loaded])
        // which does NOT require IsDefined — so we use get_all_children() + should_traverse
        if !prune_children {
            for child in prim.get_all_children() {
                if self.should_traverse(&child) {
                    self.populate_subtree(&child, added);
                }
            }
        }
    }

    /// Check if a prim passes the traversal predicate.
    /// C++ parity: _GetPrimPredicate (UsdPrimIsActive && !UsdPrimIsAbstract
    /// && optionally UsdPrimIsLoaded). Note: C++ does NOT require UsdPrimIsDefined
    /// to pick up instances and overs.
    fn should_traverse(&self, prim: &Prim) -> bool {
        prim.is_active() && !prim.is_abstract() && (self.include_unloaded_prims || prim.is_loaded())
    }

    // ========================================================================
    // Resync handling - C++ parity: _ApplyPendingResyncs
    // ========================================================================

    /// Apply pending resync requests with coalescing and populated-path optimization.
    /// C++ parity: _ApplyPendingResyncs.
    fn apply_pending_resyncs(&self) {
        let Some(stage) = self.get_stage() else {
            return;
        };

        let mut resyncs: Vec<Path> = {
            let mut pending = self.usd_prims_to_resync.lock().expect("Lock poisoned");
            std::mem::take(&mut *pending)
        };

        if resyncs.is_empty() {
            return;
        }

        let mut removed_prims = Vec::new();
        let mut added_prims = Vec::new();

        // Sort and coalesce paths with common prefix (C++ parity)
        resyncs.sort();
        let mut last_resynced = 0usize;

        for i in 0..resyncs.len() {
            let prim_path = &resyncs[i];

            // Skip paths subsumed by an earlier resync (C++ coalescing)
            if i > 0 && prim_path.has_prefix(&resyncs[last_resynced]) {
                continue;
            }
            last_resynced = i;

            let prim_path = prim_path.clone();

            // Check RepresentedByAncestor -> convert to property dirtying (C++ parity: item #10)
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                let entry = self.adapter_manager.lookup_adapters(&prim);
                if let Some(ref prim_adapter) = entry.prim_adapter {
                    if prim_adapter.get_population_mode() == PopulationMode::RepresentedByAncestor {
                        if let Some((_ancestor_prim, _ancestor_adapter)) =
                            self.find_responsible_ancestor(&prim)
                        {
                            // Convert to property resync with empty token (C++ parity)
                            self.usd_properties_to_resync
                                .lock()
                                .expect("Lock poisoned")
                                .insert(prim_path.clone(), vec![Token::new("")]);
                            continue;
                        }
                    }
                }
            }

            // Standard resync: remove + repopulate
            removed_prims.push(RemovedPrimEntry {
                prim_path: prim_path.clone(),
            });

            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                self.populate_subtree(&prim, &mut added_prims);
            }

            // At absolute root, also repopulate prototypes (C++ parity)
            if prim_path.is_absolute_root_path() {
                for proto_prim in stage.get_prototypes() {
                    self.populate_subtree(&proto_prim, &mut added_prims);
                }
            }

            // Prune redundant property updates for resynced subtrees (C++ parity)
            {
                let mut props_resync = self.usd_properties_to_resync.lock().expect("Lock poisoned");
                delete_prefix(&prim_path, &mut props_resync);
            }
            {
                let mut props_update = self.usd_properties_to_update.lock().expect("Lock poisoned");
                delete_prefix(&prim_path, &mut props_update);
            }
        }

        // Optimization: filter out removed prims that are re-added (C++ parity: item #9)
        // A single PrimsAdded is sufficient to "re-sync" an existing prim.
        {
            let added_paths: HashSet<&Path> = added_prims.iter().map(|e| &e.prim_path).collect();

            let populated = self.populated_paths.lock().expect("Lock poisoned");
            let mut filtered_removed = Vec::new();

            for entry in &removed_prims {
                let removed_path = &entry.prim_path;
                // Check all populated paths under removed_path
                for pop_path in populated.range(removed_path.clone()..) {
                    if !pop_path.has_prefix(removed_path) {
                        break;
                    }
                    if !added_paths.contains(pop_path) {
                        // This populated path won't be re-added -> explicitly remove it
                        filtered_removed.push(RemovedPrimEntry {
                            prim_path: pop_path.clone(),
                        });
                    }
                }
            }

            // Use filtered list
            drop(populated);
            removed_prims = filtered_removed;
        }

        // Update populated paths
        {
            let mut populated = self.populated_paths.lock().expect("Lock poisoned");
            for entry in &removed_prims {
                populated.remove(&entry.prim_path);
            }
            for entry in &added_prims {
                populated.insert(entry.prim_path.clone());
            }
        }

        self.notify_prims_removed(&removed_prims);
        self.notify_prims_added(&added_prims);
    }

    // ========================================================================
    // _FindResponsibleAncestor - C++ parity: item #10
    // ========================================================================

    /// Walk up the hierarchy to find an ancestor adapter with
    /// RepresentsSelfAndDescendents mode.
    /// C++ parity: _FindResponsibleAncestor.
    fn find_responsible_ancestor(
        &self,
        prim: &Prim,
    ) -> Option<(Prim, super::prim_adapter::PrimAdapterHandle)> {
        let mut current = prim.parent();
        while current.is_valid() {
            let entry = self.adapter_manager.lookup_adapters(&current);
            if let Some(ref prim_adapter) = entry.prim_adapter {
                if prim_adapter.get_population_mode()
                    == PopulationMode::RepresentsSelfAndDescendents
                {
                    return Some((current, prim_adapter.clone()));
                }
            }
            current = current.parent();
        }
        None
    }

    // ========================================================================
    // _ComputeDirtiedEntries - C++ parity
    // ========================================================================

    /// Compute dirtied entries from property changes.
    /// C++ parity: _ComputeDirtiedEntries.
    /// Handles RepresentedByAncestor delegation and repopulate locator detection.
    fn compute_dirtied_entries(
        &self,
        path_to_properties: &HashMap<Path, Vec<Token>>,
        invalidation_type: PropertyInvalidationType,
        dirtied: &mut Vec<DirtiedPrimEntry>,
    ) {
        let Some(stage) = self.get_stage() else {
            return;
        };

        for (prim_path, properties) in path_to_properties {
            let Some(prim) = stage.get_prim_at_path(prim_path) else {
                continue;
            };

            let entry = self.adapter_manager.lookup_adapters(&prim);

            // Handle RepresentedByAncestor: delegate to ancestor (C++ parity)
            if let Some(ref prim_adapter) = entry.prim_adapter {
                if prim_adapter.get_population_mode() == PopulationMode::RepresentedByAncestor {
                    if let Some((ancestor_prim, ancestor_adapter)) =
                        self.find_responsible_ancestor(&prim)
                    {
                        // Give the parent adapter a chance to invalidate its subprims
                        let ancestor_subprims =
                            ancestor_adapter.get_imaging_subprims(&ancestor_prim);
                        for subprim in &ancestor_subprims {
                            let dirty_locators = ancestor_adapter
                                .invalidate_imaging_subprim_from_descendant(
                                    &ancestor_prim,
                                    &prim,
                                    subprim,
                                    properties,
                                    invalidation_type,
                                );

                            if !dirty_locators.is_empty() {
                                let path = if subprim.as_str().is_empty() {
                                    ancestor_prim.get_path().clone()
                                } else {
                                    ancestor_prim
                                        .get_path()
                                        .append_property(subprim.as_str())
                                        .unwrap_or_else(|| ancestor_prim.get_path().clone())
                                };
                                dirtied.push(DirtiedPrimEntry {
                                    prim_path: path,
                                    dirty_locators,
                                });
                            }
                        }
                        continue; // Handled by ancestor
                    }
                    // No responsible ancestor found -> fall through to handle ourselves
                }
            }

            // Standard invalidation via all adapters
            let subprims = collect_imaging_subprims(&prim, &entry);

            for subprim in &subprims {
                let dirty_locators =
                    resolve_invalidation(&entry, &prim, subprim, properties, invalidation_type);

                if !dirty_locators.is_empty() {
                    // Check for repopulate locator (C++ parity)
                    let repopulate_locator = HdDataSourceLocator::from_token(
                        UsdImagingTokens::stage_scene_index_repopulate().clone(),
                    );
                    if dirty_locators.contains(&repopulate_locator) {
                        // Queue for resync instead
                        self.usd_prims_to_resync
                            .lock()
                            .expect("Lock poisoned")
                            .push(prim_path.clone());
                    } else {
                        let subpath = if subprim.as_str().is_empty() {
                            prim_path.clone()
                        } else {
                            prim_path
                                .append_property(subprim.as_str())
                                .unwrap_or_else(|| prim_path.clone())
                        };
                        dirtied.push(DirtiedPrimEntry {
                            prim_path: subpath,
                            dirty_locators,
                        });
                    }
                }
            }
        }
    }

    // ========================================================================
    // Legacy apply methods (kept for backwards compat, now route to new system)
    // ========================================================================

    /// Resync a single prim (legacy).
    #[allow(dead_code)] // Legacy C++ backwards-compat path, kept for future wiring
    fn resync_prim(&self, path: &Path) {
        let Some(stage) = self.get_stage() else {
            return;
        };
        let Some(prim) = stage.get_prim_at_path(path) else {
            return;
        };

        let mut removed = Vec::new();
        self.collect_removed_prims(&prim, &mut removed);
        if !removed.is_empty() {
            self.notify_prims_removed(&removed);
        }

        let mut added = Vec::new();
        self.populate_subtree(&prim, &mut added);
        if !added.is_empty() {
            self.notify_prims_added(&added);
        }
    }

    /// Recursively collect prims to remove (for legacy path).
    #[allow(dead_code)] // Called by resync_prim (legacy path)
    fn collect_removed_prims(&self, prim: &Prim, removed: &mut Vec<RemovedPrimEntry>) {
        if !prim.is_valid() {
            return;
        }

        let entry = self.adapter_manager.lookup_adapters(prim);
        let subprims = collect_imaging_subprims(prim, &entry);

        for subprim in &subprims {
            let hydra_path = if subprim.as_str().is_empty() {
                prim.get_path().clone()
            } else {
                prim.get_path()
                    .append_property(subprim.as_str())
                    .unwrap_or_else(|| prim.get_path().clone())
            };
            removed.push(RemovedPrimEntry {
                prim_path: hydra_path,
            });
        }

        let prune = entry
            .prim_adapter
            .as_ref()
            .is_some_and(|a| a.should_cull_children());

        if !prune {
            for child in prim.get_children() {
                if self.should_traverse(&child) {
                    self.collect_removed_prims(&child, removed);
                }
            }
        }
    }

    // ========================================================================
    // Observer notification
    // ========================================================================

    /// Dispatch through `HdSceneIndexBaseImpl` — the standard observer
    /// mechanism shared by all scene indices (C++ `_SendPrimsAdded`).
    fn notify_prims_added(&self, entries: &[AddedPrimEntry]) {
        self.base.send_prims_added(self, entries);
    }

    /// C++ `_SendPrimsRemoved`.
    fn notify_prims_removed(&self, entries: &[RemovedPrimEntry]) {
        self.base.send_prims_removed(self, entries);
    }

    /// C++ `_SendPrimsDirtied`.
    fn notify_prims_dirtied(&self, entries: &[DirtiedPrimEntry]) {
        if flo_debug_enabled() {
            let summary = summarize_dirtied_entries(entries);
            eprintln!(
                "[dirty-trace] stage=stage_scene_index emitter={} total={} unique={} dup_paths={} dup_instances={} first={}",
                self.get_display_name(),
                summary.total,
                summary.unique_paths,
                summary.duplicate_paths,
                summary.duplicate_instances,
                summary.first_path,
            );
        }
        log::info!(
            "[stage_si] _SendPrimsDirtied: {} entries, observed={}",
            entries.len(),
            self.base.is_observed()
        );
        self.base.send_prims_dirtied(self, entries);
    }
}

// ============================================================================
// HdSceneIndexBase implementation
// ============================================================================

impl HdSceneIndexBase for StageSceneIndex {
    /// Get prim at path.
    /// C++ parity: GetPrim - handles absolute root (DataSourceStage), instance proxy
    /// check, property path subprim extraction, multi-adapter overlay, empty fallback.
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        let Some(stage) = self.get_stage() else {
            return HdSceneIndexPrim::default();
        };

        // Item #1: absolute root -> return DataSourceStage (C++ parity)
        if prim_path.is_absolute_root_path() {
            return HdSceneIndexPrim {
                prim_type: Token::new(""),
                data_source: Some(
                    Arc::new(DataSourceStage::new(stage.clone())) as HdContainerDataSourceHandle
                ),
            };
        }

        // Extract prim path (strips property component if present)
        let actual_prim_path = prim_path.get_prim_path();

        let Some(prim) = stage.get_prim_at_path(&actual_prim_path) else {
            return HdSceneIndexPrim::default();
        };

        // Item #2: instance proxy check -> return empty prim (C++ parity)
        if prim.is_instance_proxy() {
            return HdSceneIndexPrim::default();
        }

        // Check traversal predicate
        if !self.should_traverse(&prim) {
            return HdSceneIndexPrim::default();
        }

        // Item #3: property path -> extract subprim name token (C++ parity)
        let subprim = if prim_path.is_property_path() {
            prim_path.get_name_token()
        } else {
            Token::new("")
        };

        // Item #4: multi-adapter overlay via AdapterManager (C++ parity)
        let entry = self.adapter_manager.lookup_adapters(&prim);
        let globals: Arc<dyn DataSourceStageGlobals> = self.stage_globals.clone();
        let mut data_source = resolve_subprim_data(&entry, &prim, &subprim, &globals);

        // C++ parity: if empty subprim and no data source, use empty container
        if subprim.as_str().is_empty() && data_source.is_none() {
            data_source =
                Some(HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle);
        }

        HdSceneIndexPrim {
            prim_type: resolve_subprim_type(&entry, &prim, &subprim),
            data_source,
        }
    }

    /// Get child prim paths.
    /// C++ parity: GetChildPrimPaths - checks subprim leaf, population mode,
    /// adds subprims, adds prototypes at root.
    fn get_child_prim_paths(&self, prim_path: &Path) -> Vec<Path> {
        let Some(stage) = self.get_stage() else {
            return Vec::new();
        };

        // Item #6 subprim check: non-root/prim paths are leaves (C++ parity)
        if !prim_path.is_absolute_root_or_prim_path() {
            return Vec::new();
        }

        let Some(prim) = stage.get_prim_at_path(prim_path) else {
            return Vec::new();
        };

        let mut result = Vec::new();

        let entry = self.adapter_manager.lookup_adapters(&prim);

        // Item #5: check adapter population mode (C++ parity)
        // Only skip children if the prim adapter uses RepresentsSelfAndDescendents
        let skip_children = entry.prim_adapter.as_ref().is_some_and(|a| {
            a.get_population_mode() == PopulationMode::RepresentsSelfAndDescendents
        });

        if !skip_children {
            // C++ parity: uses _GetPrimPredicate (Active && !Abstract [&& Loaded])
            // which does NOT require IsDefined — so we use get_all_children()
            for child in prim.get_all_children() {
                if self.should_traverse(&child) {
                    result.push(child.get_path().clone());
                }
            }
        }

        // Item #6: add subprims (non-empty tokens) as children (C++ parity)
        let prim_path_owned = prim.get_path().clone();
        let subprims = collect_imaging_subprims(&prim, &entry);
        for subprim in &subprims {
            if !subprim.as_str().is_empty() {
                if let Some(subpath) = prim_path_owned.append_property(subprim.as_str()) {
                    result.push(subpath);
                }
            }
        }

        // Item #7: at absolute root, add stage prototypes (C++ parity)
        if prim_path.is_absolute_root_path() {
            for proto_prim in stage.get_prototypes() {
                result.push(proto_prim.get_path().clone());
            }
        }

        result
    }

    fn add_observer(&self, observer: Arc<dyn HdSceneIndexObserver>) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &Arc<dyn HdSceneIndexObserver>) {
        self.base.remove_observer(observer);
    }

    fn set_display_name(&mut self, _name: String) {}
    fn add_tag(&mut self, _tag: Token) {}
    fn remove_tag(&mut self, _tag: &Token) {}
    fn has_tag(&self, _tag: &Token) -> bool {
        false
    }
    fn get_tags(&self) -> Vec<Token> {
        Vec::new()
    }
}

// ============================================================================
// StageGlobals implementation
// ============================================================================

/// Stage globals implementation for StageSceneIndex.
/// C++ parity: _StageGlobals inner class.
struct StageGlobalsImpl {
    time: Arc<RwLock<TimeCode>>,
    time_varying: Arc<RwLock<HashMap<Path, HdDataSourceLocatorSet>>>,
    asset_path_dependents: Arc<RwLock<BTreeSet<Path>>>,
}

impl StageGlobalsImpl {
    fn new(time: Arc<RwLock<TimeCode>>) -> Self {
        Self {
            time,
            time_varying: Arc::new(RwLock::new(HashMap::new())),
            asset_path_dependents: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    fn set_time(&self, time: TimeCode, dirtied: &mut Vec<DirtiedPrimEntry>) {
        *self.time.write() = time;

        let time_varying = self.time_varying.read();
        if std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some() {
            eprintln!(
                "[stage_globals] set_time={} tracked={}",
                time.value(),
                time_varying.len()
            );
            for (path, locators) in time_varying.iter() {
                eprintln!(
                    "[stage_globals] tracked path={} locators={:?}",
                    path,
                    locators
                );
            }
        }
        log::debug!(
            "[stage_globals] set_time={} time_varying_count={}",
            time.value(),
            time_varying.len()
        );
        for (path, locators) in time_varying.iter() {
            dirtied.push(DirtiedPrimEntry {
                prim_path: path.clone(),
                dirty_locators: locators.clone(),
            });
        }
    }

    fn clear(&self) {
        self.time_varying.write().clear();
        self.asset_path_dependents.write().clear();
    }

    /// Remove asset path dependents under a given path.
    /// C++ parity: RemoveAssetPathDependentsUnder.
    #[allow(dead_code)] // C++ API parity, used in tests + future ApplyPendingUpdates
    fn remove_asset_path_dependents_under(&self, path: &Path) {
        let mut dependents = self.asset_path_dependents.write();
        let to_remove: Vec<Path> = dependents
            .range(path.clone()..)
            .take_while(|p| p.has_prefix(path))
            .cloned()
            .collect();
        for p in to_remove {
            dependents.remove(&p);
        }
    }

    /// Invalidate asset path dependents under a given path.
    /// C++ parity: InvalidateAssetPathDependentsUnder.
    #[allow(dead_code)] // C++ API parity, will be called from ApplyPendingUpdates
    fn invalidate_asset_path_dependents_under(
        &self,
        path: &Path,
        prims_to_invalidate: &mut Vec<Path>,
        properties_to_invalidate: &mut HashMap<Path, Vec<Token>>,
    ) {
        let dependents = self.asset_path_dependents.read();
        for dep_path in dependents
            .range(path.clone()..)
            .take_while(|p| p.has_prefix(path))
        {
            if dep_path.is_absolute_root_or_prim_path() {
                prims_to_invalidate.push(dep_path.clone());
            } else if dep_path.is_property_path() {
                properties_to_invalidate
                    .entry(dep_path.get_prim_path())
                    .or_default()
                    .push(dep_path.get_name_token());
            }
        }
    }
}

impl DataSourceStageGlobals for StageGlobalsImpl {
    fn get_time(&self) -> TimeCode {
        *self.time.read()
    }

    fn flag_as_time_varying(&self, hydra_path: &Path, locator: &HdDataSourceLocator) {
        if std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some() {
            eprintln!(
                "[stage_globals] flag path={} locator={:?}",
                hydra_path,
                locator
            );
        }
        let mut time_varying = self.time_varying.write();
        time_varying
            .entry(hydra_path.clone())
            .or_insert_with(HdDataSourceLocatorSet::empty)
            .insert(locator.clone());
    }

    fn flag_as_asset_path_dependent(&self, usd_path: &Path) {
        let mut dependents = self.asset_path_dependents.write();
        dependents.insert(usd_path.clone());
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_scene_index_new() {
        let scene = StageSceneIndex::new();
        assert!(scene.get_stage().is_none());
        assert_eq!(scene.get_time(), TimeCode::default_time());
    }

    #[test]
    fn test_set_stage() {
        let scene = StageSceneIndex::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        scene.set_stage(stage);
        assert!(scene.get_stage().is_some());
    }

    #[test]
    fn test_set_time() {
        let scene = StageSceneIndex::new();
        let time = TimeCode::new(123.0);

        scene.set_time(time, false);
        assert_eq!(scene.get_time(), time);
    }

    #[test]
    fn test_get_prim_absolute_root_returns_stage_data_source() {
        let scene = StageSceneIndex::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        scene.set_stage(stage);

        // Item #1: absolute root should return DataSourceStage
        let prim = scene.get_prim(&Path::absolute_root());
        assert_eq!(prim.prim_type.as_str(), "");
        assert!(
            prim.data_source.is_some(),
            "Absolute root should return DataSourceStage"
        );
    }

    #[test]
    fn test_get_prim_no_stage() {
        let scene = StageSceneIndex::new();
        let prim = scene.get_prim(&Path::absolute_root());
        assert_eq!(prim.prim_type.as_str(), "");
        assert!(prim.data_source.is_none());
    }

    #[test]
    fn test_get_child_prim_paths() {
        let scene = StageSceneIndex::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        stage.define_prim("/World", "Xform").expect("define prim");
        scene.set_stage(stage);

        let children = scene.get_child_prim_paths(&Path::absolute_root());
        assert!(!children.is_empty());
    }

    #[test]
    fn test_get_child_prim_paths_subprim_is_leaf() {
        let scene = StageSceneIndex::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        scene.set_stage(stage);

        // Property paths should be treated as leaves
        let prop_path = Path::from_string("/World.myProp").unwrap();
        let children = scene.get_child_prim_paths(&prop_path);
        assert!(children.is_empty());
    }

    #[test]
    fn test_populated_paths_tracking() {
        let scene = StageSceneIndex::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        stage.define_prim("/World", "Xform").expect("define prim");
        stage
            .define_prim("/World/Cube", "Mesh")
            .expect("define prim");
        scene.set_stage(stage);

        let populated = scene.populated_paths.lock().expect("Lock poisoned");
        // Should have at least the root and the defined prims
        assert!(
            populated.len() >= 2,
            "Should track populated paths, got {}",
            populated.len()
        );
    }

    #[test]
    fn test_collect_imaging_subprims_no_adapter() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let entry = AdaptersEntry::empty();
        let subprims = collect_imaging_subprims(&prim, &entry);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].as_str().is_empty());
    }

    #[test]
    fn test_delete_prefix() {
        let mut map = HashMap::new();
        map.insert(
            Path::from_string("/World/A").unwrap(),
            vec![Token::new("x")],
        );
        map.insert(
            Path::from_string("/World/A/B").unwrap(),
            vec![Token::new("y")],
        );
        map.insert(Path::from_string("/Other").unwrap(), vec![Token::new("z")]);

        delete_prefix(&Path::from_string("/World/A").unwrap(), &mut map);

        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&Path::from_string("/Other").unwrap()));
    }

    #[test]
    fn test_stage_globals_asset_path_dependents() {
        let time = Arc::new(RwLock::new(TimeCode::default_time()));
        let globals = StageGlobalsImpl::new(time);

        globals.flag_as_asset_path_dependent(&Path::from_string("/World/Tex").unwrap());
        globals.flag_as_asset_path_dependent(&Path::from_string("/World/Tex.file").unwrap());
        globals.flag_as_asset_path_dependent(&Path::from_string("/Other").unwrap());

        // Remove under /World/Tex
        globals.remove_asset_path_dependents_under(&Path::from_string("/World/Tex").unwrap());

        let deps = globals.asset_path_dependents.read();
        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&Path::from_string("/Other").unwrap()));
    }
}
