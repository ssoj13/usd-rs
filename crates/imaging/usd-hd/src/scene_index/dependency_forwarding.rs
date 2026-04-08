//! HdDependencyForwardingSceneIndex - forwards dirty notices based on __dependencies schema.
//!
//! Port of pxr/imaging/hd/dependencyForwardingSceneIndex.{h,cpp}
//!
//! When a prim A depends on data from prim B (via HdDependenciesSchema), this
//! scene index forwards PrimsDirtied from B to A so downstream observers see
//! the invalidation.

use super::base::HdSceneIndexHandle;
use super::filtering::{
    FilteringObserverTarget, FilteringSceneIndexObserver, HdSingleInputFilteringSceneIndexBase,
};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use super::{HdSceneIndexBase, HdSceneIndexPrim, SdfPathVector, si_ref};
use crate::data_source::HdDataSourceBaseHandle;
use crate::data_source::HdRetainedTypedSampledDataSource;
use crate::data_source::{HdDataSourceLocator, HdDataSourceLocatorSet};
use crate::schema::HdDependenciesSchema;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Internal map types mirroring C++ structure
// ---------------------------------------------------------------------------

/// Locator pair stored per named dependency entry.
#[derive(Clone, Debug)]
struct LocatorsEntry {
    depended_on_data_source_locator: HdDataSourceLocator,
    affected_data_source_locator: HdDataSourceLocator,
}

/// Entry for a single affected prim within the depended-on map.
/// Mirrors C++ `_AffectedPrimDependencyEntry`.
#[derive(Default, Clone)]
struct AffectedPrimDependencyEntry {
    /// Keyed by dependency name (TfToken), value = locator pair.
    locators_entry_map: HashMap<Token, LocatorsEntry>,
    /// Mirrors C++ `flaggedForDeletion`.
    flagged_for_deletion: bool,
}

/// depended-on path -> { affected path -> AffectedPrimDependencyEntry }
type DependedOnPrimsAffectedPrimsMap =
    HashMap<SdfPath, HashMap<SdfPath, AffectedPrimDependencyEntry>>;

/// Entry for an affected prim's set of paths it depends on.
/// Mirrors C++ `_AffectedPrimToDependsOnPathsEntry`.
#[derive(Default, Clone)]
struct AffectedPrimToDependsOnPathsEntry {
    depends_on_paths: HashSet<SdfPath>,
    flagged_for_deletion: bool,
}

/// affected path -> AffectedPrimToDependsOnPathsEntry
type AffectedPrimToDependsOnPathsEntryMap = HashMap<SdfPath, AffectedPrimToDependsOnPathsEntry>;

// ---------------------------------------------------------------------------
// Cycle-detection node
// ---------------------------------------------------------------------------

/// Node key for visited-set cycle detection: (prim_path, locator).
#[derive(Clone, Debug, PartialEq, Eq)]
struct VisitedNode {
    prim_path: SdfPath,
    locator: HdDataSourceLocator,
}

impl Hash for VisitedNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.prim_path.hash(state);
        self.locator.hash(state);
    }
}

// ---------------------------------------------------------------------------
// HdDependencyForwardingSceneIndex
// ---------------------------------------------------------------------------

/// Dependency forwarding scene index.
///
/// Reads __dependencies from prims and forwards PrimsDirtied from depended-on
/// prims to affected prims.
///
/// Port of C++ `HdDependencyForwardingSceneIndex`.
pub struct HdDependencyForwardingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,

    /// dependedOn -> affected -> AffectedPrimDependencyEntry
    /// Mirrors C++ `_dependedOnPrimToDependentsMap`.
    depended_on_to_dependents: RwLock<DependedOnPrimsAffectedPrimsMap>,

    /// affected -> AffectedPrimToDependsOnPathsEntry
    /// Mirrors C++ `_affectedPrimToDependsOnPathsMap`.
    affected_to_depends_on: RwLock<AffectedPrimToDependsOnPathsEntryMap>,

    /// Paths whose dependent-entries may have been flagged.
    /// Mirrors C++ `_potentiallyDeletedDependedOnPaths`.
    potentially_deleted_depended_on: RwLock<HashSet<SdfPath>>,

    /// Paths whose affected-entry may have been flagged.
    /// Mirrors C++ `_potentiallyDeletedAffectedPaths`.
    potentially_deleted_affected: RwLock<HashSet<SdfPath>>,

    /// When true, caller must call remove_deleted_entries() manually.
    manual_garbage_collect: RwLock<bool>,

    /// Observer registered on the input scene — kept alive so the weak ref in
    /// FilteringSceneIndexObserver doesn't dangle.
    input_observer: RwLock<Option<HdSceneIndexObserverHandle>>,
}

impl HdDependencyForwardingSceneIndex {
    /// Creates a new dependency forwarding scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        let arc = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            depended_on_to_dependents: RwLock::new(HashMap::new()),
            affected_to_depends_on: RwLock::new(HashMap::new()),
            potentially_deleted_depended_on: RwLock::new(HashSet::new()),
            potentially_deleted_affected: RwLock::new(HashSet::new()),
            manual_garbage_collect: RwLock::new(false),
            input_observer: RwLock::new(None),
        }));

        // Register as observer on the input scene so we receive dirty/add/remove notifications.
        {
            let weak: std::sync::Weak<RwLock<dyn FilteringObserverTarget>> =
                Arc::downgrade(&(arc.clone() as Arc<RwLock<dyn FilteringObserverTarget>>));
            let observer_handle: HdSceneIndexObserverHandle =
                Arc::new(FilteringSceneIndexObserver::new(weak));
            let s = arc.read();
            if let Some(input) = s.base.get_input_scene() {
                {
                    let input_lock = input.write();
                    input_lock.add_observer(observer_handle.clone());
                }
            }
            drop(s);
            arc.read().input_observer.write().replace(observer_handle);
        }

        arc
    }

    /// Enable manual garbage collection (for unit tests).
    pub fn set_manual_garbage_collect(&self, manual: bool) {
        *self.manual_garbage_collect.write() = manual;
    }

    /// Remove flagged entries from both maps.
    ///
    /// Optional output vecs receive the paths that were actually removed —
    /// used by unit tests to validate bookkeeping.
    ///
    /// Port of C++ `RemoveDeletedEntries`.
    pub fn remove_deleted_entries(
        &self,
        mut removed_affected: Option<&mut Vec<SdfPath>>,
        mut removed_depended_on: Option<&mut Vec<SdfPath>>,
    ) {
        // Take a snapshot of the pending sets and clear them.
        let dep_on_set: HashSet<SdfPath> = {
            let mut g = self.potentially_deleted_depended_on.write();
            std::mem::take(&mut *g)
        };
        let aff_set: HashSet<SdfPath> = {
            let mut g = self.potentially_deleted_affected.write();
            std::mem::take(&mut *g)
        };

        // Phase 1 — clean up depended-on entries.
        let mut dep_map = self.depended_on_to_dependents.write();
        for depended_on_path in &dep_on_set {
            let Some(affected_map) = dep_map.get_mut(depended_on_path) else {
                continue;
            };

            // Remove affected entries that are flagged for deletion.
            affected_map.retain(|_, entry| !entry.flagged_for_deletion);

            // If all affected entries are gone, remove the depended-on entry.
            if affected_map.is_empty() {
                if let Some(out) = removed_depended_on.as_deref_mut() {
                    out.push(depended_on_path.clone());
                }
                dep_map.remove(depended_on_path);
            }
        }
        drop(dep_map);

        // Phase 2 — clean up affected-prim entries.
        let mut aff_map = self.affected_to_depends_on.write();
        for affected_path in &aff_set {
            if let Some(entry) = aff_map.get(affected_path) {
                if entry.flagged_for_deletion {
                    aff_map.remove(affected_path);
                    if let Some(out) = removed_affected.as_deref_mut() {
                        out.push(affected_path.clone());
                    }
                }
            }
        }
    }

    fn get_input_scene(&self) -> Option<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned()
    }

    /// Re-read __dependencies for prim_path and populate the reverse maps.
    ///
    /// `sender` is an optional already-unlocked reference to the input scene,
    /// used when called from an observer callback to avoid re-acquiring the
    /// input scene's RwLock (which is already held by the caller's write lock,
    /// causing a deadlock if we tried to acquire it again).
    ///
    /// Port of C++ `_UpdateDependencies`.
    fn update_dependencies(&self, prim_path: &SdfPath, sender: Option<&dyn HdSceneIndexBase>) {
        // When a sender reference is available (observer callback), use it directly.
        // Otherwise, lock the input scene handle.
        let prim = if let Some(s) = sender {
            s.get_prim(prim_path)
        } else {
            let Some(input) = self.get_input_scene() else {
                return;
            };
            si_ref(&input).get_prim(prim_path)
        };
        let Some(ref prim_ds) = prim.data_source else {
            return;
        };

        let deps_schema = HdDependenciesSchema::get_from_parent(prim_ds);
        if !deps_schema.is_defined() {
            // No __dependencies on this prim — skip; do NOT insert into
            // affected_to_depends_on so subsequent get_prim calls will
            // retry (lazy population).
            return;
        }

        // Token names for the dependency schema fields.
        // We read them directly via as_any() downcast because HdSchema::get_typed()
        // cannot downcast fat-pointer trait objects (Arc<dyn TraitA> -> Arc<dyn TraitB>).
        let path_tok = usd_tf::Token::new("dependedOnPrimPath");
        let dep_loc_tok = usd_tf::Token::new("dependedOnDataSourceLocator");
        let aff_loc_tok = usd_tf::Token::new("affectedDataSourceLocator");

        /// Extract a `T` value from a data source handle by downcasting to the concrete retained type.
        fn read_retained<T>(ds: &crate::data_source::HdDataSourceBaseHandle) -> Option<T>
        where
            T: Clone + Send + Sync + std::fmt::Debug + 'static,
            HdRetainedTypedSampledDataSource<T>: crate::data_source::HdTypedSampledDataSource<T>,
        {
            use crate::data_source::HdTypedSampledDataSource as Typed;
            ds.as_any()
                .downcast_ref::<HdRetainedTypedSampledDataSource<T>>()
                .map(|r| Typed::get_typed_value(r, 0.0))
        }

        // Read the __dependencies container directly — a container whose children are named
        // dependency containers, each holding dependedOnPrimPath / locator fields.
        let deps_container = match deps_schema.get_container() {
            Some(c) => c.clone(),
            None => return,
        };

        let mut aff_map = self.affected_to_depends_on.write();
        let mut dep_map = self.depended_on_to_dependents.write();

        // Presence (even if empty set) means we have been here.
        let entry = aff_map.entry(prim_path.clone()).or_default();
        entry.flagged_for_deletion = false;
        entry.depends_on_paths.clear();

        for entry_name in deps_container.get_names() {
            // Each child of __dependencies is itself a container (the individual dependency entry).
            use crate::data_source::cast_to_container;
            let dep_container = match deps_container
                .get(&entry_name)
                .and_then(|ds| cast_to_container(&ds))
            {
                Some(c) => c,
                None => continue,
            };

            let mut depended_on_prim_path: usd_sdf::Path = dep_container
                .get(&path_tok)
                .and_then(|ds| read_retained::<usd_sdf::Path>(&ds))
                .unwrap_or_default();

            let depended_on_locator: HdDataSourceLocator = dep_container
                .get(&dep_loc_tok)
                .and_then(|ds| read_retained::<HdDataSourceLocator>(&ds))
                .unwrap_or_default();

            let affected_locator: HdDataSourceLocator = dep_container
                .get(&aff_loc_tok)
                .and_then(|ds| read_retained::<HdDataSourceLocator>(&ds))
                .unwrap_or_default();

            // Empty path means self-dependency.
            if depended_on_prim_path.is_empty() {
                depended_on_prim_path = prim_path.clone();
            }

            entry.depends_on_paths.insert(depended_on_prim_path.clone());

            let reverse_affected = dep_map.entry(depended_on_prim_path).or_default();
            let reverse_entry = reverse_affected.entry(prim_path.clone()).or_default();
            reverse_entry.flagged_for_deletion = false;
            reverse_entry.locators_entry_map.insert(
                entry_name,
                LocatorsEntry {
                    depended_on_data_source_locator: depended_on_locator,
                    affected_data_source_locator: affected_locator,
                },
            );
        }
    }

    /// Flag the dependency entries for prim_path as deleted (lazy cleanup).
    ///
    /// Port of C++ `_ClearDependencies`.
    fn clear_dependencies(&self, prim_path: &SdfPath) {
        let mut aff_map = self.affected_to_depends_on.write();
        let Some(entry) = aff_map.get_mut(prim_path) else {
            return;
        };

        // Flag the affected-prim entry for deletion.
        entry.flagged_for_deletion = true;
        let depends_on_paths = entry.depends_on_paths.clone();
        drop(aff_map);

        self.potentially_deleted_affected
            .write()
            .insert(prim_path.clone());

        // Flag the corresponding entries in the depended-on map.
        let mut dep_map = self.depended_on_to_dependents.write();
        for depended_on_path in &depends_on_paths {
            let Some(affected_map) = dep_map.get_mut(depended_on_path) else {
                continue;
            };
            if let Some(aff_entry) = affected_map.get_mut(prim_path) {
                aff_entry.flagged_for_deletion = true;
                self.potentially_deleted_depended_on
                    .write()
                    .insert(depended_on_path.clone());
            }
        }
    }

    /// Reset all dependency tracking state.
    ///
    /// Port of C++ `_ResetDependencies`.
    fn reset_dependencies(&self) {
        if *self.manual_garbage_collect.read() {
            // Flag everything for deletion so remove_deleted_entries can report it.
            {
                let mut dep_map = self.depended_on_to_dependents.write();
                let mut pot_dep = self.potentially_deleted_depended_on.write();
                for (path, aff_map) in dep_map.iter_mut() {
                    pot_dep.insert(path.clone());
                    for entry in aff_map.values_mut() {
                        entry.flagged_for_deletion = true;
                    }
                }
            }
            {
                let mut aff_map = self.affected_to_depends_on.write();
                let mut pot_aff = self.potentially_deleted_affected.write();
                for (path, entry) in aff_map.iter_mut() {
                    pot_aff.insert(path.clone());
                    entry.flagged_for_deletion = true;
                }
            }
        } else {
            self.depended_on_to_dependents.write().clear();
            self.affected_to_depends_on.write().clear();
            self.potentially_deleted_depended_on.write().clear();
            self.potentially_deleted_affected.write().clear();
        }
    }

    /// Propagate dirtiness from prim_path to its dependents.
    ///
    /// Port of C++ `_PrimDirtied`.
    fn prim_dirtied(
        &self,
        prim_path: &SdfPath,
        source_locator_set: &HdDataSourceLocatorSet,
        visited: &mut HashSet<VisitedNode>,
        more_dirtied: &mut Vec<DirtiedPrimEntry>,
        rebuild_deps: &mut HashSet<SdfPath>,
    ) {
        let deps_loc = HdDependenciesSchema::get_default_locator();

        let dep_map = self.depended_on_to_dependents.read();
        let Some(affected_map) = dep_map.get(prim_path) else {
            return;
        };

        // Collect what needs dirtying before recursing (can't recurse while holding read lock).
        let mut to_dirty: Vec<(SdfPath, HdDataSourceLocatorSet)> = Vec::new();

        for (affected_prim_path, dep_entry) in affected_map.iter() {
            let mut affected_locators = HdDataSourceLocatorSet::new();

            for loc_entry in dep_entry.locators_entry_map.values() {
                if !source_locator_set
                    .intersects_locator(&loc_entry.depended_on_data_source_locator)
                {
                    continue;
                }

                let node = VisitedNode {
                    prim_path: affected_prim_path.clone(),
                    locator: loc_entry.affected_data_source_locator.clone(),
                };
                if visited.contains(&node) {
                    continue;
                }
                visited.insert(node);
                affected_locators.insert(loc_entry.affected_data_source_locator.clone());

                let aff_loc = &loc_entry.affected_data_source_locator;
                if aff_loc == &deps_loc {
                    rebuild_deps.insert(affected_prim_path.clone());
                } else if aff_loc.intersects(&deps_loc) {
                    let deps_node = VisitedNode {
                        prim_path: affected_prim_path.clone(),
                        locator: deps_loc.clone(),
                    };
                    if !visited.contains(&deps_node) {
                        visited.insert(deps_node);
                        rebuild_deps.insert(affected_prim_path.clone());
                    }
                }
            }

            if !affected_locators.is_empty() {
                to_dirty.push((affected_prim_path.clone(), affected_locators));
            }
        }
        drop(dep_map);

        for (affected_path, affected_locators) in to_dirty {
            more_dirtied.push(DirtiedPrimEntry {
                prim_path: affected_path.clone(),
                dirty_locators: affected_locators.clone(),
            });
            self.prim_dirtied(
                &affected_path,
                &affected_locators,
                visited,
                more_dirtied,
                rebuild_deps,
            );
        }
    }
}

impl HdSceneIndexBase for HdDependencyForwardingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // Lazy-populate dependencies on first access.
        {
            let aff = self.affected_to_depends_on.read();
            if !aff.contains_key(prim_path) {
                drop(aff);
                // No sender available here — caller is not inside an observer callback.
                self.update_dependencies(prim_path, None);
            }
        }

        if let Some(input) = self.get_input_scene() {
            return si_ref(&input).get_prim(prim_path);
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: super::observer::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &super::observer::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdDependencyForwardingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdDependencyForwardingSceneIndex {
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut rebuild: HashSet<SdfPath> = HashSet::new();
        let mut additional_dirtied: Vec<DirtiedPrimEntry> = Vec::new();
        let mut visited: HashSet<VisitedNode> = HashSet::new();

        for entry in entries {
            // Clear stale deps for this prim if any, then re-read from the
            // newly-added data source. This matches C++ _PrimsAdded which
            // calls _ClearDependencies then _UpdateDependencies for every entry.
            self.clear_dependencies(&entry.prim_path);
            self.update_dependencies(&entry.prim_path, Some(sender));

            // Propagate dirty to any prims that depended on this prim.
            let dep_map = self.depended_on_to_dependents.read();
            if let Some(affected_map) = dep_map.get(&entry.prim_path) {
                let mut to_dirty: Vec<(SdfPath, HdDataSourceLocatorSet)> = Vec::new();
                for (affected_path, dep_entry) in affected_map.iter() {
                    if *affected_path == entry.prim_path {
                        continue;
                    }
                    let mut locs = HdDataSourceLocatorSet::new();
                    for loc_entry in dep_entry.locators_entry_map.values() {
                        locs.insert(loc_entry.affected_data_source_locator.clone());
                    }
                    if !locs.is_empty() {
                        to_dirty.push((affected_path.clone(), locs));
                    }
                }
                drop(dep_map);
                for (affected_path, locs) in to_dirty {
                    additional_dirtied.push(DirtiedPrimEntry {
                        prim_path: affected_path.clone(),
                        dirty_locators: locs.clone(),
                    });
                    self.prim_dirtied(
                        &affected_path,
                        &locs,
                        &mut visited,
                        &mut additional_dirtied,
                        &mut rebuild,
                    );
                }
            } else {
                drop(dep_map);
            }
        }

        for p in &rebuild {
            self.clear_dependencies(p);
            self.update_dependencies(p, Some(sender));
        }

        if !*self.manual_garbage_collect.read() {
            self.remove_deleted_entries(None, None);
        }

        self.base.forward_prims_added(self, entries);

        if !additional_dirtied.is_empty() {
            let dirty_entries = additional_dirtied.clone();
            // These derived dirties originate from this filtering scene index, so
            // downstream observers must see `self` as the sender just like
            // `_SendPrimsDirtied(this, ...)` in OpenUSD.
            self.base.base().send_prims_dirtied(self, &dirty_entries);
        }
    }

    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Fast path: if root is removed, reset everything.
        if let Some(first) = entries.first() {
            if first.prim_path.is_absolute_root_path() {
                self.reset_dependencies();
                self.base.forward_prims_removed(self, entries);
                return;
            }
        }

        let mut visited: HashSet<VisitedNode> = HashSet::new();
        let mut additional_dirtied: Vec<DirtiedPrimEntry> = Vec::new();
        let mut rebuild: HashSet<SdfPath> = HashSet::new();

        // Flag affected prims that are being removed.
        {
            let mut aff_map = self.affected_to_depends_on.write();
            let mut pot_aff = self.potentially_deleted_affected.write();
            for (aff_path, aff_entry) in aff_map.iter_mut() {
                for removed in entries {
                    if aff_path.has_prefix(&removed.prim_path) {
                        aff_entry.flagged_for_deletion = true;
                        pot_aff.insert(aff_path.clone());
                        break;
                    }
                }
            }
        }

        // If we flagged every affected prim, reset and early-out.
        {
            let aff_map = self.affected_to_depends_on.read();
            let pot_aff = self.potentially_deleted_affected.read();
            let total = aff_map.len();
            let flagged = pot_aff.len();
            if flagged > 0 && flagged == total {
                drop(aff_map);
                drop(pot_aff);
                self.reset_dependencies();
                self.base.forward_prims_removed(self, entries);
                return;
            }
        }

        // Walk depended-on map: flag dead affected entries and emit dirty
        // for entries whose depended-on prim was removed but the affected prim lives.
        let dep_snapshot: Vec<(SdfPath, Vec<(SdfPath, bool, Vec<LocatorsEntry>)>)> = {
            let dep_map = self.depended_on_to_dependents.read();
            dep_map
                .iter()
                .map(|(dep_path, aff_map)| {
                    let entries_snap: Vec<_> = aff_map
                        .iter()
                        .map(|(aff_path, entry)| {
                            (
                                aff_path.clone(),
                                entry.flagged_for_deletion,
                                entry.locators_entry_map.values().cloned().collect(),
                            )
                        })
                        .collect();
                    (dep_path.clone(), entries_snap)
                })
                .collect()
        };

        for (dep_path, affected_entries) in dep_snapshot {
            for (aff_path, already_flagged, loc_entries) in &affected_entries {
                // If affected prim is being removed, flag its entry.
                let aff_is_removed = entries.iter().any(|e| aff_path.has_prefix(&e.prim_path));
                if aff_is_removed {
                    let mut dep_map = self.depended_on_to_dependents.write();
                    if let Some(aff_map) = dep_map.get_mut(&dep_path) {
                        if let Some(entry) = aff_map.get_mut(aff_path) {
                            entry.flagged_for_deletion = true;
                        }
                    }
                    self.potentially_deleted_depended_on
                        .write()
                        .insert(dep_path.clone());
                    continue;
                }

                if *already_flagged {
                    continue;
                }

                // If the depended-on prim itself is being removed, send dirty to affected.
                let dep_is_removed = entries.iter().any(|e| dep_path.has_prefix(&e.prim_path));
                if dep_is_removed {
                    let mut locs = HdDataSourceLocatorSet::new();
                    for loc_entry in loc_entries {
                        locs.insert(loc_entry.affected_data_source_locator.clone());
                    }
                    if !locs.is_empty() {
                        additional_dirtied.push(DirtiedPrimEntry {
                            prim_path: aff_path.clone(),
                            dirty_locators: locs.clone(),
                        });
                        self.prim_dirtied(
                            aff_path,
                            &locs,
                            &mut visited,
                            &mut additional_dirtied,
                            &mut rebuild,
                        );
                    }
                }
            }
        }

        for p in &rebuild {
            self.clear_dependencies(p);
            self.update_dependencies(p, Some(sender));
        }

        if !*self.manual_garbage_collect.read() {
            self.remove_deleted_entries(None, None);
        }

        self.base.forward_prims_removed(self, entries);

        if !additional_dirtied.is_empty() {
            let dirty_entries = additional_dirtied.clone();
            // These derived dirties are synthesized here after dependency
            // analysis; preserve the filtering scene-index sender contract.
            self.base.base().send_prims_dirtied(self, &dirty_entries);
        }
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let deps_loc = HdDependenciesSchema::get_default_locator();
        let mut visited: HashSet<VisitedNode> = HashSet::new();
        let mut additional: Vec<DirtiedPrimEntry> = Vec::new();
        let mut rebuild: HashSet<SdfPath> = HashSet::new();

        for entry in entries {
            // If __dependencies locator was dirtied, schedule dependency rebuild.
            if entry.dirty_locators.intersects_locator(&deps_loc) {
                let node = VisitedNode {
                    prim_path: entry.prim_path.clone(),
                    locator: deps_loc.clone(),
                };
                if !visited.contains(&node) {
                    visited.insert(node);
                    rebuild.insert(entry.prim_path.clone());
                }
            }
            self.prim_dirtied(
                &entry.prim_path,
                &entry.dirty_locators,
                &mut visited,
                &mut additional,
                &mut rebuild,
            );
        }

        for p in &rebuild {
            self.clear_dependencies(p);
            self.update_dependencies(p, Some(sender));
        }

        if !*self.manual_garbage_collect.read() {
            self.remove_deleted_entries(None, None);
        }

        if entries.len() >= 500 || additional.len() >= 500 || rebuild.len() >= 500 {
            let first_path = entries
                .first()
                .map(|entry| entry.prim_path.to_string())
                .unwrap_or_else(|| "<none>".to_string());
            log::info!(
                "[dependency_forwarding] on_prims_dirtied in={} additional={} rebuild={} first={}",
                entries.len(),
                additional.len(),
                rebuild.len(),
                first_path
            );
        }

        if additional.is_empty() {
            self.base.forward_prims_dirtied(self, entries);
        } else {
            let mut combined: Vec<DirtiedPrimEntry> = entries.to_vec();
            combined.extend(additional);
            self.base.forward_prims_dirtied(self, &combined);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
